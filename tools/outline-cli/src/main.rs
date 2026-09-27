//! 命令行符号大纲：对源文件跑 zed 的 tree-sitter `outline.scm` 查询，
//! 输出 `OutlineItem`（文本 / 签名 / 行号 / 起止范围 / 模块关系 …）。
//!
//! 语言默认按路径自动识别（走 `LanguageRegistry::language_for_file_path`，
//! 与 zed 编辑器里用的是同一套 `LanguageMatcher`），可用 `--lang` 覆盖。

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context as _, Result, bail};
use clap::{Parser, ValueEnum};
use futures::FutureExt as _;
use gpui::{AppContext as _, TestAppContext};
use language::{Buffer, LanguageRegistry, LoadedLanguage, Point};

#[derive(Parser)]
#[command(name = "outline", about = "打印源文件的符号大纲")]
struct Args {
    /// 输入文件（可多个）
    paths: Vec<PathBuf>,

    /// 显式指定语言（语言名或扩展名），覆盖按路径的自动识别
    #[arg(short, long)]
    lang: Option<String>,

    /// 输出格式。JSON 恒为嵌套结构；text 的嵌套表达见 --nesting
    #[arg(short, long, value_enum, default_value_t = Format::Json)]
    format: Format,

    /// 要输出的字段；逗号分隔。`all` = 全部，不给 = 最少内容（text,line）。
    /// 可选：text name signature line range selection body annotation relations all
    /// 例：--fields text,line,relations
    // 不用 `num_args = 1..`：那会把后面的位置参数（文件路径）一并吃掉。
    #[arg(long, value_delimiter = ',')]
    fields: Vec<Field>,

    /// 跳过前 N 个符号（在把所有文件的符号拼成一个扁平列表之后生效）
    #[arg(long, default_value_t = 0)]
    offset: usize,

    /// 最多输出 N 个符号；不给就是全部。与 --offset 配合可翻页。
    #[arg(long)]
    limit: Option<usize>,

    /// text 模式下如何表达嵌套：缩进（默认）或显式深度数字
    #[arg(long, value_enum, default_value_t = Nesting::Indent)]
    nesting: Nesting,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Json,
}

/// text 模式的嵌套表达方式。JSON 模式恒为嵌套结构（`children`），与此无关。
#[derive(Clone, Copy, ValueEnum)]
enum Nesting {
    /// 每层缩进两格
    Indent,
    /// 每行前缀 `[深度]`
    Number,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Field {
    /// 全部字段
    All,
    /// 大纲文本（zed 查询拼装：上下文捕获 + 符号名，如 `impl Foo`）
    Text,
    /// 仅符号名（由 name_ranges 从 text 切出）
    Name,
    /// 源码首行原文（去尾部 `{`），如 `fn main()` / `pub fn foo() -> u32`
    Signature,
    /// 1-based 起始行 —— agent 拿它做 targeted read
    Line,
    /// [[start_row, start_col], [end_row, end_col]]，0-based
    Range,
    Selection,
    /// 函数体 / 声明体的行范围 [start_row, end_row]
    Body,
    /// 注解（`#[test]`、doc 注释）范围
    Annotation,
    /// 模块关系：is_import / is_exported / is_public
    Relations,
}

/// 不给 `--fields` 时的最小集合 —— 刚够 agent 认符号并定位。
const MINIMAL: [Field; 2] = [Field::Text, Field::Line];

fn resolve_fields(fields: &[Field]) -> Vec<Field> {
    if fields.is_empty() {
        MINIMAL.to_vec()
    } else if fields.contains(&Field::All) {
        vec![
            Field::Text,
            Field::Name,
            Field::Signature,
            Field::Line,
            Field::Range,
            Field::Selection,
            Field::Body,
            Field::Annotation,
            Field::Relations,
        ]
    } else {
        fields.to_vec()
    }
}

/// 模块关系。**启发式**：从 signature 前缀判断。
///
/// - `is_public`：带可见性修饰（rust `pub`、java/kotlin `public`）
/// - `is_exported`：被导出（js/ts `export`；rust 的 `pub` 视为导出）
/// - `is_import`：是导入语句（`use` / `import` 开头）。
///   注意 zed 的 rust outline.scm **不抓 `use`**，所以 rust 下恒为 false；
///   抓 import 的语言（如部分 js/ts 查询）才会出现 true。
#[derive(serde::Serialize, Clone, Copy)]
struct Relations {
    is_import: bool,
    is_exported: bool,
    is_public: bool,
}

impl Relations {
    fn from_signature(signature: &str) -> Self {
        let s = signature.trim_start();
        // `pub` / `pub(crate)` —— 按 word 边界匹配，避免把 `publication` 误判进来。
        let word = |prefix: &str| {
            s.starts_with(prefix)
                && s[prefix.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !c.is_alphanumeric() && c != '_')
        };
        let is_public = word("pub") || word("public");
        let is_exported = word("export") || is_public;
        let is_import = word("use") || word("import");
        Self {
            is_import,
            is_exported,
            is_public,
        }
    }
}

/// 一个符号。JSON 里嵌套（`children`）；text 里用缩进或 `[深度]` 表达层级。
#[derive(serde::Serialize)]
struct Node {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    /// 1-based 起始行 —— agent 拿它做 targeted read。
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
    /// [[start_row, start_col], [end_row, end_col]]，0-based。
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<[[u32; 2]; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selection: Option<[[u32; 2]; 2]>,
    /// [start_row, end_row]，0-based。
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<[u32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotation: Option<[[u32; 2]; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relations: Option<Relations>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<Node>,
}

/// 一个文件的输出：路径 + 嵌套符号树。
#[derive(serde::Serialize)]
struct FileOutline {
    path: String,
    items: Vec<Node>,
}

/// 扁平中间态：树化的原料（depth + 已按 fields 裁剪好的节点）。
struct FlatNode {
    depth: usize,
    node: Node,
}

impl FlatNode {
    fn new(source: &str, item: &language::OutlineItem<Point>, fields: &[Field]) -> Self {
        let has = |f: Field| fields.contains(&f);
        let text = item.text.to_string();
        // `name_ranges` 标出 text 里哪些片段是「符号名本身」，拼起来就是干净的符号名。
        let name: String = item
            .name_ranges
            .iter()
            .filter_map(|range| text.get(range.clone()))
            .collect();

        // signature：源码里 item 起始行的原文，去掉行尾的 `{`。
        // （zed 的 text 是查询拼装的，`fn main` 少了 `()`；ast-grep 的 signature
        // 是首行原文 `fn main()` —— 对齐它。）
        let signature = source
            .lines()
            .nth(item.range.start.row as usize)
            .map(|line| line.trim().trim_end_matches('{').trim().to_string());

        let relations = has(Field::Relations).then(|| {
            signature
                .as_deref()
                .map_or(Relations::from_signature(""), Relations::from_signature)
        });

        Self {
            depth: item.depth,
            node: Node {
                text: has(Field::Text).then_some(text),
                name: has(Field::Name).then_some(name),
                signature: has(Field::Signature).then(|| signature.clone().unwrap_or_default()),
                line: has(Field::Line).then_some(item.range.start.row + 1),
                range: has(Field::Range)
                    .then(|| [to_pair(item.range.start), to_pair(item.range.end)]),
                selection: has(Field::Selection).then(|| {
                    [
                        to_pair(item.selection_range.start),
                        to_pair(item.selection_range.end),
                    ]
                }),
                body: has(Field::Body).then(|| {
                    [
                        item.body_range
                            .as_ref()
                            .map_or(item.range.start.row, |r| r.start.row),
                        item.body_range
                            .as_ref()
                            .map_or(item.range.end.row, |r| r.end.row),
                    ]
                }),
                annotation: has(Field::Annotation).then(|| {
                    [
                        to_pair(
                            item.annotation_range
                                .as_ref()
                                .map_or(item.range.start, |r| r.start),
                        ),
                        to_pair(
                            item.annotation_range
                                .as_ref()
                                .map_or(item.range.start, |r| r.end),
                        ),
                    ]
                }),
                relations,
                children: Vec::new(),
            },
        }
    }
}

/// 把（同一文件内的）扁平列表按 depth 重建为嵌套树。
///
/// 栈里是「当前路径上的父节点」，栈顶是最内层。遇到更浅的 depth 就把
/// 完成的子树一路弹出、挂到新栈顶的 `children` 上 —— 弹出的是**整棵子树**，
/// 不是兄弟列表（存节点列表的话弹出后 extend 会把层级拍平成兄弟）。
fn nest(flat: Vec<FlatNode>) -> Vec<Node> {
    let mut roots: Vec<Node> = Vec::new();
    let mut stack: Vec<Node> = Vec::new();

    for entry in flat {
        let depth = entry.depth;
        while stack.len() > depth {
            let done = stack.pop().unwrap();
            match stack.last_mut() {
                Some(parent) => parent.children.push(done),
                None => roots.push(done),
            }
        }
        stack.push(entry.node);
    }
    while let Some(done) = stack.pop() {
        match stack.last_mut() {
            Some(parent) => parent.children.push(done),
            None => roots.push(done),
        }
    }
    roots
}

/// 按路径自动识别；给了 `--lang` 就用它（接受语言名如 `rust`，或扩展名如 `rs`）。
async fn resolve_language(
    registry: Arc<LanguageRegistry>,
    path: &Path,
    lang: Option<&str>,
) -> Result<Arc<language::Language>> {
    if let Some(name) = lang {
        return registry
            .language_for_name_or_extension(name)
            .await
            .with_context(|| format!("--lang {name:?} 没有匹配的语言"));
    }

    // 自动识别：与 zed 编辑器同一套 `LanguageMatcher`。
    // 认不出来要明确报错 —— 否则会静默输出一个空大纲，看起来像"这个文件没有符号"。
    let language_id = registry.language_for_file_path(path).with_context(|| {
        format!(
            "无法从路径识别 {} 的语言，请用 --lang 指定（如 --lang rust）",
            path.display()
        )
    })?;
    registry
        .load_language(language_id)
        .await
        .with_context(|| format!("加载 {} 的语言失败", path.display()))?
}

/// 只注册语法树与查询，**不带任何 LSP adapter** —— outline 用不到它们，
/// 而 `languages::init()` 会顺带把 bash/go/python/… 的 LSP adapter 全建一遍。
fn register_all_grammars(registry: &Arc<LanguageRegistry>) {
    // 先把 tree-sitter 语法注册进 registry —— 少了这一步，
    // 加载语言时会报 "no such grammar rust"（语言认出来了但语法没登记）。
    registry.register_native_grammars(grammars::native_grammars());

    for (name, _grammar) in grammars::native_grammars() {
        let config = grammars::load_config(name);
        registry.register_language(
            config.name.clone(),
            config.grammar.clone(),
            config.matcher.clone(),
            config.hidden,
            None,
            Arc::new(move || {
                let config = config.clone();
                async move {
                    Ok(LoadedLanguage {
                        config,
                        queries: grammars::load_queries(name),
                        context_provider: None,
                        toolchain_provider: None,
                        manifest_name: None,
                    })
                }
                .boxed()
            }),
        );
    }
}

fn to_pair(point: Point) -> [u32; 2] {
    [point.row, point.column]
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.paths.is_empty() {
        bail!("至少需要一个输入文件；用 --help 看用法");
    }
    let fields = resolve_fields(&args.fields);

    let mut cx = TestAppContext::single();
    let registry = Arc::new(LanguageRegistry::new(cx.background_executor.clone()));
    // `block_test` 在 ForegroundExecutor 上（不是 Background）：它驱动测试调度器，
    // 让 `load_language_for_file_path` 这类异步能在同步的 main 里跑完。
    let foreground = cx.foreground_executor.clone();
    register_all_grammars(&registry);

    let mut flat: Vec<(String, FlatNode)> = Vec::new();
    for path in &args.paths {
        let text = fs::read_to_string(path).with_context(|| format!("读取 {}", path.display()))?;

        let language =
            foreground.block_test(resolve_language(registry.clone(), path, args.lang.as_deref()))?;

        let buffer = cx.new(|cx| {
            let mut buffer = Buffer::local(text.clone(), cx);
            // 同步解析：命令行里不等异步解析完成。
            buffer.set_sync_parse_timeout(None);
            buffer.set_language(Some(language), cx);
            buffer
        });
        // `set_sync_parse_timeout` 只管后续编辑；首次解析仍是异步的，
        // 必须等解析空闲，否则语法树为空、outline 也为空。
        let idle = buffer.read_with(&cx, |buffer, _| buffer.parsing_idle());
        foreground.block_test(idle);

        let items = buffer.read_with(&cx, |buffer, _| {
            let snapshot = buffer.snapshot();
            snapshot.outline_items_as_points_containing(0..snapshot.len(), true, None)
        });

        if items.is_empty() {
            // 别静默：空结果通常意味着"这个语言没有 outline 查询"（比如被识别成
            // Plain Text），而不是"这个文件没有符号"。
            eprintln!(
                "{}: 没有取到符号（语言可能没有 outline 查询，可用 --lang 指定）",
                path.display()
            );
        }
        let path = path.display().to_string();
        for item in items {
            flat.push((path.clone(), FlatNode::new(&text, &item, &fields)));
        }
    }

    // 翻页：对扁平化后的总列表切窗，窗口内的深度关系再重建为嵌套。
    let total = flat.len();
    let start = args.offset.min(total);
    let end = start + args.limit.unwrap_or(total - start).min(total - start);
    let truncated = end < total;
    let window: Vec<(String, FlatNode)> = flat
        .into_iter()
        .skip(start)
        .take(end - start)
        .collect();

    if args.offset > 0 || args.limit.is_some() {
        // 翻页时告知全貌，agent 才知道还有没有下一页。
        eprintln!(
            "显示 {}-{} / 共 {} 条{}",
            total.min(start + 1),
            end,
            total,
            if truncated {
                format!("（还有 {} 条，用 --offset {} 翻页）", total - end, end)
            } else {
                String::new()
            }
        );
    }

    match args.format {
        Format::Json => {
            // 按文件分组（保持顺序），组内重建嵌套。
            let mut files: Vec<(String, Vec<FlatNode>)> = Vec::new();
            for (path, node) in window {
                match files.last_mut() {
                    Some((p, nodes)) if *p == path => nodes.push(node),
                    _ => files.push((path, vec![node])),
                }
            }
            let out: Vec<FileOutline> = files
                .into_iter()
                .map(|(path, nodes)| FileOutline {
                    path,
                    items: nest(nodes),
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&out)?)
        }
        Format::Text => print_text(&window, &fields, args.nesting),
    }
    Ok(())
}

fn print_text(window: &[(String, FlatNode)], fields: &[Field], nesting: Nesting) {
    let has = |f: Field| fields.contains(&f);
    let mut current_path = String::new();
    for (path, entry) in window {
        if path != &current_path {
            current_path = path.clone();
            println!("{}", current_path);
        }
        let node = &entry.node;
        let line = match nesting {
            Nesting::Indent => "  ".repeat(entry.depth + 1),
            Nesting::Number => format!("  [{}]", entry.depth),
        };
        let mut parts: Vec<String> = Vec::new();
        if let Some(text) = &node.text {
            parts.push(text.clone());
        }
        if let Some(name) = &node.name {
            if !name.is_empty() && Some(name) != node.text.as_ref() {
                parts.push(format!("({name})"));
            }
        }
        if let Some(signature) = &node.signature {
            if Some(signature) != node.text.as_ref() {
                parts.push(format!("«{signature}»"));
            }
        }
        if let Some(line) = node.line {
            parts.push(format!("L{line}"));
        }
        if let Some(range) = node.range {
            parts.push(format!(
                "{:?}..{:?}",
                [range[0][0] + 1, range[0][1] + 1],
                [range[1][0] + 1, range[1][1] + 1]
            ));
        }
        if let Some(body) = node.body {
            parts.push(format!("body {}..{}", body[0] + 1, body[1] + 1));
        }
        if let Some(annotation) = node.annotation {
            parts.push(format!(
                "annotation {:?}",
                [annotation[0][0] + 1, annotation[0][1] + 1]
            ));
        }
        if let Some(relations) = &node.relations {
            let mut flags: Vec<&str> = Vec::new();
            if relations.is_import {
                flags.push("import");
            }
            if relations.is_exported {
                flags.push("exported");
            }
            if relations.is_public {
                flags.push("public");
            }
            if !flags.is_empty() {
                parts.push(format!("[{}]", flags.join(",")));
            }
        }
        let _ = has;
        println!("{}{}", line, parts.join("  "));
    }
}

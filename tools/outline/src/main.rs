//! 命令行符号大纲：对源文件跑 zed 的 tree-sitter `outline.scm` 查询，
//! 输出 `OutlineItem`（层级 / 文本 / 行号 / 起止范围 …）。
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

    /// 输出格式
    #[arg(short, long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// 要输出的字段；逗号分隔或重复传。默认全部。
    /// 可选：depth text name line range selection body annotation
    /// 例：--fields depth,text,line
    // 不用 `num_args = 1..`：那会把后面的位置参数（文件路径）一并吃掉。
    #[arg(long, value_delimiter = ',')]
    fields: Vec<Field>,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Json,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Field {
    Depth,
    Text,
    Name,
    Line,
    Range,
    Selection,
    Body,
    Annotation,
}

impl Field {
    /// 未指定 `--fields` 时的默认集合（全量）。
    fn all() -> Vec<Field> {
        vec![
            Field::Depth,
            Field::Text,
            Field::Name,
            Field::Line,
            Field::Range,
            Field::Selection,
            Field::Body,
            Field::Annotation,
        ]
    }
}

/// 每个符号最终输出的一行 / 一个 JSON 对象。
#[derive(serde::Serialize)]
struct Row {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
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
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.paths.is_empty() {
        bail!("至少需要一个输入文件；用 --help 看用法");
    }
    let fields = if args.fields.is_empty() {
        Field::all()
    } else {
        args.fields
    };

    let mut cx = TestAppContext::single();
    let registry = Arc::new(LanguageRegistry::new(cx.background_executor.clone()));
    // `block_test` 在 ForegroundExecutor 上（不是 Background）：它驱动测试调度器，
    // 让 `load_language_for_file_path` 这类异步能在同步的 main 里跑完。
    let foreground = cx.foreground_executor.clone();
    register_all_grammars(&registry);

    let mut rows = Vec::new();
    for path in &args.paths {
        let text = fs::read_to_string(path).with_context(|| format!("读取 {}", path.display()))?;

        let language =
            foreground.block_test(resolve_language(registry.clone(), path, args.lang.as_deref()))?;

        let buffer = cx.new(|cx| {
            let mut buffer = Buffer::local(text, cx);
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
        for item in items {
            rows.push(build_row(path, &item, &fields));
        }
    }

    match args.format {
        Format::Json => println!("{}", serde_json::to_string_pretty(&rows)?),
        Format::Text => print_text(&rows, &fields),
    }
    Ok(())
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

fn build_row(path: &Path, item: &language::OutlineItem<Point>, fields: &[Field]) -> Row {
    let has = |f: Field| fields.contains(&f);
    let text = item.text.to_string();
    // `name_ranges` 标出 text 里哪些片段是「符号名本身」，拼起来就是干净的符号名。
    let name: String = item
        .name_ranges
        .iter()
        .filter_map(|range| text.get(range.clone()))
        .collect();

    Row {
        path: path.display().to_string(),
        depth: has(Field::Depth).then_some(item.depth),
        text: has(Field::Text).then_some(text),
        name: has(Field::Name).then_some(name),
        line: has(Field::Line).then_some(item.range.start.row + 1),
        range: has(Field::Range).then(|| [to_pair(item.range.start), to_pair(item.range.end)]),
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
    }
}

fn to_pair(point: Point) -> [u32; 2] {
    [point.row, point.column]
}

fn print_text(rows: &[Row], fields: &[Field]) {
    let mut current_path = String::new();
    for row in rows {
        if row.path != current_path {
            current_path = row.path.clone();
            println!("{}", current_path);
        }
        let indent = "  ".repeat(row.depth.unwrap_or(0) + 1);
        let mut parts: Vec<String> = Vec::new();
        if fields.contains(&Field::Depth) {
            parts.push(format!("[{}]", row.depth.unwrap_or(0)));
        }
        if let Some(text) = &row.text {
            parts.push(text.clone());
        }
        if let Some(name) = &row.name {
            if !name.is_empty() && Some(name) != row.text.as_ref() {
                parts.push(format!("({name})"));
            }
        }
        if let Some(line) = row.line {
            parts.push(format!("L{line}"));
        }
        if let Some(range) = row.range {
            parts.push(format!(
                "{:?}..{:?}",
                [range[0][0] + 1, range[0][1] + 1],
                [range[1][0] + 1, range[1][1] + 1]
            ));
        }
        if let Some(body) = row.body {
            parts.push(format!("body {}..{}", body[0] + 1, body[1] + 1));
        }
        if let Some(annotation) = row.annotation {
            parts.push(format!(
                "annotation {:?}",
                [annotation[0][0] + 1, annotation[0][1] + 1]
            ));
        }
        println!("{}{}", indent, parts.join("  "));
    }
}

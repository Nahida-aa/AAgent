# 从 zed 搬包到 AAgent：踩过的坑与做法

搬 `project` 时（1000+ 错误 → 0）沉淀的规则。参照仓库在 `~/repos/learn_ls/zed`。
**zed 源码是第一参照**，gpui-component / 自己的直觉都靠后。

---

## 1. zed 的单文件 crate 拆成多模块后，可见性要补

zed 的 `project.rs`（7000+ 行）是 **crate 根**，里面所有 `pub`/`use` 对整个 crate
天然可见。我们拆成 `project/` + `types/` + 顶层模块后，子模块靠 `use super::*;`
取用，就必须显式转发。

`packages/project/src/project/mod.rs:34-55` 是那块转发区，注释已经写清了三条：

- 必须是 `pub use`，**不能**是 `pub(crate) use` —— glob 导入不提升可见性，
  子模块拿不到就报 E0603。
- **不要转发 `proto`**。子模块自己写 `use ::rpc::proto;`。
  否则 `use super::*` 把 `proto` 带进子模块，文件里的 `::rpc::proto::X`
  会被解析成 `proto::proto::X`。
- 同理不要转发与子模块同名的东西。

判定方法：子模块里出现「找不到 X」时，先去 zed 确认 X 是不是 crate 根里的裸名。
是 → 加到转发块，别在子模块里写 `use crate::xxx::X`。

## 2. 模块名与外部 crate 同名时用绝对路径

`project/` 下有 `mod rpc;`、`mod dap;`、`mod lsp;`，会遮蔽同名外部 crate。
必须写 `::rpc::`、`::dap::`、`::lsp::`：

```rust
use ::rpc::proto::REMOTE_SERVER_PROJECT_ID;   // constructors.rs
use ::dap::client::DebugAdapterClient;        // mod.rs
```

症状是 E0433「cannot find module or crate `proto`」或「找不到 `dap::client`」——
不是依赖没配，是被本地 `mod` 挡住了。

## 3. `collections::` 不是 `std::collections::`

zed 用自己的 `collections` crate（`HashMap` = `FxHashMap`，hasher 是
`FxBuildHasher`）。照搬代码里写 `HashMap` 指的是**前者**。

如果某个文件写 `use std::collections::HashMap`，而它在别处与 zed 版相遇，
就是 E0308：`expected &HashSet<TaskHook>, found &HashSet<TaskHook, FxBuildHasher>`。

**搬代码时把 `std::collections::` 改成 zed 的 `collections::`。**
（`BTreeMap` / `BTreeSet` / `HashSet` / `VecDeque` 同理。）

> 现存还有几处没改干净：`project/src/project/toolchains.rs:4`、
> `project/src/project/lsp_rpc.rs:11`、`project/src/search_history.rs:1`。

## 4. 路径类型是 `RelPath`，不是 `std::path::Path`

- `path::RelPath`：内部恒为 unix 风格 `/` 分隔的**相对**路径。
- 要 `&Path`（调 std / fs API）→ `.as_std_path()`；要 `&str` → `.as_unix_str()`。
- **展示给用户**必须 `.display(path_style)`，不能直接 `as_unix_str()`。
- 空路径用 `RelPath::empty()` / `RelPath::empty_arc()`，不是 `Path::new("")`。
- `PathStyle` 只有一个：`path::PathStyle`（`util::paths` 是它的 re-export）。
  `aa_gpui_fuzzy` 也 re-export 同一个类型，所以 `fn path_style()` 无需转换。
- `PathEntry.path`（worktree）是 `Arc<RelPath>`。

## 5. 新建的占位包要整个替换

`cargo new` 生成的 `pub fn add(left: u64, right: u64) -> u64` 会一直留着。
`remote_connection/src/lib.rs` 就是一路留到搬代码那天才被 847 行真代码替换。
**开新包后先确认模板函数删了没有。**

## 6. 顺序：先整段照搬，再查缺

不要搬一个方法就推演一次缺什么。整段粘过去 → `cargo check` → 按错误补。
一批错误常常同源（比如 20 条 `REMOTE_SERVER_PROJECT_ID` 就是缺一行 `use`）。

**只看错误**（warning 常常上百条，淹没真正的错误）：

```bash
CARGO_BUILD_WARNINGS=allow cargo check -p project
```

按文件聚合、看哪类错误最多：

```bash
CARGO_BUILD_WARNINGS=allow cargo check -p project --message-format short 2>&1 \
  | grep 'error\[' | sed 's/:[0-9]*:[0-9]*:.*error/ error/' | sort | uniq -c | sort -rn
```

> 别用 `grep -c error` 数错误：warning 文本里的 "error" 单词
> （如 `EstablishConnectionError`、"will become a hard error"）会被算进去。
> 认准 `error[E0xxx]`，或看 exit code。

## 7. 别把「上一轮的旧数字」当基线

`cargo check` 的错误数会被并行的其他改动影响。判断自己的改动是好是坏，
用 `git stash` 前后各测一次，别拿记忆里的数字比。

## 8. 格式化

`.rustfmt.toml` 开了 `fn_single_line = true`（+ `unstable_features`），
所以短函数写一行是合规的：

```rust
pub fn is_file(&self) -> bool { !self.is_dir() }
```

`#[inline]` 这类属性另起一行更好读（rustfmt 不动属性，纯风格）。

## 9. 许可证

默认 GPL-3.0-or-later（根 `license.workspace`）。只有**零 zed 来源代码**的包
才显式写 `license = "Apache-2.0"`。

两个能安全引入的 zed 基础 crate 是 **Apache-2.0**：`path`、`gpui`（crates.io 侧）。
引它们不会把 GPL 传染进来。

## 10. 依赖用 Cargo 别名，不改 zed 源码

包要改名（如 `aa_gpui_kit_theme` → `theme`）就在 Cargo.toml 写
`theme = { package = "aa_gpui_kit_theme", ... }`，**不要**去改被搬文件里的
`use` 语句 —— 那会让以后对 zed 做 diff 变困难。

每个包在一个 Cargo.toml 里只能声明一个名字（别名 + 真名同时出现会报 E0463 类错误）。

同一条的限制更强：**同一个 member 的 `[dependencies]` 里也不能同时写真名和别名**，
会报 `error: the crate X depends on crate Y multiple times with different names`。
真名和别名都可以在根 `[workspace.dependencies]` 里声明，由各 member 挑一个。

## 10.1 模糊匹配：zed 有两套包，我们两套都要

zed 正在把 `fuzzy` 迁到 `fuzzy_nucleo`，HEAD 上是两套并存，而且**按 crate 分工不同**：

| zed crate | 依赖 | 签名差异 |
| --- | --- | --- |
| `editor`、`worktree` | `fuzzy`（旧） | `match_strings(..., smart_case: bool, penalize_length: bool, max, &cancel, executor)`，`StringMatch.string: String` |
| `language`、`command_palette`、`file_finder` | `fuzzy_nucleo`（新） | `match_strings(..., Case, LengthPenalty, max)`，`StringMatch.string: SharedString` |
| `project` | **两个都要** | `match_strings_async` 另有 Future 版 |

我们的 `aa_gpui_fuzzy` 对齐的是 `fuzzy_nucleo`。所以 AAgent 侧的做法是镜像 zed 的拓扑：

- `fuzzy_nucleo = { package = "aa_gpui_fuzzy", ... }`（语言/补全类已经迁移了的 crate）
- `fuzzy = { git = "https://github.com/zed-industries/zed", rev = f6838a7... }`
  （editor / worktree 用的旧包，**直接 git 依赖 zed 的真实实现**）

引入 zed 真实的旧 `fuzzy`、而不是让我们自己的包去兼容旧签名，是因为它只依赖
`gpui` / `gpui_util` / `path` / `log`，这四个我们已经在同一 rev 上引用了，不会引入
新的第三方 crate，也不会出现两份 gpui；代价只有「二进制里两个匹配引擎」，而这正是
zed 当下的状态。收益是 editor 那 5 处 `fuzzy::match_strings(...)` 能与 zed
逐行对齐、一字不改。

**CharBag 必须只有一个类型**：旧 `fuzzy` 的 `CharBag` 是被 `fuzzy_nucleo` 复用的
（见 zed `crates/fuzzy_nucleo/Cargo.toml` 依赖 `fuzzy`）。我们最初在 gpui_learn 里
自己实现了一份，于是 worktree（用旧 fuzzy）产出的 `CharBag` 与 project（用 nucleo
侧）期望的 `CharBag` 就成了两个类型，`packages/project/src/fuzzy.rs` 里
`PathMatchCandidate { char_bag: entry.char_bag }` 直接 E0308。别在边界处做转换，
在根上修：gpui_learn 的 `aa_gpui_fuzzy` 依赖 zed 的 `fuzzy` 并
`pub use fuzzy::CharBag;`，删掉本地副本。

## 11. Cargo 别名对 proc 宏不可见

第 10 条的别名只在写代码的**人**眼里存在。proc 宏展开时拿到的是 token，
**看不见 Cargo 别名**，所以宏生成的代码里如果硬编码了自己的 crate 名，
消费方起了别名就会 E0433「cannot find module or crate `aa_gpui_kit_component`」。

典型现场：`workspace/src/notifications/mod.rs` 的 `#[derive(RegisterComponent)]`
（消费方把 `aa_gpui_kit_component` 起别名成 `component`）。

处理顺序：

1. **先确认是不是这个原因**：错误落在 `#[derive(...)]` 那一行，且报的是宏包
   自己的 crate 名 → 就是它。
2. **宏侧加 `crate = "..."` 覆盖**（serde 的 `#[serde(crate = "...")]` 同型），
   默认还是真名，存量代码不用动。derive 能带 helper attribute：
   `#[proc_macro_derive(RegisterComponent, attributes(register_component))]`，
   用 `syn::parse_nested_meta` 读 `crate`，值 parse 成 `syn::Path`，
   `quote!` 里全部用 `#krate::` 而不是字面量。
3. **消费方**在自己那一行加属性，别去改 gpui_learn：
   ```rust
   #[derive(RegisterComponent)]
   #[register_component(crate = "component")]  // 本 crate 起的 Cargo 别名
   ```
   消费者用不用别名是他自己的选择，不要反过来强迫别人写
   `use aa_gpui_kit_component as component;`。

> 函数式宏（如 `derive_dynamic_spacing!`）**没法带 helper attribute**，
> 要覆盖只能做成宏参数。现在它硬编码 `aa_gpui_kit_theme::` 是安全的：
> 唯一调用方是 gpui_learn 自己的 `ui` 包（`styles/spacing.rs`），
> 那里没起别名；AAgent 零调用。**哪天有外部调用方了再说。**

推论（写宏时遵守）：**宏输出里不许硬编码自己的 crate 名**，要么用 `$crate`
（`macro_rules!` 免疫别名问题），要么留 `crate = "..."` 覆盖口。

## 12. E0599「找不到方法」先分清：是缺 import，还是 gpui_learn 缺扩展 trait

照搬 zed 代码报 E0599 时，编译器的提示经常是误导（比如建议你用 `rounded_none`）。
zed 有大量**扩展 trait**——方法看着像元素自带，其实住在别的 crate 里：

| 方法 | 真身 | 我们补的位置 |
|---|---|---|
| `rounded_client_corners` | `theme::ClientDecorationsExt`（zed `theme.rs:58`） | `theme/src/lib.rs` |
| `IconButton::tab_index` / `size` / `layer` / `track_focus` | `ui::ButtonCommon`（zed `icon_button.rs`） | `ui/src/components/button/mod.rs` |
| `PopoverMenu::trigger_with_tooltip` | 约束 `T: PopoverTrigger + ButtonCommon` | `ui/src/components/popover_menu.rs` |
| `menu.separator().when_some(..)` / `.map(..)` | `impl FluentBuilder for ContextMenu`（zed `context_menu.rs:271`） | `ui/src/components/context_menu/` |
| `Tooltip::for_action(_in)` | zed `tooltip.rs:96/109` | `ui/src/components/tooltip/mod.rs` |
| `PlayerColors::agent` | zed `players.rs:130` | `theme/src/styles/players.rs` |

判定方法：报的是 `aa_gpui_kit_ui::X` / `aa_gpui_kit_theme::X` **找不到方法或
trait bounds 不满足** → 去 zed grep 方法名，看它定义在哪个 trait / impl 上，
再确认 gpui_learn 有没有对应物。别急着改调用点。

注意 `FluentBuilder`：gpui 只给了 `impl<T: IntoElement> FluentBuilder for T`（blanket），
所以**非元素类型**（`ContextMenu`、菜单条目）要显式补一条 `impl FluentBuilder for X {}`
才写得动 `.when(..)` / `.when_some(..)`。在自己的 crate 里补这条 impl 不会
和 blanket impl 冲突（zed 就是这么写的）。

### 12.1 「private field, not a method」= trait 没进作用域

同一类 E0599 里最误导的一种：

```text
error[E0599]: no method named `children` found for struct `PaneAxisElement`
403 |             .children(rendered_children)
    |             -^^^^^^^^ private field, not a method
```

`.children(..)` 其实是 `gpui::ParentElement` 的提供方法；`PaneAxisElement`
恰好也有个**私有** `children` 字段，trait 不在作用域时编译器就去解析字段，
于是报「私有字段」而不是「缺 import」。`impl ParentElement` 一直都在
`pane/group/element.rs:472`，只是调用点没 `use`。

判定方法：报 "private field, not a method" → **先查是不是同名 trait 方法**，
别去改字段可见性，也别重复写一个 `impl`（会撞 E0119 conflicting implementations）。

### 12.2 关联函数（get_global / get / boxed_clone）同理

`X::get_global(cx)`、`X::get(..)`、`SplitUp { .. }.boxed_clone()` 这类**关联函数**
也要求 trait 在作用域：`settings::Settings`、`gpui::Action`。它们不在
`ui::prelude::*` 里，照搬 zed 代码时要单独 `use`。

## 13. 同一个类型全仓库只准有一份定义（IconName / RunnableTag 之鉴）

zed 单仓库里类型天然唯一。我们拆成两个仓库后，极易在「自家包」里重定义一个
同名类型 —— 一旦 editor 照搬 zed 代码（它假定全仓库一个类型），就出现
「expected X, found X」式 E0308 / E0599，报错文本极具迷惑性（两个类型打印
出来名字一样）。

已踩过的两例：

| 类型 | zed 里在哪 | 我们错在哪 | 修法 |
|---|---|---|---|
| `IconName` | `crates/icons` 唯一一份 | `aa_icons`（真身）与 `aa_gpui_kit_assets`（build.rs 生成）各一份，`base::Icon` 用了后者 | 全部归到 `aa_icons`，assets 删生成枚举（2026-09-27） |
| `RunnableTag` | `crates/task` 唯一一份，language 复用 | `task` 与 `language` 各一份 | language 删副本，`pub use task::RunnableTag` |

**修法永远是「收敛到一份」**：谁在 zed 里是真身就归谁，另一侧删掉改 re-export。
在边界写 `From` 转换是下策 —— 每个跨界点都要转换，还会持续繁殖。

判定方法：E0308 报「两个同名类型不相通」→ 先怀疑重定义，`rg "pub struct 类型名"`
跨包搜一遍，而不是琢磨怎么转换。

## 14. 收 `impl Trait` 的函数，喂 `.into()` 会 E0283 —— 参数要用具体类型

zed 代码大量出现 `.color(cx.theme().status().error.into())`：Hsla 经 `.into()`
喂给 `Icon::color`。若 `Icon::color` 收 `impl ResolveColor`（我们曾为「base 不依赖
主题包」发明的桥接 trait），rustc 对 `Into<?T>` 的候选（Color / Rgba / Background /
Fill / HighlightStyle / Hsla 自身）**不做跨约束裁剪**，即使全仓库只有
`Color: ResolveColor` 一个实现也报 E0283 type annotations needed。

结论：**吃 zed 调用面的 API 参数要用具体类型**（zed 的 `Icon::color(Color)`，
`Color::Custom(Hsla)` 兜底裸色）。桥接 trait 在这种参数上根本立不住 ——
`Icon` 后来整个归位到 `ui`（与 zed 同布局），`color(Color)` 落地，trait 删除。

推论：`base`（自研、theme-free）里不要放「zed ui 也有」的控件；凡是 zed 有
的，放 `ui` 并收 zed 的具体参数类型，否则每搬一个文件都要重新打一遍补丁。


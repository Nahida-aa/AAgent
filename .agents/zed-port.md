# 从 zed 搬包到 aacode：踩过的坑与做法

搬 `project` 时（1000+ 错误 → 0）沉淀的规则。参照仓库在 `~/repos/ide_ls/learn_ls/zed`。
**zed 源码是第一参照**，gpui-component / 自己的直觉都靠后。

**aacode crate edition: 2024** — 规则 #1 基于 Rust 2024 可见性变化。

---

## 1. Rust 2024 Edition 可见性规则（最重要）

**这一节是搬代码的基础。没搞懂之前别碰可见性。**

### 1.1 核心模型：模块是树形的

```
lib.rs (crate 根)
  ├── pub mod element;        ← pub: crate 内外都可见
  ├── mod panel;              ← Rust 2024 = pub(crate), 整个 crate 可见
  ├── mod persistence;        ← 同上
  └── pub mod view;
        ├── mod mode;         ← 私有, 只有 view/ 内部可见
        └── mod hover;        ← 同上
```

### 1.2 Rust 2024 独有的变化

| 位置 | 写法 | 实际含义 |
|------|------|---------|
| **lib.rs (根)** | `mod xxx;` | 整个 crate 可见 (= `pub(crate) mod`) |
| **lib.rs (根)** | `pub(crate) mod xxx;` | 和上面**完全一样**, 多余, rustc 会警告 |
| **lib.rs (根)** | `pub mod xxx;` | crate 内外都可见 |
| **子模块 mod.rs** | `mod xxx;` | 父模块 + 子模块可见 |
| **子模块 mod.rs** | `pub(crate) mod xxx;` | 整个 crate 可见 |
| **子模块 mod.rs** | `pub mod xxx;` | crate 内外都可见 |

**关键点: lib.rs 里的私有 mod 在 Rust 2024 下自动 pub(crate)。pub(crate) 前缀多余。**

### 1.3 兄弟模块互访

| 路径 | 含义 | 在哪能用 |
|------|------|---------|
| `super::xxx` | 从父节点出发 | **只能在子模块里** (lib.rs 没有父节点, 会 E0433) |
| `crate::xxx` | 从 crate 根出发 | 任何地方 |
| 隐式 `xxx::yyy` | 同上, 但更短 | 任何地方 |

兄弟互访私有子模块时:
- `super::bro::field` ✅ (子模块里, 从父节点出发找兄弟)
- `crate::bro::field` ✅ (任何地方, 但 bro 必须是 pub 或 Rust 2024 私有 = pub(crate))

### 1.4 转发块 (pub use) 的真正用途

旧版说"必须 pub use, 不能 pub(crate) use" — 这在**子模块 mod.rs** 里是对的,
但在 **lib.rs** 里是多余的 (Rust 2024 下私有 mod 已经全 crate 可见)。

转发块的唯一作用: **让外部 crate 能访问**。

如果某个类型只给 crate 内部用:
- 不需要转发块
- 调用点自己写 `use crate::xxx::yyy;`

如果某个类型要给外部 crate 用:
- 在**子模块 mod.rs** 里写 `pub use xxx::yyy;` (不是 pub(crate) use)
- 在 **lib.rs** 里只需要 `pub mod 子模块;`, 不需要额外 pub use

### 1.5 不要做的事

- ❌ **不要在 lib.rs 里加 `pub(crate) use` 共享导出块** — Rust 2024 私有 mod 已经全 crate 可见
- ❌ **不要搞 prelude.rs** — 每个文件独立加自己需要的 use
- ❌ **不要盲目改私有 mod 为 pub(crate)** — 先确认是不是兄弟模块真的需要跨 crate 访问
- ❌ **不要在 lib.rs 里写 `super::xxx`** — crate 根没有父节点, 会 E0433
- ❌ **不要把可见性修复等同于"加共享导出块"** — 90% 的情况调用点自己加 use 就行

### 1.6 验证

完整可见性验证实验见 `docs/visibility_lab/`:
```bash
cd docs/visibility_lab && cargo check
```
所有 ✅ 注释的行能编译, ❌ 取消注释会看到对应的 E0603/E0432/E0433。

---

## 2. 单文件 crate 拆多模块: 转发块什么时候需要

> 这一节描述的是 **zed 原版 (单文件 project.rs) 拆成 aacode 多模块** 的场景。
> 不是通用可见性修复模板。

zed 的 `project.rs` (7000+ 行) 是 crate 根, 所有定义天然对整个 crate 可见。
我们拆成 `project/mod.rs` + `project/types.rs` + `project/buffers.rs` 后:

- **如果类型还在 lib.rs** → Rust 2024 下私有 mod 已经全 crate 可见, 不需要转发
- **如果类型搬到了子模块** (比如 `project/types.rs` 里定义) → crate 内部用 `use crate::project::types::X`,
  外部 crate 要用才在 `project/mod.rs` 加 `pub use types::X;`

转发块的三条经验 (来自 project crate, 仍然有效):

- **不要转发 `proto`**。子模块自己写 `use ::rpc::proto;`。
  否则 `use super::*` 把 `proto` 带进子模块, 文件里的 `::rpc::proto::X`
  会被解析成 `proto::proto::X`。
- 同理不要转发与子模块同名的东西。

判定方法: 子模块里出现「找不到 X」时, 先去 zed 确认 X 是不是 crate 根里的裸名。
但**先别急着加转发块** — Rust 2024 下大概率是调用点自己加 `use crate::xxx::X` 就够了。

## 3. 模块名与外部 crate 同名时用绝对路径

`project/` 下有 `mod rpc;`、`mod dap;`、`mod lsp;`，会遮蔽同名外部 crate。
必须写 `::rpc::`、`::dap::`、`::lsp::`：

```rust
use ::rpc::proto::REMOTE_SERVER_PROJECT_ID;   // constructors.rs
use ::dap::client::DebugAdapterClient;        // mod.rs
```

症状是 E0433「cannot find module or crate `proto`」或「找不到 `dap::client`」——
不是依赖没配，是被本地 `mod` 挡住了。

## 4. `collections::` 不是 `std::collections::`

zed 用自己的 `collections` crate（`HashMap` = `FxHashMap`，hasher 是
`FxBuildHasher`）。照搬代码里写 `HashMap` 指的是**前者**。

如果某个文件写 `use std::collections::HashMap`，而它在别处与 zed 版相遇，
就是 E0308：`expected &HashSet<TaskHook>, found &HashSet<TaskHook, FxBuildHasher>`。

**搬代码时把 `std::collections::` 改成 zed 的 `collections::`。**
（`BTreeMap` / `BTreeSet` / `HashSet` / `VecDeque` 同理。）

> 现存还有几处没改干净：`project/src/project/toolchains.rs:4`、
> `project/src/project/lsp_rpc.rs:11`、`project/src/search_history.rs:1`。

## 5. 路径类型是 `RelPath`，不是 `std::path::Path`

- `path::RelPath`：内部恒为 unix 风格 `/` 分隔的**相对**路径。
- 要 `&Path`（调 std / fs API）→ `.as_std_path()`；要 `&str` → `.as_unix_str()`。
- **展示给用户**必须 `.display(path_style)`，不能直接 `as_unix_str()`。
- 空路径用 `RelPath::empty()` / `RelPath::empty_arc()`，不是 `Path::new("")`。
- `PathStyle` 只有一个：`path::PathStyle`（`util::paths` 是它的 re-export）。
  `aa_gpui_fuzzy` 也 re-export 同一个类型，所以 `fn path_style()` 无需转换。
- `PathEntry.path`（worktree）是 `Arc<RelPath>`。

## 6. 新建的占位包要整个替换

`cargo new` 生成的 `pub fn add(left: u64, right: u64) -> u64` 会一直留着。
`remote_connection/src/lib.rs` 就是一路留到搬代码那天才被 847 行真代码替换。
**开新包后先确认模板函数删了没有。**

## 7. 顺序：先整段照搬，再查缺

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

## 8. 别把「上一轮的旧数字」当基线

`cargo check` 的错误数会被并行的其他改动影响。判断自己的改动是好是坏，
用 `git stash` 前后各测一次，别拿记忆里的数字比。

## 9. 格式化

`.rustfmt.toml` 开了 `fn_single_line = true`（+ `unstable_features`），
所以短函数写一行是合规的：

```rust
pub fn is_file(&self) -> bool { !self.is_dir() }
```

`#[inline]` 这类属性另起一行更好读（rustfmt 不动属性，纯风格）。

## 10. 许可证

默认 GPL-3.0-or-later（根 `license.workspace`）。只有**零 zed 来源代码**的包
才显式写 `license = "Apache-2.0"`。

两个能安全引入的 zed 基础 crate 是 **Apache-2.0**：`path`、`gpui`（crates.io 侧）。
引它们不会把 GPL 传染进来。

## 11. 依赖用 Cargo 别名，不改 zed 源码

包要改名（如 `aa_gpui_kit_theme` → `theme`）就在 Cargo.toml 写
`theme = { package = "aa_gpui_kit_theme", ... }`，**不要**去改被搬文件里的
`use` 语句 —— 那会让以后对 zed 做 diff 变困难。

每个包在一个 Cargo.toml 里只能声明一个名字（别名 + 真名同时出现会报 E0463 类错误）。

同一条的限制更强：**同一个 member 的 `[dependencies]` 里也不能同时写真名和别名**，
会报 `error: the crate X depends on crate Y multiple times with different names`。
真名和别名都可以在根 `[workspace.dependencies]` 里声明，由各 member 挑一个。

## 11.1. 模糊匹配：zed 有两套包，我们两套都要

zed 正在把 `fuzzy` 迁到 `fuzzy_nucleo`，HEAD 上是两套并存，而且**按 crate 分工不同**：

| zed crate | 依赖 | 签名差异 |
| --- | --- | --- |
| `editor`、`worktree` | `fuzzy`（旧） | `match_strings(..., smart_case: bool, penalize_length: bool, max, &cancel, executor)`，`StringMatch.string: String` |
| `language`、`command_palette`、`file_finder` | `fuzzy_nucleo`（新） | `match_strings(..., Case, LengthPenalty, max)`，`StringMatch.string: SharedString` |
| `project` | **两个都要** | `match_strings_async` 另有 Future 版 |

我们的 `aa_gpui_fuzzy` 对齐的是 `fuzzy_nucleo`。所以 aacode 侧的做法是镜像 zed 的拓扑：

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

## 12. Cargo 别名对 proc 宏不可见

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
> 那里没起别名；aacode 零调用。**哪天有外部调用方了再说。**

推论（写宏时遵守）：**宏输出里不许硬编码自己的 crate 名**，要么用 `$crate`
（`macro_rules!` 免疫别名问题），要么留 `crate = "..."` 覆盖口。

## 13. E0599「找不到方法」先分清：是缺 import，还是 gpui_learn 缺扩展 trait

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

### 13.1. 「private field, not a method」= trait 没进作用域

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

### 13.2. 关联函数（get_global / get / boxed_clone）同理

`X::get_global(cx)`、`X::get(..)`、`SplitUp { .. }.boxed_clone()` 这类**关联函数**
也要求 trait 在作用域：`settings::Settings`、`gpui::Action`。它们不在
`ui::prelude::*` 里，照搬 zed 代码时要单独 `use`。

## 14. 同一个类型全仓库只准有一份定义（IconName / RunnableTag 之鉴）

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

## 15. 收 `impl Trait` 的函数，喂 `.into()` 会 E0283 —— 参数要用具体类型

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

## 16. zed 版本同步计划（rev → rev）

> 本节是**持续维护**的活文档（目标 rev、批次、分工、验收都在这儿改）；
> §1~§15 是踩坑沉淀，追加式。两节性质不同，编号只为查找方便，不作强制标准。


### 16.1. 为什么这份计划单独维护

前 3 节是「搬一个包」的坑，一次性、局部。本节是**持续性的大工程**：aacode 是 copy-fork
（不是 git fork，没有 merge base），`packages/` 下167 个 crate 是从 zed 整包搬过来的，
换 rev 不会让它们自动获得上游修复 —— 编译照过，腐化看不见。

所以同步必须：可分派、可验收、可回溯。**动手前先读本节，改本节的状态再动手。**

工具入口（都在 aacode 仓库根）：

```sh
just zed-fork-scan                # 概览：命中哪些 fork crate
just zed-fork-commits             # 逐条分诊
just zed-fork-files               # 每 crate 的文件 diff
just zed-fork-all                 # 上游全貌，★ 标出我们 fork 了哪些
just gh-releases zed-industries/zed 1 0 true | cut -f2   # 最新 stable 的 sha
```

### 16.2. 当前基线（2026-10-03）

| 项 | 值 |
| --- | --- |
| aacode / gpui_learn 锁的 zed rev | `bd747337d7be138834e20972b9e203c7b239cc47`（09-28 main 线） |
| 最新 stable | `v1.22.0` = `76659a55a8c10ed355a070f8764a0b1733e3c115` |
| zed `origin/main` HEAD | `badfb8d31f`（10-03） |
| aacode 直锁 zed rev 的 crate | 29 个（`gpui*` 6 + `collections` + 基础设施） |
| aacode 本地 fork 的 crate | 167 个（`packages/*` path 依赖） |
| gpui_learn 的 `ui` 被 aacode 锁在 | `c712ba16`（aacode `Cargo.toml:536`） |

**不要把 stable 当升级目标。** zed 的 stable tag 不在 main 上，是只含 cherry-pick 的发布线，
`v1.22.0` 比 pin 少 72 个提交（切过去是 -38030 行）。从 main 线pin 出发，stable 是**降级**。
详见 `CLAUDE.local.md` 的「为什么不追最新」。

### 16.3. 依赖顺序：gpui_learn 必须先于 aacode

```
W1  gpui_learn 换 zed rev  →  适配 vendored 平台层  →  commit + push
                                    ↓
W2  aacode 换 29 处 zed rev + Cargo.toml:536 的 ui rev  →  适配  →  commit + push
                                    ↓
W3  fork 同步（port 上游 diff 进 packages/*）—— 与 W1/W2 解耦，可并行多人
```

- W1/W2 是**强顺序**：aacode 从 gpui_learn 拉 `aa_gpui_kit_ui`，gpui API 一变，
  aacode 编译不过。必须同批次一起推。
- W3 与 W1/W2 **没有依赖**，可以按 crate / 按提交任意切分给多人。
- W3 里的 `packages/ui` 例外：它吃 gpui，要等 W2 落地才能验。

### 16.4. 批次划分

rev 同步是原子的 —— 换 rev 会把区间内**全部**改动带进来，没法只挑一半。
所以批次只能按日期切。按提交数/gpui 命中/fork 命中切出来是这样：

| 批次 | 区间 | 总提交 | gpui 命中 | fork 命中 | 说明 |
| --- | --- | --- | --- | --- | --- |
| **A** | `bd747337` → `afecd6d719`（09-30） | 17 | 2 | 11 | 先跑通流程 + 建验收标准 |
| **B** | `afecd6d719` → `95cd535a5f`（10-01） | 27 | 5 | 21 | |
| **C** | `95cd535a5f` → `badfb8d31f`（10-03） | 33 | 5 | 10 | 到 main HEAD，收尾 |

批次 A 触及的 fork crate：`node_runtime`(3) `project`(3) `proto`(3) `remote_server`(3)
`cloud_api_client`(2) `acp_thread`(1) `client`(1) `http_client`(1) `open_ai`(1) `which_key`(1)。

**先只做批次 A。** 目的是把工具链、验收标准、提交规范跑通并留下记录，
而不是立刻吃到 77 个提交。批次 A 稳定后再决定 B/C 怎么切。

### 16.5. 工作流

| ID | 范围 | 前置 | 完成判据 | 负责 | 状态 |
| --- | --- | --- | --- | --- | --- |
| W1.1 | gpui_learn 根 `Cargo.toml` 10 处 rev → 目标 rev | 批次目标确定 | `cargo check` 过 | | ☐ |
| W1.2 | gpui_learn vendored 平台层适配（`gpui-android` 等） | W1.1 | Web/Android 示例仍能build | | ☐ |
| W1.3 | gpui_learn commit + push，记下新 sha | W1.2 | 远端可拉 | | ☐ |
| W2.1 | aacode 根 `Cargo.toml` 29 处 zed rev + `ui` rev | W1.3 | `cargo check` 过 | | ☐ |
| W2.2 | aacode gpui API 适配（`Platform` trait 新成员等） | W2.1 | 主线 `app` `workspace` `ui` 过 | | ☐ |
| W2.3 | aacode commit + push | W2.2 | 远端可拉 | | ☐ |
| W3.x | fork 同步：按 §4.4 的 fork 命中清单逐 crate port | 与 W1/W2 无依赖 | 见 §4.7 | | ☐ |
| W4.x | 补功能：**只在主线 `app`/`workspace`/`ui` 卡住时**按需 port | — | 主线能跑通该流程 | | ☐ |

W3 的拆分方式（每人认领，互不重叠）：

```sh
# 认领前先看某批提交落在哪些 crate
just zed-fork-commits | grep -oE '(agent|acp_thread|editor|language|lsp|project|workspace)'

# 认领后逐条深挖
git -C ~/repos/ide_ls/learn_ls/zed show <sha> -- crates/<crate>
```

**认领原则**：按 crate 认领（不按提交），避免两个人同时改同一个 `packages/*`。
一个 crate 一个 owner，PR 里写明认领的 sha 列表。

### 16.6. 同步索引（每次同步一个文件）

分工、逐条 port 记录、踩的坑都下沉到 `.agents/fork-sync/<old7>_<new7>.md`，
这里只留索引。约定见该目录的 `README.md`。

```sh
just fork-sync new      # 生成下一次同步的计划文件（区间数据自动填好）
just fork-sync list     # 看有哪些同步、什么状态
```

| 区间 | 批次 | 状态 | 负责人 | 文件 |
| --- | --- | --- | --- | --- |
| `bd747337` → `afecd6d719` | A | 🟡 计划中 | — | [bd74733_afecd6d.md](fork-sync/bd74733_afecd6d.md) |
| `afecd6d719` → `95cd535a5f` | B | ⚪ 未开始 | — | — |
| `95cd535a5f` → `badfb8d31f` | C | ⚪ 未开始 | — | — |

状态：⚪ 计划中 / 🟡 进行中 / 🟢 完成 / 🔴 放弃。
分工按 crate 认领，一个 crate 一个 owner，避免两人改同一个 `packages/*`。

### 16.7. 验收标准

**W1 / W2（rev 同步）**：

```sh
# aacode 主线三包（阶段 2 的门槛口径）
cargo check -p app -p workspace -p ui

# 下限（AGENTS.md 的14 包口径，提交前必须过）
cargo check --workspace
```

gpui 的 `Platform` trait 新增成员是最常见的破坏点，vendored 平台层要逐个补。

**W3（fork 同步）** 的单条完成判据 —— 不能只看编译过：

- [ ] 上游 diff 已逐文件看过，不是盲合
- [ ] 确认 aacode 在该文件上**没有**自己的改动（`git log --oneline -- packages/<crate>/<file>`）
      有则必须人工调和，并在 commit message 里说明
- [ ] 目标 crate 的 `cargo check` 过
- [ ] 有测试的跑测试；没有的说明为什么这个改动不需要测试
- [ ] commit message 带上游 sha 和 PR 号

### 16.8. 提交规范

```
chore(zed): 同步 rev bd747337 → afecd6d719（批次 A，17 提交 / gpui 2 / fork 11）
fork(agent): 同步上游 c32938c34c（open_ai 工具调用可选参数修复，#64920）
fork(acp_thread): 同步上游 3209c7d31f 权限作用域（#65080），调和 aacode 的 xxx 改动
```

- rev 同步**一个批次一个 commit**，适配改动放同一个 commit 或紧邻，别拆散
- fork 同步**一个上游 sha 一个 commit**，便于单独 revert
- 每个批次做完：把自己那份同步文件的状态改成 🟢，回 §16.6 索引表更新状态与负责人

### 16.9. 批次记录

不在这里记 —— 每批的完成日 / 踩的坑 / 遗留写在各自的
`.agents/fork-sync/<old7>_<new7>.md` 里，做完把 §16.6 索引表的状态更新一下。

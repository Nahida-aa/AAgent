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

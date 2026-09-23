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

看错误用短格式，好按文件聚合：

```bash
cargo check -p project --message-format short 2>&1 | grep error | sed 's/:[0-9]*:[0-9]*:.*error/ error/' | sort | uniq -c | sort -rn
```

注意 `grep -c error` 会把 warning 里出现的 "error" 单词（如
`EstablishConnectionError`）也算进去。**认准 `error[E0xxx]` 或看 exit code。**

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

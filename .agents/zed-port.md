# 从 zed 搬包到 aacode：踩过的坑与做法

搬 `project` 时（1000+ 错误 → 0）沉淀的规则。参照仓库在 `~/repos/learn_ls/zed`。
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

## 4. zed 版本同步计划（rev → rev）

### 4.1 为什么这份计划单独维护

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

### 4.2 当前基线（2026-10-03）

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

### 4.3 依赖顺序：gpui_learn 必须先于 aacode

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

### 4.4 批次划分

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

### 4.5 工作流

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

### 4.6 分工表

| 人 | 负责 | 批次 | 状态 |
| --- | --- | --- | --- |
| （待填） | W1 gpui_learn rev | A | ☐ |
| （待填） | W2 aacode rev + gpui 适配 | A | ☐ |
| （待填） | W3 fork 同步 · agent 系（`agent` `agent_ui` `acp_thread` `agent_servers`） | A/B/C | ☐ |
| （待填） | W3 fork 同步 · 语言系（`language` `languages` `language_models` `editor`） | | ☐ |
| （待填） | W3 fork 同步 · 基建（`project` `fs` `http_client` `cloud_api_*` `node_runtime`） | | ☐ |
| （待填） | 验收 / review | | ☐ |

### 4.7 验收标准

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

### 4.8 提交规范

```
chore(zed): 同步 rev bd747337 → afecd6d719（批次 A，17 提交 / gpui 2 / fork 11）
fork(agent): 同步上游 c32938c34c（open_ai 工具调用可选参数修复，#64920）
fork(acp_thread): 同步上游 3209c7d31f 权限作用域（#65080），调和 aacode 的 xxx 改动
```

- rev 同步**一个批次一个 commit**，适配改动放同一个 commit 或紧邻，别拆散
- fork 同步**一个上游 sha 一个 commit**，便于单独 revert
- 每个批次做完回 §4.5 把 ☐ 改成 ☑，并在下面追加实测数据

### 4.9 批次记录

| 批次 | 实际目标 rev | 完成日 | 踩的坑 | 遗留 |
| --- | --- | --- | --- | --- |
| A | | | | |
| B | | | | |
| C | | | | |


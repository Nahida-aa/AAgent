# 同步 bd74733 → afecd6d（批次 A）

| | |
| --- | --- |
| 状态 | 🟡 计划中 |
| from | `bd747337d7be138834e20972b9e203c7b239cc47`（2026-09-28） |
| to | `afecd6d719aad92aecfa2860f49c4f2956708831`（2026-09-30） |
| 区间提交数 | 17 |
| gpui 命中提交 | 2 |
| fork 命中 | 11 个 crate |
| ⚠ 需人工调和（L2） | 5 |
| 负责人 | — |
| 创建 | 2026-10-03 |

## 1. 为什么做这一批

先跑通流程 + 建验收标准，不追求吃到多少代码

## 2. 目标 / 非目标

目标：

-

非目标：

-

## 3. 工作流

rev 同步是原子的（换 rev 会带进区间内全部改动），所以 W1/W2 必须同批次推；
W3 与它们无依赖，可按 crate 多人并行。

### W1 gpui_learn（必须先于 W2）

- [ ] W1.1 根 `Cargo.toml` 10 处 rev → `afecd6d`
- [ ] W1.2 vendored 平台层适配（`gpui-android` 等）
- [ ] W1.3 commit + push，记下 gpui_learn 新 sha

### W2 aacode rev + gpui 适配

- [ ] W2.1 根 `Cargo.toml` 29 处 zed rev + `Cargo.toml:536` 的 `ui` rev
- [ ] W2.2 gpui API 适配（`Platform` trait 新成员等）
- [ ] W2.3 `cargo check -p app -p workspace -p ui` 过
- [ ] W2.4 `cargo check --workspace` 过（下限）
- [ ] W2.5 commit + push

### W3 fork 同步（按 crate 认领，一 crate 一 owner）

命中 11 个 crate：

  - [ ] acp_thread（待认领）
  - [ ] agent_ui（待认领）
  - [ ] client（待认领）
  - [ ] cloud_api_client（待认领）
  - [ ] http_client（待认领）
  - [ ] node_runtime（待认领）
  - [ ] open_ai（待认领）
  - [ ] project（待认领）
  - [ ] proto（待认领）
  - [ ] remote_server（待认领）
  - [ ] which_key（待认领）

逐条 port 记录（一个上游 sha 一个 commit，便于单独 revert）：

| 上游 sha | crate | aacode 改过该文件? | 动作 | port commit | 认领 |
| --- | --- | --- | --- | --- | --- |
| `afecd6d719` | `node_runtime` | ✓（与上游old等价基础上新增SystemNode） | node_runtime: Expose standalone system Node discovery (#64928) | `23f4b0d` | — |
| `017f9b89aa` | `agent_ui` | ✓（1 行，L1 已知变换内） | agent_ui: Hide wrap guides in the agent message editor (#64886) | `69031de` | — |
| `5d5963361f` | `project` | ✓ | Diff LSP format responses that replace the whole buffer (#57269) | `0e73083` | — |
| `1dc8844439` | `which_key` | ✓ | which_key: Show task names for task::Spawn bindings (#64937) | `7ef1a04` | — |
| `c87632ef44` | `project`, `proto`, `remote_server` | ? | Read remote shell config when creating a terminal shell (#61451) | | |
| `14dd03e896` | `acp_thread`, `agent_ui` | ? | agent_ui: Guard follow-up sends from stale send results (#64917) | | |
| `12f79c0aeb` | `client`, `cloud_api_client` | ? | cloud_api_client: Use the platform TLS verifier for the cloud websocket (#63686) | | |
| `c32938c34c` | `open_ai` | ? | open_ai: Fix issues with optional arguments when model calls a tool (#64920) | | |
| `ead2d9eac0` | `http_client` | ? | http_client: Ensure GitHub digest prefix is always stripped (#64905) | | |

## 4. ⚠ 冲突预警（按「差异能否被已知移植变换解释」分级）

**不要用「本地有没有提交碰过这个文件」判断冲突** —— copy-fork 里port 动作本身就会让每个
被同步过的文件出现在 `git log` 里，那样判几乎全是误报。本节用的是：把本地文件与
**上游 old 版本**比，看差异能否被已知变换（workspace-ify / `src/x.rs`→`src/lib.rs` /
`zed_actions`→`aacode_actions` / `zlog`→`a_log` 等）解释。

| 级别 | 含义 | 本批 |
| --- | --- | --- |
| **L0 快进** | 本地 == 上游 old，干净照搬 | 14 |
| **L1 已知变换** | 差异全是机械移植变换，按例应用即可 | 2 |
| **L2 需人工** | 有解释不了的差异 | 5 |

L2 只能筛出「与上游 old 不同、且不像机械变换」的文件，**判断不了意图**。
「我们故意删了这些测试」「这个偏离是设计而非疏漏」只有人知道，所以 L2 一律人工确认，
不要自动当成冲突。

| 本地路径 | 上游路径 | 级别 | 未解释行 | 样例 |
| --- | --- | --- | --- | --- |
| `packages/acp_thread/src/connection.rs` | `crates/acp_thread/src/connection.rs` | **L0** | 0 | — |
| `packages/agent_ui/src/conversation_view.rs` | `crates/agent_ui/src/conversation_view.rs` | **L0** | 0 | — |
| `packages/agent_ui/src/conversation_view/thread_view.rs` | `crates/agent_ui/src/conversation_view/thread_view.rs` | **L1** | 0 | — |
| `packages/agent_ui/src/message_editor.rs` | `crates/agent_ui/src/message_editor.rs` | **L1** | 0 | — |
| `packages/client/Cargo.toml` | `crates/client/Cargo.toml` | **L1** | 1 | < workspace = true; |
| `packages/cloud_api_client/Cargo.toml` | `crates/cloud_api_client/Cargo.toml` | **L1** | 1 | < workspace = true; |
| `packages/cloud_api_client/src/websocket/native.rs` | `crates/cloud_api_client/src/websocket/native.rs` | **L0** | 0 | — |
| `packages/http_client/src/github.rs` | `crates/http_client/src/github.rs` | **L0** | 0 | — |
| `packages/node_runtime/Cargo.toml` | `crates/node_runtime/Cargo.toml` | **L1** | 1 | < workspace = true; |
| `packages/node_runtime/src/node_runtime.rs` | `crates/node_runtime/src/node_runtime.rs` | **L0** | 0 | — |
| `packages/open_ai/src/completion.rs` | `crates/open_ai/src/completion.rs` | **L0** | 0 | — |
| `packages/project/src/lsp_store.rs` | `crates/project/src/lsp_store.rs` | **L0** | 0 | — |
| `packages/project/src/terminals.rs` | `crates/project/src/terminals.rs` | **L0** | 0 | — |
| `packages/project/tests/integration/project_tests.rs` | `crates/project/tests/integration/project_tests.rs` | **L0** | 0 | — |
| `packages/proto/proto/task.proto` | `crates/proto/proto/task.proto` | **L0** | 0 | — |
| `packages/proto/proto/zed.proto` | `crates/proto/proto/zed.proto` | **L0** | 0 | — |
| `packages/proto/src/proto.rs` | `crates/proto/src/proto.rs` | **L0** | 0 | — |
| `packages/remote_server/Cargo.toml` | `crates/remote_server/Cargo.toml` | **L1** | 2 | < [[bin]];< workspace = true; |
| `packages/remote_server/src/headless_project.rs` | `crates/remote_server/src/headless_project.rs` | **L0** | 0 | — |
| `packages/remote_server/src/remote_editing_tests.rs` | `crates/remote_server/src/remote_editing_tests.rs` | **L2** | 550 |     CompletionSource, LanguageServerLogType, ProgressToken, Project, ProjectPath,;    LanguageServerLogType, ProgressToken, Project, ProjectPath,;    lsp_store: |
| `packages/which_key/src/which_key.rs` | `crates/which_key/src/which_key.rs` | **L0** | 0 | — |

### L2 逐个确认

**remote_editing_tests.rs**（packages/remote_server/src/remote_editing_tests.rs）
- aacode 在 old（bd747337）时就与上游不同：上游版本完整，aacode 版本已做大量裁剪（差异 550 行，主要是删除测试函数）。
- 结论：这是**有意裁剪的本地改动**（aacode 不跑这些 remote editing 集成测试），不属于「冲突」而是「设计偏离」。
- 同步策略：**不要一刀替换整个文件**。遇到上游在这个文件有改动时，按需 cherry-pick/手动合并，优先保留 aacode 现有裁剪，不要把测试全集强行搬回来。

其余 L2（Cargo.toml）已归一化判定为 L1。

```sh
# 通用核对命令
git log --oneline -- <本地路径>
git -C ~/repos/ide_ls/learn_ls/zed diff bd747337d7be138834e20972b9e203c7b239cc47..afecd6d719aad92aecfa2860f49c4f2956708831 -- <上游路径>
diff <(git -C ~/repos/ide_ls/learn_ls/zed show bd747337d7be138834e20972b9e203c7b239cc47:<上游路径>) <本地路径>
```
## 5. 验收

- [ ] `cargo check -p app -p workspace -p ui`
- [ ] `cargo check --workspace`
- [ ] 每条 fork 同步都确认过「该文件 aacode 自己没改动」，有的已在 commit 里说明
- [ ] gpui_learn 侧的 Web/Android 示例仍能 build
- [ ] 回来更新 zed-port.md §16 索引与本文件状态

## 6. 踩的坑

<!-- 做的时候随手记，这里是后来人最需要的东西 -->

## 7. 遗留 / 下一批

-

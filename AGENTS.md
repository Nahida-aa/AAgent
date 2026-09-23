<!-- intent-skills:start -->

## Skill Loading

Before editing files for a substantial task:

- Run `bunx @tanstack/intent@latest list` from the workspace root to see available local skills.
- If a listed skill matches the task, run `bunx @tanstack/intent@latest load <package>#<skill>` before changing files.
- Use the loaded `SKILL.md` guidance while making the change.
- Monorepos: when working across packages, run the skill check from the workspace root and prefer the local skill for the package being changed.
- Multiple matches: prefer the most specific local skill for the package or concern you are changing; load additional skills only when the task spans multiple packages or concerns.

<!-- intent-skills:end -->

- 修改代码后, 如果认为适合提交, 就自行提交

## ⚠️ 主线声明

**GPUI 桌面（packages/app + packages/workspace + packages/ui）是唯一主线。**

以下前端方案是历史遗留或实验性的，**不保证能编译、能运行或与主线兼容**：
- `packages/cli/` — Ratatui TUI（旧 CLI 入口）
- `packages/tui/` — OpenTUI + Solid.js TUI（Bun）
- `packages/webui/` — Solid.js Web 前端（TanStack Router）
- `packages/ui-solid/` — Kobalte UI 组件库
- `packages/shared/` — TS 共享 lib

`cargo check --workspace` 只要求主线相关包通过（`kernel`、`core`、`llm`、`extensions`、`extension-sdk`、`extension-mcp`、`function-tools`、`config`、`session`、`server`、`app`、`workspace`、`ui`）。其他包即使编译失败也不阻塞提交。

---

所有代码统一放在 `packages/` 下，不分 Rust/TS：

**主线包（必须编译通过）：**

- `packages/kernel/` 微内核（ToolRegistry、ToolProvider）
- `packages/core/` 核心类型（Extension trait、LLM 抽象）
- `packages/extensions/` 扩展系统 + WASM 加载器（wasmtime Component Model）
- `packages/extension-sdk/` 扩展公开 API
- `packages/extension-mcp/` MCP 客户端适配器（JSON-RPC 2.0 over stdio）
- `packages/function-tools/` LLM function calling 工具集（fs 工具）
- `packages/llm/` LLM Provider 实现（OpenAI 兼容 + Ollama 原生）
- `packages/config/` LLM 配置系统（aa.json + AA_* env）
- `packages/session/` 共享对话循环（run_turn），事件驱动
- `packages/server/` HTTP/API 服务（axum, `/chat` SSE, `/health`, `/tools`）
- `packages/workspace/` **GPUI Workspace 框架**（状态栏、面板、Dock）
- `packages/ui/` **GPUI UI 组件层**
- `packages/app/` **GPUI 桌面前端**（主窗口、标题栏、Agent 面板）

**非主线（仅参考/历史，不保证编译）：**

- `packages/cli/` 旧 CLI 入口（`aa run` 行模式、Ratatui TUI）
- `packages/tui/` OpenTUI + Solid.js TUI（独立 Bun 进程）
- `packages/webui/` Solid.js + TanStack Router Web 前端
- `packages/ui-solid/` Kobalte 共享 UI 组件库
- `packages/shared/` TS 共享 lib（utils、i18n）
- `packages/sdk-ts/` 自动生成的 TypeScript SDK（openapi-ts）

**Rust edition**: 2024（`set_var`/`remove_var` 需 `unsafe` 块）

📁 **目录名 vs Cargo.toml name 规则**：目录名用简洁形式（如 `kernel/`），`package.name` 用 `aa-` 前缀（如 `aa-kernel`）。不要混用。

## 架构决策

- **Server 层不单独拆 C/S**，内嵌在 `packages/server/` 作为同进程调用边界
- **主线（GPUI 桌面）直接调 `session::run_turn()`**，不走网络、不走 SSE
- `packages/server/` 可选暴露 HTTP/SSE（`aa serve`）给远程客户端（非主线前端用）

## GPUI 桌面架构（主线）

- `packages/app/` — 应用入口：主窗口、标题栏（SSD/CSD 条件渲染）、Agent 面板
- `packages/workspace/` — Workspace 框架：StatusBar（left/right/hidden items）、DockSide、面板切换
- `packages/ui/` — 共享 GPUI UI 组件
- **GPUI 来源**：Zed git rev `f6838a7c`（pin 在根 Cargo.toml `[patch.crates-io]`）
- **WindowControlArea**：`Drag` + `Close/Minimize/Maximize` 按钮布局（吸收自 aa-player）
- **服务器调用**：同进程 `session::run_turn()`，不经过 HTTP

## 非主线架构（仅供参考，不保证工作）

### 旧 CLI（Ratatui）
- `aa run` — 行模式对话
- `aa` — Ratatui TUI

### OpenTUI + Solid.js TUI
- 独立 Bun 进程，localhost HTTP/SSE 连 Rust server
- `@opentui/*` 必须 0.3.4（0.4.1 Solid context 不传播）
- JSX 通过 Babel 插件转换，需 `bunfig.toml` + `--conditions=browser`

### WebUI（Solid.js + TanStack）
- 通过 HTTP/SSE 连接 Rust server

## 已验证

- **Ollama 端到端**：`aa run --provider ollama --model gemma4:31b-cloud` 成功返回 "Hello"（462 prompt + 2 completion tokens）
- **TUI 编译通过**（无终端时 panic 属预期）
- **`Config::resolve()` 正确性**：cli 传 `Option`，不覆盖 provider 默认值（ollama→`http://localhost:11434`）
- **`RunArgs` 改为 `Option<String>`**：避免 clap default 覆盖 provider 特定默认值

纠错记录见 `.agents/CORRECTIONS.md`。

## 从 zed 搬代码

**搬 zed 的包之前，先读 `.agents/zed-port.md`。**

zed 参照仓库在 `~/repos/learn_ls/zed`，单文件 crate 拆分后的可见性转发、
`collections::` vs `std::collections::`、`RelPath` vs `std::path::Path`、
模块遮蔽要用 `::rpc::` 绝对路径等坑都记在那儿，照搬时直接套用，别重新推演。

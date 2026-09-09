# AAgent

个人 AI 助手平台。Rust 微内核 + WASM 插件 + LLM 集成 + 多前端。

## 架构

```
┌──────────┐ ┌──────────┐ ┌────────┐ ┌─────────┐
│  CLI     │ │  Server  │ │  Web   │ │ Desktop │
│ (tui)    │ │  (axum)  │ │ (Solid)│ │ (Tauri) │
└────┬─────┘ └────┬─────┘ └───┬────┘ └────┬────┘
     └────────────┴────────────┴───────────┘
                      │
              ┌───────┴────────┐
              │  aa-kernel     │
              │  ToolRegistry  │
              │  ToolProvider  │
              └───────┬────────┘
        ┌─────────────┼──────────────┐
        │             │              │
  ┌─────┴─────┐ ┌─────┴─────┐ ┌─────┴─────┐
  │ Built-in  │ │  WASM     │ │  MCP      │
  │ Tools     │ │  Plugins  │ │  Adapter  │
  └───────────┘ └───────────┘ └───────────┘
```

## 进度

### 已完成

| 包 | 状态 | 说明 |
|----|------|------|
| `kernel` | ✅ | ToolProvider trait、ToolRegistry、Kernel builder |
| `core` | ✅ | Extension trait、llm 抽象 (ModelProvider、Message、StreamEvent) |
| `llm` | ✅ | OpenAI 兼容 Provider（流式 + 非流式） |
| `ollama` | ✅ | Ollama 原生 Provider（/api/chat） |
| `function-tools` | ✅ | 6 个 fs 工具（read/write/ls/find/grep/info） |
| `extension-mcp` | ✅ | MCP 客户端适配器（stdio JSON-RPC 2.0） |
| `extensions` | ✅ | 扩展注册器 + WASM 加载器（wasmtime Component Model） |
| `extension-sdk` | ✅ | 扩展公开 API re-export |
| `config` | ✅ | `aa.json` + `AA_*` env 配置系统 |
| `server` | ✅ | axum HTTP：`/health` `/tools` `/tools/{name}` `/chat`（AG-UI SSE） |
| `cli` | ✅ | 主入口（`aa`=TUI, `aa run`=行模式, `aa tool`, `aa extension`），含 Ratatui TUI |
| WASM 插件 e2e | ✅ | WIT 定义 + WAT 组件 + 加载测试 |
| `session` | ✅ | 共享对话循环（`run_turn`），CLI/TUI 共用 |

### 进行中

| 包 | 状态 | 说明 |
|----|------|------|
| `web` | 🏗️ | Solid.js 骨架（TanStack Router + Query）、未接 `useChat` |
| `ui-solid` | 🏗️ | 共享 UI 组件库（Kobalte、Tailwind v4） |
| `shared` | 🏗️ | 共享 lib（utils、i18n、db） |

### 待办

- [ ] **Web 前端接 SSE** — `useChat({ connection: fetchServerSentEvents('/api/chat') })` 聊天气泡
- [ ] **extension-skills** — 技能引擎（prompt 模板 + 工具链）
- [ ] **LLM 配置编辑器** — Web 前端设置页编辑 provider 配置
- [ ] **desktop** — Tauri 桌面端
- [ ] **TUI 增强** — markdown 渲染、分页、命令行历史

## 快速开始

```bash
# Server（默认 OpenAI）
export AA_API_KEY="sk-..."
cargo run -p aa-server

# 或用 Ollama
cargo run -p aa-server &
# aa.json:
# { "model": "ollama/llama3.2", "provider": { "ollama": { "base_url": "http://localhost:11434" } } }

# TUI（默认）
AA_API_KEY="sk-..." cargo run -p aa-cli

# 行模式
AA_API_KEY="sk-..." cargo run -p aa-cli run
```

## 配置

`aa.json`（当前目录或 `~/.config/aa/aa.json`）：

```json
{
  "model": "openai/gpt-4o-mini",
  "provider": {
    "openai": {
      "api_key": "sk-...",
      "base_url": "https://api.openai.com/v1"
    },
    "ollama": {
      "base_url": "http://localhost:11434"
    }
  }
}
```

环境变量（优先级高于 `aa.json`）：`AA_PROVIDER` `AA_MODEL` `AA_API_KEY` `AA_BASE_URL`

旧的 `AA_LLM_*` 系列变量作为向后兼容 fallback。

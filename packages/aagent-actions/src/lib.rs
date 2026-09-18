//! 全局 Actions — 对齐 zed `crates/zed_actions`。
//!
//! Action 是 GPUI 的"命令"抽象：每个 Action 是一个 struct/enum，
//! 可以被 keybinding 绑定、被 command palette 调用、被其他 entity dispatch。
//!
//! 两个关键宏:
//! - `actions!(namespace, [Action1, Action2, ...])` — 定义无参数 unit struct actions
//! - `#[derive(Action)]` — 带字段的 struct action（需要 Clone + PartialEq）
//!
//! Zed 设计要点:
//! - `init()` 函数 — 确保 crate 被 binary 引用（否则 Rust 会优化掉 Action 注册）
//! - `#[action(namespace = xxx)]` — 控制 action 的命名空间（用于 JSON keybinding）
//! - `#[action(no_json, no_register)]` — 内部转发 action，不暴露给用户
//! - `#[serde(deny_unknown_fields)]` — 带字段的 action 严格反序列化

use gpui::{Action, actions};

/// 空初始化函数 — 防止 Rust 优化掉 action 注册（对齐 zed L6-L13）。
/// 必须在 main.rs 里被调用一次。
pub fn init() {}

// ============================================================
// aagent 主命名空间（对齐 zed 的 `zed` 主命名空间）
// ============================================================

actions!(
    aagent,
    [
        /// Opens the settings editor.
        OpenSettings,
        /// Quits the application.
        Quit,
        /// Shows information about AAgent.
        About,
    ]
);

// ============================================================
// agents_sidebar — Sidebar 相关 action（对齐 zed L925-L946）
// ============================================================

pub mod agents_sidebar {
    use gpui::{Action, actions};

    /// Toggles the thread switcher popup when the sidebar is focused.
    /// 带字段的 action — 用 derive 宏（需要 Clone + PartialEq + Deserialize + JsonSchema）。
    #[derive(Clone, PartialEq, Default, serde::Deserialize, schemars::JsonSchema, Action)]
    #[action(namespace = agents_sidebar)]
    #[serde(deny_unknown_fields)]
    pub struct ToggleThreadSwitcher {
        #[serde(default)]
        pub select_last: bool,
    }

    actions!(
        agents_sidebar,
        [
            /// Opens/closes the sidebar on the current side.
            ToggleSidebar,
            /// Moves focus to the sidebar's search/filter editor.
            FocusSidebarFilter,
            /// Adds a new thread in the sidebar.
            NewThread,
        ]
    );
}

// ============================================================
// assistant — Agent Panel 相关（对齐 zed L658-L694）
// ============================================================

pub mod assistant {
    use gpui::actions;

    actions!(
        assistant,
        [
            /// Toggles the agent panel.
            Toggle,
            /// Toggles focus on the agent panel.
            ToggleFocus,
            /// Focuses the agent panel without opening it if closed.
            FocusAgent,
        ]
    );
}

// ============================================================
// dock — Dock 面板切换（无 zed 对应 — AAgent 自定义）
// ============================================================

pub mod dock {
    use gpui::actions;

    actions!(
        dock,
        [
            /// Toggles the left dock (Agent Panel).
            ToggleLeftDock,
            /// Toggles the right dock (Project/Git/Outline...).
            ToggleRightDock,
            /// Toggles the bottom dock (Terminal/Debug...).
            ToggleBottomDock,
        ]
    );
}

// ============================================================
// theme
// ============================================================

pub mod theme {
    use gpui::actions;

    actions!(theme, [ToggleMode]);
}

// ============================================================
// dev — 开发/调试
// ============================================================

pub mod dev {
    use gpui::actions;

    actions!(
        dev,
        [
            /// Toggles the developer inspector for debugging UI elements.
            ToggleInspector,
        ]
    );
}

//! ActivateItem action — 带字段的 action，用于跳转到 Pane 里指定 index 的 item。
//!
//! 其他无参 actions 用 `actions!` 宏在 mod.rs 统一声明。

use gpui::Action;

/// 激活指定 index 的 item。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema, gpui::Action,
)]
#[action(namespace = pane)]
pub struct ActivateItem(pub usize);

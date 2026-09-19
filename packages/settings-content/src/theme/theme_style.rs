//! 完整主题文件的反序列化目标 + 相关辅助类型。
//!
//! - `ThemeStyleContent` 是主题 JSON 的顶层结构体（`flatten` colors + status + syntax + accents + players）
//! - `WindowBackgroundContent` 窗口背景外观
//! - `AccentContent` accent 颜色条目
//! - `PlayerColorContent` 多人光标颜色

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::theme::highlight_style::HighlightStyleContent;
use crate::theme::status_colors::StatusColorsContent;
use crate::theme::theme_color::{ThemeColor, ThemeColorsContent};

// ---------- WindowBackgroundContent ----------

/// 窗口背景外观。
#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowBackgroundContent {
    Opaque,
    Transparent,
    Blurred,
}

// ---------- AccentContent / PlayerColorContent ----------

/// accent 颜色条目（可空）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccentContent(pub Option<ThemeColor>);

/// 多人光标颜色。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerColorContent {
    pub cursor: Option<ThemeColor>,
    pub background: Option<ThemeColor>,
    pub selection: Option<ThemeColor>,
}

// ---------- ThemeStyleContent ----------

/// 完整主题文件的反序列化目标。
///
/// 用于从主题 JSON 文件一次性解析出所有颜色 + 状态 + syntax + accent + players。
/// `colors` 和 `status` 用 `#[serde(flatten)]` 内联，使 JSON 结构扁平（所有 `border` / `editor.*` / `conflict` 等都在顶层）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ThemeStyleContent {
    #[serde(rename = "background.appearance")]
    pub window_background_appearance: Option<WindowBackgroundContent>,

    #[serde(default)]
    pub accents: Vec<AccentContent>,

    #[serde(flatten, default)]
    pub colors: ThemeColorsContent,

    #[serde(flatten, default)]
    pub status: StatusColorsContent,

    #[serde(default)]
    pub players: Vec<PlayerColorContent>,

    /// 语法高亮。
    #[serde(default)]
    pub syntax: BTreeMap<String, HighlightStyleContent>,
}

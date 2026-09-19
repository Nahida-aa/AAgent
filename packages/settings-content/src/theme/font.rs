//! 字体相关 settings 类型。
//!
//! 对齐 Zed `settings_content::theme` 中的字体部分。

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use settings_macros::MergeFrom;

// ---------- FontSize ----------

/// 字体大小（像素）。
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize, MergeFrom)]
#[serde(transparent)]
pub struct FontSize(pub f32);

impl From<f32> for FontSize {
    fn from(v: f32) -> Self {
        Self(v)
    }
}

impl From<FontSize> for f32 {
    fn from(v: FontSize) -> Self {
        v.0
    }
}

// ---------- FontFamilyName ----------

/// 字体族名称（包 `Arc<str>`）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, MergeFrom)]
#[serde(transparent)]
pub struct FontFamilyName(pub Arc<str>);

impl From<String> for FontFamilyName {
    fn from(s: String) -> Self {
        Self(Arc::from(s))
    }
}

impl From<&str> for FontFamilyName {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

// ---------- FontWeightContent ----------

/// 字体粗细（CSS 单位 100-900）。
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize, MergeFrom)]
#[serde(transparent)]
pub struct FontWeightContent(pub f32);

impl FontWeightContent {
    pub const THIN: Self = Self(100.0);
    pub const EXTRA_LIGHT: Self = Self(200.0);
    pub const LIGHT: Self = Self(300.0);
    pub const NORMAL: Self = Self(400.0);
    pub const MEDIUM: Self = Self(500.0);
    pub const SEMIBOLD: Self = Self(600.0);
    pub const BOLD: Self = Self(700.0);
    pub const EXTRA_BOLD: Self = Self(800.0);
    pub const BLACK: Self = Self(900.0);
}

// ---------- FontStyleContent ----------

/// 字体样式（serif / italic / oblique）。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum FontStyleContent {
    Normal,
    Italic,
    Oblique,
}

// ---------- BufferLineHeight ----------

/// 编辑器行高。
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, MergeFrom)]
#[serde(rename_all = "snake_case")]
pub enum BufferLineHeight {
    #[default]
    Comfortable,
    Standard,
    Custom(f32),
}

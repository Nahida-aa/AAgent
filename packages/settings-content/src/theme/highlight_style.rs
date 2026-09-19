//! 语法高亮样式。

use serde::{Deserialize, Serialize};
use settings_macros::{MergeFrom, with_fallible_options};

use crate::theme::font::{FontStyleContent, FontWeightContent};
use crate::theme::theme_color::ThemeColor;

/// 语法高亮样式（颜色 + 可选字体样式/粗细）。
///
/// `#[with_fallible_options]` 会自动给每个 Option 字段加：
/// `#[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::fallible_options::deserialize")]`
#[with_fallible_options]
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, MergeFrom)]
#[serde(default)]
pub struct HighlightStyleContent {
    pub color: Option<ThemeColor>,
    pub background_color: Option<ThemeColor>,
    pub font_style: Option<FontStyleContent>,
    pub font_weight: Option<FontWeightContent>,
}

impl HighlightStyleContent {
    pub fn is_empty(&self) -> bool {
        self.color.is_none()
            && self.background_color.is_none()
            && self.font_style.is_none()
            && self.font_weight.is_none()
    }
}

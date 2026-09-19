//! 语法高亮样式。

use serde::{Deserialize, Serialize};

use crate::theme::font::{FontStyleContent, FontWeightContent};
use crate::theme::theme_color::ThemeColor;

/// 语法高亮样式（颜色 + 可选字体样式/粗细）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HighlightStyleContent {
    pub color: Option<ThemeColor>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub background_color: Option<ThemeColor>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub font_style: Option<FontStyleContent>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
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

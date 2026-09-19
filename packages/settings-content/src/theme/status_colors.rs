//! 诊断/VCS 状态色。
//!
//! conflict / created / deleted / error / hidden / hint / ignored / info / modified / predictive / renamed / success / unreachable / warning
//!
//! 每个 status 有三组：单色 + `.background` + `.border`。

use serde::{Deserialize, Serialize};

use crate::theme::theme_color::ThemeColor;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct StatusColorsContent {
    #[serde(rename = "conflict")]
    pub conflict: Option<ThemeColor>,
    #[serde(rename = "conflict.background")]
    pub conflict_background: Option<ThemeColor>,
    #[serde(rename = "conflict.border")]
    pub conflict_border: Option<ThemeColor>,

    #[serde(rename = "created")]
    pub created: Option<ThemeColor>,
    #[serde(rename = "created.background")]
    pub created_background: Option<ThemeColor>,
    #[serde(rename = "created.border")]
    pub created_border: Option<ThemeColor>,

    #[serde(rename = "deleted")]
    pub deleted: Option<ThemeColor>,
    #[serde(rename = "deleted.background")]
    pub deleted_background: Option<ThemeColor>,
    #[serde(rename = "deleted.border")]
    pub deleted_border: Option<ThemeColor>,

    #[serde(rename = "error")]
    pub error: Option<ThemeColor>,
    #[serde(rename = "error.background")]
    pub error_background: Option<ThemeColor>,
    #[serde(rename = "error.border")]
    pub error_border: Option<ThemeColor>,

    #[serde(rename = "hidden")]
    pub hidden: Option<ThemeColor>,
    #[serde(rename = "hidden.background")]
    pub hidden_background: Option<ThemeColor>,
    #[serde(rename = "hidden.border")]
    pub hidden_border: Option<ThemeColor>,

    #[serde(rename = "hint")]
    pub hint: Option<ThemeColor>,
    #[serde(rename = "hint.background")]
    pub hint_background: Option<ThemeColor>,
    #[serde(rename = "hint.border")]
    pub hint_border: Option<ThemeColor>,

    #[serde(rename = "ignored")]
    pub ignored: Option<ThemeColor>,
    #[serde(rename = "ignored.background")]
    pub ignored_background: Option<ThemeColor>,
    #[serde(rename = "ignored.border")]
    pub ignored_border: Option<ThemeColor>,

    #[serde(rename = "info")]
    pub info: Option<ThemeColor>,
    #[serde(rename = "info.background")]
    pub info_background: Option<ThemeColor>,
    #[serde(rename = "info.border")]
    pub info_border: Option<ThemeColor>,

    #[serde(rename = "modified")]
    pub modified: Option<ThemeColor>,
    #[serde(rename = "modified.background")]
    pub modified_background: Option<ThemeColor>,
    #[serde(rename = "modified.border")]
    pub modified_border: Option<ThemeColor>,

    #[serde(rename = "predictive")]
    pub predictive: Option<ThemeColor>,
    #[serde(rename = "predictive.background")]
    pub predictive_background: Option<ThemeColor>,
    #[serde(rename = "predictive.border")]
    pub predictive_border: Option<ThemeColor>,

    #[serde(rename = "renamed")]
    pub renamed: Option<ThemeColor>,
    #[serde(rename = "renamed.background")]
    pub renamed_background: Option<ThemeColor>,
    #[serde(rename = "renamed.border")]
    pub renamed_border: Option<ThemeColor>,

    #[serde(rename = "success")]
    pub success: Option<ThemeColor>,
    #[serde(rename = "success.background")]
    pub success_background: Option<ThemeColor>,
    #[serde(rename = "success.border")]
    pub success_border: Option<ThemeColor>,

    #[serde(rename = "unreachable")]
    pub unreachable: Option<ThemeColor>,
    #[serde(rename = "unreachable.background")]
    pub unreachable_background: Option<ThemeColor>,
    #[serde(rename = "unreachable.border")]
    pub unreachable_border: Option<ThemeColor>,

    #[serde(rename = "warning")]
    pub warning: Option<ThemeColor>,
    #[serde(rename = "warning.background")]
    pub warning_background: Option<ThemeColor>,
    #[serde(rename = "warning.border")]
    pub warning_border: Option<ThemeColor>,
}

//! RegisterSetting 编译期注册项 + SettingValue 运行时存储。
//!
//! 对齐 Zed `crates/settings/src/settings_store.rs` 中的同名类型。

use std::any::{Any, TypeId};
use std::path::PathBuf;
use std::sync::Arc;

use path::rel_path::RelPath;

use crate::SettingsContent;
use crate::{Settings, WorktreeId};

// ---------- SettingValue ----------

/// 每种 setting 类型的运行时值存储。
///
/// 对齐 Zed：
/// - `global_value` — 全局（worktree 级别）的 setting 值
/// - `local_values` — per-worktree + per-project 的本地覆盖
///   当前 AAgent 还没多 worktree / 多 workspace，Vec 始终为空，
///   但 Zed 形状先完整搭好，以后直接填值。
#[doc(hidden)]
#[derive(Debug)]
pub struct SettingValue<T> {
    #[doc(hidden)]
    pub global_value: Option<T>,
    #[doc(hidden)]
    pub local_values: Vec<(WorktreeId, Arc<RelPath>, T)>,
}

impl<T> Default for SettingValue<T> {
    fn default() -> Self {
        Self {
            global_value: None,
            local_values: Vec::new(),
        }
    }
}

// ---------- AnySettingValue ----------

/// Type-erased 的 SettingValue trait object。
pub trait AnySettingValue: 'static + Send + Sync {
    /// 设置类型名（debug 用）。
    fn type_name(&self) -> &'static str;

    /// 从 SettingsContent 构造一个值（用于初始化）。
    fn from_settings(&self, content: &SettingsContent) -> Box<dyn Any>;
}

impl<T: Settings> AnySettingValue for SettingValue<T> {
    fn type_name(&self) -> &'static str { std::any::type_name::<T>() }

    fn from_settings(&self, content: &SettingsContent) -> Box<dyn Any> {
        Box::new(T::from_settings(content))
    }
}

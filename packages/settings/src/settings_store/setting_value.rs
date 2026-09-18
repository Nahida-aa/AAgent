//! RegisterSetting 编译期注册项 + SettingValue 运行时存储。
//!
//! 对齐 Zed `crates/settings/src/settings_store.rs` 中的同名类型。

use std::any::{Any, TypeId};
use std::path::PathBuf;
use std::sync::Arc;

use crate::Settings;
use crate::SettingsContent;

// ---------- WorktreeId + RelPath ----------

/// AAgent 简化版 WorktreeId（Zed 里是复杂的多 worktree 系统）。
/// 当前单 worktree 场景下固定为 0，留着以后扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WorktreeId(pub usize);

/// 简化版相对路径 — Zed 用 `RelPath`，AAgent 先用 PathBuf 占位。
/// 存的是 worktree 根目录下的相对路径（比如项目目录、配置子目录）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct RelPath(pub PathBuf);

// ---------- RegisteredSetting ----------

/// inventory 编译期收集的注册条目。
///
/// 每个 `#[derive(RegisterSetting)]` 生成一个这样的条目，由
/// `inventory::collect!` 在 `SettingsStore::init()` 时拉取。
pub struct RegisteredSetting {
    pub settings_value: fn() -> Box<dyn AnySettingValue>,
    pub from_settings: fn(&SettingsContent) -> Box<dyn Any>,
    pub id: fn() -> TypeId,
}

inventory::collect!(RegisteredSetting);

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
    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }

    fn from_settings(&self, content: &SettingsContent) -> Box<dyn Any> {
        Box::new(T::from_settings(content))
    }
}

//! AAgent 设置系统（对齐 Zed `crates/settings`，极简版）。
//!
//! 基础设施层 — SettingsStore + Settings trait + RegisterSetting 支持。
//! Setting struct 在 `settings-content` crate（language_model / agent / editor ...）。
//!
//! 加载模型：
//!   1. RustEmbed 内嵌的 `settings/default.json`（兜底）
//!   2. `~/.config/aa/settings.json`（用户覆盖）
//!
//! 注册机制：
//!   - 每种 setting struct 用 `#[derive(RegisterSetting)]` 标记
//!   - 宏展开生成 `inventory::submit!` 条目（编译期收集）
//!   - `SettingsStore::init()` 时遍历 `inventory::collect!(RegisteredSetting)` 初始化

use rust_embed::RustEmbed;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

pub mod settings_store;
use settings_macros::{MergeFrom, with_fallible_options};
pub use settings_store::{
    AnySettingValue, RegisteredSetting, RelPath, SettingValue, SettingsStore, WorktreeId,
};

// ---------- SettingsContent ----------

/// 设置内容的统一入口 — 简化版对齐 Zed `settings_json::SettingsContent`。
/// Zed 版本有 layered content tracking（User / Default / Server / Project），
/// AAgent 简化版就是一个 `serde_json::Value` wrapper。
#[with_fallible_options]
#[derive(
    Debug, PartialEq, Default, Clone, Serialize, JsonSchema, MergeFrom, Clone, Debug, Default,
)]
pub struct SettingsContent {
    pub value: Value,
    /// Configuration of the terminal in Zed.
    pub terminal: Option<TerminalSettingsContent>,
}

impl SettingsContent {
    pub fn new(value: Value) -> Self {
        Self { value }
    }

    pub fn empty() -> Self {
        Self {
            value: Value::Object(serde_json::Map::new()),
        }
    }

    /// 按路径取一个字段（同 SettingsStore::try_get_path）。
    pub fn try_get<T: serde::de::DeserializeOwned>(&self, path: &[&str]) -> Option<T> {
        let mut current = &self.value;
        for key in path {
            current = match current.get(*key) {
                Some(v) => v,
                None => return None,
            };
        }
        serde_json::from_value(current.clone()).ok()
    }

    /// 从合并后的 Value 树反序列化整个结构体。
    pub fn deserialize<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.value.clone())
    }
}

// ---------- Settings trait ----------

/// 可以从设置文件反序列化的类型。
///
/// 每个 setting struct（如 `AgentSettings`、`LanguageModelSettings`）实现这个 trait。
/// 宏 `#[derive(RegisterSetting)]` 要求类型也实现本 trait。
pub trait Settings: 'static + Sized + Send + Sync + DeserializeOwned {
    /// 从合并后的 SettingsContent 反序列化。
    ///
    /// # 约定
    /// - default.json 里必须有这个 setting 的 key（和 json 字段名一致）
    /// - 缺失时 panic（运行时错误 → 开发者漏了 default.json entry）
    fn from_settings(content: &SettingsContent) -> Self;

    /// 从一个 JSON Value 直接反序列化（便利方法）。
    fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }
}

// ---------- private 模块（宏需要） ----------

/// #[doc(hidden)] — 给 `settings-macros::RegisterSetting` 宏展开用的类型。
#[doc(hidden)]
pub mod private {
    pub use crate::settings_store::setting_value::{RegisteredSetting, SettingValue};
    pub use inventory;
}

// ---------- RustEmbed ----------

/// 对齐 Zed crates/settings/src/settings.rs 的 SettingsAssets 模式：
/// settings 归 settings crate 自己 embed，icons/fonts 归 aa-gpui-kit-assets。
#[derive(RustEmbed)]
#[folder = "../../assets"]
#[include = "settings/*"]
#[exclude = "*.DS_Store"]
pub struct SettingsAssets;

/// 从内嵌资源读取 default.json 原始文本。
pub fn embedded_default_json() -> std::borrow::Cow<'static, str> {
    match SettingsAssets::get("settings/default.json")
        .expect("settings/default.json must be embedded")
        .data
    {
        std::borrow::Cow::Borrowed(bytes) => std::borrow::Cow::Borrowed(
            std::str::from_utf8(bytes).expect("embedded default.json is UTF-8"),
        ),
        std::borrow::Cow::Owned(bytes) => std::borrow::Cow::Owned(
            String::from_utf8(bytes).expect("embedded default.json is UTF-8"),
        ),
    }
}

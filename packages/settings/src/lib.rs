//! AAgent 设置系统（对齐 Zed `crates/settings`，极简版）。
//!
//! 两层加载模型：
//!   1. RustEmbed 内嵌的 `settings/default.json`（兜底）
//!   2. 用户 `~/.config/aa/settings.json`（覆盖 —— 后续接入）
//!
//! 模块：
//! - `settings_store` — SettingsStore struct（gpui Global，持有 JSON Value 树）
//! - `llm` — LLM 配置（Config/ProviderConfig/McpConfig，aa.json 旧格式兼容）

pub mod llm;
pub mod settings_store;

use std::borrow::Cow;

pub use llm::{Config, McpConfig, McpServerDef, ProviderConfig, ResolvedConfig};
pub use settings_store::SettingsStore;

use rust_embed::RustEmbed;

// ---------- RustEmbed ----------

/// 对齐 Zed crates/settings/src/settings.rs 的 SettingsAssets 模式：
/// settings 归 settings crate 自己 embed，icons/fonts 归 aa-gpui-kit-assets。
#[derive(RustEmbed)]
#[folder = "../../assets"]
#[include = "settings/*"]
#[exclude = "*.DS_Store"]
pub struct SettingsAssets;

/// 从内嵌资源读取 default.json 原始文本。
pub fn embedded_default_json() -> Cow<'static, str> {
    match SettingsAssets::get("settings/default.json")
        .expect("settings/default.json must be embedded")
        .data
    {
        Cow::Borrowed(bytes) => {
            Cow::Borrowed(std::str::from_utf8(bytes).expect("embedded default.json is UTF-8"))
        }
        Cow::Owned(bytes) => {
            Cow::Owned(String::from_utf8(bytes).expect("embedded default.json is UTF-8"))
        }
    }
}

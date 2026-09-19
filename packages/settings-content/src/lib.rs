//! 设置内容定义（对齐 Zed `crates/settings_content`）。
//!
//! 存放从 settings JSON 反序列化的 Setting struct。
//! 按功能分子模块：language_model、agent、editor、theme ...
//!
//! settings crate（settings + settings_store）是**基础设施**，
//! 负责 RustEmbed + SettingsStore Global；
//! settings_content 是**内容层**，只放数据结构。

pub mod agent;
pub mod dock;
pub mod language_model;
pub mod theme;

pub use agent::{SidebarDockPosition, SidebarSide};
pub use dock::DockPosition;
pub use language_model::{Config, McpConfig, McpServerDef, ProviderConfig, ResolvedConfig};
pub use theme::{
    BufferLineHeight, DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME, FontFamilyName, FontSize,
    FontWeightContent, IconThemeName, IconThemeSelection, ThemeAppearanceMode, ThemeName,
    ThemeSelection, ThemeSettingsContent, UiDensity,
};

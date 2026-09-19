//! 设置内容定义（对齐 Zed `crates/settings_content`）。
//!
//! 存放从 settings JSON 反序列化的 Setting struct。
//! 按功能分子模块：language_model、agent、editor、theme ...
//!
//! settings crate（settings + settings_store）是**基础设施**，
//! 负责 RustEmbed + SettingsStore Global；
//! settings_content 是**内容层**，只放数据结构。

pub mod fallible_options;
pub mod merge_from;

pub mod agent;
pub mod dock;
pub mod language_model;
pub mod theme;

// ---------- 通用工具 re-export ----------

pub use fallible_options::{FallibleOption, deserialize as deserialize_fallible};
pub use merge_from::MergeFrom;

// ---------- 各子模块 re-export ----------

pub use agent::{SidebarDockPosition, SidebarSide};
pub use dock::DockPosition;
pub use language_model::{Config, McpConfig, McpServerDef, ProviderConfig, ResolvedConfig};
pub use theme::{
    AccentContent, BufferLineHeight, DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME, FontFamilyName,
    FontSize, FontStyleContent, FontWeightContent, HighlightStyleContent, IconThemeName,
    IconThemeSelection, PlayerColorContent, StatusColorsContent, ThemeAppearanceMode, ThemeColor,
    ThemeColorsContent, ThemeName, ThemeSelection, ThemeSettingsContent, ThemeStyleContent,
    UiDensity, WindowBackgroundContent,
};

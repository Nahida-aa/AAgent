//! Terminal settings — 数据类型定义在 `settings_content::terminal`。
//!
//! 这个模块只做 re-export，让终端 backend 用 `terminal::settings::*` 访问。
//! 真正的定义在 `packages/settings-content/src/terminal/`。

pub use settings_content::terminal::*;

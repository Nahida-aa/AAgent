//! 全局设置存储（对齐 Zed `crates/settings/src/settings_store.rs`）。
//!
//! 持有一棵 `serde_json::Value` 值树（从 RustEmbed 内嵌的 default.json 解析），
//! 运行时可叠加用户 settings.json 覆盖。
//! 各 Setting struct 通过 `SettingsStore::get_path` / `get_raw` 读取。

pub mod setting_value;

pub use setting_value::{AnySettingValue, RegisteredSetting, RelPath, SettingValue, WorktreeId};

use std::path::PathBuf;

use gpui::{App, Global};
use serde::de::DeserializeOwned;
use serde_json::Value;

/// 全局设置存储。gpui `Global` trait 让它能通过 `cx.global::<SettingsStore>()` 访问。
pub struct SettingsStore {
    /// 内嵌 default.json 解析后的 JSON 值树（兜底）
    defaults: Value,
    /// 运行时可覆盖的 JSON 值树（用户 settings.json + 运行时 set_override）
    overrides: Value,
}

impl SettingsStore {
    /// 应用启动时调用，初始化 SettingsStore 并注册为 gpui Global。
    /// 用 json5 解析（default.json 有 `//` 注释，serde_json 不支持）。
    pub fn init(cx: &mut App) {
        let defaults: Value = json5::from_str(&crate::embedded_default_json())
            .expect("default.json must be valid JSON5");
        let mut store = Self {
            defaults,
            overrides: Value::Object(serde_json::Map::new()),
        };
        // 尝试加载用户 settings.json（没有就跳过）
        store.try_load_user_settings();
        cx.set_global(store);
    }

    /// 尝试从 `~/.config/aa/settings.json` 加载用户覆盖。
    /// 文件不存在或解析失败 → 静默忽略（default.json 兜底）。
    fn try_load_user_settings(&mut self) {
        let path = user_settings_path();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return, // 文件不存在，跳过
        };
        let user_value: Value = match json5::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("settings: user settings parse error ({path:?}): {e}");
                return;
            }
        };
        self.overrides = merge_values(self.overrides.clone(), user_value);
    }

    /// 递归设置 overrides 中的一个值（key 不存在则创建中间路径）。
    pub fn set_override(&mut self, path: &[&str], value: Value) {
        let mut current = &mut self.overrides;
        for &key in &path[..path.len() - 1] {
            if !current.is_object() {
                *current = Value::Object(serde_json::Map::new());
            }
            current = current
                .as_object_mut()
                .unwrap()
                .entry(key.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
        }
        if let Some(last) = path.last() {
            if !current.is_object() {
                *current = Value::Object(serde_json::Map::new());
            }
            current
                .as_object_mut()
                .unwrap()
                .insert(last.to_string(), value);
        }
    }

    /// 合并后的设置值树（overrides 优先覆盖 defaults）。
    /// overrides 为空 Object 时直接返回 defaults clone（不做合并开销）。
    pub fn merged(&self) -> Value {
        if self.overrides.is_object() && !self.overrides.as_object().unwrap().is_empty() {
            merge_values(self.defaults.clone(), self.overrides.clone())
        } else {
            self.defaults.clone()
        }
    }

    /// 从 merged 设置里按路径取一个字段。
    /// 路径上任何一环缺失 → panic（和 Zed Settings::from_settings 行为一致）。
    pub fn get_path<T>(&self, path: &[&str]) -> T
    where
        T: DeserializeOwned,
    {
        self.try_get_path::<T>(path)
            .unwrap_or_else(|e| panic!("settings path `{}`: {e}", path.join(".")))
    }

    /// 非 panic 版本的 get_path。路径缺失或反序列化失败 → Err。
    pub fn try_get_path<T>(&self, path: &[&str]) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        let merged = self.merged();
        let mut current = &merged;
        for (i, key) in path.iter().enumerate() {
            match current.get(*key) {
                Some(next) => current = next,
                None => {
                    return Err(format!(
                        "missing key `{key}` at path `{}`",
                        path[..i].join(".")
                    ));
                }
            }
        }
        serde_json::from_value(current.clone()).map_err(|e| format!("deserialize failed: {e}"))
    }

    /// 从 merged 设置里反序列化整个 Value 树为 T。
    pub fn get_raw<T>(&self) -> T
    where
        T: DeserializeOwned,
    {
        serde_json::from_value(self.merged())
            .unwrap_or_else(|e| panic!("settings deserialize failed: {e}"))
    }
}

impl Global for SettingsStore {}

/// 递归合并两个 JSON Value（source 覆盖 target 中同路径的值）。
fn merge_values(target: Value, source: Value) -> Value {
    match (target, source) {
        (Value::Object(mut t), Value::Object(s)) => {
            for (k, sv) in s {
                let tv = t.remove(&k);
                let merged = match tv {
                    Some(tv) => merge_values(tv, sv),
                    None => sv,
                };
                t.insert(k, merged);
            }
            Value::Object(t)
        }
        (_, source) => source,
    }
}

/// 用户 settings.json 路径：`$HOME/.config/aa/settings.json`。
/// Zed 对应 `~/.config/zed/settings.json`。
fn user_settings_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("aa")
        .join("settings.json")
}

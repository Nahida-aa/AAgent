//! 全局设置存储（对齐 Zed `crates/settings/src/settings_store.rs`）。
//!
//! 持有一棵 `serde_json::Value` 值树（从 RustEmbed 内嵌的 default.json 解析），
//! 运行时可叠加用户 settings.json 覆盖。
//! 各 Setting struct 通过 `SettingsStore::get_path` / `get_raw` 读取。

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
        let store = Self {
            defaults,
            overrides: Value::Object(serde_json::Map::new()),
        };
        cx.set_global(store);
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
    /// 例如 `store.get_path::<f64>(["ui", "ui_font_size"])`。
    /// 路径上任何一环缺失 → panic（和 Zed Settings::from_settings 行为一致）。
    pub fn get_path<T>(&self, path: &[&str]) -> T
    where
        T: DeserializeOwned,
    {
        let merged = self.merged();
        let mut current = &merged;
        for (i, key) in path.iter().enumerate() {
            let next = current.get(*key).unwrap_or_else(|| {
                panic!(
                    "settings path `{}` missing key `{key}`",
                    path[..i].join(".")
                )
            });
            current = next;
        }
        serde_json::from_value(current.clone()).unwrap_or_else(|e| {
            panic!("settings path `{}` deserialize failed: {e}", path.join("."))
        })
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

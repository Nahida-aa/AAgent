//! AAgent 设置系统（对齐 Zed `crates/settings`，极简版）。
//!
//! 两层加载模型：
//!   1. RustEmbed 内嵌的 `settings/default.json`（兜底）
//!   2. `aa.json`（后续覆盖 —— 目前还没接）
//!
//! 不做 Zed 那套 Settings trait / RegisterSetting 宏 / 文件 watch / 多层合并。
//! 各 Setting struct 直接 `serde::Deserialize` 从 JSON Value 树读，各自字段默认自己管。

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;

use gpui::{App, Global};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

// ---------- RustEmbed ----------

/// 对齐 Zed crates/settings/src/settings.rs 的 SettingsAssets 模式：
/// settings 归 settings crate 自己 embed，icons/fonts 归 aa-gpui-kit-assets。
#[derive(RustEmbed)]
#[folder = "../../assets"]
#[include = "settings/*"]
#[exclude = "*.DS_Store"]
pub struct SettingsAssets;

/// 从内嵌资源读取 default.json 原始文本。
fn embedded_default_json() -> Cow<'static, str> {
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

// ---------- SettingsStore ----------

/// 全局设置存储。gpui `Global` trait 让它能通过 `cx.global::<SettingsStore>()` 访问。
///
/// 内部持有一棵 serde_json::Value（从 default.json 解析的完整 JSON 值树）。
/// 各 Setting struct 通过 `SettingsStore::get_raw::<T>()` 从 Value 树反序列化。
pub struct SettingsStore {
    /// 内嵌 default.json 解析后的 JSON 值树（兜底）
    defaults: Value,
    /// 运行时可覆盖的 JSON 值树（目前为空占位）
    overrides: Value,
}

impl SettingsStore {
    /// 应用启动时调用，初始化 SettingsStore 并注册为 gpui Global。
    /// 用 json5 解析（default.json 有 `//` 注释，serde_json 不支持）。
    pub fn init(cx: &mut App) {
        let defaults: Value =
            json5::from_str(&embedded_default_json()).expect("default.json must be valid JSON5");
        let store = Self {
            defaults,
            overrides: Value::Object(serde_json::Map::new()),
        };
        cx.set_global(store);
    }

    /// 合并后的设置值树（overrides 优先覆盖 defaults）。
    /// 目前 overrides 永远是空 Object，但合并基础设施在这了。
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

/// 完整的 LLM 配置。
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// `provider/model` 格式的模型标识。
    #[serde(default)]
    pub model: Option<String>,
    /// 按 provider 名称索引的配置。
    #[serde(default)]
    pub provider: HashMap<String, ProviderConfig>,
    /// MCP 服务器配置。
    #[serde(default)]
    pub mcp: Option<McpConfig>,
}

/// 单个 provider 的配置。
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
}

/// MCP 服务器配置。
#[derive(Debug, Clone, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: HashMap<String, McpServerDef>,
}

/// 单个 MCP 服务器定义。
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerDef {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// 解析后的运行配置（所有字段都已填充默认值）。
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// provider 标识，如 `"openai"` 或 `"ollama"`。
    pub provider: String,
    /// 模型名称，如 `"gpt-4o-mini"`。
    pub model: String,
    /// API key（可能为空）。
    pub api_key: String,
    /// API base URL。
    pub base_url: String,
}

impl Config {
    /// Get MCP servers as a JSON value (compatible with `McpToolProvider::from_json`),
    /// falling back to `AA_MCP_SERVERS` env var for backward compatibility.
    pub fn mcp_servers_json(&self) -> Option<serde_json::Value> {
        // aa.json has priority
        if let Some(mcp) = &self.mcp {
            if !mcp.servers.is_empty() {
                let map: serde_json::Map<String, serde_json::Value> = mcp
                    .servers
                    .iter()
                    .map(|(name, def)| {
                        let val = serde_json::json!({
                            "command": def.command,
                            "args": def.args,
                        });
                        (name.clone(), val)
                    })
                    .collect();
                return Some(serde_json::Value::Object(map));
            }
        }
        // Fall back to env var
        if let Ok(json_str) = std::env::var("AA_MCP_SERVERS") {
            if !json_str.is_empty() {
                if let Ok(val) = serde_json::from_str(&json_str) {
                    return Some(val);
                }
            }
        }
        None
    }

    /// 从标准位置加载 `aa.json`。
    ///
    /// 搜索路径（先找到就用）：
    /// 1. `./aa.json`
    /// 2. `$HOME/.config/aa/aa.json`
    pub fn load() -> Self {
        for path in config_paths() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str(&content) {
                    return cfg;
                }
            }
        }
        Config {
            model: None,
            provider: HashMap::new(),
            mcp: None,
        }
    }

    /// 将配置解析为运行时值。
    ///
    /// 优先级（高 → 低）：
    /// - CLI 参数（调用者传入的 `cli_*`）
    /// - `AA_MODEL` / `AA_PROVIDER` / `AA_API_KEY` / `AA_BASE_URL` env vars
    /// - 旧的 `AA_LLM_*` 环境变量（向后兼容）
    /// - `aa.json` 中的值
    /// - 硬编码默认值
    pub fn resolve(
        &self,
        cli_provider: Option<&str>,
        cli_model: Option<&str>,
        cli_base_url: Option<&str>,
    ) -> ResolvedConfig {
        // 决定 provider
        let provider = pick(
            cli_provider.map(String::from),
            env("AA_PROVIDER"),
            env("AA_LLM_PROVIDER"),
            self.model
                .as_ref()
                .and_then(|m| m.split_once('/'))
                .map(|(p, _)| p.to_string()),
            "ollama",
        );

        // 决定 model name
        let default_model = match provider.as_str() {
            "ollama" => "gemma4:31b-cloud",
            _ => "gpt-4o-mini",
        };

        let model = pick(
            cli_model.map(String::from),
            env("AA_MODEL"),
            env("AA_LLM_MODEL"),
            self.model.as_ref().map(|m| {
                m.split_once('/')
                    .map(|(_, m)| m.to_string())
                    .unwrap_or_else(|| m.clone())
            }),
            default_model,
        );

        // 决定 API key
        let api_key = pick(
            env("AA_API_KEY"),
            env("AA_LLM_API_KEY"),
            self.provider.get(&provider).and_then(|p| p.api_key.clone()),
            None,
            "",
        );

        // 决定 base URL
        let default_base_url = match provider.as_str() {
            "ollama" => "http://localhost:11434",
            _ => "https://api.openai.com/v1",
        };

        let base_url = pick(
            cli_base_url.map(String::from),
            env("AA_BASE_URL"),
            env("AA_LLM_BASE_URL"),
            self.provider
                .get(&provider)
                .and_then(|p| p.base_url.clone()),
            default_base_url,
        );

        ResolvedConfig {
            provider,
            model,
            api_key,
            base_url,
        }
    }
}

fn config_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("aa.json")];
    if let Ok(home) = std::env::var("HOME") {
        paths.push(
            PathBuf::from(home)
                .join(".config")
                .join("aa")
                .join("aa.json"),
        );
    }
    paths
}

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

/// 返回第一个 `Some` 值，全部为 `None` 时返回 `default`。
fn pick(
    a: Option<String>,
    b: Option<String>,
    c: Option<String>,
    d: Option<String>,
    default: &str,
) -> String {
    a.or(b).or(c).or(d).unwrap_or_else(|| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_defaults() {
        let cfg = Config {
            model: None,
            provider: HashMap::new(),
            mcp: None,
        };
        let resolved = cfg.resolve(None, None, None);
        assert_eq!(resolved.provider, "ollama");
        assert_eq!(resolved.model, "gemma4:31b-cloud");
        assert_eq!(resolved.base_url, "http://localhost:11434");
        assert_eq!(resolved.api_key, "");
    }

    #[test]
    fn test_resolve_from_model_string() {
        let cfg = Config {
            model: Some("ollama/llama3.2".into()),
            provider: HashMap::new(),
            mcp: None,
        };
        let resolved = cfg.resolve(None, None, None);
        assert_eq!(resolved.provider, "ollama");
        assert_eq!(resolved.model, "llama3.2");
        assert_eq!(resolved.base_url, "http://localhost:11434");
    }

    #[test]
    fn test_resolve_model_without_provider_prefix() {
        let cfg = Config {
            model: Some("deepseek-chat".into()),
            provider: HashMap::new(),
            mcp: None,
        };
        let resolved = cfg.resolve(None, None, None);
        assert_eq!(resolved.provider, "ollama");
        assert_eq!(resolved.model, "deepseek-chat");
    }

    #[test]
    fn test_resolve_cli_overrides_file() {
        let cfg = Config {
            model: Some("ollama/llama3.2".into()),
            provider: HashMap::new(),
            mcp: None,
        };
        let resolved = cfg.resolve(
            Some("openai"),
            Some("gpt-4o"),
            Some("https://custom.com/v1"),
        );
        assert_eq!(resolved.provider, "openai");
        assert_eq!(resolved.model, "gpt-4o");
        assert_eq!(resolved.base_url, "https://custom.com/v1");
    }

    #[test]
    fn test_resolve_provider_config_from_file() {
        let mut provider = HashMap::new();
        provider.insert(
            "openai".into(),
            ProviderConfig {
                api_key: Some("sk-from-file".into()),
                base_url: Some("https://file-url.com/v1".into()),
            },
        );
        let cfg = Config {
            model: Some("openai/gpt-4o".into()),
            provider,
            mcp: None,
        };
        let resolved = cfg.resolve(None, None, None);
        assert_eq!(resolved.api_key, "sk-from-file");
        assert_eq!(resolved.base_url, "https://file-url.com/v1");
    }

    #[test]
    fn test_config_loading() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("aa.json");
        let mut f = std::fs::File::create(&config_path).unwrap();
        f.write_all(br#"{"model": "ollama/llama3.2", "provider": {"ollama": {"base_url": "http://localhost:12345"}}}"#).unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let cfg = Config::load();
        std::env::set_current_dir(prev).unwrap();

        assert_eq!(cfg.model.as_deref(), Some("ollama/llama3.2"));
        assert_eq!(
            cfg.provider.get("ollama").unwrap().base_url.as_deref(),
            Some("http://localhost:12345")
        );
    }

    #[test]
    fn test_mcp_config_from_file() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("aa.json");
        let mut f = std::fs::File::create(&config_path).unwrap();
        f.write_all(br#"{"mcp": {"servers": {"my-tool": {"command": "npx", "args": ["-y", "mcp-server"]}}}}"#).unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let cfg = Config::load();
        std::env::set_current_dir(prev).unwrap();

        let json = cfg.mcp_servers_json().unwrap();
        assert_eq!(json["my-tool"]["command"], "npx");
        assert_eq!(json["my-tool"]["args"][0], "-y");
        assert_eq!(json["my-tool"]["args"][1], "mcp-server");
    }

    #[test]
    fn test_mcp_config_fallback_env() {
        let cfg = Config {
            model: None,
            provider: HashMap::new(),
            mcp: None,
        };
        assert!(cfg.mcp_servers_json().is_none());

        // SAFETY: test-only code
        unsafe {
            std::env::set_var(
                "AA_MCP_SERVERS",
                r#"{"env-tool": {"command": "docker", "args": []}}"#,
            )
        };
        let json = cfg.mcp_servers_json().unwrap();
        assert_eq!(json["env-tool"]["command"], "docker");
        unsafe { std::env::remove_var("AA_MCP_SERVERS") };
    }
}

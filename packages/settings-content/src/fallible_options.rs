//! 容错反序列化。
//!
//! 对齐 Zed `crates/settings_content/src/fallible_options.rs`，但做了大幅简化：
//!
//! Zed 版依赖 `thread_local!` + `anyhow` + `serde_json_lenient` + `serde_path_to_error`
//! 收集所有解析错误，最终返回 `(Option<T>, ParseStatus)`。
//!
//! **我们先只做核心容错** —— 单个字段反序列化失败时降级成 `Default::default()`（Option 的 default 就是 None）。
//! 以后 settings-store 升级到 Zed 风格时再改实现。
//!
//! 用法：配合 `#[with_fallible_options]` attribute，macro 自动给每个 Option<T> 字段加：
//! ```ignore
//! #[serde(default, skip_serializing_if = "Option::is_none",
//!           deserialize_with = "crate::fallible_options::deserialize")]
//! ```

use serde::Deserializer;
use serde::de::DeserializeOwned;

/// 标记可容错的类型（现在只有 Option<T>）。
pub trait FallibleOption: Default {}
impl<T> FallibleOption for Option<T> {}

/// 容错反序列化器 —— 解析失败时返回 Default::default()。
///
/// 用作 serde 的 `deserialize_with` 值：
/// ```ignore
/// #[serde(deserialize_with = "crate::fallible_options::deserialize")]
/// field: Option<u32>,
/// ```
pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + FallibleOption,
{
    match T::deserialize(deserializer) {
        Ok(value) => Ok(value),
        // 解析出错（比如字符串给了数字字段），降级为 None
        Err(_) => Ok(T::default()),
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use crate::fallible_options::deserialize as fallible_deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Test {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "fallible_deserialize"
        )]
        foo: Option<String>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "fallible_deserialize"
        )]
        bar: Option<u32>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "fallible_deserialize"
        )]
        baz: Option<bool>,
    }

    use crate::merge_from::MergeFrom;

    #[test]
    fn fallible_option_serde_value() {
        let t: Test = serde_json::from_value(json!({
            "foo": "bar",
            "bar": "not-a-number",
            "baz": 3,
        }))
        .unwrap();
        assert_eq!(
            t,
            Test {
                foo: Some("bar".into()),
                bar: None,
                baz: None,
            }
        );
    }

    // MergeFrom impl for Test...
    impl MergeFrom for Test {
        fn merge_from(&mut self, other: &Self) {
            self.foo.merge_from(&other.foo);
            self.bar.merge_from(&other.bar);
            self.baz.merge_from(&other.baz);
        }
    }

    impl Clone for Test {
        fn clone(&self) -> Self {
            Test {
                foo: self.foo.clone(),
                bar: self.bar.clone(),
                baz: self.baz.clone(),
            }
        }
    }
}

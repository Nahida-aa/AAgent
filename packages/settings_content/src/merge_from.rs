/// Trait for recursively merging settings structures.
///
/// 对齐 Zed `crates/settings_content/src/merge_from.rs`。
///
/// When settings are loaded, values from different sources (defaults → user → project)
/// need to be combined. `MergeFrom` defines how:
///
/// * **Option<T>** — None ignores, Some recursively merges (deep)
/// * **Vec / BTreeSet / HashSet** — overwrite（完全替换）
/// * **HashMap / BTreeMap** — deep merge（相同 key 递归，新 key 插入）
/// * **serde_json::Value** — Object deep merge, 其他 overwrite
/// * **primitive types** — overwrite
///
/// # Example
/// ```ignore
/// #[derive(Clone, MergeFrom)]
/// struct MySettings {
///     font_size: Option<u32>,
///     language: Option<String>,
/// }
/// // merge_from_option 提供方便的 None-ignore 语义
/// let mut base = MySettings::default();
/// base.merge_from_option(user_settings.as_ref());
/// ```
pub trait MergeFrom {
    /// Merge from a source of the same type.
    fn merge_from(&mut self, other: &Self);

    /// Merge from an optional source — None 时跳过。
    fn merge_from_option(&mut self, other: Option<&Self>) {
        if let Some(other) = other {
            self.merge_from(other);
        }
    }
}

// ---------- primitive overwrites ----------

macro_rules! merge_from_overwrites {
    ($($type:ty),+ $(,)?) => {
        $(
            impl MergeFrom for $type {
                fn merge_from(&mut self, other: &Self) {
                    *self = other.clone();
                }
            }
        )+
    }
}

merge_from_overwrites!(
    u16,
    u32,
    u64,
    usize,
    i16,
    i32,
    i64,
    bool,
    f64,
    f32,
    char,
    std::num::NonZeroUsize,
    std::num::NonZeroU32,
    String,
    std::sync::Arc<str>,
    std::path::PathBuf,
    std::sync::Arc<std::path::Path>,
);

// ---------- Option<T> deep merge ----------

impl<T: Clone + MergeFrom> MergeFrom for Option<T> {
    fn merge_from(&mut self, other: &Self) {
        let Some(other) = other else {
            return;
        };

        if let Some(this) = self {
            this.merge_from(other);
        } else {
            self.replace(other.clone());
        }
    }
}

// ---------- Vec overwrite ----------

impl<T: Clone> MergeFrom for Vec<T> {
    fn merge_from(&mut self, other: &Self) {
        *self = other.clone()
    }
}

// ---------- Box<T> ----------

impl<T: MergeFrom> MergeFrom for Box<T> {
    fn merge_from(&mut self, other: &Self) {
        self.as_mut().merge_from(other.as_ref())
    }
}

// ---------- collections ----------

impl<K, V> MergeFrom for std::collections::HashMap<K, V>
where
    K: Clone + std::hash::Hash + Eq,
    V: Clone + MergeFrom,
{
    fn merge_from(&mut self, other: &Self) {
        for (key, value) in other {
            if let Some(existing) = self.get_mut(key) {
                existing.merge_from(value);
            } else {
                self.insert(key.clone(), value.clone());
            }
        }
    }
}

impl<K, V, S> MergeFrom for collections::IndexMap<K, V, S>
where
    K: Clone + std::hash::Hash + Eq,
    V: Clone + MergeFrom,
    S: Default,
{
    fn merge_from(&mut self, other: &Self) {
        for (key, value) in other {
            if let Some(existing) = self.get_mut(key) {
                existing.merge_from(value);
            } else {
                self.insert(key.clone(), value.clone());
            }
        }
    }
}

impl<K, V> MergeFrom for std::collections::BTreeMap<K, V>
where
    K: Clone + std::hash::Hash + Eq + Ord,
    V: Clone + MergeFrom,
{
    fn merge_from(&mut self, other: &Self) {
        for (key, value) in other {
            if let Some(existing) = self.get_mut(key) {
                existing.merge_from(value);
            } else {
                self.insert(key.clone(), value.clone());
            }
        }
    }
}

impl<T> MergeFrom for std::collections::BTreeSet<T>
where
    T: Clone + Ord,
{
    fn merge_from(&mut self, other: &Self) {
        for item in other {
            self.insert(item.clone());
        }
    }
}

impl<T> MergeFrom for std::collections::HashSet<T>
where
    T: Clone + std::hash::Hash + Eq,
{
    fn merge_from(&mut self, other: &Self) {
        for item in other {
            self.insert(item.clone());
        }
    }
}

// ---------- serde_json::Value deep merge ----------

impl MergeFrom for serde_json::Value {
    fn merge_from(&mut self, other: &Self) {
        match (self, other) {
            (serde_json::Value::Object(this), serde_json::Value::Object(other)) => {
                for (key, value) in other {
                    if let Some(existing) = this.get_mut(key) {
                        existing.merge_from(value);
                    } else {
                        this.insert(key.clone(), value.clone());
                    }
                }
            }
            (this, other) => *this = other.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn option_merge_some() {
        let mut base: Option<u32> = Some(10);
        base.merge_from(&Some(20));
        assert_eq!(base, Some(20)); // u32 是 overwrite
    }

    #[test]
    fn option_merge_none_ignored() {
        let mut base: Option<u32> = Some(10);
        base.merge_from(&None);
        assert_eq!(base, Some(10));
    }

    #[test]
    fn btree_map_deep_merge() {
        let mut base: std::collections::BTreeMap<String, Option<u32>> =
            [("a".into(), Some(1)), ("b".into(), None)].into();
        let other: std::collections::BTreeMap<String, Option<u32>> =
            [("b".into(), Some(2)), ("c".into(), Some(3))].into();
        base.merge_from(&other);
        assert_eq!(base.get("a"), Some(&Some(1)));
        assert_eq!(base.get("b"), Some(&Some(2)));
        assert_eq!(base.get("c"), Some(&Some(3)));
    }

    #[test]
    fn serde_json_value_object_deep_merge() {
        let mut base = json!({"a": {"x": 1, "y": 2}, "b": 3});
        let other = json!({"a": {"y": 20, "z": 30}, "c": 4});
        base.merge_from(&other);
        assert_eq!(
            base,
            json!({"a": {"x": 1, "y": 20, "z": 30}, "b": 3, "c": 4})
        );
    }
}

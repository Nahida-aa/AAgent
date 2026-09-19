// re-export settings_content 的 merge_from 供 MergeFrom derive 使用。
// settings-macros #[derive(MergeFrom)] 生成 `crate::merge_from::MergeFrom`。
mod merge_from {
    pub use settings_content::merge_from::*;
}

pub mod shell;

use std::sync::{Arc, LazyLock};

use crate::Language;
use language_core::{BracketPair, BracketPairConfig, LanguageConfig, LanguageMatcher, SoftWrap};

pub static PLAIN_TEXT: LazyLock<Arc<Language>> = LazyLock::new(|| {
    Arc::new(Language::new(
        LanguageConfig {
            name: "Plain Text".into(),
            soft_wrap: Some(SoftWrap::EditorWidth),
            autoclose_before: ")]}".into(),
            matcher: (LanguageMatcher {
                path_suffixes: vec!["txt".to_owned()],
                first_line_pattern: None,
                modeline_aliases: vec!["text".to_owned(), "txt".to_owned()],
            })
            .into(),
            brackets: BracketPairConfig {
                pairs: vec![
                    BracketPair {
                        start: "(".to_string(),
                        end: ")".to_string(),
                        close: true,
                        surround: true,
                        newline: false,
                    },
                    BracketPair {
                        start: "[".to_string(),
                        end: "]".to_string(),
                        close: true,
                        surround: true,
                        newline: false,
                    },
                    BracketPair {
                        start: "{".to_string(),
                        end: "}".to_string(),
                        close: true,
                        surround: true,
                        newline: false,
                    },
                    BracketPair {
                        start: "\"".to_string(),
                        end: "\"".to_string(),
                        close: true,
                        surround: true,
                        newline: false,
                    },
                    BracketPair {
                        start: "'".to_string(),
                        end: "'".to_string(),
                        close: true,
                        surround: true,
                        newline: false,
                    },
                ],
                disabled_scopes_by_bracket_ix: Default::default(),
            },
            ..Default::default()
        },
        None,
    ))
});

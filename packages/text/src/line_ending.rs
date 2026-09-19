//! 行尾处理 — 对齐 Zed `crates/text/src/text.rs::LineEnding`。

use std::borrow::Cow;

/// 文本行尾类型。
///
/// 对齐 Zed `LineEnding`，精简版：去掉 regex 依赖，用简单 replace。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum LineEnding {
    /// Unix 行尾 `\n`。默认。
    #[default]
    Unix,
    /// Windows 行尾 `\r\n`。
    Windows,
}

impl LineEnding {
    /// 返回行尾字符串字面量。
    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Unix => "\n",
            LineEnding::Windows => "\r\n",
        }
    }

    /// 短标签，用于 settings UI 等显示。
    pub fn label(&self) -> &'static str {
        match self {
            LineEnding::Unix => "LF",
            LineEnding::Windows => "CRLF",
        }
    }

    /// 检测文本的行尾类型。
    ///
    /// 扫描前 1000 字节里的第一个换行：
    /// - 前一个字符是 `\r` → Windows
    /// - 否则 → Unix
    /// - 无换行 → 返回 `LineEnding::default()`
    pub fn detect(text: &str) -> Self {
        let mut max_ix = text.len().min(1000);
        while !text.is_char_boundary(max_ix) {
            max_ix -= 1;
        }
        if let Some(ix) = text[..max_ix].find('\n') {
            if ix > 0 && text.as_bytes()[ix - 1] == b'\r' {
                Self::Windows
            } else {
                Self::Unix
            }
        } else {
            Self::default()
        }
    }

    /// 规范化 — 把所有 `\r\n` 和孤立的 `\r` 都转成 `\n`。
    ///
    /// Zed 用 regex `r"\r\n|\r"`；我们直接 `replace` 两次。
    pub fn normalize(text: &mut String) {
        // 先 CRLF → LF，再孤立 CR → LF（顺序重要）
        let replaced = text.replace("\r\n", "\n").replace('\r', "\n");
        if replaced != *text {
            *text = replaced;
        }
    }

    /// Cow 版本 — 不修改原字符串，返回规范化后的引用或 owned。
    pub fn normalize_cow(text: Cow<str>) -> Cow<str> {
        match text {
            Cow::Borrowed(s) => {
                let replaced = s.replace("\r\n", "\n").replace('\r', "\n");
                if replaced == s {
                    Cow::Borrowed(s)
                } else {
                    Cow::Owned(replaced)
                }
            }
            Cow::Owned(mut s) => {
                Self::normalize(&mut s);
                Cow::Owned(s)
            }
        }
    }

    /// 把文本转成使用当前行尾。
    ///
    /// 先 detect 现有行尾，相同则直接返回；不同则转换。
    /// 混合行尾不支持（只看第一个换行）。
    pub fn apply(&self, text: String) -> String {
        match (LineEnding::detect(&text), self) {
            (LineEnding::Unix, LineEnding::Unix) | (LineEnding::Windows, LineEnding::Windows) => {
                text
            }
            (LineEnding::Unix, LineEnding::Windows) => text.replace('\n', "\r\n"),
            (LineEnding::Windows, LineEnding::Unix) => {
                let mut result = text;
                Self::normalize(&mut result);
                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_str() {
        assert_eq!(LineEnding::Unix.as_str(), "\n");
        assert_eq!(LineEnding::Windows.as_str(), "\r\n");
    }

    #[test]
    fn test_label() {
        assert_eq!(LineEnding::Unix.label(), "LF");
        assert_eq!(LineEnding::Windows.label(), "CRLF");
    }

    #[test]
    fn test_detect_unix() {
        assert_eq!(LineEnding::detect("hello\nworld\n"), LineEnding::Unix);
        assert_eq!(LineEnding::detect("🍐✅\n"), LineEnding::Unix);
        assert_eq!(LineEnding::detect("abcd\n"), LineEnding::Unix);
    }

    #[test]
    fn test_detect_windows() {
        assert_eq!(
            LineEnding::detect("hello\r\nworld\r\n"),
            LineEnding::Windows
        );
        assert_eq!(LineEnding::detect("🍐✅\r\n"), LineEnding::Windows);
        assert_eq!(LineEnding::detect("abcd\r\n"), LineEnding::Windows);
    }

    #[test]
    fn test_detect_empty() {
        assert_eq!(LineEnding::detect(""), LineEnding::Unix);
        assert_eq!(LineEnding::detect("no newlines here"), LineEnding::Unix);
    }

    #[test]
    fn test_normalize_crlf() {
        let mut s = String::from("a\r\nb\r\nc");
        LineEnding::normalize(&mut s);
        assert_eq!(s, "a\nb\nc");
    }

    #[test]
    fn test_normalize_mixed() {
        let mut s = String::from("a\rb\nc\r\nd");
        LineEnding::normalize(&mut s);
        assert_eq!(s, "a\nb\nc\nd");
    }

    #[test]
    fn test_normalize_already_unix() {
        let mut s = String::from("a\nb\nc");
        LineEnding::normalize(&mut s);
        assert_eq!(s, "a\nb\nc");
    }

    #[test]
    fn test_apply_unix_to_windows() {
        assert_eq!(
            LineEnding::Windows.apply("a\nb\nc".to_string()),
            "a\r\nb\r\nc"
        );
    }

    #[test]
    fn test_apply_windows_to_unix() {
        assert_eq!(LineEnding::Unix.apply("a\r\nb\r\nc".to_string()), "a\nb\nc");
    }

    #[test]
    fn test_apply_noop() {
        assert_eq!(LineEnding::Unix.apply("a\nb".to_string()), "a\nb");
        assert_eq!(LineEnding::Windows.apply("a\r\nb".to_string()), "a\r\nb");
    }

    #[test]
    fn test_normalize_cow_borrowed() {
        let s: &str = "a\r\nb";
        let result = LineEnding::normalize_cow(Cow::Borrowed(s));
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "a\nb");
    }

    #[test]
    fn test_normalize_cow_owned() {
        let result = LineEnding::normalize_cow(Cow::Owned("a\r\nb".to_string()));
        assert_eq!(result, "a\nb");
    }

    #[test]
    fn test_normalize_cow_noop() {
        let s: &str = "a\nb";
        let result = LineEnding::normalize_cow(Cow::Borrowed(s));
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "a\nb");
    }

    #[test]
    fn test_detect_large_text() {
        assert_eq!(LineEnding::detect(&"🍐✅\n".repeat(1000)), LineEnding::Unix);
        assert_eq!(LineEnding::detect(&"abcd\n".repeat(1000)), LineEnding::Unix);
        assert_eq!(
            LineEnding::detect(&"🍐✅\r\n".repeat(1000)),
            LineEnding::Windows
        );
        assert_eq!(
            LineEnding::detect(&"abcd\r\n".repeat(1000)),
            LineEnding::Windows
        );
    }
}

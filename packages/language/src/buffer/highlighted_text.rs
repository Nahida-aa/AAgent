//! HighlightedText — 带样式高亮的文本片段。
//!
//! 对齐 Zed `crates/language/src/buffer.rs:633` 的 `HighlightedText` / `HighlightedTextBuilder`。
//! 简化版：保留核心 Builder API（push_styled / push_plain / build）和 first_line_preview，
//! 去掉依赖 syntax highlighting 的 `from_buffer_range` / `add_text_from_buffer_range`。

use super::*;

use std::fmt::Write;
use std::ops::Range;

use gpui::{HighlightStyle, SharedString, StyledText, TextStyle};

/// 带样式高亮的文本。
///
/// `text` 是完整文本，`highlights` 是若干 `(range, style)` 对。
/// 与 Zed 版唯一差别：不依赖 `text::BufferSnapshot` / `SyntaxSnapshot`。
#[derive(Default, Clone, Debug)]
pub struct HighlightedText {
    pub text: SharedString,
    pub highlights: Vec<(Range<usize>, HighlightStyle)>,
}

/// Builder 模式构造 `HighlightedText`。
///
/// 对齐 Zed `HighlightedTextBuilder` (buffer.rs:640)。
#[derive(Default, Debug)]
pub struct HighlightedTextBuilder {
    text: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
}

impl HighlightedText {
    pub fn from_buffer_range<T: ToOffset>(
        range: Range<T>,
        snapshot: &text::BufferSnapshot,
        syntax_snapshot: &SyntaxSnapshot,
        override_style: Option<HighlightStyle>,
        syntax_theme: &SyntaxTheme,
    ) -> Self {
        let mut highlighted_text = HighlightedTextBuilder::default();
        highlighted_text.add_text_from_buffer_range(
            range,
            snapshot,
            syntax_snapshot,
            override_style,
            syntax_theme,
        );
        highlighted_text.build()
    }
    /// 转成 GPUI `StyledText`，用于渲染。
    ///
    /// 对齐 Zed buffer.rs:664。
    pub fn to_styled_text(&self, default_style: &TextStyle) -> StyledText {
        gpui::StyledText::new(self.text.clone())
            .with_default_highlights(default_style, self.highlights.iter().cloned())
    }

    /// Returns the first line without leading whitespace unless highlighted,
    /// and a boolean indicating if there are more lines after.
    ///
    /// 对齐 Zed buffer.rs:671。
    pub fn first_line_preview(self) -> (Self, bool) {
        let newline_ix = self.text.find('\n').unwrap_or(self.text.len());
        let first_line = &self.text[..newline_ix];

        // Trim leading whitespace, unless an edit starts prior to it.
        let mut preview_start_ix = first_line.len() - first_line.trim_start().len();
        if let Some((first_highlight_range, _)) = self.highlights.first() {
            preview_start_ix = preview_start_ix.min(first_highlight_range.start);
        }

        let preview_text = &first_line[preview_start_ix..];
        let preview_highlights = self
            .highlights
            .into_iter()
            .skip_while(|(range, _)| range.end <= preview_start_ix)
            .take_while(|(range, _)| range.start < newline_ix)
            .filter_map(|(mut range, highlight)| {
                range.start = range.start.saturating_sub(preview_start_ix);
                range.end = range.end.min(newline_ix).saturating_sub(preview_start_ix);
                if range.is_empty() {
                    None
                } else {
                    Some((range, highlight))
                }
            });

        let preview = Self {
            text: SharedString::new(preview_text),
            highlights: preview_highlights.collect(),
        };

        (preview, self.text.len() > newline_ix)
    }
}

impl HighlightedTextBuilder {
    /// Finish building.
    ///
    /// 对齐 Zed buffer.rs:707。
    pub fn build(self) -> HighlightedText {
        HighlightedText {
            text: self.text.into(),
            highlights: self.highlights,
        }
    }

    /// Append a displayable value to the text, highlighting its range with `style`.
    ///
    /// 对齐 Zed buffer.rs:716。
    pub fn push_styled(&mut self, value: impl std::fmt::Display, style: HighlightStyle) {
        let start = self.text.len();
        let _ = write!(&mut self.text, "{value}");
        let end = self.text.len();
        if end > start {
            self.highlights.push((start..end, style));
        }
    }

    /// Append a displayable value to the text without any highlighting.
    ///
    /// 对齐 Zed buffer.rs:726。
    pub fn push_plain(&mut self, value: impl std::fmt::Display) {
        let _ = write!(&mut self.text, "{value}");
    }

    pub fn add_text_from_buffer_range<T: ToOffset>(
        &mut self,
        range: Range<T>,
        snapshot: &text::BufferSnapshot,
        syntax_snapshot: &SyntaxSnapshot,
        override_style: Option<HighlightStyle>,
        syntax_theme: &SyntaxTheme,
    ) {
        let range = range.to_offset(snapshot);
        for chunk in Self::highlighted_chunks(range, snapshot, syntax_snapshot) {
            let start = self.text.len();
            self.text.push_str(chunk.text);
            let end = self.text.len();

            if let Some(highlight_style) = chunk
                .syntax_highlight_id
                .and_then(|id| syntax_theme.get(id).cloned())
            {
                let highlight_style = override_style.map_or(highlight_style, |override_style| {
                    highlight_style.highlight(override_style)
                });
                self.highlights.push((start..end, highlight_style));
            } else if let Some(override_style) = override_style {
                self.highlights.push((start..end, override_style));
            }
        }
    }

    fn highlighted_chunks<'a>(
        range: Range<usize>,
        snapshot: &'a text::BufferSnapshot,
        syntax_snapshot: &'a SyntaxSnapshot,
    ) -> BufferChunks<'a> {
        let captures = syntax_snapshot.captures(range.clone(), snapshot, |grammar| {
            grammar
                .highlights_config
                .as_ref()
                .map(|config| &config.query)
        });

        let highlight_maps = captures
            .grammars()
            .iter()
            .map(|grammar| grammar.highlight_map())
            .collect();

        BufferChunks::new(
            snapshot.as_rope(),
            range,
            Some((captures, highlight_maps)),
            false,
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_plain_and_styled() {
        let mut b = HighlightedTextBuilder::default();
        b.push_plain("Hello ");
        b.push_styled("world", HighlightStyle::default());
        b.push_plain("!");

        let ht = b.build();
        assert_eq!(ht.text.to_string(), "Hello world!");
        assert_eq!(ht.highlights.len(), 1);
        assert_eq!(ht.highlights[0].0, 6..11); // "world"
    }

    #[test]
    fn empty_push_styled_is_ignored() {
        let mut b = HighlightedTextBuilder::default();
        b.push_styled("", HighlightStyle::default());
        let ht = b.build();
        assert!(ht.highlights.is_empty());
        assert!(ht.text.is_empty());
    }

    #[test]
    fn first_line_preview_basic() {
        let mut b = HighlightedTextBuilder::default();
        b.push_plain("  foo\nbar");
        let ht = b.build();

        let (preview, has_more) = ht.first_line_preview();
        assert_eq!(preview.text.to_string(), "foo");
        assert!(has_more);
    }

    #[test]
    fn first_line_preview_preserves_leading_highlight() {
        // 如果 leading whitespace 里有 highlight，不能 trim 掉
        let mut b = HighlightedTextBuilder::default();
        b.push_styled("  ", HighlightStyle::default()); // leading whitespace BUT highlighted
        b.push_plain("foo");
        b.push_plain("\nbar");
        let ht = b.build();

        let (preview, _) = ht.first_line_preview();
        // highlight 在 0..2，preview_start 不能跳到 2
        assert_eq!(preview.text.to_string(), "  foo");
    }
}

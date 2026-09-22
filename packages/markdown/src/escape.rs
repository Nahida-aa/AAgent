use std::borrow::Cow;

enum EscapeAction {
    PassThrough,
    Nbsp(usize),
    DoubleNewline,
    PrefixBackslash,
}

impl EscapeAction {
    fn output_len(&self, c: char) -> usize {
        match self {
            Self::PassThrough => c.len_utf8(),
            Self::Nbsp(count) => count * '\u{00A0}'.len_utf8(),
            Self::DoubleNewline => 2,
            Self::PrefixBackslash => '\\'.len_utf8() + c.len_utf8(),
        }
    }

    fn write_to(&self, c: char, output: &mut String) {
        match self {
            Self::PassThrough => output.push(c),
            Self::Nbsp(count) => {
                for _ in 0..*count {
                    output.push('\u{00A0}');
                }
            }
            Self::DoubleNewline => {
                output.push('\n');
                output.push('\n');
            }
            Self::PrefixBackslash => {
                // '\\' is a single backslash in Rust, e.g. '|' -> '\|'
                output.push('\\');
                output.push(c);
            }
        }
    }
}

pub struct MarkdownEscaper {
    in_leading_whitespace: bool,
}

impl MarkdownEscaper {
    const TAB_SIZE: usize = 4;

    fn new() -> Self {
        Self {
            in_leading_whitespace: true,
        }
    }

    fn next(&mut self, c: char) -> EscapeAction {
        let action = if self.in_leading_whitespace && c == '\t' {
            EscapeAction::Nbsp(Self::TAB_SIZE)
        } else if self.in_leading_whitespace && c == ' ' {
            EscapeAction::Nbsp(1)
        } else if c == '\n' {
            EscapeAction::DoubleNewline
        } else if c.is_ascii_punctuation() {
            EscapeAction::PrefixBackslash
        } else {
            EscapeAction::PassThrough
        };

        self.in_leading_whitespace =
            c == '\n' || (self.in_leading_whitespace && (c == ' ' || c == '\t'));
        action
    }
}

/// `Markdown::escape` 的实现
pub(crate) fn escape(s: &str) -> Cow<'_, str> {
    let output_len: usize = {
        let mut escaper = MarkdownEscaper::new();
        s.chars().map(|c| escaper.next(c).output_len(c)).sum()
    };

    if output_len == s.len() {
        return s.into();
    }

    let mut escaper = MarkdownEscaper::new();
    let mut output = String::with_capacity(output_len);
    for c in s.chars() {
        escaper.next(c).write_to(c, &mut output);
    }
    output.into()
}

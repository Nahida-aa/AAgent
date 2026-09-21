use super::*;

pub(crate) static LINE_SEPARATORS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\r\n|\r").expect("Failed to create LINE_SEPARATORS_REGEX"));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineIndent {
    pub tabs: u32,
    pub spaces: u32,
    pub line_blank: bool,
}

impl LineIndent {
    pub fn from_chunks(chunks: &mut Chunks) -> Self {
        let mut tabs = 0;
        let mut spaces = 0;
        let mut line_blank = true;

        'outer: while let Some(chunk) = chunks.peek() {
            for ch in chunk.chars() {
                if ch == '\t' {
                    tabs += 1;
                } else if ch == ' ' {
                    spaces += 1;
                } else {
                    if ch != '\n' {
                        line_blank = false;
                    }
                    break 'outer;
                }
            }

            chunks.next();
        }

        Self {
            tabs,
            spaces,
            line_blank,
        }
    }

    /// Constructs a new `LineIndent` which only contains spaces.
    pub fn spaces(spaces: u32) -> Self {
        Self {
            tabs: 0,
            spaces,
            line_blank: true,
        }
    }

    /// Constructs a new `LineIndent` which only contains tabs.
    pub fn tabs(tabs: u32) -> Self {
        Self {
            tabs,
            spaces: 0,
            line_blank: true,
        }
    }

    /// Indicates whether the line is empty.
    pub fn is_line_empty(&self) -> bool { self.tabs == 0 && self.spaces == 0 && self.line_blank }

    /// Indicates whether the line is blank (contains only whitespace).
    pub fn is_line_blank(&self) -> bool { self.line_blank }

    /// Returns the number of indentation characters (tabs or spaces).
    pub fn raw_len(&self) -> u32 { self.tabs + self.spaces }

    /// Returns the number of indentation characters (tabs or spaces), taking tab size into account.
    pub fn len(&self, tab_size: u32) -> u32 { self.tabs * tab_size + self.spaces }
}

impl From<&str> for LineIndent {
    fn from(value: &str) -> Self { Self::from_iter(value.chars()) }
}

impl FromIterator<char> for LineIndent {
    fn from_iter<T: IntoIterator<Item = char>>(chars: T) -> Self {
        let mut tabs = 0;
        let mut spaces = 0;
        let mut line_blank = true;
        for c in chars {
            if c == '\t' {
                tabs += 1;
            } else if c == ' ' {
                spaces += 1;
            } else {
                if c != '\n' {
                    line_blank = false;
                }
                break;
            }
        }
        Self {
            tabs,
            spaces,
            line_blank,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineEnding {
    Unix,
    Windows,
}

impl Default for LineEnding {
    fn default() -> Self {
        #[cfg(unix)]
        return Self::Unix;

        #[cfg(not(unix))]
        return Self::Windows;
    }
}
impl LineEnding {
    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Unix => "\n",
            LineEnding::Windows => "\r\n",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            LineEnding::Unix => "LF",
            LineEnding::Windows => "CRLF",
        }
    }

    pub fn detect(text: &str) -> Self {
        let mut max_ix = cmp::min(text.len(), 1000);
        while !text.is_char_boundary(max_ix) {
            max_ix -= 1;
        }

        if let Some(ix) = text[..max_ix].find(['\n']) {
            if ix > 0 && text.as_bytes()[ix - 1] == b'\r' {
                Self::Windows
            } else {
                Self::Unix
            }
        } else {
            Self::default()
        }
    }

    pub fn normalize(text: &mut String) {
        if let Cow::Owned(replaced) = LINE_SEPARATORS_REGEX.replace_all(text, "\n") {
            *text = replaced;
        }
    }

    pub fn normalize_arc(text: Arc<str>) -> Arc<str> {
        if let Cow::Owned(replaced) = LINE_SEPARATORS_REGEX.replace_all(&text, "\n") {
            replaced.into()
        } else {
            text
        }
    }

    pub fn normalize_cow(text: Cow<str>) -> Cow<str> {
        if let Cow::Owned(replaced) = LINE_SEPARATORS_REGEX.replace_all(&text, "\n") {
            replaced.into()
        } else {
            text
        }
    }

    /// Converts `text` to use this line ending.
    ///
    /// Detects the existing line ending of `text` first; if it already matches
    /// `self`, the string is returned unchanged. Mixed line endings are not
    /// supported: detection is based on the first newline found.
    pub fn apply(&self, text: String) -> String {
        match (LineEnding::detect(&text), self) {
            (LineEnding::Unix, LineEnding::Unix) | (LineEnding::Windows, LineEnding::Windows) => {
                text
            }
            (LineEnding::Unix, LineEnding::Windows) => text.replace('\n', "\r\n"),
            (LineEnding::Windows, LineEnding::Unix) => {
                let mut result = text;
                LineEnding::normalize(&mut result);
                result
            }
        }
    }
}

pub fn chunks_with_line_ending(rope: &Rope, line_ending: LineEnding) -> impl Iterator<Item = &str> {
    rope.chunks().flat_map(move |chunk| {
        let mut newline = false;
        let end_with_newline = chunk.ends_with('\n').then_some(line_ending.as_str());
        chunk
            .lines()
            .flat_map(move |line| {
                let ending = if newline {
                    Some(line_ending.as_str())
                } else {
                    None
                };
                newline = true;
                ending.into_iter().chain([line])
            })
            .chain(end_with_newline)
    })
}

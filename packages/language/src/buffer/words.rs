use super::*;

pub struct WordsQuery<'a> {
    /// Only returns words with all chars from the fuzzy string in them.
    pub fuzzy_contents: Option<&'a str>,
    /// Skips words that start with a digit.
    pub skip_digits: bool,
    /// Buffer offset range, to look for words.
    pub range: Range<usize>,
}

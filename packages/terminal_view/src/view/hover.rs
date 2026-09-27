use terminal::HoveredWord;

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, Eq, PartialEq))]
pub struct HoverTarget {
    pub tooltip: String,
    pub hovered_word: HoveredWord,
}

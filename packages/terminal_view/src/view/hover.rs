use terminal::HoveredWord;

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, Eq, PartialEq))]
pub(super) struct HoverTarget {
    pub(super) tooltip: String,
    pub(super) hovered_word: HoveredWord,
}

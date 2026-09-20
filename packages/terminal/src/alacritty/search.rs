use alacritty_terminal::{
    grid::Dimensions as _,
    index::{Column, Direction as AlacDirection, Point as AlacPoint},
    term::{
        Term,
        search::{Match, RegexIter, RegexSearch},
    },
};

use crate::{Range, Search};

use super::conversions::*;
use super::types::{AlacrittySearch, ZedListener};

pub(crate) fn search_matches(term: &Term<ZedListener>, searcher: Search) -> Vec<Range> {
    let mut searcher = searcher.into_alacritty();
    all_search_matches(term, &mut searcher)
        .map(Range::from_alacritty)
        .collect()
}

fn all_search_matches<'a, T>(
    term: &'a Term<T>,
    regex: &'a mut RegexSearch,
) -> impl Iterator<Item = Match> + 'a {
    let start = AlacPoint::new(term.grid().topmost_line(), Column(0));
    let end = AlacPoint::new(term.grid().bottommost_line(), term.grid().last_column());
    RegexIter::new(start, end, AlacDirection::Right, term, regex)
}

use alacritty_terminal::{grid::Dimensions as _, index::Boundary};
use util::paths::PathStyle;

use crate::Point;

use super::conversions::*;
use super::{AlacrittyTerm, HyperlinkMatch, RegexSearches};

pub(crate) fn find_from_terminal_point(
    term: &AlacrittyTerm,
    point: Point,
    regex_searches: &mut RegexSearches,
    path_style: PathStyle,
) -> Option<HyperlinkMatch> {
    let point = point.to_alacritty().grid_clamp(term, Boundary::Grid);
    super::hyperlinks::find_from_grid_point(term, point, regex_searches, path_style)
}

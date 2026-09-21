use aa_gpui_kit_theme::SyntaxTheme;
use language_core::highlight_map::{HighlightId, HighlightMap};

#[inline]
pub fn build_highlight_map(capture_names: &[&str], theme: &SyntaxTheme) -> HighlightMap {
    HighlightMap::from_ids(
        capture_names
            .iter()
            .map(|capture_name| theme.highlight_id(capture_name).map(HighlightId::new)),
    )
}

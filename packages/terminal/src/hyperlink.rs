use std::sync::Arc;

use crate::alacritty::AlacrittyHyperlink;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Hyperlink {
    pub data: HyperlinkData,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HyperlinkData {
    Alacritty(AlacrittyHyperlink),
    Owned { id: Option<Arc<str>>, uri: Arc<str> },
}

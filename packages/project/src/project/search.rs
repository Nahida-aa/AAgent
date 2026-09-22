use super::*;

use gpui::{Context, Entity};
use itertools::Itertools;

use super::Project;
use crate::project_search::{SearchResultsHandle};
use crate::search::{SearchQuery, SearchResult};
use crate::SearchResults;

impl Project {
    fn search_impl(&mut self, query: SearchQuery, cx: &mut Context<Self>) -> SearchResultsHandle { /* 原样 */ }
    pub fn search(&mut self, query: SearchQuery, cx: &mut Context<Self>) -> SearchResults<SearchResult> { /* 原样 */ }
}

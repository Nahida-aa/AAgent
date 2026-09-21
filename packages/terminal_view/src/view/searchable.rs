use std::sync::Arc;

use gpui::{App, Context, Entity, Task, Window};
use project::search::SearchQuery;
use settings::SeedQuerySetting;
use terminal::Range;
use workspace::searchable::{
    Direction, SearchOptions, SearchToken, SearchableItem, SearchableItemHandle,
};

use super::TerminalView;
use super::helpers::regex_search_for_query;

impl SearchableItem for TerminalView {
    type Match = Range;

    fn supported_options(&self) -> SearchOptions {
        SearchOptions {
            case: false,
            word: false,
            regex: true,
            replacement: false,
            selection: false,
            select_all: false,
            find_in_results: false,
        }
    }

    fn clear_matches(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.terminal().update(cx, |term, _| term.matches.clear())
    }

    fn update_matches(
        &mut self,
        matches: &[Self::Match],
        _active_match_index: Option<usize>,
        _token: SearchToken,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal()
            .update(cx, |term, _| term.matches = matches.to_vec())
    }

    fn query_suggestion(
        &mut self,
        _seed_query_override: Option<SeedQuerySetting>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> String {
        self.terminal()
            .read(cx)
            .last_content
            .selection_text
            .clone()
            .unwrap_or_default()
    }

    fn activate_match(
        &mut self,
        index: usize,
        _: &[Self::Match],
        _token: SearchToken,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal()
            .update(cx, |term, _| term.activate_match(index));
        cx.notify();
    }

    fn select_matches(
        &mut self,
        matches: &[Self::Match],
        _token: SearchToken,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal()
            .update(cx, |term, _| term.select_matches(matches));
        cx.notify();
    }

    fn find_matches(
        &mut self,
        query: Arc<SearchQuery>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Vec<Self::Match>> {
        if let Some(s) = regex_search_for_query(&query) {
            self.terminal()
                .update(cx, |term, cx| term.find_matches(s, cx))
        } else {
            Task::ready(vec![])
        }
    }

    fn active_match_index(
        &mut self,
        direction: Direction,
        matches: &[Self::Match],
        _token: SearchToken,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        if !matches.is_empty() {
            if let Some(selection_head) = self.terminal().read(cx).selection_head {
                match direction {
                    Direction::Prev => Some(
                        matches
                            .iter()
                            .enumerate()
                            .rev()
                            .find(|(_, search_match)| {
                                search_match.contains(selection_head)
                                    || search_match.start() < selection_head
                            })
                            .map(|(ix, _)| ix)
                            .unwrap_or(0),
                    ),
                    Direction::Next => Some(
                        matches
                            .iter()
                            .enumerate()
                            .find(|(_, search_match)| {
                                search_match.contains(selection_head)
                                    || search_match.start() > selection_head
                            })
                            .map(|(ix, _)| ix)
                            .unwrap_or(matches.len().saturating_sub(1)),
                    ),
                }
            } else {
                Some(matches.len().saturating_sub(1))
            }
        } else {
            None
        }
    }

    fn replace(
        &mut self,
        _: &Self::Match,
        _: &SearchQuery,
        _token: SearchToken,
        _window: &mut Window,
        _: &mut Context<Self>,
    ) {
    }
}

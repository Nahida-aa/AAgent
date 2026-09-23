use super::*;

use super::Project;
use crate::MAX_PROJECT_SEARCH_HISTORY_SIZE;
use crate::SearchResults;
use crate::project_search;
use crate::project_search::SearchResultsHandle;
use crate::search::SearchInputKind;
use crate::search::{SearchQuery, SearchResult};
use gpui::{Context, Entity};
use itertools::Itertools;
use std::{
    borrow::Cow,
    collections::BTreeMap,
    ffi::OsString,
    future::Future,
    ops::{Not as _, Range},
    path::{Path, PathBuf},
    pin::pin,
    str::{self, FromStr},
    sync::Arc,
    time::Duration,
};

impl Project {
    pub(crate) fn new_search_history() -> SearchHistory {
        SearchHistory::new(
            Some(MAX_PROJECT_SEARCH_HISTORY_SIZE),
            crate::search_history::QueryInsertionBehavior::AlwaysInsert,
        )
    }
    #[inline]
    pub fn search_history(&self, kind: SearchInputKind) -> &SearchHistory {
        match kind {
            SearchInputKind::Query => &self.search_history,
            SearchInputKind::Include => &self.search_included_history,
            SearchInputKind::Exclude => &self.search_excluded_history,
        }
    }
    #[inline]
    pub fn search_history_mut(&mut self, kind: SearchInputKind) -> &mut SearchHistory {
        match kind {
            SearchInputKind::Query => &mut self.search_history,
            SearchInputKind::Include => &mut self.search_included_history,
            SearchInputKind::Exclude => &mut self.search_excluded_history,
        }
    }

    pub(crate) fn search_impl(
        &mut self,
        query: SearchQuery,
        cx: &mut Context<Self>,
    ) -> SearchResultsHandle {
        let client: Option<(AnyProtoClient, _)> = if let Some(ssh_client) = &self.remote_client {
            Some((ssh_client.read(cx).proto_client(), 0))
        } else if let Some(remote_id) = self.remote_id() {
            self.is_local()
                .not()
                .then(|| (self.collab_client.clone().into(), remote_id))
        } else {
            None
        };
        let searcher = if query.is_opened_only() {
            project_search::Search::open_buffers_only(
                self.buffer_store.clone(),
                self.worktree_store.clone(),
                project_search::Search::MAX_SEARCH_RESULT_FILES + 1,
            )
        } else {
            match client {
                Some((client, remote_id)) => project_search::Search::remote(
                    self.buffer_store.clone(),
                    self.worktree_store.clone(),
                    project_search::Search::MAX_SEARCH_RESULT_FILES + 1,
                    (client, remote_id, self.remotely_created_models.clone()),
                ),
                None => project_search::Search::local(
                    self.fs.clone(),
                    self.buffer_store.clone(),
                    self.worktree_store.clone(),
                    project_search::Search::MAX_SEARCH_RESULT_FILES + 1,
                    cx,
                ),
            }
        };
        searcher.into_handle(query, cx)
    }
    pub fn search(
        &mut self,
        query: SearchQuery,
        cx: &mut Context<Self>,
    ) -> SearchResults<SearchResult> {
        self.search_impl(query, cx).results(cx)
    }
}

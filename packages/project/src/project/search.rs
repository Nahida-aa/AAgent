use super::*;

use gpui::{Context, Entity};
use itertools::Itertools;

use super::Project;
use crate::project_search;
use crate::project_search::SearchResultsHandle;
use crate::search::{SearchQuery, SearchResult};
use crate::SearchResults;

impl Project {
    fn search_impl(&mut self, query: SearchQuery, cx: &mut Context<Self>) -> SearchResultsHandle {
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

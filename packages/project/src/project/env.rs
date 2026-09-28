use std::ops::Range;

use crate::git_store::{Repository, RepositoryId};

use super::*;

use ::git::status::FileStatus;
use anyhow::{Context as _, Result, anyhow};
use buffer_diff::BufferDiff;
use text::Anchor;

impl Project {
    #[inline]
    pub fn peek_environment_error<'a>(&'a self, cx: &'a App) -> Option<&'a String> {
        self.environment.read(cx).peek_environment_error()
    }

    #[inline]
    pub fn pop_environment_error(&mut self, cx: &mut Context<Self>) {
        self.environment.update(cx, |environment, _| {
            environment.pop_environment_error();
        });
    }
}

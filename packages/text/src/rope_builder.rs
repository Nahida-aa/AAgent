use super::fragment::{Fragment, FragmentTextSummary};
use super::*;

pub(crate) struct RopeBuilder<'a> {/* 原字段 */}

impl<'a> RopeBuilder<'a> {
    pub(crate) fn new(/* ... */) -> Self { /* 原样 */
    }
    pub(crate) fn append(&mut self, len: FragmentTextSummary) { /* 原样 */
    }
    pub(crate) fn push_fragment(&mut self, fragment: &Fragment, was_visible: bool) { /* 原样 */
    }
    fn push(&mut self, len: usize, was_visible: bool, is_visible: bool) { /* 原样 */
    }
    pub(crate) fn push_str(&mut self, text: &str) { /* 原样 */
    }
    pub(crate) fn finish(self) -> (Rope, Rope) { /* 原样 */
    }
}

use super::fragment::*;
use super::*;
use std::{
    borrow::Cow,
    cmp::{self, Ordering, Reverse},
    fmt::Display,
    future::Future,
    iter::Iterator,
    num::NonZeroU64,
    ops::{self, Deref, Range, Sub},
    str,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FullOffset(pub usize);

impl ops::AddAssign<usize> for FullOffset {
    fn add_assign(&mut self, rhs: usize) { self.0 += rhs; }
}

impl ops::Add<usize> for FullOffset {
    type Output = Self;

    fn add(mut self, rhs: usize) -> Self::Output {
        self += rhs;
        self
    }
}

impl ops::Sub for FullOffset {
    type Output = usize;

    fn sub(self, rhs: Self) -> Self::Output { self.0 - rhs.0 }
}

impl sum_tree::Dimension<'_, FragmentSummary> for usize {
    fn zero(_: &Option<clock::Global>) -> Self { Default::default() }

    fn add_summary(&mut self, summary: &FragmentSummary, _: &Option<clock::Global>) {
        *self += summary.text.visible;
    }
}

impl sum_tree::Dimension<'_, FragmentSummary> for FullOffset {
    fn zero(_: &Option<clock::Global>) -> Self { Default::default() }

    fn add_summary(&mut self, summary: &FragmentSummary, _: &Option<clock::Global>) {
        self.0 += summary.text.visible + summary.text.deleted;
    }
}

impl<'a> sum_tree::Dimension<'a, FragmentSummary> for Option<&'a Locator> {
    fn zero(_: &Option<clock::Global>) -> Self { Default::default() }

    fn add_summary(&mut self, summary: &'a FragmentSummary, _: &Option<clock::Global>) {
        *self = Some(&summary.max_id);
    }
}

impl sum_tree::SeekTarget<'_, FragmentSummary, FragmentTextSummary> for usize {
    fn cmp(
        &self,
        cursor_location: &FragmentTextSummary,
        _: &Option<clock::Global>,
    ) -> cmp::Ordering {
        Ord::cmp(self, &cursor_location.visible)
    }
}

use super::*;

#[derive(Eq, PartialEq, Clone, Debug)]
pub(crate) struct Fragment {
    id: Locator,
    timestamp: clock::Lamport,
    insertion_offset: u32,
    len: u32,
    visible: bool,
    deletions: SmallVec<[clock::Lamport; 2]>,
    max_undos: clock::Global,
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub(crate) struct FragmentSummary {
    text: FragmentTextSummary,
    max_id: Locator,
    max_version: clock::Global,
    min_insertion_version: clock::Global,
    max_insertion_version: clock::Global,
}

#[derive(Copy, Default, Clone, Debug, PartialEq, Eq)]
pub(crate) struct FragmentTextSummary {
    visible: usize,
    deleted: usize,
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub(crate) struct InsertionFragment {
    timestamp: clock::Lamport,
    split_offset: u32,
    fragment_id: Locator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct InsertionFragmentKey {
    timestamp: clock::Lamport,
    split_offset: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InsertionSlice {
    // Inline the lamports to allow the replica ids to share the same alignment
    // saving 4 bytes space edit_id: clock::Lamport,
    edit_id_value: clock::Seq,
    edit_id_replica_id: ReplicaId,
    // insertion_id: clock::Lamport,
    insertion_id_value: clock::Seq,
    insertion_id_replica_id: ReplicaId,
    range: Range<u32>,
}

impl Ord for InsertionSlice {
    fn cmp(&self, other: &Self) -> Ordering {
        Lamport {
            value: self.edit_id_value,
            replica_id: self.edit_id_replica_id,
        }
        .cmp(&Lamport {
            value: other.edit_id_value,
            replica_id: other.edit_id_replica_id,
        })
        .then_with(|| {
            Lamport {
                value: self.insertion_id_value,
                replica_id: self.insertion_id_replica_id,
            }
            .cmp(&Lamport {
                value: other.insertion_id_value,
                replica_id: other.insertion_id_replica_id,
            })
        })
        .then_with(|| self.range.start.cmp(&other.range.start))
        .then_with(|| self.range.end.cmp(&other.range.end))
    }
}

impl PartialOrd for InsertionSlice {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl InsertionSlice {
    fn from_fragment(edit_id: clock::Lamport, fragment: &Fragment) -> Self {
        Self {
            edit_id_value: edit_id.value,
            edit_id_replica_id: edit_id.replica_id,
            insertion_id_value: fragment.timestamp.value,
            insertion_id_replica_id: fragment.timestamp.replica_id,
            range: fragment.insertion_offset..fragment.insertion_offset + fragment.len,
        }
    }
}

use super::fragment::*;
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    Edit(EditOperation),
    Undo(UndoOperation),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditOperation {
    pub timestamp: clock::Lamport,
    pub version: clock::Global,
    pub ranges: Vec<Range<FullOffset>>,
    pub new_text: Vec<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoOperation {
    pub timestamp: clock::Lamport,
    pub version: clock::Global,
    pub counts: HashMap<clock::Lamport, u32>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Edit<D> {
    pub old: Range<D>,
    pub new: Range<D>,
}

impl<D> Edit<D>
where
    D: PartialEq,
{
    /* is_empty 原样 */
}
impl<D, DDelta> Edit<D>
where
    D: Sub<D, Output = DDelta> + Copy,
{
    /* old_len / new_len */
}
impl<D1, D2> Edit<(D1, D2)> {
    /* flatten */
}

impl Operation {
    pub fn replica_id(&self) -> ReplicaId { /* 原样 */
    }
    pub fn timestamp(&self) -> clock::Lamport { /* 原样 */
    }
    pub fn as_edit(&self) -> Option<&EditOperation> { /* 原样 */
    }
    pub fn is_edit(&self) -> bool { /* 原样 */
    }
}

impl operation_queue::Operation for Operation {
    /* 原样 */
}

// Edits 迭代器
pub(crate) struct Edits<'a, D: TextDimension, F: FnMut(&FragmentSummary) -> bool> {/* 原字段 */}

impl<D: TextDimension + Ord, F: FnMut(&FragmentSummary) -> bool> Iterator for Edits<'_, D, F> {
    /* 原 impl 整段搬入 */
}

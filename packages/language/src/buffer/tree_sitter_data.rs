use super::*;

/// Cached tree-sitter-derived data for a buffer, shared between [`Buffer`]
/// and the [`BufferSnapshot`]s it produces.
///
/// Chunked into row ranges (`RowChunks`) so that syntax highlighting and
/// bracket matching can be cached and invalidated at chunk granularity.
#[derive(Debug)]
pub struct TreeSitterData {
    pub(crate) chunks: RowChunks,
    pub(crate) brackets_by_chunks: Mutex<HashMap<RowChunkId, Vec<BracketMatch>>>,
    pub(crate) highlights_by_chunks: ChunkHighlightCache,
}

pub(crate) const MAX_ROWS_IN_A_CHUNK: u32 = 50;
pub(crate) const MAX_BYTES_TO_HIGHLIGHT_IN_A_CHUNK: usize = 4 * MAX_BYTES_TO_QUERY;

impl TreeSitterData {
    pub(crate) fn clear(&mut self, snapshot: &text::BufferSnapshot) {
        self.chunks = RowChunks::new(snapshot, MAX_ROWS_IN_A_CHUNK);
        self.brackets_by_chunks.get_mut().clear();
        self.highlights_by_chunks.clear();
    }

    pub(crate) fn new(snapshot: &text::BufferSnapshot) -> Self {
        Self {
            chunks: RowChunks::new(snapshot, MAX_ROWS_IN_A_CHUNK),
            brackets_by_chunks: Mutex::new(HashMap::default()),
            highlights_by_chunks: ChunkHighlightCache::default(),
        }
    }

    pub(crate) fn version(&self) -> &clock::Global { self.chunks.version() }
}

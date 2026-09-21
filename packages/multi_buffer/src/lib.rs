/// One or more [`Buffers`](Buffer) being edited in a single view.
///
/// See <https://zed.dev/features#multi-buffers>
pub struct MultiBuffer {
    /// A snapshot of the [`Excerpt`]s in the MultiBuffer.
    /// Use [`MultiBuffer::snapshot`] to get a up-to-date snapshot.
    snapshot: RefCell<MultiBufferSnapshot>,
    /// Contains the state of the buffers being edited
    buffers: BTreeMap<BufferId, BufferState>,
    /// Mapping from buffer IDs to their diff states
    diffs: HashMap<BufferId, DiffState>,
    subscriptions: Topic<MultiBufferOffset>,
    /// If true, the multi-buffer only contains a single [`Buffer`] and a single [`Excerpt`]
    singleton: bool,
    /// The history of the multi-buffer.
    history: History,
    /// The explicit title of the multi-buffer.
    /// If `None`, it will be derived from the underlying path or content.
    title: Option<String>,
    /// The writing capability of the multi-buffer.
    capability: Capability,
    buffer_changed_since_sync: Rc<Cell<bool>>,
}

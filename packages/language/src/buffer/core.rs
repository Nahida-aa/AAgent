use super::*;
use crate::{
    Language, ModelineSettings,
    buffer::{edit::AutoindentRequest, snapshot::SelectionSet, tree_sitter_data::TreeSitterData},
    diagnostic_set::DiagnosticSet,
    language_settings::LanguageSettings,
    syntax_map::SyntaxMap,
};
use ::util::debug_panic;
use clock::{Lamport, ReplicaId};
use collections::HashMap;
use encoding_rs::Encoding;
use fs::MTime;
use futures::channel::oneshot;
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, HighlightStyle, SharedString, StyledText,
    Task, TextStyle,
};
use lsp::LanguageServerId;
use parking_lot::Mutex;
use settings::AutoIndentMode;
use std::{
    any::Any,
    cell::Cell,
    cmp::{self, Ordering, Reverse},
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    future::Future,
    iter::{self, Iterator, Peekable},
    mem,
    num::NonZeroU32,
    ops::{Deref, Range},
    path::PathBuf,
    rc,
    sync::Arc,
    time::{Duration, Instant},
    vec,
};
use sum_tree::TreeMap;
use text::{
    Anchor, Bias, Buffer as TextBuffer, BufferId, BufferSnapshot as TextBufferSnapshot, Edit,
    LineEnding, LineIndent, OffsetRangeExt, OffsetUtf16, Patch, Point, PointUtf16, Rope, Selection,
    SelectionGoal, Subscription, TextDimension, TextSummary, ToOffset, ToOffsetUtf16, ToPoint,
    ToPointUtf16, Transaction, TransactionId, Unclipped, operation_queue::OperationQueue,
};

/// An in-memory representation of a source code file, including its text,
/// syntax trees, git status, and diagnostics.
pub struct Buffer {
    text: TextBuffer,
    branch_state: Option<BufferBranchState>,
    /// Filesystem state, `None` when there is no path.
    file: Option<Arc<dyn File>>,
    /// The mtime of the file when this buffer was last loaded from
    /// or saved to disk.
    saved_mtime: Option<MTime>,
    /// The version vector when this buffer was last loaded from
    /// or saved to disk.
    saved_version: clock::Global,
    preview_version: clock::Global,
    transaction_depth: usize,
    was_dirty_before_starting_transaction: Option<bool>,
    reload_task: Option<Task<Result<()>>>,
    language: Option<Arc<Language>>,
    content_language_detection_enabled: bool,
    autoindent_requests: Vec<Arc<AutoindentRequest>>,
    wait_for_autoindent_txs: Vec<oneshot::Sender<()>>,
    pending_autoindent: Option<Task<()>>,
    sync_parse_timeout: Option<Duration>,
    syntax_map: Mutex<SyntaxMap>,
    reparse: Option<Task<()>>,
    parse_status: (watch::Sender<ParseStatus>, watch::Receiver<ParseStatus>),
    non_text_state_update_count: usize,
    diagnostics: TreeMap<LanguageServerId, DiagnosticSet>,
    remote_selections: TreeMap<ReplicaId, SelectionSet>,
    diagnostics_timestamp: clock::Lamport,
    completion_triggers: BTreeSet<String>,
    completion_triggers_per_language_server: HashMap<LanguageServerId, BTreeSet<String>>,
    completion_triggers_timestamp: clock::Lamport,
    deferred_ops: OperationQueue<Operation>,
    capability: Capability,
    has_conflict: bool,
    /// Memoize calls to has_changes_since(saved_version).
    /// The contents of a cell are (self.version, has_changes) at the time of a last call.
    has_unsaved_edits: Cell<(clock::Global, bool)>,
    change_bits: Vec<rc::Weak<Cell<bool>>>,
    modeline: Option<Arc<ModelineSettings>>,
    _subscriptions: Vec<gpui::Subscription>,
    resolved_settings: Option<Arc<LanguageSettings>>,
    _settings_observer: Option<gpui::Subscription>,
    tree_sitter_data: Arc<TreeSitterData>,
    encoding: &'static Encoding,
    has_bom: bool,
    reload_with_encoding_txns: HashMap<TransactionId, (&'static Encoding, bool)>,
}
pub(crate) struct BufferBranchState {
    base_buffer: Entity<Buffer>,
    merged_operations: Vec<Lamport>,
}

impl Buffer {
    /// Create a new buffer with the given base text.
    pub fn local<T: Into<String>>(base_text: T, cx: &mut Context<Self>) -> Self {
        Self::build(
            TextBuffer::new(
                ReplicaId::LOCAL,
                cx.entity_id().as_non_zero_u64().into(),
                base_text.into(),
            ),
            None,
            Capability::ReadWrite,
            cx,
        )
    }

    /// Create a new buffer with the given base text that has proper line endings and other normalization applied.
    pub fn local_normalized(
        base_text_normalized: Rope,
        line_ending: LineEnding,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(
            TextBuffer::new_normalized(
                ReplicaId::LOCAL,
                cx.entity_id().as_non_zero_u64().into(),
                line_ending,
                base_text_normalized,
            ),
            None,
            Capability::ReadWrite,
            cx,
        )
    }

    /// Create a new buffer that is a replica of a remote buffer.
    pub fn remote(
        remote_id: BufferId,
        replica_id: ReplicaId,
        capability: Capability,
        base_text: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(
            TextBuffer::new(replica_id, remote_id, base_text.into()),
            None,
            capability,
            cx,
        )
    }

    /// Create a new buffer that is a replica of a remote buffer, populating its
    /// state from the given protobuf message.
    pub fn from_proto(
        replica_id: ReplicaId,
        capability: Capability,
        message: proto::BufferState,
        file: Option<Arc<dyn File>>,
        cx: &mut Context<Self>,
    ) -> Result<Self> {
        let buffer_id = BufferId::new(message.id).context("Could not deserialize buffer_id")?;
        let buffer = TextBuffer::new(replica_id, buffer_id, message.base_text);
        let mut this = Self::build(buffer, file, capability, cx);
        this.text.set_line_ending(proto::deserialize_line_ending(
            rpc::proto::LineEnding::try_from(message.line_ending)
                .ok()
                .context("missing line_ending")?,
        ));
        this.saved_version = proto::deserialize_version(&message.saved_version);
        this.saved_mtime = message.saved_mtime.map(|time| time.into());
        Ok(this)
    }

    /// Serialize the buffer's state to a protobuf message.
    pub fn to_proto(&self, cx: &App) -> proto::BufferState {
        proto::BufferState {
            id: self.remote_id().into(),
            file: self.file.as_ref().map(|f| f.to_proto(cx)),
            base_text: self.base_text().to_string(),
            line_ending: proto::serialize_line_ending(self.line_ending()) as i32,
            saved_version: proto::serialize_version(&self.saved_version),
            saved_mtime: self.saved_mtime.map(|time| time.into()),
        }
    }

    /// Serialize as protobufs all of the changes to the buffer since the given version.
    pub fn serialize_ops(
        &self,
        since: Option<clock::Global>,
        cx: &App,
    ) -> Task<Vec<proto::Operation>> {
        let mut operations = Vec::new();
        operations.extend(self.deferred_ops.iter().map(proto::serialize_operation));

        operations.extend(self.remote_selections.iter().map(|(_, set)| {
            proto::serialize_operation(&Operation::UpdateSelections {
                selections: set.selections.clone(),
                lamport_timestamp: set.lamport_timestamp,
                line_mode: set.line_mode,
                cursor_shape: set.cursor_shape,
            })
        }));

        for (server_id, diagnostics) in self.diagnostics.iter() {
            operations.push(proto::serialize_operation(&Operation::UpdateDiagnostics {
                lamport_timestamp: self.diagnostics_timestamp,
                server_id: *server_id,
                diagnostics: diagnostics.iter().cloned().collect(),
            }));
        }

        for (server_id, completions) in &self.completion_triggers_per_language_server {
            operations.push(proto::serialize_operation(
                &Operation::UpdateCompletionTriggers {
                    triggers: completions.iter().cloned().collect(),
                    lamport_timestamp: self.completion_triggers_timestamp,
                    server_id: *server_id,
                },
            ));
        }

        let text_operations = self.text.operations().clone();
        cx.background_spawn(async move {
            let since = since.unwrap_or_default();
            operations.extend(
                text_operations
                    .iter()
                    .filter(|(_, op)| !since.observed(op.timestamp()))
                    .map(|(_, op)| proto::serialize_operation(&Operation::Buffer(op.clone()))),
            );
            operations.sort_unstable_by_key(proto::lamport_timestamp_for_operation);
            operations
        })
    }

    /// Assign a language to the buffer, returning the buffer.
    pub fn with_language_async(mut self, language: Arc<Language>, cx: &mut Context<Self>) -> Self {
        self.set_language_async(Some(language), cx);
        self
    }

    /// Assign a language to the buffer, blocking for up to 1ms to reparse the buffer, returning the buffer.
    #[a_tracing::instrument(skip_all, fields(lang = language.config.name.0.as_str()))]
    pub fn with_language(mut self, language: Arc<Language>, cx: &mut Context<Self>) -> Self {
        self.set_language(Some(language), cx);
        self
    }

    /// Returns the [`Capability`] of this buffer.
    pub fn capability(&self) -> Capability { self.capability }

    /// Whether this buffer can only be read.
    pub fn read_only(&self) -> bool { !self.capability.editable() }

    /// Builds a [`Buffer`] with the given underlying [`TextBuffer`], diff base, [`File`] and [`Capability`].
    pub fn build(
        buffer: TextBuffer,
        file: Option<Arc<dyn File>>,
        capability: Capability,
        cx: &mut Context<Self>,
    ) -> Self {
        let saved_mtime = file.as_ref().and_then(|file| file.disk_state().mtime());
        let snapshot = buffer.snapshot();
        let syntax_map = Mutex::new(SyntaxMap::new(&snapshot));
        let tree_sitter_data = TreeSitterData::new(snapshot);
        let mut this = Self {
            saved_mtime,
            tree_sitter_data: Arc::new(tree_sitter_data),
            saved_version: buffer.version(),
            preview_version: buffer.version(),
            reload_task: None,
            transaction_depth: 0,
            was_dirty_before_starting_transaction: None,
            has_unsaved_edits: Cell::new((buffer.version(), false)),
            text: buffer,
            branch_state: None,
            file,
            capability,
            syntax_map,
            reparse: None,
            non_text_state_update_count: 0,
            sync_parse_timeout: if cfg!(any(test, feature = "test-support")) {
                Some(Duration::from_millis(10))
            } else {
                Some(Duration::from_millis(1))
            },
            parse_status: watch::channel(ParseStatus::Idle),
            autoindent_requests: Default::default(),
            wait_for_autoindent_txs: Default::default(),
            pending_autoindent: Default::default(),
            language: None,
            content_language_detection_enabled: false,
            remote_selections: Default::default(),
            diagnostics: Default::default(),
            diagnostics_timestamp: Lamport::MIN,
            completion_triggers: Default::default(),
            completion_triggers_per_language_server: Default::default(),
            completion_triggers_timestamp: Lamport::MIN,
            deferred_ops: OperationQueue::new(),
            has_conflict: false,
            change_bits: Default::default(),
            modeline: None,
            _subscriptions: Vec::new(),
            resolved_settings: None,
            _settings_observer: Some(cx.observe_global::<SettingsStore>(|this, cx| {
                this.refresh_resolved_settings(cx);
            })),
            encoding: encoding_rs::UTF_8,
            has_bom: false,
            reload_with_encoding_txns: HashMap::default(),
        };
        this.resolved_settings = this.compute_resolved_settings(cx);
        this
    }

    fn compute_resolved_settings(&self, cx: &App) -> Option<Arc<LanguageSettings>> {
        cx.try_global::<SettingsStore>()?;
        Some(LanguageSettings::resolve_uncached(self, None, cx))
    }

    fn refresh_resolved_settings(&mut self, cx: &mut Context<Self>) {
        let resolved = self.compute_resolved_settings(cx);
        if resolved.as_deref() != self.resolved_settings.as_deref() {
            self.resolved_settings = resolved;
            self.non_text_state_update_count += 1;
            self.was_changed();
            cx.emit(BufferEvent::SettingsChanged);
            cx.notify();
        }
    }

    pub(crate) fn resolved_settings(&self) -> Option<&Arc<LanguageSettings>> {
        self.resolved_settings.as_ref()
    }

    #[a_tracing::instrument(skip_all)]
    pub fn build_snapshot(
        text: Rope,
        language: Option<Arc<Language>>,
        language_registry: Option<Arc<LanguageRegistry>>,
        modeline: Option<Arc<ModelineSettings>>,
        cx: &mut App,
    ) -> impl Future<Output = BufferSnapshot> + use<> {
        let entity_id = cx.reserve_entity::<Self>().entity_id();
        let buffer_id = entity_id.as_non_zero_u64().into();
        async move {
            let text =
                TextBuffer::new_normalized(ReplicaId::LOCAL, buffer_id, Default::default(), text);
            let text = text.into_snapshot();
            let mut syntax = SyntaxMap::new(&text).snapshot();
            if let Some(language) = language.clone() {
                let language_registry = language_registry.clone();
                syntax.reparse(&text, language_registry, language);
            }
            let tree_sitter_data = TreeSitterData::new(&text);
            BufferSnapshot {
                text,
                syntax,
                file: None,
                diagnostics: Default::default(),
                remote_selections: Default::default(),
                tree_sitter_data: Arc::new(tree_sitter_data),
                language,
                non_text_state_update_count: 0,
                capability: Capability::ReadOnly,
                modeline,
                resolved_settings: None,
            }
        }
    }

    pub fn build_empty_snapshot(cx: &mut App) -> BufferSnapshot {
        let entity_id = cx.reserve_entity::<Self>().entity_id();
        let buffer_id = entity_id.as_non_zero_u64().into();
        let text = TextBuffer::new_normalized(
            ReplicaId::LOCAL,
            buffer_id,
            Default::default(),
            Rope::new(),
        );
        let text = text.into_snapshot();
        let syntax = SyntaxMap::new(&text).snapshot();
        let tree_sitter_data = TreeSitterData::new(&text);
        BufferSnapshot {
            text,
            syntax,
            tree_sitter_data: Arc::new(tree_sitter_data),
            file: None,
            diagnostics: Default::default(),
            remote_selections: Default::default(),
            language: None,
            non_text_state_update_count: 0,
            capability: Capability::ReadOnly,
            modeline: None,
            resolved_settings: None,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn build_snapshot_sync(
        text: Rope,
        language: Option<Arc<Language>>,
        language_registry: Option<Arc<LanguageRegistry>>,
        cx: &mut App,
    ) -> BufferSnapshot {
        let entity_id = cx.reserve_entity::<Self>().entity_id();
        let buffer_id = entity_id.as_non_zero_u64().into();
        let text =
            TextBuffer::new_normalized(ReplicaId::LOCAL, buffer_id, Default::default(), text)
                .into_snapshot();
        let mut syntax = SyntaxMap::new(&text).snapshot();
        if let Some(language) = language.clone() {
            syntax.reparse(&text, language_registry, language);
        }
        let tree_sitter_data = TreeSitterData::new(&text);
        BufferSnapshot {
            text,
            syntax,
            tree_sitter_data: Arc::new(tree_sitter_data),
            file: None,
            diagnostics: Default::default(),
            remote_selections: Default::default(),
            language,
            non_text_state_update_count: 0,
            capability: Capability::ReadOnly,
            modeline: None,
            resolved_settings: None,
        }
    }

    /// Retrieve a snapshot of the buffer's current state. This is computationally
    /// cheap, and allows reading from the buffer on a background thread.
    pub fn snapshot(&self) -> BufferSnapshot {
        let text = self.text.snapshot();

        let syntax = {
            let mut syntax_map = self.syntax_map.lock();
            syntax_map.interpolate(text);
            syntax_map.snapshot()
        };

        let tree_sitter_data = if self.text.version() != *self.tree_sitter_data.version() {
            Arc::new(TreeSitterData::new(text))
        } else {
            self.tree_sitter_data.clone()
        };

        BufferSnapshot {
            text: text.clone(),
            syntax,
            tree_sitter_data,
            file: self.file.clone(),
            remote_selections: self.remote_selections.clone(),
            diagnostics: self.diagnostics.clone(),
            language: self.language.clone(),
            non_text_state_update_count: self.non_text_state_update_count,
            capability: self.capability,
            modeline: self.modeline.clone(),
            resolved_settings: self.resolved_settings.clone(),
        }
    }

    pub fn branch(&mut self, cx: &mut Context<Self>) -> Entity<Self> {
        let this = cx.entity();
        cx.new(|cx| {
            let mut branch = Self {
                branch_state: Some(BufferBranchState {
                    base_buffer: this.clone(),
                    merged_operations: Default::default(),
                }),
                language: self.language.clone(),
                content_language_detection_enabled: self.content_language_detection_enabled,
                has_conflict: self.has_conflict,
                has_unsaved_edits: Cell::new(self.has_unsaved_edits.get_mut().clone()),
                _subscriptions: vec![cx.subscribe(&this, Self::on_base_buffer_event)],
                ..Self::build(self.text.branch(), self.file.clone(), self.capability(), cx)
            };
            if let Some(language_registry) = self.language_registry() {
                branch.set_language_registry(language_registry);
            }

            // Reparse the branch buffer so that we get syntax highlighting immediately.
            branch.reparse(cx, true);

            branch
        })
    }

    #[a_tracing::instrument(skip_all)]
    pub fn preview_edits(
        &self,
        edits: Arc<[(Range<Anchor>, Arc<str>)]>,
        cx: &App,
    ) -> Task<EditPreview> {
        let registry = self.language_registry();
        let language = self.language().cloned();
        let old_snapshot = self.text.snapshot().clone();
        let mut branch_buffer = self.text.branch();
        let mut syntax_snapshot = self.syntax_map.lock().snapshot();
        cx.background_spawn(async move {
            if !edits.is_empty() {
                if let Some(language) = language.clone() {
                    syntax_snapshot.reparse(&old_snapshot, registry.clone(), language);
                }

                branch_buffer.edit(edits.iter().cloned());
                let snapshot = branch_buffer.snapshot();
                syntax_snapshot.interpolate(&snapshot);

                if let Some(language) = language {
                    syntax_snapshot.reparse(&snapshot, registry, language);
                }
            }
            EditPreview {
                old_snapshot,
                applied_edits_snapshot: branch_buffer.into_snapshot(),
                syntax_snapshot,
            }
        })
    }

    /// Applies all of the changes in this buffer that intersect any of the
    /// given `ranges` to its base buffer.
    ///
    /// If `ranges` is empty, then all changes will be applied. This buffer must
    /// be a branch buffer to call this method.
    pub fn merge_into_base(&mut self, ranges: Vec<Range<usize>>, cx: &mut Context<Self>) {
        let Some(base_buffer) = self.base_buffer() else {
            debug_panic!("not a branch buffer");
            return;
        };

        let mut ranges = if ranges.is_empty() {
            &[0..usize::MAX]
        } else {
            ranges.as_slice()
        }
        .iter()
        .peekable();

        let mut edits = Vec::new();
        for edit in self.edits_since::<usize>(&base_buffer.read(cx).version()) {
            let mut is_included = false;
            while let Some(range) = ranges.peek() {
                if range.end < edit.new.start {
                    ranges.next().unwrap();
                } else {
                    if range.start <= edit.new.end {
                        is_included = true;
                    }
                    break;
                }
            }

            if is_included {
                edits.push((
                    edit.old.clone(),
                    self.text_for_range(edit.new.clone()).collect::<String>(),
                ));
            }
        }

        let operation = base_buffer.update(cx, |base_buffer, cx| {
            // cx.emit(BufferEvent::DiffBaseChanged);
            base_buffer.edit(edits, None, cx)
        });

        if let Some(operation) = operation
            && let Some(BufferBranchState {
                merged_operations, ..
            }) = &mut self.branch_state
        {
            merged_operations.push(operation);
        }
    }

    fn on_base_buffer_event(
        &mut self,
        _: Entity<Buffer>,
        event: &BufferEvent,
        cx: &mut Context<Self>,
    ) {
        let BufferEvent::Operation { operation, .. } = event else {
            return;
        };
        let Some(BufferBranchState {
            merged_operations, ..
        }) = &mut self.branch_state
        else {
            return;
        };

        let mut operation_to_undo = None;
        if let Operation::Buffer(text::Operation::Edit(operation)) = &operation
            && let Ok(ix) = merged_operations.binary_search(&operation.timestamp)
        {
            merged_operations.remove(ix);
            operation_to_undo = Some(operation.timestamp);
        }

        self.apply_ops([operation.clone()], cx);

        if let Some(timestamp) = operation_to_undo {
            let counts = [(timestamp, u32::MAX)].into_iter().collect();
            self.undo_operations(counts, cx);
        }
    }

    pub fn as_text_snapshot(&self) -> &text::BufferSnapshot { &self.text }

    /// Retrieve a snapshot of the buffer's raw text, without any
    /// language-related state like the syntax tree or diagnostics.
    #[a_tracing::instrument(skip_all)]
    pub fn text_snapshot(&self) -> text::BufferSnapshot {
        // todo lw
        self.text.snapshot().clone()
    }

    /// The file associated with the buffer, if any.
    pub fn file(&self) -> Option<&Arc<dyn File>> { self.file.as_ref() }

    /// The version of the buffer that was last saved or reloaded from disk.
    pub fn saved_version(&self) -> &clock::Global { &self.saved_version }

    /// The mtime of the buffer's file when the buffer was last saved or reloaded from disk.
    pub fn saved_mtime(&self) -> Option<MTime> { self.saved_mtime }

    /// Returns the character encoding of the buffer's file.
    pub fn encoding(&self) -> &'static Encoding { self.encoding }

    /// Sets the character encoding of the buffer.
    pub fn set_encoding(&mut self, encoding: &'static Encoding) { self.encoding = encoding; }

    /// Returns whether the buffer has a Byte Order Mark.
    pub fn has_bom(&self) -> bool { self.has_bom }

    /// Sets whether the buffer has a Byte Order Mark.
    pub fn set_has_bom(&mut self, has_bom: bool) { self.has_bom = has_bom; }

    pub fn set_content_language_detection_enabled(&mut self, enabled: bool) {
        self.content_language_detection_enabled = enabled;
    }

    pub fn content_language_detection_enabled(&self) -> bool {
        self.content_language_detection_enabled
    }

    /// Assign a language to the buffer.
    pub fn set_language_async(&mut self, language: Option<Arc<Language>>, cx: &mut Context<Self>) {
        self.set_language_(language, cfg!(any(test, feature = "test-support")), cx);
    }

    /// Assign a language to the buffer, blocking for up to 1ms to reparse the buffer.
    pub fn set_language(&mut self, language: Option<Arc<Language>>, cx: &mut Context<Self>) {
        self.set_language_(language, true, cx);
    }

    #[a_tracing::instrument(skip_all)]
    fn set_language_(
        &mut self,
        language: Option<Arc<Language>>,
        may_block: bool,
        cx: &mut Context<Self>,
    ) {
        if language == self.language {
            return;
        }
        self.non_text_state_update_count += 1;
        self.syntax_map.lock().clear(&self.text);
        Self::invalidate_tree_sitter_data(&mut self.tree_sitter_data, self.text.snapshot());
        let old_language = std::mem::replace(&mut self.language, language);
        self.refresh_resolved_settings(cx);
        self.was_changed();
        self.reparse(cx, may_block);
        let has_fresh_language =
            self.language.is_some() && old_language.is_none_or(|old| old == *PLAIN_TEXT);
        cx.emit(BufferEvent::LanguageChanged(has_fresh_language));
    }

    /// Assign a language registry to the buffer. This allows the buffer to retrieve
    /// other languages if parts of the buffer are written in different languages.
    pub fn set_language_registry(&self, language_registry: Arc<LanguageRegistry>) {
        self.syntax_map
            .lock()
            .set_language_registry(language_registry);
    }

    pub fn language_registry(&self) -> Option<Arc<LanguageRegistry>> {
        self.syntax_map.lock().language_registry()
    }

    /// Assign the line ending type to the buffer.
    pub fn set_line_ending(&mut self, line_ending: LineEnding, cx: &mut Context<Self>) {
        self.text.set_line_ending(line_ending);

        let lamport_timestamp = self.text.lamport_clock.tick();
        self.send_operation(
            Operation::UpdateLineEnding {
                line_ending,
                lamport_timestamp,
            },
            true,
            cx,
        );
    }

    /// Assign the buffer [`ModelineSettings`].
    pub fn set_modeline(
        &mut self,
        modeline: Option<ModelineSettings>,
        cx: &mut Context<Self>,
    ) -> bool {
        if modeline.as_ref() != self.modeline.as_deref() {
            self.modeline = modeline.map(Arc::new);
            self.refresh_resolved_settings(cx);
            true
        } else {
            false
        }
    }

    /// Returns the [`ModelineSettings`].
    pub fn modeline(&self) -> Option<&Arc<ModelineSettings>> { self.modeline.as_ref() }

    /// Assign the buffer a new [`Capability`].
    pub fn set_capability(&mut self, capability: Capability, cx: &mut Context<Self>) {
        if self.capability != capability {
            self.capability = capability;
            cx.emit(BufferEvent::CapabilityChanged)
        }
    }

    /// This method is called to signal that the buffer has been saved.
    pub fn did_save(
        &mut self,
        version: clock::Global,
        mtime: Option<MTime>,
        cx: &mut Context<Self>,
    ) {
        self.saved_version = version.clone();
        self.has_unsaved_edits.set((version, false));
        self.has_conflict = false;
        self.saved_mtime = mtime;
        self.was_changed();
        cx.emit(BufferEvent::Saved);
        cx.notify();
    }

    /// Reloads the contents of the buffer from disk.
    pub fn reload(&mut self, cx: &Context<Self>) -> oneshot::Receiver<Option<Transaction>> {
        self.reload_impl(None, cx)
    }

    /// Reloads the contents of the buffer from disk using the specified encoding.
    ///
    /// This bypasses automatic encoding detection heuristics (like BOM checks) for non-Unicode encodings,
    /// allowing users to force a specific interpretation of the bytes.
    pub fn reload_with_encoding(
        &mut self,
        encoding: &'static Encoding,
        cx: &Context<Self>,
    ) -> oneshot::Receiver<Option<Transaction>> {
        self.reload_impl(Some(encoding), cx)
    }

    fn reload_impl(
        &mut self,
        force_encoding: Option<&'static Encoding>,
        cx: &Context<Self>,
    ) -> oneshot::Receiver<Option<Transaction>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        let prev_version = self.text.version();

        self.reload_task = Some(cx.spawn(async move |this, cx| {
            let Some((new_mtime, load_bytes_task, current_encoding)) =
                this.update(cx, |this, cx| {
                    let file = this.file.as_ref()?.as_local()?;
                    Some((
                        file.disk_state().mtime(),
                        file.load_bytes(cx),
                        this.encoding,
                    ))
                })?
            else {
                return Ok(());
            };

            let target_encoding = force_encoding.unwrap_or(current_encoding);

            let bytes = load_bytes_task.await?;

            anyhow::ensure!(
                analyze_byte_content(&bytes) != ByteContent::Binary,
                "Binary files are not supported"
            );

            let is_unicode = target_encoding == encoding_rs::UTF_8
                || target_encoding == encoding_rs::UTF_16LE
                || target_encoding == encoding_rs::UTF_16BE;

            let (new_text, has_bom, encoding_used) = if force_encoding.is_some() && !is_unicode {
                let (cow, _had_errors) = target_encoding.decode_without_bom_handling(&bytes);
                (cow.into_owned(), false, target_encoding)
            } else {
                let (cow, used_enc, _had_errors) = target_encoding.decode(&bytes);

                let actual_has_bom = if used_enc == encoding_rs::UTF_8 {
                    bytes.starts_with(&[0xEF, 0xBB, 0xBF])
                } else if used_enc == encoding_rs::UTF_16LE {
                    bytes.starts_with(&[0xFF, 0xFE])
                } else if used_enc == encoding_rs::UTF_16BE {
                    bytes.starts_with(&[0xFE, 0xFF])
                } else {
                    false
                };
                (cow.into_owned(), actual_has_bom, used_enc)
            };

            let diff = this.update(cx, |this, cx| this.diff(new_text, cx))?.await;
            this.update(cx, |this, cx| {
                if this.version() == diff.base_version {
                    this.finalize_last_transaction();
                    let old_encoding = this.encoding;
                    let old_has_bom = this.has_bom;
                    this.apply_diff(diff, cx);
                    this.encoding = encoding_used;
                    this.has_bom = has_bom;
                    let transaction = this.finalize_last_transaction().cloned();
                    if let Some(ref txn) = transaction {
                        if old_encoding != encoding_used || old_has_bom != has_bom {
                            this.reload_with_encoding_txns
                                .insert(txn.id, (old_encoding, old_has_bom));
                        }
                    }
                    tx.send(transaction).ok();
                    this.has_conflict = false;
                    this.did_reload(this.version(), this.line_ending(), new_mtime, cx);
                } else {
                    if !diff.edits.is_empty()
                        || this
                            .edits_since::<usize>(&diff.base_version)
                            .next()
                            .is_some()
                    {
                        this.has_conflict = true;
                    }

                    this.did_reload(prev_version, this.line_ending(), this.saved_mtime, cx);
                }

                this.reload_task.take();
            })
        }));
        rx
    }

    /// This method is called to signal that the buffer has been reloaded.
    pub fn did_reload(
        &mut self,
        version: clock::Global,
        line_ending: LineEnding,
        mtime: Option<MTime>,
        cx: &mut Context<Self>,
    ) {
        self.saved_version = version;
        self.has_unsaved_edits
            .set((self.saved_version.clone(), false));
        self.text.set_line_ending(line_ending);
        self.saved_mtime = mtime;
        cx.emit(BufferEvent::Reloaded);
        cx.notify();
    }

    /// Updates the [`File`] backing this buffer. This should be called when
    /// the file has changed or has been deleted.
    pub fn file_updated(&mut self, new_file: Arc<dyn File>, cx: &mut Context<Self>) {
        let was_dirty = self.is_dirty();
        let mut file_changed = false;

        if let Some(old_file) = self.file.as_ref() {
            if new_file.path() != old_file.path() {
                file_changed = true;
            }

            let old_state = old_file.disk_state();
            let new_state = new_file.disk_state();
            if old_state != new_state {
                file_changed = true;
                if !was_dirty && matches!(new_state, DiskState::Present { .. }) {
                    cx.emit(BufferEvent::ReloadNeeded)
                }
            }
        } else {
            file_changed = true;
        };

        self.file = Some(new_file);
        if file_changed {
            self.refresh_resolved_settings(cx);
            self.was_changed();
            self.non_text_state_update_count += 1;
            if was_dirty != self.is_dirty() {
                cx.emit(BufferEvent::DirtyChanged);
            }
            cx.emit(BufferEvent::FileHandleChanged);
            cx.notify();
        }
    }

    pub fn base_buffer(&self) -> Option<Entity<Self>> {
        Some(self.branch_state.as_ref()?.base_buffer.clone())
    }

    /// Returns the primary [`Language`] assigned to this [`Buffer`].
    pub fn language(&self) -> Option<&Arc<Language>> { self.language.as_ref() }

    /// Returns the [`Language`] at the given location.
    pub fn language_at<D: ToOffset>(&self, position: D) -> Option<Arc<Language>> {
        let offset = position.to_offset(self);
        let text: &TextBufferSnapshot = &self.text;
        self.syntax_map
            .lock()
            .layers_for_range(offset..offset, text, false)
            .filter(|layer| {
                layer
                    .included_sub_ranges
                    .is_none_or(|ranges| offset_in_sub_ranges(ranges, offset, text))
            })
            .last()
            .map(|info| info.language.clone())
            .or_else(|| self.language.clone())
    }

    /// Returns each [`Language`] for the active syntax layers at the given location.
    pub fn languages_at<D: ToOffset>(&self, position: D) -> Vec<Arc<Language>> {
        let offset = position.to_offset(self);
        let text: &TextBufferSnapshot = &self.text;
        let mut languages: Vec<Arc<Language>> = self
            .syntax_map
            .lock()
            .layers_for_range(offset..offset, text, false)
            .filter(|layer| {
                // For combined injections, check if offset is within the actual sub-ranges.
                layer
                    .included_sub_ranges
                    .is_none_or(|ranges| offset_in_sub_ranges(ranges, offset, text))
            })
            .map(|info| info.language.clone())
            .collect();

        if languages.is_empty()
            && let Some(buffer_language) = self.language()
        {
            languages.push(buffer_language.clone());
        }

        languages
    }

    /// An integer version number that accounts for all updates besides
    /// the buffer's text itself (which is versioned via a version vector).
    pub fn non_text_state_update_count(&self) -> usize { self.non_text_state_update_count }

    /// Whether the buffer is being parsed in the background.
    #[cfg(any(test, feature = "test-support"))]
    pub fn is_parsing(&self) -> bool { self.reparse.is_some() }

    /// Indicates whether the buffer contains any regions that may be
    /// written in a language that hasn't been loaded yet.
    pub fn contains_unknown_injections(&self) -> bool {
        self.syntax_map.lock().contains_unknown_injections()
    }

    /// Sets the sync parse timeout for this buffer.
    ///
    /// Setting this to `None` disables sync parsing entirely.
    pub fn set_sync_parse_timeout(&mut self, timeout: Option<Duration>) {
        self.sync_parse_timeout = timeout;
    }

    fn invalidate_tree_sitter_data(
        tree_sitter_data: &mut Arc<TreeSitterData>,
        snapshot: &text::BufferSnapshot,
    ) {
        match Arc::get_mut(tree_sitter_data) {
            Some(tree_sitter_data) => tree_sitter_data.clear(snapshot),
            None => {
                let new_tree_sitter_data = TreeSitterData::new(snapshot);
                *tree_sitter_data = Arc::new(new_tree_sitter_data)
            }
        }
    }

    /// Called after an edit to synchronize the buffer's main parse tree with
    /// the buffer's new underlying state.
    ///
    /// Locks the syntax map and interpolates the edits since the last reparse
    /// into the foreground syntax tree.
    ///
    /// Then takes a stable snapshot of the syntax map before unlocking it.
    /// The snapshot with the interpolated edits is sent to a background thread,
    /// where we ask Tree-sitter to perform an incremental parse.
    ///
    /// Meanwhile, in the foreground if `may_block` is true, we block the main
    /// thread for up to 1ms waiting on the parse to complete. As soon as it
    /// completes, we proceed synchronously, unless a 1ms timeout elapses.
    ///
    /// If we time out waiting on the parse, we spawn a second task waiting
    /// until the parse does complete and return with the interpolated tree still
    /// in the foreground. When the background parse completes, call back into
    /// the main thread and assign the foreground parse state.
    ///
    /// If the buffer or grammar changed since the start of the background parse,
    /// initiate an additional reparse recursively. To avoid concurrent parses
    /// for the same buffer, we only initiate a new parse if we are not already
    /// parsing in the background.
    #[a_tracing::instrument(skip_all)]
    pub fn reparse(&mut self, cx: &mut Context<Self>, may_block: bool) {
        if self.text.version() != *self.tree_sitter_data.version() {
            Self::invalidate_tree_sitter_data(&mut self.tree_sitter_data, self.text.snapshot());
        }
        if self.reparse.is_some() {
            return;
        }
        let language = if let Some(language) = self.language.clone() {
            language
        } else {
            return;
        };

        let text = self.text_snapshot();
        let parsed_version = self.version();

        let mut syntax_map = self.syntax_map.lock();
        syntax_map.interpolate(&text);
        let language_registry = syntax_map.language_registry();
        let mut syntax_snapshot = syntax_map.snapshot();
        drop(syntax_map);

        self.parse_status.0.send(ParseStatus::Parsing).unwrap();
        if may_block && let Some(sync_parse_timeout) = self.sync_parse_timeout {
            if let Ok(()) = syntax_snapshot.reparse_with_timeout(
                &text,
                language_registry.clone(),
                language.clone(),
                sync_parse_timeout,
            ) {
                self.did_finish_parsing(
                    syntax_snapshot,
                    Some(Duration::from_millis(300)),
                    false,
                    cx,
                );
                self.reparse = None;
                return;
            }
        }

        let parse_task = cx.background_spawn({
            let language = language.clone();
            let language_registry = language_registry.clone();
            async move {
                syntax_snapshot.reparse(&text, language_registry, language);
                syntax_snapshot
            }
        });

        self.reparse = Some(cx.spawn(async move |this, cx| {
            let new_syntax_map = parse_task.await;
            this.update(cx, move |this, cx| {
                let grammar_changed = || {
                    this.language
                        .as_ref()
                        .is_none_or(|current_language| !Arc::ptr_eq(&language, current_language))
                };
                let language_registry_changed = || {
                    new_syntax_map.contains_unknown_injections()
                        && language_registry.is_some_and(|registry| {
                            registry.version() != new_syntax_map.language_registry_version()
                        })
                };
                let parse_again = this.version.changed_since(&parsed_version)
                    || language_registry_changed()
                    || grammar_changed();
                this.did_finish_parsing(new_syntax_map, None, parse_again, cx);
                this.reparse = None;
                if parse_again {
                    this.reparse(cx, false);
                }
            })
            .ok();
        }));
    }

    fn did_finish_parsing(
        &mut self,
        syntax_snapshot: SyntaxSnapshot,
        block_budget: Option<Duration>,
        parse_again: bool,
        cx: &mut Context<Self>,
    ) {
        self.non_text_state_update_count += 1;
        self.syntax_map.lock().did_parse(syntax_snapshot);
        self.was_changed();

        let parsing_complete = !parse_again || self.language.is_none();
        if self.language.is_none() {
            self.syntax_map.lock().clear(&self.text);
        }
        if parsing_complete {
            self.request_autoindent(cx, block_budget);
            self.parse_status.0.send(ParseStatus::Idle).unwrap();
        }
        Self::invalidate_tree_sitter_data(&mut self.tree_sitter_data, &self.text.snapshot());
        cx.emit(BufferEvent::Reparsed);
        cx.notify();
    }

    pub fn parse_status(&self) -> watch::Receiver<ParseStatus> { self.parse_status.1.clone() }

    /// Wait until the buffer is no longer parsing
    pub fn parsing_idle(&self) -> impl Future<Output = ()> + use<> {
        let mut parse_status = self.parse_status();
        async move {
            while *parse_status.borrow() != ParseStatus::Idle {
                if parse_status.changed().await.is_err() {
                    break;
                }
            }
        }
    }

    /// Assign to the buffer a set of diagnostics created by a given language server.
    pub fn update_diagnostics(
        &mut self,
        server_id: LanguageServerId,
        diagnostics: DiagnosticSet,
        cx: &mut Context<Self>,
    ) {
        let lamport_timestamp = self.text.lamport_clock.tick();
        let op = Operation::UpdateDiagnostics {
            server_id,
            diagnostics: diagnostics.iter().cloned().collect(),
            lamport_timestamp,
        };

        self.apply_diagnostic_update(server_id, diagnostics, lamport_timestamp, cx);
        self.send_operation(op, true, cx);
    }

    pub fn buffer_diagnostics(
        &self,
        for_server: Option<LanguageServerId>,
    ) -> Vec<&DiagnosticEntry<Anchor>> {
        match for_server {
            Some(server_id) => self
                .diagnostics
                .get(&server_id)
                .map_or_else(Vec::new, |diagnostics| diagnostics.iter().collect()),
            None => self
                .diagnostics
                .iter()
                .flat_map(|(_, diagnostic_set)| diagnostic_set.iter())
                .collect(),
        }
    }

    fn request_autoindent(&mut self, cx: &mut Context<Self>, block_budget: Option<Duration>) {
        if let Some(indent_sizes) = self.compute_autoindents() {
            let indent_sizes = cx.background_spawn(indent_sizes);
            let Some(block_budget) = block_budget else {
                self.pending_autoindent = Some(cx.spawn(async move |this, cx| {
                    let indent_sizes = indent_sizes.await;
                    this.update(cx, |this, cx| {
                        this.apply_autoindents(indent_sizes, cx);
                    })
                    .ok();
                }));
                return;
            };
            match cx
                .foreground_executor()
                .block_with_timeout(block_budget, indent_sizes)
            {
                Ok(indent_sizes) => self.apply_autoindents(indent_sizes, cx),
                Err(indent_sizes) => {
                    self.pending_autoindent = Some(cx.spawn(async move |this, cx| {
                        let indent_sizes = indent_sizes.await;
                        this.update(cx, |this, cx| {
                            this.apply_autoindents(indent_sizes, cx);
                        })
                        .ok();
                    }));
                }
            }
        } else {
            self.autoindent_requests.clear();
            for tx in self.wait_for_autoindent_txs.drain(..) {
                tx.send(()).ok();
            }
        }
    }

    fn compute_autoindents(
        &self,
    ) -> Option<impl Future<Output = BTreeMap<u32, IndentSize>> + use<>> {
        let max_rows_between_yields = 100;
        let snapshot = self.snapshot();
        if snapshot.syntax.is_empty() || self.autoindent_requests.is_empty() {
            return None;
        }

        let autoindent_requests = self.autoindent_requests.clone();
        Some(async move {
            let mut indent_sizes = BTreeMap::<u32, (IndentSize, bool)>::new();
            for request in autoindent_requests {
                // Resolve each edited range to its row in the current buffer and in the
                // buffer before this batch of edits.
                let mut row_ranges = Vec::new();
                let mut old_to_new_rows = BTreeMap::new();
                let mut language_indent_sizes_by_new_row = Vec::new();
                for entry in &request.entries {
                    let position = entry.range.start;
                    let new_row = position.to_point(&snapshot).row;
                    let new_end_row = entry.range.end.to_point(&snapshot).row + 1;
                    language_indent_sizes_by_new_row.push((new_row, entry.indent_size));

                    if let Some(old_row) = entry.old_row {
                        old_to_new_rows.insert(old_row, new_row);
                    }
                    row_ranges.push((new_row..new_end_row, entry.original_indent_column));
                }

                // Build a map containing the suggested indentation for each of the edited lines
                // with respect to the state of the buffer before these edits. This map is keyed
                // by the rows for these lines in the current state of the buffer.
                let mut old_suggestions = BTreeMap::<u32, (IndentSize, bool)>::default();
                let old_edited_ranges =
                    contiguous_ranges(old_to_new_rows.keys().copied(), max_rows_between_yields);
                let mut language_indent_sizes = language_indent_sizes_by_new_row.iter().peekable();
                let mut language_indent_size = IndentSize::default();
                for old_edited_range in old_edited_ranges {
                    let suggestions = request
                        .before_edit
                        .suggest_autoindents(old_edited_range.clone())
                        .into_iter()
                        .flatten();
                    for (old_row, suggestion) in old_edited_range.zip(suggestions) {
                        if let Some(suggestion) = suggestion {
                            let new_row = *old_to_new_rows.get(&old_row).unwrap();

                            // Find the indent size based on the language for this row.
                            while let Some((row, size)) = language_indent_sizes.peek() {
                                if *row > new_row {
                                    break;
                                }
                                language_indent_size = *size;
                                language_indent_sizes.next();
                            }

                            let suggested_indent = old_to_new_rows
                                .get(&suggestion.basis_row)
                                .and_then(|from_row| {
                                    Some(old_suggestions.get(from_row).copied()?.0)
                                })
                                .unwrap_or_else(|| {
                                    request
                                        .before_edit
                                        .logical_indent_size_for_line(suggestion.basis_row)
                                })
                                .with_delta(suggestion.delta, language_indent_size);
                            old_suggestions
                                .insert(new_row, (suggested_indent, suggestion.within_error));
                        }
                    }
                    yield_now().await;
                }

                // Compute new suggestions for each line, but only include them in the result
                // if they differ from the old suggestion for that line.
                let mut language_indent_sizes = language_indent_sizes_by_new_row.iter().peekable();
                let mut language_indent_size = IndentSize::default();
                for (row_range, original_indent_column) in row_ranges {
                    let new_edited_row_range = if request.is_block_mode {
                        row_range.start..row_range.start + 1
                    } else {
                        row_range.clone()
                    };

                    let suggestions = snapshot
                        .suggest_autoindents(new_edited_row_range.clone())
                        .into_iter()
                        .flatten();
                    for (new_row, suggestion) in new_edited_row_range.zip(suggestions) {
                        if let Some(suggestion) = suggestion {
                            // Find the indent size based on the language for this row.
                            while let Some((row, size)) = language_indent_sizes.peek() {
                                if *row > new_row {
                                    break;
                                }
                                language_indent_size = *size;
                                language_indent_sizes.next();
                            }

                            let suggested_indent = indent_sizes
                                .get(&suggestion.basis_row)
                                .copied()
                                .map(|e| e.0)
                                .unwrap_or_else(|| {
                                    snapshot.logical_indent_size_for_line(suggestion.basis_row)
                                })
                                .with_delta(suggestion.delta, language_indent_size);

                            if old_suggestions.get(&new_row).is_none_or(
                                |(old_indentation, was_within_error)| {
                                    suggested_indent != *old_indentation
                                        && (!suggestion.within_error || *was_within_error)
                                },
                            ) {
                                indent_sizes.insert(
                                    new_row,
                                    (suggested_indent, request.ignore_empty_lines),
                                );
                            }
                        }
                    }

                    if let (true, Some(original_indent_column)) =
                        (request.is_block_mode, original_indent_column)
                    {
                        let new_indent =
                            if let Some((indent, _)) = indent_sizes.get(&row_range.start) {
                                *indent
                            } else {
                                snapshot.indent_size_for_line(row_range.start)
                            };
                        let delta = new_indent.len as i64 - original_indent_column as i64;
                        if delta != 0 {
                            for row in row_range.skip(1) {
                                indent_sizes.entry(row).or_insert_with(|| {
                                    let mut size = snapshot.indent_size_for_line(row);
                                    // A line with no indentation has an arbitrary
                                    // indent kind, so it can adopt the new kind.
                                    if size.len == 0 {
                                        size.kind = new_indent.kind;
                                    }
                                    if size.kind == new_indent.kind {
                                        match delta.cmp(&0) {
                                            Ordering::Greater => size.len += delta as u32,
                                            Ordering::Less => {
                                                size.len = size.len.saturating_sub(-delta as u32)
                                            }
                                            Ordering::Equal => {}
                                        }
                                    }
                                    (size, request.ignore_empty_lines)
                                });
                            }
                        }
                    }

                    yield_now().await;
                }
            }

            indent_sizes
                .into_iter()
                .filter_map(|(row, (indent, ignore_empty_lines))| {
                    if ignore_empty_lines && snapshot.line_len(row) == 0 {
                        None
                    } else {
                        Some((row, indent))
                    }
                })
                .collect()
        })
    }

    fn apply_autoindents(
        &mut self,
        indent_sizes: BTreeMap<u32, IndentSize>,
        cx: &mut Context<Self>,
    ) {
        self.autoindent_requests.clear();
        for tx in self.wait_for_autoindent_txs.drain(..) {
            tx.send(()).ok();
        }

        let edits: Vec<_> = indent_sizes
            .into_iter()
            .filter_map(|(row, indent_size)| {
                let current_size = indent_size_for_line(self, row);
                Self::edit_for_indent_size_adjustment(row, current_size, indent_size)
            })
            .collect();

        let preserve_preview = self.preserve_preview();
        self.edit(edits, None, cx);
        if preserve_preview {
            self.refresh_preview();
        }
    }

    /// Create a minimal edit that will cause the given row to be indented
    /// with the given size. After applying this edit, the length of the line
    /// will always be at least `new_size.len`.
    pub fn edit_for_indent_size_adjustment(
        row: u32,
        current_size: IndentSize,
        new_size: IndentSize,
    ) -> Option<(Range<Point>, String)> {
        if new_size.kind == current_size.kind {
            match new_size.len.cmp(&current_size.len) {
                Ordering::Greater => {
                    let point = Point::new(row, 0);
                    Some((
                        point..point,
                        iter::repeat(new_size.char())
                            .take((new_size.len - current_size.len) as usize)
                            .collect::<String>(),
                    ))
                }

                Ordering::Less => Some((
                    Point::new(row, 0)..Point::new(row, current_size.len - new_size.len),
                    String::new(),
                )),

                Ordering::Equal => None,
            }
        } else {
            Some((
                Point::new(row, 0)..Point::new(row, current_size.len),
                iter::repeat(new_size.char())
                    .take(new_size.len as usize)
                    .collect::<String>(),
            ))
        }
    }

    /// Spawns a background task that asynchronously computes a `Diff` between the buffer's text
    /// and the given new text.
    pub fn diff<T>(&self, new_text: T, cx: &App) -> Task<Diff>
    where
        T: AsRef<str> + Send + 'static,
    {
        let old_text = self.as_rope().clone();
        let base_version = self.version();
        cx.background_spawn(async move {
            let old_text = old_text.to_string();
            let mut new_text = new_text.as_ref().to_owned();
            let line_ending = LineEnding::detect(&new_text);
            LineEnding::normalize(&mut new_text);
            let edits = text_diff(&old_text, &new_text);
            Diff {
                base_version,
                line_ending,
                edits,
            }
        })
    }

    /// Spawns a background task that returns a `Diff` removing trailing whitespace from line ends.
    ///
    /// When `modified_rows` is `Some`, only lines whose row falls within one of the given ranges
    /// are trimmed; when it is `None`, the whole buffer is scanned.
    pub fn remove_trailing_whitespace(
        &self,
        modified_rows: Option<&[Range<u32>]>,
        cx: &App,
    ) -> Task<Diff> {
        let old_text = self.as_rope().clone();
        let line_ending = self.line_ending();
        let base_version = self.version();
        let modified_rows = modified_rows.map(|rows| rows.to_vec());
        cx.background_spawn(async move {
            let ranges = trailing_whitespace_ranges(&old_text, modified_rows.as_deref());
            let empty = Arc::<str>::from("");
            Diff {
                base_version,
                line_ending,
                edits: ranges
                    .into_iter()
                    .map(|range| (range, empty.clone()))
                    .collect(),
            }
        })
    }

    /// Returns a `Diff` ensuring the buffer ends with a trailing newline.
    ///
    /// When `modified_rows` is `None`, the whole buffer is considered: trailing whitespace and
    /// blank lines at the end of the file are collapsed into a single newline.
    ///
    /// When `modified_rows` is `Some`, the operation is scoped to a "Format Selection": a single
    /// newline is appended only when the last line is non-empty and its row falls within one of
    /// the ranges. Trailing blank lines are left intact so that formatting a selection cannot
    /// delete unselected rows.
    pub fn ensure_final_newline(&self, modified_rows: Option<&[Range<u32>]>) -> Diff {
        let len = self.len();
        let line_ending = self.line_ending();
        let base_version = self.version();
        let newline = Arc::<str>::from("\n");

        let edits = if len == 0 {
            Vec::new()
        } else if let Some(modified_rows) = modified_rows {
            let max_point = self.max_point();
            let last_line_is_empty = max_point.column == 0;
            let last_row_is_modified = modified_rows
                .iter()
                .any(|range| range.contains(&max_point.row));
            if last_line_is_empty || !last_row_is_modified {
                Vec::new()
            } else {
                Vec::from([(len..len, newline)])
            }
        } else {
            let mut offset = len;
            let mut already_normalized = false;
            for chunk in self.as_rope().reversed_chunks_in_range(0..len) {
                let non_whitespace_len = chunk
                    .trim_end_matches(|c: char| c.is_ascii_whitespace())
                    .len();
                offset -= chunk.len();
                offset += non_whitespace_len;
                if non_whitespace_len != 0 {
                    already_normalized =
                        offset == len - 1 && chunk.get(non_whitespace_len..) == Some("\n");
                    break;
                }
            }
            if already_normalized {
                Vec::new()
            } else {
                Vec::from([(offset..len, newline)])
            }
        };
        Diff {
            base_version,
            line_ending,
            edits,
        }
    }

    /// Applies a diff to the buffer. If the buffer has changed since the given diff was
    /// calculated, then adjust the diff to account for those changes, and discard any
    /// parts of the diff that conflict with those changes.
    pub fn apply_diff(&mut self, diff: Diff, cx: &mut Context<Self>) -> Option<TransactionId> {
        let snapshot = self.snapshot();
        let mut edits_since = snapshot.edits_since::<usize>(&diff.base_version).peekable();
        let mut delta = 0;
        let adjusted_edits = diff.edits.into_iter().filter_map(|(range, new_text)| {
            while let Some(edit_since) = edits_since.peek() {
                // If the edit occurs after a diff hunk, then it does not
                // affect that hunk.
                if edit_since.old.start > range.end {
                    break;
                }
                // If the edit precedes the diff hunk, then adjust the hunk
                // to reflect the edit.
                else if edit_since.old.end < range.start {
                    delta += edit_since.new_len() as i64 - edit_since.old_len() as i64;
                    edits_since.next();
                }
                // If the edit intersects a diff hunk, then discard that hunk.
                else {
                    return None;
                }
            }

            let start = (range.start as i64 + delta) as usize;
            let end = (range.end as i64 + delta) as usize;
            Some((start..end, new_text))
        });

        self.start_transaction();
        self.text.set_line_ending(diff.line_ending);
        self.edit(adjusted_edits, None, cx);
        self.end_transaction(cx)
    }

    pub fn has_unsaved_edits(&self) -> bool {
        let (last_version, has_unsaved_edits) = self.has_unsaved_edits.take();

        if last_version == self.version {
            self.has_unsaved_edits
                .set((last_version, has_unsaved_edits));
            return has_unsaved_edits;
        }

        let has_edits = self.has_edits_since(&self.saved_version);
        self.has_unsaved_edits
            .set((self.version.clone(), has_edits));
        has_edits
    }

    /// Checks if the buffer has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        if self.capability == Capability::ReadOnly {
            return false;
        }
        if self.has_conflict {
            return true;
        }
        match self.file.as_ref().map(|f| f.disk_state()) {
            Some(DiskState::New) | Some(DiskState::Deleted) => {
                !self.is_empty() && self.has_unsaved_edits()
            }
            _ => self.has_unsaved_edits(),
        }
    }

    /// Marks the buffer as having a conflict regardless of current buffer state.
    pub fn set_conflict(&mut self) { self.has_conflict = true; }

    /// Checks if the buffer and its file have both changed since the buffer
    /// was last saved or reloaded.
    pub fn has_conflict(&self) -> bool {
        if self.has_conflict {
            return true;
        }
        let Some(file) = self.file.as_ref() else {
            return false;
        };
        match file.disk_state() {
            DiskState::New => false,
            DiskState::Present { mtime, .. } => match self.saved_mtime {
                Some(saved_mtime) => mtime.bad_is_greater_than(saved_mtime) && self.has_unsaved_edits(),
                None => true,
            },
            DiskState::Deleted => false,
            DiskState::Historic { .. } => false,
        }
    }

    /// Gets a [`Subscription`] that tracks all of the changes to the buffer's text.
    pub fn subscribe(&mut self) -> Subscription<usize> { self.text.subscribe() }

    /// Adds a bit to the list of bits that are set when the buffer's text changes.
    ///
    /// This allows downstream code to check if the buffer's text has changed without
    /// waiting for an effect cycle, which would be required if using eents.
    pub fn record_changes(&mut self, bit: rc::Weak<Cell<bool>>) {
        if let Err(ix) = self
            .change_bits
            .binary_search_by_key(&rc::Weak::as_ptr(&bit), rc::Weak::as_ptr)
        {
            self.change_bits.insert(ix, bit);
        }
    }

    /// Set the change bit for all "listeners".
    fn was_changed(&mut self) {
        self.change_bits.retain(|change_bit| {
            change_bit
                .upgrade()
                .inspect(|bit| {
                    _ = bit.replace(true);
                })
                .is_some()
        });
    }

    /// Starts a transaction, if one is not already in-progress. When undoing or
    /// redoing edits, all of the edits performed within a transaction are undone
    /// or redone together.
    pub fn start_transaction(&mut self) -> Option<TransactionId> {
        self.start_transaction_at(Instant::now())
    }

    /// Starts a transaction, providing the current time. Subsequent transactions
    /// that occur within a short period of time will be grouped together. This
    /// is controlled by the buffer's undo grouping duration.
    pub fn start_transaction_at(&mut self, now: Instant) -> Option<TransactionId> {
        self.transaction_depth += 1;
        if self.was_dirty_before_starting_transaction.is_none() {
            self.was_dirty_before_starting_transaction = Some(self.is_dirty());
        }
        self.text.start_transaction_at(now)
    }

    /// Terminates the current transaction, if this is the outermost transaction.
    pub fn end_transaction(&mut self, cx: &mut Context<Self>) -> Option<TransactionId> {
        self.end_transaction_at(Instant::now(), cx)
    }

    pub fn end_transaction_with_source(
        &mut self,
        source: BufferEditSource,
        cx: &mut Context<Self>,
    ) -> Option<TransactionId> {
        self.end_transaction_at_internal(Instant::now(), source, cx)
    }

    /// Terminates the current transaction, providing the current time. Subsequent transactions
    /// that occur within a short period of time will be grouped together. This
    /// is controlled by the buffer's undo grouping duration.
    pub fn end_transaction_at(
        &mut self,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> Option<TransactionId> {
        self.end_transaction_at_internal(now, BufferEditSource::User, cx)
    }

    fn end_transaction_at_internal(
        &mut self,
        now: Instant,
        source: BufferEditSource,
        cx: &mut Context<Self>,
    ) -> Option<TransactionId> {
        assert!(self.transaction_depth > 0);
        self.transaction_depth -= 1;
        let was_dirty = if self.transaction_depth == 0 {
            self.was_dirty_before_starting_transaction.take().unwrap()
        } else {
            false
        };
        if let Some((transaction_id, start_version)) = self.text.end_transaction_at(now) {
            self.did_edit(&start_version, was_dirty, source, cx);
            Some(transaction_id)
        } else {
            None
        }
    }

    /// Manually add a transaction to the buffer's undo history.
    pub fn push_transaction(&mut self, transaction: Transaction, now: Instant) {
        self.text.push_transaction(transaction, now);
    }

    /// Differs from `push_transaction` in that it does not clear the redo
    /// stack. Intended to be used to create a parent transaction to merge
    /// potential child transactions into.
    ///
    /// The caller is responsible for removing it from the undo history using
    /// `forget_transaction` if no edits are merged into it. Otherwise, if edits
    /// are merged into this transaction, the caller is responsible for ensuring
    /// the redo stack is cleared. The easiest way to ensure the redo stack is
    /// cleared is to create transactions with the usual `start_transaction` and
    /// `end_transaction` methods and merging the resulting transactions into
    /// the transaction created by this method
    pub fn push_empty_transaction(&mut self, now: Instant) -> TransactionId {
        self.text.push_empty_transaction(now)
    }

    /// Prevent the last transaction from being grouped with any subsequent transactions,
    /// even if they occur with the buffer's undo grouping duration.
    pub fn finalize_last_transaction(&mut self) -> Option<&Transaction> {
        self.text.finalize_last_transaction()
    }

    /// Manually group all changes since a given transaction.
    pub fn group_until_transaction(&mut self, transaction_id: TransactionId) {
        self.text.group_until_transaction(transaction_id);
    }

    /// Manually remove a transaction from the buffer's undo history
    pub fn forget_transaction(&mut self, transaction_id: TransactionId) -> Option<Transaction> {
        self.text.forget_transaction(transaction_id)
    }

    /// Retrieve a transaction from the buffer's undo history
    pub fn get_transaction(&self, transaction_id: TransactionId) -> Option<&Transaction> {
        self.text.get_transaction(transaction_id)
    }

    /// Manually merge two transactions in the buffer's undo history.
    pub fn merge_transactions(&mut self, transaction: TransactionId, destination: TransactionId) {
        self.text.merge_transactions(transaction, destination);
    }

    /// Waits for the buffer to receive operations with the given timestamps.
    pub fn wait_for_edits<It: IntoIterator<Item = clock::Lamport>>(
        &mut self,
        edit_ids: It,
    ) -> impl Future<Output = Result<()>> + use<It> {
        self.text.wait_for_edits(edit_ids)
    }

    /// Waits for the buffer to receive the operations necessary for resolving the given anchors.
    pub fn wait_for_anchors<It: IntoIterator<Item = Anchor>>(
        &mut self,
        anchors: It,
    ) -> impl 'static + Future<Output = Result<()>> + use<It> {
        self.text.wait_for_anchors(anchors)
    }

    /// Waits for the buffer to receive operations up to the given version.
    pub fn wait_for_version(
        &mut self,
        version: clock::Global,
    ) -> impl Future<Output = Result<()>> + use<> {
        self.text.wait_for_version(version)
    }

    /// Forces all futures returned by [`Buffer::wait_for_version`], [`Buffer::wait_for_edits`], or
    /// [`Buffer::wait_for_version`] to resolve with an error.
    pub fn give_up_waiting(&mut self) { self.text.give_up_waiting(); }

    pub fn wait_for_autoindent_applied(&mut self) -> Option<oneshot::Receiver<()>> {
        let mut rx = None;
        if !self.autoindent_requests.is_empty() {
            let channel = oneshot::channel();
            self.wait_for_autoindent_txs.push(channel.0);
            rx = Some(channel.1);
        }
        rx
    }

    /// Stores a set of selections that should be broadcasted to all of the buffer's replicas.
    pub fn set_active_selections(
        &mut self,
        selections: Arc<[Selection<Anchor>]>,
        line_mode: bool,
        cursor_shape: CursorShape,
        cx: &mut Context<Self>,
    ) {
        let lamport_timestamp = self.text.lamport_clock.tick();
        self.remote_selections.insert(
            self.text.replica_id(),
            SelectionSet {
                selections: selections.clone(),
                lamport_timestamp,
                line_mode,
                cursor_shape,
            },
        );
        self.send_operation(
            Operation::UpdateSelections {
                selections,
                line_mode,
                lamport_timestamp,
                cursor_shape,
            },
            true,
            cx,
        );
        self.non_text_state_update_count += 1;
        cx.notify();
    }

    /// Clears the selections, so that other replicas of the buffer do not see any selections for
    /// this replica.
    pub fn remove_active_selections(&mut self, cx: &mut Context<Self>) {
        if self
            .remote_selections
            .get(&self.text.replica_id())
            .is_none_or(|set| !set.selections.is_empty())
        {
            self.set_active_selections(Arc::default(), false, Default::default(), cx);
        }
    }

    pub fn set_agent_selections(
        &mut self,
        selections: Arc<[Selection<Anchor>]>,
        line_mode: bool,
        cursor_shape: CursorShape,
        cx: &mut Context<Self>,
    ) {
        let lamport_timestamp = self.text.lamport_clock.tick();
        self.remote_selections.insert(
            ReplicaId::AGENT,
            SelectionSet {
                selections,
                lamport_timestamp,
                line_mode,
                cursor_shape,
            },
        );
        self.non_text_state_update_count += 1;
        cx.notify();
    }

    pub fn remove_agent_selections(&mut self, cx: &mut Context<Self>) {
        self.set_agent_selections(Arc::default(), false, Default::default(), cx);
    }

    /// Replaces the buffer's entire text.
    pub fn set_text<T>(&mut self, text: T, cx: &mut Context<Self>) -> Option<clock::Lamport>
    where
        T: Into<Arc<str>>,
    {
        self.autoindent_requests.clear();
        self.edit([(0..self.len(), text)], None, cx)
    }

    /// Appends the given text to the end of the buffer.
    pub fn append<T>(&mut self, text: T, cx: &mut Context<Self>) -> Option<clock::Lamport>
    where
        T: Into<Arc<str>>,
    {
        self.edit([(self.len()..self.len(), text)], None, cx)
    }

    /// Applies the given edits to the buffer. Each edit is specified as a range of text to
    /// delete, and a string of text to insert at that location. Adjacent edits are coalesced.
    /// Inserted text is normalized to LF line endings before being applied.
    ///
    /// If an [`AutoindentMode`] is provided, then the buffer will enqueue an auto-indent
    /// request for the edited ranges, which will be processed when the buffer finishes
    /// parsing.
    ///
    /// Parsing takes place at the end of a transaction, and may compute synchronously
    /// or asynchronously, depending on the changes.
    pub fn edit<I, S, T>(
        &mut self,
        edits_iter: I,
        autoindent_mode: Option<AutoindentMode>,
        cx: &mut Context<Self>,
    ) -> Option<clock::Lamport>
    where
        I: IntoIterator<Item = (Range<S>, T)>,
        S: ToOffset,
        T: Into<Arc<str>>,
    {
        self.edit_internal(
            edits_iter,
            autoindent_mode,
            true,
            AutoIndentExclusion::PrecedingLine,
            cx,
        )
    }

    /// Like [`edit`](Self::edit), but preserves the following line's
    /// indentation when the inserted text ends with a newline.
    pub fn edit_before<I, S, T>(
        &mut self,
        edits_iter: I,
        autoindent_mode: Option<AutoindentMode>,
        cx: &mut Context<Self>,
    ) -> Option<clock::Lamport>
    where
        I: IntoIterator<Item = (Range<S>, T)>,
        S: ToOffset,
        T: Into<Arc<str>>,
    {
        self.edit_internal(
            edits_iter,
            autoindent_mode,
            true,
            AutoIndentExclusion::FollowingLine,
            cx,
        )
    }

    /// Like [`edit`](Self::edit), but does not coalesce adjacent edits.
    pub fn edit_non_coalesce<I, S, T>(
        &mut self,
        edits_iter: I,
        autoindent_mode: Option<AutoindentMode>,
        cx: &mut Context<Self>,
    ) -> Option<clock::Lamport>
    where
        I: IntoIterator<Item = (Range<S>, T)>,
        S: ToOffset,
        T: Into<Arc<str>>,
    {
        self.edit_internal(
            edits_iter,
            autoindent_mode,
            false,
            AutoIndentExclusion::PrecedingLine,
            cx,
        )
    }

    fn edit_internal<I, S, T>(
        &mut self,
        edits_iter: I,
        autoindent_mode: Option<AutoindentMode>,
        coalesce_adjacent: bool,
        autoindent_exclusion: AutoIndentExclusion,
        cx: &mut Context<Self>,
    ) -> Option<clock::Lamport>
    where
        I: IntoIterator<Item = (Range<S>, T)>,
        S: ToOffset,
        T: Into<Arc<str>>,
    {
        // Skip invalid edits and coalesce contiguous ones.
        let mut edits: Vec<(Range<usize>, Arc<str>)> = Vec::new();

        for (range, new_text) in edits_iter {
            let mut range = range.start.to_offset(self)..range.end.to_offset(self);

            if range.start > range.end {
                mem::swap(&mut range.start, &mut range.end);
            }
            let new_text = new_text.into();
            if !new_text.is_empty() || !range.is_empty() {
                let prev_edit = edits.last_mut();
                let should_coalesce = prev_edit.as_ref().is_some_and(|(prev_range, _)| {
                    if coalesce_adjacent {
                        prev_range.end >= range.start
                    } else {
                        prev_range.end > range.start
                    }
                });

                if let Some((prev_range, prev_text)) = prev_edit
                    && should_coalesce
                {
                    prev_range.end = cmp::max(prev_range.end, range.end);
                    *prev_text = format!("{prev_text}{new_text}").into();
                } else {
                    edits.push((range, new_text));
                }
            }
        }
        if edits.is_empty() {
            return None;
        }

        self.start_transaction();
        self.pending_autoindent.take();
        let autoindent_request = autoindent_mode
            .and_then(|mode| self.language.as_ref().map(|_| (self.snapshot(), mode)));

        let edit_operation = self.text.edit(edits.iter().cloned());
        let edit_id = edit_operation.timestamp();

        if let Some((before_edit, mode)) = autoindent_request {
            let mut delta = 0isize;
            let mut previous_setting = None;
            let entries: Vec<_> = edits
                .into_iter()
                .enumerate()
                .zip(&edit_operation.as_edit().unwrap().new_text)
                .filter(|((_, (range, _)), _)| {
                    let language = before_edit.language_at(range.start);
                    let language_id = language.map(|l| l.id());
                    if let Some((cached_language_id, apply_syntax_indent)) = previous_setting
                        && cached_language_id == language_id
                    {
                        apply_syntax_indent
                    } else {
                        // The auto-indent setting is not present in editorconfigs, hence
                        // we can avoid passing the file here.
                        let auto_indent_mode = LanguageSettings::resolve(
                            None,
                            language.map(|l| l.name()).as_ref(),
                            cx,
                        )
                        .auto_indent;
                        let apply_syntax_indent = auto_indent_mode == AutoIndentMode::SyntaxAware;
                        previous_setting = Some((language_id, apply_syntax_indent));
                        apply_syntax_indent
                    }
                })
                .map(|((ix, (range, _)), new_text)| {
                    let new_text_length = new_text.len();
                    let old_start = range.start.to_point(&before_edit);
                    let old_end = range.end.to_point(&before_edit);
                    let new_start = (delta + range.start as isize) as usize;
                    let range_len = range.end - range.start;
                    delta += new_text_length as isize - range_len as isize;

                    // Decide what range of the insertion to auto-indent, and whether
                    // the first line of the insertion should be considered a newly-inserted line
                    // or an edit to an existing line.
                    let mut range_of_insertion_to_indent = 0..new_text_length;
                    let mut first_line_is_new = true;

                    let old_line_start = before_edit.indent_size_for_line(old_start.row).len;
                    let old_line_end = before_edit.line_len(old_start.row);

                    if old_start.column > old_line_start {
                        first_line_is_new = false;
                    }

                    if !new_text.contains('\n')
                        && (old_end.row == old_start.row || old_line_end == old_line_start)
                    {
                        first_line_is_new = false;
                    }

                    // When the insertion introduces a line boundary, exclude
                    // the adjacent pre-existing line from auto-indentation.
                    match autoindent_exclusion {
                        AutoIndentExclusion::PrecedingLine => {
                            if new_text.starts_with('\n') {
                                range_of_insertion_to_indent.start += 1;
                                first_line_is_new = true;
                            }
                        }
                        AutoIndentExclusion::FollowingLine => {
                            if new_text.ends_with('\n') {
                                range_of_insertion_to_indent.end -= 1;
                                first_line_is_new = true;
                            }
                        }
                    }

                    let mut original_indent_column = None;
                    if let AutoindentMode::Block {
                        original_indent_columns,
                    } = &mode
                    {
                        original_indent_column = Some(if new_text.starts_with('\n') {
                            indent_size_for_text(
                                new_text[range_of_insertion_to_indent.clone()].chars(),
                            )
                            .len
                        } else {
                            original_indent_columns
                                .get(ix)
                                .copied()
                                .flatten()
                                .unwrap_or_else(|| {
                                    indent_size_for_text(
                                        new_text[range_of_insertion_to_indent.clone()].chars(),
                                    )
                                    .len
                                })
                        });

                        // Avoid auto-indenting the line after the edit.
                        if new_text[range_of_insertion_to_indent.clone()].ends_with('\n') {
                            range_of_insertion_to_indent.end -= 1;
                        }
                    }

                    AutoindentRequestEntry {
                        original_indent_column,
                        old_row: if first_line_is_new {
                            None
                        } else {
                            Some(old_start.row)
                        },
                        indent_size: before_edit.language_indent_size_at(range.start, cx),
                        range: self.anchor_before(new_start + range_of_insertion_to_indent.start)
                            ..self.anchor_after(new_start + range_of_insertion_to_indent.end),
                    }
                })
                .collect();

            if !entries.is_empty() {
                self.autoindent_requests.push(Arc::new(AutoindentRequest {
                    before_edit,
                    entries,
                    is_block_mode: matches!(mode, AutoindentMode::Block { .. }),
                    ignore_empty_lines: false,
                }));
            }
        }

        self.end_transaction(cx);
        self.send_operation(Operation::Buffer(edit_operation), true, cx);
        Some(edit_id)
    }

    fn did_edit(
        &mut self,
        old_version: &clock::Global,
        was_dirty: bool,
        source: BufferEditSource,
        cx: &mut Context<Self>,
    ) {
        self.was_changed();

        if self.edits_since::<usize>(old_version).next().is_none() {
            return;
        }

        self.reparse(cx, true);
        cx.emit(BufferEvent::Edited { source });
        let is_dirty = self.is_dirty();
        if was_dirty != is_dirty {
            cx.emit(BufferEvent::DirtyChanged);
        }
        if was_dirty && !is_dirty {
            if let Some(file) = self.file.as_ref() {
                if matches!(file.disk_state(), DiskState::Present { .. })
                    && file.disk_state().mtime() != self.saved_mtime
                {
                    cx.emit(BufferEvent::ReloadNeeded);
                }
            }
        }
        cx.notify();
    }

    pub fn autoindent_ranges<I, T>(&mut self, ranges: I, cx: &mut Context<Self>)
    where
        I: IntoIterator<Item = Range<T>>,
        T: ToOffset + Copy,
    {
        let before_edit = self.snapshot();
        let entries = ranges
            .into_iter()
            .map(|range| AutoindentRequestEntry {
                range: before_edit.anchor_before(range.start)..before_edit.anchor_after(range.end),
                old_row: None,
                indent_size: before_edit.language_indent_size_at(range.start, cx),
                original_indent_column: None,
            })
            .collect();
        self.autoindent_requests.push(Arc::new(AutoindentRequest {
            before_edit,
            entries,
            is_block_mode: false,
            ignore_empty_lines: true,
        }));
        self.request_autoindent(cx, Some(Duration::from_micros(300)));
    }

    // Inserts newlines at the given position to create an empty line, returning the start of the new line.
    // You can also request the insertion of empty lines above and below the line starting at the returned point.
    pub fn insert_empty_line(
        &mut self,
        position: impl ToPoint,
        space_above: bool,
        space_below: bool,
        cx: &mut Context<Self>,
    ) -> Point {
        let mut position = position.to_point(self);

        self.start_transaction();

        self.edit(
            [(position..position, "\n")],
            Some(AutoindentMode::EachLine),
            cx,
        );

        if position.column > 0 {
            position += Point::new(1, 0);
        }

        if !self.is_line_blank(position.row) {
            self.edit(
                [(position..position, "\n")],
                Some(AutoindentMode::EachLine),
                cx,
            );
        }

        if space_above && position.row > 0 && !self.is_line_blank(position.row - 1) {
            self.edit(
                [(position..position, "\n")],
                Some(AutoindentMode::EachLine),
                cx,
            );
            position.row += 1;
        }

        if space_below
            && (position.row == self.max_point().row || !self.is_line_blank(position.row + 1))
        {
            self.edit(
                [(position..position, "\n")],
                Some(AutoindentMode::EachLine),
                cx,
            );
        }

        self.end_transaction(cx);

        position
    }

    /// Applies the given remote operations to the buffer.
    pub fn apply_ops<I: IntoIterator<Item = Operation>>(&mut self, ops: I, cx: &mut Context<Self>) {
        self.pending_autoindent.take();
        let was_dirty = self.is_dirty();
        let old_version = self.version.clone();
        let mut deferred_ops = Vec::new();
        let buffer_ops = ops
            .into_iter()
            .filter_map(|op| match op {
                Operation::Buffer(op) => Some(op),
                _ => {
                    if self.can_apply_op(&op) {
                        self.apply_op(op, cx);
                    } else {
                        deferred_ops.push(op);
                    }
                    None
                }
            })
            .collect::<Vec<_>>();
        for operation in buffer_ops.iter() {
            self.send_operation(Operation::Buffer(operation.clone()), false, cx);
        }
        self.text.apply_ops(buffer_ops);
        self.deferred_ops.insert(deferred_ops);
        self.flush_deferred_ops(cx);
        self.did_edit(&old_version, was_dirty, BufferEditSource::Remote, cx);
        // Notify independently of whether the buffer was edited as the operations could include a
        // selection update.
        cx.notify();
    }

    fn flush_deferred_ops(&mut self, cx: &mut Context<Self>) {
        let mut deferred_ops = Vec::new();
        for op in self.deferred_ops.drain().iter().cloned() {
            if self.can_apply_op(&op) {
                self.apply_op(op, cx);
            } else {
                deferred_ops.push(op);
            }
        }
        self.deferred_ops.insert(deferred_ops);
    }

    pub fn has_deferred_ops(&self) -> bool {
        !self.deferred_ops.is_empty() || self.text.has_deferred_ops()
    }

    fn can_apply_op(&self, operation: &Operation) -> bool {
        match operation {
            Operation::Buffer(_) => {
                unreachable!("buffer operations should never be applied at this layer")
            }
            Operation::UpdateDiagnostics {
                diagnostics: diagnostic_set,
                ..
            } => diagnostic_set.iter().all(|diagnostic| {
                self.text.can_resolve(&diagnostic.range.start)
                    && self.text.can_resolve(&diagnostic.range.end)
            }),
            Operation::UpdateSelections { selections, .. } => selections
                .iter()
                .all(|s| self.can_resolve(&s.start) && self.can_resolve(&s.end)),
            Operation::UpdateCompletionTriggers { .. } | Operation::UpdateLineEnding { .. } => true,
        }
    }

    fn apply_op(&mut self, operation: Operation, cx: &mut Context<Self>) {
        match operation {
            Operation::Buffer(_) => {
                unreachable!("buffer operations should never be applied at this layer")
            }
            Operation::UpdateDiagnostics {
                server_id,
                diagnostics: diagnostic_set,
                lamport_timestamp,
            } => {
                let snapshot = self.snapshot();
                self.apply_diagnostic_update(
                    server_id,
                    DiagnosticSet::from_sorted_entries(diagnostic_set.iter().cloned(), &snapshot),
                    lamport_timestamp,
                    cx,
                );
            }
            Operation::UpdateSelections {
                selections,
                lamport_timestamp,
                line_mode,
                cursor_shape,
            } => {
                if let Some(set) = self.remote_selections.get(&lamport_timestamp.replica_id)
                    && set.lamport_timestamp > lamport_timestamp
                {
                    return;
                }

                self.remote_selections.insert(
                    lamport_timestamp.replica_id,
                    SelectionSet {
                        selections,
                        lamport_timestamp,
                        line_mode,
                        cursor_shape,
                    },
                );
                self.text.lamport_clock.observe(lamport_timestamp);
                self.non_text_state_update_count += 1;
            }
            Operation::UpdateCompletionTriggers {
                triggers,
                lamport_timestamp,
                server_id,
            } => {
                if triggers.is_empty() {
                    self.completion_triggers_per_language_server
                        .remove(&server_id);
                } else {
                    self.completion_triggers_per_language_server
                        .insert(server_id, triggers.into_iter().collect());
                }
                self.completion_triggers = self
                    .completion_triggers_per_language_server
                    .values()
                    .flat_map(|triggers| triggers.iter().cloned())
                    .collect();
                self.text.lamport_clock.observe(lamport_timestamp);
            }
            Operation::UpdateLineEnding {
                line_ending,
                lamport_timestamp,
            } => {
                self.text.set_line_ending(line_ending);
                self.text.lamport_clock.observe(lamport_timestamp);
            }
        }
    }

    fn apply_diagnostic_update(
        &mut self,
        server_id: LanguageServerId,
        diagnostics: DiagnosticSet,
        lamport_timestamp: clock::Lamport,
        cx: &mut Context<Self>,
    ) {
        if lamport_timestamp > self.diagnostics_timestamp {
            if diagnostics.is_empty() {
                self.diagnostics.remove(&server_id);
            } else {
                self.diagnostics.insert(server_id, diagnostics);
            }
            self.diagnostics_timestamp = lamport_timestamp;
            self.non_text_state_update_count += 1;
            self.text.lamport_clock.observe(lamport_timestamp);
            cx.notify();
            cx.emit(BufferEvent::DiagnosticsUpdated);
        }
    }

    fn send_operation(&mut self, operation: Operation, is_local: bool, cx: &mut Context<Self>) {
        self.was_changed();
        cx.emit(BufferEvent::Operation {
            operation,
            is_local,
        });
    }

    /// Removes the selections for a given peer.
    pub fn remove_peer(&mut self, replica_id: ReplicaId, cx: &mut Context<Self>) {
        self.remote_selections.remove(&replica_id);
        cx.notify();
    }

    /// Undoes the most recent transaction.
    pub fn undo(&mut self, cx: &mut Context<Self>) -> Option<TransactionId> {
        let was_dirty = self.is_dirty();
        let old_version = self.version.clone();

        if let Some((transaction_id, operation)) = self.text.undo() {
            self.send_operation(Operation::Buffer(operation), true, cx);
            self.did_edit(&old_version, was_dirty, BufferEditSource::User, cx);
            self.restore_encoding_for_transaction(transaction_id, was_dirty);
            Some(transaction_id)
        } else {
            None
        }
    }

    /// Manually undoes a specific transaction in the buffer's undo history.
    pub fn undo_transaction(
        &mut self,
        transaction_id: TransactionId,
        cx: &mut Context<Self>,
    ) -> bool {
        let was_dirty = self.is_dirty();
        let old_version = self.version.clone();
        if let Some(operation) = self.text.undo_transaction(transaction_id) {
            self.send_operation(Operation::Buffer(operation), true, cx);
            self.did_edit(&old_version, was_dirty, BufferEditSource::User, cx);
            true
        } else {
            false
        }
    }

    /// Manually undoes all changes after a given transaction in the buffer's undo history.
    pub fn undo_to_transaction(
        &mut self,
        transaction_id: TransactionId,
        cx: &mut Context<Self>,
    ) -> bool {
        let was_dirty = self.is_dirty();
        let old_version = self.version.clone();

        let operations = self.text.undo_to_transaction(transaction_id);
        let undone = !operations.is_empty();
        for operation in operations {
            self.send_operation(Operation::Buffer(operation), true, cx);
        }
        if undone {
            self.did_edit(&old_version, was_dirty, BufferEditSource::User, cx)
        }
        undone
    }

    pub fn undo_operations(&mut self, counts: HashMap<Lamport, u32>, cx: &mut Context<Buffer>) {
        let was_dirty = self.is_dirty();
        let operation = self.text.undo_operations(counts);
        let old_version = self.version.clone();
        self.send_operation(Operation::Buffer(operation), true, cx);
        self.did_edit(&old_version, was_dirty, BufferEditSource::User, cx);
    }

    /// Manually redoes a specific transaction in the buffer's redo history.
    pub fn redo(&mut self, cx: &mut Context<Self>) -> Option<TransactionId> {
        let was_dirty = self.is_dirty();
        let old_version = self.version.clone();

        if let Some((transaction_id, operation)) = self.text.redo() {
            self.send_operation(Operation::Buffer(operation), true, cx);
            self.did_edit(&old_version, was_dirty, BufferEditSource::User, cx);
            self.restore_encoding_for_transaction(transaction_id, was_dirty);
            Some(transaction_id)
        } else {
            None
        }
    }

    fn restore_encoding_for_transaction(&mut self, transaction_id: TransactionId, was_dirty: bool) {
        if let Some((old_encoding, old_has_bom)) =
            self.reload_with_encoding_txns.get(&transaction_id)
        {
            let current_encoding = self.encoding;
            let current_has_bom = self.has_bom;
            self.encoding = *old_encoding;
            self.has_bom = *old_has_bom;
            if !was_dirty {
                self.saved_version = self.version.clone();
                self.has_unsaved_edits
                    .set((self.saved_version.clone(), false));
            }
            self.reload_with_encoding_txns
                .insert(transaction_id, (current_encoding, current_has_bom));
        }
    }

    /// Manually undoes all changes until a given transaction in the buffer's redo history.
    pub fn redo_to_transaction(
        &mut self,
        transaction_id: TransactionId,
        cx: &mut Context<Self>,
    ) -> bool {
        let was_dirty = self.is_dirty();
        let old_version = self.version.clone();

        let operations = self.text.redo_to_transaction(transaction_id);
        let redone = !operations.is_empty();
        for operation in operations {
            self.send_operation(Operation::Buffer(operation), true, cx);
        }
        if redone {
            self.did_edit(&old_version, was_dirty, BufferEditSource::User, cx)
        }
        redone
    }

    /// Override current completion triggers with the user-provided completion triggers.
    pub fn set_completion_triggers(
        &mut self,
        server_id: LanguageServerId,
        triggers: BTreeSet<String>,
        cx: &mut Context<Self>,
    ) {
        self.completion_triggers_timestamp = self.text.lamport_clock.tick();
        if triggers.is_empty() {
            self.completion_triggers_per_language_server
                .remove(&server_id);
        } else {
            self.completion_triggers_per_language_server
                .insert(server_id, triggers.clone());
        }
        self.completion_triggers = self
            .completion_triggers_per_language_server
            .values()
            .flat_map(|triggers| triggers.iter().cloned())
            .collect();
        self.send_operation(
            Operation::UpdateCompletionTriggers {
                triggers: triggers.into_iter().collect(),
                lamport_timestamp: self.completion_triggers_timestamp,
                server_id,
            },
            true,
            cx,
        );
        cx.notify();
    }

    /// Returns a list of strings which trigger a completion menu for this language.
    /// Usually this is driven by LSP server which returns a list of trigger characters for completions.
    pub fn completion_triggers(&self) -> &BTreeSet<String> { &self.completion_triggers }

    /// Call this directly after performing edits to prevent the preview tab
    /// from being dismissed by those edits. It causes `should_dismiss_preview`
    /// to return false until there are additional edits.
    pub fn refresh_preview(&mut self) { self.preview_version = self.version.clone(); }

    /// Whether we should preserve the preview status of a tab containing this buffer.
    pub fn preserve_preview(&self) -> bool { !self.has_edits_since(&self.preview_version) }

    pub fn set_group_interval(&mut self, group_interval: Duration) {
        self.text.set_group_interval(group_interval);
    }

    // TODO: see if ep can use this instead of Buffer::branch
    pub fn snapshot_with_edits<I, S, T>(
        &mut self,
        edits: I,
        cx: &mut Context<Self>,
    ) -> Task<EditedBufferSnapshot>
    where
        I: IntoIterator<Item = (Range<S>, T)>,
        S: ToOffset,
        T: Into<Arc<str>>,
    {
        let mut snapshot = self.snapshot();
        let text = snapshot.text.clone();
        let mut syntax = snapshot.syntax.clone();
        let language = self.language().cloned();
        let registry = self.language_registry();
        let new_text = self.text.snapshot_with_edits(edits);
        cx.background_spawn(async move {
            if let Some(language) = language.clone() {
                syntax.reparse(&text, registry.clone(), language);
            }

            syntax.interpolate(&new_text.snapshot);

            if let Some(language) = language {
                syntax.reparse(&new_text.snapshot, registry, language);
            }

            snapshot.text = new_text.snapshot.clone();
            snapshot.syntax = syntax;
            snapshot.tree_sitter_data = Arc::new(TreeSitterData::new(&snapshot.text));

            EditedBufferSnapshot {
                text: new_text,
                snapshot,
            }
        })
    }

    pub fn fast_forward(&mut self, edited: EditedBufferSnapshot, cx: &mut Context<Self>) {
        let base_version = edited.text.base_version.clone();
        let did_edit = edited.text.did_edit;
        self.text.fast_forward(edited.text);
        if edited.snapshot.language == self.language {
            self.reparse = None;
            self.did_finish_parsing(edited.snapshot.syntax, None, false, cx);
            if did_edit {
                cx.emit(BufferEvent::Edited {
                    source: BufferEditSource::User,
                });
            }
        } else {
            self.did_edit(&base_version, false, BufferEditSource::User, cx);
        }
    }
}

impl EventEmitter<BufferEvent> for Buffer {}

impl Deref for Buffer {
    type Target = TextBuffer;

    fn deref(&self) -> &Self::Target { &self.text }
}

#[cfg(any(test, feature = "test-support"))]
impl Buffer {
    /* edit_via_marked_text / randomly_edit / randomly_undo_redo 原样 */
}

mod entry;
pub use crate::entry::Entry;

/// A set of local or remote files that are being opened as part of a project.
/// Responsible for tracking related FS (for local)/collab (for remote) events and corresponding updates.
/// Stores git repositories data and the diagnostics for the file(s).
///
/// Has an absolute path, and may be set to be visible in Zed UI or not.
/// May correspond to a directory or a single file.
/// Possible examples:
/// * a drag and dropped file — may be added as an invisible, "ephemeral" entry to the current worktree
/// * a directory opened in Zed — may be added as a visible entry to the current worktree
///
/// Uses [`Entry`] to track the state of each file/directory, can look up absolute paths for entries.
pub enum Worktree {
    Local(LocalWorktree),
    Remote(RemoteWorktree),
}

pub struct LocalWorktree {
    snapshot: LocalSnapshot,
    scan_requests_tx: async_channel::Sender<ScanRequest>,
    path_prefixes_to_scan_tx: async_channel::Sender<PathPrefixScanRequest>,
    is_scanning: (watch::Sender<bool>, watch::Receiver<bool>),
    snapshot_subscriptions: VecDeque<(usize, oneshot::Sender<()>)>,
    _background_scanner_tasks: Vec<Task<()>>,
    update_observer: Option<UpdateObservationState>,
    fs: Arc<dyn Fs>,
    fs_case_sensitive: bool,
    visible: bool,
    next_entry_id: Arc<AtomicUsize>,
    settings: WorktreeSettings,
    share_private_files: bool,
    scanning_enabled: bool,
    force_defer_watch: bool,
}

pub struct RemoteWorktree {
    snapshot: Snapshot,
    background_snapshot: Arc<Mutex<(Snapshot, Vec<proto::UpdateWorktree>)>>,
    project_id: u64,
    client: AnyProtoClient,
    file_scan_inclusions: PathMatcher,
    updates_tx: Option<UnboundedSender<proto::UpdateWorktree>>,
    update_observer: Option<mpsc::UnboundedSender<proto::UpdateWorktree>>,
    snapshot_subscriptions: VecDeque<(usize, oneshot::Sender<()>)>,
    replica_id: ReplicaId,
    visible: bool,
    disconnected: bool,
    received_initial_update: bool,
}

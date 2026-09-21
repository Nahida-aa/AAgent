use aa_gpui_kit_theme::Theme;
use anyhow::{Context as _, Result, anyhow};
use collections::{FxHashMap, HashMap, HashSet, hash_map};
use futures::{
    Future,
    channel::{mpsc, oneshot},
    future::{BoxFuture, FutureExt as _},
};
use gpui::BackgroundExecutor;
use parking_lot::{Mutex, RwLock};
use postage::watch;
use std::{path::Path, sync::Arc};

pub struct LanguageRegistry {
    state: RwLock<LanguageRegistryState>,
    language_server_download_dir: Option<Arc<Path>>,
    executor: BackgroundExecutor,
    lsp_binary_status_tx: ServerStatusSender,
}

struct LanguageRegistryState {
    next_language_server_id: usize,
    languages: Vec<Arc<Language>>,
    language_settings: AllLanguageSettingsContent,
    available_languages: AvailableLanguages,
    grammars: HashMap<Arc<str>, AvailableGrammar>,
    lsp_adapters: HashMap<LanguageName, Vec<Arc<CachedLspAdapter>>>,
    all_lsp_adapters: HashMap<LanguageServerName, Arc<CachedLspAdapter>>,
    available_lsp_adapters:
        HashMap<LanguageServerName, Arc<dyn Fn() -> Arc<CachedLspAdapter> + 'static + Send + Sync>>,
    loading_languages: HashMap<LanguageId, Vec<oneshot::Sender<Result<Arc<Language>>>>>,
    subscription: (watch::Sender<()>, watch::Receiver<()>),
    theme: Option<Arc<Theme>>,
    version: usize,
    reload_count: usize,

    #[cfg(any(test, feature = "test-support"))]
    fake_server_entries: HashMap<LanguageServerName, FakeLanguageServerEntry>,
}

#[derive(Clone, Default)]
struct ServerStatusSender {
    state: Arc<Mutex<ServerStatusSenderState>>,
}

#[derive(Default)]
struct ServerStatusSenderState {
    next_subscription_id: usize,
    txs: HashMap<usize, mpsc::UnboundedSender<ServerStatus>>,
}

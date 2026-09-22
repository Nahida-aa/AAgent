use super::*;

pub struct HoverBlock {
    pub text: String,
    pub kind: HoverBlockKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HoverBlockKind {
    PlainText,
    Markdown,
    Code { language: String },
}

#[derive(Debug, Clone)]
pub struct Hover {
    pub contents: Vec<HoverBlock>,
    pub range: Option<Range<language::Anchor>>,
    pub language: Option<Arc<Language>>,
}

impl Hover {
    pub fn is_empty(&self) -> bool {
        self.contents.iter().all(|block| block.text.is_empty())
    }
}

enum EntitySubscription {
    Project(PendingEntitySubscription<Project>),
    BufferStore(PendingEntitySubscription<BufferStore>),
    GitStore(PendingEntitySubscription<GitStore>),
    WorktreeStore(PendingEntitySubscription<WorktreeStore>),
    LspStore(PendingEntitySubscription<LspStore>),
    SettingsObserver(PendingEntitySubscription<SettingsObserver>),
    DapStore(PendingEntitySubscription<DapStore>),
    BreakpointStore(PendingEntitySubscription<BreakpointStore>),
}

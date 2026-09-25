// 主语是“按键分发状态”，是 Workspace 自身字段类型。

// 服务 send_keystrokes / send_keystrokes_impl，是 Workspace 级输入机制，不属于 item/pane/dock/action 领域。

// 和 DelayedDebouncedEditAction 一样是 Workspace 的辅助状态类型，放 core/ 平级

#[derive(Default)]
pub(crate) struct DispatchingKeystrokes {
    dispatched: HashSet<Vec<Keystroke>>,
    queue: VecDeque<Keystroke>,
    task: Option<Shared<Task<()>>>,
}

//! 窗口栈恢复 —— 「上次开着哪些窗口，这次 reopen」。
//!
//! 注意别和 [`aa_session`] 搞混，两个 crate 都带 session：
//! - **`session`（本 crate）** = zed `crates/session` 的移植，只管窗口栈的持久化/恢复，
//!   跟对话内容、agent 轮次**完全无关**。
//! - [`aa_session`]（`packages/aa_session`，crate 名 `aa-session`）= aacode 自己的对话轮次，
//!   定义 `SessionEvent`（Token/ToolCall/ToolResult/Done/Error）、`TurnInput`、`run_turn()`。
//!
//! 移植说明：上游文件是 `crates/session/src/session.rs`，按 `.agents/zed-port.md` 里
//! 「单文件 crate 拆成 `src/lib.rs`」的约定落到本仓库；除格式化外与上游无逻辑差异，
//! 所以**在 zed 侧更新这个 crate 时可以直接 diff `src/session.rs`**。
//!
//! 数据落在 KVP（`db::kvp::KeyValueStore`）里，两个 key：
//! - `session_id` —— 上一次运行的 session id
//! - `session_window_stack` —— 上一次运行的窗口 id 列表（JSON 数组）
//!
//! 「记录上一次」的实现关键是**先读后写**：读出旧值存进 `old_*`，再把新值盖上去。

use db::kvp::KeyValueStore;
use gpui::{App, AppContext as _, Context, Subscription, Task, WindowId};
use util::ResultExt;

/// 一次运行的会话身份 + 上一次运行的快照。
///
/// 三个字段分两组：`session_id` 是本次的，`old_*` 是从 KVP 读出来的**上一次**的值，
/// 供「恢复上次窗口」用。所有读取失败都静默降级成 `None`（`.ok().flatten()`），
/// 读不到不影响本次运行。
pub struct Session {
    /// 本次运行的 id，由调用方生成后传进来。
    session_id: String,
    /// 上一次运行的 id（KVP 里存的值），用来判断「是不是重开的同一个 app」。
    old_session_id: Option<String>,
    /// 上一次运行结束时的窗口栈，用来 reopen 窗口。
    old_window_ids: Option<Vec<WindowId>>,
}

const SESSION_ID_KEY: &str = "session_id";
const SESSION_WINDOW_STACK_KEY: &str = "session_window_stack";

impl Session {
    /// 从 KVP 里读出「上一次」的 id 和窗口栈，然后立刻把本次的 `session_id` 写回去。
    pub async fn new(session_id: String, db: KeyValueStore) -> Self {
        // 先读：拿到的是上一次的值
        let old_session_id = db.read_kvp(SESSION_ID_KEY).ok().flatten();

        // 后写：覆盖成这一次。写失败只 log 不 panic —— 恢复功能不该拖垮启动
        db.write_kvp(SESSION_ID_KEY.to_string(), session_id.clone())
            .await
            .log_err();

        // 窗口栈存成 JSON 数组（`Vec<u64>`），读回来再转成 `Vec<WindowId>`。
        // `WindowId` 本身不是 serde 可序列化的，这是绕它一圈的原因
        let old_window_ids = db
            .read_kvp(SESSION_WINDOW_STACK_KEY)
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str::<Vec<u64>>(&json).ok())
            .map(|vec: Vec<u64>| {
                vec.into_iter()
                    .map(WindowId::from)
                    .collect::<Vec<WindowId>>()
            });

        Self {
            session_id,
            old_session_id,
            old_window_ids,
        }
    }

    /// 测试用：全新 session，没有「上一次」。
    #[cfg(any(test, feature = "test-support"))]
    pub fn test() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            old_session_id: None,
            old_window_ids: None,
        }
    }

    /// 测试用：伪造一个「上一次」，用来验证恢复逻辑。
    #[cfg(any(test, feature = "test-support"))]
    pub fn test_with_old_session(old_session_id: String) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            old_session_id: Some(old_session_id),
            old_window_ids: None,
        }
    }

    pub fn id(&self) -> &str { &self.session_id }
}

/// 给 GPUI 用的包装：把 [`Session`] 变成一个 entity，并接管两件事——
///
/// 1. **周期持久化**：每 500ms 看一眼窗口栈，变了就写 KVP。
/// 2. **退出兜底**：`on_app_quit` 时再存一次，避免 500ms 窗口内的改动丢掉。
///
/// 之所以要有这个包装而不是裸用 `Session`：窗口栈只有 `&App` 能读，而要拿到
/// `&App` 就得挂进 GPUI 的 entity 生命周期里；同时 GPUI 也没有「窗口栈变了」的
/// 事件，只能自己轮询。
pub struct AppSession {
    session: Session,
    /// 那个 500ms 轮询循环的 handle。字段名下划线前缀 = 只为持有而不读。
    /// 必须存着：drop 掉就等于把 task 取消，轮询立刻停。
    _serialization_task: Task<()>,
    /// 同上，`on_app_quit` 的订阅句柄，drop 就退订了。
    _subscriptions: Vec<Subscription>,
}

impl AppSession {
    /// `cx.on_app_quit` 挂退出钩子，并起一个后台 task 做周期性持久化。
    pub fn new(session: Session, cx: &Context<Self>) -> Self {
        let _subscriptions = vec![cx.on_app_quit(Self::app_will_quit)];

        let _serialization_task = if cfg!(not(any(test, feature = "test-support"))) {
            let db = KeyValueStore::global(cx);
            cx.spawn(async move |_, cx| {
                // Disabled in tests: the infinite loop bypasses "parking forbidden" checks,
                // causing tests to hang instead of panicking.
                {
                    // 上一次已落盘的值，用来去重：栈没变就不重复写
                    let mut current_window_stack = Vec::new();
                    loop {
                        if let Some(windows) = cx.update(|cx| window_stack(cx))
                            && !windows.is_empty()
                            && windows != current_window_stack
                        {
                            store_window_stack(db.clone(), &windows).await;
                            current_window_stack = windows;
                        }

                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(500))
                            .await;
                    }
                }
            })
        } else {
            // 测试环境下不启轮询，否则那个死循环会让测试挂住而不是报错
            Task::ready(())
        };

        Self {
            session,
            _subscriptions,
            _serialization_task,
        }
    }

    /// 退出时的兜底落盘。轮询是 500ms 粒度，最后这 500ms 内的窗口变化只能靠这里补上。
    /// 用 `background_spawn` 而不是 `spawn`：退出流程要等这个 task 完成，不能被前台更新挡住。
    fn app_will_quit(&mut self, cx: &mut Context<Self>) -> Task<()> {
        if let Some(window_stack) = window_stack(cx)
            && !window_stack.is_empty()
        {
            let db = KeyValueStore::global(cx);
            cx.background_spawn(async move { store_window_stack(db, &window_stack).await })
        } else {
            Task::ready(())
        }
    }

    pub fn id(&self) -> &str { self.session.id() }

    /// 上一次运行的 id，`None` 表示这是第一次运行（KVP 里还没有）。
    pub fn last_session_id(&self) -> Option<&str> { self.session.old_session_id.as_deref() }

    #[cfg(any(test, feature = "test-support"))]
    pub fn replace_session_for_test(&mut self, session: Session) { self.session = session; }

    /// 上一次运行的窗口栈，调用方据此 reopen 窗口。`None` = 没有上次，或读失败。
    pub fn last_session_window_stack(&self) -> Option<Vec<WindowId>> {
        self.session.old_window_ids.clone()
    }
}

/// 当前所有窗口的 id，按栈序（`cx.window_stack()` 的顺序 = z-order）。
/// 用 `&App` 而非 `Context<Self>`：这个 helper 要能在 `cx.update` 闭包里和退出钩子里复用。
fn window_stack(cx: &App) -> Option<Vec<u64>> {
    Some(
        cx.window_stack()?
            .into_iter()
            .map(|window| window.window_id().as_u64())
            .collect(),
    )
}

/// 把窗口栈序列化成 JSON 写进 KVP。失败只 log —— 存不进去不该影响退出。
async fn store_window_stack(db: KeyValueStore, windows: &[u64]) {
    if let Ok(window_ids_json) = serde_json::to_string(windows) {
        db.write_kvp(SESSION_WINDOW_STACK_KEY.to_string(), window_ids_json)
            .await
            .log_err();
    }
}

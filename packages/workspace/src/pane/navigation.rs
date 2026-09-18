//! Navigation history — "Go Back / Go Forward" 在 tab 间的切换。
//!
//! Zed 的 NavHistory 很复杂：
//! - NavHistoryState { backward_stack, forward_stack, closed_stack, tag_stack }
//! - 每个 Item 有自己的 ItemNavHistory（光标位置级别的导航）
//! - NavigationMode（History 还是 Tags）
//!
//! AAgent 简化版：在 Pane 级别维护一个 tab 激活历史的双向栈，
//! 让 GoBack / GoForward 在最近激活过的 tab 之间切换。
//! 不实现 item 内部的光标位置导航（等有 Editor 再说）。

use gpui::EntityId;
use std::collections::VecDeque;

/// 一次导航记录。
#[derive(Debug, Clone)]
pub struct NavigationEntry {
    pub entity_id: EntityId,
}

/// Tab 级别的导航历史 — GoBack / GoForward。
///
/// 每次 activate_item 时 push 到 forward_stack 之前，
/// GoBack 从 forward_stack pop 回 backward_stack，
/// GoForward 反过来。
///
/// 跟 ActivationHistory 的区别：
/// - ActivationHistory: "最近激活优先" 排序，实现 ActivateLastItem
/// - NavHistory: 严格的 back/forward 栈，实现 GoBack/GoForward
pub struct NavHistory {
    /// 已经离开的 tab，按 "最近离开的在尾部" 排序。
    backward_stack: VecDeque<NavigationEntry>,
    /// 可以前进回去的 tab，按 "下一个要回去的在尾部" 排序。
    forward_stack: VecDeque<NavigationEntry>,
    /// 最近关闭的 tab（ReopenClosedItem 用）。
    closed_stack: VecDeque<NavigationEntry>,
}

const MAX_STACK_LEN: usize = 50;

impl NavHistory {
    pub fn new() -> Self {
        Self {
            backward_stack: VecDeque::new(),
            forward_stack: VecDeque::new(),
            closed_stack: VecDeque::new(),
        }
    }

    /// 激活新 item 时调 — 把当前 active push 到 backward，清空 forward。
    /// 对齐 Zed NavHistory::push_navigation。
    pub fn record_navigation(&mut self, from: EntityId, to: EntityId) {
        self.backward_stack.push_back(NavigationEntry { entity_id: from });
        self.backward_stack.truncate(MAX_STACK_LEN);
        // 用户手动导航后 forward stack 失效（浏览器行为）
        self.forward_stack.clear();
        // 如果目标已经在 backward 里（用户走了个圈回来），不重复记录
        let _ = to;
    }

    /// 关闭 item 时调 — 塞进 closed stack（用于 ReopenClosedItem）。
    pub fn record_closed(&mut self, entity_id: EntityId) {
        self.closed_stack.push_back(NavigationEntry { entity_id });
        self.closed_stack.truncate(MAX_STACK_LEN);
    }

    /// GoBack — 从 backward pop，push 到 forward。返回要激活的 EntityId。
    pub fn go_back(&mut self) -> Option<EntityId> {
        let entry = self.backward_stack.pop_back()?;
        self.forward_stack.push_back(entry.clone());
        Some(entry.entity_id)
    }

    /// GoForward — 从 forward pop，push 到 backward。
    pub fn go_forward(&mut self) -> Option<EntityId> {
        let entry = self.forward_stack.pop_back()?;
        self.backward_stack.push_back(entry.clone());
        Some(entry.entity_id)
    }

    /// ReopenClosedItem — 从 closed stack pop。
    pub fn pop_closed(&mut self) -> Option<EntityId> {
        self.closed_stack.pop_back().map(|e| e.entity_id)
    }

    pub fn can_go_back(&self) -> bool {
        !self.backward_stack.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.backward_stack.clear();
        self.forward_stack.clear();
        self.closed_stack.clear();
    }
}

impl Default for NavHistory {
    fn default() -> Self {
        Self::new()
    }
}

//! Navigation history — "Go Back / Go Forward" 在 item 间的切换。
//!
//! 对齐 Zed `NavHistoryState` + `NavigationEntry` 的核心子集。
//!
//! Zed 完整结构:
//! ```ignore
//! pub struct NavigationEntry {
//!     pub item: Arc<dyn WeakItemHandle + Send + Sync>,
//!     pub data: Option<Arc<dyn Any + Send + Sync>>,
//!     pub timestamp: usize,
//!     pub is_preview: bool,
//!     pub row: Option<u32>,  // Neovim-style dedup
//! }
//! ```
//!
//! AAgent 裁剪:
//! - `WeakItemHandle` → `EntityId`（没有 Send/Sync，GPUI 主线程）
//! - `data` → `Option<()>` 占位（以后 Editor 存光标位置）
//! - `timestamp` 保留，NavHistory 内部自增计数器
//! - `is_preview` / `row` 保留

use gpui::EntityId;
use std::collections::VecDeque;

/// 一次导航记录。对齐 Zed NavigationEntry。
#[derive(Debug, Clone)]
pub struct NavigationEntry {
    pub entity_id: EntityId,
    /// item 内部的恢复数据（比如光标位置、scroll 位置）。
    /// 类型擦除为 () 占位，等有 Editor 时换成 `Arc<dyn Any>`。
    pub data: Option<()>,
    /// 单调递增的时间戳 — Zed 用 `Arc<AtomicUsize>` 从全局计数器拿，
    /// AAgent 简化为 NavHistory 内部自增。
    pub timestamp: usize,
    /// 是否 preview item（Zed 有 "preview tab" 概念）。
    pub is_preview: bool,
    /// Neovim 风格去重 row — 同一 item + 同一 row 的导航会被合并。
    pub row: Option<u32>,
}

/// NavHistory — 维护 backward/forward/closed 三个栈。
///
/// 对齐 Zed NavHistoryState 的核心字段:
/// - backward_stack: 已经离开的导航记录
/// - forward_stack: 可以前进回去的记录
/// - closed_stack: 最近关闭的 item（ReopenClosedItem 用）
///
/// 跟 ActivationHistory 的区别:
/// - ActivationHistory: 扁平 "最近激活优先" 排序，实现 ActivateLastItem
/// - NavHistory: 严格 back/forward 双向栈，实现 GoBack/GoForward
pub struct NavHistory {
    backward_stack: VecDeque<NavigationEntry>,
    forward_stack: VecDeque<NavigationEntry>,
    closed_stack: VecDeque<NavigationEntry>,
    next_timestamp: usize,
}

const MAX_STACK_LEN: usize = 50;

impl NavHistory {
    pub fn new() -> Self {
        Self {
            backward_stack: VecDeque::new(),
            forward_stack: VecDeque::new(),
            closed_stack: VecDeque::new(),
            next_timestamp: 0,
        }
    }

    fn next_timestamp(&mut self) -> usize {
        let ts = self.next_timestamp;
        self.next_timestamp += 1;
        ts
    }

    /// 激活新 item 时调 — 把当前 active push 到 backward，清空 forward。
    /// 对齐 Zed NavHistory::push_navigation。
    pub fn record_navigation(&mut self, from: EntityId, to: EntityId) {
        let ts = self.next_timestamp();
        // 去重：如果 backward 尾部已经是同一个 item（走了个小圈），更新 timestamp 就行。
        if let Some(last) = self.backward_stack.back_mut() {
            if last.entity_id == from {
                last.timestamp = ts;
                self.forward_stack.clear();
                let _ = to;
                return;
            }
        }
        self.backward_stack.push_back(NavigationEntry {
            entity_id: from,
            data: None,
            timestamp: ts,
            is_preview: false,
            row: None,
        });
        self.backward_stack.truncate(MAX_STACK_LEN);
        // 用户手动导航后 forward stack 失效（浏览器行为）
        self.forward_stack.clear();
        let _ = to;
    }

    /// 关闭 item 时调 — 塞进 closed stack（用于 ReopenClosedItem）。
    pub fn record_closed(&mut self, entity_id: EntityId) {
        let ts = self.next_timestamp();
        self.closed_stack.push_back(NavigationEntry {
            entity_id,
            data: None,
            timestamp: ts,
            is_preview: false,
            row: None,
        });
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

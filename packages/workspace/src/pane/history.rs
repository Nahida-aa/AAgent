//! Activation history — 记录 item 的激活顺序，支持 "activate last"。
//!
//! Zed Pane 里有 `activation_history: Vec<ActivationHistoryEntry>` +
//! `next_activation_timestamp: Arc<AtomicUsize>`，每次 active item 变化
//! 时把 (entity_id, timestamp) push 进去，然后 "ActivateLastItem"
//! 就是按 timestamp 找最近激活过的那个。
//!
//! AAgent 简化版：存一份 EntityId 按 "最近激活优先" 排序的列表，
//! 不做持久化、不做 timestamp。

use gpui::EntityId;

/// 一次激活记录。
#[derive(Debug, Clone)]
pub struct ActivationHistoryEntry {
    pub entity_id: EntityId,
}

/// Activation history 的简化实现。
///
/// 维护一个 Vec<EntityId>，最近激活的排在最前。
/// 上限 MAX_HISTORY_LEN，超过时丢弃最老的。
pub struct ActivationHistory {
    entries: Vec<ActivationHistoryEntry>,
}

const MAX_HISTORY_LEN: usize = 20;

impl ActivationHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// 记录一次激活。如果已经在列表里，先移除旧位置再 push 到头部。
    pub fn record_activation(&mut self, entity_id: EntityId) {
        self.entries.retain(|e| e.entity_id != entity_id);
        self.entries.insert(0, ActivationHistoryEntry { entity_id });
        self.entries.truncate(MAX_HISTORY_LEN);
    }

    /// 返回除了 `exclude` 之外最近激活的 item。
    pub fn most_recent_excluding(&self, exclude: EntityId) -> Option<EntityId> {
        self.entries
            .iter()
            .find(|e| e.entity_id != exclude)
            .map(|e| e.entity_id)
    }

    /// 移除某个 item 的历史记录。
    pub fn remove(&mut self, entity_id: EntityId) {
        self.entries.retain(|e| e.entity_id != entity_id);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ActivationHistory {
    fn default() -> Self {
        Self::new()
    }
}

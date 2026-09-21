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
use gpui::{App, Context, FocusOutEvent, Window};
use project::Project;
use settings::Settings;
use std::collections::VecDeque;
use theme_settings::ThemeSettings;

use super::Pane;
use super::history::{NavigationMode, TagNavigationMode};
use crate::item::{ItemSettings, PreviewTabsSettings};
use crate::workspace_settings::{FocusFollowsMouse, TabBarSettings, WorkspaceSettings};

impl Pane {
    pub fn activate_item(
        &mut self,
        index: usize,
        activate_pane: bool,
        focus_item: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use NavigationMode::{GoingBack, GoingForward};
        if index < self.items.len() {
            let prev_active_item_ix = mem::replace(&mut self.active_item_index, index);
            if (prev_active_item_ix != self.active_item_index
                || matches!(self.nav_history.mode(), GoingBack | GoingForward))
                && let Some(prev_item) = self.items.get(prev_active_item_ix)
            {
                prev_item.deactivated(window, cx);
            }
            self.update_history(index);
            self.update_toolbar(window, cx);
            self.update_status_bar(window, cx);

            if focus_item {
                self.focus_active_item(window, cx);
            }

            cx.emit(Event::ActivateItem {
                local: activate_pane,
                focus_changed: focus_item,
            });

            self.update_active_tab(index);
            cx.notify();
        }
    }

    pub(super) fn update_active_tab(&mut self, index: usize) {
        if !self.is_tab_pinned(index) {
            self.suppress_scroll = false;
            self.tab_bar_scroll_handle
                .scroll_to_item(index - self.pinned_tab_count);
        }
    }

    pub(super) fn update_history(&mut self, index: usize) {
        if let Some(newly_active_item) = self.items.get(index) {
            self.activation_history
                .retain(|entry| entry.entity_id != newly_active_item.item_id());
            self.activation_history.push(ActivationHistoryEntry {
                entity_id: newly_active_item.item_id(),
                timestamp: self
                    .next_activation_timestamp
                    .fetch_add(1, Ordering::SeqCst),
            });
        }
    }

    pub fn activate_previous_item(
        &mut self,
        action: &ActivatePreviousItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut index = self.active_item_index;
        if index > 0 {
            index -= 1;
        } else if action.wrap_around && !self.items.is_empty() {
            index = self.items.len() - 1;
        }
        self.activate_item(index, true, true, window, cx);
    }

    pub fn activate_next_item(
        &mut self,
        action: &ActivateNextItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut index = self.active_item_index;
        if index + 1 < self.items.len() {
            index += 1;
        } else if action.wrap_around {
            index = 0;
        }
        self.activate_item(index, true, true, window, cx);
    }

    pub fn swap_item_left(
        &mut self,
        _: &SwapItemLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = self.active_item_index;
        if index == 0 {
            return;
        }

        self.items.swap(index, index - 1);
        self.activate_item(index - 1, true, true, window, cx);
    }

    pub fn swap_item_right(
        &mut self,
        _: &SwapItemRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = self.active_item_index;
        if index + 1 >= self.items.len() {
            return;
        }

        self.items.swap(index, index + 1);
        self.activate_item(index + 1, true, true, window, cx);
    }

    pub fn activate_last_item(
        &mut self,
        _: &ActivateLastItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = self.items.len().saturating_sub(1);
        self.activate_item(index, true, true, window, cx);
    }
    pub fn navigate_backward(&mut self, _: &GoBack, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspace.upgrade() {
            let pane = cx.entity().downgrade();
            window.defer(cx, move |window, cx| {
                workspace.update(cx, |workspace, cx| {
                    workspace.go_back(pane, window, cx).detach_and_log_err(cx)
                })
            })
        }
    }

    pub(super) fn navigate_forward(
        &mut self,
        _: &GoForward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(workspace) = self.workspace.upgrade() {
            let pane = cx.entity().downgrade();
            window.defer(cx, move |window, cx| {
                workspace.update(cx, |workspace, cx| {
                    workspace
                        .go_forward(pane, window, cx)
                        .detach_and_log_err(cx)
                })
            })
        }
    }

    pub fn go_to_older_tag(
        &mut self,
        _: &GoToOlderTag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(workspace) = self.workspace.upgrade() {
            let pane = cx.entity().downgrade();
            window.defer(cx, move |window, cx| {
                workspace.update(cx, |workspace, cx| {
                    workspace
                        .navigate_tag_history(pane, TagNavigationMode::Older, window, cx)
                        .detach_and_log_err(cx)
                })
            })
        }
    }

    pub fn go_to_newer_tag(
        &mut self,
        _: &GoToNewerTag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(workspace) = self.workspace.upgrade() {
            let pane = cx.entity().downgrade();
            window.defer(cx, move |window, cx| {
                workspace.update(cx, |workspace, cx| {
                    workspace
                        .navigate_tag_history(pane, TagNavigationMode::Newer, window, cx)
                        .detach_and_log_err(cx)
                })
            })
        }
    }

    pub(super) fn history_updated(&mut self, cx: &mut Context<Self>) {
        self.toolbar.update(cx, |_, cx| cx.notify());
    }

    pub fn alternate_file(
        &mut self,
        _: &AlternateFile,
        window: &mut Window,
        cx: &mut Context<Pane>,
    ) {
        let (_, alternative) = &self.alternate_file_items;
        if let Some(alternative) = alternative {
            let existing = self
                .items()
                .find_position(|item| item.item_id() == alternative.id());
            if let Some((ix, _)) = existing {
                self.activate_item(ix, true, true, window, cx);
            } else if let Some(upgraded) = alternative.upgrade() {
                self.add_item(upgraded, true, true, None, window, cx);
            }
        }
    }

    pub fn track_alternate_file_items(&mut self) {
        if let Some(item) = self.active_item().map(|item| item.downgrade_item()) {
            let (current, _) = &self.alternate_file_items;
            match current {
                Some(current) => {
                    if current.id() != item.id() {
                        self.alternate_file_items =
                            (Some(item), self.alternate_file_items.0.take());
                    }
                }
                None => {
                    self.alternate_file_items = (Some(item), None);
                }
            }
        }
    }
    pub(super) fn update_toolbar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active_item = self
            .items
            .get(self.active_item_index)
            .map(|item| item.as_ref());
        self.toolbar.update(cx, |toolbar, cx| {
            toolbar.set_active_item(active_item, window, cx);
        });
    }

    pub(super) fn update_status_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = self.workspace.clone();
        let pane = cx.entity();

        window.defer(cx, move |window, cx| {
            let Ok(status_bar) =
                workspace.read_with(cx, |workspace, _| workspace.status_bar.clone())
            else {
                return;
            };

            status_bar.update(cx, move |status_bar, cx| {
                status_bar.set_active_pane(&pane, window, cx);
            });
        });
    }
}

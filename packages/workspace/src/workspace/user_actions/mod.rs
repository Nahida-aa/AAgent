//! 用户触发的操作：保存、关闭、prompt、modal、快捷键、主题切换。
//!
//! 这些操作与 `workspace/mod.rs` 里的稳定状态访问器相对：前者改变状态或
//! 触发副作用，后者只读取当前状态。放在一个目录里，是为了让「用户能做
//! 什么」和「workspace 现在是什么」在文件树上也是分开的。
//!
//! 这一组里的每个文件对应一类操作：
//! - [`save`]：保存 dirty item
//! - [`close`]：关闭 workspace / pane / item，以及相关的 dirty prompt
//! - [`prompts`]：打开/新建路径的 prompt
//! - [`modals`]：modal 的显示与关闭
//! - [`keystrokes`]：向当前焦点元素派发按键
//! - [`theme`]：主题模式与全局开关


// save_all / save_all_internal / save_active_item
mod save;
// prepare_to_close / close_* / prompt_to_save_or_discard_dirty_items
mod close;
// prompt_for_open_path / prompt_for_new_path / set_prompt_for_*
mod prompts;
// toggle_modal / hide_modal / active_modal / reopen_last_picker
mod modals;
// send_keystrokes / send_keystrokes_impl
mod keystrokes;
// toggle_theme_mode / toggle_edit_predictions_all_files / clear_bookmarks
mod theme;

pub(crate) use close::*;
pub(crate) use keystrokes::*;
pub(crate) use modals::*;
pub(crate) use prompts::*;
pub(crate) use save::*;
pub(crate) use theme::*;

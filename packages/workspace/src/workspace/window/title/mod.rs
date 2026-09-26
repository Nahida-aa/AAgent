use super::*;
use gpui::{App, Context, Entity};

use super::Workspace;
use crate::dock::Dock;

// WindowTitleNeeds
mod needs;
// WindowTitleContext
mod context;
// WindowTitleTemplatePart,(parse\render)_window_title_format
mod format;
// update_window_title / apply_window_title / project_window_title
mod render;




impl Workspace {




}
pub(crate) use context::{WindowTitleContext};
pub(crate) use needs::{WindowTitleNeeds};

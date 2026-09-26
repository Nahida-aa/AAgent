use std::path::Path;

use gpui::{App, Context, Entity, Window};
use project::Project;
use anyhow;

use super::traits::Item;
use crate::Pane;
use crate::invalid_item_view::InvalidItemView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectItemKind(pub &'static str);

pub trait ProjectItem: Item {
    type Item: project::ProjectItem;

    fn project_item_kind() -> Option<ProjectItemKind> { None }

    fn for_project_item(
        project: Entity<Project>,
        pane: Option<&Pane>,
        item: Entity<Self::Item>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self
    where
        Self: Sized;

    /// A fallback handler, which will be called after [`project::ProjectItem::try_open`] fails,
    /// with the error from that failure as an argument.
    /// Allows to open an item that can gracefully display and handle errors.
    fn for_broken_project_item(
        _abs_path: &Path,
        _is_local: bool,
        _e: &anyhow::Error,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<InvalidItemView>
    where
        Self: Sized,
    {
        None
    }
}

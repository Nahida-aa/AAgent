use super::*;
impl Workspace {
    // │   ├── focusable_parts
    /// Returns the currently-visible major window regions ("parts"), in a stable
    /// cyclic order: title bar, left dock, editor, right dock, bottom dock,
    /// status bar. Closed docks are skipped. Used by
    /// [`FocusNextPart`]/[`FocusPreviousPart`] so keyboard and screen-reader
    /// users can move between regions without a mouse.
    fn focusable_parts(&self, cx: &App) -> Vec<FocusablePart> {
        // The interactive focus target inside an open dock: the active panel, or
        // the dock itself as a fallback. This is what region navigation focuses
        // when no screen reader is active (preserving the previous behavior).
        fn dock_content_handle(dock: &Entity<Dock>, cx: &App) -> FocusHandle {
            let dock = dock.read(cx);
            dock.active_panel()
                .map(|panel| panel.activation_focus_handle(cx))
                .unwrap_or_else(|| dock.focus_handle(cx))
        }

        let dock_part = |dock: &Entity<Dock>, wrapper: &FocusHandle| {
            dock.read(cx)
                .is_open()
                .then(|| FocusablePart::landmark(wrapper.clone(), dock_content_handle(dock, cx)))
        };

        let mut parts = Vec::new();
        if self.titlebar_item.is_some() {
            // The title bar is an ARIA toolbar, so region navigation lands on
            // its first control rather than the toolbar container.
            parts.push(FocusablePart::toolbar(self.titlebar_focus_handle.clone()));
        }
        parts.extend(dock_part(
            &self.left_dock,
            &self.region_focus_handles.left_dock,
        ));

        let center_pane = self
            .last_active_center_pane
            .as_ref()
            .and_then(|pane| pane.upgrade())
            .unwrap_or_else(|| self.center.first_pane());
        parts.push(FocusablePart::landmark(
            self.region_focus_handles.editor.clone(),
            center_pane.read(cx).focus_handle(cx),
        ));

        parts.extend(dock_part(
            &self.right_dock,
            &self.region_focus_handles.right_dock,
        ));
        parts.extend(dock_part(
            &self.bottom_dock,
            &self.region_focus_handles.bottom_dock,
        ));
        // The status bar is an ARIA toolbar, so region navigation lands on its
        // first control rather than the toolbar container.
        parts.push(FocusablePart::toolbar(
            self.status_bar.read(cx).focus_handle(cx),
        ));
        parts
    }
    // │   ├── move_part_focus
    /// Moves focus to the next (or previous) visible window region. See
    /// [`FocusNextPart`].
    pub(crate) fn move_part_focus(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let parts = self.focusable_parts(cx);
        if parts.is_empty() {
            return;
        }
        let current = parts
            .iter()
            .position(|part| part.contains_focused(window, cx));
        let next_index = match current {
            Some(index) if forward => (index + 1) % parts.len(),
            Some(index) => (index + parts.len() - 1) % parts.len(),
            None => 0,
        };
        let part = &parts[next_index];
        match &part.behavior {
            PartBehavior::Toolbar => {
                // The ARIA toolbar pattern requires focus to rest on the first
                // control, not the container. Focusing the tab-group container
                // and advancing descends into its first control; if the toolbar
                // has no focusable control, restore focus to the container so
                // navigation doesn't escape into an unrelated region.
                let container = part.container.clone();
                window.focus(&container, cx);
                window.focus_next(cx);
                let landed_inside = window
                    .focused(cx)
                    .is_some_and(|handle| container.contains(&handle, window));
                if !landed_inside {
                    window.focus(&container, cx);
                }
            }
            PartBehavior::Landmark { content } => {
                // Only redirect to the (otherwise non-focusable) wrapper when a
                // screen reader is active, so it is announced as a landmark.
                // Without a screen reader, focus the interactive content so
                // sighted keyboard users land somewhere usable, unchanged from
                // before this feature existed.
                if window.is_a11y_active() {
                    window.focus(&part.container, cx);
                } else {
                    window.focus(content, cx);
                }
            }
        }
        cx.notify();
    }
    // │   └── move_titlebar_item_focus
    /// Moves focus between the interactive controls within the title bar
    /// toolbar in response to arrow keys. Navigation is clamped to the title
    /// bar so arrows move between items and stop at the ends (ARIA toolbar
    /// semantics); Tab is still used to leave the toolbar.
    pub(crate) fn move_titlebar_item_focus(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = window.focused(cx);
        if forward {
            window.focus_next(cx);
        } else {
            window.focus_prev(cx);
        }
        let landed_in_titlebar = window
            .focused(cx)
            .is_some_and(|handle| self.titlebar_focus_handle.contains(&handle, window));
        // If Tab navigation wandered out of the toolbar, restore the previous
        // item so the ends of the toolbar act as stops rather than exits.
        if !landed_in_titlebar && let Some(previous) = previous {
            window.focus(&previous, cx);
        }
        cx.notify();
    }
    // │   ├── FocusablePart
    // │   ├── PartBehavior（含 Toolbar / Landmark 两个变体）
    // │   ├── FocusablePart::toolbar / landmark / contains_focused
    // │   ├── RegionFocusHandles
    // │   ├── RegionFocusHandles::new / dock
}
/// A focusable major window region used by region navigation
/// ([`FocusNextPart`]/[`FocusPreviousPart`]).
struct FocusablePart {
    /// The region's container/ancestor handle. For landmark regions this is the
    /// wrapper that carries the landmark role + label (and is only focusable
    /// while a screen reader is active); for toolbars it is the toolbar
    /// container.
    container: FocusHandle,
    behavior: PartBehavior,
}

pub(crate) enum PartBehavior {
    /// An ARIA toolbar (title bar, status bar). Region navigation focuses the
    /// container and descends to its first control, which is usable for
    /// everyone, so this is not gated on assistive technology.
    Toolbar,
    /// A landmark region (a dock or the editor). The wrapper carries the
    /// landmark role + label, so when a screen reader is active we focus it so
    /// it is announced. Otherwise we focus the interactive `content` so sighted
    /// keyboard users land on something usable, exactly as before - and the
    /// wrapper is left non-focusable so it never affects mouse focus.
    Landmark { content: FocusHandle },
}

impl FocusablePart {
    fn toolbar(container: FocusHandle) -> Self {
        Self {
            container,
            behavior: PartBehavior::Toolbar,
        }
    }

    fn landmark(wrapper: FocusHandle, content: FocusHandle) -> Self {
        Self {
            container: wrapper,
            behavior: PartBehavior::Landmark { content },
        }
    }

    /// Whether focus currently lies within this region. Checks both the wrapper
    /// (focused/ancestor when a screen reader is active) and, for landmarks, the
    /// interactive content (focused when a screen reader is not active, since
    /// the wrapper isn't in the focus tree then).
    fn contains_focused(&self, window: &Window, cx: &App) -> bool {
        if self.container.contains_focused(window, cx) {
            return true;
        }
        match &self.behavior {
            PartBehavior::Landmark { content } => content.contains_focused(window, cx),
            PartBehavior::Toolbar => false,
        }
    }
}

/// Focus handles for the landmark region wrappers (the docks and the editor).
/// Region navigation ([`FocusNextPart`]/[`FocusPreviousPart`]) focuses these so
/// it lands on the wrapper that actually carries the landmark role and label
/// (e.g. `Main`/"Editor", `Complementary`/"Left dock"). Without them, region
/// navigation would focus an inner element that has no accessibility node, and
/// assistive technology would fall back to announcing the whole window.
pub(crate) struct RegionFocusHandles {
    pub(crate) left_dock: FocusHandle,
    pub(crate) right_dock: FocusHandle,
    pub(crate) bottom_dock: FocusHandle,
    pub(crate) editor: FocusHandle,
}

impl RegionFocusHandles {
    pub(crate) fn new(cx: &mut App) -> Self {
        Self {
            left_dock: cx.focus_handle(),
            right_dock: cx.focus_handle(),
            bottom_dock: cx.focus_handle(),
            editor: cx.focus_handle(),
        }
    }

    pub(crate) fn dock(&self, position: DockPosition) -> &FocusHandle {
        match position {
            DockPosition::Left => &self.left_dock,
            DockPosition::Right => &self.right_dock,
            DockPosition::Bottom => &self.bottom_dock,
        }
    }
}

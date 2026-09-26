use gpui::{Anchor, AnyElement, App, Context, IntoElement, ParentElement, Styled, Window};
use gpui_util::maybe;
use project::ProjectPath;
use ui::{ContextMenu, IconButton, IconName, IconSize, PopoverMenu, Tooltip, prelude::*};
use util::{markdown::MarkdownInlineCode, paths::PathStyle, truncate_and_remove_front};

use super::Pane;
use crate::{
    CloseWindow, NewCenterTerminal, NewFile, NewTerminal, OpenInTerminal, OpenOptions,
    OpenTerminal, OpenVisible, SplitDirection, ToggleFileFinder, ToggleProjectSymbols, ToggleZoom,
    Workspace, WorkspaceItemBuilder, ZoomIn, ZoomOut,
    focus_follows_mouse::FocusFollowsMouse as _,
    invalid_item_view::InvalidItemView,
    item::{
        ActivateOnClose, ClosePosition, Item, ItemBufferKind, ItemHandle, ItemSettings,
        PreviewTabsSettings, ProjectItemKind, SaveOptions, ShowCloseButton, ShowDiagnostics,
        TabContentParams, TabTooltipContent, WeakItemHandle,
    },
    move_item,
    notifications::NotifyResultExt,
    toolbar::Toolbar,
    workspace_settings::{AutosaveSetting, FocusFollowsMouse, TabBarSettings, WorkspaceSettings},
};

pub(super) fn dirty_message_for(buffer_path: Option<ProjectPath>, path_style: PathStyle) -> String {
    let path = buffer_path.as_ref().and_then(|p| {
        let path = p.path.display(path_style);
        if path.is_empty() { None } else { Some(path) }
    });
    match path {
        Some(path) => {
            let path = truncate_and_remove_front(&path, 80);
            format!(
                "{} contains unsaved edits. Do you want to save it?",
                MarkdownInlineCode(path.as_str())
            )
        }
        None => "This buffer contains unsaved edits. Do you want to save it?".to_string(),
    }
}

pub fn tab_details(items: &[Box<dyn ItemHandle>], _window: &Window, cx: &App) -> Vec<usize> {
    util::disambiguate::compute_disambiguation_details(items, |item, detail| {
        item.tab_content_text(detail, cx)
    })
}

pub fn render_item_indicator(item: Box<dyn ItemHandle>, cx: &App) -> Option<ui::Indicator> {
    maybe!({
        let indicator_color = match (item.has_conflict(cx), item.is_dirty(cx)) {
            (true, _) => Color::Warning,
            (_, true) => Color::Accent,
            (false, false) => return None,
        };

        Some(Indicator::dot().color(indicator_color))
    })
}

pub(super) fn default_render_tab_bar_buttons(
    pane: &mut Pane,
    window: &mut Window,
    cx: &mut Context<Pane>,
) -> (Option<AnyElement>, Option<AnyElement>) {
    if !pane.has_focus(window, cx) && !pane.context_menu_focused(window, cx) {
        return (None, None);
    }
    let (can_clone, can_split_move) = match pane.active_item() {
        Some(active_item) if active_item.can_split(cx) => (true, false),
        Some(_) => (false, pane.items_len() > 1),
        None => (false, false),
    };
    // Ideally we would return a vec of elements here to pass directly to the [TabBar]'s
    // `end_slot`, but due to needing a view here that isn't possible.
    let right_children = h_flex()
        // Instead we need to replicate the spacing from the [TabBar]'s `end_slot` here.
        .gap(DynamicSpacing::Base04.rems(cx))
        .child(
            PopoverMenu::new("pane-tab-bar-popover-menu")
                .trigger_with_tooltip(
                    IconButton::new("plus", IconName::Plus).icon_size(IconSize::Small),
                    Tooltip::text("New…"),
                )
                .anchor(Anchor::TopRight)
                .with_handle(pane.new_item_context_menu_handle.clone())
                .menu(move |window, cx| {
                    Some(ContextMenu::build(window, cx, |menu, _, _| {
                        menu.action("New File", NewFile.boxed_clone())
                            .action("Open File", ToggleFileFinder::default().boxed_clone())
                            .separator()
                            .action("Search Project", DeploySearch::default().boxed_clone())
                            .action("Search Symbols", ToggleProjectSymbols.boxed_clone())
                            .separator()
                            .action("New Terminal", NewTerminal::default().boxed_clone())
                            .action(
                                "New Center Terminal",
                                NewCenterTerminal::default().boxed_clone(),
                            )
                    }))
                }),
        )
        .child(
            PopoverMenu::new("pane-tab-bar-split")
                .trigger_with_tooltip(
                    IconButton::new("split", IconName::Split)
                        .icon_size(IconSize::Small)
                        .disabled(!can_clone && !can_split_move),
                    Tooltip::text("Split Pane"),
                )
                .anchor(Anchor::TopRight)
                .with_handle(pane.split_item_context_menu_handle.clone())
                .menu(move |window, cx| {
                    ContextMenu::build(window, cx, |menu, _, _| {
                        let mode = SplitMode::MovePane;
                        if can_split_move {
                            menu.action("Split Right", SplitRight { mode }.boxed_clone())
                                .action("Split Left", SplitLeft { mode }.boxed_clone())
                                .action("Split Up", SplitUp { mode }.boxed_clone())
                                .action("Split Down", SplitDown { mode }.boxed_clone())
                        } else {
                            menu.action("Split Right", SplitRight::default().boxed_clone())
                                .action("Split Left", SplitLeft::default().boxed_clone())
                                .action("Split Up", SplitUp::default().boxed_clone())
                                .action("Split Down", SplitDown::default().boxed_clone())
                        }
                    })
                    .into()
                }),
        )
        .child({
            let zoomed = pane.is_zoomed();
            IconButton::new("toggle_zoom", IconName::Maximize)
                .icon_size(IconSize::Small)
                .toggle_state(zoomed)
                .selected_icon(IconName::Minimize)
                .on_click(cx.listener(|pane, _, window, cx| {
                    pane.toggle_zoom(&crate::ToggleZoom, window, cx);
                }))
                .tooltip(move |_window, cx| {
                    Tooltip::for_action(
                        if zoomed { "Zoom Out" } else { "Zoom In" },
                        &ToggleZoom,
                        cx,
                    )
                })
        })
        .into_any_element()
        .into();
    (None, right_children)
}

use gpui::{
    App, AppContext, Context, ElementId, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
};

use crate::terminal::TerminalView;

/// A minimal clone of zed's `Agent` enum (zed: crates/agent_ui/src/agent_ui.rs:425).
/// Placeholder agents are listed in the new-thread menu; only `Terminal` has a
/// working implementation so far.
#[derive(Debug, Clone, PartialEq)]
pub enum Agent {
    AAgent,
    Terminal,
    Placeholder(&'static str),
}

impl Agent {
    pub fn label(&self) -> &str {
        match self {
            Agent::AAgent => "AAgent",
            Agent::Terminal => "Terminal",
            Agent::Placeholder(name) => name,
        }
    }
}

/// The surface currently shown below the toolbar (zed: `VisibleSurface`).
#[derive(Debug, Clone, PartialEq)]
pub enum VisibleSurface {
    AgentThread,
    Terminal,
}

pub struct AgentPanel {
    pub focus_handle: FocusHandle,
    pub surface: VisibleSurface,
    pub selected_agent: Agent,
    pub terminal: Entity<TerminalView>,
    pub new_thread_menu_open: bool,
    pub composer_focus: FocusHandle,
    pub composer_text: String,
}

const EXTERNAL_AGENT_PLACEHOLDERS: &[&str] = &["opencode"];

impl AgentPanel {
    pub fn new(cx: &mut Context<Self>, working_dir: std::path::PathBuf) -> Self {
        let focus_handle = cx.focus_handle();
        let composer_focus = cx.focus_handle();
        let terminal = cx.new(|cx| TerminalView::new(None, working_dir, cx));
        Self {
            focus_handle,
            surface: VisibleSurface::AgentThread,
            selected_agent: Agent::AAgent,
            terminal,
            new_thread_menu_open: false,
            composer_focus,
            composer_text: String::new(),
        }
    }

    pub fn new_thread(&mut self) {
        self.surface = match self.selected_agent {
            Agent::Terminal => VisibleSurface::Terminal,
            _ => VisibleSurface::AgentThread,
        };
        self.new_thread_menu_open = false;
    }

    pub fn toggle_new_thread_menu(&mut self) {
        self.new_thread_menu_open = !self.new_thread_menu_open;
    }

    pub fn is_agent_selected(&self, agent: &Agent) -> bool {
        &self.selected_agent == agent
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_agent_label = self.selected_agent.label().to_string();

        let new_thread_menu_button = gpui::div()
            .id("new_thread_menu_btn")
            .h(gpui::px(24.0))
            .w(gpui::px(24.0))
            .flex_none()
            .items_center()
            .justify_center()
            .rounded(gpui::rems(0.25))
            .hover(|style| style.bg(gpui::rgb(0x2a2a2e)))
            .active(|style| style.bg(gpui::rgb(0x3a3a40)))
            .on_click(cx.listener(|this, _event, window, _cx| {
                this.toggle_new_thread_menu();
                if this.new_thread_menu_open {
                    window.focus(&this.focus_handle, _cx);
                }
            }))
            .child(gpui::div().text_color(gpui::rgb(0x8a8a92)).child("+"));

        let options_menu_button = gpui::div()
            .id("options_menu_btn")
            .h(gpui::px(24.0))
            .w(gpui::px(24.0))
            .flex_none()
            .items_center()
            .justify_center()
            .rounded(gpui::rems(0.25))
            .hover(|style| style.bg(gpui::rgb(0x2a2a2e)))
            .active(|style| style.bg(gpui::rgb(0x3a3a40)))
            .child(gpui::div().text_color(gpui::rgb(0x8a8a92)).child("…"));

        let title = match self.surface {
            VisibleSurface::Terminal => gpui::div()
                .text_color(gpui::rgb(0x8a8a92))
                .child(format!("New {} Thread", selected_agent_label))
                .into_any_element(),
            VisibleSurface::AgentThread => gpui::div()
                .text_color(gpui::rgb(0xdbdbe1))
                .child(selected_agent_label)
                .into_any_element(),
        };

        gpui::div()
            .id("agent-panel-toolbar")
            .flex_row()
            .h(gpui::px(32.0))
            .flex_shrink_0()
            .items_center()
            .justify_between()
            .px(gpui::rems(0.5))
            .bg(gpui::rgb(0x1a1a1e))
            .border_b_1()
            .border_color(gpui::rgb(0x2a2a2e))
            .child(
                gpui::div()
                    .flex_row()
                    .items_center()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(title),
            )
            .child(
                gpui::div()
                    .flex_row()
                    .items_center()
                    .flex_none()
                    .gap_1()
                    .child(new_thread_menu_button)
                    .child(options_menu_button),
            )
    }

    fn render_new_thread_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let menu_items = std::iter::once(Agent::AAgent)
            .chain(std::iter::once(Agent::Terminal))
            .chain(
                EXTERNAL_AGENT_PLACEHOLDERS
                    .iter()
                    .copied()
                    .map(Agent::Placeholder),
            )
            .collect::<Vec<_>>();

        let item_elements = menu_items
            .into_iter()
            .map(|agent| {
                let is_selected = self.is_agent_selected(&agent);
                let label = agent.label().to_string();
                gpui::div()
                    .id(ElementId::Name(SharedString::from(label.clone())))
                    .flex_row()
                    .items_center()
                    .w_full()
                    .px(gpui::rems(0.25))
                    .py(gpui::rems(0.125))
                    .gap_1()
                    .hover(|style| style.bg(gpui::rgb(0x2e2e33)))
                    .on_click(cx.listener(move |this, _event, _window, _cx| {
                        this.selected_agent = agent.clone();
                        this.new_thread();
                    }))
                    .child(
                        gpui::div()
                            .w(gpui::px(8.0))
                            .h(gpui::px(8.0))
                            .rounded_full()
                            .bg(if is_selected {
                                gpui::rgb(0x6ea8fe)
                            } else {
                                gpui::rgb(0x3a3a40)
                            }),
                    )
                    .child(
                        gpui::div()
                            .text_color(gpui::rgb(0xdbdbe1))
                            .child(label.to_string()),
                    )
            })
            .collect::<Vec<_>>();

        gpui::div()
            .id("new_thread_menu")
            .absolute()
            .top(gpui::px(34.0))
            .right(gpui::px(8.0))
            .w(gpui::px(220.0))
            .max_h(gpui::px(300.0))
            .py(gpui::rems(0.25))
            .overflow_y_scroll()
            .rounded_md()
            .bg(gpui::rgb(0x232327))
            .border_1()
            .border_color(gpui::rgb(0x3a3a40))
            .shadow_lg()
            .children(item_elements)
    }

    fn render_surface(&self, cx: &mut Context<Self>) -> impl IntoElement {
        match self.surface {
            VisibleSurface::Terminal => gpui::div()
                .size_full()
                .child(self.terminal.clone())
                .into_any_element(),
            VisibleSurface::AgentThread => self.render_conversation(cx).into_any_element(),
        }
    }

    fn render_conversation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        gpui::div()
            .flex_col()
            .size_full()
            .child(self.render_message_region(cx))
            .child(self.render_composer(cx))
    }

    /// Placeholder message list. Mirrors the flex-1 scrollable region that
    /// zed's thread view reserves above the composer.
    fn render_message_region(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let has_messages = false;
        let empty_state = if !has_messages {
            gpui::div()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .p_4()
                .child(
                    gpui::div()
                        .flex_col()
                        .items_center()
                        .child(
                            gpui::div()
                                .text_color(gpui::rgb(0x8a8a92))
                                .child(gpui::Text::new_inaccessible("New AAgent Thread".into()))
                                .into_any_element(),
                        )
                        .child(
                            gpui::div()
                                .text_color(gpui::rgb(0x6a6a72))
                                .child(gpui::Text::new_inaccessible(
                                    "Ask AAgent a question or choose a different agent from the + menu.".into(),
                                ))
                                .into_any_element(),
                        ),
                )
                .into_any_element()
        } else {
            gpui::div().into_any_element()
        };

        gpui::div()
            .flex_1()
            .min_h_0()
            .size_full()
            .child(empty_state)
    }

    /// Bottom composer mirroring zed's `render_message_editor`:
    /// editor region + a footer row with add-context/thinking + mode/model/send.
    fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_text = !self.composer_text.is_empty();
        let placeholder = !has_text;

        let composer_focus = self.composer_focus.clone();
        let editor_region = gpui::div()
            .id("message-editor")
            .w_full()
            .min_h_0()
            .py_1()
            .text_color(if placeholder {
                gpui::rgb(0x6a6a72)
            } else {
                gpui::rgb(0xdbdbe1)
            })
            .track_focus(&composer_focus)
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, _cx| {
                    let keystroke = &event.keystroke;
                    if let Some(c) = &keystroke.key_char {
                        if !keystroke.modifiers.control
                            && !keystroke.modifiers.alt
                            && !keystroke.modifiers.platform
                        {
                            this.composer_text.push_str(c);
                        }
                    } else if keystroke.key == "backspace" {
                        this.composer_text.pop();
                    }
                    if let Some(_c) = &this.composer_text.chars().next() {
                        // no-op: future send handling
                    }
                }),
            )
            .child(if placeholder {
                gpui::div()
                    .text_color(gpui::rgb(0x6a6a72))
                    .child(gpui::Text::new_inaccessible("Message AAgent…".into()))
                    .into_any_element()
            } else {
                gpui::div()
                    .text_color(gpui::rgb(0xdbdbe1))
                    .child(gpui::Text::new_inaccessible(
                        self.composer_text.as_str().into(),
                    ))
                    .into_any_element()
            });

        let send_button = gpui::div()
            .id("send-button")
            .flex_none()
            .px_2()
            .py_1()
            .rounded(gpui::rems(0.25))
            .bg(gpui::rgb(0x2e2e33))
            .hover(|style| style.bg(gpui::rgb(0x3a3a40)))
            .active(|style| style.bg(gpui::rgb(0x4a4a50)))
            .on_click({
                let composer_focus = self.composer_focus.clone();
                cx.listener(move |this, _event, window, _cx| {
                    this.composer_text.clear();
                    window.focus(&composer_focus, _cx);
                })
            })
            .child(
                gpui::div()
                    .text_color(gpui::rgb(0xdbdbe1))
                    .child(gpui::Text::new_inaccessible("Send".into()))
                    .into_any_element(),
            );

        let footer_left = gpui::div()
            .flex_row()
            .flex_wrap()
            .gap_0p5()
            .child(self.render_footer_button("Add Context", cx))
            .child(self.render_footer_button("Thinking", cx));

        let footer_right = gpui::div()
            .flex_row()
            .gap_1()
            .child(self.render_footer_button("Auto", cx))
            .child(self.render_footer_button("model", cx))
            .child(send_button);

        gpui::div()
            .flex_row()
            .py_2()
            .justify_center()
            .bg(gpui::rgb(0x111115))
            .border_t_1()
            .border_color(gpui::rgb(0x2a2a2e))
            .child(
                gpui::div()
                    .w_full()
                    .max_w(gpui::px(760.0))
                    .px_2()
                    .flex_col()
                    .gap_2()
                    .child(editor_region)
                    .child(
                        gpui::div()
                            .flex_row()
                            .w_full()
                            .justify_between()
                            .child(footer_left)
                            .child(footer_right),
                    ),
            )
    }

    fn render_footer_button(&self, label: &str, _cx: &Context<Self>) -> impl IntoElement {
        gpui::div()
            .flex_none()
            .px_1p5()
            .py_0p5()
            .rounded(gpui::rems(0.25))
            .hover(|style| style.bg(gpui::rgb(0x2e2e33)))
            .child(
                gpui::div()
                    .text_color(gpui::rgb(0x9a9aa2))
                    .child(gpui::Text::new_inaccessible(label.into()))
                    .into_any_element(),
            )
    }
}

impl Render for AgentPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let overlay = self.new_thread_menu_open.then(|| {
            gpui::div()
                .id("new-thread-menu-overlay")
                .absolute()
                .inset_0()
                .track_focus(&self.focus_handle.clone())
                .on_key_down(
                    cx.listener(|this, event: &gpui::KeyDownEvent, _window, _cx| {
                        if event.keystroke.key == "escape" {
                            this.new_thread_menu_open = false;
                        }
                    }),
                )
                .on_click(cx.listener(|this, _event, window, _cx| {
                    this.new_thread_menu_open = false;
                    window.focus(&this.focus_handle, _cx);
                }))
                .into_any_element()
        });

        let menu = self
            .new_thread_menu_open
            .then(|| self.render_new_thread_menu(cx).into_any_element());

        gpui::div()
            .id("agent-panel")
            .key_context("agent_panel")
            .flex_col()
            .size_full()
            .relative()
            .track_focus(&self.focus_handle.clone())
            .bg(gpui::rgb(0x141417))
            .on_click(cx.listener(|this, _event, window, _cx| {
                window.focus(&this.focus_handle, _cx);
            }))
            .child(
                gpui::div()
                    .size_full()
                    .flex_col()
                    .child(self.render_toolbar(cx))
                    .child(self.render_surface(cx)),
            )
            .children(overlay.into_iter())
            .children(menu.into_iter())
    }
}

impl Focusable for AgentPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

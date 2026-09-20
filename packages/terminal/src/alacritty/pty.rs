use std::{borrow::Cow, sync::Arc};

use alacritty_terminal::{
    event::{Event as AlacTermEvent, EventListener, Notify, WindowSize},
    event_loop::{Msg, Notifier},
};
use futures::channel::mpsc::UnboundedSender;

use crate::{PtyEvent, TerminalBounds, events::TerminalBackendEvent};

pub(super) struct PtySender {
    notifier: Notifier,
}

impl PtySender {
    pub(super) fn notify(&self, input: impl Into<Cow<'static, [u8]>>) {
        self.notifier.notify(input);
    }

    pub(super) fn resize(&self, bounds: TerminalBounds) {
        if let Err(error) = self
            .notifier
            .0
            .send(Msg::Resize(window_size_from_terminal_bounds(bounds)))
        {
            log::error!("failed to resize alacritty pty: {error}");
        }
    }

    pub(super) fn shutdown(&self) {
        if let Err(error) = self.notifier.0.send(Msg::Shutdown) {
            log::debug!("failed to shut down alacritty pty loop: {error}");
        }
    }
}

fn window_size_from_terminal_bounds(bounds: TerminalBounds) -> WindowSize {
    WindowSize {
        num_lines: bounds.num_lines() as u16,
        num_cols: bounds.num_columns() as u16,
        cell_width: f32::from(bounds.cell_width()) as u16,
        cell_height: f32::from(bounds.line_height()) as u16,
    }
}

#[derive(Clone)]
pub(super) struct ZedListener(UnboundedSender<PtyEvent>);

impl EventListener for ZedListener {
    fn send_event(&self, event: AlacTermEvent) {
        self.0.unbounded_send(PtyEvent::Event(event.into())).ok();
    }
}

impl From<AlacTermEvent> for TerminalBackendEvent {
    fn from(event: AlacTermEvent) -> Self {
        match event {
            AlacTermEvent::MouseCursorDirty => Self::MouseCursorDirty,
            AlacTermEvent::Title(title) => Self::Title(title),
            AlacTermEvent::ResetTitle => Self::ResetTitle,
            AlacTermEvent::ClipboardStore(_, data) => Self::ClipboardStore(data),
            AlacTermEvent::ClipboardLoad(_, format) => Self::ClipboardLoad(format),
            AlacTermEvent::ColorRequest(index, format) => Self::ColorRequest(index, format),
            AlacTermEvent::PtyWrite(output) => Self::PtyWrite(output),
            AlacTermEvent::TextAreaSizeRequest(format) => {
                Self::TextAreaSizeRequest(Arc::new(move |bounds| {
                    format(window_size_from_terminal_bounds(bounds))
                }))
            }
            AlacTermEvent::CursorBlinkingChange => Self::CursorBlinkingChange,
            AlacTermEvent::Wakeup => Self::Wakeup,
            AlacTermEvent::Bell => Self::Bell,
            AlacTermEvent::Exit => Self::Exit,
            AlacTermEvent::ChildExit(status) => Self::ChildExit(status),
        }
    }
}

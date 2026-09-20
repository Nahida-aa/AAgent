use std::borrow::Cow;

use alacritty_terminal::{
    event::Notify,
    event_loop::{Msg, Notifier},
    sync::FairMutex,
    term::search::RegexSearch,
};
use futures::channel::mpsc::UnboundedSender;

use crate::{PtyEvent, TerminalBounds, alacritty::pty::window_size_from_terminal_bounds};

use super::AlacrittyTermLock;

#[derive(Clone)]
pub(crate) struct ZedListener(pub(crate) UnboundedSender<PtyEvent>);

#[derive(Clone, Debug)]
pub(crate) struct AlacrittySearch {
    pub(crate) search: RegexSearch,
}

pub(crate) struct PtySender {
    pub(super) notifier: Notifier,
}

impl PtySender {
    pub(crate) fn notify(&self, input: impl Into<Cow<'static, [u8]>>) {
        self.notifier.notify(input);
    }

    pub(crate) fn resize(&self, bounds: TerminalBounds) {
        if let Err(error) = self
            .notifier
            .0
            .send(Msg::Resize(window_size_from_terminal_bounds(bounds)))
        {
            log::error!("failed to resize alacritty pty: {error}");
        }
    }

    pub(crate) fn shutdown(&self) {
        if let Err(error) = self.notifier.0.send(Msg::Shutdown) {
            log::debug!("failed to shut down alacritty pty loop: {error}");
        }
    }
}

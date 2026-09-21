use std::cmp;

use gpui::{App, Context, Pixels, ScrollWheelEvent, Window, px};
use settings::Settings;
use terminal::terminal_settings::TerminalSettings;
use terminal::{
    Clear, Modes, ScrollLineDown, ScrollLineUp, ScrollPageDown, ScrollPageUp, ScrollToBottom,
    ScrollToTop,
};

use super::TerminalView;
use super::helpers::viewport_line_for_point;

impl TerminalView {
    pub(super) fn clear(&mut self, _: &Clear, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_top = px(0.);
        self.terminal.update(cx, |term, _| term.clear());
        cx.notify();
    }

    pub(super) fn scroll_wheel(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let terminal_content = self.terminal.read(cx).last_content();

        if self.block_below_cursor.is_some() && terminal_content.display_offset == 0 {
            let line_height = terminal_content.terminal_bounds.line_height;
            let y_delta = event.delta.pixel_delta(line_height).y;
            if y_delta < Pixels::ZERO || self.scroll_top > Pixels::ZERO {
                self.scroll_top = cmp::max(
                    Pixels::ZERO,
                    cmp::min(self.scroll_top - y_delta, self.max_scroll_top(cx)),
                );
                cx.notify();
                return;
            }
        }
        self.terminal.update(cx, |term, cx| {
            term.scroll_wheel(
                event,
                TerminalSettings::get_global(cx).scroll_multiplier.max(0.01),
            )
        });
    }

    fn max_scroll_top(&self, cx: &App) -> Pixels {
        let terminal = self.terminal.read(cx);

        let Some(block) = self.block_below_cursor.as_ref() else {
            return Pixels::ZERO;
        };

        let line_height = terminal.last_content().terminal_bounds.line_height;
        let viewport_lines = terminal.viewport_lines();
        let cursor_line = viewport_line_for_point(
            terminal.last_content.cursor.point,
            terminal.last_content.display_offset,
        )
        .unwrap_or_default();
        let max_scroll_top_in_lines =
            (block.height as usize).saturating_sub(viewport_lines.saturating_sub(cursor_line + 1));

        max_scroll_top_in_lines as f32 * line_height
    }

    pub(super) fn is_alt_screen(&self, cx: &App) -> bool {
        self.terminal
            .read(cx)
            .last_content
            .mode
            .contains(Modes::ALT_SCREEN)
    }

    pub(super) fn scroll_line_up(
        &mut self,
        _: &ScrollLineUp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_alt_screen(cx) {
            cx.propagate();
            return;
        }

        let terminal_content = self.terminal.read(cx).last_content();
        if self.block_below_cursor.is_some()
            && terminal_content.display_offset == 0
            && self.scroll_top > Pixels::ZERO
        {
            let line_height = terminal_content.terminal_bounds.line_height;
            self.scroll_top = cmp::max(self.scroll_top - line_height, Pixels::ZERO);
            return;
        }

        self.terminal.update(cx, |term, _| term.scroll_line_up());
        cx.notify();
    }

    pub(super) fn scroll_line_down(
        &mut self,
        _: &ScrollLineDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_alt_screen(cx) {
            cx.propagate();
            return;
        }

        let terminal_content = self.terminal.read(cx).last_content();
        if self.block_below_cursor.is_some() && terminal_content.display_offset == 0 {
            let max_scroll_top = self.max_scroll_top(cx);
            if self.scroll_top < max_scroll_top {
                let line_height = terminal_content.terminal_bounds.line_height;
                self.scroll_top = cmp::min(self.scroll_top + line_height, max_scroll_top);
            }
            return;
        }

        self.terminal.update(cx, |term, _| term.scroll_line_down());
        cx.notify();
    }

    pub(super) fn scroll_page_up(
        &mut self,
        _: &ScrollPageUp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_alt_screen(cx) {
            cx.propagate();
            return;
        }

        if self.scroll_top == Pixels::ZERO {
            self.terminal.update(cx, |term, _| term.scroll_page_up());
        } else {
            let line_height = self
                .terminal
                .read(cx)
                .last_content
                .terminal_bounds
                .line_height();
            let visible_block_lines = (self.scroll_top / line_height) as usize;
            let viewport_lines = self.terminal.read(cx).viewport_lines();
            let visible_content_lines = viewport_lines - visible_block_lines;

            if visible_block_lines >= viewport_lines {
                self.scroll_top = ((visible_block_lines - viewport_lines) as f32) * line_height;
            } else {
                self.scroll_top = px(0.);
                self.terminal
                    .update(cx, |term, _| term.scroll_up_by(visible_content_lines));
            }
        }
        cx.notify();
    }

    pub(super) fn scroll_page_down(
        &mut self,
        _: &ScrollPageDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_alt_screen(cx) {
            cx.propagate();
            return;
        }

        self.terminal.update(cx, |term, _| term.scroll_page_down());
        let terminal = self.terminal.read(cx);
        if terminal.last_content().display_offset < terminal.viewport_lines() {
            self.scroll_top = self.max_scroll_top(cx);
        }
        cx.notify();
    }

    pub(super) fn scroll_to_top(
        &mut self,
        _: &ScrollToTop,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_alt_screen(cx) {
            cx.propagate();
            return;
        }

        self.terminal.update(cx, |term, _| term.scroll_to_top());
        cx.notify();
    }

    pub(super) fn scroll_to_bottom(
        &mut self,
        _: &ScrollToBottom,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_alt_screen(cx) {
            cx.propagate();
            return;
        }

        self.terminal.update(cx, |term, _| term.scroll_to_bottom());
        if self.block_below_cursor.is_some() {
            self.scroll_top = self.max_scroll_top(cx);
        }
        cx.notify();
    }

    pub(super) fn toggle_vi_mode(
        &mut self,
        _: &terminal::ToggleViMode,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal.update(cx, |term, _| term.toggle_vi_mode());
        cx.notify();
    }
}

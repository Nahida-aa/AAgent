pub mod alacritty;
pub mod element;

use std::path::PathBuf;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent, Render, Window,
    div, prelude::*, px,
};
use tracing::debug;

use crate::terminal::alacritty::{AlacrittyBackend, TerminalBounds};

/// A terminal session view: owns the pty-backed backend and forwards keyboard
/// input into it. Renders via [`element::TerminalElement`].
pub struct TerminalView {
    backend: std::sync::Arc<AlacrittyBackend>,
    focus_handle: FocusHandle,
    /// last known column/line count, so the element can skip layout when size unchanged.
    bounds: TerminalBounds,
}

impl TerminalView {
    pub fn new(shell: Option<String>, working_dir: PathBuf, cx: &mut App) -> Self {
        // Placeholder bounds; the element re-measures the real cell metrics on
        // first layout and replaces these via `set_bounds`.
        let bounds = TerminalBounds {
            cell_width: px(10.0),
            line_height: px(16.0),
            width: px(800.0),
            height: px(600.0),
            font_size: px(15.0),
        };
        let backend = match AlacrittyBackend::new(bounds, shell, working_dir) {
            Ok(b) => std::sync::Arc::new(b),
            Err(e) => {
                debug!(error = ?e, "failed to spawn pty");
                panic!("pty spawn failed: {e}")
            }
        };
        Self {
            backend,
            focus_handle: cx.focus_handle(),
            bounds,
        }
    }

    pub fn backend(&self) -> &AlacrittyBackend {
        &self.backend
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let mut bytes: Vec<u8> = Vec::new();
        let k = &event.keystroke;

        // Handle modifying keys with to_string conventions.
        let key_str = k.key.as_str();
        let is_alt = k.modifiers.alt;

        match key_str {
            "enter" => bytes.extend_from_slice(b"\r"),
            "backspace" => bytes.push(0x7f),
            "tab" => bytes.extend_from_slice(if k.modifiers.shift { b"\x1b[Z" } else { b"\t" }),
            "escape" => bytes.push(0x1b),
            "left" => bytes.extend_from_slice(if is_alt { b"\x1b[1;3D" } else { b"\x1b[D" }),
            "right" => bytes.extend_from_slice(if is_alt { b"\x1b[1;3C" } else { b"\x1b[C" }),
            "up" => bytes.extend_from_slice(if is_alt { b"\x1b[1;3A" } else { b"\x1b[A" }),
            "down" => bytes.extend_from_slice(if is_alt { b"\x1b[1;3B" } else { b"\x1b[B" }),
            "home" => bytes.extend_from_slice(b"\x1b[H"),
            "end" => bytes.extend_from_slice(b"\x1b[F"),
            "pageup" => bytes.extend_from_slice(b"\x1b[5~"),
            "pagedown" => bytes.extend_from_slice(b"\x1b[6~"),
            "delete" => bytes.extend_from_slice(b"\x1b[3~"),
            "colon" if is_alt => {}
            "shift" | "ctrl" | "alt" | "super" => {}
            _ => {
                if let Some(c) = k.key.chars().next() {
                    // Prefer an explicit `key_char` if the platform produced one.
                    let printable = k
                        .key_char
                        .as_deref()
                        .and_then(|s| s.chars().next())
                        .unwrap_or(c);

                    // If a plain modifier is held (ctrl/alt) on a printable char,
                    // send the control-ish escape instead of the text.
                    let with_ctrl = k.modifiers.control;
                    let is_letter = printable.is_ascii_alphabetic();
                    if with_ctrl && is_letter {
                        bytes.push(printable.to_ascii_uppercase() as u8 & 0x1f);
                    } else if is_alt {
                        bytes.push(0x1b);
                        let mut s = [0u8; 4];
                        let _ = printable.encode_utf8(&mut s);
                        bytes.extend_from_slice(printable.to_string().as_bytes());
                    } else if printable.is_control() {
                        // skip
                    } else {
                        bytes.extend_from_slice(printable.to_string().as_bytes());
                    }
                }
            }
        }

        if !bytes.is_empty() {
            self.backend.write_input(&bytes);
            cx.notify();
        }
    }
}

impl EventEmitter<()> for TerminalView {}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .key_context("terminal")
            .on_key_down(cx.listener(Self::on_key))
            .child(element::TerminalElement::new(
                self.backend.clone(),
                self.bounds,
            ))
    }
}

/// Build bounds from the window's real UI metrics, mirroring how zed
/// computes terminal dimensions: the font size comes from the global
/// buffer font (here surfaced through `window.text_style()`), rem scaling
/// uses the window's rem size, and the cell width is measured from the
/// actual metrics of the resolved font.
///
/// The element re-measures on every layout pass and corrects the grid via
/// [`element::TerminalElement`]; this is only the pre-layout approximation.
pub fn initial_bounds(window: &Window) -> TerminalBounds {
    let rem = window.rem_size();
    let text_style = window.text_style();
    let font_size = text_style.font_size.to_pixels(rem);

    let text_system = window.text_system();
    let font_id = text_system.resolve_font(&text_style.font());
    let cell_width = text_system
        .advance(font_id, font_size, 'm')
        .map(|adv| adv.width)
        .unwrap_or(px(f32::from(font_size) * 0.66));

    // Default terminal line-height multiplier; matches zed’s base value.
    const LINE_HEIGHT_MULTIPLIER: f32 = 1.35;
    let line_height = px(f32::from(font_size) * LINE_HEIGHT_MULTIPLIER);

    TerminalBounds {
        cell_width,
        line_height,
        width: rem,
        height: rem,
        font_size,
    }
}

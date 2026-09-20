use crate::terminal_settings::CursorShape as SettingsCursorShape;
use alacritty_terminal::{
    term::{Config, Osc52, SEMANTIC_ESCAPE_CHARS},
    vte::ansi::{CursorShape as AlacCursorShape, CursorStyle as AlacCursorStyle},
};

use super::AlacrittyTermConfig;

pub(crate) fn display_only_term_config(
    scrolling_history: usize,
    cursor_shape: SettingsCursorShape,
) -> AlacrittyTermConfig {
    Config {
        scrolling_history,
        default_cursor_style: alacritty_cursor_style(cursor_shape),
        semantic_escape_chars: format!("{SEMANTIC_ESCAPE_CHARS}─"),
        osc52: Osc52::Disabled,
        ..Config::default()
    }
}

pub(crate) fn pty_term_config(
    scrolling_history: usize,
    cursor_shape: SettingsCursorShape,
) -> AlacrittyTermConfig {
    Config {
        scrolling_history,
        default_cursor_style: alacritty_cursor_style(cursor_shape),
        semantic_escape_chars: format!("{SEMANTIC_ESCAPE_CHARS}─"),
        ..Config::default()
    }
}

pub(crate) fn set_default_cursor_style(
    config: &mut AlacrittyTermConfig,
    cursor_shape: SettingsCursorShape,
) {
    config.default_cursor_style = alacritty_cursor_style(cursor_shape);
}

fn alacritty_cursor_style(cursor_shape: SettingsCursorShape) -> AlacCursorStyle {
    AlacCursorStyle {
        shape: alacritty_cursor_shape(cursor_shape),
        blinking: false,
    }
}

fn alacritty_cursor_shape(cursor_shape: SettingsCursorShape) -> AlacCursorShape {
    match cursor_shape {
        SettingsCursorShape::Block => AlacCursorShape::Block,
        SettingsCursorShape::Underline => AlacCursorShape::Underline,
        SettingsCursorShape::Bar => AlacCursorShape::Beam,
        SettingsCursorShape::Hollow => AlacCursorShape::HollowBlock,
    }
}

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::Event;
use crate::alacritty::{clear_saved_screen, last_non_empty_lines, make_content};
use crate::mappings::keys::to_esc_str;
use crate::{
    TerminalBounds,
    cursor::Point,
    events::InternalEvent,
    mappings::mouse::{grid_point, grid_point_and_side, mouse_button_report, mouse_moved_report},
    modes::Modes,
    selection::{Scroll, Selection, SelectionPhase, SelectionSide, SelectionType, ViMotion},
    terminal_settings::TerminalSettings,
};
use gpui::{
    Bounds, Context, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point as GpuiPoint, ScrollWheelEvent, Task, TouchPhase, Window,
};
use std::{
    borrow::Cow,
    cmp::{self, min},
};
use util::ShellKind;

use super::Terminal;

impl Terminal {}

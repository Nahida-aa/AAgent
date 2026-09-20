use std::sync::Arc;

use alacritty_terminal::vte::ansi::Color;
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Rgba, SharedString, Style, TextAlign, TextRun, Window, fill, point, px, size,
};
use terminal::TerminalBounds;
use terminal::alacritty::{AlacrittyBackend, DisplayCell};

const DEFAULT_BG: Rgba = Rgba {
    r: 0.051,
    g: 0.067,
    b: 0.090,
    a: 1.0,
};

/// Block-style cursor fill; matches the app's accent blue.
const CURSOR_COLOR: Rgba = Rgba {
    r: 0x6e as f32 / 255.0,
    g: 0xa8 as f32 / 255.0,
    b: 0xfe as f32 / 255.0,
    a: 1.0,
};

/// Renders one terminal screenful. Paints cell backgrounds as quads and each
/// visible line as a shaped line; refined run-splitting can come later.
pub struct TerminalElement {
    backend: Arc<AlacrittyBackend>,
    bounds: TerminalBounds,
}

impl TerminalElement {
    pub fn new(backend: Arc<AlacrittyBackend>, bounds: TerminalBounds) -> Self {
        Self { backend, bounds }
    }

    /// Paints the terminal cursor. Block draws the full cell (opaque), then
    /// re-draws the cell's character in the terminal background so it reads as
    /// reverse video; Underline/Beam/HollowBlock draw thin strips or outlines.
    fn paint_cursor(
        &self,
        bounds: Bounds<Pixels>,
        cells: &[DisplayCell],
        rows: usize,
        cols: usize,
        window: &mut Window,
        cx: &mut App,
    ) {
        use alacritty_terminal::vte::ansi::CursorShape;

        let Some(cursor) = self.backend.read_cursor() else {
            return;
        };
        let (row, col) = (cursor.row as usize, cursor.col);
        if row >= rows || col >= cols {
            return;
        }
        let origin = bounds.origin;
        let cell_width = px(self.bounds.cell_width);
        let line_height = px(self.bounds.line_height);
        let font_size = px(self.bounds.font_size);
        let x0 = px(col as f32 * self.bounds.cell_width);
        let y0 = px(row as f32 * self.bounds.line_height);

        let cursor_color = CURSOR_COLOR;
        match cursor.shape {
            CursorShape::Block => {
                let cell_bounds =
                    Bounds::new(origin + point(x0, y0), size(cell_width, line_height));
                window.paint_quad(fill(cell_bounds, cursor_color));

                let cell = cells[row * cols + col];
                let ch = cell.c;
                if !ch.is_whitespace() && ch != '\0' {
                    let run = TextRun {
                        len: ch.len_utf8(),
                        font: window.text_style().font(),
                        color: DEFAULT_BG.into(),
                        ..Default::default()
                    };
                    let shaped = window.text_system().shape_line(
                        SharedString::from(ch.to_string()),
                        font_size,
                        &[run],
                        None,
                    );
                    let _ = shaped.paint(
                        origin + point(x0, y0),
                        line_height,
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                }
            }
            CursorShape::Underline => {
                let underline = Bounds::new(
                    origin + point(x0, y0 + line_height - px(2.0)),
                    size(cell_width, px(2.0)),
                );
                window.paint_quad(fill(underline, cursor_color));
            }
            CursorShape::Beam => {
                let beam = Bounds::new(origin + point(x0, y0), size(px(2.0), line_height));
                window.paint_quad(fill(beam, cursor_color));
            }
            CursorShape::HollowBlock => {
                let cell_bounds =
                    Bounds::new(origin + point(x0, y0), size(cell_width, line_height));
                window.paint_quad(fill(cell_bounds, cursor_color));
                // Hollow block: invert the cell so the interior reads as the
                // terminal background with a colored border.
                window.paint_quad(fill(
                    Bounds::new(
                        origin + point(x0 + px(2.0), y0 + px(2.0)),
                        size(cell_width - px(4.0), line_height - px(4.0)),
                    ),
                    DEFAULT_BG,
                ));
            }
            CursorShape::Hidden => {}
        }
    }
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> { Some(ElementId::Name("aa-terminal".into())) }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> { None }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (
            window.request_layout(
                Style {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                None,
                cx,
            ),
            (),
        )
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Re-measure cell metrics from the actual font, then tell the backend
        // if the grid needs resizing.
        let text_system = window.text_system();
        let font = window.text_style().font();
        let font_id = text_system.resolve_font(&font);
        let cell_width = text_system
            .advance(font_id, px(self.bounds.font_size), 'm')
            .map(|size| f32::from(size.width))
            .unwrap_or(self.bounds.cell_width);
        let line_height = self.bounds.font_size * 1.35;

        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        let _ = cx;

        let new_bounds = TerminalBounds {
            cell_width,
            line_height,
            width,
            height,
            font_size: self.bounds.font_size,
        };
        if new_bounds != self.bounds {
            self.backend.set_bounds(new_bounds);
            self.bounds = new_bounds;
        }

        // Redraw whenever the pty produced output since our last paint.
        if self.backend.has_events() {
            self.backend.drain_events();
            window.refresh();
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (cells, rows, cols) = self.backend.read_cells();

        // Background of the whole terminal.
        window.paint_quad(fill(bounds, DEFAULT_BG));

        let origin = bounds.origin;
        let cell_width = px(self.bounds.cell_width);
        let line_height = px(self.bounds.line_height);
        let font_size = px(self.bounds.font_size);

        for row in 0..rows {
            let row_y = px(row as f32 * self.bounds.line_height);
            let base = row * cols;

            // Per-cell backgrounds where they differ from the default.
            for col in 0..cols {
                let cell = &cells[base + col];
                if !is_default_color(cell.bg) {
                    let x = px(col as f32 * self.bounds.cell_width);
                    let cell_bounds =
                        Bounds::new(origin + point(x, row_y), size(cell_width, line_height));
                    window.paint_quad(fill(cell_bounds, color_parts(cell.bg)));
                }
            }

            // Build the visible text of this row (strip trailing whitespace so
            // shaping doesn't waste work), picking the fg color from the first
            // cell in the row as a coarse approximation.
            let mut text = String::with_capacity(cols);
            let mut last_non_space_col = 0usize;
            for col in 0..cols {
                let c = cells[base + col].c;
                if !c.is_whitespace() {
                    last_non_space_col = col + 1;
                }
                text.push(c);
            }
            if last_non_space_col == 0 {
                continue;
            }
            // 按 **字符数** 截断（不是字节数），避免 UTF-8 多字节字符中间截断。
            // `last_non_space_col` 是终端 grid 列数，等于 char 数（我们的 cell
            // 简化版不处理 CJK 宽字符，先假设 1 cell = 1 char）。
            text = text.chars().take(last_non_space_col).collect();

            let fg = color_parts(cells[base].fg);
            let run = TextRun {
                len: text.len(),
                font: window.text_style().font(),
                color: fg.into(),
                ..Default::default()
            };

            let line =
                window
                    .text_system()
                    .shape_line(SharedString::from(text), font_size, &[run], None);
            let _ = line.paint(
                origin + point(px(0.0), row_y),
                line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }

        self.paint_cursor(bounds, &cells, rows, cols, window, cx);
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element { self }
}

fn is_default_color(c: Color) -> bool {
    matches!(
        c,
        Color::Named(alacritty_terminal::vte::ansi::NamedColor::Foreground)
            | Color::Named(alacritty_terminal::vte::ansi::NamedColor::Background)
    )
}

fn color_parts(c: Color) -> Rgba {
    let [r, g, b, a] = color_parts_u8(c);
    Rgba {
        r: r / 255.0,
        g: g / 255.0,
        b: b / 255.0,
        a: a / 255.0,
    }
}

fn color_parts_u8(c: Color) -> [f32; 4] {
    match c {
        Color::Named(named) => named_color(named),
        Color::Spec(rgb) => [rgb.r as f32, rgb.g as f32, rgb.b as f32, 255.0],
        Color::Indexed(i) => ansi_256(i),
    }
}

fn named_color(named: alacritty_terminal::vte::ansi::NamedColor) -> [f32; 4] {
    use alacritty_terminal::vte::ansi::NamedColor as NC;
    match named {
        NC::Foreground | NC::Background | NC::BrightForeground | NC::DimForeground => {
            [201.0, 209.0, 217.0, 255.0]
        }
        NC::Black | NC::BrightBlack | NC::DimBlack => [13.0, 17.0, 23.0, 255.0],
        NC::Red | NC::BrightRed | NC::DimRed => [239.0, 83.0, 80.0, 255.0],
        NC::Green | NC::BrightGreen | NC::DimGreen => [94.0, 183.0, 84.0, 255.0],
        NC::Yellow | NC::BrightYellow | NC::DimYellow => [247.0, 200.0, 106.0, 255.0],
        NC::Blue | NC::BrightBlue | NC::DimBlue => [122.0, 166.0, 218.0, 255.0],
        NC::Magenta | NC::BrightMagenta | NC::DimMagenta => [195.0, 151.0, 216.0, 255.0],
        NC::Cyan | NC::BrightCyan | NC::DimCyan => [112.0, 192.0, 190.0, 255.0],
        NC::White | NC::BrightWhite | NC::DimWhite => [234.0, 232.0, 133.0, 255.0],
        _ => [201.0, 209.0, 217.0, 255.0],
    }
}

fn ansi_256(n: u8) -> [f32; 4] {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    if (16..=231).contains(&n) {
        let v = n - 16;
        [
            LEVELS[(v / 36) as usize] as f32,
            LEVELS[((v % 36) / 6) as usize] as f32,
            LEVELS[(v % 6) as usize] as f32,
            255.0,
        ]
    } else if (232..=255).contains(&n) {
        let v = 8 + (n - 232) * 10;
        [v as f32, v as f32, v as f32, 255.0]
    } else {
        ansi_color(n)
    }
}

fn ansi_color(n: u8) -> [f32; 4] {
    let base = match n % 8 {
        0 => [13.0, 17.0, 23.0],
        1 => [239.0, 83.0, 80.0],
        2 => [94.0, 183.0, 84.0],
        3 => [247.0, 200.0, 106.0],
        4 => [122.0, 166.0, 218.0],
        5 => [195.0, 151.0, 216.0],
        6 => [112.0, 192.0, 190.0],
        _ => [234.0, 232.0, 133.0],
    };
    if n >= 8 {
        [
            (base[0] as f32).max(85.0 as f32),
            (base[1] as f32).max(85.0 as f32),
            (base[2] as f32).max(85.0 as f32),
            255.0,
        ]
    } else {
        [base[0] as f32, base[1] as f32, base[2] as f32, 255.0]
    }
}

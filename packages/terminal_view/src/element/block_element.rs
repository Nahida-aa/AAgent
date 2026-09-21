use gpui::{Bounds, Hsla, Pixels, Window, fill, point, size};
use terminal::{Cell, Color, is_app_chosen_exact_color as terminal_is_app_chosen_exact_color};

use super::layout::{BackgroundRegion, LayoutPoint};

pub(super) const BLOCK_SUBCELL_COLUMNS: i32 = 8;
pub(super) const BLOCK_SUBCELL_LINES: i32 = 24;

#[derive(Clone, Debug)]
pub struct BlockElementLayoutRect {
    point: LayoutPoint,
    num_of_columns: usize,
    num_of_lines: usize,
    color: Hsla,
}

impl BlockElementLayoutRect {
    fn new(point: LayoutPoint, num_of_columns: usize, num_of_lines: usize, color: Hsla) -> Self {
        Self {
            point,
            num_of_columns,
            num_of_lines,
            color,
        }
    }

    pub fn paint(
        &self,
        origin: GpuiPoint<Pixels>,
        dimensions: &TerminalBounds,
        window: &mut Window,
    ) {
        let subcell_width = dimensions.cell_width / BLOCK_SUBCELL_COLUMNS as f32;
        let subcell_height = dimensions.line_height / BLOCK_SUBCELL_LINES as f32;
        let position = point(
            origin.x + self.point.column as f32 * subcell_width,
            origin.y + self.point.line as f32 * subcell_height,
        );
        let size = size(
            subcell_width * self.num_of_columns as f32,
            subcell_height * self.num_of_lines as f32,
        );

        window.paint_quad(fill(Bounds::new(position, size), self.color));
    }

    pub fn line(&self) -> i32 {
        (self.point.line + self.num_of_lines as i32 - 1) / BLOCK_SUBCELL_LINES
    }
}

impl super::TerminalElement {
    /// Checks if a character is a decorative block/box-like character that should
    /// preserve its exact colors without contrast adjustment.
    ///
    /// This specifically targets characters used as visual connectors, separators,
    /// and borders where color matching with adjacent backgrounds is critical.
    /// Regular icons (git, folders, etc.) are excluded as they need to remain readable.
    ///
    /// Fixes https://github.com/zed-industries/zed/issues/34234
    pub(super) fn is_decorative_character(ch: char) -> bool {
        matches!(
            ch as u32,
            // Unicode Box Drawing and Block Elements
            0x2500..=0x257F // Box Drawing (└ ┐ ─ │ etc.)
            | 0x2580..=0x259F // Block Elements (▀ ▄ █ ░ ▒ ▓ etc.)
            | 0x25A0..=0x25FF // Geometric Shapes (■ ▶ ● etc. - includes triangular/circular separators)
            | 0x1FB00..=0x1FB3B // Symbols for Legacy Computing sextants used by terminal QR renderers

            // Private Use Area - Powerline separator symbols only
            | 0xE0B0..=0xE0B7 // Powerline separators: triangles (E0B0-E0B3) and half circles (E0B4-E0B7)
            | 0xE0B8..=0xE0BF // Powerline separators: corner triangles
            | 0xE0C0..=0xE0CA // Powerline separators: flames (E0C0-E0C3), pixelated (E0C4-E0C7), and ice (E0C8 & E0CA)
            | 0xE0CC..=0xE0D1 // Powerline separators: honeycombs (E0CC-E0CD) and lego (E0CE-E0D1)
            | 0xE0D2..=0xE0D7 // Powerline separators: trapezoid (E0D2 & E0D4) and inverted triangles (E0D6-E0D7)
        )
    }
    /// Returns the filled subcells of a sextant character as a bitmap, where
    /// bit `row * 2 + column` is set when that 2x3 subcell is filled.
    ///
    /// U+1FB00..=U+1FB3B enumerate all 2x3 fill combinations except the four
    /// that already exist as Block Elements (empty, `▌` = 0b010101,
    /// `▐` = 0b101010, and `█` = 0b111111), hence the gap adjustments.
    pub(super) fn sextant_char_to_filled_bits(ch: char) -> Option<u8> {
        let offset = (ch as u32).checked_sub(0x1FB00)?;
        if offset > 0x3B {
            return None;
        }

        Some((offset + 1 + u32::from(offset >= 20) + u32::from(offset >= 40)) as u8)
    }
    /// Returns the filled quadrants of a quadrant character as a bitmap, where
    /// bit `row * 2 + column` is set when that 2x2 subcell is filled.
    pub(super) fn quadrant_char_to_filled_bits(ch: char) -> Option<u8> {
        Some(match ch {
            '▘' => 0b0001,
            '▝' => 0b0010,
            '▖' => 0b0100,
            '▗' => 0b1000,
            '▚' => 0b1001,
            '▞' => 0b0110,
            '▛' => 0b0111,
            '▜' => 0b1011,
            '▙' => 0b1101,
            '▟' => 0b1110,
            _ => return None,
        })
    }
    /// Returns `(column, line, num_of_columns, num_of_lines)` in subcell units
    /// for block element characters that consist of a single rectangle.
    pub(super) fn block_char_to_rect(ch: char) -> Option<(i32, i32, i32, i32)> {
        let codepoint = ch as u32;
        Some(match codepoint {
            // ▀ upper half
            0x2580 => (0, 0, 8, 12),
            // ▁▂▃▄▅▆▇█ lower blocks of 1..=8 eighths
            0x2581..=0x2588 => {
                let eighths = (codepoint - 0x2580) as i32;
                (0, 24 - eighths * 3, 8, eighths * 3)
            }
            // ▉▊▋▌▍▎▏ left blocks of 7..=1 eighths
            0x2589..=0x258F => (0, 0, (0x2590 - codepoint) as i32, 24),
            // ▐ right half
            0x2590 => (4, 0, 4, 24),
            // ▔ upper eighth
            0x2594 => (0, 0, 8, 3),
            // ▕ right eighth
            0x2595 => (7, 0, 1, 24),
            _ => return None,
        })
    }
    /// Approximates the shade characters `░▒▓` with the foreground color at
    /// reduced opacity instead of the stipple patterns fonts use, trading
    /// pattern fidelity for seamless cell coverage.
    pub(super) fn shade_char_to_opacity(ch: char) -> Option<f32> {
        match ch {
            '░' => Some(0.25),
            '▒' => Some(0.5),
            '▓' => Some(0.75),
            _ => None,
        }
    }
    pub(super) fn collect_block_element_regions(
        point: LayoutPoint,
        ch: char,
        color: Hsla,
        regions: &mut Vec<BackgroundRegion>,
    ) -> bool {
        if let Some((column, line, num_of_columns, num_of_lines)) = Self::block_char_to_rect(ch) {
            Self::push_block_element_region(
                point,
                column,
                line,
                num_of_columns,
                num_of_lines,
                color,
                regions,
            );
            return true;
        }

        if let Some(filled) = Self::quadrant_char_to_filled_bits(ch) {
            for row in 0..2 {
                for column in 0..2 {
                    if filled & (1 << (row * 2 + column)) != 0 {
                        Self::push_block_element_region(
                            point,
                            column * 4,
                            row * 12,
                            4,
                            12,
                            color,
                            regions,
                        );
                    }
                }
            }
            return true;
        }

        if let Some(filled) = Self::sextant_char_to_filled_bits(ch) {
            for row in 0..3 {
                for column in 0..2 {
                    if filled & (1 << (row * 2 + column)) != 0 {
                        Self::push_block_element_region(
                            point,
                            column * 4,
                            row * 8,
                            4,
                            8,
                            color,
                            regions,
                        );
                    }
                }
            }
            return true;
        }

        if let Some(opacity) = Self::shade_char_to_opacity(ch) {
            Self::push_block_element_region(point, 0, 0, 8, 24, color.opacity(opacity), regions);
            return true;
        }

        false
    }
    pub(super) fn push_block_element_region(
        point: LayoutPoint,
        column: i32,
        line: i32,
        num_of_columns: i32,
        num_of_lines: i32,
        color: Hsla,
        regions: &mut Vec<BackgroundRegion>,
    ) {
        let start_line = point.line * BLOCK_SUBCELL_LINES + line;
        let start_col = point.column * BLOCK_SUBCELL_COLUMNS + column;
        let end_line = start_line + num_of_lines - 1;
        let end_col = start_col + num_of_columns - 1;

        // Extend the previous region when possible (e.g. runs of `█` in a QR
        // code) to keep the quadratic merge pass over a small input.
        if let Some(last_region) = regions.last_mut()
            && last_region.color == color
            && last_region.start_line == start_line
            && last_region.end_line == end_line
            && last_region.end_col + 1 == start_col
        {
            last_region.end_col = end_col;
            return;
        }

        regions.push(BackgroundRegion::with_extents(
            start_line, start_col, end_line, end_col, color,
        ));
    }
    pub(super) fn block_element_regions_to_rects(
        regions: Vec<BackgroundRegion>,
    ) -> Vec<BlockElementLayoutRect> {
        merge_background_regions(regions)
            .into_iter()
            .map(|region| {
                BlockElementLayoutRect::new(
                    LayoutPoint::new(region.start_line, region.start_col),
                    (region.end_col - region.start_col + 1) as usize,
                    (region.end_line - region.start_line + 1) as usize,
                    region.color,
                )
            })
            .collect()
    }
}

//! A frame as the flat array the page draws (#595): [`CELL_BYTES`] little-endian bytes a cell,
//! row by row.
//!
//! | bytes | what |
//! |---|---|
//! | 0..4 | the cell's character (its symbol's first `char`), or 0 for the blank second half of a wide glyph; bits 24.. are [`Style`] flags |
//! | 4..8 | foreground, `0x00RRGGBB`, or [`DEFAULT`] for the page's own |
//! | 8..12 | background, the same |
//!
//! Reverse video is applied here (foreground and background swapped), so the page never needs
//! to know about it. Every ratatui colour becomes RGB here too, so the page has one colour
//! model: the palette is true-colour already, and named and indexed colours (the few the TUI
//! uses directly) get xterm's values.

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

pub const CELL_BYTES: usize = 12;

/// "The page's default colour", for `Color::Reset`.
pub const DEFAULT: u32 = 0x0100_0000;

/// Text-style flags, in bits 24.. of a cell's first word.
pub struct Style;

impl Style {
    pub const BOLD: u32 = 1 << 24;
    pub const DIM: u32 = 1 << 25;
    pub const ITALIC: u32 = 1 << 26;
    pub const UNDERLINE: u32 = 1 << 27;
    pub const CROSSED_OUT: u32 = 1 << 28;
}

/// `buf`'s cells, encoded into `out` (cleared first).
pub fn encode(buf: &Buffer, out: &mut Vec<u8>) {
    out.clear();
    out.reserve(buf.content.len() * CELL_BYTES);
    for cell in &buf.content {
        let ch = cell.symbol().chars().next().map_or(0, u32::from);
        let mut word = ch;
        let m = cell.modifier;
        for (modifier, flag) in [
            (Modifier::BOLD, Style::BOLD),
            (Modifier::DIM, Style::DIM),
            (Modifier::ITALIC, Style::ITALIC),
            (Modifier::UNDERLINED, Style::UNDERLINE),
            (Modifier::CROSSED_OUT, Style::CROSSED_OUT),
        ] {
            if m.contains(modifier) {
                word |= flag;
            }
        }
        let (mut fg, mut bg) = (rgb(cell.fg), rgb(cell.bg));
        if m.contains(Modifier::REVERSED) {
            // A default swapped into the other slot must still mean that slot's own default
            // inverted, which the page can't know; spell both defaults out first.
            let fg_default = if fg == DEFAULT { DEFAULT_FG } else { fg };
            let bg_default = if bg == DEFAULT { DEFAULT_BG } else { bg };
            (fg, bg) = (bg_default, fg_default);
        }
        if m.contains(Modifier::HIDDEN) {
            word &= !0x00FF_FFFF;
        }
        out.extend_from_slice(&word.to_le_bytes());
        out.extend_from_slice(&fg.to_le_bytes());
        out.extend_from_slice(&bg.to_le_bytes());
    }
}

/// The page's default foreground and background (its CSS uses the same), needed here only to
/// reverse a default-coloured cell.
pub const DEFAULT_FG: u32 = 0x00D0_D0D0;
pub const DEFAULT_BG: u32 = 0x0012_1212;

/// `color` as `0x00RRGGBB`, or [`DEFAULT`].
pub fn rgb(color: Color) -> u32 {
    let pack = |r: u8, g: u8, b: u8| (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b);
    match color {
        Color::Reset => DEFAULT,
        Color::Rgb(r, g, b) => pack(r, g, b),
        Color::Indexed(i) => indexed(i),
        Color::Black => indexed(0),
        Color::Red => indexed(1),
        Color::Green => indexed(2),
        Color::Yellow => indexed(3),
        Color::Blue => indexed(4),
        Color::Magenta => indexed(5),
        Color::Cyan => indexed(6),
        Color::Gray => indexed(7),
        Color::DarkGray => indexed(8),
        Color::LightRed => indexed(9),
        Color::LightGreen => indexed(10),
        Color::LightYellow => indexed(11),
        Color::LightBlue => indexed(12),
        Color::LightMagenta => indexed(13),
        Color::LightCyan => indexed(14),
        Color::White => indexed(15),
    }
}

/// xterm's 256-colour table.
fn indexed(i: u8) -> u32 {
    const BASE: [u32; 16] = [
        0x000000, 0xCD0000, 0x00CD00, 0xCDCD00, 0x0000EE, 0xCD00CD, 0x00CDCD, 0xE5E5E5, 0x7F7F7F,
        0xFF0000, 0x00FF00, 0xFFFF00, 0x5C5CFF, 0xFF00FF, 0x00FFFF, 0xFFFFFF,
    ];
    match i {
        0..=15 => BASE[usize::from(i)],
        16..=231 => {
            let i = u32::from(i) - 16;
            let level = |v: u32| if v == 0 { 0 } else { 55 + v * 40 };
            (level(i / 36) << 16) | (level(i / 6 % 6) << 8) | level(i % 6)
        }
        232..=255 => {
            let v = 8 + (u32::from(i) - 232) * 10;
            (v << 16) | (v << 8) | v
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use ratatui::style::Style as RStyle;

    use super::*;

    #[test]
    fn a_cell_encodes_its_char_colours_and_flags() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        buf.set_string(
            0,
            0,
            "a",
            RStyle::default().fg(Color::Rgb(1, 2, 3)).bg(Color::Indexed(196)).bold(),
        );
        let mut out = Vec::new();
        encode(&buf, &mut out);
        assert_eq!(out.len(), 2 * CELL_BYTES);
        let word = |i: usize| u32::from_le_bytes(out[i * 4..i * 4 + 4].try_into().unwrap());
        assert_eq!(word(0), u32::from('a') | Style::BOLD);
        assert_eq!(word(1), 0x010203);
        assert_eq!(word(2), 0xFF0000);
        assert_eq!(word(3), u32::from(' '));
        assert_eq!(word(4), DEFAULT);
    }

    #[test]
    fn reverse_video_swaps_the_colours_spelling_out_defaults() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        buf.set_string(0, 0, "x", RStyle::default().fg(Color::Rgb(9, 9, 9)).reversed());
        let mut out = Vec::new();
        encode(&buf, &mut out);
        let word = |i: usize| u32::from_le_bytes(out[i * 4..i * 4 + 4].try_into().unwrap());
        assert_eq!(word(1), DEFAULT_BG);
        assert_eq!(word(2), 0x090909);
    }

    #[test]
    fn the_colour_cube_and_greys_match_xterm() {
        assert_eq!(indexed(16), 0x000000);
        assert_eq!(indexed(231), 0xFFFFFF);
        assert_eq!(indexed(232), 0x080808);
        assert_eq!(indexed(255), 0xEEEEEE);
    }
}

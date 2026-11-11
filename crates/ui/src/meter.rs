//! The one meter (#654): every bar in the game is drawn from the same two glyphs, `█` and `░`
//! (`#` and `.` with `--ascii`), a number of cells wide. The frontends add the colour and the
//! value after it: ten cells with the value in the settler view and on Endeavor, five in the
//! Nation roster.
//!
//! A low need shows on the meter itself: the cells wear the warning colour and a `!` sits
//! directly against them ([`flag`]), so it reads without colour and no loose word or mark floats
//! between columns.

/// A filled and an empty cell, in Unicode.
pub const FULL: char = '█';
pub const EMPTY: char = '░';
/// The same pair for `--ascii`.
pub const ASCII_FULL: char = '#';
pub const ASCII_EMPTY: char = '.';

/// The mark directly after a low meter's cells.
pub const LOW: char = '!';

/// The filled and the empty glyph.
pub fn glyphs(ascii: bool) -> (char, char) {
    if ascii { (ASCII_FULL, ASCII_EMPTY) } else { (FULL, EMPTY) }
}

/// A meter as `cells` cells: `frac` of them filled (rounded), as the filled and the empty run.
pub fn cells(frac: f32, cells: usize, ascii: bool) -> (String, String) {
    let (full, empty) = glyphs(ascii);
    let filled = ((frac.clamp(0.0, 1.0) * cells as f32).round() as usize).min(cells);
    (
        std::iter::repeat_n(full, filled).collect(),
        std::iter::repeat_n(empty, cells - filled).collect(),
    )
}

/// The one column after a meter's cells: `!` while the meter is low, else a space, so a row's
/// columns stay put either way.
pub fn flag(low: bool) -> char {
    if low { LOW } else { ' ' }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_meter_is_filled_by_the_fraction_and_the_ends_clamp() {
        assert_eq!(cells(1.0, 10, true), ("##########".into(), String::new()));
        assert_eq!(cells(0.0, 10, true), (String::new(), "..........".into()));
        assert_eq!(cells(0.23, 10, true), ("##".into(), "........".into()));
        assert_eq!(cells(0.72, 10, true).0.len(), 7);
        assert_eq!(cells(7.0, 10, true).0.len(), 10, "out of range clamps");
        assert_eq!(cells(0.4, 5, false), ("██".into(), "░░░".into()));
    }

    #[test]
    fn the_flag_is_a_bang_only_when_low() {
        assert_eq!(flag(true), '!');
        assert_eq!(flag(false), ' ');
    }
}

//! ratatui colours as egui ones. The palette is built at true colour, so nearly every colour is
//! already RGB; the few named ones a screen uses get the usual xterm values.

use eframe::egui::Color32;
use ratatui::style::Color;

/// `c` as egui draws it, or `fallback` for `Color::Reset`.
pub fn to_egui(c: Color, fallback: Color32) -> Color32 {
    match c {
        Color::Reset => fallback,
        Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
        Color::Black => Color32::from_rgb(0, 0, 0),
        Color::Red => Color32::from_rgb(205, 49, 49),
        Color::Green => Color32::from_rgb(13, 188, 121),
        Color::Yellow => Color32::from_rgb(229, 229, 16),
        Color::Blue => Color32::from_rgb(36, 114, 200),
        Color::Magenta => Color32::from_rgb(188, 63, 188),
        Color::Cyan => Color32::from_rgb(17, 168, 205),
        Color::Gray => Color32::from_rgb(204, 204, 204),
        Color::DarkGray => Color32::from_rgb(118, 118, 118),
        Color::LightRed => Color32::from_rgb(241, 76, 76),
        Color::LightGreen => Color32::from_rgb(35, 209, 139),
        Color::LightYellow => Color32::from_rgb(245, 245, 67),
        Color::LightBlue => Color32::from_rgb(59, 142, 234),
        Color::LightMagenta => Color32::from_rgb(214, 112, 214),
        Color::LightCyan => Color32::from_rgb(41, 184, 219),
        Color::White => Color32::from_rgb(229, 229, 229),
        Color::Indexed(i) => {
            let v = i.saturating_mul(10);
            Color32::from_rgb(v, v, v)
        }
    }
}

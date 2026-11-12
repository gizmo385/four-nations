//! The window's theme (#859): dark, whatever the operating system or browser prefers. The map's
//! palette (`fn_tui::color`) is drawn for a dark background, so a light egui theme around it
//! would clash.

use eframe::egui::{self, ThemePreference};

/// Pins the theme to dark: the preference is `Dark`, not `System`, so a light desktop or a
/// `prefers-color-scheme: light` page doesn't change it.
pub fn install(ctx: &egui::Context) {
    ctx.set_theme(ThemePreference::Dark);
}

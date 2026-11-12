//! The map's font. egui bundles Hack (monospace) with emoji fallbacks, which covers 70 of the 80
//! glyphs `data/tui.toml` names; the other ten (`✤ ✢ ✣ Ϟ ⌂ ♆ ✦ ɱ ☼ ✶`) need a font with wider
//! symbol coverage. DejaVu Sans Mono draws all 80, so it is compiled in as a fallback on every
//! platform (#859): the browser has no system fonts to fall back on, and a desktop's differ.
//!
//! The font is unmodified DejaVu Sans Mono 2.37 (about 340 KB) under the Bitstream Vera licence;
//! `assets/fonts/LICENSE-DejaVu.txt` holds the licence and must stay beside it.

use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

/// DejaVu Sans Mono, the symbol-capable fallback (see the module note for its licence).
pub const FALLBACK_FONT: &[u8] = include_bytes!("../assets/fonts/DejaVuSansMono.ttf");

/// egui's own fonts alone, with no fallback: what the glyph test measures the fallback against.
pub fn egui_definitions() -> FontDefinitions {
    FontDefinitions::default()
}

/// egui's own fonts with [`FALLBACK_FONT`] after Hack in the monospace family.
pub fn definitions() -> FontDefinitions {
    let mut defs = egui_definitions();
    defs.font_data.insert("fallback".to_owned(), Arc::new(FontData::from_static(FALLBACK_FONT)));
    let mono = defs.families.entry(FontFamily::Monospace).or_default();
    mono.insert(1.min(mono.len()), "fallback".to_owned());
    defs
}

pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}

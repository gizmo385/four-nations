//! The map's font. egui bundles Hack (monospace) with emoji fallbacks, which covers 70 of the 80
//! glyphs `data/tui.toml` names; the other ten (`✤ ✢ ✣ Ϟ ⌂ ♆ ✦ ɱ ☼ ✶`) need a font with wider
//! symbol coverage. The spike loads DejaVu Sans Mono from the system as a fallback (or the file
//! `FN_GUI_FONT` names) instead of committing a font file; a real frontend would bundle one with
//! `include_bytes!`.

use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

/// Where DejaVu Sans Mono usually is on Linux.
pub const SYSTEM_FALLBACK: &str = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf";

/// The fallback font's bytes, if there is one.
pub fn fallback_bytes() -> Option<Vec<u8>> {
    let path = std::env::var("FN_GUI_FONT").unwrap_or_else(|_| SYSTEM_FALLBACK.to_owned());
    std::fs::read(path).ok()
}

/// egui's own fonts with `extra` after Hack in the monospace family.
pub fn definitions(extra: Option<Vec<u8>>) -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    if let Some(bytes) = extra {
        defs.font_data.insert("fallback".to_owned(), Arc::new(FontData::from_owned(bytes)));
        let mono = defs.families.entry(FontFamily::Monospace).or_default();
        mono.insert(1.min(mono.len()), "fallback".to_owned());
    }
    defs
}

pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions(fallback_bytes()));
}

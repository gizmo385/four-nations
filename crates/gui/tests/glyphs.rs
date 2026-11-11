//! Glyph coverage of egui's bundled fonts (the map widget draws in them) for every glyph
//! `data/tui.toml` names: printed for the findings, and failing on any the monospace family
//! can't draw at all.

use eframe::egui::{self, FontFamily, FontId};

/// Every non-ASCII character inside a quoted string in `data/tui.toml`.
fn glyphs() -> Vec<char> {
    let text = include_str!("../../../data/tui.toml");
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.trim_start().starts_with('#')) {
        let mut inside = false;
        for c in line.chars() {
            if c == '"' {
                inside = !inside;
            } else if inside && !c.is_ascii() && !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}

fn missing(ctx: &egui::Context) -> Vec<char> {
    let font = FontId::new(16.0, FontFamily::Monospace);
    glyphs().into_iter().filter(|&c| !ctx.fonts_mut(|f| f.has_glyph(&font, c))).collect()
}

fn context(extra: Option<Vec<u8>>) -> egui::Context {
    let ctx = egui::Context::default();
    ctx.set_fonts(fn_gui::fonts::definitions(extra));
    let _ = ctx.run_ui(Default::default(), |_| {});
    ctx
}

/// egui's own fonts miss exactly the ten symbols `fn_gui::fonts` names: a change in either the
/// data or egui shows here.
#[test]
fn egui_alone_misses_ten_symbols() {
    let all = glyphs();
    assert!(all.len() > 20, "found the glyphs: {all:?}");
    let gone = missing(&context(None));
    println!("{} glyphs in data/tui.toml; egui's fonts alone miss {gone:?}", all.len());
    assert_eq!(gone, ['✤', '✢', '✣', 'Ϟ', '⌂', '♆', '✦', 'ɱ', '☼', '✶']);
}

/// With DejaVu Sans Mono as the fallback every glyph draws (skipped where it isn't installed).
#[test]
fn with_the_fallback_font_every_glyph_draws() {
    let Some(bytes) = fn_gui::fonts::fallback_bytes() else {
        println!("no fallback font at {}; skipped", fn_gui::fonts::SYSTEM_FALLBACK);
        return;
    };
    assert_eq!(missing(&context(Some(bytes))), Vec::<char>::new());
}

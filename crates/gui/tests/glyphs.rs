//! Glyph coverage of egui's bundled fonts (the map widget draws in them) for every glyph
//! `data/tui.toml` names: printed for the findings, and failing on any the monospace family
//! can't draw at all (#859: with the bundled fallback, none).

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

fn context(defs: egui::FontDefinitions) -> egui::Context {
    let ctx = egui::Context::default();
    ctx.set_fonts(defs);
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    ctx
}

/// egui's own fonts alone miss some of the symbols (ten at #842, eleven with `♅`): the reason
/// `fn_gui::fonts` bundles a fallback. The list is printed, not pinned, so a new glyph in the
/// data doesn't break this; the next test is the one that has to pass.
#[test]
fn egui_alone_misses_some_symbols() {
    let all = glyphs();
    assert!(all.len() > 20, "found the glyphs: {all:?}");
    let gone = missing(&context(fn_gui::fonts::egui_definitions()));
    println!("{} glyphs in data/tui.toml; egui's fonts alone miss {gone:?}", all.len());
    assert!(gone.contains(&'✤'), "egui's fonts are expected to miss ✤: {gone:?}");
}

/// With the bundled fallback font (compiled in on every platform, the browser included) every
/// glyph draws.
#[test]
fn with_the_bundled_font_every_glyph_draws() {
    assert_eq!(missing(&context(fn_gui::fonts::definitions())), Vec::<char>::new());
    // And through the entry points' own setup.
    let ctx = egui::Context::default();
    fn_gui::setup(&ctx);
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    assert_eq!(missing(&ctx), Vec::<char>::new());
}

//! The Seal style (#874): egui's visuals, spacing and type scale come from `fn_ui::theme`, and
//! switching nation re-themes the window. None of it needs a GPU.

use eframe::egui::{self, Color32, CornerRadius, Shadow, TextStyle, Theme as EguiTheme};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::style::{self, color32};
use fn_sim::{Defs, Nation};
use fn_tui::shell::Shell;
use fn_ui::theme::Theme;

fn theme_of(defs: &Defs, nation: Nation) -> Theme {
    Theme::for_nation(defs.nation(nation).color)
}

/// What `data/nations.toml` says a nation's colour is, as egui draws it.
fn nation_colour(defs: &Defs, nation: Nation) -> Color32 {
    let [r, g, b] = defs.nation(nation).color;
    Color32::from_rgb(r, g, b)
}

#[test]
fn the_visuals_are_the_themes_tokens_with_square_corners_and_no_shadows() {
    let defs = Defs::bundled();
    for nation in Nation::ALL {
        let theme = theme_of(&defs, nation);
        let v = style::visuals(&theme);
        assert!(v.dark_mode);
        assert_eq!(v.panel_fill, color32(theme.panel));
        assert_eq!(v.window_fill, color32(theme.panel));
        assert_eq!(v.extreme_bg_color, color32(theme.inset));
        assert_eq!(v.text_color(), color32(theme.text));
        assert_eq!(v.weak_text_color(), color32(theme.dim));
        assert_eq!(v.selection.bg_fill, color32(theme.select));
        assert_eq!(v.hyperlink_color, color32(theme.accent_text));

        // The active and hovered widgets are the accent with `on_accent` text.
        for w in [v.widgets.active, v.widgets.hovered] {
            assert_eq!(w.bg_fill, color32(theme.accent), "{nation:?}");
            assert_eq!(w.weak_bg_fill, color32(theme.accent), "{nation:?}");
            assert_eq!(w.fg_stroke.color, color32(theme.on_accent), "{nation:?}");
        }
        // Strokes at rest: 1 px, `accent_soft`.
        for w in [v.widgets.noninteractive, v.widgets.inactive] {
            assert_eq!(w.bg_stroke.width, 1.0);
            assert_eq!(w.bg_stroke.color, color32(theme.accent_soft), "{nation:?}");
        }
        assert_eq!(v.window_stroke.width, 1.0);
        assert_eq!(v.window_stroke.color, color32(theme.accent_soft));

        // Square and flat.
        let w = &v.widgets;
        for r in [
            w.noninteractive.corner_radius,
            w.inactive.corner_radius,
            w.hovered.corner_radius,
            w.active.corner_radius,
            w.open.corner_radius,
            v.window_corner_radius,
            v.menu_corner_radius,
        ] {
            assert_eq!(r, CornerRadius::ZERO);
        }
        assert_eq!(v.window_shadow, Shadow::NONE);
        assert_eq!(v.popup_shadow, Shadow::NONE);
    }
}

#[test]
fn the_accent_follows_nations_toml_for_every_nation() {
    let defs = Defs::bundled();
    let mut seen = Vec::new();
    for nation in Nation::ALL {
        let v = style::visuals(&theme_of(&defs, nation));
        assert_eq!(v.widgets.active.bg_fill, nation_colour(&defs, nation), "{nation:?}");
        seen.push(v.widgets.active.bg_fill);
    }
    // Four nations, four accents: the style does not collapse to one colour.
    seen.sort_by_key(|c| c.to_array());
    seen.dedup();
    assert_eq!(seen.len(), Nation::ALL.len());
}

#[test]
fn the_type_scale_is_20_18_14_12_in_monospace() {
    let s = style::style(&theme_of(&Defs::bundled(), Nation::Earth));
    let size = |t: &TextStyle| s.text_styles[t].size;
    assert_eq!(size(&TextStyle::Heading), 20.0);
    assert_eq!(size(&style::title_style()), 18.0);
    assert_eq!(size(&TextStyle::Body), 14.0);
    assert_eq!(size(&TextStyle::Button), 14.0);
    assert_eq!(size(&TextStyle::Small), 12.0);
    for font in s.text_styles.values() {
        assert_eq!(font.family, egui::FontFamily::Monospace);
    }
}

#[test]
fn the_spacing_is_on_the_4_6_8_12_16_steps() {
    let steps = style::STEPS;
    assert_eq!(steps, [4.0, 6.0, 8.0, 12.0, 16.0]);
    let sp = style::spacing();
    let on_steps = |v: f32| steps.contains(&v);
    assert!(on_steps(sp.item_spacing.x) && on_steps(sp.item_spacing.y));
    assert!(on_steps(sp.button_padding.x) && on_steps(sp.button_padding.y));
    assert!(on_steps(sp.indent));
    for m in [sp.window_margin, sp.menu_margin] {
        for side in [m.left, m.right, m.top, m.bottom] {
            assert!(on_steps(f32::from(side)), "{side}");
        }
    }
}

fn harness(app: &mut GuiApp) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 960.0))
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(3);
    h
}

fn game(nation: Nation) -> Shell {
    Shell::new_game(fn_tui::new_session(7, nation), (120, 40), None)
}

/// The window's active-widget fill, as the context holds it.
fn active_fill(h: &Harness<'_>) -> Color32 {
    h.ctx.style_of(EguiTheme::Dark).visuals.widgets.active.bg_fill
}

#[test]
fn the_window_wears_the_nations_style() {
    let defs = Defs::bundled();
    for nation in Nation::ALL {
        let mut app = GuiApp::with_shell(game(nation));
        let h = harness(&mut app);
        assert_eq!(active_fill(&h), nation_colour(&defs, nation), "{nation:?}");
        // The root `Ui` has it too, from the first frame.
        assert_eq!(h.ctx.global_style().visuals.selection.bg_fill.a(), 255);
    }
}

#[test]
fn switching_nation_in_the_game_re_themes_the_window() {
    let defs = Defs::bundled();
    let mut app = GuiApp::with_shell(game(Nation::Earth));
    let mut h = harness(&mut app);
    assert_eq!(active_fill(&h), nation_colour(&defs, Nation::Earth));

    // Esc opens the menu; its Switch nation… row opens the list; Water is the second nation in
    // the list; Enter asks, and the second answer is
    // "Start over without saving".
    let keys = |h: &mut Harness<'_>, ks: &[egui::Key]| {
        for k in ks {
            h.key_press(*k);
            h.run_steps(2);
        }
    };
    use egui::Key::{ArrowDown, Enter, Escape};
    let to = Nation::ALL.iter().position(|n| *n == Nation::Water).unwrap();
    keys(&mut h, &[Escape]);
    keys(&mut h, &[ArrowDown; 5]);
    keys(&mut h, &[Enter]);
    keys(&mut h, &vec![ArrowDown; to]);
    keys(&mut h, &[Enter]);
    h.get_by_label_contains("Start over as the");
    keys(&mut h, &[ArrowDown, Enter]);
    h.run_steps(3);
    // The same window, now Water's.
    assert_eq!(active_fill(&h), nation_colour(&defs, Nation::Water));
    assert_ne!(nation_colour(&defs, Nation::Water), nation_colour(&defs, Nation::Earth));
    drop(h);
    assert_eq!(app.shell().nation(), Nation::Water);
}

#[test]
fn headings_wear_the_diamond_and_the_panel_the_seal() {
    let mut app = GuiApp::with_shell(game(Nation::Fire));
    let h = harness(&mut app);
    // The seal on the panel's top edge, and a diamond beside each section heading.
    h.get_by_label_contains("◆ FIRE NATION ◆");
    assert!(h.query_all_by_label("◇").count() >= 2);
    // The heading's words are still a label of their own, with its key flush right.
    h.get_by_label("SETTLEMENT");
    h.get_by_label("[N]");
}

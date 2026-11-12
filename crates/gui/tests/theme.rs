//! The window is dark by default, whatever the system prefers (#859).

use eframe::egui::{self, Theme, ThemePreference};

#[test]
fn setup_pins_the_dark_theme() {
    let ctx = egui::Context::default();
    // A light system: egui would follow it while the preference is `System`.
    ctx.set_theme(ThemePreference::Light);
    fn_gui::setup(&ctx);
    assert_eq!(ctx.options(|o| o.theme_preference), ThemePreference::Dark);
    let _ = ctx.run_ui(Default::default(), |_| {});
    assert_eq!(ctx.theme(), Theme::Dark);
    assert!(ctx.global_style().visuals.dark_mode);
}

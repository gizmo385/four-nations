//! `cargo run -p fn_gui [-- SEED NATION ...]`: the game in a window (#842's spike). Takes
//! `fn_tui`'s own arguments (no arguments for the title screen) plus `--keys SCRIPT` to play
//! before the window opens; `F12` hides the egui panel.

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Four Nations")
            .with_inner_size([1600.0, 960.0]),
        ..Default::default()
    };
    let saves = fn_ui::saves::default_save_dir();
    let seed_hint = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    eframe::run_native(
        "Four Nations",
        options,
        Box::new(move |cc| {
            fn_gui::fonts::install(&cc.egui_ctx);
            Ok(Box::new(fn_gui::app::GuiApp::new(saves, seed_hint)))
        }),
    )
}

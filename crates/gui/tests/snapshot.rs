//! A snapshot of the map widget through `egui_kittest` (#842's question: does it run headless?).
//! It renders with wgpu, so it needs an adapter: a GPU, or a software one such as Mesa's lavapipe
//! (`mesa-vulkan-drivers` on Debian and Ubuntu). Without one `egui_kittest` panics building the
//! harness ("No adapter found"), which is why `fn_gui` stays out of `cargo xtask ci`.

use eframe::egui::{self, Sense};
use fn_gui::camera::Camera;
use fn_gui::mapview;
use fn_sim::Nation;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};

#[test]
fn the_map_widget_renders_headless() {
    let app = App::new(fn_tui::new_session(7, Nation::Earth), (120, 40));
    let palette = Palette::new(&app.session.defs, Nation::Earth, ColorDepth::TrueColor, false);
    let camera = Camera::new(app.ui.cursor, 18.0);
    let mut harness =
        egui_kittest::Harness::builder().with_size(egui::vec2(640.0, 400.0)).build_ui(|ui| {
            let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
            mapview::paint(ui, rect, &camera, &app, &palette);
        });
    harness.run();
    match harness.render() {
        Ok(image) => {
            assert_eq!((image.width(), image.height()), (640, 400));
            let dir = std::env::temp_dir().join("fn_gui_snapshot.png");
            image.save(&dir).unwrap();
            println!("rendered headless to {}", dir.display());
        }
        Err(e) => panic!("the harness has an adapter but could not render: {e}"),
    }
}

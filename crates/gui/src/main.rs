//! `cargo run -p fn_gui [-- SEED NATION ...]`: the game in a window (#842's spike). Takes
//! `fn_tui`'s own arguments (no arguments for the title screen) plus `--keys SCRIPT` to play
//! before the window opens; `F12` hides the egui panel.
//!
//! Built for wasm32 (#843's check of egui in the browser), the same app runs on a page's canvas
//! instead: a game at the default site of the seed, nation and map size in the page's query
//! (`?seed=7&nation=2&size=0`). See `web/gui/index.html`.

#[cfg(target_arch = "wasm32")]
fn main() {
    wasm_bindgen_futures::spawn_local(web::start());
}

#[cfg(target_arch = "wasm32")]
mod web {
    use eframe::wasm_bindgen::JsCast;
    use fn_tui::keyev::{KeyCode, KeyEvent, KeyModifiers};
    use fn_tui::shell::Shell;

    fn query(name: &str) -> Option<u64> {
        let search = web_sys::window()?.location().search().ok()?;
        search.trim_start_matches('?').split('&').find_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            (k == name).then(|| v.parse().ok()).flatten()
        })
    }

    pub async fn start() {
        let document = web_sys::window().and_then(|w| w.document()).expect("a page");
        let canvas = document
            .get_element_by_id("the_canvas")
            .expect("a canvas with id the_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("a canvas");
        let seed = query("seed").unwrap_or(7);
        let nation = fn_sim::Nation::ALL[query("nation").unwrap_or(2) as usize % 4];
        let defs = fn_sim::Defs::bundled();
        let size = defs.embark.map_sizes[query("size").unwrap_or(0) as usize % 3].clone();
        let mut shell = Shell::new_embark(
            seed,
            nation,
            size,
            fn_sim::embark::SurveyMode::Exact,
            None,
            (120, 40),
            None,
        );
        for c in ['e', 'y'] {
            shell.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    fn_gui::setup(&cc.egui_ctx);
                    Ok(Box::new(fn_gui::app::GuiApp::with_shell(shell)))
                }),
            )
            .await;
        if let Err(e) = result {
            web_sys::console::error_1(&e);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
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
            fn_gui::setup(&cc.egui_ctx);
            Ok(Box::new(fn_gui::app::GuiApp::new(saves, seed_hint)))
        }),
    )
}

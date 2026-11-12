//! `cargo run -p fn_gui [-- SEED NATION ...]`: the game in a window (#842's spike). Takes
//! `fn_tui`'s own arguments (no arguments for the title screen) plus `--keys SCRIPT` to play
//! before the window opens; `F12` shows the dev timing overlay.
//!
//! Built for wasm32 (#843's check of egui in the browser), the same app runs on a page's canvas
//! instead (`web/play/index.html`, published at `/play/`, #866): the title screen, as the desktop
//! app opens with no arguments. A query with a `seed` skips it for a game at the default site of
//! that seed, `nation` and map size (`?seed=7&nation=2&size=0`), which is how the page's scripts
//! start a fixed game. The page can't quit, so `Shell::with_quit(false)` hides Quit; there is no
//! runs folder, so no Replays (the replayer is its own page). Saves there go to the browser's
//! `localStorage` (`fn_gui::browser_saves`); natively, to the save directory as in `fn_tui`.

#[cfg(target_arch = "wasm32")]
fn main() {
    wasm_bindgen_futures::spawn_local(web::start());
}

#[cfg(target_arch = "wasm32")]
mod web {
    use eframe::wasm_bindgen::JsCast;
    use fn_tui::shell::Shell;
    use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};

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
        fn_gui::webpage::guard_game_keys(&canvas);
        let saves = Some(fn_gui::browser_saves::local::store());
        let shell = match query("seed") {
            // A fixed game, for the scripts that drive the page (`web/play/check-*.cjs`).
            Some(seed) => {
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
                    saves,
                );
                for c in ['e', 'y'] {
                    shell.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
                }
                shell
            }
            // Otherwise the title screen, offering a fresh seed each visit. This is the one
            // place the wall clock is read, as `main` does natively.
            None => {
                let seed_hint = web_time::SystemTime::now()
                    .duration_since(web_time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos() as u64);
                Shell::new_title((120, 40), saves, seed_hint)
            }
        }
        // Nothing to quit to in a browser tab (#866).
        .with_quit(false);
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
    let saves = fn_ui::saves::DiskStore::default_shared();
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

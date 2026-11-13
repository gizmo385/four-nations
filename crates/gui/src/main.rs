//! `cargo run -p fn_gui [-- SEED NATION ...]`: the game in a window (#842's spike). Takes
//! `fn_tui`'s own arguments (no arguments for the title screen) plus `--keys SCRIPT` to play
//! before the window opens; `F12` shows the dev timing overlay. `--replay LOG` replays a run log
//! (`run.jsonl`) instead (#997).
//!
//! Built for wasm32 (#843's check of egui in the browser), the same app runs on a page's canvas
//! instead (`web/play/index.html`, published at `/play/`, #866): the title screen, as the desktop
//! app opens with no arguments. A query with a `seed` skips it for a game at the default site of
//! that seed, `nation` and map size (`?seed=7&nation=2&size=0`), which is how the page's scripts
//! start a fixed game. The page can't quit, so `Shell::with_quit(false)` hides Quit; there is no
//! runs folder, so no Replays. The replay page (`web/index.html`, the site's root) runs this same
//! wasm on a canvas marked `data-app="replay"`: a run log replayed (`fn_gui::replay`, #999). Saves there go to the browser's
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

    /// The replay page (`web/index.html`, #999) hands a log it read (its file picker, its run
    /// list) to the replay here; a `.jsonl.gz` is unpacked on this side.
    #[eframe::wasm_bindgen::prelude::wasm_bindgen]
    pub fn fn_open_log(name: String, bytes: Vec<u8>) {
        fn_gui::replay::arrived(name, Ok(bytes));
    }

    pub async fn start() {
        let document = web_sys::window().and_then(|w| w.document()).expect("a page");
        let canvas = document
            .get_element_by_id("the_canvas")
            .expect("a canvas with id the_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("a canvas");
        // The replay page's canvas says so (#999): the same wasm replays a run log there.
        if canvas.get_attribute("data-app").as_deref() == Some("replay") {
            let result = eframe::WebRunner::new()
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(|cc| {
                        fn_gui::setup(&cc.egui_ctx);
                        Ok(Box::new(fn_gui::replay::ReplayPage::default()))
                    }),
                )
                .await;
            if let Err(e) = result {
                web_sys::console::error_1(&e);
            }
            return;
        }
        fn_gui::webpage::guard_game_keys(&canvas);
        fn_gui::savefiles::install(&canvas);
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
    // `--replay LOG` (#997): a run log replayed in the window instead of a game.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, path] = args.as_slice()
        && flag == "--replay"
    {
        return replay(path);
    }
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

/// `--replay LOG`: the window replays `path`, a `run.jsonl`.
#[cfg(not(target_arch = "wasm32"))]
fn replay(path: &str) -> eframe::Result {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("fn_gui: {path}: {e}");
        std::process::exit(2)
    });
    let app = fn_gui::replay::ReplayApp::open(&text).unwrap_or_else(|e| {
        eprintln!("fn_gui: {path}: {e}");
        std::process::exit(2)
    });
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Four Nations replay")
            .with_inner_size([1600.0, 960.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Four Nations replay",
        options,
        Box::new(move |cc| {
            fn_gui::setup(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

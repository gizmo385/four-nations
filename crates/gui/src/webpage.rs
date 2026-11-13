//! The page around the canvas, on the web (#860): the two things the game does to it that are not
//! drawing.
//!
//! - [`guard_game_keys`]: `F5` is the quicksave and `F6` the saves screen, but they are also the
//!   browser's reload and address-bar keys, and `eframe` never takes them from the browser (it
//!   leaves `F5` alone on purpose). While the canvas has focus, those two (and `F9`, quickload)
//!   go to the game alone. `Ctrl`/`Cmd`/`Alt` with them, and any focus elsewhere on the page,
//!   still reach the browser, so a hard reload (`Ctrl+Shift+R`) works.
//! - [`publish`]: the game's state as `data-fn-*` attributes on the canvas, for a test or a
//!   script driving the page (`web/play/check-saves.cjs`) to read, since the game is pixels:
//!   `data-fn-mode` (`title`, `embark` or `game`), and in a game `data-fn-tick`, `data-fn-seed`
//!   and `data-fn-status` (the status line, where saves and loads say what they did); on the
//!   title screen `data-fn-page` names its step (`menu`, `nation`, `seed` or `load`).

use eframe::wasm_bindgen::JsCast;
use eframe::wasm_bindgen::prelude::Closure;
use fn_tui::shell::{Mode, Shell};
use fn_tui::title::Page;

/// The keys [`guard_game_keys`] keeps from the browser.
const GAME_KEYS: [&str; 3] = ["F5", "F6", "F9"];

/// Keeps `F5`, `F6` and `F9` from the browser while `canvas` has focus. Call once at start.
pub fn guard_game_keys(canvas: &web_sys::HtmlCanvasElement) {
    let Some(window) = web_sys::window() else { return };
    let canvas: web_sys::EventTarget = canvas.clone().into();
    let handler =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
            let on_canvas = event.target().is_some_and(|t| t == canvas);
            let chord = event.ctrl_key() || event.meta_key() || event.alt_key();
            if on_canvas && !chord && GAME_KEYS.contains(&event.key().as_str()) {
                event.prevent_default();
            }
        });
    // Capture phase: before anything on the page can act on the key first.
    let _ = window.add_event_listener_with_callback_and_bool(
        "keydown",
        handler.as_ref().unchecked_ref(),
        true,
    );
    // The listener lives as long as the page.
    handler.forget();
}

/// Writes `shell`'s state onto the canvas element (`#the_canvas`), when it changed since `last`.
pub fn publish(shell: &Shell, last: &mut String) {
    let state = match &shell.mode {
        Mode::Title(title) => vec![
            ("mode", "title".to_owned()),
            (
                "page",
                match title.page {
                    Page::Menu => "menu",
                    Page::Nation { .. } => "nation",
                    Page::Seed { .. } => "seed",
                    Page::Load => "load",
                }
                .to_owned(),
            ),
        ],
        Mode::Embark(_) => vec![("mode", "embark".to_owned())],
        Mode::Game(app) => vec![
            ("mode", "game".to_owned()),
            ("tick", app.session.world.tick.to_string()),
            ("seed", app.session.world.seed.to_string()),
            ("status", app.ui.status.clone()),
        ],
    };
    let summary = format!("{state:?}");
    if summary == *last {
        return;
    }
    let Some(canvas) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("the_canvas"))
    else {
        return;
    };
    for name in ["page", "tick", "seed", "status"] {
        let _ = canvas.remove_attribute(&format!("data-fn-{name}"));
    }
    for (name, value) in state {
        let _ = canvas.set_attribute(&format!("data-fn-{name}"), &value);
    }
    *last = summary;
}

/// The screen's safe-area insets (#974): the CSS `env(safe-area-inset-*)` that `index.html`
/// puts in `--fn-safe-top` and the rest, in CSS pixels, which are egui's points. Zero where the
/// page has none (a desktop browser) or can't be read.
pub fn safe_area() -> crate::frame::SafeArea {
    let read = || -> Option<crate::frame::SafeArea> {
        let window = web_sys::window()?;
        let root = window.document()?.document_element()?;
        let style = window.get_computed_style(&root).ok()??;
        let px = |name: &str| {
            let value = style.get_property_value(name).unwrap_or_default();
            value.trim().trim_end_matches("px").parse::<f32>().unwrap_or(0.0).max(0.0)
        };
        Some(crate::frame::SafeArea {
            top: px("--fn-safe-top"),
            right: px("--fn-safe-right"),
            bottom: px("--fn-safe-bottom"),
            left: px("--fn-safe-left"),
        })
    };
    read().unwrap_or_default()
}

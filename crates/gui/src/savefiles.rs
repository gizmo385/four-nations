//! Saves as files in the browser (#976, docs/design/touch-pwa.md §2): an installed iOS web app's
//! storage is its own and can be cleared, so the saves screen can export a slot as a
//! `NAME.fnsave` download and import one back.
//!
//! A browser opens a file picker (and, in Safari, starts a download) only inside a user gesture,
//! and egui runs its frame later, from `requestAnimationFrame`. So the saves screen's buttons
//! [`arm`] an action while the finger or mouse is down on them, and a `pointerup` / `touchend`
//! listener on the canvas ([`install`]) carries it out inside the gesture. The outcome comes back
//! as a status line through [`take_outcome`], which the window hands the game each frame.
//!
//! The slot logic ([`crate::browser_saves::export_value`], [`crate::browser_saves::import_value`])
//! builds on every target and is tested natively; this module is the page's part.

/// What a press on one of the saves screen's file buttons does once it lifts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Download slot `name` as a file.
    Export(String),
    /// Open a file picker and add the chosen file as a slot.
    Import,
}

/// A file action's result, for the status line: what happened, and whether the slot list
/// changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub status: String,
    pub changed: bool,
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;

    use eframe::wasm_bindgen::JsCast;
    use eframe::wasm_bindgen::prelude::Closure;

    use super::{Action, Outcome};
    use crate::browser_saves::{self, local::LocalStorage};

    thread_local! {
        static TARGETS: RefCell<String> = const { RefCell::new(String::new()) };
        static ARMED: RefCell<Option<Action>> = const { RefCell::new(None) };
        static OUTCOME: RefCell<Option<Outcome>> = const { RefCell::new(None) };
    }

    pub fn set_targets(targets: String) {
        TARGETS.with(|t| *t.borrow_mut() = targets);
    }

    pub fn targets() -> String {
        TARGETS.with(|t| t.borrow().clone())
    }

    pub fn arm(action: Option<Action>) {
        ARMED.with(|a| *a.borrow_mut() = action);
    }

    pub fn take_outcome() -> Option<Outcome> {
        OUTCOME.with(|o| o.borrow_mut().take())
    }

    fn finish(status: String, changed: bool) {
        OUTCOME.with(|o| *o.borrow_mut() = Some(Outcome { status, changed }));
    }

    /// Listens on `canvas` for the end of a press, and carries out what is armed then.
    pub fn install(canvas: &web_sys::HtmlCanvasElement) {
        let handler = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
            if let Some(action) = ARMED.with(|a| a.borrow_mut().take()) {
                run(action);
            }
        });
        for event in ["pointerup", "touchend"] {
            let _ = canvas.add_event_listener_with_callback_and_bool(
                event,
                handler.as_ref().unchecked_ref(),
                true,
            );
        }
        handler.forget();
    }

    fn run(action: Action) {
        let result = match action {
            Action::Export(name) => export(&name).map(|file| (format!("Exported {file}"), false)),
            Action::Import => import().map(|()| (String::new(), false)),
        };
        match result {
            Ok((status, changed)) if !status.is_empty() => finish(status, changed),
            Ok(_) => {}
            Err(why) => finish(why, false),
        }
    }

    fn document() -> Result<web_sys::Document, String> {
        web_sys::window().and_then(|w| w.document()).ok_or_else(|| "no page".to_owned())
    }

    /// Downloads slot `name` as `name.fnsave`, the bytes `localStorage` holds. Returns the file
    /// name.
    fn export(name: &str) -> Result<String, String> {
        let value = browser_saves::export_value(&LocalStorage, name)?;
        let file = browser_saves::export_file_name(name);
        let parts = js_sys::Array::of1(&value.into());
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("application/octet-stream");
        let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options)
            .map_err(|e| format!("could not export: {e:?}"))?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|e| format!("could not export: {e:?}"))?;
        let link: web_sys::HtmlAnchorElement = document()?
            .create_element("a")
            .map_err(|e| format!("could not export: {e:?}"))?
            .unchecked_into();
        link.set_href(&url);
        link.set_download(&file);
        link.click();
        // The download has its own copy once started; the URL can go after this task.
        let revoke = Closure::once_into_js(move || {
            let _ = web_sys::Url::revoke_object_url(&url);
        });
        if let Some(w) = web_sys::window() {
            let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(
                revoke.unchecked_ref(),
                10_000,
            );
        }
        Ok(file)
    }

    /// Opens a file picker; the chosen file is read and added as a slot later.
    fn import() -> Result<(), String> {
        let input: web_sys::HtmlInputElement = document()?
            .create_element("input")
            .map_err(|e| format!("could not open a file: {e:?}"))?
            .unchecked_into();
        input.set_type("file");
        input.set_accept(&format!(".{}", browser_saves::FILE_EXTENSION));
        let picked = input.clone();
        let on_change = Closure::once_into_js(move || {
            let Some(file) = picked.files().and_then(|f| f.get(0)) else { return };
            let name = file.name();
            wasm_bindgen_futures::spawn_local(async move {
                let read = wasm_bindgen_futures::JsFuture::from(file.text()).await;
                let outcome = match read.ok().and_then(|t| t.as_string()) {
                    None => Err(format!("could not read {name}")),
                    Some(text) => browser_saves::import_value(&LocalStorage, &name, &text),
                };
                match outcome {
                    Ok(slot) => finish(format!("Imported {name} as {slot}"), true),
                    Err(why) => finish(why, false),
                }
            });
        });
        input.set_onchange(Some(on_change.unchecked_ref()));
        input.click();
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::{arm, install, set_targets, take_outcome, targets};

/// Where the file buttons were drawn last frame, `export X Y import X Y` in CSS pixels (empty when
/// they weren't), for a script driving the page.
pub fn describe_targets(export: eframe::egui::Pos2, import: eframe::egui::Pos2) -> String {
    format!("export {} {} import {} {}", export.x, export.y, import.x, import.y)
}

/// Off the web there are no file buttons; nothing is ever armed.
#[cfg(not(target_arch = "wasm32"))]
pub fn arm(_: Option<Action>) {}

/// Off the web there are no targets.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_targets(_: String) {}

/// Off the web nothing ever comes back.
#[cfg(not(target_arch = "wasm32"))]
pub fn take_outcome() -> Option<Outcome> {
    None
}

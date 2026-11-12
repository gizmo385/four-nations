//! A spike (#842): the game in an egui window. It drives the same [`fn_tui::shell::Shell`] the
//! terminal loop drives (`key`, `advance`, `resize`, draw), so every screen works as it does in
//! the terminal, with three additions this spike exists to price:
//!
//! - **The map** is a widget of its own ([`mapview`]): the per-tile look `fn_tui::map::tile_cell`
//!   computes, painted as glyphs in egui's bundled monospace font, with pan by drag, zoom by the
//!   wheel and, zoomed far out, one coloured block per tile. A click moves the cursor, and a drag
//!   with a designate tool active makes the rectangle (`fn_ui::pointer`).
//! - **Everything else** is the ratatui buffer the terminal would show, painted as egui shapes
//!   by the cell painter ([`cells`], called from [`app`]).
//! - **One real panel**: the side panel's header and People rows as egui widgets fed from
//!   `fn_ui` view models ([`panel`]).
//!
//! Nothing here reads the clock but [`app`]'s frame time, and nothing here is in `cargo xtask ci`.

pub mod app;
pub mod browser_saves;
pub mod camera;
pub mod cells;
pub mod color;
pub mod fonts;
pub mod keys;
pub mod mapview;
#[cfg(target_arch = "wasm32")]
pub mod page;
pub mod panel;
pub mod theme;

/// What a new window or page sets up once: the bundled fonts ([`fonts`]) and the dark theme
/// ([`theme`]). Both entry points (`main.rs`, native and web) call it.
pub fn setup(ctx: &eframe::egui::Context) {
    fonts::install(ctx);
    theme::install(ctx);
}

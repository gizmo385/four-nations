//! The game in an egui window (docs/design/egui-frontend.md). It drives the same
//! [`fn_tui::shell::Shell`] the terminal loop drives (`key`, `advance`, `resize`, draw), so every
//! screen works as it does in the terminal until it has a native painter:
//!
//! - **The frame** ([`frame`]) decides, each frame, whether what the shell shows is native, cells
//!   over the whole window or cells in one box, and lays the native game out: the hint bar
//!   ([`hints`]), the side panel, the map and the tool banner.
//! - **The map** is a widget of its own ([`mapview`]): the per-tile look `fn_tui::map::tile_cell`
//!   computes, painted as glyphs in egui's bundled monospace font, with pan by drag, zoom by the
//!   wheel and, zoomed far out, one coloured block per tile. A click moves the cursor, and a drag
//!   with a designate tool active makes the rectangle (`fn_ui::pointer`).
//! - **Cells** ([`cells`]) paint a ratatui buffer as egui shapes: the unconverted screens, the
//!   title and embark, boxes, and the side panel's slot until it is native.
//!
//! Nothing here reads the clock but [`app`]'s frame time.

pub mod app;
pub mod browser_saves;
pub mod camera;
pub mod cells;
pub mod color;
pub mod fonts;
pub mod frame;
pub mod hints;
pub mod keys;
pub mod mapview;
#[cfg(target_arch = "wasm32")]
pub mod page;
pub mod theme;

/// What a new window or page sets up once: the bundled fonts ([`fonts`]) and the dark theme
/// ([`theme`]). Both entry points (`main.rs`, native and web) call it.
pub fn setup(ctx: &eframe::egui::Context) {
    fonts::install(ctx);
    theme::install(ctx);
}

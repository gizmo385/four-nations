//! The game in an egui window (docs/design/egui-frontend.md). It drives the same
//! [`fn_tui::shell::Shell`] the terminal loop drives (`key`, `advance`, `resize`, draw), so every
//! screen works as it does in the terminal until it has a native painter:
//!
//! - **The frame** ([`frame`]) decides, each frame, whether what the shell shows is native, cells
//!   over the whole window or cells in one box, and lays the native game out: the hint bar
//!   ([`hints`]), the side panel, the map and the tool banner.
//! - **The map** is a widget of its own ([`mapview`]): the per-tile look `fn_tui::map::tile_cell`
//!   computes, painted as glyphs in egui's bundled monospace font, with pan by drag, zoom by the
//!   wheel and, zoomed far out, a cached one-pixel-per-tile texture ([`overview`]). Tiles are
//!   square. Clicks, drags and right clicks are the keys they stand for (`fn_ui::pointer`, #878),
//!   and hovering a tile shows its tooltip.
//! - **The side panel** ([`side`]) is `fn_ui::panel::SidePanel` as egui widgets; its clicks are
//!   the keys they stand for. Its inspector ([`inspector`], #882) is native too.
//! - **The title** ([`title`]) is native: the steps of `fn_tui::title` as egui rows and buttons,
//!   every click a key press.
//! - **Pages** ([`page`], [`screens`]): the full screens as egui widgets in one frame, over the
//!   whole window above the hint bar. Build (#881), Research (#884), Abilities and Traders (#886) so far.
//! - **Embark** ([`embark`]) is native: the survey on square tiles and the site report as a side
//!   panel (`fn_ui::embark`'s rows); a click on the map is the arrow keys to that block.
//! - **Boxes** ([`boxes`]): the game menu, the confirm boxes and embark's exit prompt as
//!   `egui::Modal`s over whatever is showing (#879).
//! - **Cells** ([`cells`]) paint a ratatui buffer as egui shapes: the unconverted screens.
//!
//! Nothing here reads the clock but [`app`]'s frame time.

pub mod app;
pub mod boxes;
pub mod browser_saves;
pub mod camera;
pub mod cells;
pub mod color;
pub mod embark;
pub mod fonts;
pub mod frame;
pub mod hints;
pub mod inspector;
pub mod keys;
pub mod mapview;
pub mod overview;
pub mod page;
pub mod screens;
pub mod side;
pub mod style;
pub mod theme;
pub mod title;
#[cfg(target_arch = "wasm32")]
pub mod webpage;

/// What a new window or page sets up once: the bundled fonts ([`fonts`]) and the dark theme
/// ([`theme`]). Both entry points (`main.rs`, native and web) call it. The Seal style
/// ([`style`]) follows the nation, so `GuiApp` installs it, and again when the nation changes.
pub fn setup(ctx: &eframe::egui::Context) {
    fonts::install(ctx);
    theme::install(ctx);
}

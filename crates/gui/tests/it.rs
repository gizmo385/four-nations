//! `fn_gui`'s tests, as one binary (the workspace's convention). Nothing here opens a window.

mod browser_saves;
mod camera;
mod cells;
mod frame;
mod glyphs;
mod inspector;
mod keys;
mod page;
mod semantic;
mod side;
#[cfg(feature = "shots")]
mod snapshot;
mod theme;
mod title;

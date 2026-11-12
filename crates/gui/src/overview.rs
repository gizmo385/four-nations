//! The map's overview, below [`crate::camera::GLYPH_MIN`] pixels a tile (#878): one pixel per
//! tile in a texture, drawn stretched over the map. The block-per-tile painter this replaces
//! looked up 98 000 tiles and added a rectangle for each every frame (24 ms on the largest map);
//! a texture is one quad, and is rebuilt only when the world changes.
//!
//! The texture is the world as `fn_tui::map::cell_look` shows it with nothing of the player's
//! own on it: no cursor, rectangle, ghost or selection (a neutral [`UiState`]). Those are drawn
//! over the texture each frame as a few shapes ([`crate::mapview`]), so moving the cursor or
//! dragging a rectangle never rebuilds it.
//!
//! What the world changed is read off a [`Stamp`]: the tick, the length of the command log (a
//! designation or a building placed while paused is a command, and changes the map without a
//! tick), and the world's seed and size (a new game).

use eframe::egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use fn_sim::Pos;
use fn_tui::color::Palette;
use fn_tui::map::{Overlays, Thing, cell_look, style_cell};
use fn_ui::session::Session;
use fn_ui::state::UiState;

use crate::color::to_egui;

/// What the overview texture was built from: when this changes, the map has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    seed: u64,
    tick: u64,
    commands: usize,
    size: (i32, i32),
}

impl Stamp {
    pub fn of(session: &Session) -> Self {
        let world = &session.world;
        Self {
            seed: world.seed,
            tick: world.tick,
            commands: world.command_log().len(),
            size: (world.map.width(), world.map.height()),
        }
    }
}

/// The colour a tile shows in the overview: its background, or, for something standing on it, its
/// foreground, so units, buildings and designations stay visible from far out.
pub fn block_color(thing: &Thing, bg: Color32, fg: Color32) -> Color32 {
    if *thing != Thing::Ground { fg } else { bg }
}

/// One pixel per tile of `session`'s world, row by row, as the overview shows it.
pub fn image(session: &Session, palette: &Palette) -> ColorImage {
    let (w, h) = (session.world.map.width(), session.world.map.height());
    let mut neutral = UiState::new(session, (80, 24));
    // Off the map, so no tile wears the cursor.
    neutral.cursor = Pos::new(-1, -1);
    let over = Overlays::gather(session, &neutral);
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let pixel = match cell_look(session, &neutral, palette, &over, Pos::new(x, y)) {
                Some(look) => {
                    let cell = style_cell(&look, &session.defs, palette);
                    let bg = to_egui(cell.style.bg.unwrap_or_default(), Color32::BLACK);
                    let fg = to_egui(cell.style.fg.unwrap_or_default(), Color32::WHITE);
                    block_color(&look.thing, bg, fg)
                }
                None => Color32::BLACK,
            };
            pixels.push(pixel);
        }
    }
    ColorImage::new([w as usize, h as usize], pixels)
}

/// The cached overview texture.
#[derive(Default)]
pub struct Overview {
    texture: Option<(Stamp, TextureHandle)>,
    /// How many times the texture has been built, for the tests and the F12 overlay.
    pub builds: u32,
}

impl Overview {
    /// The texture for `session`'s world as it is now, built first if the world changed since
    /// the last call.
    pub fn texture(
        &mut self,
        ctx: &Context,
        session: &Session,
        palette: &Palette,
    ) -> &TextureHandle {
        let stamp = Stamp::of(session);
        if self.texture.as_ref().is_none_or(|(s, _)| *s != stamp) {
            self.builds += 1;
            let img = image(session, palette);
            match &mut self.texture {
                Some((s, handle)) => {
                    handle.set(img, TextureOptions::NEAREST);
                    *s = stamp;
                }
                None => {
                    let handle = ctx.load_texture("overview", img, TextureOptions::NEAREST);
                    self.texture = Some((stamp, handle));
                }
            }
        }
        &self.texture.as_ref().expect("built above").1
    }
}

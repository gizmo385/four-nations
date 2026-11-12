//! Where the map widget looks, apart from the terminal's own view (`UiState::view`): a centre in
//! tiles and a tile size in pixels. Pure arithmetic, so the tests check it without a window.
//!
//! Tiles are square (decision #868's recommended default: shapes read true and a drag covers what
//! it looks like it covers). The terminal's tall cells are not reproduced here.

use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};
use fn_sim::Pos;

/// The largest a tile grows (pixels a side).
pub const MAX_CELL: f32 = 48.0;
/// Below this size glyphs stop being legible and the map draws one block per tile.
pub const GLYPH_MIN: f32 = 9.0;
/// The smallest a tile may shrink: one pixel per tile.
pub const MIN_CELL: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// The tile coordinate (fractional) at the middle of the widget.
    pub centre: Vec2,
    /// A tile's side in pixels (tiles are square).
    pub cell: f32,
}

impl Camera {
    pub fn new(centre: Pos, cell: f32) -> Self {
        Self { centre: vec2(centre.x as f32 + 0.5, centre.y as f32 + 0.5), cell }
    }

    /// Whether the map draws glyphs (else one block per tile).
    pub fn glyphs(&self) -> bool {
        self.cell >= GLYPH_MIN
    }

    /// One tile's size on screen: a square.
    pub fn tile_size(&self) -> Vec2 {
        vec2(self.cell, self.cell)
    }

    /// The screen position of tile `(x, y)`'s top-left corner in `rect`.
    pub fn tile_to_screen(&self, rect: Rect, x: f32, y: f32) -> Pos2 {
        let t = self.tile_size();
        rect.center() + vec2((x - self.centre.x) * t.x, (y - self.centre.y) * t.y)
    }

    /// The tile under screen point `p` (it may be off the map).
    pub fn screen_to_tile(&self, rect: Rect, p: Pos2) -> Pos {
        let t = self.tile_size();
        let d = p - rect.center();
        let x = self.centre.x + d.x / t.x;
        let y = self.centre.y + d.y / t.y;
        Pos::new(x.floor() as i32, y.floor() as i32)
    }

    /// The tiles at least partly visible in `rect`, as `(x0, y0, x1, y1)` (end exclusive), clamped
    /// to a `w` x `h` map.
    pub fn visible(&self, rect: Rect, w: i32, h: i32) -> (i32, i32, i32, i32) {
        let t = self.tile_size();
        let half = rect.size() / 2.0;
        let x0 = (self.centre.x - half.x / t.x).floor() as i32;
        let y0 = (self.centre.y - half.y / t.y).floor() as i32;
        let x1 = (self.centre.x + half.x / t.x).ceil() as i32 + 1;
        let y1 = (self.centre.y + half.y / t.y).ceil() as i32 + 1;
        (x0.clamp(0, w), y0.clamp(0, h), x1.clamp(0, w), y1.clamp(0, h))
    }

    /// Moves the view by a drag of `delta` pixels (the map follows the pointer).
    pub fn pan(&mut self, delta: Vec2) {
        let t = self.tile_size();
        self.centre -= vec2(delta.x / t.x, delta.y / t.y);
    }

    /// Zooms by `factor` keeping the tile under `p` where it is.
    pub fn zoom_at(&mut self, rect: Rect, p: Pos2, factor: f32) {
        let before = self.tile_to_world(rect, p);
        self.cell = (self.cell * factor).clamp(MIN_CELL, MAX_CELL);
        let after = self.tile_to_world(rect, p);
        self.centre += before - after;
    }

    /// The smallest tile that still shows the whole `w` x `h` map in `rect`, at least
    /// [`MIN_CELL`].
    pub fn whole_map_cell(rect: Rect, w: i32, h: i32) -> f32 {
        (rect.width() / w as f32).min(rect.height() / h as f32).max(MIN_CELL)
    }

    /// Keeps the centre on the map.
    pub fn clamp(&mut self, w: i32, h: i32) {
        self.centre.x = self.centre.x.clamp(0.0, w as f32);
        self.centre.y = self.centre.y.clamp(0.0, h as f32);
    }

    /// Whether tile `pos` is inside `rect`, `margin` tiles in from its edges.
    pub fn shows(&self, rect: Rect, pos: Pos, margin: f32) -> bool {
        let t = self.tile_size();
        let p = self.tile_to_screen(rect, pos.x as f32 + 0.5, pos.y as f32 + 0.5);
        rect.shrink2(vec2(t.x * margin, t.y * margin)).contains(p)
    }

    fn tile_to_world(&self, rect: Rect, p: Pos2) -> Vec2 {
        let t = self.tile_size();
        let d = p - rect.center();
        self.centre + vec2(d.x / t.x, d.y / t.y)
    }
}

/// A rect for tests: `w` x `h` pixels from the origin.
pub fn rect(w: f32, h: f32) -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h))
}

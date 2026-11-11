//! The map widget's camera: screen points to tiles and back, zoom about the pointer, and a
//! whole-map overview on the largest map.

use eframe::egui::{pos2, vec2};
use fn_gui::camera::{Camera, GLYPH_MIN, MAX_CELL, rect};
use fn_sim::Pos;

#[test]
fn the_tile_under_the_centre_is_the_centre_tile_and_round_trips() {
    let r = rect(800.0, 600.0);
    let cam = Camera::new(Pos::new(50, 40), 20.0);
    assert_eq!(cam.screen_to_tile(r, r.center()), Pos::new(50, 40));
    for (x, y) in [(0, 0), (49, 40), (60, 33), (12, 70)] {
        let p = cam.tile_to_screen(r, x as f32 + 0.5, y as f32 + 0.5);
        assert_eq!(cam.screen_to_tile(r, p), Pos::new(x, y));
    }
}

#[test]
fn zooming_keeps_the_tile_under_the_pointer() {
    let r = rect(800.0, 600.0);
    let mut cam = Camera::new(Pos::new(50, 40), 20.0);
    let p = pos2(130.0, 470.0);
    let before = cam.screen_to_tile(r, p);
    cam.zoom_at(r, p, 1.6);
    assert_eq!(cam.screen_to_tile(r, p), before);
    cam.zoom_at(r, p, 1000.0);
    assert_eq!(cam.cell, MAX_CELL, "zoom stops at the largest cell");
}

#[test]
fn the_largest_map_fits_in_an_overview_drawn_as_blocks() {
    let r = rect(1000.0, 900.0);
    let cell = Camera::whole_map_cell(r, 384, 256);
    let cam = Camera { centre: vec2(192.0, 128.0), cell };
    assert!(!cam.glyphs() && cell < GLYPH_MIN, "far out, one block per tile");
    assert_eq!(cam.visible(r, 384, 256), (0, 0, 384, 256), "the whole map shows");
}

#[test]
fn a_drag_pans_the_map_with_the_pointer() {
    let r = rect(800.0, 600.0);
    let mut cam = Camera::new(Pos::new(50, 40), 20.0);
    let p = pos2(400.0, 300.0);
    let under = cam.screen_to_tile(r, p);
    cam.pan(vec2(-120.0, 0.0));
    assert_eq!(cam.screen_to_tile(r, p - vec2(120.0, 0.0)), under, "the tile followed");
}

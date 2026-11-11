//! The map widget: the tiles `fn_tui::map::tile_cell` computes (the same look the terminal
//! draws, marks, tints and cursor included), painted by egui at any size. Glyphs, each centred
//! in its own tile, while cells are legible; below [`crate::camera::GLYPH_MIN`] one block
//! of the tile's background (or, for something standing on it, its foreground) per tile.
//!
//! Input: a click moves the cursor; a drag pans in Select mode and with the secondary or middle
//! button, and with a designate tool active the primary drag makes the rectangle
//! (`fn_tui::pointer`); the wheel zooms about the pointer.

use eframe::egui::{
    self, Color32, FontFamily, FontId, Mesh, PointerButton, Rect, Sense, Shape, Stroke, Ui, vec2,
};
use fn_sim::Pos;
use fn_tui::app::App;
use fn_tui::color::Palette;
use fn_tui::map::{Overlays, Thing, cell_look, style_cell};
use fn_tui::pointer::{self, Pointer};
use fn_ui::tools::Tool;

use crate::camera::Camera;
use crate::color::to_egui;

/// The widget's own state between frames.
pub struct MapView {
    pub camera: Camera,
    pointer: Pointer,
    /// The cursor as of the last frame, so a keyboard move can bring the camera along.
    last_cursor: Option<Pos>,
    /// How long the last paint took, for the overlay and the findings.
    pub last_paint_ms: f32,
}

impl MapView {
    pub fn new(app: &App) -> Self {
        Self {
            camera: Camera::new(app.ui.cursor, 18.0),
            pointer: Pointer::new(),
            last_cursor: None,
            last_paint_ms: 0.0,
        }
    }

    /// Lays out, handles input for and paints the map into `rect`.
    pub fn show(&mut self, ui: &mut Ui, rect: Rect, app: &mut App, palette: &Palette) {
        let (w, h) = (app.session.world.map.width(), app.session.world.map.height());
        let response = ui.interact(rect, ui.id().with("map"), Sense::click_and_drag());

        // A keyboard move off screen brings the camera along.
        if self.last_cursor != Some(app.ui.cursor) && !self.camera.shows(rect, app.ui.cursor, 3.0) {
            self.camera.centre = vec2(app.ui.cursor.x as f32 + 0.5, app.ui.cursor.y as f32 + 0.5);
        }

        // The wheel zooms about the pointer.
        if let Some(hover) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y + i.zoom_delta().ln() * 200.0);
            if scroll != 0.0 {
                self.camera.zoom_at(rect, hover, (scroll / 200.0).exp());
            }
        }

        let rectangle_tool =
            !matches!(app.ui.tool, Tool::Select | Tool::Gate) && pointer::map_has_pointer(app);
        let primary_pans = !rectangle_tool;
        if response.dragged_by(PointerButton::Secondary)
            || response.dragged_by(PointerButton::Middle)
            || (primary_pans && response.dragged_by(PointerButton::Primary))
        {
            self.camera.pan(response.drag_delta());
        }
        if rectangle_tool && let Some(p) = response.interact_pointer_pos() {
            let tile = self.camera.screen_to_tile(rect, p);
            if response.drag_started_by(PointerButton::Primary) {
                // Where the button went down, not where the drag threshold was crossed.
                let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(p);
                self.pointer.down(app, self.camera.screen_to_tile(rect, origin));
            }
            if response.dragged_by(PointerButton::Primary) {
                self.pointer.moved(app, tile);
            }
            if response.drag_stopped_by(PointerButton::Primary) {
                self.pointer.up(app, tile);
            }
        }
        if response.clicked_by(PointerButton::Primary)
            && let Some(p) = response.interact_pointer_pos()
        {
            pointer::click(app, self.camera.screen_to_tile(rect, p));
        }
        self.camera.clamp(w, h);
        self.last_cursor = Some(app.ui.cursor);

        let started = std::time::Instant::now();
        paint(ui, rect, &self.camera, app, palette);
        self.last_paint_ms = started.elapsed().as_secs_f32() * 1000.0;
    }
}

/// Paints the tiles of `app`'s world visible through `camera` into `rect`.
pub fn paint(ui: &Ui, rect: Rect, camera: &Camera, app: &App, palette: &Palette) {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 12));
    let (session, state) = (&app.session, &app.ui);
    let (w, h) = (session.world.map.width(), session.world.map.height());
    let (x0, y0, x1, y1) = camera.visible(rect, w, h);
    let over = Overlays::gather(session, state);
    let tile = camera.tile_size();
    let glyphs = camera.glyphs();
    let font = FontId::new(camera.cell * 0.8, FontFamily::Monospace);

    let mut mesh = Mesh::default();
    let mut glyph_shapes = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            let pos = Pos::new(x, y);
            let Some(look) = cell_look(session, state, palette, &over, pos) else { continue };
            let cell = style_cell(&look, &session.defs, palette);
            let bg = to_egui(cell.style.bg.unwrap_or_default(), Color32::BLACK);
            let fg = to_egui(cell.style.fg.unwrap_or_default(), Color32::WHITE);
            let at = camera.tile_to_screen(rect, x as f32, y as f32);
            // In the overview a tile with something on it shows the thing's colour, so units,
            // buildings and designations stay visible from far out.
            let block = if !glyphs && look.thing != Thing::Ground { fg } else { bg };
            mesh.add_colored_rect(Rect::from_min_size(at, tile), block);
            if glyphs && cell.glyph != ' ' {
                // Each glyph is centred in its own tile, whatever font it fell back to and
                // whatever its advance, so nothing drifts off the grid.
                let galley =
                    ui.fonts_mut(|f| f.layout_no_wrap(cell.glyph.to_string(), font.clone(), fg));
                let centre = at + tile / 2.0;
                glyph_shapes.push(Shape::galley(centre - galley.size() / 2.0, galley, fg));
            }
        }
    }
    painter.add(Shape::mesh(mesh));
    painter.extend(glyph_shapes);
    // The cursor's own outline, so it reads at any zoom.
    let c = camera.tile_to_screen(rect, state.cursor.x as f32, state.cursor.y as f32);
    painter.rect_stroke(
        Rect::from_min_size(c, tile).expand(1.0),
        0.0,
        Stroke::new(1.5_f32, Color32::WHITE),
        egui::StrokeKind::Outside,
    );
}

//! The map widget: the tiles `fn_tui::map::tile_cell` computes (the same look the terminal
//! draws, marks, tints and cursor included), painted by egui at any size. Glyphs, each centred
//! in its own tile, while cells are legible; below [`crate::camera::GLYPH_MIN`] one block
//! of the tile's background (or, for something standing on it, its foreground) per tile.
//!
//! Tiles are square (#868's default). Below the glyph size the overview is a cached texture,
//! one pixel per tile ([`crate::overview`]), with the cursor, the rectangle and the selected unit
//! drawn over it.
//!
//! Input (the rules are `fn_ui::pointer`'s, and each click is the keys it stands for): in Select
//! a left click points at the tile and then selects the unit there (`Enter`) or deselects
//! (`Esc`); with a tool a click is a one-tile apply, a left drag makes the rectangle and a right
//! click is `Esc`. A drag pans in Select mode and, with the secondary or middle button, in any;
//! the wheel zooms about the pointer. Resting the pointer on a tile for [`Hover::DELAY`] seconds
//! shows its tooltip (`fn_ui::inspect::tooltip_lines`) without moving the cursor.
//!
//! Touch (#977, [`crate::touch`]): a tap is a click and one finger drags as the left button
//! does. Two fingers pan and pinch-zoom about their midpoint, and a rectangle in progress is
//! dropped when the second lands. A long press shows the tile's tooltip until the finger lifts,
//! and never applies a tool.

use eframe::egui::{
    self, Color32, FontFamily, FontId, Mesh, PointerButton, Pos2, Rect, RichText, Sense, Shape,
    Stroke, Ui, vec2,
};
use fn_sim::Pos;
use fn_tui::color::{Palette, Tint};
use fn_tui::map::{Overlays, cell_look, style_cell};
use fn_ui::app::App;
use fn_ui::inspect::{Line, tooltip_lines};
use fn_ui::inspector::hint_text;
use fn_ui::pointer::{self, Pointer};
use fn_ui::tools::Tool;

use crate::camera::Camera;
use crate::color::to_egui;
use crate::overview::{Overview, block_color};
use crate::touch::Gestures;

/// A tile's side in pixels in a fresh window: square, so a little smaller than the terminal's
/// tall cell was, to keep about as many tiles across.
pub const FIRST_CELL: f32 = 14.0;

/// Which tile's tooltip is up: the pointer must rest on one tile for [`Hover::DELAY`] seconds.
/// Pure (`now` is egui's frame time), so a test steps it without a window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hover {
    tile: Option<Pos>,
    since: f64,
}

impl Hover {
    /// How long the pointer rests before the tooltip shows.
    pub const DELAY: f64 = 0.4;

    /// This frame's tile under the pointer (`None` when it is off the map, over something else,
    /// or busy with a press), at time `now`: the tile whose tooltip should show.
    pub fn update(&mut self, tile: Option<Pos>, now: f64) -> Option<Pos> {
        if self.tile != tile {
            self.tile = tile;
            self.since = now;
        }
        self.tile.filter(|_| now - self.since >= Self::DELAY)
    }
}

/// The widget's own state between frames.
pub struct MapView {
    pub camera: Camera,
    pointer: Pointer,
    /// The cursor as of the last frame, so a keyboard move can bring the camera along.
    last_cursor: Option<Pos>,
    /// How long the last paint took, for the overlay and the findings.
    pub last_paint_ms: f32,
    hover: Hover,
    /// What the fingers are doing, on a touch screen.
    gestures: Gestures,
    /// The overview texture, below the glyph size.
    pub overview: Overview,
}

impl MapView {
    pub fn new(app: &App) -> Self {
        Self {
            camera: Camera::new(app.ui.cursor, FIRST_CELL),
            pointer: Pointer::new(),
            last_cursor: None,
            last_paint_ms: 0.0,
            hover: Hover::default(),
            gestures: Gestures::default(),
            overview: Overview::default(),
        }
    }

    /// Lays out, handles input for and paints the map into `rect`. With a screen, box or
    /// question open the map still paints (it is what the box floats over) but takes no input
    /// (`fn_ui::pointer::map_has_pointer`).
    pub fn show(&mut self, ui: &mut Ui, rect: Rect, app: &mut App, palette: &Palette) {
        let (w, h) = (app.session.world.map.width(), app.session.world.map.height());
        let live = pointer::map_has_pointer(app);
        let sense = if live { Sense::click_and_drag() } else { Sense::hover() };
        let response = ui.interact(rect, ui.scope_id().with("map"), sense);

        // A cursor sent somewhere to be looked at (a notice's jump button, the log's `Enter`) is
        // centred, not just brought into sight.
        if app.ui.take_centring() {
            self.camera.centre = vec2(app.ui.cursor.x as f32 + 0.5, app.ui.cursor.y as f32 + 0.5);
        }
        // A keyboard move off screen brings the camera along.
        if self.last_cursor != Some(app.ui.cursor) && !self.camera.shows(rect, app.ui.cursor, 3.0) {
            self.camera.centre = vec2(app.ui.cursor.x as f32 + 0.5, app.ui.cursor.y as f32 + 0.5);
        }

        // The fingers, on a touch screen (a mouse is never `touching`).
        let (touching, fingers, at, multi_touch) = ui.input(|i| {
            let touching = i.any_touches();
            let mt = i.multi_touch();
            let fingers = mt.map_or(usize::from(touching), |m| m.num_touches);
            (touching, fingers, i.pointer.latest_pos(), mt)
        });
        let touch = self.gestures.update(touching, at, fingers, ui.input(|i| i.time));
        if live && touch.cancel {
            self.pointer.abandon(app);
        }
        // Two fingers pan and zoom about their midpoint; one-finger rules are off till they lift.
        if live
            && touch.multi
            && let Some(mt) = multi_touch
        {
            self.camera.pan(mt.translation_delta);
            if mt.zoom_delta != 1.0 && rect.contains(mt.center_pos) {
                self.camera.zoom_at(rect, mt.center_pos, mt.zoom_delta);
            }
        }
        let one_finger = !touch.multi && !self.gestures.long_pressing() && !touch.swallow_release;

        // The wheel zooms about the pointer.
        if live
            && !touch.multi
            && let Some(hover) = response.hover_pos()
        {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y + i.zoom_delta().ln() * 200.0);
            if scroll != 0.0 {
                self.camera.zoom_at(rect, hover, (scroll / 200.0).exp());
            }
        }

        let rectangle_tool =
            !matches!(app.ui.tool, Tool::Select | Tool::Gate) && pointer::map_has_pointer(app);
        let primary_pans = !rectangle_tool;
        if live
            && one_finger
            && (response.dragged_by(PointerButton::Secondary)
                || response.dragged_by(PointerButton::Middle)
                || (primary_pans && response.dragged_by(PointerButton::Primary)))
        {
            self.camera.pan(response.drag_delta());
        }
        if live
            && one_finger
            && rectangle_tool
            && let Some(p) = response.interact_pointer_pos()
        {
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
        if live
            && one_finger
            && let Some(p) = response.interact_pointer_pos()
        {
            let tile = self.camera.screen_to_tile(rect, p);
            if response.clicked_by(PointerButton::Primary) {
                pointer::click_with(app, tile, pointer::Button::Primary);
            } else if response.clicked_by(PointerButton::Secondary) {
                pointer::click_with(app, tile, pointer::Button::Secondary);
            }
        }
        self.camera.clamp(w, h);
        self.last_cursor = Some(app.ui.cursor);

        // The tile the pointer rests on, unless a button is down (a press or a drag is not
        // hovering) or something else is over the map.
        let busy = ui.input(|i| i.pointer.any_down());
        let resting = (live && response.hovered() && !busy)
            .then(|| response.hover_pos())
            .flatten()
            .map(|p| self.camera.screen_to_tile(rect, p))
            .filter(|&t| app.session.world.map.get(t).is_some());
        let tip = self.hover.update(resting, ui.input(|i| i.time));
        // A long press shows its tile at once, finger down (touch has no hover).
        let pressed = touch
            .long_press
            .filter(|_| live)
            .map(|p| self.camera.screen_to_tile(rect, p))
            .filter(|&t| app.session.world.map.get(t).is_some());
        let tip = pressed.or(tip);

        let started = web_time::Instant::now();
        if self.camera.glyphs() {
            paint(ui, rect, &self.camera, app, palette);
        } else {
            self.paint_overview(ui, rect, app, palette);
        }
        self.last_paint_ms = started.elapsed().as_secs_f32() * 1000.0;

        if let Some(tile) = tip {
            show_tooltip(ui, &response, app, tile);
        }
    }

    /// The overview: the cached texture over the whole map, then the player's own marks.
    fn paint_overview(&mut self, ui: &Ui, rect: Rect, app: &App, palette: &Palette) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 12));
        let (w, h) = (app.session.world.map.width(), app.session.world.map.height());
        let texture = self.overview.texture(ui.ctx(), &app.session, palette);
        let whole = Rect::from_min_max(
            self.camera.tile_to_screen(rect, 0.0, 0.0),
            self.camera.tile_to_screen(rect, w as f32, h as f32),
        );
        painter.image(
            texture.id(),
            whole,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        paint_marks(ui, rect, &self.camera, app, palette);
    }
}

/// What the pointer rests on, in a tooltip at the pointer: `fn_ui::inspect::tooltip_lines` (the
/// terminal's own words), the title in strong type, the action hint dim.
fn show_tooltip(ui: &Ui, response: &egui::Response, app: &App, tile: Pos) {
    let Some(lines) = tooltip_lines(&app.session, app.ui.tool, tile) else { return };
    egui::Tooltip::always_open(
        ui.ctx().clone(),
        response.layer_id,
        response.id,
        egui::PopupAnchor::Pointer,
    )
    .show(|ui| {
        for line in lines {
            match line {
                Line::Title(t) => ui.label(RichText::new(t).monospace().strong()),
                Line::Detail(d) => ui.label(RichText::new(d).monospace()),
                Line::Hint(h) => match hint_text(h) {
                    Some(text) => ui.label(RichText::new(text).monospace().weak()),
                    None => continue,
                },
            };
        }
    });
}

/// The player's own marks over the map, whatever the zoom: the cursor's outline, and in the
/// overview (where tiles are a pixel or two) the rectangle being dragged and the selected unit,
/// which the cached texture does not hold.
fn paint_marks(ui: &Ui, rect: Rect, camera: &Camera, app: &App, palette: &Palette) {
    let painter = ui.painter_at(rect);
    let state = &app.ui;
    let tile = camera.tile_size();
    if !camera.glyphs() {
        if let Some((a, b)) = state.rect() {
            let (lo, hi) =
                (Pos::new(a.x.min(b.x), a.y.min(b.y)), Pos::new(a.x.max(b.x), a.y.max(b.y)));
            let area = Rect::from_min_max(
                camera.tile_to_screen(rect, lo.x as f32, lo.y as f32),
                camera.tile_to_screen(rect, hi.x as f32 + 1.0, hi.y as f32 + 1.0),
            );
            let hue = Tint::for_tool(state.tool).map_or(Color32::WHITE, |t| {
                let [r, g, b] = palette.tint_rgb(t);
                Color32::from_rgb(r, g, b)
            });
            painter.rect_filled(area, 0.0, hue.gamma_multiply(0.45));
            painter.rect_stroke(area, 0.0, Stroke::new(1.0_f32, hue), egui::StrokeKind::Outside);
        }
        if let Some(unit) = state.selected_unit(&app.session) {
            let at = camera.tile_to_screen(rect, unit.pos.x as f32 + 0.5, unit.pos.y as f32 + 0.5);
            let ring = (tile.x * 0.5).max(4.0);
            painter.circle_stroke(at, ring, Stroke::new(1.5_f32, Color32::WHITE));
        }
    }
    // The cursor's own outline, so it reads at any zoom.
    let c = camera.tile_to_screen(rect, state.cursor.x as f32, state.cursor.y as f32);
    painter.rect_stroke(
        Rect::from_min_size(c, tile).expand(1.0),
        0.0,
        Stroke::new(1.5_f32, Color32::WHITE),
        egui::StrokeKind::Outside,
    );
}

/// Paints the tiles of `app`'s world visible through `camera` into `rect`, uncached (the window
/// goes through [`MapView`]'s overview texture below the glyph size instead).
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
            // Without glyphs (`paint` in a test; the window uses `crate::overview`) a tile with
            // something on it shows the thing's colour, so it stays visible from far out.
            let block = if glyphs { bg } else { block_color(&look.thing, bg, fg) };
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
    paint_marks(ui, rect, camera, app, palette);
}

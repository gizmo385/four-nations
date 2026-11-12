//! The map's mouse (#878): each click is the keys it stands for (so a click's command log is the
//! keyboard path's), hovering shows the tooltip without moving the cursor, square tiles, and the
//! cached overview. kittest drives a real egui pass without a renderer, so none of it needs a GPU.

use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::camera::{Camera, GLYPH_MIN};
use fn_gui::mapview::{Hover, MapView};
use fn_gui::overview::{self, Overview};
use fn_sim::{Nation, Pos};
use fn_tui::color::{ColorDepth, Palette};
use fn_ui::app::App;
use fn_ui::inspect::{Line, tooltip_lines};
use fn_ui::tools::Tool;

const SIZE: (u16, u16) = (120, 40);
const WINDOW: (f32, f32) = (1000.0, 700.0);

fn game() -> App {
    App::new(fn_tui::new_session(7, Nation::Earth), SIZE)
}

fn play(app: &mut App, script: &str) {
    app.play(&fn_ui::script::parse(script).expect("a valid script"));
}

fn palette(app: &App) -> Palette {
    Palette::new(&app.session.defs, Nation::Earth, ColorDepth::TrueColor, false)
}

fn settler(app: &App) -> Pos {
    app.session.world.units.iter().find(|u| u.inside().is_none()).expect("a settler").pos
}

/// The resource tile nearest `from`: something a Harvest rectangle marks.
fn resource_near(app: &App, from: Pos) -> Pos {
    let w = &app.session.world;
    (0..w.map.height())
        .flat_map(|y| (0..w.map.width()).map(move |x| Pos::new(x, y)))
        .filter(|&p| w.map.get(p).is_some_and(|t| t.resource.is_some()))
        .min_by_key(|p| (p.x - from.x).abs() + (p.y - from.y).abs())
        .expect("a resource")
}

fn window() -> Rect {
    Rect::from_min_size(Pos2::ZERO, vec2(WINDOW.0, WINDOW.1))
}

/// A map widget in a window, looking at `centre` with tiles `cell` pixels a side. Hand back the
/// camera that was set so the test can place the pointer.
fn view_at(app: &App, centre: Pos, cell: f32) -> (MapView, Camera) {
    let mut view = MapView::new(app);
    view.camera = Camera::new(centre, cell);
    let camera = view.camera;
    (view, camera)
}

fn harness<'a>(view: &'a mut MapView, app: &'a mut App) -> Harness<'a> {
    let palette = palette(app);
    let mut h = Harness::builder()
        .with_size(vec2(WINDOW.0, WINDOW.1))
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| {
            let rect = ui.available_rect_before_wrap();
            view.show(ui, rect, app, &palette);
        });
    h.run_steps(2);
    h
}

/// Where tile `p`'s middle is on screen.
fn at(camera: &Camera, p: Pos) -> Pos2 {
    camera.tile_to_screen(window(), p.x as f32 + 0.5, p.y as f32 + 0.5)
}

fn press(h: &mut Harness<'_>, pos: Pos2, button: PointerButton, pressed: bool) {
    h.event(Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE });
    h.run_steps(1);
}

fn click(h: &mut Harness<'_>, pos: Pos2, button: PointerButton) {
    h.hover_at(pos);
    h.run_steps(1);
    press(h, pos, button, true);
    press(h, pos, button, false);
    h.run_steps(1);
}

#[test]
fn tiles_are_square_at_every_zoom() {
    for cell in [1.0, 4.0, GLYPH_MIN - 0.1, GLYPH_MIN, 14.0, 48.0] {
        let t = Camera::new(Pos::new(10, 10), cell).tile_size();
        assert_eq!(t.x, t.y, "cell {cell}");
        assert_eq!(t.x, cell);
    }
}

#[test]
fn a_click_in_select_mode_selects_the_unit_like_enter() {
    let mut keyboard = game();
    let mut mouse = game();
    let unit = settler(&mouse);
    // The cursor starts on the settler: both move off, then one walks back and presses Enter,
    // the other clicks.
    let off = Pos::new(unit.x + 5, unit.y + 2);
    keyboard.ui.point_at(off);
    mouse.ui.point_at(off);
    keyboard.ui.point_at(unit);
    play(&mut keyboard, "<enter>");

    let (mut view, camera) = view_at(&mouse, Pos::new(unit.x + 2, unit.y + 1), 16.0);
    let mut h = harness(&mut view, &mut mouse);
    click(&mut h, at(&camera, unit), PointerButton::Primary);
    drop(h);
    assert!(mouse.ui.selected.is_some());
    assert_eq!((mouse.ui.selected, mouse.ui.cursor), (keyboard.ui.selected, keyboard.ui.cursor));
    assert_eq!(mouse.session.world.command_log(), keyboard.session.world.command_log());

    // A click on bare ground deselects (Esc) and moves the cursor there; it never opens the menu.
    let bare = Pos::new(unit.x + 4, unit.y + 3);
    assert!(mouse.session.world.units_at(bare).next().is_none());
    let (mut view, camera) = view_at(&mouse, Pos::new(unit.x + 2, unit.y + 1), 16.0);
    let mut h = harness(&mut view, &mut mouse);
    click(&mut h, at(&camera, bare), PointerButton::Primary);
    click(&mut h, at(&camera, bare), PointerButton::Primary);
    drop(h);
    assert_eq!((mouse.ui.selected, mouse.ui.cursor), (None, bare));
    assert!(mouse.ui.modal.is_none());
}

#[test]
fn a_click_with_a_tool_is_the_keyboards_two_enters() {
    let mut keyboard = game();
    let mut mouse = game();
    let start = mouse.ui.cursor;
    let target = resource_near(&mouse, start);
    play(&mut keyboard, "h");
    keyboard.ui.point_at(target);
    play(&mut keyboard, "<enter> <enter>");
    assert!(!keyboard.session.world.command_log().is_empty());

    play(&mut mouse, "h");
    let middle = Pos::new((start.x + target.x) / 2, (start.y + target.y) / 2);
    let (mut view, camera) = view_at(&mouse, middle, 16.0);
    assert!(camera.shows(window(), target, 0.0) && camera.shows(window(), start, 0.0));
    let mut h = harness(&mut view, &mut mouse);
    click(&mut h, at(&camera, target), PointerButton::Primary);
    drop(h);
    assert_eq!(mouse.session.world.command_log(), keyboard.session.world.command_log());
    assert_eq!((mouse.ui.tool, mouse.ui.anchor, mouse.ui.cursor), (Tool::Harvest, None, target));
}

#[test]
fn a_drag_with_a_tool_is_the_keyboard_rectangle() {
    let mut keyboard = game();
    let mut mouse = game();
    let start = mouse.ui.cursor;
    let (a, b) = (Pos::new(start.x - 6, start.y - 3), Pos::new(start.x - 1, start.y + 1));
    play(&mut keyboard, "h <left*6> <up*3> <enter> <right*5> <down*4> <enter>");
    assert!(!keyboard.session.world.command_log().is_empty());

    play(&mut mouse, "h");
    let (mut view, camera) = view_at(&mouse, start, 16.0);
    let mut h = harness(&mut view, &mut mouse);
    h.hover_at(at(&camera, a));
    h.run_steps(1);
    press(&mut h, at(&camera, a), PointerButton::Primary, true);
    for p in [Pos::new(a.x + 1, a.y), Pos::new(a.x + 3, a.y + 1), b] {
        h.hover_at(at(&camera, p));
        h.run_steps(1);
    }
    press(&mut h, at(&camera, b), PointerButton::Primary, false);
    h.run_steps(1);
    drop(h);
    assert_eq!(mouse.session.world.command_log(), keyboard.session.world.command_log());
    assert_eq!(mouse.ui.anchor, None);
}

#[test]
fn a_right_click_is_esc_with_a_tool_and_does_nothing_in_select() {
    let mut app = game();
    let start = app.ui.cursor;
    play(&mut app, "h <enter>");
    assert!(app.ui.anchor.is_some());
    let (mut view, camera) = view_at(&app, start, 16.0);
    let mut h = harness(&mut view, &mut app);
    let spot = at(&camera, Pos::new(start.x + 3, start.y));
    click(&mut h, spot, PointerButton::Secondary);
    h.run_steps(1);
    drop(h);
    assert_eq!((app.ui.tool, app.ui.anchor), (Tool::Harvest, None), "the corner dropped");

    let mut h = harness(&mut view, &mut app);
    click(&mut h, spot, PointerButton::Secondary);
    drop(h);
    assert_eq!(app.ui.tool, Tool::Select, "the second Esc left the tool");

    let mut h = harness(&mut view, &mut app);
    click(&mut h, spot, PointerButton::Secondary);
    drop(h);
    assert!(app.ui.modal.is_none(), "in Select a right click would open the menu as Esc does");
}

#[test]
fn hovering_a_tile_shows_its_tooltip_after_the_delay_and_leaves_the_cursor() {
    let mut app = game();
    let unit = settler(&app);
    let spot = Pos::new(unit.x + 2, unit.y);
    // Two tiles off, so the cursor is not on the tile whose tooltip is up.
    let title = match tooltip_lines(&app.session, app.ui.tool, spot).expect("on the map").remove(0)
    {
        Line::Title(t) => t,
        other => panic!("a tooltip starts with its title, not {other:?}"),
    };
    let cursor = app.ui.cursor;
    let (mut view, camera) = view_at(&app, unit, 16.0);
    let mut h = harness(&mut view, &mut app);
    h.hover_at(at(&camera, spot));
    h.run_steps(1);
    assert!(h.query_by_label_contains(&title).is_none(), "not yet: the delay is 0.4 s");
    h.run_steps(4);
    assert!(h.query_by_label_contains(&title).is_some(), "{title} after resting on the tile");
    // Moving to another tile hides it and starts the wait again.
    h.hover_at(at(&camera, Pos::new(spot.x + 5, spot.y + 5)));
    h.run_steps(1);
    assert!(h.query_by_label_contains(&title).is_none());
    drop(h);
    assert_eq!(app.ui.cursor, cursor, "hovering does not move the cursor");
}

#[test]
fn the_hover_delay_restarts_on_a_new_tile_and_waits_for_the_pointer_to_rest() {
    let mut hover = Hover::default();
    let (a, b) = (Pos::new(1, 1), Pos::new(2, 1));
    assert_eq!(hover.update(Some(a), 10.0), None);
    assert_eq!(hover.update(Some(a), 10.0 + Hover::DELAY - 0.01), None);
    assert_eq!(hover.update(Some(a), 10.0 + Hover::DELAY), Some(a));
    assert_eq!(hover.update(Some(b), 11.0), None, "a new tile starts the wait again");
    assert_eq!(hover.update(Some(b), 11.5), Some(b));
    assert_eq!(hover.update(None, 11.6), None, "off the map or busy: nothing");
    assert_eq!(hover.update(Some(b), 11.7), None, "and the wait starts over");
}

#[test]
fn the_overview_texture_is_rebuilt_only_when_the_map_changes() {
    let mut app = game();
    let ctx = egui::Context::default();
    let palette = palette(&app);
    let mut overview = Overview::default();
    for _ in 0..5 {
        overview.texture(&ctx, &app.session, &palette);
    }
    assert_eq!(overview.builds, 1, "five frames, one build");
    // A cursor move or a corner is the player's, not the map's.
    play(&mut app, "h <enter> <right*3>");
    overview.texture(&ctx, &app.session, &palette);
    assert_eq!(overview.builds, 1);
    // A designation while paused changes the map without a tick.
    play(&mut app, "<esc>");
    let tree = resource_near(&app, settler(&app));
    app.ui.point_at(tree);
    play(&mut app, "<enter> <enter>");
    assert!(!app.session.world.command_log().is_empty());
    overview.texture(&ctx, &app.session, &palette);
    assert_eq!(overview.builds, 2, "a command is a change");
    // And so is a tick.
    app.session.world.tick(&app.session.defs);
    overview.texture(&ctx, &app.session, &palette);
    overview.texture(&ctx, &app.session, &palette);
    assert_eq!(overview.builds, 3);
}

#[test]
fn the_window_draws_the_overview_from_one_cached_texture() {
    let mut app = game();
    let (w, h) = (app.session.world.map.width(), app.session.world.map.height());
    let cell = Camera::whole_map_cell(window(), w, h);
    assert!(cell < GLYPH_MIN, "the whole map is an overview in this window");
    let (mut view, _) = view_at(&app, Pos::new(w / 2, h / 2), cell);
    let mut harness = harness(&mut view, &mut app);
    harness.run_steps(6);
    drop(harness);
    assert_eq!(view.overview.builds, 1, "eight frames of a still map, one build");
}

#[test]
fn the_overview_image_has_a_pixel_per_tile_and_shows_what_stands_on_it() {
    let mut app = game();
    play(&mut app, "<tick 1>");
    let image = overview::image(&app.session, &palette(&app));
    let (w, h) = (app.session.world.map.width(), app.session.world.map.height());
    assert_eq!(image.size, [w as usize, h as usize]);
    let unit = settler(&app);
    let pixel = image.pixels[(unit.y * w + unit.x) as usize];
    // A settler's tile is the unit's colour, not the ground's.
    let neighbours = [Pos::new(unit.x + 1, unit.y), Pos::new(unit.x - 1, unit.y)];
    assert!(
        neighbours.iter().any(|n| image.pixels[(n.y * w + n.x) as usize] != pixel),
        "the settler stands out from its surroundings"
    );
    let distinct: std::collections::BTreeSet<_> =
        image.pixels.iter().map(|c| c.to_array()).collect();
    assert!(distinct.len() > 4, "land, water and what is built show apart");
}

/// The cost of one whole-map frame on the largest map (384 x 256): the old uncached painter, the
/// first frame of the overview (it builds the texture) and a frame after it. Prints the figures
/// for the PR: `cargo test --release -p fn_gui --test it overview_cost -- --ignored --nocapture`.
#[test]
#[ignore = "timing: run in release and read the output"]
fn overview_cost() {
    use std::cell::Cell;
    use std::time::Instant;
    let session = fn_ui::session::Session::new(7, Nation::Earth, 384, 256);
    let mut app = App::new(session, SIZE);
    let (w, h) = (app.session.world.map.width(), app.session.world.map.height());
    let cell = Camera::whole_map_cell(window(), w, h);
    let camera = Camera { centre: vec2(w as f32 / 2.0, h as f32 / 2.0), cell };
    let palette = palette(&app);

    let old = Cell::new(Vec::new());
    {
        let app = &app;
        let mut harness = Harness::builder().with_size(vec2(WINDOW.0, WINDOW.1)).build_ui(|ui| {
            let started = Instant::now();
            fn_gui::mapview::paint(ui, window(), &camera, app, &palette);
            let mut v = old.take();
            v.push(started.elapsed().as_secs_f32() * 1000.0);
            old.set(v);
        });
        harness.run_steps(6);
    }
    let old = old.take();

    let (mut view, _) = view_at(&app, Pos::new(w / 2, h / 2), cell);
    let new = Cell::new(Vec::new());
    {
        let palette = &palette;
        let view = &mut view;
        let app = &mut app;
        let mut harness = Harness::builder().with_size(vec2(WINDOW.0, WINDOW.1)).build_ui(|ui| {
            view.show(ui, window(), app, palette);
            let mut v = new.take();
            v.push(view.last_paint_ms);
            new.set(v);
        });
        harness.run_steps(6);
    }
    let new = new.take();
    println!("map {w}x{h}, cell {cell:.2}px");
    println!("uncached painter, per frame (ms): {old:.1?}");
    println!("overview texture, per frame (ms; the first builds it): {new:.1?}");
}

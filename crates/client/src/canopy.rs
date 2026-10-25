//! Tree canopies fade where they hide something (#77). A 2x2 tree overlaps the row above it, so in
//! a forest it hides units, designations and the hover outline (Pillar 4, readable simulation).
//! A canopy fades to `FADED_ALPHA` while it is within `FADE_RADIUS` of the cursor or of the
//! selected unit, or while it is in front of any unit it overlaps, and comes back when none of
//! those holds. The trunk stays solid, so the tree never vanishes.
//!
//! View only: nothing here reads or writes the sim beyond the unit positions the client already
//! draws. Each tree is two sprites: the trunk (`ResourceSprite`, the sheet row 0 with the canopy
//! blanked out by `trees::trunks_of_sheet`, built by `build_trunks` once the sheet has loaded)
//! and, as its child, the canopy copy (`trees::canopy_of`) carrying a `CanopyFade`. The two cover
//! disjoint pixels, so a canopy at full alpha is the whole tree, pixel for pixel.
//!
//! What fades is decided by pure functions (`target_alpha`, `step_alpha`) on world-space
//! rectangles; `fade_canopies` only gathers the cursor, the selection and the unit sprites, and
//! applies the result over `FADE_SECONDS` of frame time.
//!
//! Buildings join in (#210) where they can hide something: a sprite bigger than its footprint, or
//! a footprint units walk on (`buildings::fades`). Such a sprite carries a `BuildingFade`, and
//! `fade_buildings` fades the whole sprite by the same rules.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;
use fn_sim::Pos;

use super::{Art, Selection, TILE, UnitSprite, row_depth, sprites, tile_translation};

/// How close (in world pixels, from the edge of the canopy) the cursor or the selected unit has to
/// be for a canopy to fade.
pub const FADE_RADIUS: f32 = 12.0;
/// The opacity of a faded canopy: enough that the tree still reads as a tree, little enough that
/// what is under it shows.
pub const FADED_ALPHA: f32 = 0.35;
/// How long a canopy takes to fade all the way out, or back in.
pub const FADE_SECONDS: f32 = 0.15;
const _: () =
    assert!(FADED_ALPHA > 0.0 && FADED_ALPHA < 1.0 && FADE_RADIUS > 0.0 && FADE_SECONDS > 0.0);
/// The half-size of the part of a unit's 16px sprite that a canopy has to cover to hide it: the
/// character fills most of the height and about two thirds of the width of its cell.
const UNIT_HALF: Vec2 = Vec2::new(5.0, 7.0);
/// The canopy sprite's z above its trunk's.
pub const CANOPY_Z: f32 = 0.00001;

/// The fading systems. Whatever sets a faded sprite's own tint (`buildings`) runs before them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanopySet;

/// A building sprite that fades like a canopy (`buildings::fades`): how faded it is now, and
/// where it is.
#[derive(Component)]
pub struct BuildingFade {
    alpha: f32,
    /// The sprite's whole cell, in world pixels.
    bounds: Rect,
    z: f32,
}

impl BuildingFade {
    /// The fade of a sprite of `cell` standing with the middle of its bottom edge at `foot`
    /// (whose z is its depth).
    pub fn new(foot: Vec3, cell: sprites::Cell) -> Self {
        let size = Vec2::new(cell.w as f32, cell.h as f32) * sprites::CELL;
        let min = Vec2::new(foot.x - size.x / 2.0, foot.y);
        BuildingFade { alpha: 1.0, bounds: Rect::from_corners(min, min + size), z: foot.z }
    }
}

/// A tree's canopy sprite: how opaque it is now, and where it is, for `target_alpha`.
#[derive(Component)]
pub struct CanopyFade {
    alpha: f32,
    /// The part of the sprite that has canopy in it, in world pixels.
    bounds: Rect,
    /// The z the tree draws at; units with a lower z are behind it.
    z: f32,
}

impl CanopyFade {
    /// The canopy of the tree standing on tile `p`.
    pub fn new(p: Pos) -> Self {
        // The sprite stands on the bottom edge of its tile (see `spawn_entities`).
        let foot = tile_translation(p, 0.0).truncate() - Vec2::Y * TILE / 2.0;
        CanopyFade { alpha: 1.0, bounds: canopy_bounds(foot), z: row_depth(p.y) }
    }
}

/// A unit as the fade sees it: where its sprite is and the z it draws at (both as drawn, so they
/// move smoothly while the unit walks).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub centre: Vec2,
    pub z: f32,
}

/// What canopies fade for.
#[derive(Default)]
pub struct Focus {
    pub cursor: Option<Vec2>,
    pub selected: Option<Body>,
    pub units: Vec<Body>,
}

/// The canopy of a tree whose sprite stands with the middle of its bottom edge at `foot`.
pub fn canopy_bounds(foot: Vec2) -> Rect {
    let [x0, y0, x1, y1] = sprites::trees::CANOPY_EXTENT;
    Rect::new(foot.x + x0, foot.y + y0, foot.x + x1, foot.y + y1)
}

/// The distance from `p` to the nearest point of `rect`: 0 inside it.
fn distance_to(rect: Rect, p: Vec2) -> f32 {
    (rect.min - p).max(p - rect.max).max(Vec2::ZERO).length()
}

/// Whether a canopy at depth `tree_z` hides `unit`: it draws in front of it (rows further down
/// the screen draw on top, so a unit in the tree's own row or below it is in front of the tree)
/// and covers some of it.
pub fn hides(canopy: Rect, tree_z: f32, unit: Body) -> bool {
    tree_z > unit.z
        && !canopy.intersect(Rect::from_center_half_size(unit.centre, UNIT_HALF)).is_empty()
}

/// The opacity a canopy is heading for: `FADED_ALPHA` when the cursor or the selected unit is
/// within `FADE_RADIUS` of it, or it hides any unit, and 1 otherwise.
pub fn target_alpha(canopy: Rect, tree_z: f32, focus: &Focus) -> f32 {
    let near = |p: Vec2| distance_to(canopy, p) <= FADE_RADIUS;
    let fade = focus.cursor.is_some_and(near)
        || focus.selected.is_some_and(|s| near(s.centre))
        || focus.units.iter().any(|&u| hides(canopy, tree_z, u));
    if fade { FADED_ALPHA } else { 1.0 }
}

/// Moves `current` toward `target` over `dt` seconds, at the speed that takes `FADE_SECONDS` to
/// cross the whole range from `FADED_ALPHA` to 1, without overshooting.
pub fn step_alpha(current: f32, target: f32, dt: f32) -> f32 {
    let step = (1.0 - FADED_ALPHA) / FADE_SECONDS * dt;
    if target > current { (current + step).min(target) } else { (current - step).max(target) }
}

/// A 1x1 transparent image, which the trunks are until `build_trunks` replaces them.
pub fn blank_trunks() -> Image {
    Image::new_fill(
        Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// `FN_CURSOR=X,Y`: the cursor, as map tile coordinates, for a headless shot (no mouse there).
#[derive(Resource)]
struct DemoCursor(Vec2);

fn parse_demo_cursor(s: &str) -> Option<Vec2> {
    let (x, y) = s.split_once(',')?;
    Some(Vec2::new(x.trim().parse::<f32>().ok()? * TILE, -y.trim().parse::<f32>().ok()? * TILE))
}

/// `FN_CANOPY_DEMO=TICKS`: moves the settlers into the densest forest near the first of them, onto
/// free tiles around its middle, designates a few of the trees there, and runs the game `ticks`
/// more (so the settlers have walked off toward them), so a shot shows units under canopies.
/// Prints where the units and designations start.
pub fn place_demo_units(session: &mut fn_ui::Session, ticks: u32) {
    let Some(home) = session.world.units.first().map(|u| u.pos) else { return };
    let map = &session.world.map;
    let trees = |p: Pos| {
        let near = (-3..=3).flat_map(|dy| (-3..=3).map(move |dx| Pos::new(p.x + dx, p.y + dy)));
        near.filter(|q| {
            map.get(*q)
                .and_then(|t| t.resource)
                .is_some_and(|r| session.defs.resource(r).id.ends_with("_tree"))
        })
        .count()
    };
    let free = |p: Pos| map.get(p).is_some_and(|t| t.terrain.is_walkable() && t.resource.is_none());
    let around = |c: Pos, r: i32| {
        (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| Pos::new(c.x + dx, c.y + dy)))
    };
    let Some(heart) = around(home, 25).filter(|p| free(*p)).max_by_key(|p| (trees(*p), p.x, p.y))
    else {
        return;
    };
    let mut spots: Vec<Pos> = around(heart, 6).filter(|p| free(*p)).collect();
    spots.sort_by_key(|p| ((p.x - heart.x).pow(2) + (p.y - heart.y).pow(2), p.x, p.y));
    for (unit, spot) in session.world.units.iter_mut().zip(spots) {
        unit.pos = spot;
        eprintln!("canopy demo: unit {} at {},{}", unit.id.0, spot.x, spot.y);
    }
    // And a few harvest designations on the trees there, which have to show through too.
    let map = &session.world.map;
    let mut marked: Vec<Pos> = around(heart, 3)
        .filter(|p| {
            map.get(*p).and_then(|t| t.resource).is_some_and(|r| session.defs.harvest(r).is_some())
        })
        .collect();
    marked.sort_by_key(|p| ((p.x - heart.x).pow(2) + (p.y - heart.y).pow(2), p.x, p.y));
    for pos in marked.into_iter().step_by(2).take(5) {
        let _ = session.apply(fn_sim::world::Command::DesignateHarvest { pos });
        eprintln!("canopy demo: harvest at {},{}", pos.x, pos.y);
    }
    session.tick_n(ticks);
}

pub struct CanopyPlugin;

/// `FN_CANOPY_FADE=off`: no canopy fades, to shoot the "before" of a scene.
#[derive(Resource)]
struct FadeOff;

impl Plugin for CanopyPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("FN_CANOPY_FADE").is_ok_and(|v| v == "off") {
            app.insert_resource(FadeOff);
        }
        if let Ok(text) = std::env::var("FN_CURSOR") {
            let at = parse_demo_cursor(&text).unwrap_or_else(|| panic!("bad FN_CURSOR {text:?}"));
            app.insert_resource(DemoCursor(at));
        }
        app.add_systems(
            Update,
            (build_trunks, fade_canopies, fade_buildings).chain().in_set(CanopySet),
        );
    }
}

/// Once the tree sheet has loaded, fills `Art::trunks` with the trunks alone.
fn build_trunks(art: Res<Art>, mut images: ResMut<Assets<Image>>, mut done: Local<bool>) {
    if *done {
        return;
    }
    let Some(sheet) = images.get(&art.image(sprites::trees::SHEET)) else { return };
    let (Some(data), format) = (&sheet.data, sheet.texture_descriptor.format) else { return };
    let trunks = match format {
        TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm => Image::new(
            sheet.texture_descriptor.size,
            TextureDimension::D2,
            sprites::trees::trunks_of_sheet(data, sheet.width()),
            format,
            RenderAssetUsages::RENDER_WORLD,
        ),
        // An unexpected pixel format: show the whole trees (the canopy is drawn over them, so it
        // just doesn't fade) rather than no trunks.
        other => {
            warn!("tree sheet is {other:?}, not RGBA8: canopies won't fade");
            sheet.clone()
        }
    };
    *done = true;
    let _ = images.insert(&art.trunks, trunks);
}

/// The cursor in world pixels: the window's, or `FN_CURSOR` in a headless run.
fn cursor_world(
    demo: Option<&DemoCursor>,
    window: &Query<&Window, With<PrimaryWindow>>,
    camera: &Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) -> Option<Vec2> {
    if let Some(demo) = demo {
        return Some(demo.0);
    }
    let cursor = window.single().ok()?.cursor_position()?;
    let (camera, camera_tf) = camera.single().ok()?;
    camera.viewport_to_world_2d(camera_tf, cursor).ok()
}

/// What fades this frame: the cursor, the selected unit and every unit as drawn, or nothing with
/// `FN_CANOPY_FADE=off`.
fn gather_focus(
    selection: &Selection,
    demo: Option<&DemoCursor>,
    fade_off: bool,
    window: &Query<&Window, With<PrimaryWindow>>,
    camera: &Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    units: &Query<(&UnitSprite, &Transform)>,
) -> Focus {
    if fade_off {
        return Focus::default();
    }
    let mut focus = Focus { cursor: cursor_world(demo, window, camera), ..default() };
    for (UnitSprite(id), tf) in units {
        let body = Body { centre: tf.translation.truncate(), z: tf.translation.z };
        if selection.0 == Some(*id) {
            focus.selected = Some(body);
        }
        focus.units.push(body);
    }
    focus
}

/// Moves each canopy's opacity toward `target_alpha` and applies it, on top of the trunk's tint
/// (a resting fruit tree is greyed by `jobview`; its canopy follows).
fn fade_canopies(
    time: Res<Time>,
    selection: Res<Selection>,
    demo: Option<Res<DemoCursor>>,
    fade_off: Option<Res<FadeOff>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    units: Query<(&UnitSprite, &Transform)>,
    trunks: Query<&Sprite, Without<CanopyFade>>,
    mut canopies: Query<(&mut Sprite, &mut CanopyFade, &ChildOf)>,
) {
    let focus =
        gather_focus(&selection, demo.as_deref(), fade_off.is_some(), &window, &camera, &units);
    let dt = time.delta_secs();
    for (mut sprite, mut fade, parent) in &mut canopies {
        let target = target_alpha(fade.bounds, fade.z, &focus);
        fade.alpha = step_alpha(fade.alpha, target, dt);
        let tint = trunks.get(parent.parent()).map_or(Color::WHITE, |t| t.color);
        let color = tint.with_alpha(tint.alpha() * fade.alpha);
        if sprite.color != color {
            sprite.color = color;
        }
    }
}

/// Fades the building sprites that may hide something, by the canopies' rules. `buildings` sets
/// each sprite's own opacity (a site's progress) every frame before this runs, so the fade
/// multiplies that.
fn fade_buildings(
    time: Res<Time>,
    selection: Res<Selection>,
    demo: Option<Res<DemoCursor>>,
    fade_off: Option<Res<FadeOff>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    units: Query<(&UnitSprite, &Transform)>,
    mut buildings: Query<(&mut Sprite, &mut BuildingFade)>,
) {
    if buildings.is_empty() {
        return;
    }
    let focus =
        gather_focus(&selection, demo.as_deref(), fade_off.is_some(), &window, &camera, &units);
    let dt = time.delta_secs();
    for (mut sprite, mut fade) in &mut buildings {
        let target = target_alpha(fade.bounds, fade.z, &focus);
        fade.alpha = step_alpha(fade.alpha, target, dt);
        if fade.alpha < 1.0 {
            let color = sprite.color.with_alpha(sprite.color.alpha() * fade.alpha);
            sprite.color = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A canopy of the tree on tile (10, 10), as `spawn_entities` places it.
    fn tree() -> CanopyFade {
        CanopyFade::new(Pos::new(10, 10))
    }

    /// A body standing on tile `p`, as a unit on that tile is drawn.
    fn on_tile(p: Pos) -> Body {
        let at = crate::unit_translation(p);
        Body { centre: at.truncate(), z: at.z }
    }

    fn focus_on(cursor: Option<Vec2>, units: &[Body]) -> Focus {
        Focus { cursor, selected: None, units: units.to_vec() }
    }

    #[test]
    fn a_canopy_with_nothing_near_is_opaque() {
        let t = tree();
        let far = Pos::new(30, 30);
        let focus = focus_on(Some(tile_translation(far, 0.0).truncate()), &[on_tile(far)]);
        assert_eq!(target_alpha(t.bounds, t.z, &focus), 1.0);
        assert_eq!(target_alpha(t.bounds, t.z, &Focus::default()), 1.0);
    }

    #[test]
    fn the_cursor_fades_a_canopy_within_the_radius_of_its_edge() {
        let t = tree();
        let edge = t.bounds.max.x;
        let at = |dx: f32| focus_on(Some(Vec2::new(edge + dx, t.bounds.center().y)), &[]);
        assert_eq!(target_alpha(t.bounds, t.z, &at(FADE_RADIUS)), FADED_ALPHA);
        assert_eq!(target_alpha(t.bounds, t.z, &at(FADE_RADIUS + 0.5)), 1.0);
        // Over the canopy itself.
        assert_eq!(target_alpha(t.bounds, t.z, &at(-4.0)), FADED_ALPHA);
        // The corner is measured as a distance, not as a box: 9px right and 9px up is 12.7 away.
        let corner = focus_on(Some(t.bounds.max + Vec2::splat(9.0)), &[]);
        assert_eq!(target_alpha(t.bounds, t.z, &corner), 1.0);
    }

    #[test]
    fn the_cursor_on_the_tile_above_a_tree_fades_it() {
        // The hover outline on the tile behind a tree is what the issue asks to keep visible.
        let t = tree();
        for (dx, dy) in [(0, -1), (-1, -1), (1, -1), (0, -2)] {
            let cursor = tile_translation(Pos::new(10 + dx, 10 + dy), 0.0).truncate();
            let focus = focus_on(Some(cursor), &[]);
            assert_eq!(target_alpha(t.bounds, t.z, &focus), FADED_ALPHA, "tile {dx},{dy}");
        }
        // Three rows up is clear of it.
        let cursor = tile_translation(Pos::new(10, 7), 0.0).truncate();
        assert_eq!(target_alpha(t.bounds, t.z, &focus_on(Some(cursor), &[])), 1.0);
    }

    #[test]
    fn a_unit_behind_the_tree_is_hidden_by_it_and_one_in_front_is_not() {
        let t = tree();
        // The row above covers the unit, and so do the columns either side (the canopy is two
        // tiles wide); further out or two rows up is clear of it.
        for dx in [-1, 0, 1] {
            assert!(hides(t.bounds, t.z, on_tile(Pos::new(10 + dx, 9))), "{dx}");
        }
        for (dx, dy) in [(2, -1), (-2, -1), (0, -2), (0, -3)] {
            assert!(!hides(t.bounds, t.z, on_tile(Pos::new(10 + dx, 10 + dy))), "{dx},{dy}");
        }
        // In the tree's own row and below it, the unit draws over the tree.
        for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1)] {
            assert!(!hides(t.bounds, t.z, on_tile(Pos::new(10 + dx, 10 + dy))), "{dx},{dy}");
        }
    }

    #[test]
    fn a_unit_under_the_canopy_fades_it() {
        let t = tree();
        let under = focus_on(None, &[on_tile(Pos::new(10, 9))]);
        assert_eq!(target_alpha(t.bounds, t.z, &under), FADED_ALPHA);
        let beside = focus_on(None, &[on_tile(Pos::new(13, 9))]);
        assert_eq!(target_alpha(t.bounds, t.z, &beside), 1.0);
    }

    #[test]
    fn a_unit_mid_step_counts_by_the_depth_it_is_drawn_at() {
        let t = tree();
        // Walking from the tree's row up to the row above: in front of it until the drawn z
        // drops below the tree's, hidden by it after.
        let (from, to) = (on_tile(Pos::new(10, 10)), on_tile(Pos::new(10, 9)));
        let at = |f: f32| Body {
            centre: from.centre.lerp(to.centre, f),
            z: from.z + (to.z - from.z) * f,
        };
        assert!(!hides(t.bounds, t.z, at(0.0)));
        assert!(!hides(t.bounds, t.z, at(0.5)));
        assert!(hides(t.bounds, t.z, at(1.0)));
    }

    #[test]
    fn the_selected_unit_fades_the_canopies_near_it_even_in_front_of_them() {
        let t = tree();
        // Beside the canopy and drawn over it, so it hides nothing, but within the radius.
        let near = Body { centre: t.bounds.max + Vec2::new(FADE_RADIUS / 2.0, 0.0), z: t.z + 1.0 };
        let mut focus = focus_on(None, &[near]);
        assert_eq!(target_alpha(t.bounds, t.z, &focus), 1.0, "only the selected unit has a radius");
        focus.selected = Some(near);
        assert_eq!(target_alpha(t.bounds, t.z, &focus), FADED_ALPHA);
        focus.selected = Some(Body { centre: near.centre + Vec2::X * FADE_RADIUS, ..near });
        assert_eq!(target_alpha(t.bounds, t.z, &focus), 1.0);
    }

    #[test]
    fn alpha_steps_to_its_target_in_fade_seconds_without_overshooting() {
        let close = |a: f32, b: f32| assert!((a - b).abs() < 1e-5, "{a} != {b}");
        close(step_alpha(1.0, FADED_ALPHA, FADE_SECONDS), FADED_ALPHA);
        close(step_alpha(FADED_ALPHA, 1.0, FADE_SECONDS), 1.0);
        assert_eq!(step_alpha(1.0, FADED_ALPHA, 10.0), FADED_ALPHA);
        assert_eq!(step_alpha(0.5, 0.5, 1.0), 0.5);
        let half = step_alpha(1.0, FADED_ALPHA, FADE_SECONDS / 2.0);
        assert!((half - (1.0 + FADED_ALPHA) / 2.0).abs() < 1e-6, "{half}");
        // Frame by frame, at 60 fps, it gets there in `FADE_SECONDS`.
        let mut a = 1.0;
        let frames = (FADE_SECONDS * 60.0).ceil() as usize;
        for _ in 0..frames {
            a = step_alpha(a, FADED_ALPHA, 1.0 / 60.0);
        }
        assert_eq!(a, FADED_ALPHA);
    }

    #[test]
    fn parses_the_demo_cursor() {
        assert_eq!(parse_demo_cursor("3,4"), Some(Vec2::new(3.0 * TILE, -4.0 * TILE)));
        assert_eq!(parse_demo_cursor(" 2.5 , 1 "), Some(Vec2::new(2.5 * TILE, -TILE)));
        assert_eq!(parse_demo_cursor("3"), None);
        assert_eq!(parse_demo_cursor("a,b"), None);
    }
}

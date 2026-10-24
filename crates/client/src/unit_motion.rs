//! Where a unit's sprite is between two tiles (#194). View state only: it reads the sim through
//! `World::stride` and `World::steps_per_tick` and writes nothing back.
//!
//! The sim puts a unit on a tile only when its step progress (`move_speed` per tick) reaches what
//! the step needs, so a sprite that merely chased the unit's tile crossed it in a burst and then
//! stood still. Instead the sprite goes where the sim *is* along the step:
//!
//! - **Walking a route** (a job, a need): once per fixed step (`FixedPostUpdate`, after the sim
//!   ran) `sample_motion` reads the unit's [`Stride`] and puts a sample on the line the walk
//!   follows (`place`): the tile it is on plus the progress made over what the step needs. The
//!   sprite is drawn between the last two samples by how far into the fixed step the frame is
//!   (`Time<Fixed>::overstep_fraction`), the usual fixed-step interpolation. The speed is the sim's
//!   own: `move_speed`, terrain cost, diagonals and the game speed (ticks per fixed step) are all
//!   in the samples, and a paused game takes no more samples, so a unit holds where it is.
//! - **Anything else** (an idle wander, whose next tile is drawn from the random stream only when
//!   the step lands): the sprite chases the unit's tile at the unit's walking pace
//!   (`glide_speed`), which keeps pace with successive steps and holds while paused.
//! - **Teleports** (`is_teleport`: further than the sim can move a unit in the ticks that ran,
//!   or farther than `SNAP_TILES` for a chase) put the sprite straight on the tile.
//!
//! The sprite's `Transform` is what the selection ring, the canopy fade and `unit_anim` read.

use bevy::prelude::*;
use fn_sim::Pos;
use fn_sim::world::{Stride, TICKS_PER_SECOND};

use super::{Sim, TILE, UnitSprite, unit_translation};

/// A chasing sprite further than this many tiles from its unit snaps to it instead of gliding.
pub const SNAP_TILES: f32 = 3.0;
/// The most tiles a unit moves in one tick (a diagonal step), with a little slack: a jump further
/// than this many tiles per tick that ran is a teleport, not a walk.
pub const MAX_TILES_PER_TICK: f32 = 1.5;

/// Which step of a walk a [`Stride`] is in and how far into it: `(i, t)`, the unit heading for
/// `tiles[i + 1]` and `t` of the way there, in `[0, 1]`. `needed[i]` is what step `i` takes and
/// `progress` is the progress made toward the first one. Progress beyond the last step is
/// "arrived": `(needed.len() - 1, 1.0)`. Nothing needed (`needed` empty) gives `(0, 0.0)`.
pub fn locate(needed: &[f32], progress: f32) -> (usize, f32) {
    let mut left = progress.max(0.0);
    for (i, &n) in needed.iter().enumerate() {
        if left < n {
            return (i, left / n);
        }
        left -= n;
    }
    (needed.len().saturating_sub(1), if needed.is_empty() { 0.0 } else { 1.0 })
}

/// A point on a unit's walk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Where the sprite is (z is the depth of the rows it crosses).
    pub at: Vec3,
    /// The tile it is leaving and the one it is heading for.
    pub from: Pos,
    pub to: Pos,
}

/// The point `extra` ticks of walking past the state the sim is in: the stride's progress plus
/// `extra * speed`, on the line through its tiles. This is the whole of the position rule; the
/// sim supplied everything it depends on.
pub fn place(s: &Stride, extra: f32) -> Placement {
    let (i, t) = locate(&s.needed, s.progress + extra * s.speed);
    let (from, to) = (s.tiles[i], s.tiles[i + 1]);
    Placement { at: unit_translation(from).lerp(unit_translation(to), t), from, to }
}

/// Whether a jump of `distance` pixels between two samples, with `ticks` sim ticks run in between,
/// is further than a walk can go (a new map, a nation switch, a unit placed somewhere).
pub fn is_teleport(distance: f32, ticks: u32) -> bool {
    distance > MAX_TILES_PER_TICK * TILE * ticks.max(1) as f32
}

/// How fast a sprite chasing its unit goes, in pixels per real second: the unit's straight-step
/// pace at the game speed, 0 while paused.
pub fn glide_speed(steps_per_tick: f32, game_speed: u32, paused: bool) -> f32 {
    if paused { 0.0 } else { steps_per_tick * TILE * (TICKS_PER_SECOND * game_speed) as f32 }
}

/// A sprite at `from` chasing `to` at `speed` pixels a second for `dt` seconds: it moves toward
/// it by that far without overshooting, and snaps if it is further than `SNAP_TILES` away.
pub fn chase(from: Vec3, to: Vec3, speed: f32, dt: f32) -> Vec3 {
    let gap = (to - from).truncate().length();
    if gap > SNAP_TILES * TILE {
        return to;
    }
    let reach = (speed * dt).max(0.0);
    if gap <= reach { to } else { from.lerp(to, reach / gap) }
}

/// A unit sprite's motion state: the last two fixed-step samples of where the unit is.
#[derive(Component)]
pub struct UnitMotion {
    prev: Vec3,
    cur: Vec3,
    /// The unit was on a route at the last sample, so `prev` to `cur` is a stretch of its walk.
    striding: bool,
    /// The last sample was a teleport: put the sprite on `cur` at once.
    snap: bool,
    /// The tile the sprite is heading for: the end of the step it is in, else the unit's tile.
    pub dest: Pos,
    /// The way the current step goes, `(dx, dy)` in tiles, while on a route.
    pub heading: Option<(i32, i32)>,
}

impl UnitMotion {
    /// A unit standing on `pos`, drawn at `at`.
    pub fn new(pos: Pos, at: Vec3) -> Self {
        UnitMotion { prev: at, cur: at, striding: false, snap: false, dest: pos, heading: None }
    }
}

/// Takes a sample of each unit's place after the sim has run for a fixed step.
pub fn sample_motion(sim: Res<Sim>, mut units: Query<(&UnitSprite, &Transform, &mut UnitMotion)>) {
    let ticks = sim.clock.per_fixed_step();
    for (UnitSprite(id), tf, mut m) in &mut units {
        let Some(unit) = sim.world.unit(*id) else { continue };
        let stride = sim.world.stride(*id);
        let (at, dest, heading) = match &stride {
            Some(s) => {
                let p = place(s, 0.0);
                (p.at, p.to, Some((p.to.x - p.from.x, p.to.y - p.from.y)))
            }
            None => (unit_translation(unit.pos), unit.pos, None),
        };
        m.dest = dest;
        m.heading = heading;
        if is_teleport((at - m.cur).truncate().length(), ticks) {
            m.prev = at;
            m.cur = at;
            m.snap = true;
            m.striding = stride.is_some();
            continue;
        }
        // Between two samples of one walk the line is exact; coming onto a walk from a glide, start
        // from where the sprite is.
        m.prev = if m.striding && stride.is_some() { m.cur } else { tf.translation };
        m.cur = at;
        m.striding = stride.is_some();
    }
}

/// Moves each unit sprite to where its unit is along its step (see the module docs).
pub fn sync_units(
    sim: Res<Sim>,
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
    mut units: Query<(&UnitSprite, &mut Transform, &mut UnitMotion)>,
) {
    let overstep = fixed.overstep_fraction();
    let speed = glide_speed(sim.world.steps_per_tick(), sim.clock.speed(), sim.clock.paused);
    for (_, mut tf, mut m) in &mut units {
        if m.snap {
            m.snap = false;
            tf.translation = m.cur;
        } else if m.striding {
            tf.translation = m.prev.lerp(m.cur, overstep);
        } else {
            tf.translation = chase(tf.translation, m.cur, speed, time.delta_secs());
        }
    }
}

pub struct UnitMotionPlugin;

impl Plugin for UnitMotionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedPostUpdate, sample_motion);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stride(tiles: &[(i32, i32)], needed: &[f32], progress: f32, speed: f32) -> Stride {
        Stride {
            tiles: tiles.iter().map(|&(x, y)| Pos::new(x, y)).collect(),
            needed: needed.to_vec(),
            progress,
            speed,
        }
    }

    fn x_of(s: &Stride, extra: f32) -> f32 {
        place(s, extra).at.x
    }

    #[test]
    fn locate_finds_the_step_and_how_far_into_it() {
        let needed = [6.0, 6.0, 8.0];
        assert_eq!(locate(&needed, 0.0), (0, 0.0));
        assert_eq!(locate(&needed, 3.0), (0, 0.5));
        assert_eq!(locate(&needed, 6.0), (1, 0.0));
        assert_eq!(locate(&needed, 16.0), (2, 0.5));
        assert_eq!(locate(&needed, 99.0), (2, 1.0), "past the end is arrived");
        assert_eq!(locate(&needed, -1.0), (0, 0.0));
        assert_eq!(locate(&[], 3.0), (0, 0.0));
    }

    #[test]
    fn mid_step_the_sprite_is_between_the_two_tiles() {
        let s = stride(&[(3, 3), (4, 3)], &[6.0], 3.0, 1.0);
        let (a, b) = (unit_translation(Pos::new(3, 3)), unit_translation(Pos::new(4, 3)));
        let p = place(&s, 0.0);
        assert_eq!((p.from, p.to), (Pos::new(3, 3), Pos::new(4, 3)));
        assert_eq!(p.at, a.lerp(b, 0.5));
        assert_eq!(x_of(&s, 0.0), a.x + 8.0);
    }

    #[test]
    fn arrival_puts_the_sprite_on_the_last_tile_and_no_further() {
        let s = stride(&[(3, 3), (4, 3)], &[6.0], 5.0, 1.0);
        let end = unit_translation(Pos::new(4, 3));
        assert_eq!(place(&s, 1.0).at, end);
        assert_eq!(place(&s, 50.0).at, end);
    }

    #[test]
    fn the_sprite_goes_at_a_constant_speed_across_tiles() {
        // Three straight steps, progress 1.5 ticks' worth a tick: the x advance per tick never
        // changes, including across the tile boundaries at progress 6 and 12.
        let s = stride(&[(0, 0), (1, 0), (2, 0), (3, 0)], &[6.0; 3], 0.0, 1.5);
        let xs: Vec<f32> = (0..=12).map(|t| x_of(&s, t as f32)).collect();
        for w in xs.windows(2).take(11) {
            assert!((w[1] - w[0] - 1.5 * TILE / 6.0).abs() < 1e-3, "{xs:?}");
        }
        assert_eq!(*xs.last().unwrap(), unit_translation(Pos::new(3, 0)).x);
    }

    #[test]
    fn a_faster_unit_covers_ground_faster_and_a_costly_step_is_slower() {
        let slow = stride(&[(0, 0), (1, 0)], &[6.0], 0.0, 1.0);
        let fast = stride(&[(0, 0), (1, 0)], &[6.0], 0.0, 1.6);
        let dx = |s: &Stride| x_of(s, 2.0) - x_of(s, 0.0);
        assert!((dx(&fast) / dx(&slow) - 1.6).abs() < 1e-4);
        let costly = stride(&[(0, 0), (1, 0)], &[12.0], 0.0, 1.0);
        assert!((dx(&slow) / dx(&costly) - 2.0).abs() < 1e-4);
    }

    #[test]
    fn game_speed_scales_the_ticks_between_samples() {
        // At 3x the sim runs 3 ticks per fixed step, so the samples are 3 ticks of walking apart.
        let s = stride(&[(0, 0), (1, 0), (2, 0)], &[6.0, 6.0], 0.0, 1.0);
        let per_fixed_step = |speed: u32| x_of(&s, speed as f32) - x_of(&s, 0.0);
        assert!((per_fixed_step(3) / per_fixed_step(1) - 3.0).abs() < 1e-4);
        assert!((glide_speed(0.2, 3, false) / glide_speed(0.2, 1, false) - 3.0).abs() < 1e-4);
    }

    #[test]
    fn a_paused_game_holds_a_chasing_sprite() {
        assert_eq!(glide_speed(0.2, 5, true), 0.0);
        let from = Vec3::new(10.0, 0.0, 1.0);
        let to = Vec3::new(26.0, 0.0, 1.0);
        assert_eq!(chase(from, to, 0.0, 1.0), from);
    }

    #[test]
    fn a_chase_moves_at_its_speed_and_stops_on_the_tile() {
        let from = Vec3::new(0.0, 0.0, 1.0);
        let to = Vec3::new(16.0, 0.0, 1.0);
        let mid = chase(from, to, 20.0, 0.25);
        assert!((mid.x - 5.0).abs() < 1e-4, "{mid}");
        assert_eq!(chase(mid, to, 20.0, 5.0), to, "no overshoot");
        assert_eq!(chase(to, to, 20.0, 1.0), to);
    }

    #[test]
    fn a_chase_snaps_when_the_gap_is_not_a_walk() {
        let from = Vec3::new(0.0, 0.0, 1.0);
        let far = Vec3::new(4.0 * TILE, 0.0, 1.0);
        assert_eq!(chase(from, far, 1.0, 0.01), far);
        let near = Vec3::new(2.0 * TILE, 0.0, 1.0);
        assert_ne!(chase(from, near, 1.0, 0.01), near);
    }

    #[test]
    fn a_teleport_is_a_jump_a_walk_could_not_make_in_the_ticks_that_ran() {
        assert!(!is_teleport(TILE, 1), "one step in a tick");
        assert!(!is_teleport(1.42 * TILE, 1), "a diagonal in a tick");
        assert!(is_teleport(2.0 * TILE, 1), "two tiles in a tick");
        assert!(!is_teleport(2.0 * TILE, 5), "but not in five");
        assert!(!is_teleport(TILE, 0), "a single-step while paused is one tick");
        assert!(is_teleport(30.0 * TILE, 5), "a new map");
    }

    #[test]
    fn a_sprite_crossing_a_row_boundary_changes_depth_smoothly() {
        let s = stride(&[(3, 3), (3, 4)], &[6.0], 3.0, 1.0);
        let (a, b) = (unit_translation(Pos::new(3, 3)).z, unit_translation(Pos::new(3, 4)).z);
        assert!((place(&s, 0.0).at.z - (a + b) / 2.0).abs() < 1e-5);
    }

    // ---- in an app, on a real world ----

    use crate::sprites::CELL;
    use crate::unit_anim;
    use crate::unit_anim::{UnitView, animate_units};
    use fn_sim::world::Command;
    use fn_sim::world::UnitId;
    use std::time::Duration;

    const FRAME: Duration = Duration::from_micros(16_667);
    /// Frames in a fixed step (a tenth of a second).
    const FIXED_STEP_FRAMES: usize = 7;

    /// A game of one settler with a tree ten tiles east of it, marked for harvest, so that it
    /// walks a straight 9-tile path; a headless app that runs the sim at its fixed step as the
    /// game does, with `sample_motion`, `sync_units` and `animate_units`, 60 frames a second.
    fn walking_app(speed_ups: u32) -> (App, Entity, UnitId) {
        let mut session = fn_ui::Session::new(1, fn_sim::Nation::Earth, 48, 32);
        let origin = session.world.units[0].pos;
        crate::level_meadow(&mut session, origin);
        session.world.units.truncate(1);
        let tree = Pos::new(origin.x + 10, origin.y);
        session.world.map.get_mut(tree).unwrap().resource = session.defs.resource_id("oak_tree");
        session.apply(Command::DesignateHarvest { pos: tree }).unwrap();
        for _ in 0..speed_ups {
            session.clock.faster();
        }
        let id = session.world.units[0].id;
        let sim = Sim(session);

        let mut app = App::new();
        crate::insert_test_art(&mut app);
        app.add_plugins(bevy::time::TimePlugin)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(FRAME))
            .insert_resource(Time::<Fixed>::from_hz(f64::from(TICKS_PER_SECOND)))
            .insert_resource(sim)
            .add_systems(FixedUpdate, crate::tick_sim)
            .add_plugins(UnitMotionPlugin)
            .add_systems(Update, (sync_units, animate_units).chain());
        let at = unit_translation(origin);
        let character = {
            let sim = app.world().resource::<Sim>();
            crate::unit_character(sim, &sim.world.units[0]).to_owned()
        };
        let sprite = app.world().resource::<crate::Art>().unit_sprite(&character);
        let e = app
            .world_mut()
            .spawn((
                sprite,
                Transform::from_translation(at),
                UnitSprite(id),
                UnitView::new(origin, at.truncate()),
                UnitMotion::new(origin, at),
            ))
            .id();
        (app, e, id)
    }

    /// Runs one frame and returns where the sprite is, whether the unit is walking a route, and
    /// the sheet row drawn.
    fn frame(app: &mut App, e: Entity, id: UnitId) -> (Vec2, bool, u32) {
        app.update();
        let tf = app.world().get::<Transform>(e).unwrap().translation.truncate();
        let walking = app.world().resource::<Sim>().world.stride(id).is_some();
        let row = app.world().get::<Sprite>(e).unwrap().rect.map_or(0, |r| (r.min.y / CELL) as u32);
        (tf, walking, row)
    }

    /// Walks the settler to its tree at game speed `1 + speed_ups` steps up, recording the sprite
    /// every frame until it has stopped at the end of the path.
    fn record_walk(speed_ups: u32) -> (Vec<(Vec2, bool, u32)>, f32) {
        let (mut app, e, id) = walking_app(speed_ups);
        let sim = app.world().resource::<Sim>();
        let expected = glide_speed(sim.world.steps_per_tick(), sim.clock.speed(), false);
        let mut frames = vec![frame(&mut app, e, id)];
        let mut still = 0;
        for _ in 0..1500 {
            let f = frame(&mut app, e, id);
            still = if f.0 == frames.last().unwrap().0 { still + 1 } else { 0 };
            frames.push(f);
            if !f.1 && still >= 30 && frames.len() > 60 {
                break;
            }
        }
        (frames, expected / 60.0)
    }

    /// The frames from the first movement to the sprite's arrival on its last tile.
    fn mid_path(frames: &[(Vec2, bool, u32)]) -> &[(Vec2, bool, u32)] {
        let far = |a: Vec2, b: Vec2| a.distance(b) > 0.01;
        let start = frames.iter().position(|f| far(f.0, frames[0].0)).expect("it moved");
        let end = frames.iter().rposition(|f| far(f.0, frames.last().unwrap().0)).expect("stopped");
        &frames[start - 1..=end + 1]
    }

    #[test]
    fn a_walking_sprite_never_stalls_between_tiles_at_any_game_speed() {
        for speed_ups in 0..4 {
            let (frames, per_frame) = record_walk(speed_ups);
            let path = mid_path(&frames);
            let tiles = (path.last().unwrap().0.x - path[0].0.x) / TILE;
            assert!(tiles >= 8.0, "speed {speed_ups}: it walked {tiles} tiles");
            // Every frame from start to arrival moves the sprite, along the row, and at the pace
            // the sim walks (within the fixed step's rounding), however many tiles the sim moves it
            // in one fixed step.
            // (Leaving out the first and last fixed step, which ease the sprite in and out.)
            let steady = &path[FIXED_STEP_FRAMES..path.len() - FIXED_STEP_FRAMES];
            for (n, w) in steady.windows(2).enumerate() {
                let d = w[1].0 - w[0].0;
                assert!(d.y.abs() < 1e-3, "speed {speed_ups} frame {n}: left the row: {d}");
                assert!(
                    d.x > 0.5 * per_frame && d.x < 1.5 * per_frame,
                    "speed {speed_ups} frame {n}: moved {} px, pace {per_frame}",
                    d.x
                );
            }
            // Overall it keeps the pace to within a few percent.
            let covered = path.last().unwrap().0.x - path[0].0.x;
            let took = (path.len() - 1) as f32 * per_frame;
            assert!((covered / took - 1.0).abs() < 0.1, "speed {speed_ups}: {covered} in {took}");
        }
    }

    #[test]
    fn the_walk_cycle_plays_through_the_whole_walk_and_ends_on_row_zero() {
        let (frames, _) = record_walk(0);
        let path = mid_path(&frames);
        let mut rows: Vec<u32> = path.iter().map(|f| f.2).collect();
        rows.dedup();
        assert!(rows.len() >= 12, "it cycled through several strides: {rows:?}");
        for w in rows.windows(2) {
            assert_eq!(w[1], (w[0] + 1) % unit_anim::WALK_ROWS, "no idle frame mid-path: {rows:?}");
        }
        assert_eq!(frames.last().unwrap().2, 0, "standing again on arrival");
    }

    #[test]
    fn a_paused_game_holds_the_sprite_mid_step_and_resuming_carries_on() {
        let (mut app, e, id) = walking_app(0);
        let start = frame(&mut app, e, id).0;
        for _ in 0..70 {
            frame(&mut app, e, id);
        }
        app.world_mut().resource_mut::<Sim>().clock.paused = true;
        // A fixed step's worth of frames for what was already in flight to land.
        for _ in 0..12 {
            frame(&mut app, e, id);
        }
        let held = frame(&mut app, e, id).0;
        assert!(held.x > start.x + 5.0, "it was part way along: {start} -> {held}");
        for n in 0..90 {
            let at = frame(&mut app, e, id).0;
            assert!(at.distance(held) < 1e-3, "still held {n} frames on: {held} -> {at}");
        }
        app.world_mut().resource_mut::<Sim>().clock.paused = false;
        let mut at = held;
        for _ in 0..30 {
            at = frame(&mut app, e, id).0;
        }
        assert!(at.x > held.x + 5.0, "walking again: {held} -> {at}");
    }

    #[test]
    fn a_unit_placed_far_away_is_snapped_to_not_glided_to() {
        let (mut app, e, id) = walking_app(0);
        let home = unit_translation(app.world().resource::<Sim>().world.units[0].pos).truncate();
        let far = Pos::new(
            app.world().resource::<Sim>().world.units[0].pos.x,
            app.world().resource::<Sim>().world.units[0].pos.y - 6,
        );
        app.world_mut().resource_mut::<Sim>().world.units[0].pos = far;
        // The first fixed step takes the sample. Until then the sprite is where it was; then it
        // is on the far tile in one frame, and was never anywhere between.
        let seen: Vec<Vec2> = (0..12).map(|_| frame(&mut app, e, id).0).collect();
        let there = unit_translation(far).truncate();
        let jump = seen.iter().position(|p| p.distance(home) > 1.0).expect("it moved");
        assert!(seen[..jump].iter().all(|p| p.distance(home) < 1e-3), "{seen:?}");
        assert!(seen[jump].distance(there) < TILE, "{:?} is not near {there}", seen[jump]);
    }
}

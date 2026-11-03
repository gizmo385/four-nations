//! `FN_WALK_DEMO=FRAMES`: a stage for shots of units walking (#165, #194). The ground east of the
//! first settler is levelled, three settlers are put at the edges of the meadow with a tree each
//! marked for harvest 7 to 9 tiles away (one walks east, one west, one south), and the game runs
//! as it does in play. Time is driven by hand at 60 frames a second (`TimeUpdateStrategy`), and
//! the game starts once the art has loaded, so after `FRAMES` frames the walkers are in the same
//! place whatever the machine; then the game pauses and a headless shot is taken. Different
//! values catch different moments of the walk, which is how to make a frame strip.
//!
//! Everything is the game as played: the settlers walk their own routes in the sim, and the
//! sprites follow `unit_motion` and `unit_anim` as in play.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use fn_sim::Pos;
use fn_sim::world::Command;
use fn_ui::Session;

use super::{Art, Sim, TILE};

/// The length of every frame of the demo, as a 60 fps game would see it.
pub const FRAME: Duration = Duration::from_micros(16_667);
/// The camera's scale: 0.35 puts the whole meadow in view at nearly 3x.
const ZOOM: f32 = 0.35;

/// Where a walker starts and where its tree is, from the first settler's tile.
const WALKS: [((i32, i32), (i32, i32)); 3] =
    [((1, -7), (10, -7)), ((20, 6), (13, 6)), ((5, -4), (5, 3))];

#[derive(Resource)]
pub struct WalkDemo {
    frames: u32,
    until: u32,
    /// Where the camera looks.
    centre: Vec2,
}

impl WalkDemo {
    /// Whether the demo has run as many frames as it was asked to.
    pub fn done(&self) -> bool {
        self.frames >= self.until
    }
}

/// How the app should keep time for the demo.
pub fn time_strategy() -> TimeUpdateStrategy {
    TimeUpdateStrategy::ManualDuration(FRAME)
}

/// Levels the ground around the first settler, puts the settlers at the starts of their walks, marks
/// their trees for harvest, and holds the game until the art is in (`run`).
pub fn prepare(session: &mut Session, until: u32) -> WalkDemo {
    let origin = session.world.units.first().map(|u| u.pos).unwrap_or(Pos::new(0, 0));
    super::level_meadow(session, origin);
    session.clock.paused = true;
    let at = |(dx, dy): (i32, i32)| Pos::new(origin.x + dx, origin.y + dy);
    session.world.units.truncate(WALKS.len());
    let tree = session.defs.resource_id("oak_tree");
    for (unit, &(from, to)) in session.world.units.iter_mut().zip(&WALKS) {
        unit.pos = at(from);
        if let Some(tile) = session.world.map.get_mut(at(to)) {
            tile.resource = tree;
        }
    }
    for &(_, to) in &WALKS {
        let _ = session.apply(Command::DesignateHarvest { pos: at(to) });
    }
    // The meadow's middle, nudged right of centre of the view since the side panel covers its
    // right edge.
    let middle = Vec2::new((origin.x as f32 + 10.0) * TILE, -(origin.y as f32 - 0.5) * TILE);
    let centre = middle + Vec2::X * ZOOM * 128.0;
    WalkDemo { frames: 0, until, centre }
}

/// Starts the game once the art is in, counts frames, and pauses it for the shot after the last.
pub fn run(art: Res<Art>, mut demo: ResMut<WalkDemo>, mut sim: ResMut<Sim>) {
    if !art.all_loaded() || demo.done() {
        sim.clock.paused = true;
        return;
    }
    sim.clock.paused = false;
    demo.frames += 1;
}

/// Looks at the meadow, close up.
pub fn frame_camera(
    demo: Res<WalkDemo>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    for (mut tf, mut projection) in &mut camera {
        tf.translation = demo.centre.extend(tf.translation.z);
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = ZOOM;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stage_has_a_tree_for_each_walker_and_each_walk_is_long_and_inside_the_meadow() {
        let mut session = Session::new(7, fn_sim::Nation::Earth, 96, 64);
        let origin = session.world.units[0].pos;
        let demo = prepare(&mut session, 100);
        assert!(!demo.done());
        assert!(session.clock.paused, "held until the art is in");
        assert_eq!(session.world.units.len(), WALKS.len());
        for &((fx, fy), (tx, ty)) in &WALKS {
            assert!((tx - fx).abs() + (ty - fy).abs() >= 7, "a long walk");
            for (x, y) in [(fx, fy), (tx, ty)] {
                assert!((0..=20).contains(&x) && (-9..=8).contains(&y), "on the meadow");
                assert!(session.world.map.get(Pos::new(origin.x + x, origin.y + y)).is_some());
            }
        }
        // They set off at once and walk their own routes.
        session.clock.paused = false;
        session.tick_n(2);
        for u in &session.world.units {
            assert!(session.world.stride(u.id, &session.defs).is_some(), "{:?} is walking", u.id);
        }
    }
}

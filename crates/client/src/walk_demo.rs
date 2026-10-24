//! `FN_WALK_DEMO=STEPS`: a stage for shots of the walk cycles (#165). The game is paused, the
//! ground east of the first settler is levelled, and the settlers walk to and fro on fixed tracks
//! (one tile every `STEP_SECS`) so every facing is on screen; a headless shot is taken once
//! `STEPS` steps have been walked, so different values catch different frames of the cycle.
//! The sprites follow the same `sync_units` and `unit_anim` path as in play.

use bevy::prelude::*;
use fn_sim::Pos;
use fn_ui::Session;

use super::{Art, Sim, TILE};

/// Real seconds between two steps of the demo walkers.
const STEP_SECS: f32 = 0.3;
/// The camera's scale: 0.35 puts the whole meadow in view at nearly 3x.
const ZOOM: f32 = 0.35;

/// A walker going to and fro over `len` tiles from `base` in direction `dir`, `phase` steps in.
struct Track {
    base: Pos,
    dir: (i32, i32),
    len: i32,
    phase: i32,
}

impl Track {
    /// Where it stands after `steps` steps: a triangle wave over the track.
    fn at(&self, steps: u32) -> Pos {
        let period = 2 * self.len;
        let m = (steps as i32 + self.phase).rem_euclid(period);
        let along = if m <= self.len { m } else { period - m };
        Pos::new(self.base.x + self.dir.0 * along, self.base.y + self.dir.1 * along)
    }
}

#[derive(Resource)]
pub struct WalkDemo {
    tracks: Vec<Track>,
    steps: u32,
    until: u32,
    since_step: f32,
    /// Where the camera looks.
    centre: Vec2,
}

impl WalkDemo {
    /// Whether the demo has walked as many steps as it was asked to.
    pub fn done(&self) -> bool {
        self.steps >= self.until
    }
}

/// Levels the ground around the first settler, pauses the game and puts the settlers on their
/// tracks: two across, two up and down, one on the diagonal, one standing.
pub fn prepare(session: &mut Session, until: u32) -> WalkDemo {
    let origin = session.world.units.first().map(|u| u.pos).unwrap_or(Pos::new(0, 0));
    super::level_meadow(session, origin);
    session.clock.paused = true;
    let at = |dx: i32, dy: i32| Pos::new(origin.x + dx, origin.y + dy);
    let tracks = vec![
        Track { base: at(1, -5), dir: (1, 0), len: 5, phase: 0 },
        Track { base: at(1, -2), dir: (1, 0), len: 5, phase: 5 },
        Track { base: at(9, -6), dir: (0, 1), len: 6, phase: 0 },
        Track { base: at(12, -6), dir: (0, 1), len: 6, phase: 6 },
        Track { base: at(14, -6), dir: (1, 1), len: 4, phase: 0 },
        Track { base: at(7, 3), dir: (0, 0), len: 1, phase: 0 },
    ];
    for (unit, track) in session.world.units.iter_mut().zip(&tracks) {
        unit.pos = track.at(0);
    }
    // The meadow's middle, nudged right of centre of the view since the side panel covers its
    // right edge.
    let middle = Vec2::new((origin.x as f32 + 9.5) * TILE, -(origin.y as f32 - 1.5) * TILE);
    let centre = middle + Vec2::X * ZOOM * 128.0;
    WalkDemo { tracks, steps: 0, until, since_step: 0.0, centre }
}

/// Walks the demo settlers on, once the art is in.
pub fn step_walkers(
    time: Res<Time<Real>>,
    art: Res<Art>,
    mut demo: ResMut<WalkDemo>,
    mut sim: ResMut<Sim>,
) {
    if !art.all_loaded() || demo.done() {
        return;
    }
    demo.since_step += time.delta_secs();
    if demo.since_step < STEP_SECS {
        return;
    }
    demo.since_step -= STEP_SECS;
    demo.steps += 1;
    for (unit, track) in sim.world.units.iter_mut().zip(&demo.tracks) {
        unit.pos = track.at(demo.steps);
    }
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
    fn a_track_goes_to_and_fro() {
        let t = Track { base: Pos::new(5, 5), dir: (1, 0), len: 2, phase: 0 };
        let xs: Vec<i32> = (0..6).map(|n| t.at(n).x).collect();
        assert_eq!(xs, [5, 6, 7, 6, 5, 6]);
        let back = Track { phase: 2, ..t };
        assert_eq!((0..3).map(|n| back.at(n).x).collect::<Vec<_>>(), [7, 6, 5]);
    }
}

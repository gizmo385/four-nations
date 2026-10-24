//! `FN_CAST_DEMO=T`: a stage for shots of the bend animations (#166). A meadow is levelled beside
//! the first settler (with a strip of river for the abilities that need water), the nation's
//! ability is designated on it, and the game runs until `T` seconds from the moment the bend
//! completes (negative: still channelling; positive: it has landed), then pauses, so a headless
//! shot catches that frame of the animation. Take several with different `T` for a strip.
//! Wind Carry first has a settler harvest something near a new stockpile, so there is a stack
//! on the ground to blow.
//!
//! Everything else is the game as played: the sim runs to that moment through its own commands,
//! and `cast` draws it from the world's state.

use bevy::prelude::*;
use fn_sim::world::{Command, TICKS_PER_SECOND};
use fn_sim::{Pos, Terrain};
use fn_ui::Session;

use super::TILE;

/// The camera's scale: close enough to see the pixels.
const ZOOM: f32 = 0.28;
/// Give up waiting for the bend after this many in-game seconds.
const GIVE_UP_SECS: u32 = 400;

#[derive(Resource)]
pub struct CastDemo {
    /// Where the camera looks.
    centre: Vec2,
}

/// Sets the stage and runs the game to `t` seconds from the bend's completion. Returns `None` if
/// the nation has no ability or the bend never happened.
pub fn prepare(session: &mut Session, t: f32) -> Option<CastDemo> {
    let ability = fn_ui::tools::ability_for_nation(&session.defs, session.world.nation)?;
    let origin = session.world.units.first()?.pos;
    super::level_meadow(session, origin);
    let at = |dx: i32, dy: i32| Pos::new(origin.x + dx, origin.y + dy);
    // A strip of river, for Raise Earth (a river tile with dry ground beside it) and Water the
    // Land (fresh water in reach).
    for dy in -4..=4 {
        if let Some(tile) = session.world.map.get_mut(at(9, dy)) {
            tile.terrain = Terrain::River;
        }
    }
    for (i, unit) in session.world.units.iter_mut().enumerate() {
        unit.pos = at(3 + (i as i32 % 3), -1 + (i as i32 / 3) * 2);
    }
    let target = match session.defs.ability(ability).id.as_str() {
        "raise_earth" => at(9, 0),
        "wind_carry" => stage_a_stack(session, origin)?,
        // Water the Land and Hearthflame: a tile by the river, so the patch is on the bank.
        _ => at(7, 0),
    };
    session.apply(Command::Bend { ability, pos: target }).ok()?;
    run_to(session, t)?;
    session.clock.paused = true;
    let middle = Vec2::new((origin.x as f32 + 7.0) * TILE, -(origin.y as f32) * TILE);
    // The side panel covers the right edge of the view: nudge the middle right of centre.
    Some(CastDemo { centre: middle + Vec2::X * ZOOM * 128.0 })
}

/// Wind Carry needs a stack on the ground and a stockpile in reach: a stockpile beside the river's
/// far bank, and the first thing a settler harvests on the near side. Returns the stack's tile.
fn stage_a_stack(session: &mut Session, origin: Pos) -> Option<Pos> {
    let at = |dx: i32, dy: i32| Pos::new(origin.x + dx, origin.y + dy);
    for dy in 0..2 {
        for dx in 12..14 {
            session.apply(Command::DesignateStockpile { pos: at(dx, dy) }).ok()?;
        }
    }
    let spot = at(5, 0);
    let resource = session.defs.resources.iter().enumerate().find_map(|(i, r)| {
        let id = fn_sim::defs::ResourceId(i as u16);
        (!r.id.ends_with("_tree")
            && session.defs.harvest(id).is_some_and(|h| h.unlocked_by.is_none()))
        .then_some(id)
    })?;
    session.world.map.get_mut(spot)?.resource = Some(resource);
    session.apply(Command::DesignateHarvest { pos: spot }).ok()?;
    for _ in 0..GIVE_UP_SECS * TICKS_PER_SECOND {
        session.tick();
        if let Some((pos, _)) = session.world.ground_items().next() {
            return Some(pos);
        }
    }
    None
}

/// Runs the game to `t` seconds from the completion of the first bend.
fn run_to(session: &mut Session, t: f32) -> Option<()> {
    let limit = GIVE_UP_SECS * TICKS_PER_SECOND;
    if t < 0.0 {
        for _ in 0..limit {
            let channels = session.world.bend_channels(&session.defs);
            if channels.first().is_some_and(|c| c.remaining_secs <= -t) {
                return Some(());
            }
            session.tick();
        }
        return None;
    }
    for _ in 0..limit {
        if !session.world.bend_events().is_empty() {
            session.tick_n((t * TICKS_PER_SECOND as f32).round() as u32);
            return Some(());
        }
        session.tick();
    }
    None
}

/// Looks at the stage, close up.
pub fn frame_camera(
    demo: Res<CastDemo>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    for (mut tf, mut projection) in &mut camera {
        tf.translation = demo.centre.extend(tf.translation.z);
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = ZOOM;
        }
    }
}

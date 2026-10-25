//! `FN_TUNNEL_DEMO=1`: a stage for shots of Earth's tunnels (#132). A meadow is levelled beside
//! the first settler with a block of mountain on its east half, `tunneling` is researched (the
//! slow way, settlers kept fed), and the settlers bore a tunnel into the rock a tile at a time and
//! mine two gold veins: one beside the tunnel (it yields `tunnel_count`) and one on the mountain's
//! outer edge (the plain `count`), so both stacks lie on the ground in the shot. The game is paused
//! there. Everything runs through the sim's own commands.

use bevy::math::Vec2;
use fn_sim::needs::Needs;
use fn_sim::world::{Command, TICKS_PER_SECOND};
use fn_sim::{Pos, Terrain};
use fn_ui::Session;

use super::cast_demo::{CastDemo, ZOOM};
use super::{TILE, level_meadow};

/// Give up on one step (a bore, a harvest) after this many in-game seconds.
const GIVE_UP_SECS: u32 = 400;

/// Sets the stage and runs the game until the tunnel is bored and both veins are mined. `None`
/// if the nation can't bore tunnels or a step never finished.
pub fn prepare(session: &mut Session) -> Option<CastDemo> {
    let ability = session.defs.ability_id("bore_tunnel")?;
    let tech = session.defs.ability(ability).unlocked_by?;
    session.apply(Command::SetResearch { tech }).ok()?;
    run_until(session, 4 * 3600, |s| s.world.research().is_known(tech))?;

    let origin = session.world.units.first()?.pos;
    level_meadow(session, origin);
    let at = |dx: i32, dy: i32| Pos::new(origin.x + dx, origin.y + dy);
    let (gold, stone) =
        (session.defs.resource_id("gold_vein")?, session.defs.resource_id("stone")?);
    for dy in -6..=6 {
        for dx in 8..=20 {
            if let Some(tile) = session.world.map.get_mut(at(dx, dy)) {
                tile.terrain = Terrain::Mountain;
            }
        }
    }
    for (dx, dy) in [(9, -3), (10, -3), (10, -4), (13, -2), (9, 3), (15, 3)] {
        session.world.map.get_mut(at(dx, dy))?.resource = Some(stone);
    }
    // One vein the tunnel will run past, and one on the edge, mined from outside.
    let (inside, outside) = (at(12, 1), at(8, 4));
    session.world.map.get_mut(inside)?.resource = Some(gold);
    session.world.map.get_mut(outside)?.resource = Some(gold);
    session.world.map_changed();
    for (i, unit) in session.world.units.iter_mut().enumerate() {
        unit.pos = at(3 + (i as i32 % 3), -1 + (i as i32 / 3) * 2);
    }

    for dx in 8..=16 {
        let pos = at(dx, 0);
        let events = session.world.bend_events().len();
        session.apply(Command::Bend { ability, pos }).ok()?;
        run_until(session, GIVE_UP_SECS, |s| s.world.bend_events().len() > events)?;
    }
    for pos in [inside, outside] {
        session.apply(Command::DesignateHarvest { pos }).ok()?;
    }
    run_until(session, GIVE_UP_SECS, |s| {
        [inside, outside].iter().all(|&p| s.world.map.get(p).is_some_and(|t| t.resource.is_none()))
    })?;
    session.clock.paused = true;
    let middle = Vec2::new((origin.x as f32 + 11.0) * TILE, -(origin.y as f32) * TILE);
    // The side panel covers the right edge of the view: nudge the middle right of centre.
    Some(CastDemo::looking_at(middle + Vec2::X * ZOOM * 128.0))
}

/// Ticks a second at a time, with the settlers kept fed and their chi full (the demo is about
/// the tunnel, not the meters), until `done` or `secs` in-game seconds have passed.
fn run_until(session: &mut Session, secs: u32, done: impl Fn(&Session) -> bool) -> Option<()> {
    for _ in 0..secs {
        if done(session) {
            return Some(());
        }
        for unit in &mut session.world.units {
            unit.needs = Needs::full(&session.defs.needs);
            unit.chi = session.defs.chi.max;
        }
        session.tick_n(TICKS_PER_SECOND);
    }
    done(session).then_some(())
}

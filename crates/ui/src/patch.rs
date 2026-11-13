//! The patches the map tints for the fusions (docs/design/horizons-phase-6.md, "Mockups", #763):
//! where an Aqueduct irrigates while the cursor is on one, and where Call Rain falls or would
//! fall. A frontend asks for the tiles and colours them; which plots a patch really waters is
//! the sim's (`World::irrigation_rate`).

use std::collections::BTreeSet;

use fn_sim::Pos;
use fn_sim::bending::Effect;

use crate::session::Session;
use crate::tools::{Tool, bend_ability};

/// The tiles an aqueduct run irrigates: every tile within `irrigation_radius` (Chebyshev) of a
/// tile of the run the cursor is on. Empty off an aqueduct, and for a dry run, which waters
/// nothing.
pub fn aqueduct_reach(session: &Session, cursor: Pos) -> BTreeSet<Pos> {
    let (world, defs) = (&session.world, &session.defs);
    let aqueduct = |p: Pos| world.building_at(p).and_then(|b| defs.building(b).aqueduct.as_ref());
    let Some(def) = aqueduct(cursor) else { return BTreeSet::new() };
    if !world.is_flowing(cursor, defs) {
        return BTreeSet::new();
    }
    // The run: the tiles 4-connected to the cursor's, as `World::aqueduct_flow` counts them.
    let mut run = BTreeSet::from([cursor]);
    let mut stack = vec![cursor];
    while let Some(p) = stack.pop() {
        for n in world.map.neighbors(p, false) {
            if aqueduct(n).is_some() && run.insert(n) {
                stack.push(n);
            }
        }
    }
    let r = def.irrigation_radius as i32;
    let mut tiles = BTreeSet::new();
    for p in run {
        for dy in -r..=r {
            for dx in -r..=r {
                let q = Pos::new(p.x + dx, p.y + dy);
                if world.map.get(q).is_some() {
                    tiles.insert(q);
                }
            }
        }
    }
    tiles
}

/// The tiles Call Rain tints: every rain still falling, while the Bend tool is up or the cursor
/// is inside one, and, with the Bend tool on a rain bend, the patch it would cover around the
/// cursor.
pub fn rain_tiles(session: &Session, tool: Tool, cursor: Pos) -> BTreeSet<Pos> {
    let (world, defs) = (&session.world, &session.defs);
    let square = |center: Pos, radius: u32| {
        let r = radius as i32;
        (-r..=r)
            .flat_map(move |dy| (-r..=r).map(move |dx| Pos::new(center.x + dx, center.y + dy)))
            .filter(|&p| world.map.get(p).is_some())
            .collect::<Vec<Pos>>()
    };
    let mut tiles = BTreeSet::new();
    let inside = world.rains().any(|r| r.center.chebyshev(cursor) <= r.radius as i32);
    if tool == Tool::Bend || inside {
        for rain in world.rains() {
            tiles.extend(square(rain.center, rain.radius));
        }
    }
    if tool == Tool::Bend
        && let Some(id) = bend_ability(session)
        && let Effect::Rain { radius, .. } = defs.ability(id).effect
    {
        tiles.extend(square(cursor, radius));
    }
    tiles
}

//! Taking finished buildings down (#342).

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::{Tile, TileMap};

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn field() -> TileMap {
    Grid::from_fn(24, 16, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

/// A world on open land with units on `spawns` and a finished `name` (raised, so its seats and
/// craft jobs are open) anchored at `anchor`.
fn with(name: &str, anchor: Pos, spawns: &[Pos], defs: &Defs) -> World {
    let mut w = World::with_map(5, field(), Nation::Earth, spawns, defs);
    let id = defs.building_id(name).unwrap();
    let (fw, fh) = defs.building(id).footprint;
    w.raise(Footprint::new(anchor, fw, fh), id, defs);
    w
}

/// Ticks until the building anchored at `anchor` is gone, or panics after `limit` ticks.
fn tick_until_gone(w: &mut World, anchor: Pos, limit: u32, defs: &Defs) {
    for _ in 0..limit {
        if w.anchored(anchor).is_none() {
            return;
        }
        w.tick(defs);
    }
    panic!("still standing after {limit} ticks: {:?}", w.jobs());
}

/// Every ground stack of `item`, summed, within `reach` tiles (Chebyshev) of `pos`.
fn dropped_near(w: &World, pos: Pos, item: crate::jobs::ItemId, reach: i32) -> u32 {
    w.ground_items()
        .filter(|(p, s)| {
            s.item == item && (p.x - pos.x).abs() <= reach && (p.y - pos.y).abs() <= reach
        })
        .map(|(_, s)| s.count)
        .sum()
}

#[test]
fn the_bundled_refund_is_half_rounded_down() {
    let defs = Defs::bundled();
    assert_eq!(defs.deconstruct.refund_of(5), 2);
    assert_eq!(defs.deconstruct.refund_of(10), 5);
    assert_eq!(defs.deconstruct.refund_of(1), 0);
}

#[test]
fn any_tile_marks_the_whole_building_once() {
    let defs = Defs::bundled();
    let anchor = at(10, 6);
    let mut w = with("study_hall", anchor, &[at(1, 1)], &defs);
    // The bottom-right tile names the hall; the mark lands on the anchor.
    w.apply(Command::Deconstruct { pos: at(11, 7) }, &defs).unwrap();
    assert_eq!(w.deconstruct_designations().collect::<Vec<_>>(), [anchor]);
    for p in Footprint::new(anchor, 2, 2).tiles() {
        assert!(w.is_marked_for_deconstruct(p), "{p:?}");
        assert_eq!(
            w.apply(Command::Deconstruct { pos: p }, &defs),
            Err(CommandError::Designate(DesignateError::AlreadyDesignated))
        );
    }
    assert!(!w.is_marked_for_deconstruct(at(12, 6)));
    assert_eq!(
        w.apply(Command::Deconstruct { pos: at(3, 3) }, &defs),
        Err(CommandError::Designate(DesignateError::NoBuilding))
    );
    assert_eq!(
        w.apply(Command::Deconstruct { pos: at(-1, 3) }, &defs),
        Err(CommandError::Designate(DesignateError::OutOfBounds))
    );
}

#[test]
fn the_endeavor_gate_cannot_be_taken_down() {
    let defs = Defs::bundled();
    let mut w = World::with_map(5, field(), Nation::Earth, &[at(1, 1)], &defs);
    let gate = defs.buildings.iter().position(|b| b.endeavor).unwrap();
    let gate = crate::building::BuildingId(gate as u16);
    w.put_building(Footprint::new(at(10, 6), 4, 4), gate, &defs);
    assert_eq!(
        w.apply(Command::Deconstruct { pos: at(12, 8) }, &defs),
        Err(CommandError::Designate(DesignateError::CannotDeconstruct))
    );
}

#[test]
fn taking_down_a_2x2_hall_clears_the_footprint_and_its_seats_and_refunds_half_by_the_anchor() {
    let defs = Defs::bundled();
    let anchor = at(10, 6);
    let mut w = with("study_hall", anchor, &[at(4, 6), at(4, 8)], &defs);
    let hall = defs.building(defs.building_id("study_hall").unwrap());
    let seats =
        |w: &World| w.jobs().iter().filter(|j| j.kind == JobKind::Study { pos: anchor }).count();
    assert_eq!(seats(&w), hall.seats as usize);
    w.apply(Command::Deconstruct { pos: at(10, 7) }, &defs).unwrap();
    tick_until_gone(&mut w, anchor, 20_000, &defs);

    assert!(Footprint::new(anchor, 2, 2).tiles().all(|p| w.map[p].building.is_none()));
    assert_eq!(w.buildings().count(), 0);
    assert_eq!(seats(&w), 0, "the hall's seats go with it");
    assert_eq!(w.deconstruct_designations().count(), 0);
    for u in &w.units {
        assert!(
            !matches!(u.job_task, Some(Task::Study(_) | Task::Deconstruct(_))),
            "{:?}",
            u.job_task
        );
    }
    for cost in &hall.cost {
        // The whole refund is still on the ground right after, before anyone hauls it.
        assert_eq!(
            dropped_near(&w, anchor, cost.item, 3),
            defs.deconstruct.refund_of(cost.count),
            "{cost:?}"
        );
    }
    w.validate(&defs).unwrap();
    w.check_invariants(&defs).unwrap();
}

#[test]
fn a_production_building_gives_back_its_batch_inputs_in_full() {
    let defs = Defs::bundled();
    let anchor = at(10, 6);
    let mut w = with("forge", anchor, &[at(4, 6)], &defs);
    let forge = defs.building(defs.building_id("forge").unwrap());
    let recipe = forge.recipe.as_ref().unwrap();
    // The forge's crafter isn't hired: only the mark is open, so the batch stays as set.
    w.jobs.retain(|j| j.kind != JobKind::Craft { pos: anchor });
    w.anchored_mut(anchor).unwrap().craft =
        Some(CraftState { delivered: vec![2; recipe.inputs.len()], progress: 0.0 });
    w.apply(Command::Deconstruct { pos: anchor }, &defs).unwrap();
    tick_until_gone(&mut w, anchor, 40_000, &defs);
    for input in &recipe.inputs {
        let refund = forge
            .cost
            .iter()
            .find(|c| c.item == input.item)
            .map_or(0, |c| defs.deconstruct.refund_of(c.count));
        assert_eq!(dropped_near(&w, anchor, input.item, 3), refund + 2, "{input:?}");
    }
}

#[test]
fn taking_down_a_bed_someone_sleeps_in_does_not_strand_them() {
    let defs = Defs::bundled();
    let bed = at(10, 6);
    let mut w = with("bed", bed, &[at(10, 6), at(4, 6)], &defs);
    // Unit 0 is asleep in the bed; unit 1 is free to do the work.
    w.units[0].bed = Some(bed);
    w.units[0].need_task = Some(Task::Sleep);
    w.units[0].needs.energy = 1.0;
    w.apply(Command::Deconstruct { pos: bed }, &defs).unwrap();
    tick_until_gone(&mut w, bed, 20_000, &defs);

    let sleeper = &w.units[0];
    assert_ne!(sleeper.bed, Some(bed), "still bound to a bed that's gone");
    assert!(sleeper.inside.is_none());
    assert!(w.map[sleeper.pos].building.is_none_or(|b| b.passable()));
    w.check_invariants(&defs).unwrap();
    // And it carries on living: a few more ticks find it somewhere to sleep or something to do.
    for _ in 0..200 {
        w.tick(&defs);
    }
    w.check_invariants(&defs).unwrap();
}

#[test]
fn cancelling_the_mark_leaves_the_building_untouched() {
    let defs = Defs::bundled();
    let anchor = at(10, 6);
    let mut w = with("study_hall", anchor, &[at(8, 6)], &defs);
    let before = w.anchored(anchor).unwrap().clone();
    w.apply(Command::Deconstruct { pos: anchor }, &defs).unwrap();
    // Work starts on it, then the mark is taken back from another tile.
    let mut worked = false;
    for _ in 0..2_000 {
        w.tick(&defs);
        if w.jobs()
            .iter()
            .any(|j| j.kind == JobKind::Deconstruct { pos: anchor } && j.progress > 0.0)
        {
            worked = true;
            break;
        }
    }
    assert!(worked, "nobody started on it");
    w.apply(Command::CancelDeconstruct { pos: at(11, 7) }, &defs).unwrap();
    assert_eq!(w.deconstruct_designations().count(), 0);
    assert_eq!(
        w.apply(Command::CancelDeconstruct { pos: anchor }, &defs),
        Err(CommandError::Designate(DesignateError::NotDesignated))
    );
    for _ in 0..50 {
        w.tick(&defs);
    }
    assert_eq!(w.anchored(anchor), Some(&before));
    assert!(w.units.iter().all(|u| !matches!(u.job_task, Some(Task::Deconstruct(_)))));
    w.check_invariants(&defs).unwrap();
}

#[test]
fn a_pending_mark_survives_save_and_load() {
    let defs = Defs::bundled();
    let anchor = at(10, 6);
    let mut w = with("study_hall", anchor, &[at(1, 1)], &defs);
    w.apply(Command::Deconstruct { pos: at(11, 6) }, &defs).unwrap();
    let loaded = World::load(&w.save(&defs).unwrap(), &defs).unwrap();
    assert_eq!(loaded.deconstruct_designations().collect::<Vec<_>>(), [anchor]);
    assert!(loaded.is_marked_for_deconstruct(at(11, 7)));
    assert_eq!(loaded.command_log(), w.command_log());
}

#[test]
fn removing_a_building_some_other_way_drops_its_mark() {
    let defs = Defs::bundled();
    let anchor = at(10, 6);
    let mut w = with("campfire", anchor, &[at(1, 1)], &defs);
    w.apply(Command::Deconstruct { pos: anchor }, &defs).unwrap();
    let key = w.map[anchor].building.unwrap().key();
    w.remove_building(key, &defs).unwrap();
    assert_eq!(w.deconstruct_designations().count(), 0);
}

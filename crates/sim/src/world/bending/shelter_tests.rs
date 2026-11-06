//! Earth Tent and Air Shield (#362): the same `heat_source` effect as Hearthflame, but with
//! `cooks = false`. A shelter keeps the cold away and gives its own `sleep_bonus`, exactly like a
//! flame, but cooks nothing, and (being a bend effect on a tile, not a building) is never a bed.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::jobs::{ItemId, Stack};
use crate::mapgen::Tile;
use crate::needs::Needs;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A flat field of land, `w` wide, every tile at `temperature`.
fn field(w: i32, temperature: u8) -> TileMap {
    Grid::from_fn(w, 1, |_| Tile {
        elevation: 100,
        temperature,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn season_ticks(defs: &Defs) -> u64 {
    (defs.seasons.calendar.season_minutes as f64 * 60.0 * TICKS_PER_SECOND as f64) as u64
}

/// A world of `nation`, on `normal` difficulty, at winter's first tick: cold enough (a polar
/// tile) that a unit standing on `pos` is cold unless something warms it.
fn cold_world(defs: &Defs, nation: Nation, pos: Pos) -> World {
    let mut w = World::with_map(1, field(5, 0), nation, &[pos], defs);
    w.difficulty = Some("normal".into());
    w.tick = 3 * season_ticks(defs); // winter's first tick (spring, summer, autumn, then winter)
    w.units[0].needs = Needs::full(&defs.needs);
    w.units[0].need_task = None;
    w.units[0].job_task = None;
    w
}

/// `nation`'s own unlock tech for `ability`, so it can be cast at all.
fn unlock(ability: &str) -> &'static str {
    match ability {
        "earth_tent" => "earth_shaping",
        "air_shield" => "meditation",
        other => panic!("no unlock tech recorded for {other}"),
    }
}

/// `nation` and its own shelter ability, in the order the AC lists them.
const SHELTERS: [(Nation, &str); 2] = [(Nation::Earth, "earth_tent"), (Nation::Air, "air_shield")];

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for n in 0..limit {
        if done(w) {
            return n;
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

/// Grants the tech `ability` needs, then casts it on `pos` with unit 0 and runs until it lands,
/// which it should.
fn raise_shelter(w: &mut World, defs: &Defs, ability: &str, pos: Pos) {
    w.research.grant(defs.tech_id(unlock(ability)).unwrap());
    w.refresh_modifiers(defs);
    w.units[0].chi = defs.chi.max;
    let id = defs.ability_id(ability).unwrap();
    w.apply(Command::Bend { ability: id, pos }, defs).unwrap();
    let events = w.bend_events().len();
    run_until(w, defs, 2000, |w| w.bend_events().len() > events);
    assert_eq!(w.bend_events().last().unwrap().outcome, BendOutcome::Done, "{ability}");
}

fn cabbage_item(defs: &Defs) -> ItemId {
    defs.items.iter().position(|i| i.id == "cabbage").map(|i| ItemId(i as u16)).unwrap()
}

/// A cold unit inside either shelter isn't cold (AC): the shelter keeps the cold off exactly
/// like a flame would, through `is_heated`.
#[test]
fn a_cold_unit_inside_either_shelter_is_not_cold() {
    let defs = Defs::bundled();
    for (nation, ability) in SHELTERS {
        let pos = at(2, 0);
        let mut w = cold_world(&defs, nation, pos);
        assert!(w.is_cold(pos, &defs), "{ability}: should start cold");
        raise_shelter(&mut w, &defs, ability, pos);
        assert!(w.is_heated(pos), "{ability}: should warm its own tile");
        assert!(!w.is_cold(pos, &defs), "{ability}: a shelter should keep the cold off");
    }
}

/// A meal eaten inside either shelter isn't cooked (AC): `cooks = false` splits cooking from
/// warmth (`is_heated` stays true, `is_cooking` does not), so the meal's `FoodDef::hunger`
/// applies, not `cooked_hunger`.
#[test]
fn a_meal_eaten_inside_either_shelter_is_not_cooked() {
    let defs = Defs::bundled();
    let food = defs.foods.iter().find(|f| f.id == "cabbage").unwrap();
    let (raw, cooked) = (food.hunger, food.cooked_hunger.unwrap());
    assert!(cooked > raw, "cooked must be worth more, or this test can't tell them apart");
    let cabbage = cabbage_item(&defs);
    for (nation, ability) in SHELTERS {
        let pos = at(2, 0);
        let mut w = World::with_map(1, field(5, 128), nation, &[pos], &defs);
        raise_shelter(&mut w, &defs, ability, pos);
        assert!(w.is_heated(pos), "{ability}: should still warm the tile");
        assert!(!w.is_cooking(pos), "{ability}: a shelter cooks nothing");

        let before = defs.needs.hunger.low - 5.0;
        w.units[0].needs.hunger = before;
        w.units[0].carrying = Some(Stack { item: cabbage, count: 1 });
        run_until(&mut w, &defs, 200, |w| w.units[0].carrying.is_none());
        let gained = w.units[0].needs.hunger - before;
        assert!((gained - raw).abs() < 0.5, "{ability}: ate for {gained}, raw is {raw}");
    }
}

/// A sleeper in Earth Tent's reach sleeps faster, by its own `sleep_bonus` (1.4), same as any
/// other heat source (AC: "a sleeper in the tent sleeps faster").
#[test]
fn a_sleeper_in_earth_tents_reach_sleeps_faster() {
    let defs = Defs::bundled();
    let base = defs.needs.general.sleep_energy_per_minute;
    let pos = at(2, 0);
    let mut w = World::with_map(1, field(5, 128), Nation::Earth, &[pos], &defs);
    raise_shelter(&mut w, &defs, "earth_tent", pos);

    w.units[0].need_task = Some(Task::Sleep);
    w.units[0].route = None;
    w.units[0].needs = Needs { energy: 10.0, ..Needs::full(&defs.needs) };
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert!(w.units[0].is_sleeping());
    let gained = w.units[0].needs.energy - 10.0;
    assert!((gained - base * 1.4).abs() < 0.01, "{gained} vs {}", base * 1.4);
}

#[cfg(test)]
mod bed_tests {
    use super::*;

    /// Earth Tent isn't a bed (AC): it's a bend effect on a tile, not a building, so it has no
    /// `BuildingDef::bed` for growth's free-bed check or the bed sign to find. Mirrors
    /// `population::tests::no_newcomer_without_a_free_bed`, with a lit tent standing in for
    /// nothing at all: the outcome is the same.
    #[test]
    fn earth_tent_is_not_a_bed_for_growth() {
        let defs = Defs::bundled();
        let pos = at(2, 0);
        let mut w = World::with_map(1, field(5, 128), Nation::Earth, &[pos], &defs);
        raise_shelter(&mut w, &defs, "earth_tent", pos);
        assert!(defs.building(defs.building_id("bed").unwrap()).bed.is_some());

        let fish = defs.item_id("fish").unwrap();
        w.stored.insert(at(4, 0), Stack { item: fish, count: defs.needs.growth.food_per_newcomer });
        let before = w.units.len();
        let interval =
            (defs.needs.growth.arrival_minutes * 60.0 * TICKS_PER_SECOND as f32).round() as u64;
        for _ in 0..interval {
            w.tick(&defs);
        }
        assert_eq!(w.units.len(), before, "a lit tent is not a bed, so still no newcomer");
        assert!(w.arrivals().is_empty());
    }
}

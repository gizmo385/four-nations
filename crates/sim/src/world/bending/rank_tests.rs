//! The mastery rank (#360): forms, `World::rank_of`'s thresholds, a rank-gated ability, and a
//! master's chi discount. Real abilities' own target rules aren't the point here, so these use
//! the test-only `probe` effect (`bending::testing`): walkable ground, no tech, nothing but the
//! chi.

use super::*;
use crate::bending::testing::load;
use crate::bending::{AbilityId, Rank};
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

fn arena(width: i32, height: i32) -> TileMap {
    Grid::from_fn(width, height, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for n in 0..limit {
        if done(w) {
            return n;
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

/// Three ordinary Earth probes (`probe_a`, `probe_b`, `probe_c`: no rank, no tech), and two more
/// gated by rank (`adept_probe`, `master_probe`), for testing forms and rank gates without a real
/// effect's own target rules.
const PROBES: &str = "
[[ability]]
id = 'probe_a'
nation = 'earth'
name = 'Probe A'
verb = 'Probe'
chi = 10.0
work = 0.0
[ability.effect]
kind = 'probe'
[[ability]]
id = 'probe_b'
nation = 'earth'
name = 'Probe B'
verb = 'Probe'
chi = 10.0
work = 0.0
[ability.effect]
kind = 'probe'
[[ability]]
id = 'probe_c'
nation = 'earth'
name = 'Probe C'
verb = 'Probe'
chi = 10.0
work = 0.0
[ability.effect]
kind = 'probe'
[[ability]]
id = 'adept_probe'
nation = 'earth'
name = 'Adept probe'
verb = 'Probe'
chi = 10.0
work = 0.0
rank = 'adept'
[ability.effect]
kind = 'probe'
[[ability]]
id = 'master_probe'
nation = 'earth'
name = 'Master probe'
verb = 'Probe'
chi = 20.0
work = 0.0
rank = 'master'
[ability.effect]
kind = 'probe'
";

fn probe_rank_defs() -> Defs {
    load(PROBES, "").expect("test probes are valid")
}

fn id(defs: &Defs, ability: &str) -> AbilityId {
    defs.ability_id(ability).unwrap()
}

/// Gives unit `i` `bends` finished bends, spread over the first `forms` of its own nation's
/// abilities (`World::nation_of`), without going through a real bend.
fn give(w: &mut World, defs: &Defs, i: usize, bends: u32, forms: u32) {
    w.units[i].bends = bends;
    w.units[i].bends_by.clear();
    if forms == 0 {
        return;
    }
    let nation = w.nation_of(&w.units[i]);
    let ids: Vec<AbilityId> = defs
        .abilities
        .iter()
        .enumerate()
        .filter(|(_, a)| a.nation == nation)
        .take(forms as usize)
        .map(|(idx, _)| AbilityId(idx as u16))
        .collect();
    assert_eq!(ids.len(), forms as usize, "{nation:?} needs at least {forms} abilities");
    let per = (bends / forms).max(1);
    for (n, &aid) in ids.iter().enumerate() {
        let share = if n + 1 == ids.len() { bends - per * (forms - 1) } else { per };
        w.units[i].bends_by.insert(aid, share);
    }
}

/// Every nation's own adept and master thresholds, bundled (#344's numbers, now
/// `data/bending.toml`'s `[mastery]`): Fire 10, Earth 8, Water 8, Air 16 master bends, half that
/// (rounded up) for adept, 2 forms for adept and 3 for master.
#[test]
fn rank_of_reads_each_nations_own_thresholds() {
    let defs = Defs::bundled();
    for (nation, master_bends, adept_bends) in
        [(Nation::Fire, 10, 5), (Nation::Earth, 8, 4), (Nation::Water, 8, 4), (Nation::Air, 16, 8)]
    {
        let mut w = World::with_map(1, arena(5, 5), nation, &[at(0, 0)], &defs);
        assert_eq!(w.rank_of(&w.units[0], &defs).rank, Rank::Novice, "{nation:?}: fresh unit");

        give(&mut w, &defs, 0, adept_bends - 1, 2);
        assert_eq!(w.rank_of(&w.units[0], &defs).rank, Rank::Novice, "{nation:?}: 1 short");
        give(&mut w, &defs, 0, adept_bends, 2);
        assert_eq!(w.rank_of(&w.units[0], &defs).rank, Rank::Adept, "{nation:?}");

        give(&mut w, &defs, 0, master_bends - 1, 3);
        assert_eq!(w.rank_of(&w.units[0], &defs).rank, Rank::Adept, "{nation:?}: 1 short");
        give(&mut w, &defs, 0, master_bends, 3);
        assert_eq!(w.rank_of(&w.units[0], &defs).rank, Rank::Master, "{nation:?}");
    }
}

/// Enough bends in too few forms is never adept or master, whatever the count (one ability
/// spammed doesn't count).
#[test]
fn forms_not_just_bends_gate_every_rank_above_novice() {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, arena(5, 5), Nation::Fire, &[at(0, 0)], &defs);
    give(&mut w, &defs, 0, 1000, 1);
    assert_eq!(w.rank_of(&w.units[0], &defs).rank, Rank::Novice);
    give(&mut w, &defs, 0, 1000, 2);
    assert_eq!(w.rank_of(&w.units[0], &defs).rank, Rank::Adept, "2 forms is enough for adept");
}

/// `Unit::forms` counts distinct abilities finished at least one bend of: two bends of the same
/// ability is one form, and a bend of a different ability adds a second.
#[test]
fn forms_count_distinct_abilities_not_total_bends() {
    let defs = probe_rank_defs();
    let mut w = World::with_map(1, arena(9, 5), Nation::Earth, &[at(1, 1)], &defs);
    let a = id(&defs, "probe_a");
    let b = id(&defs, "probe_b");

    w.apply(Command::Bend { ability: a, pos: at(3, 1) }, &defs).unwrap();
    run_until(&mut w, &defs, 200, |w| w.bend_events().len() == 1);
    assert_eq!((w.units[0].bends(), w.units[0].forms()), (1, 1));

    w.apply(Command::Bend { ability: a, pos: at(4, 1) }, &defs).unwrap();
    run_until(&mut w, &defs, 200, |w| w.bend_events().len() == 2);
    assert_eq!((w.units[0].bends(), w.units[0].forms()), (2, 1), "same ability again: no new form");

    w.apply(Command::Bend { ability: b, pos: at(5, 1) }, &defs).unwrap();
    run_until(&mut w, &defs, 200, |w| w.bend_events().len() == 3);
    assert_eq!((w.units[0].bends(), w.units[0].forms()), (3, 2), "a different ability: a new form");
    assert_eq!(w.units[0].bends_of(a), 2);
    assert_eq!(w.units[0].bends_of(b), 1);
}

/// A rank-gated ability is refused for a novice, allowed for an adept or a master of its own
/// nation, checked next to `unlocked_by` (#360).
#[test]
fn a_rank_gated_ability_is_refused_below_its_rank_and_allowed_at_or_above_it() {
    let defs = probe_rank_defs();
    let mut w = World::with_map(1, arena(5, 5), Nation::Earth, &[at(0, 0)], &defs);
    let adept_probe = id(&defs, "adept_probe");
    let master_probe = id(&defs, "master_probe");

    assert_eq!(w.rank_of(&w.units[0], &defs).rank, Rank::Novice);
    assert!(!w.can_cast(&w.units[0], adept_probe, &defs), "a novice can't cast an adept ability");
    assert!(!w.can_cast(&w.units[0], master_probe, &defs));

    give(&mut w, &defs, 0, 4, 2);
    assert_eq!(w.rank_of(&w.units[0], &defs).rank, Rank::Adept);
    assert!(w.can_cast(&w.units[0], adept_probe, &defs), "an adept can cast its own ability");
    assert!(!w.can_cast(&w.units[0], master_probe, &defs), "but not a master's");

    give(&mut w, &defs, 0, 8, 3);
    assert_eq!(w.rank_of(&w.units[0], &defs).rank, Rank::Master);
    assert!(
        w.can_cast(&w.units[0], adept_probe, &defs),
        "a master can still cast an adept ability"
    );
    assert!(w.can_cast(&w.units[0], master_probe, &defs), "and its own");
}

/// A master pays `master_chi` of an ability's own chi, checked when claiming and spent on
/// completion, not the full cost (#360).
#[test]
fn a_master_pays_the_discounted_chi() {
    let defs = probe_rank_defs();
    let mut w = World::with_map(1, arena(9, 5), Nation::Earth, &[at(1, 1)], &defs);
    give(&mut w, &defs, 0, 8, 3);
    assert_eq!(w.rank_of(&w.units[0], &defs).rank, Rank::Master);

    let probe_a = id(&defs, "probe_a");
    let spawn = w.units[0].pos;
    w.units[0].chi = 8.0; // Below probe_a's full 10, but above its master's 80%.
    w.apply(Command::Bend { ability: probe_a, pos: spawn }, &defs).unwrap();
    run_until(&mut w, &defs, 200, |w| !w.bend_events().is_empty());
    assert_eq!(w.bend_events()[0].outcome, BendOutcome::Done);
    assert!(w.units[0].chi < 0.1, "{}", w.units[0].chi);
}

/// A save from before `bends_by` existed loads with none, and `rank_credited` defaults to
/// Novice: `Unit::forms` is 0 even for a unit whose `bends` alone would already be a master.
#[test]
fn old_saves_without_bends_by_load_with_none() {
    let defs = Defs::bundled();
    let w = World::with_map(1, arena(5, 5), Nation::Fire, &[at(0, 0)], &defs);
    let mut unit = w.units[0].clone();
    unit.bends = 1000;
    let text = ron::to_string(&unit).unwrap();
    assert!(text.contains("bends_by:{}") && text.contains("rank_credited:novice"), "{text}");
    let old = text.replace("bends_by:{},", "").replace("rank_credited:novice,", "");
    let loaded: Unit = ron::from_str(&old).expect("a save missing both fields still loads");
    assert_eq!(loaded.bends(), 1000);
    assert_eq!(loaded.forms(), 0);
}

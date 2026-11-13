use super::*;
use crate::beast::BeastId;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;
use crate::research::Condition;

fn field(width: i32, height: i32) -> TileMap {
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

/// The bundled data plus `gated`, a shared 25-point tech that needs two Earth residents and a
/// Fire or Earth master.
fn gated_defs() -> Defs {
    let mut files = crate::defs::BUNDLED_FILES;
    let research = format!(
        "{}\n[[tech]]\nid = \"gated\"\nname = \"Gated\"\ndescription = \"\"\ncost = 25\n\
         requires = {{ population = 4, residents = {{ earth = 2 }}, masters = 1, \
         masters_of = [\"earth\", \"fire\"] }}\n",
        files[3]
    );
    files[3] = &research;
    Defs::from_files(files).expect("test data is valid")
}

/// Gives unit `i` Fire's master bends (10) over 3 forms without going through real bends.
fn make_master(w: &mut World, defs: &Defs, i: usize) {
    let nation = w.nation_of(&w.units[i]);
    w.units[i].bends = 10;
    w.units[i].bends_by.clear();
    let ids: Vec<_> = defs
        .abilities
        .iter()
        .enumerate()
        .filter(|(_, a)| a.nation == nation)
        .take(3)
        .map(|(k, _)| crate::bending::AbilityId(k as u16))
        .collect();
    for (n, id) in ids.into_iter().enumerate() {
        w.units[i].bends_by.insert(id, if n == 2 { 4 } else { 3 });
    }
    assert_eq!(w.rank_of(&w.units[i], defs).rank, Rank::Master);
}

/// A Fire world of five settlers: units 0 and 1 are Earth newcomers, unit 4 is a Fire master.
fn setup(defs: &Defs) -> World {
    let spots: Vec<Pos> = (0..5).map(|x| at(x * 2, 4)).collect();
    let mut w = World::with_map(1, field(20, 8), Nation::Fire, &spots, defs);
    w.units[0].nation = Some(Nation::Earth);
    w.units[1].nation = Some(Nation::Earth);
    make_master(&mut w, defs, 4);
    w.refresh_roster(defs);
    w
}

/// The `gated` tech, queued with all its knowledge banked.
fn queue_gated(w: &mut World, defs: &Defs) -> TechId {
    let t = defs.tech_id("gated").unwrap();
    w.apply(Command::SetResearch { tech: t }, defs).unwrap();
    w.research.bank_for_test(25_000);
    t
}

#[test]
fn facts_count_residents_by_their_own_nation_and_leave_out_guests_and_beasts() {
    let defs = gated_defs();
    let mut w = setup(&defs);
    let f = w.facts();
    assert_eq!(f.population, 5);
    assert_eq!(f.residents, [0, 0, 2, 3], "air, water, earth, fire");
    assert_eq!(f.masters, [0, 0, 0, 1]);
    // The Avatar is a guest and a beast is not a person: neither is a resident, however many
    // there are, and a master among them is not a master.
    w.units[2].guest = Some(Guest::Avatar);
    make_master(&mut w, &defs, 2);
    w.units[3].beast = Some(Beast { id: BeastId(0), home: at(0, 0), bored: 0 });
    make_master(&mut w, &defs, 3);
    w.refresh_roster(&defs);
    let f = w.facts();
    assert_eq!(f.population, 3);
    assert_eq!(f.residents, [0, 0, 2, 1]);
    assert_eq!(f.masters, [0, 0, 0, 1]);
    // A master counts under their own nation: unit 0 is Earth's.
    make_master(&mut w, &defs, 0);
    w.refresh_roster(&defs);
    assert_eq!(w.facts().masters, [0, 0, 1, 1]);
}

#[test]
fn the_roster_is_counted_when_a_world_is_made_and_before_each_research_tick() {
    let defs = gated_defs();
    let mut w = setup(&defs);
    w.units[3].nation = Some(Nation::Air);
    assert_eq!(w.facts().residents[Nation::Air.index()], 0, "counted once a tick, not live");
    w.tick(&defs);
    assert_eq!(w.facts().residents[Nation::Air.index()], 1);
    let fresh = World::with_map(1, field(20, 8), Nation::Water, &[at(1, 1), at(3, 3)], &defs);
    assert_eq!((fresh.facts().population, fresh.facts().residents), (2, [0, 2, 0, 0]));
}

#[test]
fn queueing_checks_the_roster_and_names_what_is_missing() {
    let defs = gated_defs();
    let t = defs.tech_id("gated").unwrap();
    let mut w = setup(&defs);
    w.units[1].nation = None;
    let err = w.apply(Command::SetResearch { tech: t }, &defs).unwrap_err();
    let CommandError::Research(ResearchError::NotYet(c)) = err else { panic!("{err:?}") };
    assert_eq!(c, Condition::Residents { nation: Nation::Earth, need: 2, have: 1 });
    assert_eq!(c.to_string(), "2 Earth residents (1 lives here)");
    w.units[1].nation = Some(Nation::Earth);
    w.apply(Command::SetResearch { tech: t }, &defs).unwrap();
}

#[test]
fn a_tech_waits_at_the_front_when_a_condition_falls_and_finishes_when_it_returns() {
    let defs = gated_defs();
    let mut w = setup(&defs);
    let t = queue_gated(&mut w, &defs);
    // An Earth resident leaves: the knowledge is in, the condition isn't.
    w.units[1].nation = Some(Nation::Fire);
    for _ in 0..5 {
        w.tick(&defs);
        assert!(!w.research().is_known(t));
        assert_eq!(w.research().queue(), [t], "waiting at the front, not dropped");
        assert_eq!(w.research().pool_millis(), 25_000, "the banked points are kept");
    }
    let cost = w.knobs(&defs).research_cost;
    assert_eq!(
        w.research().waiting_on(&w.facts(), &defs, cost, w.nation),
        Some(Condition::Residents { nation: Nation::Earth, need: 2, have: 1 })
    );
    assert!(!w.study_wanted(&defs), "seats are not claimed while the tech waits");
    // Someone arrives: it finishes on the very next tick.
    w.units[1].nation = Some(Nation::Earth);
    w.tick(&defs);
    assert!(w.research().is_known(t));
    assert!(w.research().queue().is_empty());
    assert_eq!(w.research().pool_millis(), 0);
    assert!(w.tech_completions().any(|c| c.tech == t));
}

#[test]
fn a_master_who_dies_holds_the_tech_until_another_is_made() {
    let defs = gated_defs();
    let mut w = setup(&defs);
    let t = defs.tech_id("gated").unwrap();
    w.apply(Command::SetResearch { tech: t }, &defs).unwrap();
    // Fully depleted, so it takes damage this tick instead of regenerating.
    w.units[4].needs.hunger = 0.0;
    w.units[4].needs.health = 0.0;
    w.tick(&defs);
    w.tick(&defs);
    assert_eq!(w.units.len(), 4, "the master died");
    w.research.bank_for_test(25_000);
    w.tick(&defs);
    assert!(!w.research().is_known(t));
    let cost = w.knobs(&defs).research_cost;
    assert_eq!(
        w.research().waiting_on(&w.facts(), &defs, cost, w.nation),
        Some(Condition::Masters { count: 1, of: vec![Nation::Earth, Nation::Fire] })
    );
    // Population is 4, which is what the tech asks for; a resident ranking up finishes it.
    make_master(&mut w, &defs, 3);
    w.tick(&defs);
    assert!(w.research().is_known(t));
}

#[test]
fn a_learned_tech_never_unlearns() {
    let defs = gated_defs();
    let mut w = setup(&defs);
    let t = queue_gated(&mut w, &defs);
    w.tick(&defs);
    assert!(w.research().is_known(t));
    for u in &mut w.units {
        u.nation = Some(Nation::Water);
    }
    for _ in 0..3 {
        w.tick(&defs);
    }
    assert_eq!(w.facts().residents[Nation::Earth.index()], 0);
    assert!(w.research().is_known(t), "the conditions fell afterwards, the tech stays");
}

#[test]
fn the_same_script_gives_the_same_world() {
    let run = || {
        let defs = gated_defs();
        let mut w = setup(&defs);
        queue_gated(&mut w, &defs);
        w.units[1].nation = Some(Nation::Fire);
        for tick in 0..120 {
            if tick == 60 {
                w.units[1].nation = Some(Nation::Earth);
            }
            w.tick(&defs);
        }
        (ron::to_string(&w).unwrap(), w.facts().population, w.tech_completions().count())
    };
    let (a, b) = (run(), run());
    assert_eq!(a.2, 1, "it finished once the resident returned");
    assert_eq!(a, b);
}

/// A Fire world of six settlers: three Earth newcomers and three Fire, one of them a master.
fn steelworks_world(defs: &Defs) -> World {
    let spots: Vec<Pos> = (0..6).map(|x| at(x * 2, 4)).collect();
    let mut w = World::with_map(1, field(20, 8), Nation::Fire, &spots, defs);
    for i in 0..3 {
        w.units[i].nation = Some(Nation::Earth);
    }
    make_master(&mut w, defs, 4);
    w.refresh_roster(defs);
    w
}

#[test]
fn a_fusion_is_on_the_tree_and_researchable_only_once_the_gate_stands() {
    let defs = Defs::bundled();
    let steelworks = defs.tech_id("steelworks").unwrap();
    let mut w = steelworks_world(&defs);
    assert!(!w.united());
    assert!(w.tree_techs(&defs).all(|(id, _)| id != steelworks), "hidden before the gate");
    assert_eq!(
        w.apply(Command::SetResearch { tech: steelworks }, &defs),
        Err(CommandError::Research(ResearchError::GateFirst))
    );
    assert!(w.research().queue().is_empty());

    w.endeavor = EndeavorState::Complete { tick: 0 };
    // The fusion is every nation's, Fire's home included, once the gate stands.
    assert!(w.tree_techs(&defs).any(|(id, _)| id == steelworks));
    assert!(w.facts().united);
    w.apply(Command::SetResearch { tech: steelworks }, &defs).unwrap();
    assert_eq!(w.research().queue().last(), Some(&steelworks));
}

#[test]
fn a_fusion_waits_for_its_residents_and_needs_no_master() {
    let defs = Defs::bundled();
    let steelworks = defs.tech_id("steelworks").unwrap();
    let mut w = steelworks_world(&defs);
    w.endeavor = EndeavorState::Complete { tick: 0 };
    // Two Earth residents, three Fire and a Fire master: one Earth short.
    w.units[2].nation = Some(Nation::Water);
    w.refresh_roster(&defs);
    let err = w.apply(Command::SetResearch { tech: steelworks }, &defs).unwrap_err();
    let CommandError::Research(ResearchError::NotYet(c)) = err else { panic!("{err:?}") };
    assert_eq!(c, Condition::Residents { nation: Nation::Earth, need: 3, have: 2 });
    w.units[2].nation = Some(Nation::Earth);
    // #955: no master of either nation (the Fire master forgets his bends) is no obstacle.
    w.units[4].bends = 0;
    w.units[4].bends_by.clear();
    assert_ne!(w.rank_of(&w.units[4], &defs).rank, Rank::Master);
    w.refresh_roster(&defs);
    w.apply(Command::SetResearch { tech: steelworks }, &defs).unwrap();
    assert_eq!(w.research().queue().last(), Some(&steelworks));
}

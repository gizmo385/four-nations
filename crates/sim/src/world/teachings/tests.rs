//! The teaching draft (#737): eligibility, the draw, keeping, hidden and lost techs, the empty
//! scroll, a scroll read at a real hall, and site rules over seeds 1 to 32.
//!
//! The bundled data has no teachings yet (each follow-up appends its own), so these tests append
//! test teachings to a copy of the defs before the world is made.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;
use crate::research::{Requires, TeachingDef, TechDef, Unlocks};
use crate::world::{Command, CommandError};

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A teaching named `id`, for `nation` (`None`: every nation), with `site`.
fn teaching(id: &str, nation: Option<Nation>, site: Option<SiteRule>) -> TechDef {
    TechDef {
        id: id.into(),
        name: id.replace('_', " "),
        description: String::new(),
        nation,
        nations: Vec::new(),
        cost: 250,
        prereqs: Vec::new(),
        unlocks: Unlocks::default(),
        core: false,
        requires: Requires::default(),
        school: None,
        cost_items: Vec::new(),
        teaching: Some(TeachingDef { site }),
    }
}

/// The bundled defs plus three teachings: two shared, one Air's, and one Earth's.
fn defs_with_teachings() -> Defs {
    let mut defs = Defs::bundled();
    defs.techs.push(teaching("t_shared_a", None, None));
    defs.techs.push(teaching("t_shared_b", None, None));
    defs.techs.push(teaching("t_air", Some(Nation::Air), None));
    defs.techs.push(teaching("t_earth", Some(Nation::Earth), None));
    defs
}

fn field() -> TileMap {
    Grid::from_fn(12, 8, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn world(defs: &Defs) -> World {
    World::with_map(1, field(), Nation::Air, &[at(2, 2)], defs)
}

fn id(defs: &Defs, name: &str) -> TechId {
    defs.tech_id(name).unwrap()
}

#[test]
fn only_this_nations_and_shared_teachings_are_eligible() {
    let defs = defs_with_teachings();
    let w = world(&defs);
    let want: Vec<TechId> =
        ["t_shared_a", "t_shared_b", "t_air"].iter().map(|n| id(&defs, n)).collect();
    assert_eq!(w.eligible_teachings(&defs), want);
}

#[test]
fn a_read_offers_two_and_keeping_one_loses_the_other() {
    let defs = defs_with_teachings();
    let mut w = world(&defs);
    w.read_teaching_scroll(at(6, 6), 2, &defs);
    let offer = w.teaching_offers()[0].clone();
    assert_eq!(offer.techs.len(), 2);
    assert!(offer.techs.windows(2).all(|p| p[0] < p[1]), "in TechId order");
    assert_eq!(offer.hall, at(6, 6));
    assert!(matches!(
        w.teaching_events().last().map(|e| &e.kind),
        Some(TeachingEventKind::Offered { techs, .. }) if *techs == offer.techs
    ));
    // On offer: still hidden, and not eligible for another draft.
    let (keep, lose) = (offer.techs[0], offer.techs[1]);
    assert_eq!(
        w.apply(Command::SetResearch { tech: keep }, &defs),
        Err(CommandError::Research(ResearchError::Hidden))
    );
    assert!(!w.eligible_teachings(&defs).contains(&keep));

    w.apply(Command::KeepTeaching { tech: keep }, &defs).unwrap();
    assert_eq!(w.kept_teachings().collect::<Vec<_>>(), vec![keep]);
    assert_eq!(w.lost_teachings().collect::<Vec<_>>(), vec![lose]);
    assert!(w.teaching_offers().is_empty());
    assert_eq!(w.shelved_at(keep), Some(at(6, 6)));
    assert!(matches!(
        w.teaching_events().last().map(|e| &e.kind),
        Some(TeachingEventKind::Kept { kept, lost }) if *kept == keep && *lost == vec![lose]
    ));
    assert_eq!(
        w.apply(Command::SetResearch { tech: lose }, &defs),
        Err(CommandError::Research(ResearchError::Lost))
    );
    w.apply(Command::SetResearch { tech: keep }, &defs).unwrap();
    assert_eq!(w.research().queue(), &[keep]);
    // Neither is offered again.
    let left = w.eligible_teachings(&defs);
    assert!(!left.contains(&keep) && !left.contains(&lose));
}

#[test]
fn keeping_a_teaching_not_on_offer_is_refused() {
    let defs = defs_with_teachings();
    let mut w = world(&defs);
    let t = id(&defs, "t_air");
    assert_eq!(
        w.apply(Command::KeepTeaching { tech: t }, &defs),
        Err(CommandError::Research(ResearchError::NotOffered))
    );
    assert!(w.command_log().is_empty());
}

#[test]
fn hidden_teachings_are_refused_and_offers_pile_up_in_read_order() {
    let defs = defs_with_teachings();
    let mut w = world(&defs);
    assert_eq!(
        w.apply(Command::SetResearch { tech: id(&defs, "t_air") }, &defs),
        Err(CommandError::Research(ResearchError::Hidden))
    );
    // Three eligible: a 2-pick, then a 1-pick from what's left, then nothing.
    w.read_teaching_scroll(at(1, 1), 2, &defs);
    w.read_teaching_scroll(at(1, 1), 2, &defs);
    let offers = w.teaching_offers();
    assert_eq!((offers[0].techs.len(), offers[1].techs.len()), (2, 1));
    let all: BTreeSet<TechId> = offers.iter().flat_map(|o| o.techs.iter().copied()).collect();
    assert_eq!(all.len(), 3, "a teaching is on at most one offer");
}

#[test]
fn a_scroll_with_nothing_to_offer_reads_for_knowledge() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    assert!(w.eligible_teachings(&defs).is_empty());
    let before = w.research().pool_millis();
    w.read_teaching_scroll(at(1, 1), 2, &defs);
    assert!(w.teaching_offers().is_empty());
    let points = defs.research.empty_scroll_knowledge;
    assert_eq!(points, 100, "owner #735");
    assert!(w.research().pool_millis() > before, "the knowledge went in");
    assert!(matches!(
        w.teaching_events().last().map(|e| &e.kind),
        Some(TeachingEventKind::Empty { points: 100 })
    ));
    assert_eq!(w.facts().scrolls_read, 0, "a teaching scroll is never counted as read");
}

/// The draw is from the saved stream: a save and load in between draws exactly what the
/// unsaved world draws.
#[test]
fn the_draw_survives_a_save() {
    let defs = defs_with_teachings();
    let mut w = world(&defs);
    w.read_teaching_scroll(at(1, 1), 1, &defs);
    let mut loaded = World::load(&w.save(&defs).unwrap(), &defs).unwrap();
    w.read_teaching_scroll(at(1, 1), 1, &defs);
    loaded.read_teaching_scroll(at(1, 1), 1, &defs);
    assert_eq!(w.teaching_offers(), loaded.teaching_offers());
}

/// A world with a study hall and a teaching scroll on the ground, ready to carry and read.
fn hall_world(defs: &Defs) -> World {
    let mut w = world(defs);
    let scroll = defs.item_id("teaching_scroll").unwrap();
    let hall = defs.building_id("study_hall").unwrap();
    let (fw, fh) = defs.building(hall).footprint;
    w.put_building(crate::geom::Footprint::new(at(8, 4), fw, fh), hall, defs);
    w.open_study_seats(at(8, 4), defs.building(hall).seats);
    w.place_ground(at(2, 4), Stack { item: scroll, count: 1 });
    w
}

fn run(w: &mut World, defs: &Defs, until: impl Fn(&World) -> bool) {
    for _ in 0..20_000 {
        if until(w) {
            return;
        }
        w.units[0].needs = crate::needs::Needs::full(&defs.needs);
        w.tick(defs);
    }
    panic!("never happened");
}

/// A run log with a draft in it replays to the same offer and the same pick: the draw is from
/// a saved stream and the pick is a logged command.
#[test]
fn a_replayed_run_log_reproduces_the_draft_and_the_pick() {
    let defs = defs_with_teachings();
    let mut a = hall_world(&defs);
    run(&mut a, &defs, |w| !w.teaching_offers().is_empty());
    let offer = a.teaching_offers()[0].clone();
    a.apply(Command::KeepTeaching { tech: offer.techs[1] }, &defs).unwrap();
    for _ in 0..50 {
        a.tick(&defs);
    }
    let mut b = hall_world(&defs);
    b.replay(a.command_log(), a.tick, &defs);
    assert_eq!(
        b.teaching_events().cloned().collect::<Vec<_>>(),
        a.teaching_events().cloned().collect::<Vec<_>>()
    );
    assert_eq!(b.kept_teachings().collect::<Vec<_>>(), vec![offer.techs[1]]);
    assert_eq!(b.lost_teachings().collect::<Vec<_>>(), vec![offer.techs[0]]);
}

/// The whole path: a teaching scroll on the ground is carried to a study hall (it is readable,
/// so never stockpiled) and read there, which offers a draft shelved at that hall.
#[test]
fn a_teaching_scroll_carried_to_a_hall_opens_a_draft() {
    let defs = defs_with_teachings();
    let mut w = world(&defs);
    let scroll = defs.item_id("teaching_scroll").unwrap();
    assert_eq!(defs.item(scroll).reveals, Some(2));
    assert!(defs.item(scroll).is_readable());
    let hall = defs.building_id("study_hall").unwrap();
    let (fw, fh) = defs.building(hall).footprint;
    w.put_building(crate::geom::Footprint::new(at(8, 4), fw, fh), hall, &defs);
    w.open_study_seats(at(8, 4), defs.building(hall).seats);
    w.place_ground(at(2, 4), Stack { item: scroll, count: 1 });
    for _ in 0..20_000 {
        if !w.teaching_offers().is_empty() {
            break;
        }
        w.units[0].needs = crate::needs::Needs::full(&defs.needs);
        w.tick(&defs);
    }
    assert_eq!(w.teaching_offers().len(), 1, "read at the hall");
    assert_eq!(w.teaching_offers()[0].hall, at(8, 4));
    assert!(w.ground_items().all(|(_, s)| s.item != scroll), "used up");
    assert_eq!(w.facts().scrolls_read, 0);
}

/// Seeds 1 to 32 on Small: a desert teaching and a mountain teaching with the doc's site rules
/// (`sandbending`'s and `crystal_growing`'s) are offered only where their rule holds, and are
/// excluded somewhere, so the rule bites.
#[test]
fn no_draft_offers_a_teaching_whose_site_rule_fails() {
    let mut defs = Defs::bundled();
    let desert =
        SiteRule { terrains: None, biomes: Some(vec!["desert".into()]), within: 40, min_tiles: 40 };
    let rock = SiteRule {
        terrains: Some(vec![Terrain::Mountain]),
        biomes: None,
        within: 30,
        min_tiles: 20,
    };
    defs.techs.push(teaching("t_sand", None, Some(desert.clone())));
    defs.techs.push(teaching("t_rock", None, Some(rock.clone())));
    let (sand, stone) = (id(&defs, "t_sand"), id(&defs, "t_rock"));
    let size = defs.embark.map_size("small").unwrap();
    let mut excluded = 0;
    for seed in 1..=32 {
        let cfg = crate::embark::Survey::config(seed, size);
        let mut w = World::new(&cfg, Nation::Earth, 3, &defs);
        let holds = |w: &World, t: TechId| {
            let rule = if t == sand { &desert } else { &rock };
            w.site_holds(rule, &defs)
        };
        excluded += [sand, stone].iter().filter(|&&t| !holds(&w, t)).count();
        w.read_teaching_scroll(at(0, 0), 4, &defs);
        for &t in w.teaching_offers().iter().flat_map(|o| &o.techs) {
            assert!(holds(&w, t), "seed {seed}: offered {} with its site rule failing", t.0);
        }
    }
    assert!(excluded > 0, "some seed fails a site rule");
}

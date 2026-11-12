//! A v3 save with something in each of its nine event lists, kept as text (#605).
//!
//! Up to save version 3 the world kept bends, tech completions, deaths, endeavor events,
//! arrivals, season events, plants picked to death, harm and finished charts in nine separate
//! lists. Version 4 keeps one ordered event log (`World::events`), and `World::load` merges an
//! old save's lists into it by tick (`world/events.rs`). This file pins that merge.
//!
//! # The fixture
//!
//! `fixtures/v3_events.ron` is a 12x8 field of grass with an Earth settlement of three, saved at
//! tick 900 by the build before #605. A temporary test wrote it, with entries pushed straight
//! into each list: several kinds on tick 600, two bends on that tick (a failed one first), and
//! the rest spread before and after. The test was removed with the format change, as #602 and
//! #604 removed theirs: a file a v4 build wrote would prove nothing about v3 saves.
//! `fn_ui::alerts` loads the same fixture to check that the alert log reads as it did then.

use fn_sim::world::{Event, SeasonEventKind};
use fn_sim::{Defs, SAVE_VERSION, SaveError, World};

const FIXTURE: &str = include_str!("fixtures/v3_events.ron");

fn loaded(defs: &Defs) -> World {
    World::load(FIXTURE, defs).unwrap()
}

/// A short name for each event, to compare orders.
fn name(e: &Event) -> &'static str {
    match e {
        Event::Bend(_) => "bend",
        Event::Tech(_) => "tech",
        Event::Death(_) => "death",
        Event::Endeavor(_) => "endeavor",
        Event::Arrival(_) => "arrival",
        Event::Season(_) => "season",
        Event::Strain(_) => "strain",
        Event::Harm(_) => "harm",
        Event::Chart(_) => "chart",
        Event::Caravan(_) => "caravan",
        Event::Beast(_) => "beast",
        Event::Find(_) => "find",
        Event::Teaching(_) => "teaching",
    }
}

#[test]
fn the_fixture_is_a_v3_save() {
    let info = fn_sim::save::inspect(FIXTURE).unwrap();
    assert_eq!(info.version, 3);
    assert!(info.version < SAVE_VERSION, "the fixture loads through the upgrade to one log");
    for list in [
        "bend_events:[(",
        "deaths:[(",
        "log:[(",
        "endeavor_log:(met:[],events:[(",
        "arrivals:[(",
        "season_events:[(",
        "strain_events:[(",
        "harm_events:[(",
        "chart_events:[(",
    ] {
        assert!(FIXTURE.contains(list), "the fixture has entries in {list}");
    }
}

/// The nine lists come out as one log, by tick, and on a tick in the order the alert log has
/// always read them in (`Event::order`); each list keeps its own order.
#[test]
fn an_old_saves_event_lists_merge_into_one_log_by_tick() {
    let defs = Defs::bundled();
    let w = loaded(&defs);
    let got: Vec<(u64, &str)> = w.events().iter().map(|e| (e.tick(), name(e))).collect();
    let want = [
        (60, "season"),
        (120, "bend"),
        (200, "harm"),
        (300, "tech"),
        (450, "endeavor"),
        (600, "bend"),
        (600, "bend"),
        (600, "tech"),
        (600, "death"),
        (600, "endeavor"),
        (600, "arrival"),
        (600, "season"),
        (600, "strain"),
        (600, "harm"),
        (600, "chart"),
        (720, "chart"),
        (840, "bend"),
    ];
    assert_eq!(got, want);
    // The two bends on tick 600 keep their own order: the failed one was logged first.
    let at_600: Vec<_> = w.bend_events().filter(|b| b.tick == 600).map(|b| b.pos.x).collect();
    assert_eq!(at_600, [2, 3]);
    assert!(matches!(
        w.season_events().next_back().map(|e| e.kind),
        Some(SeasonEventKind::Changed { season: 3 })
    ));
    assert_eq!(w.deaths().next().map(|d| d.name.as_str()), Some("Kiluk"));
    assert_eq!(w.tech_completions().count(), 2);
}

/// The upgraded world saves as v4: one `events` list, none of the nine old names, and loading
/// and saving it again gives the same text.
#[test]
fn an_upgraded_save_writes_one_log_and_round_trips() {
    let defs = Defs::bundled();
    let text = loaded(&defs).save(&defs).unwrap();
    assert_eq!(fn_sim::save::inspect(&text).unwrap().version, SAVE_VERSION);
    let body = text.splitn(3, '\n').nth(2).unwrap();
    assert!(body.contains(",events:[Season((tick:60,"), "{body}");
    for old in [
        "bend_events",
        "deaths:",
        ",log:",
        "arrivals",
        "season_events",
        "strain_events",
        "harm_events",
        "chart_events",
    ] {
        assert!(!body.contains(old), "the v4 body still writes {old}");
    }
    let again = World::load(&text, &defs).unwrap();
    assert_eq!(again.save(&defs).unwrap(), text);
    assert_eq!(again.events(), loaded(&defs).events());
}

/// The upgraded world plays on: new events go after the merged ones, so the log's ticks never
/// go down, and a newcomer's id is still past the dead settler's (`fixup_next_unit` reads the
/// deaths from the log).
#[test]
fn an_upgraded_save_plays_on() {
    let defs = Defs::bundled();
    let mut w = loaded(&defs);
    let before = w.events().len();
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert!(w.events().len() >= before);
    assert!(w.events().windows(2).all(|p| p[0].tick() <= p[1].tick()));
    assert!(w.units.iter().all(|u| w.deaths().all(|d| d.id != u.id)));
    let text = w.save(&defs).unwrap();
    assert_eq!(World::load(&text, &defs).unwrap().save(&defs).unwrap(), text);
}

/// A v4 body that still carries an old list is refused rather than half-read: only a v1 to v3
/// save is merged.
#[test]
fn a_v4_save_with_an_old_list_is_refused() {
    let defs = Defs::bundled();
    let text = loaded(&defs).save(&defs).unwrap();
    let old = FIXTURE.split("bend_events:").nth(1).unwrap();
    let list = &old[..old.find("],").unwrap() + 1];
    let (head, body) = text.rsplit_once('\n').unwrap();
    let body = body.replacen('(', &format!("(bend_events:{list},"), 1);
    match World::load(&format!("{head}\n{body}"), &defs) {
        Err(SaveError::Corrupt(why)) => assert!(why.contains("event"), "{why}"),
        other => panic!("expected Corrupt, got {:?}", other.map(|_| ())),
    }
}

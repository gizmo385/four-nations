//! The event log (#605): one ordered list, typed filters over it, and the merge of old saves'
//! lists. Loading a real v3 save is `tests/save_v3_fixture.rs`.

use super::*;

/// A small Earth world played for a while: long enough to research a tech and log events.
fn played() -> (World, Defs) {
    let defs = Defs::bundled();
    let cfg = MapGenConfig { width: 48, height: 32, seed: 3, ..Default::default() };
    let mut w = World::new(&cfg, Nation::Earth, 6, &defs);
    let tech = defs.tech_id("pottery").unwrap();
    w.apply(Command::SetResearch { tech }, &defs).unwrap();
    for _ in 0..20_000 {
        w.tick(&defs);
        if w.research().is_known(tech) {
            break;
        }
    }
    assert!(w.research().is_known(tech), "researched");
    (w, defs)
}

#[test]
fn the_log_is_in_tick_order_and_the_filters_split_it() {
    let (w, _) = played();
    assert!(w.events().windows(2).all(|p| p[0].tick() <= p[1].tick()));
    assert!(w.tech_completions().count() >= 1);
    let filtered = w.bend_events().count()
        + w.tech_completions().count()
        + w.deaths().count()
        + w.endeavor_events().count()
        + w.arrivals().count()
        + w.season_events().count()
        + w.strain_events().count()
        + w.harm_events().count()
        + w.chart_events().count();
    assert_eq!(filtered, w.events().len(), "every event is in exactly one filter");
    let last = w.tech_completions().next_back().unwrap();
    assert!(matches!(w.events().last(), Some(Event::Tech(t)) if t == last), "the tick it finished");
    assert_eq!(w.events_this_tick(), &w.events()[w.events().len() - 1..]);
}

#[test]
fn events_this_tick_is_empty_once_the_tick_has_passed() {
    let (mut w, defs) = played();
    let mut quiet = false;
    for _ in 0..100 {
        w.tick(&defs);
        let now = w.events_this_tick();
        assert!(now.iter().all(|e| e.tick() == w.tick));
        let start = w.events().len() - now.len();
        assert!(w.events()[..start].iter().all(|e| e.tick() < w.tick));
        quiet |= now.is_empty();
    }
    assert!(quiet, "some tick logged nothing");
}

/// The merge's order on one tick is `Event::order`, then each old list's own order, whatever
/// order the lists were read in.
#[test]
fn old_lists_merge_by_tick_then_kind_then_their_own_order() {
    let defs = Defs::bundled();
    let cfg = MapGenConfig { width: 24, height: 16, seed: 3, ..Default::default() };
    let mut w = World::new(&cfg, Nation::Earth, 2, &defs);
    let unit = w.units[0].id;
    let pos = w.units[0].pos;
    let harm = |tick| HarmEvent { tick, unit, pos, kind: HarmEventKind::Injured };
    w.v3_harm_events = vec![harm(4), harm(9)];
    w.v3_arrivals = vec![
        Arrival { tick: 9, unit, name: "A".into() },
        Arrival { tick: 9, unit, name: "B".into() },
    ];
    w.v3_season_events = vec![SeasonEvent { tick: 2, kind: SeasonEventKind::WinterWarning }];
    w.v3_deaths = vec![Death {
        id: UnitId(99),
        name: "C".into(),
        tick: 9,
        cause: DeathCause::Starvation,
        weakened_by: None,
        pos: None,
    }];
    assert!(w.has_v3_logs());
    w.upgrade_from_v3();
    assert!(!w.has_v3_logs());
    let got: Vec<(u64, u8)> = w.events().iter().map(|e| (e.tick(), e.order())).collect();
    assert_eq!(got, [(2, 5), (4, 7), (9, 2), (9, 4), (9, 4), (9, 7)]);
    let names: Vec<&str> = w.arrivals().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["A", "B"]);
}

/// A death carries the tile it happened on (#775); a save or run log from before that has none.
#[test]
fn a_death_serializes_its_tile_and_reads_old_ones_without() {
    let death = Death {
        id: UnitId(3),
        name: "Mei".into(),
        tick: 7,
        cause: DeathCause::Starvation,
        weakened_by: None,
        pos: Some(Pos::new(4, 9)),
    };
    let json = serde_json::to_value(&death).unwrap();
    assert_eq!(json["pos"], serde_json::to_value(Pos::new(4, 9)).unwrap());
    assert_eq!(serde_json::from_value::<Death>(json.clone()).unwrap(), death);
    let mut old = json;
    old.as_object_mut().unwrap().remove("pos");
    assert_eq!(serde_json::from_value::<Death>(old).unwrap().pos, None);
}

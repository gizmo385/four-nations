use super::*;
use crate::autoplay::Economy;
use crate::mapgen::MapGenConfig;
use crate::nation::Nation;

fn world(nation: Nation, seed: u64) -> (World, Defs) {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    let cfg =
        MapGenConfig { seed, width: size.width, height: size.height, ..MapGenConfig::default() };
    (World::new(&cfg, nation, 6, &defs), defs)
}

#[test]
fn the_road_is_short_enough_and_starts_at_the_stockpile() {
    let (mut w, defs) = world(Nation::Earth, 42);
    let economy = Economy::new(2);
    economy.step(&mut w, &defs);
    let anchor = w.settlement_anchor();
    let path = road_path(&w, &defs, anchor).expect("a deposit within reach");
    assert!(path.len() <= ROAD_TILES);
    assert!(w.stockpile_zones().any(|z| z == path[0]));
}

#[test]
fn site_anchor_falls_back_to_the_settlement_anchor() {
    let (w, defs) = world(Nation::Water, 3);
    let anchor = w.settlement_anchor();
    for id in ["oil_lamp", "hothouse", "farm_plot", "chute", "lemur_roost", "bed"] {
        assert_eq!(site_anchor(&w, &defs, id, anchor), anchor, "{id}: nothing to stand beside yet");
    }
}

/// The settlement `balance::run_economy` would found on `seed` (Survey's default site), with
/// the named techs known from the first minute: the opening's own research lands them after
/// minute 120 or so, and these tests are about what it does with them.
fn founded(nation: Nation, seed: u64, techs: &[&str]) -> (World, Defs) {
    use crate::embark::Survey;
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    let cfg =
        MapGenConfig { seed, width: size.width, height: size.height, ..MapGenConfig::default() };
    let survey = Survey::new(&cfg, &defs);
    let start = survey.default_site(&defs);
    let mut w = World::found(survey, nation, start, 6, Some("normal".to_owned()), &defs)
        .expect("Survey::default_site always names a site World::found accepts");
    for t in techs {
        w.grant_tech(defs.tech_id(t).unwrap_or_else(|| panic!("no tech {t}")));
    }
    (w, defs)
}

fn play(w: &mut World, defs: &Defs, minutes: u64) {
    let economy = Economy::new(6);
    for tick in 0..minutes * 600 {
        if crate::autoplay::Opening::due(tick) {
            economy.step(w, defs);
        }
        w.tick(defs);
    }
}

/// Plays a minute at a time until `done`, for at most `limit` minutes: what the opening does
/// with a tech lands at a different minute as its other choices change (campfires, deals and
/// newcomers all move the crew around), so these tests wait for the thing, not for a minute.
fn play_until(w: &mut World, defs: &Defs, limit: u64, done: impl Fn(&World) -> bool) {
    for _ in 0..limit {
        if done(w) {
            return;
        }
        play(w, defs, 1);
    }
}

/// Every building and site of `id`, as footprints.
fn of(w: &World, defs: &Defs, id: &str) -> Vec<Footprint> {
    let building = defs.building_id(id).unwrap();
    let built = w.buildings().filter(|b| b.id == building).filter_map(|b| w.footprint_at(b.anchor));
    let sites = w.build_sites().filter(|&(_, b, _)| b == building).map(|(f, _, _)| f);
    built.chain(sites).collect()
}

fn gap_to_stores(w: &World, f: Footprint) -> i32 {
    f.tiles().flat_map(|t| w.stockpile_zones().map(move |z| z.chebyshev(t))).min().unwrap()
}

/// How many `Bend` commands for `ability` the opening has issued.
fn bends(w: &World, ability: AbilityId) -> usize {
    w.command_log()
        .iter()
        .filter(|r| matches!(r.command, Command::Bend { ability: a, .. } if a == ability))
        .count()
}

#[test]
fn the_far_deposit_is_remembered_once_it_is_quarried() {
    let (mut w, defs) = founded(Nation::Air, 1, &[]);
    play(&mut w, &defs, 120);
    let far = far_deposit(&w, &defs, w.settlement_anchor()).expect("a deposit was quarried");
    assert!(far.from_stores >= 20, "the farthest deposit is {} from stores", far.from_stores);
    assert!(w.map[far.deposit].resource.is_none(), "it is quarried out by minute 120");
    assert!(w.stockpile_zones().all(|z| z.chebyshev(far.deposit) >= far.from_stores));
}

#[test]
fn a_waystation_stands_once_beside_the_far_deposit_and_far_from_stores() {
    let (mut w, defs) = founded(Nation::Air, 1, &["waystations"]);
    // Minute by minute until the site is designated, so the far deposit is the one it was
    // placed beside (the settlement's anchor, and with it the search box, drifts).
    let mut far = None;
    for _ in 0..150 {
        let before = far_deposit(&w, &defs, w.settlement_anchor());
        play(&mut w, &defs, 1);
        if !of(&w, &defs, "waystation").is_empty() {
            far = before;
            break;
        }
    }
    let far = far.expect("a Waystation is designated");
    let ways = of(&w, &defs, "waystation");
    assert_eq!(ways.len(), 1, "one Waystation");
    assert!(gap_to_stores(&w, ways[0]) >= 20, "the building's own placement rule");
    assert!(ways[0].distance(far.stand) <= OUTPOST_SEARCH, "beside the far deposit");
    play(&mut w, &defs, 30);
    assert_eq!(of(&w, &defs, "waystation").len(), 1, "once");
}

#[test]
fn no_waystation_without_the_tech_and_none_where_nothing_is_far() {
    let (mut w, defs) = founded(Nation::Air, 1, &[]);
    play(&mut w, &defs, 100);
    assert!(of(&w, &defs, "waystation").is_empty(), "waystations isn't known by minute 100");
    // Seed 2's deposits all lie within 16 tiles of stores.
    let (mut w, defs) = founded(Nation::Air, 2, &["waystations"]);
    play(&mut w, &defs, 150);
    let far = far_deposit(&w, &defs, w.settlement_anchor()).unwrap();
    assert!(far.from_stores < 20);
    assert!(of(&w, &defs, "waystation").is_empty());
}

#[test]
fn air_raises_one_linked_wind_gate_pair_from_the_stockpile_to_the_far_deposit() {
    let (mut w, defs) = founded(Nation::Air, 1, &["wind_gates"]);
    // Until the pair works: the sweep's Wind Turbine can later stand on a gate's door and cut the
    // pair off (seed 1, minute 102; #837), which isn't what this test is about.
    play_until(&mut w, &defs, 150, |w| {
        let gates = of(w, &defs, "wind_gate");
        gates.len() == 2
            && gates.iter().all(|g| {
                matches!(
                    w.pair_status(g.anchor, &defs),
                    Some(crate::world::PairStatus::Linked { .. })
                )
            })
    });
    let gates = of(&w, &defs, "wind_gate");
    assert_eq!(gates.len(), 2, "one pair");
    let (near, far) = if gap_to_stores(&w, gates[0]) <= gap_to_stores(&w, gates[1]) {
        (gates[0], gates[1])
    } else {
        (gates[1], gates[0])
    };
    assert!(gap_to_stores(&w, near) <= OUTPOST_SEARCH + 1, "one end at the stockpile's edge");
    let pair = defs.building(defs.building_id("wind_gate").unwrap()).pair.unwrap();
    let apart = near.anchor.chebyshev(far.anchor);
    assert!((pair.min_tiles as i32..=pair.reach as i32).contains(&apart), "{apart} apart");
    assert_eq!(
        w.pair_status(near.anchor, &defs),
        Some(crate::world::PairStatus::Linked { partner: far.anchor, distance: apart as u32 }),
        "the pair is linked and working"
    );
}

#[test]
fn fire_raises_a_balloon_dock_pair_when_it_has_the_metal() {
    let (mut w, defs) = founded(Nation::Fire, 1, &["cargo_balloons"]);
    play_until(&mut w, &defs, 240, |w| {
        let docks = of(w, &defs, "balloon_dock");
        docks.len() == 2 && w.pair_partner(docks[0].anchor, &defs).is_some()
    });
    let docks = of(&w, &defs, "balloon_dock");
    assert_eq!(docks.len(), 2, "one pair");
    let pair = defs.building(defs.building_id("balloon_dock").unwrap()).pair.unwrap();
    assert!(docks[0].anchor.chebyshev(docks[1].anchor) >= pair.min_tiles as i32);
    assert!(w.pair_partner(docks[0].anchor, &defs).is_some(), "the docks are linked");
}

#[test]
fn no_pair_is_raised_for_a_nation_the_tech_isnt_open_to() {
    let (mut w, defs) = founded(Nation::Fire, 8, &[]);
    play(&mut w, &defs, 100);
    assert!(of(&w, &defs, "wind_gate").is_empty());
    assert!(of(&w, &defs, "balloon_dock").is_empty(), "cargo_balloons isn't known yet");
}

#[test]
fn earth_fire_and_water_each_raise_one_stable_with_its_beast() {
    for nation in [Nation::Earth, Nation::Fire, Nation::Water] {
        let (mut w, defs) = founded(nation, 1, &["herding"]);
        play(&mut w, &defs, 100);
        assert_eq!(of(&w, &defs, "stable").len(), 1, "{nation:?}: one Stable");
        assert_eq!(w.beasts().count(), 1, "{nation:?}: housing its beast");
    }
}

#[test]
fn water_cuts_one_short_canal_from_the_river_toward_the_plots() {
    let (mut w, defs) = founded(Nation::Water, 4, &["canals"]);
    let canal =
        |w: &World| w.map.iter().filter(|(_, t)| t.terrain == crate::defs::Terrain::Canal).count();
    play_until(&mut w, &defs, 200, |w| canal(w) >= 1);
    play(&mut w, &defs, 20);
    let ability = defs.ability_id("carve_canal").unwrap();
    let cut = bends(&w, ability);
    assert!((1..=CANAL_TILES).contains(&cut), "{cut} tiles designated");
    assert!(canal(&w) >= 1, "and some are cut");
    let radius = defs.canal.irrigation_radius as i32;
    let plots = of(&w, &defs, "farm_plot");
    let reaches = w
        .map
        .iter()
        .filter(|(_, t)| t.terrain == crate::defs::Terrain::Canal)
        .any(|(p, _)| plots.iter().any(|f| f.distance(p) <= radius));
    assert!(reaches, "the canal waters a plot");
    // Once: more minutes, no second canal.
    play(&mut w, &defs, 30);
    assert_eq!(bends(&w, ability), cut);
}

#[test]
fn earth_digs_a_den_beside_the_rock_and_lines_a_tunnel_from_it_once() {
    let (mut w, defs) = founded(Nation::Earth, 7, &["tunneling", "badgermole_ways"]);
    let den_of = |w: &World| {
        w.buildings().find(|b| defs.building(b.id).id == "badgermole_den").map(|b| b.anchor)
    };
    let laid = |w: &World| {
        den_of(w).map_or(0, |den| {
            w.boring_left(den, &defs).unwrap() + w.bored_by(den, &defs).unwrap() as usize
        })
    };
    play_until(&mut w, &defs, 220, |w| laid(w) >= MIN_TUNNEL_TILES);
    let dens = of(&w, &defs, "badgermole_den");
    assert_eq!(dens.len(), 1, "one den");
    let bore = defs.ability_id("bore_tunnel").unwrap();
    assert!(own_bends(&w, &defs).contains(&bore), "the den takes tunnelling over");
    assert!((MIN_TUNNEL_TILES..=TUNNEL_TILES).contains(&laid(&w)), "a line was laid: {}", laid(&w));
    let cast = bends(&w, bore);
    play(&mut w, &defs, 20);
    assert_eq!(bends(&w, bore), cast, "no second line");
    assert!(laid(&w) <= TUNNEL_TILES, "still one line: {}", laid(&w));
}

#[test]
fn the_phase_2_opening_is_deterministic() {
    let run = || {
        let (mut w, defs) = founded(Nation::Air, 1, &["waystations", "wind_gates"]);
        play(&mut w, &defs, 140);
        w.command_log().to_vec()
    };
    assert_eq!(run(), run());
}

use super::*;
use crate::autoplay::{BedPolicy, Economy, HealingPolicy, Opening};
use crate::embark::Survey;
use crate::mapgen::MapGenConfig;

/// The `steady` style's opening, as `data/harness.toml` spells it.
fn steady() -> Economy {
    Economy::new(6)
        .with_farms(true)
        .with_healing(HealingPolicy::Planned)
        .with_beds(BedPolicy::ToTarget { margin: 2 })
}

fn found(nation: Nation, seed: u64) -> (World, Defs) {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    let cfg =
        MapGenConfig { seed, width: size.width, height: size.height, ..MapGenConfig::default() };
    let survey = Survey::new(&cfg, &defs);
    let start = survey.default_site(&defs);
    let w = World::found(survey, nation, start, 6, Some("normal".to_owned()), &defs)
        .expect("Survey::default_site always names a site World::found accepts");
    (w, defs)
}

/// Plays `minutes` of the steady opening, standing the gate at `gate_at`.
fn play(w: &mut World, defs: &Defs, economy: &Economy, from: u64, to: u64, gate_at: u64) {
    for tick in from * 600..to * 600 {
        if tick == gate_at * 600 {
            w.complete_endeavor(defs);
        }
        if Opening::due(tick) {
            economy.step(w, defs);
        }
        w.tick(defs);
    }
}

#[test]
fn nothing_changes_before_the_gate() {
    let (mut w, defs) = found(Nation::Earth, 1);
    let economy = steady();
    play(&mut w, &defs, &economy, 0, 30, u64::MAX / 600);
    assert!(!active(&w));
    let before = w.command_log().len();
    set_targets(&mut w, &defs, true);
    let anchor = w.settlement_anchor();
    step(&mut w, &defs, anchor);
    assert_eq!(w.command_log().len(), before, "no command before the gate");
}

#[test]
fn the_bed_ceiling_lifts_once_the_city_is_united() {
    let (mut w, defs) = found(Nation::Earth, 1);
    let economy = steady();
    play(&mut w, &defs, &economy, 0, 21, 20);
    assert!(w.united());
    assert_eq!(economy.bed_ceiling(&w, &defs), u32::MAX);
}

#[test]
fn the_exchange_is_queued_cheapest_first_once_residents_are_there() {
    let (mut w, defs) = found(Nation::Earth, 1);
    let economy = steady();
    play(&mut w, &defs, &economy, 0, 5, 5);
    // No Fire resident yet: nothing can be borrowed, so no exchange tech is queued.
    set_targets(&mut w, &defs, true);
    let nation = w.nation;
    assert!(w.research().queue().iter().all(|&t| !is_exchange(&defs, nation, t)));
}

#[test]
fn can_queue_refuses_what_set_target_would() {
    let (mut w, defs) = found(Nation::Earth, 1);
    let economy = steady();
    play(&mut w, &defs, &economy, 0, 5, 5);
    let facts_ok = |w: &World, id: &str| can_queue(w, &defs, defs.tech_id(id).unwrap());
    // Brickmaking is Fire's: three Fire residents are needed and none live here.
    assert!(!facts_ok(&w, "brickmaking"));
    // Every fusion wants residents too.
    for t in defs.techs.iter().enumerate().filter(|(_, t)| t.is_fusion()) {
        assert!(!can_queue(&w, &defs, TechId(t.0 as u16)), "{}", t.1.id);
    }
}

#[test]
fn an_aqueduct_line_runs_from_the_water_to_a_plot() {
    let (mut w, defs) = found(Nation::Earth, 1);
    let economy = steady();
    play(&mut w, &defs, &economy, 0, 150, 150);
    w.grant_tech(defs.tech_id("aqueducts").unwrap());
    let stone = defs.item_id("stone").unwrap();
    assert!(w.stored_total(stone) > 0 || w.ground_items().any(|(_, s)| s.item == stone));
    for m in 150..180 {
        play(&mut w, &defs, &economy, m, m + 1, 150);
        raise_aqueduct(&mut w, &defs);
    }
    let id = defs.building_id("aqueduct").unwrap();
    let sited = building_count(&w, id);
    assert!(sited <= AQUEDUCT_TILES as u32, "{sited} tiles");
}

/// A Fire city (seed 8) played 420 minutes with the gate stood at 240 borrows techs, learns
/// fusions, builds on the bank, makes steel and sends an airship (#764). One long run, so the
/// checks that need a settled city share it. Seed 2 was the seed until the leaf bends became
/// borrowable (#969): the opening then borrows three more techs first, and whether a given city
/// loads its first airship by minute 420 is luck of the seed (most seeds don't, before or after).
#[test]
fn a_united_fire_city_borrows_fuses_and_uses_what_it_learns() {
    let (mut w, defs) = found(Nation::Fire, 8);
    let economy = steady();
    play(&mut w, &defs, &economy, 0, 420, 240);
    let nation = w.nation;
    let learned = |f: fn(&crate::research::TechDef, Nation) -> bool| {
        w.research().known_techs().filter(|&t| f(defs.tech(t), nation)).count()
    };
    assert!(learned(|t, n| t.borrowed_from(n).is_some()) >= 3, "borrowed techs");
    assert!(learned(|t, _| t.is_fusion()) >= 2, "fusions");
    let tally = |name: &str| w.tallies().school(name);
    assert!(tally("steel_made") > 0, "the Steelworks made metal");
    assert!(tally("voyages") > 0, "an airship sailed");
    let engine = defs.building_id("steam_engine").unwrap();
    assert!(
        w.buildings().any(|b| b.id == engine && w.on_bank(b, &defs)),
        "a Steam Engine stands on the bank"
    );
    assert!(w.population() > 30, "newcomers kept coming: {}", w.population());
}

/// Call Rain is a joint bend whose second nation is always a newcomer, and a newcomer ranks up
/// only through forms the city has learned the tech for (`world/bending/joint_tests.rs`, #958).
/// A newcomer has one form, so is no adept, until the city borrows a leaf bend (#969); this city
/// is a minute past the gate and has borrowed nothing. The opening used to designate it each
/// spring anyway, and the Air caster that took the first slot stood on the plot for the whole
/// year; now it designates nothing while a nation has no caster.
#[test]
fn call_rain_is_not_designated_while_a_nation_has_no_caster() {
    let (mut w, defs) = found(Nation::Air, 1);
    let economy = steady();
    play(&mut w, &defs, &economy, 0, 241, 240);
    assert!(w.united());
    w.grant_tech(defs.tech_id("cloudbending").unwrap());
    let rain = defs.ability_id("call_rain").unwrap();
    assert_eq!(w.season_index(&defs), 0, "spring");
    assert!(w.check_ability(rain, &defs).is_ok());
    let counts = w.joint_caster_counts(rain, &defs).unwrap();
    assert!(counts[1].1 > 0, "Air adepts: {counts:?}");
    assert_eq!(counts[0], (Nation::Water, 0), "no Water adept");
    call_rain(&mut w, &defs);
    assert!(w.bend_designations().all(|(a, _)| a != rain));
}

/// Why an Earth city that learns the Steelworks makes no steel (#958): the recipe idles once
/// stores hold its `stock_limit` (8) of metal, and Earth's Metalbend has piled up far more than
/// that and nothing spends it, while a Fire city's stores sit under the limit. Seed 17 learns it
/// at minute 364. The same Metalbend also draws the ore stores down to a handful.
#[test]
fn an_earth_steelworks_idles_on_the_metal_metalbend_piled_up() {
    let (mut w, defs) = found(Nation::Earth, 17);
    let economy = steady();
    play(&mut w, &defs, &economy, 0, 420, 240);
    let steelworks = defs.building_id("steelworks").unwrap();
    let anchor = w
        .buildings()
        .find(|b| b.id == steelworks)
        .map(|b| b.anchor)
        .expect("the sweep raised the Steelworks");
    let metal = defs.item_id("metal").unwrap();
    let limit = defs.building(steelworks).recipe.as_ref().unwrap().stock_limit;
    assert!(w.stored_total(metal) >= limit, "{} metal in stores", w.stored_total(metal));
    assert!(w.craft_idle(anchor, &defs), "idle at the stock limit");
    assert_eq!(w.tallies().school("steel_made"), 0);
}

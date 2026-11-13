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

/// A Fire city (seed 2) played 420 minutes with the gate stood at 240 borrows techs, learns
/// fusions, builds on the bank, makes steel and sends an airship (#764). One long run, so the
/// checks that need a settled city share it.
#[test]
fn a_united_fire_city_borrows_fuses_and_uses_what_it_learns() {
    let (mut w, defs) = found(Nation::Fire, 2);
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

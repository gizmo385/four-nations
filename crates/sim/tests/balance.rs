//! The balance harness's CI soak test (#291's split, #300): seed 7, Earth, 90 minutes, on
//! `docs/design/difficulty.md`'s "Measuring it". It checks the ordering the presets promise, not
//! any absolute numbers: those shift with every tuning pass (#301 and after), while a preset
//! becoming no gentler than the one below it would be a real regression.
//!
//! Kept cheap deliberately, including unoptimized (`xtask ci --fast`'s `opt-level = 1`,
//! owner priority #291): `fn_sim::balance::run` never leaves a unit re-planning a whole-map
//! route for material that was never going to arrive (`fn_sim::autoplay`'s module doc), which is
//! what made an early draft of this test take over ten seconds for a single run. A much longer
//! sweep over every seed, nation and preset belongs to the `balance` binary
//! (`cargo run -p fn_harness --bin balance`), not this gate.

use fn_sim::autoplay::{Economy, Opening};
use fn_sim::balance::{EconomyConfig, RunConfig, run, run_economy};
use fn_sim::embark::Survey;
use fn_sim::{Defs, MapGenConfig, Nation, World};

const SEED: u64 = 7;
const NATION: Nation = Nation::Earth;
const MINUTES: f32 = 90.0;

fn cfg(preset: &str, opening: bool, defs: &Defs) -> RunConfig {
    let size = defs.embark.map_size("small").expect("bundled data always has a small map size");
    RunConfig {
        seed: SEED,
        nation: NATION,
        preset: Some(preset.to_owned()),
        minutes: MINUTES,
        opening,
        bend_slots: None,
        full_research: false,
        width: size.width,
        height: size.height,
    }
}

/// The same config, run twice, must give bit-for-bit the same report: `World` draws no
/// randomness beyond its own seeded streams, and `autoplay::Opening` reads only `World`.
#[test]
fn two_runs_of_the_same_config_give_identical_output() {
    let defs = Defs::bundled();
    let c = cfg("normal", true, &defs);
    assert_eq!(run(&c, &defs), run(&c, &defs));
}

/// With no input: Sandbox (no seasons, no strain) never loses anyone in 90 minutes, and Hard's
/// first death, if any, comes no later than Normal's (`docs/design/difficulty.md`'s "Measuring
/// it" table: "first winter" for Hard, "first or second winter" for Normal, "none in 3 h" for
/// Sandbox).
#[test]
fn with_no_input_hard_dies_no_later_than_normal_and_sandbox_never_does() {
    let defs = Defs::bundled();
    let sandbox = run(&cfg("sandbox", false, &defs), &defs);
    let normal = run(&cfg("normal", false, &defs), &defs);
    let hard = run(&cfg("hard", false, &defs), &defs);
    assert_eq!(sandbox.first_death_minute, None, "sandbox should lose nobody: {sandbox:?}");
    let minute_or_never =
        |r: &fn_sim::balance::RunReport| r.first_death_minute.unwrap_or(f32::INFINITY);
    assert!(
        minute_or_never(&hard) <= minute_or_never(&normal),
        "hard's first death should come no later than normal's: hard {hard:?}, normal {normal:?}"
    );
}

/// With the standard opening driving it, Normal loses nobody in 90 minutes
/// (`docs/design/difficulty.md`'s "Measuring it" table: "0" deaths by 3 h).
#[test]
fn the_opening_loses_nobody_under_normal() {
    let defs = Defs::bundled();
    let normal = run(&cfg("normal", true, &defs), &defs);
    assert_eq!(normal.deaths, 0, "{normal:?}");
}

/// The economy opening's own CI soak test (#352, `docs/design/materials.md` section 7): seed 7,
/// Earth, 60 minutes. This only checks determinism (`run_economy`, the same way `run`'s own test
/// above does) and that `World::apply` keeps accepting `Economy`'s commands throughout (if one
/// ever stopped, `fn_sim::autoplay`'s module doc says this notices, the same promise
/// `fn_sim::autoplay::Opening` makes). It asserts no absolute target: those are a data question
/// for the `balance` binary's own `--economy` sweep and `docs/design/materials.md` section 7's
/// table, not this gate.
#[test]
fn the_economy_opening_is_deterministic_and_keeps_applying() {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").expect("bundled data always has a small map size");
    let cfg = EconomyConfig {
        seed: SEED,
        nation: NATION,
        preset: Some("normal".to_owned()),
        minutes: 60.0,
        farms: true,
        branch_first: false,
        beds_only: false,
        width: size.width,
        height: size.height,
    };
    assert_eq!(run_economy(&cfg, &defs), run_economy(&cfg, &defs));

    let map_cfg =
        MapGenConfig { seed: SEED, width: size.width, height: size.height, ..Default::default() };
    let survey = Survey::new(&map_cfg, &defs);
    let start = survey.default_site(&defs);
    let mut world = World::found(survey, NATION, start, 6, Some("normal".to_owned()), &defs)
        .expect("Survey::default_site always names a site World::found accepts");
    let economy = Economy::new(6);
    for tick in 0..(60 * 600) {
        if Opening::due(tick) {
            economy.step(&mut world, &defs);
        }
        world.tick(&defs);
        world.assert_invariants(&defs);
    }
    assert!(
        !world.command_log().is_empty(),
        "expected the economy opening to issue at least one command in 60 minutes"
    );
}

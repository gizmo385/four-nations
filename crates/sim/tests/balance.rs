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
//! (`cargo run -p fn_sim --bin balance`), not this gate.

use fn_sim::balance::{RunConfig, run};
use fn_sim::{Defs, Nation};

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

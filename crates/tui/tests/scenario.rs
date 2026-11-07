//! Scenario starts (#481, docs/design/agent-scripting.md, "Scenario starts"): `fn_tui --scenario
//! ID` (a human's own path, through [`Shell::for_args`]) and `Host::from_scenario` (what
//! `fn_agent serve --scenario ID` builds) start exactly the same world -- the same frame at
//! turn 0.
//!
//! One bundled preset is enough for this: every preset's own prelude already runs once, cheaply
//! checked, in `fn_tui::scenario`'s own `bundled_scenarios_parse_and_build_a_host` (a 60-minute
//! `Economy` prelude at the `ci` profile's `opt-level = 0` is not free, and this crate's own
//! test gate already runs it there for every nation).

use fn_tui::cli;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame;
use fn_tui::scenario;
use fn_tui::shell::Shell;
use fn_tui::surface::Host;

const SIZE: (u16, u16) = (120, 40);

fn shell_frame(id: &str) -> String {
    let args = cli::parse(["--scenario", id, "--frame"].into_iter().map(str::to_owned)).unwrap();
    let mut shell = Shell::for_args(&args, SIZE, None, 0).unwrap();
    let palette = Palette::new(shell.defs(), shell.nation(), ColorDepth::TrueColor, true);
    frame::to_text(&frame::render_shell(&mut shell, &palette, SIZE))
}

fn host_frame(id: &str) -> String {
    let mut host = Host::from_scenario(id).unwrap();
    host.frame(SIZE.0, SIZE.1)
}

#[test]
fn fn_tui_and_the_agent_surface_start_a_scenario_at_the_same_frame() {
    let preset = scenario::bundled().into_iter().find(|s| s.prelude.is_some()).unwrap();
    let want = shell_frame(&preset.id);
    let got = host_frame(&preset.id);
    assert_eq!(got, want, "scenario {:?}", preset.id);
}

/// A scenario's prelude actually advances the world before turn 0 (#481): the clock reads
/// further along than a plain start with no prelude, nothing it logged shows as a new alert,
/// and turn 0 still starts at turn 0, not mid-prelude. `fresh` is cheap (no prelude to run);
/// the only expensive build here is `host`'s own.
#[test]
fn a_scenario_prelude_advances_the_clock_before_turn_zero() {
    let preset = scenario::bundled().into_iter().find(|s| s.prelude.is_some()).unwrap();
    let host = Host::from_scenario(&preset.id).unwrap();
    let obs = host.observe();
    assert_eq!(obs.turn, 0, "turn 0 starts after the prelude, not during it");
    assert!(
        obs.alerts.is_empty(),
        "nothing the prelude logged should show as new: {:?}",
        obs.alerts
    );

    let fresh = Host::new(
        preset.seed,
        preset.nation,
        preset.map_size.clone(),
        preset.difficulty.clone(),
        preset.start().unwrap(),
        false,
    )
    .unwrap();
    assert_ne!(
        host.observe().clock,
        fresh.observe().clock,
        "the prelude should have moved the clock"
    );
}

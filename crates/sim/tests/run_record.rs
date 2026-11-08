//! The run record's end-to-end check on the doc's own example (`docs/design/balance-harness.md`,
//! section 3; #534): Water on seed 3, the economy opening, Normal, 180 minutes, the run that
//! starved settlers behind a sealed pocket (#487, #490).
//!
//! It is 180 minutes, so it is ignored by default (`cargo test --release -p fn_sim --test
//! run_record -- --ignored`); the unit tests in `fn_sim::balance` cover the pieces cheaply.

use fn_sim::balance::{Flag, RunSpec, run_record};
use fn_sim::{Defs, Nation};

/// With #490 fixed nobody dies on this site. If someone ever does again, the record must have
/// raised `stranded` before the first death: that is what the detector is for.
#[test]
#[ignore = "180 minutes: run with --release -- --ignored"]
fn water_seed_3_loses_nobody_or_is_flagged_stranded_first() {
    let defs = Defs::bundled();
    let record = run_record(&RunSpec::new(3, Nation::Water, 180.0, &defs), &defs);
    let Some(death) = record.first_death_minute else {
        assert!(record.autopsies.is_empty());
        return;
    };
    let stranded = record.flag_minute(Flag::Stranded);
    assert!(
        stranded.is_some_and(|m| m < death),
        "first death at {death}, stranded {stranded:?}: {:#?}",
        record.autopsies
    );
}

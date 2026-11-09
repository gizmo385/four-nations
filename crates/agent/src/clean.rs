//! `fn_agent play --horizon TURNS` (#528, docs/design/agent-hillclimb.md, "Across runs: attempts
//! and a champion"): a clean evaluation of a strategy. One governor script and one plan play from
//! turn 0 to exactly `horizon` turns with no model, so the result measures the strategy and not
//! any one-off intervention.
//!
//! The rules, which [`run`] keeps:
//! - Event wakes are ignored (the script runs with an empty `wake_on`).
//! - When the script returns (`done`) or calls `wake` (`asked`), it runs again with its `memory`
//!   kept. A run that used no turn is followed by one forced `end_turn`, so a script that returns
//!   at once can't spin.
//! - After an `error` or a `runaway` the remaining turns run with no orders (the colony coasts),
//!   so a crash costs score instead of making the run incomparable.
//!
//! The log is written as `play` always writes it: a `Script` line before each run, the run's own
//! `Keys`/`Turn` lines, and a `Wake` line when it stops. Nothing here reads a clock or a thread
//! RNG, so two runs of the same script, plan and seed write the same bytes.

use fn_tui::surface::{Host, LogLine};
use serde_json::{Value, json};

use crate::script::{self, ActFailure, Limits, Outcome};

/// The first script run that ended in an `error` or a `runaway`: why, and the turn it stopped on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoppedBy {
    /// `"error"` or `"runaway"`.
    pub reason: String,
    pub why: Option<String>,
    /// Turns played when it stopped.
    pub turn: u32,
}

impl StoppedBy {
    pub fn json(&self) -> Value {
        json!({"reason": self.reason, "why": self.why, "turn": self.turn})
    }
}

/// How a clean run went.
#[derive(Debug)]
pub struct CleanRun {
    /// Turns played: the horizon, always.
    pub turns: u32,
    /// How many times the script was started again after the first run ended with `done` or
    /// `asked` and turns were still left.
    pub restarts: u32,
    /// The first `error` or `runaway`, if any; the rest of the run coasted.
    pub stopped_by: Option<StoppedBy>,
    /// The last script run's outcome (`reason` `"budget"` when the script was still going at the
    /// horizon).
    pub last: Outcome,
    /// Every failed `act`/`keys` over all the runs, oldest first, with the turns played when it
    /// was made (#570, `test_governor`'s trace). Kept up to a cap per script run; `acts_failed`
    /// counts them all.
    pub failures: Vec<ActFailure>,
    /// `act`/`keys` calls that worked and that failed, over all the runs.
    pub acts_ok: u32,
    pub acts_failed: u32,
    /// The script's own last `log`/`print`/`debug` lines over the runs (at most
    /// `Limits::log_tail`), oldest first.
    pub log: Vec<String>,
}

/// Plays `code` on `host` for exactly `horizon` turns (see the module doc). Every log line the
/// run makes goes to `sink` in order, as [`crate::runlog::Writer::write_line`] wants them; a
/// sink error stops the run. `tag` names the script in the `Script` lines. `horizon` must be at
/// least 1.
pub fn run(
    host: &mut Host,
    code: &str,
    tag: &str,
    plan: &Value,
    horizon: u32,
    limits: &Limits,
    mut sink: impl FnMut(&LogLine) -> Result<(), String>,
) -> Result<CleanRun, String> {
    assert!(horizon >= 1, "a horizon is at least one turn");
    let start_turn = host.observe().turn;
    let played = |host: &Host| host.observe().turn - start_turn;
    // The scripts' own turn budget is clamped to `max_turns` (a `run_script` call is at most a
    // game year); a clean run is a whole horizon in one run.
    let limits = Limits { max_turns: limits.max_turns.max(horizon), ..limits.clone() };

    let mut flushed = 0usize;
    let mut flush = |host: &Host, sink: &mut dyn FnMut(&LogLine) -> Result<(), String>| {
        let lines = host.log().lines();
        for line in &lines[flushed..] {
            sink(line)?;
        }
        flushed = lines.len();
        Ok::<(), String>(())
    };

    let mut memory = json!({});
    let mut restarts = 0u32;
    let mut stopped_by = None;
    let mut runs = 0u32;
    let mut last: Option<Outcome> = None;
    let mut failures: Vec<ActFailure> = Vec::new();
    let (mut acts_ok, mut acts_failed) = (0u32, 0u32);
    let mut log: Vec<String> = Vec::new();

    while played(host) < horizon {
        let remaining = horizon - played(host);
        if runs > 0 {
            restarts += 1;
        }
        runs += 1;
        sink(&LogLine::Script {
            name: tag.to_owned(),
            version: 0,
            code: code.to_owned(),
            turns: remaining,
            wake_on: Vec::new(),
        })?;
        let request = script::Request {
            code,
            turns: Some(remaining),
            note: Some(tag),
            version: 0,
            wake_on: &[],
            memory: &memory,
            plan,
            probe: false,
        };
        let began = played(host);
        let outcome = script::run(host, request, &limits);
        failures.extend(
            outcome
                .failures
                .iter()
                .map(|f| ActFailure { turn: began + f.turn, message: f.message.clone() }),
        );
        acts_ok += outcome.acts.ok;
        acts_failed += outcome.acts.failed;
        log.extend(outcome.log.iter().cloned());
        flush(host, &mut sink)?;
        sink(&LogLine::Wake {
            reason: outcome.reason.to_owned(),
            why: outcome.why.clone(),
            turn: outcome.observe.turn,
            memory: outcome.memory.clone(),
        })?;
        let reason = outcome.reason;
        if matches!(reason, "error" | "runaway") {
            stopped_by = Some(StoppedBy {
                reason: reason.to_owned(),
                why: outcome.why.clone(),
                turn: played(host),
            });
        }
        memory = outcome.memory.clone();
        let used_a_turn = outcome.turns_run > 0;
        last = Some(outcome);
        match reason {
            // Out of turns (the horizon), or the script gave up for good: nothing more to run.
            "budget" | "error" | "runaway" => break,
            // `done`, `asked` (and `event`, which an empty `wake_on` can't produce): run it
            // again, after a forced turn when it spent none.
            _ => {
                if !used_a_turn && played(host) < horizon {
                    host.end_turn(1);
                    flush(host, &mut sink)?;
                }
            }
        }
    }

    // Coast: no orders, one turn at a time (`end_turn(n)` stops early on an alert).
    while played(host) < horizon {
        host.end_turn(1);
    }
    flush(host, &mut sink)?;

    Ok(CleanRun {
        turns: played(host),
        restarts,
        stopped_by,
        last: last.expect("the loop ran at least once (horizon >= 1)"),
        failures,
        acts_ok,
        acts_failed,
        log: {
            let drop = log.len().saturating_sub(limits.log_tail);
            log.split_off(drop)
        },
    })
}

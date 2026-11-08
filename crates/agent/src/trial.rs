//! `test_governor` (#561, docs/design/agent-hillclimb.md, "Across runs: attempts and a champion"):
//! the model's way to see what the clean evaluation will say about its current governor and
//! plan, from turn 0, while it still has an attempt left to fix them.
//!
//! The test is [`crate::clean::run`] on a brand-new [`Host`] built from the live session's own
//! header with the seed swapped for one of the track's **train** seeds, scored the way
//! `fn_agent play --horizon` scores a log (`scorecard::build` over the lines the run made). So the
//! result is the clean evaluation's own: the same stop reason, the same turn, the same score.
//!
//! What it never does:
//! - Touch the live game. The trial has a world of its own; the live `Host`, its `RunLog`, the
//!   session's `memory`, library, notes and plan are not read for writing and nothing from the
//!   trial is written back, so a `run.jsonl` replays and scores exactly as it would have without
//!   the call (the only trace is the `call` line every tool call leaves, which replay ignores).
//! - Use a held-out seed. [`Trial::seeds`] is the train seeds and nothing else (the harness hands
//!   over no others, `--trial-seeds`), and a `seed` outside it is refused.
//! - Read a clock or a thread RNG: the same governor, plan and seed give the same result.
//!
//! The cost is wall-clock time, so it is bounded: at most [`MAX_RUNS`] tests per attempt, counted
//! from the log's own `call` lines so a chunked attempt (`serve --resume`) shares one count. A
//! full 540-turn test took 4 s for a small governor that never stops and 9 s for the campaign's
//! v16 governor (run away at turn 171, then coasting), in the release build (`cargo xtask
//! agent-run` builds `fn_agent --release`); `turns` asks for a shorter one. At the limit that is
//! a minute or so of an attempt that lasts far longer.

use fn_tui::surface::{Host, LogLine, RunLogHeader};
use serde_json::{Value, json};

use crate::clean;
use crate::scorecard;
use crate::script::Limits;

/// The tool's name.
pub const TOOL: &str = "test_governor";

/// How many times one attempt may call `test_governor`, rejected calls included: two sweeps of
/// a three-seed train panel. Enough to test a fix and test it again on the other maps; few enough
/// that testing can't take the attempt over.
pub const MAX_RUNS: u32 = 6;

/// What a session needs to run the tool: the track's train seeds and horizon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trial {
    /// The train seeds a test may use, in track order. Never a held-out seed.
    pub seeds: Vec<u64>,
    /// The track's horizon: the default (and the longest) test.
    pub turns: u32,
}

impl Trial {
    /// Reads the flags' values: a non-empty list of seeds and a horizon of at least one turn.
    pub fn new(seeds: Vec<u64>, turns: u32) -> Result<Self, String> {
        if seeds.is_empty() {
            return Err("--trial-seeds wants at least one seed".to_owned());
        }
        if turns == 0 {
            return Err("--trial-turns wants at least one turn".to_owned());
        }
        Ok(Self { seeds, turns })
    }
}

/// The tool's `tools/list` entry.
pub fn tool(trial: &Trial) -> Value {
    let seeds: Vec<String> = trial.seeds.iter().map(u64::to_string).collect();
    json!({
        "name": TOOL,
        "description": format!(
            "Runs your saved \"governor\" and the current plan from turn 0 on one of the \
             evaluation maps, on a separate copy of the world: this game is not touched. It is \
             the clean evaluation the attempt will be judged by, so it shows the same stop \
             reason and turn, and the same score, as that one will. A governor that errors or \
             runs away stops for good, and the settlement coasts with no orders from then on. \
             The result is short: stopped (null, or reason, turn and why), score, and tests \
             left. Maps: {seeds}. Defaults to the map you are playing and the full {turns} \
             turns; `turns` asks for fewer. It takes a few seconds to half a minute of \
             real time, and at most {max} tests are allowed in the whole attempt, so test a \
             finished change, not every edit.",
            seeds = seeds.join(", "),
            turns = trial.turns,
            max = MAX_RUNS,
        ),
        "inputSchema": json!({
            "type": "object",
            "properties": {
                "seed": {
                    "type": "integer",
                    "description": "One of the evaluation maps; the map you are playing by default.",
                },
                "turns": {
                    "type": "integer",
                    "description": "How many turns to test, at most the full length.",
                },
            },
        }),
    })
}

/// A finished test.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub seed: u64,
    pub turns: u32,
    /// The first error or runaway, if the script had one.
    pub stopped: Option<clean::StoppedBy>,
    pub score: f64,
}

impl Verdict {
    /// The tool's result: short, with no log in it. `left` is how many tests remain.
    pub fn json(&self, left: u32) -> Value {
        let summary = match &self.stopped {
            Some(s) => format!(
                "The governor stopped at turn {} ({}{}); the settlement coasted with no orders \
                 to turn {}.",
                s.turn,
                s.reason,
                s.why.as_ref().map_or(String::new(), |w| format!(": {w}")),
                self.turns,
            ),
            None => format!("The governor ran to turn {} without stopping.", self.turns),
        };
        json!({
            "seed": self.seed,
            "turns": self.turns,
            "stopped": self.stopped.as_ref().map(clean::StoppedBy::json),
            "score": self.score,
            "tests_left": left,
            "summary": summary,
        })
    }
}

/// Plays `code` and `plan` from turn 0 on a copy of the session `header` describes, on `seed`, for
/// `turns` turns, and scores it. `header` is the live game's own; only its seed is replaced.
pub fn run(
    header: &RunLogHeader,
    seed: u64,
    code: &str,
    plan: &Value,
    turns: u32,
    limits: &Limits,
) -> Result<Verdict, String> {
    let header = RunLogHeader { seed, ..header.clone() };
    let mut host = Host::resume(&header, &[])?;
    let mut lines: Vec<LogLine> = Vec::new();
    let run = clean::run(&mut host, code, "trial", plan, turns, limits, |line| {
        lines.push(line.clone());
        Ok(())
    })?;
    let report = scorecard::build(&header, &lines, Some(turns))?;
    Ok(Verdict { seed, turns: run.turns, stopped: run.stopped_by, score: report.replay.card.score })
}

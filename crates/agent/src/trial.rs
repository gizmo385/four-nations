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
//! The result (#570) is the clean evaluation's own score and stop reason, the scorecard's signs
//! and deaths with their causes, and a short trace by time slice ([`crate::trace`]): enough to
//! see why a run failed on a map the model isn't playing, not a log. `detail` asks for the long
//! form. The summary is about 1.2 KB and the detail about 2 KB, under 3.2 KB whatever the governor logs (pinned by a test).
//!
//! The cost is wall-clock time, so it is bounded: at most [`Trial::max_tests`] tests per attempt
//! (`data/agent.toml`, `[attempt]`), counted from the log's own `call` lines so a chunked attempt
//! (`serve --resume`) shares one count. A test of what an earlier one already tested (the same
//! saved governor, plan, map and turns, #639) is a repeat: it returns the earlier verdict with
//! `repeat: true` and a note, spends no test, and its `call` line says `repeat`, so the harness
//! does not count it as work. A full 540-turn test takes 2 to 15 s in the release build, 2 s for a colony that dies out and 13 s for a healthy one
//! (`cargo xtask agent-run` builds `fn_agent --release`); `turns` asks for a shorter one.
//!
//! Testing on the map being played (the default, when it is a train seed) is a lookahead copy:
//! the trial is that same game from turn 0. That is what a climb wants, and why the tool exists
//! only in a campaign: `fn_agent serve` gets `--trial-seeds` from `cargo xtask agent-run
//! --track` and from nothing else (`xtask/src/agent_run/campaign.rs` is the one place that sets
//! it, and a test there pins that no flag does), so a race, which has no track, never has it.

use fn_tui::surface::{Host, LogLine, RunLogHeader};
use serde_json::{Value, json};

use crate::clean;
use crate::scorecard;
use crate::script::Limits;
use crate::script::limits::AttemptLimits;
use crate::trace;

/// The tool's name.
pub const TOOL: &str = "test_governor";

/// What a repeated test says (#639): the result is the earlier one, and the right next step is
/// to change something or play the live game on, not to ask again.
pub const REPEAT_NOTE: &str = "Nothing has changed since this test (same governor, plan, map and \
    turns), so this is the earlier result and it did not use a test. Testing an unchanged \
    governor again is not progress: change the governor or the plan, or play the live game on \
    toward the horizon.";

/// What a session needs to run the tool: the track's train seeds and horizon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trial {
    /// The train seeds a test may use, in track order. Never a held-out seed.
    pub seeds: Vec<u64>,
    /// The track's horizon: the default (and the longest) test.
    pub turns: u32,
    /// How many times one attempt may call the tool, rejected calls included
    /// (`data/agent.toml`, `[attempt]`).
    pub max_tests: u32,
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
        Ok(Self { seeds, turns, max_tests: AttemptLimits::bundled().max_tests })
    }

    /// The same, with a different number of tests (tests of the tool itself use a small one).
    pub fn with_max_tests(mut self, max_tests: u32) -> Self {
        self.max_tests = max_tests;
        self
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
             The result: stopped (null, or reason, turn and why), score, the signs with the \
             turn each was met or how far it got, deaths with their causes, a trace by time \
             slice (people, idle share, settlers hungry or thirsty, your act calls that did \
             nothing and the most common reason), and tests left. `detail: true` adds food, \
             tired and hurt, deaths per slice, more refusals and the last distinct lines your \
             governor's own log() wrote: log what you would want to see on a map you can't \
             watch (stores, needs, what it just designated) and read it there. A small `turns` \
             zooms the trace in on the start. Use it to find out why a map failed, not only \
             whether. Maps: {seeds}. Defaults to the map you are playing and the full {turns} \
             turns; `turns` asks for fewer. A test of the same governor, plan, map and turns \
             as an earlier one returns that result again, marked repeat, and uses no test. It \
             takes a few seconds of real time, and at most {max} tests are allowed in the whole attempt (each is a call, and a fix is \
             another: leave calls for them).",
            seeds = seeds.join(", "),
            turns = trial.turns,
            max = trial.max_tests,
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
                "detail": {
                    "type": "boolean",
                    "description": "The long form of the trace (about 2 KB instead of 1.2 KB).",
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
    pub progress: f64,
    pub care: f64,
    /// The signs, deaths and trace ([`crate::trace`]).
    pub brief: trace::Brief,
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
            "progress": round(self.progress, 3),
            "care": round(self.care, 2),
            "tests_left": left,
            "summary": summary,
            "signs": self.brief.signs,
            "deaths": self.brief.deaths,
            "trace": self.brief.trace,
        })
    }
}

fn round(x: f64, places: i32) -> f64 {
    let k = 10f64.powi(places);
    (x * k).round() / k
}

/// Plays `code` and `plan` from turn 0 on a copy of the session `header` describes, on `seed`, for
/// `turns` turns, and scores it. `header` is the live game's own; only its seed is replaced.
/// `detail` asks for the long trace.
pub fn run(
    header: &RunLogHeader,
    seed: u64,
    code: &str,
    plan: &Value,
    turns: u32,
    detail: bool,
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
    let card = &report.replay.card;
    Ok(Verdict {
        seed,
        turns: run.turns,
        stopped: run.stopped_by.clone(),
        score: card.score,
        progress: card.progress,
        care: card.care.value,
        brief: trace::brief(&report, &run, detail),
    })
}

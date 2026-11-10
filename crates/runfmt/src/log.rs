//! The run log (`run.jsonl`): a header line, then one [`LogLine`] per line, appended as the
//! run goes. Written by `fn_agent serve` (and `xtask agent-run`, which adds `end` and `say`
//! lines), read back by `fn_tui --watch`, `fn_agent`, the web replayer and `xtask`.

use serde::{Deserialize, Serialize};

/// A scenario preset's prelude (`crate::scenario::Scenario`): a deterministic autoplay opening,
/// run for `minutes` game minutes before anyone plays. Carried on [`RunLogHeader`] so `--watch`,
/// `--resume` and `fn_agent keys` can rebuild the same start a live `Host::with_prelude` ran.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prelude {
    pub opening: PreludeOpening,
    /// Game minutes the opening runs for, before turn 0.
    pub minutes: f32,
}

/// Which `fn_sim::autoplay` opening a [`Prelude`] runs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreludeOpening {
    /// `fn_sim::autoplay::Economy`: plays toward the Harmony Gate (research, stone and ore,
    /// buildings, farms, bending) -- the only kind a bundled `data/scenarios.toml` preset uses
    /// today.
    Economy,
    /// `Economy` with its research branch-first (`Economy::with_branch_first`, as `balance
    /// --branch-first` measures): the nation's own branch before the shared tier, so a nation's
    /// later abilities arrive in minutes instead of hours.
    EconomyBranchFirst,
    /// The plainer `fn_sim::autoplay::Opening`: beds, a campfire and a short research queue.
    Opening,
}

/// Enough of how a `Host` session started to rebuild it: `Host::new`'s own arguments, plus
/// how many game seconds (and ticks) one `end_turn` turn covers, and what the run was made
/// against. The first line of a run log (docs/design/agent-play.md, "Racing": "Run log"), read
/// by `fn_tui --watch` (#319) and `fn_agent keys`/`verify` (#317/#318) to build the same session
/// a live `Host` started from. `data_digest` and `crate_version` are not needed to rebuild the
/// session (a fresh one always uses whatever data and code are at hand), only to tell a reader
/// whether it still can: `--watch` refuses a log whose `data_digest` doesn't match
/// (`Watcher::new`). That is a run-log rule only: a save file maps its ids by name and loads
/// under changed data (#602), but a keystroke replay only holds for the same id order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunLogHeader<N = String> {
    pub seed: u64,
    /// `fn_sim::Nation` in `fn_tui` (its `snake_case` word in the file), a `String` elsewhere.
    pub nation: N,
    pub turn_seconds: u32,
    /// How many ticks one `end_turn` turn runs (`turn_seconds * TICKS_PER_SECOND`): what turns a
    /// `Turn` line's `index` back into a `<tick N>` a key script would use (`fn_agent keys`,
    /// docs/design/agent-play.md, "Racing"), without a reader having to know `TICKS_PER_SECOND`
    /// itself.
    pub ticks_per_turn: u32,
    pub map_size: Option<String>,
    pub difficulty: Option<String>,
    pub site: Option<(i32, i32)>,
    pub rough: bool,
    /// `Defs::id_hash`: a stable hash of the bundled data's id tables this run was made against
    /// (`Host::data_digest`'s own doc comment). A replay only holds for the same data.
    pub data_digest: u64,
    /// `env!("CARGO_PKG_VERSION")` at the time this run was made: the other half of "replays only
    /// hold for the same code and data".
    pub crate_version: String,
    /// The scenario prelude this run started with, if any (`Host::with_prelude`, #481,
    /// docs/design/agent-scripting.md, "Scenario starts"): `--watch`, `--resume` and `fn_agent
    /// keys` run it again, before replaying anything else, so turn 0 starts from the same state
    /// a live `Host::from_scenario` reached. `#[serde(default)]` so a run log written before
    /// #481 still parses.
    #[serde(default)]
    pub prelude: Option<Prelude>,
    /// The build (`crate::buildid`, #566) of the binary that began this run: a git commit with a
    /// dirty mark. The same keys can reach a different world under another sim, so a reader that
    /// scores the log says when its own build isn't this one. `None` in a log written before
    /// #566, which still reads back; left out of the line when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    /// Whether this run is a playtest (#576, `agent-run --playtest`): the agent was asked to say
    /// what it thinks of the game, not to play for a score, so the watcher and the run browser
    /// label it and don't set its score beside the scored runs'. Left out of the line when
    /// false, so every other run's header is byte for byte what it was; absent reads as false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub playtest: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}
/// One line of a run log file, after the first ([`RunLogHeader`]'s own line): the JSON shape
/// `fn_agent serve` (#317, #324) writes, one object a line, flushed as it goes, and `fn_tui
/// --watch` (#319) reads back -- the one schema both sides use, so they can't drift apart.
/// `keys` is the pressed keys as a `--keys` script (`key_token`'s syntax, parsed back by
/// `crate::keys::parse`), not the raw `KeyEvent`s, since those have no `serde` of their own
/// and a script is what a human watching -- or re-running `fn_agent keys` -- wants to read
/// anyway. `End` (#327) is the line a session of a run ends with: `fn_agent serve` writes one
/// when its session ends (stdin closed, a signal, or an error), with the world's own tick at that
/// moment and the reason, so `fn_tui --watch` can tell a run that's finished from one that's
/// merely caught up with a live agent still thinking. A log is append-only (#511) and
/// `serve --resume` adds to it after the last session's `End`, so an `End` means "the run is
/// over" only while nothing but a `Score` comes after it ([`run_end_reason`]); with more behind
/// it, it is ignored like the commentary lines.
///
/// `Script`, `Wake`, `Note` and `Call` (#479, #497, docs/design/agent-scripting.md, "Memory
/// across sessions"; docs/design/agent-studio.md, "A new log line: `call`") are `fn_agent`'s own
/// commentary on an MCP `tools/call`: nothing a `Host` ever makes itself (`fn_agent::tools::Tools`
/// writes them directly, around the `Keys`/`Turn` lines a script's own `act`/`keys`/`wait` calls
/// produce), and nothing `Host::resume` or `crate::watch::to_keys_script` replay: a chunked
/// `--resume` session reads `Script`, `Wake` and `Note` back to rebuild its script library,
/// `memory` and lab notes (`fn_agent::tools::Tools::resume`), but the game itself is exactly the
/// same seed, keys and turns with or without any of these four.
/// - `Script { name, version, code, turns, wake_on }`: a `run_script` call's own start -- `name`
///   and `code` are the tag and source a `run_script {code}` ad hoc call used (`version == 0`,
///   never saved) or a saved script's own name and code (`version >= 1`, `run_script {name}`).
/// - `Wake { reason, why, turn, memory }`: that same call's own end -- `reason`/`why` as
///   `fn_agent::script::Outcome`'s own fields, `turn` the world's turn count when it stopped,
///   and `memory` the resulting `memory` map (always present, even unchanged). Always the next
///   `Wake` after a `Script` line, since `run_script` calls never overlap.
/// - `Note { text }`: one line appended by `notes {append}`.
/// - `Call { tool, summary }`: written by `fn_agent::tools::Tools::call` just before it dispatches
///   `tool`'s own request, so it always comes ahead of that same call's own `Keys`/`Look`/
///   `Inspect`/`Script`/`Wake`/`Note` lines -- a live watcher sees "started" the moment a call
///   lands, not only once its result (and a `run_script`'s own lines) are in. `summary` is built
///   from the arguments alone (never a result, and never `run_script`'s own code), capped at 60
///   characters; empty for a tool whose arguments say nothing worth summarizing (`observe`,
///   `briefing`, `inspect`, `save_script`, `scripts`, `sdk_reference`).
/// - `Score { score, progress, care, horizon, attempt?, champion?, build? }`: written by `fn_agent score
///   --append` (#525) after a run, the scorecard's top block; ignored like the other commentary.
/// - `Say { text }`: written by `agent-run --playtest` (#576), not by `fn_agent`: a remark the
///   model made in plain text; the watcher's feed shows it, everything else ignores it.
/// - `Think { text }`: written by `agent-run` (#576): a summary of the model's thinking, shown in
///   the watcher's feed as "thinks:" and ignored by everything else.
/// - `Plan { plan }`: written by `fn_agent`'s `plan {set?, merge?}` tool after each change (#524,
///   docs/design/agent-hillclimb.md, "Within a run: a governor and a plan"): `plan` is the whole
///   plan map after the change. A `--resume` session restores the latest one; replay, `verify` and
///   `--watch` ignore it, like the other commentary lines.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogLine<N = String, E = String> {
    Header(RunLogHeader<N>),
    Keys {
        keys: String,
        label: Option<String>,
        note: Option<String>,
    },
    Look {
        x: i32,
        y: i32,
        w: u16,
        h: u16,
    },
    Inspect {
        x: i32,
        y: i32,
    },
    Turn {
        index: u32,
        tick: u64,
    },
    End {
        tick: u64,
        reason: String,
    },
    Script {
        name: String,
        version: u32,
        code: String,
        turns: u32,
        wake_on: Vec<E>,
    },
    Wake {
        reason: String,
        why: Option<String>,
        turn: u32,
        memory: serde_json::Value,
    },
    Note {
        text: String,
    },
    Call {
        tool: String,
        summary: String,
    },
    Plan {
        plan: serde_json::Value,
    },
    /// `fn_agent score --append` (#525): the run's score at its horizon, a commentary line like
    /// the others (replay, `verify`, `keys`, `--resume` and `--watch` all ignore it, so the final
    /// digest is unchanged). `attempt` and `champion` are the campaign runner's to fill in.
    Score {
        score: f64,
        progress: f64,
        care: f64,
        horizon: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attempt: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        champion: Option<bool>,
        /// The build (`crate::buildid`, #566) of the binary that computed this score: the same
        /// log can score differently on another. `None` in a line from before #566.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        build: Option<String>,
    },
    /// What the playing model said in plain text between its tool calls (#576,
    /// `agent-run --playtest`): `agent-run` appends it as the text streams in, so it sits in the
    /// log among the calls it was said around. It is the model's stated account, not its
    /// reasoning, and a commentary line like the others: replay, `verify`, scoring, `keys` and
    /// `--resume` ignore it, so the world and the score are what they are without it.
    Say {
        text: String,
    },
    /// A summary of the playing model's thinking (#576): `agent-run` asks `claude` for
    /// `--thinking-display summarized` and appends each non-empty thinking block as it streams in.
    /// It is a summary of the model's reasoning, never the raw chain of thought, and a commentary
    /// line like `Say`: replay, `verify`, scoring, `keys` and `--resume` ignore it.
    Think {
        text: String,
    },
}

/// What a `test_governor` call's `summary` starts with when it repeated an earlier test of this
/// attempt (same governor, plan, map and turns, #639): it got the earlier verdict back and did
/// not spend a test. Written by `fn_agent`, read by `agent-run` to tell a repeat from work.
pub const REPEAT_TEST_PREFIX: &str = "repeat ";

impl<N, E> LogLine<N, E> {
    /// Whether this is a `test_governor` call that ran a test (#639): not a repeat of an earlier
    /// one. Only these spend one of the attempt's tests and count as work.
    pub fn is_fresh_test(&self) -> bool {
        matches!(self, LogLine::Call { tool, summary }
            if tool == "test_governor" && !summary.starts_with(REPEAT_TEST_PREFIX))
    }

    /// Whether this line is commentary written after a run is over (`fn_agent score --append`'s
    /// `Score`), which doesn't make a run's `End` before it any less its last.
    pub fn follows_the_end(&self) -> bool {
        matches!(self, LogLine::Score { .. })
    }
}

/// The reason of `lines`' `End` line, if it is the one that ended the run (#511): the last line
/// that isn't a `Score` appended afterwards. A run log is append-only, so a resumed session
/// writes more after the `End` the last one left; such a mid-log `End` only says a session
/// stopped, and is not the run's end.
pub fn run_end_reason<N, E>(lines: &[LogLine<N, E>]) -> Option<&str> {
    match lines.iter().rfind(|l| !l.follows_the_end()) {
        Some(LogLine::End { reason, .. }) => Some(reason),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = r#"{"type":"header","seed":7,"nation":"earth","turn_seconds":30,"ticks_per_turn":150,"map_size":null,"difficulty":null,"site":null,"rough":false,"data_digest":1,"crate_version":"0.1.0"}"#;

    #[test]
    fn a_header_from_before_the_optional_fields_reads_and_writes_back_the_same() {
        let line: LogLine = serde_json::from_str(HEADER).unwrap();
        let LogLine::Header(h) = &line else { panic!("{line:?}") };
        assert_eq!(
            (h.seed, h.nation.as_str(), h.prelude, h.build.as_deref()),
            (7, "earth", None, None)
        );
        assert!(!h.playtest);
        // `build` and `playtest` are left out when absent, so the line is byte for byte as before.
        let back = serde_json::to_string(&line).unwrap();
        assert!(!back.contains("build") && !back.contains("playtest"), "{back}");
    }

    #[test]
    fn lines_read_with_the_fields_a_reader_does_not_use() {
        let wake = r#"{"type":"wake","reason":"turns","why":null,"turn":3,"memory":{}}"#;
        assert!(matches!(serde_json::from_str::<LogLine>(wake), Ok(LogLine::Wake { turn: 3, .. })));
        let script =
            r#"{"type":"script","name":"g","version":2,"code":"x","turns":5,"wake_on":["death"]}"#;
        let LogLine::Script { wake_on, .. } = serde_json::from_str::<LogLine>(script).unwrap()
        else {
            panic!()
        };
        assert_eq!(wake_on, vec!["death".to_owned()]);
        let score = r#"{"type":"score","score":1.5,"progress":1.0,"care":1.0,"horizon":9}"#;
        assert!(serde_json::from_str::<LogLine>(score).unwrap().follows_the_end());
    }

    #[test]
    fn the_run_ends_at_an_end_line_only_if_nothing_but_a_score_follows() {
        let end: LogLine = LogLine::End { tick: 5, reason: "done".into() };
        let score: LogLine = LogLine::Score {
            score: 1.0,
            progress: 1.0,
            care: 1.0,
            horizon: 1,
            attempt: None,
            champion: None,
            build: None,
        };
        let note: LogLine = LogLine::Note { text: "x".into() };
        assert_eq!(run_end_reason(&[note.clone(), end.clone(), score]), Some("done"));
        assert_eq!(run_end_reason(&[end, note]), None);
    }
}

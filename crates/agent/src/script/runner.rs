//! [`run`]: the one entry point `run_script` calls (docs/design/agent-scripting.md,
//! "`run_script`", "The wake report"). Builds a fresh, sandboxed `rhai::Engine` over `host` for
//! exactly one call, runs `code`, and reports the wake report: why it stopped, the turns it ran,
//! a diff of the world since it started, act counts and the final `observe`.

use std::collections::BTreeMap;

use fn_tui::surface::{Event, EventKind, Host, Observation};
use rhai::{Dynamic, Engine, EvalAltResult, Scope};
use serde::Serialize;

use super::bindings::{self, Context, Signal};
use super::limits::Limits;

/// What [`run`] returns: the wake report (docs/design/agent-scripting.md, "The wake report").
#[derive(Clone, Debug, Serialize)]
pub struct Outcome {
    /// `"done"` (the script returned), `"error"` (a parse or runtime error), `"asked"`
    /// (the script called `wake(msg)`), `"event"` (a `wake_on` event landed during a `wait`),
    /// `"budget"` (`turns` ran out) or `"runaway"` (too many operations with no `wait`).
    pub reason: &'static str,
    /// Why `reason` is what it is, in one line: the error's own text (`"error"`), the message
    /// `wake(msg)` was given (`"asked"`), or the event's own kind and text (`"event"`,
    /// "death: Ping died of thirst"). `None` for `"done"`, `"budget"` and `"runaway"`, which
    /// need no further explaining than `reason` itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// This run's own tag (`run_script`'s `note`, or `"script"` with none given) -- #477's
    /// stand-in for the saved script identity and version #479 adds.
    pub script: String,
    /// The script's own return value (`"done"` only, and only when it isn't `()`), as JSON
    /// through `rhai::serde`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// `"line:col"` of the error, the `wake(msg)` call, or the `wait` an event landed in, when
    /// Rhai could place it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    /// How many `wait`-driven turns actually ran this call, whichever way it stopped.
    pub turns_run: u32,
    /// The clock's own span this run covered: `"start -> end"`, the same text `Observation`'s
    /// own `clock` field already shows.
    pub clock: String,
    /// What changed between the first and the last `observe` this run took: nothing an
    /// observation doesn't already say (docs/design/agent-scripting.md, "The wake report").
    pub since_start: SinceStart,
    /// Every `act`/`keys` call this run made, ok or failed, and the last failure's own text.
    pub acts: ActsSummary,
    /// The script's own `log`/`print`/`debug` lines, oldest first, trimmed to
    /// `Limits::log_tail`.
    pub log: Vec<String>,
    pub observe: Observation,
}

/// [`Outcome::since_start`]: a diff between the `observe` taken right before this run's own
/// code started and the one taken right after it stopped, so it contains nothing an observation
/// doesn't (docs/design/agent-scripting.md, "The wake report").
#[derive(Clone, Debug, Serialize)]
pub struct SinceStart {
    /// `"6 -> 7"`: the people total before and after.
    pub people: String,
    /// Every store whose count changed, signed (`"+38"`, `"-12"`); a store that didn't move
    /// isn't listed.
    pub stores: BTreeMap<String, String>,
    /// How many events of each [`EventKind`] (`EventKind::as_str`) this run's own `events()`
    /// carried, over the whole run, not just since the last `observe`.
    pub events: BTreeMap<String, usize>,
    /// Those events' own text, deduplicated, oldest first.
    pub texts: Vec<String>,
}

/// [`Outcome::acts`]: every `act`/`keys` call this run made, tallied by
/// `crate::script::bindings::Context::record`.
#[derive(Clone, Debug, Serialize)]
pub struct ActsSummary {
    pub ok: u32,
    pub failed: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<String>,
}

/// Runs `code` against `host` for at most `limits.resolve_turns(turns)` turns, waking early on
/// any of `wake_on`'s own [`EventKind`]s (docs/design/agent-scripting.md, "`run_script`"). Every
/// `act`/`keys` step the script makes is tagged `[tag]` in the run log (`note`, or `"script"`
/// with none given -- #477 has no saved script identity yet; #479 gives every saved script a
/// real name and version to tag with instead). Never touches `host` beyond what `code` itself
/// does through the plumbing ([`bindings::register`]): every action is a normal
/// `Host::act`/`Host::press_keys` call, logged exactly as a human's or an MCP agent's would be.
pub fn run(
    host: &mut Host,
    code: &str,
    turns: Option<u32>,
    note: Option<&str>,
    wake_on: &[EventKind],
    limits: &Limits,
) -> Outcome {
    let max_turns = limits.resolve_turns(turns);
    let tag = note.filter(|n| !n.is_empty()).unwrap_or("script").to_owned();
    let before = host.observe();
    let ctx = Context::new(host, tag, max_turns, wake_on.to_vec());

    let result = {
        let mut engine = Engine::new();
        engine.set_max_operations(limits.max_operations);

        let print_ctx = ctx.clone();
        engine.on_print(move |s| print_ctx.push_log(s.to_owned()));
        let debug_ctx = ctx.clone();
        engine.on_debug(move |s, src, pos| debug_ctx.push_log(debug_line(s, src, pos)));

        bindings::register(&mut engine, &ctx);

        let mut scope = Scope::new();
        engine.eval_with_scope::<Dynamic>(&mut scope, code)
        // `engine` and `scope` -- and every clone of `ctx` they hold -- drop here, before
        // `Context::host` could be called again from outside a script (docs/design/
        // agent-scripting.md's own soundness note on `bindings::Context`).
    };

    let turns_run = ctx.turns_used();
    let log = tail(ctx.log_lines(), limits.log_tail);
    let events = ctx.events_since_start();
    let acts = ActsSummary {
        ok: ctx.acts_ok(),
        failed: ctx.acts_failed(),
        last_failure: ctx.last_failure(),
    };
    let script = ctx.tag().to_owned();
    let observe = host.observe();
    let since_start = since_start(&before, &observe, &events);
    let clock = format!("{} -> {}", before.clock, observe.clock);

    let (reason, why, value, line) = match result {
        Ok(value) => ("done", None, dynamic_to_json(value), None),
        Err(err) => {
            let (reason, why, line) = classify_error(&err);
            (reason, why, None, line)
        }
    };

    Outcome { reason, why, script, value, line, turns_run, clock, since_start, acts, log, observe }
}

/// [`SinceStart`] from the two observations this run bracketed and the events it carried
/// (`Context::events_since_start`): nothing either observation doesn't already say.
fn since_start(before: &Observation, after: &Observation, events: &[Event]) -> SinceStart {
    let people = format!("{} -> {}", before.people.total, after.people.total);
    let mut stores = BTreeMap::new();
    for store in &after.stores {
        let had = before.stores.iter().find(|s| s.name == store.name).map_or(0, |s| s.count);
        let delta = i64::from(store.count) - i64::from(had);
        if delta != 0 {
            stores.insert(store.name.clone(), format!("{delta:+}"));
        }
    }
    let mut counts = BTreeMap::new();
    let mut texts = Vec::new();
    for ev in events {
        *counts.entry(ev.kind.as_str().to_owned()).or_insert(0usize) += 1;
        if !texts.contains(&ev.text) {
            texts.push(ev.text.clone());
        }
    }
    SinceStart { people, stores, events: counts, texts }
}

/// Why `err` stopped the script: a [`Signal`] from `bindings::read_signal` (`"budget"`,
/// `"asked"` or `"event"`), Rhai's own `ErrorTooManyOperations` (`"runaway"`, from
/// `Engine::set_max_operations`), or anything else (`"error"`, with its own message and
/// `"line:col"`).
fn classify_error(err: &EvalAltResult) -> (&'static str, Option<String>, Option<String>) {
    let root = err.unwrap_inner();
    if let EvalAltResult::ErrorTerminated(token, _) = root
        && let Some(signal) = bindings::read_signal(token)
    {
        return match signal {
            Signal::Budget => ("budget", None, None),
            Signal::Wake { msg } => ("asked", Some(msg), position_of(err)),
            Signal::Event { kind, text, .. } => {
                ("event", Some(format!("{}: {text}", kind.as_str())), position_of(err))
            }
        };
    }
    if matches!(root, EvalAltResult::ErrorTooManyOperations(_)) {
        return ("runaway", Some("a loop with no wait?".to_owned()), None);
    }
    ("error", Some(err.to_string()), position_of(err))
}

/// `"line:col"` of `err`, when Rhai could place it (`None` for a position-less error, like
/// Rhai's own `ErrorTooManyOperations`).
fn position_of(err: &EvalAltResult) -> Option<String> {
    let pos = err.position();
    pos.line().map(|l| format!("{l}:{}", pos.position().unwrap_or(0)))
}

fn dynamic_to_json(value: Dynamic) -> Option<serde_json::Value> {
    if value.is_unit() {
        return None;
    }
    rhai::serde::from_dynamic::<serde_json::Value>(&value).ok()
}

/// One `debug(...)` call's own log line: the source (when the engine has one; `run_script`'s
/// inline code never does) and the position, so a script debugging a loop can tell its lines
/// apart.
fn debug_line(text: &str, source: Option<&str>, pos: rhai::Position) -> String {
    let src = source.map(|s| format!("{s}: ")).unwrap_or_default();
    match pos.line() {
        Some(line) => format!("{src}line {line}: {text}"),
        None => format!("{src}{text}"),
    }
}

/// The last `n` of `lines`, oldest first (`Limits::log_tail`).
fn tail(mut lines: Vec<String>, n: usize) -> Vec<String> {
    if lines.len() > n {
        lines = lines.split_off(lines.len() - n);
    }
    lines
}

#[cfg(test)]
mod tests {
    use fn_tui::surface::LogLine;

    use super::*;

    fn limits() -> Limits {
        Limits {
            default_turns: 15,
            max_turns: 180,
            max_operations: 50_000,
            log_tail: 30,
            default_wake: vec![EventKind::Death, EventKind::Need, EventKind::Endeavor],
        }
    }

    fn host() -> Host {
        let nation = fn_tui::cli::parse_nation("earth").unwrap();
        Host::new(7, nation, None, None, None, false).unwrap().with_turn_seconds(10)
    }

    /// `run` with no `wake_on` given (`&[]`): the common case for a test that isn't checking
    /// wake-on-event behaviour itself.
    fn run_default(host: &mut Host, code: &str, turns: Option<u32>, note: Option<&str>) -> Outcome {
        run(host, code, turns, note, &[], &limits())
    }

    /// A script that returns a value stops `"done"`, with that value as JSON and the
    /// `observe()`/`log()` calls it made along the way.
    #[test]
    fn a_returning_script_is_done_with_its_own_value() {
        let mut h = host();
        let out = run_default(&mut h, r#"log("hi"); 1 + 41"#, None, None);
        assert_eq!(out.reason, "done");
        assert_eq!(out.value, Some(serde_json::json!(42)));
        assert_eq!(out.log, vec!["hi".to_owned()]);
        assert_eq!(out.turns_run, 0);
        assert_eq!(out.script, "script", "no note given");
    }

    /// A runtime error stops `"error"`, with the message and a `"line:col"` Rhai could place.
    #[test]
    fn a_runtime_error_is_error_with_a_message_and_a_line() {
        let mut h = host();
        let out = run_default(&mut h, "let x = 1 / 0;", None, None);
        assert_eq!(out.reason, "error");
        assert!(out.why.unwrap().contains("Division by zero"));
        assert_eq!(out.line.unwrap(), "1:11");
    }

    /// A parse error stops `"error"` too (`Engine::eval_with_scope` folds a bad parse into the
    /// same `Result` a runtime error takes).
    #[test]
    fn a_parse_error_is_error_too() {
        let mut h = host();
        let out = run_default(&mut h, "let x = ;", None, None);
        assert_eq!(out.reason, "error");
        assert!(out.why.as_deref().unwrap().contains("Syntax error"), "{:?}", out.why);
    }

    /// A loop that only `wait`s stops `"budget"` once `turns` runs out mid-`wait`, having
    /// actually run every turn that fit.
    #[test]
    fn a_turn_budget_stops_the_script_budget() {
        let mut h = host();
        let out = run_default(&mut h, "loop { wait(1); }", Some(3), None);
        assert_eq!(out.reason, "budget");
        assert_eq!(out.turns_run, 3);
        assert_eq!(out.observe.turn, 3);
    }

    /// A `wait` that asks for more turns than are left still runs whatever fits, then stops the
    /// whole script `"budget"` rather than quietly returning early.
    #[test]
    fn a_wait_past_the_budget_runs_what_fits_then_stops_budget() {
        let mut h = host();
        let out = run_default(&mut h, "wait(100);", Some(5), None);
        assert_eq!(out.reason, "budget");
        assert_eq!(out.turns_run, 5);
    }

    /// A loop with no `wait` runs away: `Engine::set_max_operations` stops it deterministically
    /// before it could ever use a turn.
    #[test]
    fn a_loop_with_no_wait_is_runaway() {
        let mut h = host();
        let out = run_default(&mut h, "let x = 0; loop { x += 1; }", None, None);
        assert_eq!(out.reason, "runaway");
        assert_eq!(out.turns_run, 0);
    }

    /// The sandbox (docs/design/agent-scripting.md, "The SDK", "Language: Rhai, embedded"): no
    /// RNG is registered, no module resolver exists (`import` doesn't even parse), and
    /// `no_time` removes `timestamp()` along with every other clock function -- and no file
    /// function was ever registered, so one by any name is simply unknown.
    #[test]
    fn timestamp_import_and_a_file_function_all_fail_to_compile_or_run() {
        for code in [
            "let t = timestamp();",
            "import \"docs/design/agent-scripting.md\" as doc;",
            "let t = read_file(\"Cargo.toml\");",
        ] {
            let mut h = host();
            let out = run_default(&mut h, code, None, None);
            assert_eq!(out.reason, "error", "{code:?} should fail: {out:?}");
        }
    }

    /// Re-running the same script from the same state gives the same keys
    /// (docs/design/agent-scripting.md's own determinism claim, #477's own acceptance
    /// criteria): two fresh `Host`s, the same script, the same resulting `RunLog`.
    #[test]
    fn the_same_script_from_the_same_state_presses_the_same_keys_both_times() {
        let code = r#"
            act([#{step: "cursor", x: 5, y: 5}], "scout");
            keys("<right>");
            wait(2);
        "#;
        let run_once = || {
            let mut h = host();
            let out = run_default(&mut h, code, None, Some("keep-fed"));
            (out, h.log().lines())
        };
        let (out1, lines1) = run_once();
        let (out2, lines2) = run_once();
        assert_eq!(out1.reason, "done");
        assert_eq!(out1.turns_run, out2.turns_run);
        assert_eq!(lines1, lines2);
    }

    /// Every `act`/`keys` step a script makes is a normal `keys` log line, its own note tagged
    /// with the run's `tag` (`note`, or `"script"` with none given).
    #[test]
    fn every_act_and_keys_step_is_logged_with_a_tagged_note() {
        let mut h = host();
        let out = run_default(
            &mut h,
            r#"act([#{step: "cursor", x: 5, y: 5}], "scout"); keys("<right>");"#,
            None,
            Some("keep-fed"),
        );
        assert_eq!(out.reason, "done");
        let lines = h.log().lines();
        let LogLine::Keys { note: note1, .. } = &lines[0] else { panic!("{lines:?}") };
        let LogLine::Keys { note: note2, .. } = &lines[1] else { panic!("{lines:?}") };
        assert_eq!(note1.as_deref(), Some("[keep-fed] scout"));
        assert_eq!(note2.as_deref(), Some("[keep-fed]"));
    }

    /// With no `note` given, every step's own log line is still tagged, with the generic
    /// `"script"` stand-in for a saved script's real identity (#479).
    #[test]
    fn with_no_note_the_tag_defaults_to_script() {
        let mut h = host();
        run_default(&mut h, r#"keys("<right>");"#, None, None);
        let lines = h.log().lines();
        let LogLine::Keys { note, .. } = &lines[0] else { panic!("{lines:?}") };
        assert_eq!(note.as_deref(), Some("[script]"));
    }

    /// `print` and `debug` land in the script's own log, never on stdout -- the MCP channel
    /// `fn_agent serve` speaks JSON-RPC over.
    #[test]
    fn print_and_debug_land_in_the_log_not_stdout() {
        let mut h = host();
        let out = run_default(&mut h, r#"print("printed"); debug("debugged");"#, None, None);
        assert!(out.log.iter().any(|l| l == "printed"), "{:?}", out.log);
        assert!(out.log.iter().any(|l| l.contains("debugged")), "{:?}", out.log);
    }

    /// `Limits::log_tail` trims the script's own log to its last N lines, oldest first.
    #[test]
    fn the_log_is_trimmed_to_log_tail_most_recent_lines() {
        let mut h = host();
        let tiny = Limits { log_tail: 2, ..limits() };
        let out = run(&mut h, r#"log("a"); log("b"); log("c");"#, None, None, &[], &tiny);
        assert_eq!(out.log, vec!["b".to_owned(), "c".to_owned()]);
    }

    /// `wake(msg)` ends the script right there with reason `"asked"` and that message, however
    /// deep in a loop it was called.
    #[test]
    fn wake_ends_the_script_with_reason_asked_and_the_message() {
        let mut h = host();
        let out = run_default(&mut h, r#"wake("food has stalled");"#, None, None);
        assert_eq!(out.reason, "asked");
        assert_eq!(out.why.as_deref(), Some("food has stalled"));
    }

    /// `wake`'s own termination isn't an ordinary Rhai error: a script's `try`/`catch` around
    /// it never runs its `catch` block, and the run still stops `"asked"` instead of continuing
    /// past it (docs/design/agent-scripting.md, "`run_script`": "The wake can't be caught").
    #[test]
    fn wake_cant_be_caught_by_the_scripts_own_try_catch() {
        let mut h = host();
        let out = run_default(
            &mut h,
            r#"
                try {
                    wake("stuck");
                    log("never reached");
                } catch (err) {
                    log("caught");
                }
                log("never reached either");
            "#,
            None,
            None,
        );
        assert_eq!(out.reason, "asked");
        assert_eq!(out.why.as_deref(), Some("stuck"));
        assert!(!out.log.contains(&"caught".to_owned()), "{:?}", out.log);
        assert!(!out.log.contains(&"never reached".to_owned()), "{:?}", out.log);
    }

    /// A need flag first shown during a `wait` is in `wake_on`'s default set (`death`, `need`,
    /// `endeavor`): the whole script stops `"event"` right there, at the end of that turn, with
    /// `why` naming the kind and the flag's own text.
    #[test]
    fn a_wake_on_event_during_wait_stops_the_script_with_reason_event() {
        let mut h = host().with_turn_seconds(600);
        let out = run(
            &mut h,
            "loop { wait(1); }",
            Some(50),
            None,
            &[EventKind::Need, EventKind::Death, EventKind::Endeavor],
            &limits(),
        );
        assert_eq!(out.reason, "event");
        let why = out.why.expect("an event always says why");
        assert!(why.starts_with("need:"), "{why}");
        assert!(out.turns_run >= 1 && out.turns_run < 50, "{}", out.turns_run);
    }

    /// An event kind that isn't in `wake_on` never stops the script, but still turns up in a
    /// later `events()` call for the script to notice itself -- a `tech` finishing, say, while
    /// `wake_on` only ever asks for `death`.
    #[test]
    fn an_event_outside_wake_on_shows_in_events_but_never_stops_the_script() {
        let mut h = host().with_turn_seconds(600);
        let out = run(
            &mut h,
            r#"wait(3); let seen = events(); log(`events: ${seen.len()}`); seen.len()"#,
            Some(10),
            None,
            &[EventKind::Death],
            &limits(),
        );
        assert_eq!(out.reason, "done", "{out:?}");
        // A need flag or two should have shown over 3 turns of 600 game seconds each; none of
        // them are `death`, so `wake_on` never fires, but `events()` still saw them.
        assert!(out.value.unwrap().as_i64().unwrap() > 0, "events() should have seen something");
    }

    /// The wake report's own shape (docs/design/agent-scripting.md, "The wake report"): every
    /// field the design doc's own example carries, present on a run that wakes on an event.
    #[test]
    fn the_wake_report_has_every_field_the_design_doc_names() {
        let mut h = host().with_turn_seconds(600);
        let out = run(
            &mut h,
            "loop { wait(1); }",
            Some(50),
            Some("keep-fed"),
            &[EventKind::Need],
            &limits(),
        );
        assert_eq!(out.reason, "event");
        assert!(out.why.is_some());
        assert_eq!(out.script, "keep-fed");
        assert!(out.turns_run > 0);
        assert!(out.clock.contains(" -> "), "{}", out.clock);
        assert!(!out.since_start.events.is_empty(), "{:?}", out.since_start);
        assert_eq!(out.acts.ok, 0);
        assert_eq!(out.acts.failed, 0);
        assert!(out.acts.last_failure.is_none());
        assert_eq!(out.observe.turn, out.turns_run);
    }

    /// A 45-turn scripted run on seed 7 Earth stays well under the ~8 KB budget the design doc
    /// sets for a wake report, even once a few things have happened.
    #[test]
    fn a_45_turn_run_on_seed_7_earth_stays_under_8kb() {
        let nation = fn_tui::cli::parse_nation("earth").unwrap();
        let mut h = Host::new(7, nation, None, None, None, false).unwrap();
        let out = run(&mut h, "wait(45);", Some(45), None, &[], &limits());
        let json = serde_json::to_string(&out).unwrap();
        assert!(json.len() < 8192, "wake report was {} bytes", json.len());
    }
}

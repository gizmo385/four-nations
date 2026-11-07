//! [`run`]: the one entry point `run_script` calls (docs/design/agent-scripting.md,
//! "`run_script`"). Builds a fresh, sandboxed `rhai::Engine` over `host` for exactly one call,
//! runs `code`, and reports why it stopped.

use fn_tui::surface::{Host, Observation};
use rhai::{Dynamic, Engine, EvalAltResult, Position, Scope};
use serde::Serialize;

use super::bindings::{self, BUDGET_TOKEN, Context};
use super::limits::Limits;

/// What [`run`] returns: everything `run_script`'s result needs (docs/design/agent-scripting.md,
/// "`run_script`"), short of the wake-rule event reasons #478 adds.
#[derive(Clone, Debug, Serialize)]
pub struct Outcome {
    /// `"done"` (the script returned), `"error"` (a parse or runtime error), `"budget"`
    /// (`turns` ran out) or `"runaway"` (too many operations with no `wait`).
    pub reason: &'static str,
    /// The script's own return value (`"done"` only, and only when it isn't `()`), as JSON
    /// through `rhai::serde`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// The error's own message (`"error"` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// `"line:col"` of the error, when Rhai could place it (`"error"` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    /// How many `wait`-driven turns actually ran this call, whichever way it stopped.
    pub turns_run: u32,
    /// The script's own `log`/`print`/`debug` lines, oldest first, trimmed to
    /// `Limits::log_tail`.
    pub log: Vec<String>,
    pub observe: Observation,
}

/// Runs `code` against `host` for at most `limits.resolve_turns(turns)` turns
/// (docs/design/agent-scripting.md, "`run_script`"). Every `act`/`keys` step the script makes
/// is tagged `[tag]` in the run log (`note`, or `"script"` with none given -- #477 has no saved
/// script identity yet; #479 gives every saved script a real name and version to tag with
/// instead). Never touches `host` beyond what `code` itself does through the plumbing
/// ([`bindings::register`]): every action is a normal `Host::act`/`Host::press_keys` call,
/// logged exactly as a human's or an MCP agent's would be.
pub fn run(
    host: &mut Host,
    code: &str,
    turns: Option<u32>,
    note: Option<&str>,
    limits: &Limits,
) -> Outcome {
    let max_turns = limits.resolve_turns(turns);
    let tag = note.filter(|n| !n.is_empty()).unwrap_or("script").to_owned();
    let ctx = Context::new(host, tag, max_turns);

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
    let observe = host.observe();

    match result {
        Ok(value) => Outcome {
            reason: "done",
            value: dynamic_to_json(value),
            message: None,
            line: None,
            turns_run,
            log,
            observe,
        },
        Err(err) => outcome_from_error(&err, turns_run, log, observe),
    }
}

fn outcome_from_error(
    err: &EvalAltResult,
    turns_run: u32,
    log: Vec<String>,
    observe: Observation,
) -> Outcome {
    let (reason, message, line) = classify_error(err);
    Outcome { reason, value: None, message, line, turns_run, log, observe }
}

/// Why `err` stopped the script: `"budget"` ([`BUDGET_TOKEN`], raised by `bindings::do_wait`),
/// `"runaway"` (Rhai's own `ErrorTooManyOperations`, from `Engine::set_max_operations`), or
/// `"error"` (anything else), with that last case's own message and `"line:col"`.
fn classify_error(err: &EvalAltResult) -> (&'static str, Option<String>, Option<String>) {
    let root = err.unwrap_inner();
    if let EvalAltResult::ErrorTerminated(token, _) = root
        && token.clone().into_string().ok().as_deref() == Some(BUDGET_TOKEN)
    {
        return ("budget", None, None);
    }
    if matches!(root, EvalAltResult::ErrorTooManyOperations(_)) {
        return ("runaway", Some("a loop with no wait?".to_owned()), None);
    }
    let pos = err.position();
    let line = pos.line().map(|l| format!("{l}:{}", pos.position().unwrap_or(0)));
    ("error", Some(err.to_string()), line)
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
fn debug_line(text: &str, source: Option<&str>, pos: Position) -> String {
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
        Limits { default_turns: 15, max_turns: 180, max_operations: 50_000, log_tail: 30 }
    }

    fn host() -> Host {
        let nation = fn_tui::cli::parse_nation("earth").unwrap();
        Host::new(7, nation, None, None, None, false).unwrap().with_turn_seconds(10)
    }

    /// A script that returns a value stops `"done"`, with that value as JSON and the
    /// `observe()`/`log()` calls it made along the way.
    #[test]
    fn a_returning_script_is_done_with_its_own_value() {
        let mut h = host();
        let out = run(&mut h, r#"log("hi"); 1 + 41"#, None, None, &limits());
        assert_eq!(out.reason, "done");
        assert_eq!(out.value, Some(serde_json::json!(42)));
        assert_eq!(out.log, vec!["hi".to_owned()]);
        assert_eq!(out.turns_run, 0);
    }

    /// A runtime error stops `"error"`, with the message and a `"line:col"` Rhai could place.
    #[test]
    fn a_runtime_error_is_error_with_a_message_and_a_line() {
        let mut h = host();
        let out = run(&mut h, "let x = 1 / 0;", None, None, &limits());
        assert_eq!(out.reason, "error");
        assert!(out.message.unwrap().contains("Division by zero"));
        assert_eq!(out.line.unwrap(), "1:11");
    }

    /// A parse error stops `"error"` too (`Engine::eval_with_scope` folds a bad parse into the
    /// same `Result` a runtime error takes).
    #[test]
    fn a_parse_error_is_error_too() {
        let mut h = host();
        let out = run(&mut h, "let x = ;", None, None, &limits());
        assert_eq!(out.reason, "error");
        assert!(out.message.as_deref().unwrap().contains("Syntax error"), "{:?}", out.message);
    }

    /// A loop that only `wait`s stops `"budget"` once `turns` runs out mid-`wait`, having
    /// actually run every turn that fit.
    #[test]
    fn a_turn_budget_stops_the_script_budget() {
        let mut h = host();
        let out = run(&mut h, "loop { wait(1); }", Some(3), None, &limits());
        assert_eq!(out.reason, "budget");
        assert_eq!(out.turns_run, 3);
        assert_eq!(out.observe.turn, 3);
    }

    /// A `wait` that asks for more turns than are left still runs whatever fits, then stops the
    /// whole script `"budget"` rather than quietly returning early.
    #[test]
    fn a_wait_past_the_budget_runs_what_fits_then_stops_budget() {
        let mut h = host();
        let out = run(&mut h, "wait(100);", Some(5), None, &limits());
        assert_eq!(out.reason, "budget");
        assert_eq!(out.turns_run, 5);
    }

    /// A loop with no `wait` runs away: `Engine::set_max_operations` stops it deterministically
    /// before it could ever use a turn.
    #[test]
    fn a_loop_with_no_wait_is_runaway() {
        let mut h = host();
        let out = run(&mut h, "let x = 0; loop { x += 1; }", None, None, &limits());
        assert_eq!(out.reason, "runaway");
        assert_eq!(out.turns_run, 0);
    }

    /// The sandbox (docs/design/agent-scripting.md, "The SDK", "Language: Rhai, embedded"): no
    /// RNG is registered, no module resolver exists (`import` doesn't even parse), and
    /// `no_time` removes `timestamp()` along with every other clock function -- and no file
    /// function was ever registered, so one by any name is simply unknown.
    #[test]
    fn timestamp_import_and_a_file_function_all_fail_to_compile_or_run() {
        let limits = limits();
        for code in [
            "let t = timestamp();",
            "import \"docs/design/agent-scripting.md\" as doc;",
            "let t = read_file(\"Cargo.toml\");",
        ] {
            let mut h = host();
            let out = run(&mut h, code, None, None, &limits);
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
            let out = run(&mut h, code, None, Some("keep-fed"), &limits());
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
        let out = run(
            &mut h,
            r#"act([#{step: "cursor", x: 5, y: 5}], "scout"); keys("<right>");"#,
            None,
            Some("keep-fed"),
            &limits(),
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
        run(&mut h, r#"keys("<right>");"#, None, None, &limits());
        let lines = h.log().lines();
        let LogLine::Keys { note, .. } = &lines[0] else { panic!("{lines:?}") };
        assert_eq!(note.as_deref(), Some("[script]"));
    }

    /// `print` and `debug` land in the script's own log, never on stdout -- the MCP channel
    /// `fn_agent serve` speaks JSON-RPC over.
    #[test]
    fn print_and_debug_land_in_the_log_not_stdout() {
        let mut h = host();
        let out = run(&mut h, r#"print("printed"); debug("debugged");"#, None, None, &limits());
        assert!(out.log.iter().any(|l| l == "printed"), "{:?}", out.log);
        assert!(out.log.iter().any(|l| l.contains("debugged")), "{:?}", out.log);
    }

    /// `Limits::log_tail` trims the script's own log to its last N lines, oldest first.
    #[test]
    fn the_log_is_trimmed_to_log_tail_most_recent_lines() {
        let mut h = host();
        let tiny = Limits { log_tail: 2, ..limits() };
        let out = run(&mut h, r#"log("a"); log("b"); log("c");"#, None, None, &tiny);
        assert_eq!(out.log, vec!["b".to_owned(), "c".to_owned()]);
    }
}

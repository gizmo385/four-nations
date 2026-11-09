//! [`run`]: the one entry point `run_script` calls (docs/design/agent-scripting.md,
//! "`run_script`", "The wake report"). Builds a fresh, sandboxed `rhai::Engine` over `host` for
//! exactly one call, merges `code` with the porcelain prelude (`super::prelude::SOURCE`, #480)
//! and runs it, and reports the wake report: why it stopped, the turns it ran, a diff of the
//! world since it started, act counts and the final `observe`.

use std::collections::BTreeMap;

use fn_tui::surface::{Event, EventKind, Host, Observation};
use rhai::{Dynamic, EvalAltResult, Scope};
use serde::Serialize;

use super::bindings::{self, Context, Signal};
use super::limits::Limits;

/// What [`run`] returns: the wake report (docs/design/agent-scripting.md, "The wake report").
#[derive(Clone, Debug, Serialize)]
pub struct Outcome {
    /// `"done"` (the script returned), `"error"` (a parse or runtime error), `"asked"`
    /// (the script called `wake(msg)`), `"event"` (a `wake_on` event landed during a `wait`),
    /// `"budget"` (`turns` ran out) or `"runaway"` (over the operation budget since the last
    /// `wait`, #564).
    pub reason: &'static str,
    /// Why `reason` is what it is, in one line: the error's own text (`"error"`), the message
    /// `wake(msg)` was given (`"asked"`), or the event's own kind and text (`"event"`,
    /// "death: Ping died of thirst"), or for a `"runaway"` how much work it did since its last
    /// `wait` and the line it stopped at. `None` for `"done"` and `"budget"`, which need no
    /// further explaining than `reason` itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// This run's own tag: the caller's `note` for an ad hoc `run_script {code}` call, or the
    /// saved script's own name for `run_script {name}` (#479); `"script"` with neither given.
    pub script: String,
    /// The saved script's own version being run (#479, `run_script {name}`), omitted for an ad
    /// hoc `run_script {code}` call, which was never `save_script`d and so has none.
    #[serde(skip_serializing_if = "is_zero")]
    pub version: u32,
    /// The script's own return value (`"done"` only, and only when it isn't `()`), as JSON
    /// through `rhai::serde`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// `"line:col"` of the error, the `wake(msg)` call, the `wait` an event landed in, or where a
    /// runaway stopped, when
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
    /// The `memory` map this run leaves behind (#479, docs/design/agent-scripting.md, "The
    /// SDK"): the caller's own `memory` as the script ended it, unless that write would exceed
    /// `Limits::memory_bytes` -- then the caller's own `memory` unchanged, and `reason`/`why`
    /// are overridden to `"error"` naming the cap (`finalize_memory`), whatever the script's
    /// own code did. Always a JSON object, never `null`.
    pub memory: serde_json::Value,
    /// Each failed `act`/`keys` with the turn (into this run) it was made on, oldest first, kept
    /// for `test_governor`'s trace (#570) and not part of the tool result.
    #[serde(skip)]
    pub failures: Vec<ActFailure>,
}

/// One failed `act`/`keys` call: [`ActsSummary::last_failure`]'s text and when it happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActFailure {
    /// Turns into the run when it was made (0: before the first `wait`).
    pub turn: u32,
    pub message: String,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
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

/// [`run`]'s own arguments, bundled so the function itself takes a readable few (`host` and
/// `limits` stay their own parameters; everything specific to *this* call lives here): `code`
/// to run, `turns` and `wake_on` as `run_script`'s own (optional) arguments, `note`/`version`
/// as the run's own tag (plain for an ad hoc `run_script {code}` call, `0` version; the saved
/// script's own name and version for `run_script {name}`, #479), and `memory` as this session's
/// own persisted map going in.
pub struct Request<'a> {
    pub code: &'a str,
    pub turns: Option<u32>,
    pub note: Option<&'a str>,
    pub version: u32,
    pub wake_on: &'a [EventKind],
    pub memory: &'a serde_json::Value,
    /// The session's `plan` map (#524, docs/design/agent-hillclimb.md, "Within a run: a governor
    /// and a plan"), as of this call: what the script's read-only `plan()` returns. Not an
    /// output -- only the `plan` tool changes it, between runs.
    pub plan: &'a serde_json::Value,
    /// A probe (#527, decision #522): an inline `run_script {code}`, which can read (`observe`,
    /// `look`, `plan()`, ...) but whose `act`, `keys` and `wait` (and so every porcelain helper
    /// that acts or waits) fail with "probes can't act: put this in the governor, or in the
    /// plan". `false` for a saved script run by name and for `fn_agent play`.
    pub probe: bool,
}

/// Runs `request.code` against `host` for at most `limits.resolve_turns(request.turns)` turns,
/// waking early on any of `request.wake_on`'s own [`EventKind`]s (docs/design/
/// agent-scripting.md, "`run_script`"). Every `act`/`keys` step the script makes is tagged
/// `[tag]` (or `[tag vN]` once `request.version` is nonzero, #479's own saved-script identity)
/// in the run log (`request.note`, or `"script"` with none given). [`Outcome::memory`] is
/// `request.memory` as the script leaves it, capped by `Limits::memory_bytes`
/// (`finalize_memory`). Never touches `host` beyond what the code itself does through the
/// plumbing ([`bindings::register`]): every action is a normal `Host::act`/`Host::press_keys`
/// call, logged exactly as a human's or an MCP agent's would be.
pub fn run(host: &mut Host, request: Request<'_>, limits: &Limits) -> Outcome {
    let Request { code, turns, note, version, wake_on, memory, plan, probe } = request;
    let max_turns = limits.resolve_turns(turns);
    let tag = note.filter(|n| !n.is_empty()).unwrap_or("script").to_owned();
    let before = host.observe();
    let ctx = Context::new(host, tag, version, max_turns, wake_on.to_vec(), plan.clone(), probe);

    let (result, memory_out) = {
        // `prelude::engine()` raises the per-function expression depth above Rhai's debug-build
        // default (16 levels, #480): the porcelain layer's own nested loops and string
        // interpolation need more than that, well short of anything resembling a runaway
        // script -- the operation budget below is what actually guards against those.
        let mut engine = super::prelude::engine();
        // The operation budget counts since the last `wait`, not over the whole run (#564):
        // Rhai's own `set_max_operations` can't be reset, so `Engine::on_progress` does it
        // (`Context::check_work`) and `do_wait` starts the count over.
        ctx.set_work_limit(limits.max_operations);
        let progress_ctx = ctx.clone();
        engine.on_progress(move |ops| progress_ctx.check_work(ops));

        let print_ctx = ctx.clone();
        engine.on_print(move |s| print_ctx.push_log(s.to_owned()));
        let debug_ctx = ctx.clone();
        engine.on_debug(move |s, src, pos| debug_ctx.push_log(debug_line(s, src, pos)));

        bindings::register(&mut engine, &ctx);

        let mut scope = Scope::new();
        let initial_memory =
            rhai::serde::to_dynamic(memory).unwrap_or_else(|_| Dynamic::from_map(rhai::Map::new()));
        scope.push_dynamic("memory", initial_memory);
        // The porcelain layer (#480): compiled separately from `code` and merged in, rather
        // than textually prepended, so a parse or runtime error in the caller's own code keeps
        // the caller's own line:col (each AST's positions were computed against its own source,
        // starting at line 1, before the merge) and `prelude.rhai` need not parse again for
        // every script's own mistakes. `AST::merge` appends `code`'s own top-level statements
        // after `prelude.rhai`'s own (none: it is nothing but function definitions), so the
        // merged program's return value is exactly `code`'s own, as if the prelude had never
        // been there.
        let prelude_ast = engine
            .compile(super::prelude::SOURCE)
            .expect("bundled prelude.rhai must compile (script::prelude::tests)");
        let result =
            engine.compile(code).map_err(Box::<EvalAltResult>::from).and_then(|user_ast| {
                let ast = prelude_ast.merge(&user_ast);
                engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast)
            });
        let memory_out = scope.get_mut("memory").map_or(Dynamic::UNIT, |d| d.clone());
        (result, memory_out)
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
    let failures: Vec<ActFailure> =
        ctx.failures().into_iter().map(|(turn, message)| ActFailure { turn, message }).collect();
    let script = ctx.tag().to_owned();
    let version = ctx.version();
    let observe = host.observe();
    let since_start = since_start(&before, &observe, &events);
    let clock = format!("{} -> {}", before.clock, observe.clock);

    let (mut reason, mut why, mut value, line) = match result {
        Ok(value) => ("done", None, dynamic_to_json(value), None),
        Err(err) => {
            let (reason, why, line) = classify_error(&err, code);
            (reason, why, None, line)
        }
    };

    let (memory, memory_error) = finalize_memory(memory_out, memory, limits.memory_bytes);
    if let Some(msg) = memory_error {
        reason = "error";
        why = Some(msg);
        value = None;
    }

    Outcome {
        reason,
        why,
        script,
        version,
        value,
        line,
        turns_run,
        clock,
        since_start,
        acts,
        log,
        observe,
        memory,
        failures,
    }
}

/// [`Outcome::memory`] from the `memory` scope variable the script left behind, capped at
/// `cap` bytes of JSON (docs/design/agent-scripting.md, "The SDK": "8 KB cap"): `Ok` with the
/// new value when it's a map (or `()`, the same as never touching it) within the cap; `Err`
/// with a clear, one-line reason otherwise -- an oversize write, or `memory` rebound to
/// something that isn't a map at all -- in which case the caller keeps `previous` unchanged and
/// fails the whole run (docs/design/agent-scripting.md's own "An oversize write fails the
/// script with a clear error").
fn finalize_memory(
    dynamic: Dynamic,
    previous: &serde_json::Value,
    cap: usize,
) -> (serde_json::Value, Option<String>) {
    let parsed = if dynamic.is_unit() {
        Ok(serde_json::json!({}))
    } else if dynamic.is_map() {
        rhai::serde::from_dynamic::<serde_json::Value>(&dynamic).map_err(|e| format!("memory: {e}"))
    } else {
        Err("memory must stay a map (#{...}), not a plain value".to_owned())
    };
    match parsed {
        Ok(value) => {
            let bytes = serde_json::to_string(&value).map_or(usize::MAX, |s| s.len());
            if bytes > cap {
                let msg = format!(
                    "memory write exceeds the {cap}-byte cap ({bytes} bytes); keeping the \
                     previous memory"
                );
                (previous.clone(), Some(msg))
            } else {
                (value, None)
            }
        }
        Err(msg) => (previous.clone(), Some(msg)),
    }
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
/// `"asked"`, `"event"` or `"runaway"`, the last from `Context::check_work`), or anything else
/// (`"error"`, with its own message and `"line:col"`). `code` is the caller's own source, to quote
/// the line a runaway stopped on.
fn classify_error(
    err: &EvalAltResult,
    code: &str,
) -> (&'static str, Option<String>, Option<String>) {
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
            Signal::Runaway { spent, limit } => {
                ("runaway", Some(runaway_why(spent, limit, err, code)), position_of(err))
            }
        };
    }
    ("error", Some(err.to_string()), position_of(err))
}

/// The one line a `"runaway"` says (#564): what was spent against what limit since the last
/// `wait`, and where it was. Rhai gives a script-function call's caller's position, so the line
/// is the top-level statement the work was inside, quoted from `code`.
fn runaway_why(spent: u64, limit: u64, err: &EvalAltResult, code: &str) -> String {
    let at = err
        .position()
        .line()
        .and_then(|l| code.lines().nth(l - 1).map(|text| (l, text.trim())))
        .map_or_else(String::new, |(l, text)| format!(" Stopped at line {l}: `{text}`."));
    format!(
        "{spent} operations since the last wait, over the limit of {limit} (a wait starts the \
         count again; observe, act, cells and the other game calls count for more than one \
         each).{at} A loop that never reaches wait, or one pass that does too much before it, \
         ends here."
    )
}

/// `"line:col"` of `err`, when Rhai could place it (`None` for a position-less error).
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
            memory_bytes: 8192,
            plan_bytes: 8192,
        }
    }

    fn host() -> Host {
        let nation = fn_tui::cli::parse_nation("earth").unwrap();
        Host::new(7, nation, None, None, None, false).unwrap().with_turn_seconds(10)
    }

    static NO_PLAN: serde_json::Value = serde_json::Value::Null;

    /// An empty `memory` map, for a test that isn't checking memory behaviour itself.
    fn no_memory() -> serde_json::Value {
        serde_json::json!({})
    }

    /// `run` with no `wake_on` given (`&[]`), no saved-script version and no `memory`: the
    /// common case for a test that isn't checking wake-on-event, saved-script or memory
    /// behaviour itself.
    fn run_default(host: &mut Host, code: &str, turns: Option<u32>, note: Option<&str>) -> Outcome {
        let memory = no_memory();
        let request = Request {
            code,
            turns,
            note,
            version: 0,
            wake_on: &[],
            memory: &memory,
            plan: &NO_PLAN,
            probe: false,
        };
        run(host, request, &limits())
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

    /// `can_build` (#509) agrees with the real `build` step's own confirm: a script that scans
    /// around the cursor for a spot `can_build` calls `ok` and then `act`s a `build` step right
    /// there always gets it built, never "0 done" -- the two read the same
    /// `fn_ui::build::placement_at` check, so there's nothing for them to disagree about between
    /// the probe and the real step. Written as a plain script, not through `build_near` (a
    /// follow-up, out of scope here), so it exercises `can_build` on its own.
    #[test]
    fn a_script_can_pick_a_spot_with_can_build_and_build_there() {
        let script = r#"
            let c = observe().cursor;
            let found = ();
            for dy in range(-15, 16) {
                for dx in range(-15, 16) {
                    let x = c.x + dx;
                    let y = c.y + dy;
                    if can_build("Bed", x, y).ok {
                        found = #{x: x, y: y};
                        break;
                    }
                }
                if found != () {
                    break;
                }
            }
            if found == () {
                #{found: false}
            } else {
                let r = act(
                    [#{step: "build", building: "Bed", from: found, to: found}],
                    "picked by can_build",
                );
                #{found: true, notice: r.notices[0].notice}
            }
        "#;
        let mut h = host();
        let out = run_default(&mut h, script, None, None);
        assert_eq!(out.reason, "done", "{out:?}");
        let value = out.value.expect("the script returns a map");
        assert_eq!(value["found"], true, "{value}: no free spot found near the cursor");
        let notice = value["notice"].as_str().expect("a notice string");
        assert!(!notice.contains("0 done"), "{notice}");
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

    /// A loop with no `wait` runs away: the operation budget stops it deterministically before
    /// it could ever use a turn.
    #[test]
    fn a_loop_with_no_wait_is_runaway() {
        let mut h = host();
        let out = run_default(&mut h, "let x = 0; loop { x += 1; }", None, None);
        assert_eq!(out.reason, "runaway");
        assert_eq!(out.turns_run, 0);
    }

    /// A runaway says what is true (#564): the work since the last `wait`, the limit, and the
    /// line it stopped on -- as `line` and quoted in `why` -- without suggesting a `wait` after
    /// every scan. Two runs of the same script stop at the same place.
    #[test]
    fn a_runaway_names_the_line_it_stopped_on_and_is_deterministic() {
        let code = "let x = 0;\nwait(1);\nloop {\n    x += 1;\n}";
        let first = run_default(&mut host(), code, None, None);
        assert_eq!(first.reason, "runaway", "{first:?}");
        assert_eq!(first.turns_run, 1, "the wait before the loop ran");
        let why = first.why.as_deref().expect("a runaway says why");
        assert!(why.contains("operations since the last wait"), "{why}");
        assert!(why.contains("limit of 50000"), "{why}");
        assert!(why.contains("Stopped at line 4: `x += 1;`"), "{why}");
        assert!(!why.contains("a loop with no wait?"), "{why}");
        assert!(first.line.as_deref().is_some_and(|l| l.starts_with("4:")), "{:?}", first.line);
        let second = run_default(&mut host(), code, None, None);
        assert_eq!((second.why, second.line), (first.why, first.line));
    }

    /// Work done inside a script function is reported at the call, the statement of the caller's
    /// own code that was running (Rhai gives a terminated call its outermost call site).
    #[test]
    fn a_runaway_inside_a_function_points_at_the_call() {
        let code = "fn spin() {\n    loop { }\n}\nlet y = 1;\nspin();";
        let out = run_default(&mut host(), code, None, None);
        assert_eq!(out.reason, "runaway", "{out:?}");
        assert!(out.why.as_deref().unwrap().contains("line 5: `spin();`"), "{:?}", out.why);
    }

    /// A pass that is a little under the limit before each `wait` never runs away, however many
    /// passes there are, and one a little over does, on its first pass.
    #[test]
    fn the_limit_applies_to_one_pass_between_waits() {
        // About 3 operations per `x += 1` iteration in a `for`: 10_000 stays under 50_000.
        let under = "let x = 0; loop { for i in 0..10000 { x += 1; } wait(1); }";
        let out = run_default(&mut host(), under, Some(10), None);
        assert_eq!((out.reason, out.turns_run), ("budget", 10), "{out:?}");
        let over = "let x = 0; loop { for i in 0..60000 { x += 1; } wait(1); }";
        let out = run_default(&mut host(), over, Some(10), None);
        assert_eq!((out.reason, out.turns_run), ("runaway", 0), "{out:?}");
    }

    /// Native calls cost the budget what they cost the host (#564): Rhai counts `observe()` as
    /// one operation, so without a cost a loop of them would run for minutes. A loop of any of
    /// them with no `wait` runs away (under the test limit of 50_000, `observe` stops after a
    /// few dozen calls), and a `wait` in the loop keeps it going.
    #[test]
    fn a_loop_of_native_calls_with_no_wait_is_runaway() {
        for call in [
            "observe()",
            "cells(0, 0, 39, 39)",
            "look(0, 0, 64, 40)",
            "inspect(1, 1)",
            "screen(\"research\")",
            "can_build(\"Bed\", 1, 1)",
            "act([#{step: \"cursor\", x: 3, y: 3}])",
            "keys(\"<right>\")",
        ] {
            let code = format!("let n = 0; loop {{ {call}; n += 1; }}");
            let mut h = host();
            let out = run_default(&mut h, &code, None, None);
            assert_eq!(out.reason, "runaway", "{call}: {out:?}");
            assert_eq!(out.turns_run, 0);
            let kept = format!("loop {{ {call}; wait(1); }}");
            let out = run_default(&mut host(), &kept, Some(5), None);
            assert_eq!((out.reason, out.turns_run), ("budget", 5), "{call} with a wait: {out:?}");
        }
    }

    /// A call's cost is what the budget sees, so the same loop stops after the same number of
    /// calls on every run: the stop never depends on how fast the machine is.
    #[test]
    fn a_native_loop_stops_after_the_same_number_of_calls_every_time() {
        let code = "let n = 0; loop { cells(0, 0, 39, 39); n += 1; memory.n = n; }";
        let stops: Vec<_> = (0..2)
            .map(|_| {
                let out = run_default(&mut host(), code, None, None);
                (out.reason, out.memory["n"].clone(), out.why)
            })
            .collect();
        assert_eq!(stops[0], stops[1]);
        assert_eq!(stops[0].0, "runaway");
        // 50_000 / (20 + 39 * 39 / 2 = 780 per call, plus a few operations) is about 60 calls.
        let calls = stops[0].1.as_i64().expect("memory.n was written");
        assert!((55..=64).contains(&calls), "{calls} calls before the stop");
    }

    /// The operation budget counts since the last `wait` (#564), so a healthy loop that does a
    /// couple of thousand operations a pass and `wait`s at the end of each never runs away: 100
    /// passes of ~2000 operations is 200k, four times `limits()`'s 50k, and it stops `"budget"`
    /// only because `turns` ran out.
    #[test]
    fn a_loop_with_a_wait_in_it_never_runs_away() {
        let mut h = host();
        let code = "let x = 0; loop { for i in 0..700 { x += 1; } wait(1); }";
        let out = run_default(&mut h, code, Some(100), None);
        assert_eq!(out.reason, "budget", "{out:?}");
        assert_eq!(out.turns_run, 100);
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

    /// A nonzero `version` (`run_script {name}`, #479) suffixes every tagged note with `" vN"`
    /// (docs/design/agent-scripting.md, "Determinism, replay and watching": "its note is
    /// `[keep-fed v3]`"), while `Outcome::script` stays the plain name.
    #[test]
    fn a_nonzero_version_suffixes_the_tag_with_v_n() {
        let mut h = host();
        let memory = no_memory();
        let request = Request {
            code: r#"act([#{step: "cursor", x: 5, y: 5}], "scout");"#,
            turns: None,
            note: Some("keep-fed"),
            version: 3,
            wake_on: &[],
            memory: &memory,
            plan: &NO_PLAN,
            probe: false,
        };
        let out = run(&mut h, request, &limits());
        assert_eq!(out.reason, "done");
        assert_eq!(out.script, "keep-fed");
        assert_eq!(out.version, 3);
        let lines = h.log().lines();
        let LogLine::Keys { note, .. } = &lines[0] else { panic!("{lines:?}") };
        assert_eq!(note.as_deref(), Some("[keep-fed v3] scout"));
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
        let memory = no_memory();
        let request = Request {
            code: r#"log("a"); log("b"); log("c");"#,
            turns: None,
            note: None,
            version: 0,
            wake_on: &[],
            memory: &memory,
            plan: &NO_PLAN,
            probe: false,
        };
        let out = run(&mut h, request, &tiny);
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
        let memory = no_memory();
        let request = Request {
            code: "loop { wait(1); }",
            turns: Some(50),
            note: None,
            version: 0,
            wake_on: &[EventKind::Need, EventKind::Death, EventKind::Endeavor],
            memory: &memory,
            plan: &NO_PLAN,
            probe: false,
        };
        let out = run(&mut h, request, &limits());
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
        let memory = no_memory();
        let request = Request {
            code: r#"wait(3); let seen = events(); log(`events: ${seen.len()}`); seen.len()"#,
            turns: Some(10),
            note: None,
            version: 0,
            wake_on: &[EventKind::Death],
            memory: &memory,
            plan: &NO_PLAN,
            probe: false,
        };
        let out = run(&mut h, request, &limits());
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
        let memory = no_memory();
        let request = Request {
            code: "loop { wait(1); }",
            turns: Some(50),
            note: Some("keep-fed"),
            version: 3,
            wake_on: &[EventKind::Need],
            memory: &memory,
            plan: &NO_PLAN,
            probe: false,
        };
        let out = run(&mut h, request, &limits());
        assert_eq!(out.reason, "event");
        assert!(out.why.is_some());
        assert_eq!(out.script, "keep-fed");
        assert_eq!(out.version, 3);
        assert!(out.turns_run > 0);
        assert!(out.clock.contains(" -> "), "{}", out.clock);
        assert!(!out.since_start.events.is_empty(), "{:?}", out.since_start);
        assert_eq!(out.acts.ok, 0);
        assert_eq!(out.acts.failed, 0);
        assert!(out.acts.last_failure.is_none());
        assert_eq!(out.observe.turn, out.turns_run);
        assert_eq!(out.memory, serde_json::json!({}));
    }

    /// A 45-turn scripted run on seed 7 Earth stays well under the ~8 KB budget the design doc
    /// sets for a wake report, even once a few things have happened.
    #[test]
    fn a_45_turn_run_on_seed_7_earth_stays_under_8kb() {
        let nation = fn_tui::cli::parse_nation("earth").unwrap();
        let mut h = Host::new(7, nation, None, None, None, false).unwrap();
        let memory = no_memory();
        let request = Request {
            code: "wait(45);",
            turns: Some(45),
            note: None,
            version: 0,
            wake_on: &[],
            memory: &memory,
            plan: &NO_PLAN,
            probe: false,
        };
        let out = run(&mut h, request, &limits());
        let json = serde_json::to_string(&out).unwrap();
        assert!(json.len() < 8192, "wake report was {} bytes", json.len());
    }

    // ----------------------------------------------------------------------------------------
    // Memory (#479, docs/design/agent-scripting.md, "The SDK")
    // ----------------------------------------------------------------------------------------

    /// A script sees whatever `memory` it was given, mutates it, and [`Outcome::memory`] is
    /// that mutation -- feeding one run's own `memory` back into a second run's own call is how
    /// a saved script keeps something between `run_script` calls in a session
    /// (`fn_agent::tools::Tools`'s own job; this just proves the one `run` call that makes it
    /// possible).
    #[test]
    fn a_script_reads_and_mutates_the_memory_it_was_given() {
        let mut h = host();
        let out =
            run_default(&mut h, r#"let n = memory.count ?? 0; memory.count = n + 1;"#, None, None);
        assert_eq!(out.reason, "done", "{out:?}");
        assert_eq!(out.memory, serde_json::json!({"count": 1}));
    }

    /// Running the same script twice, each time with the previous run's own `memory`, keeps
    /// accumulating: the session-level persistence `fn_agent::tools::Tools` is built on.
    #[test]
    fn memory_persists_across_two_separate_run_calls() {
        let code = r#"let n = memory.count ?? 0; memory.count = n + 1;"#;
        let mut h = host();
        let empty = no_memory();
        let request = Request {
            code,
            turns: None,
            note: None,
            version: 0,
            wake_on: &[],
            memory: &empty,
            plan: &NO_PLAN,
            probe: false,
        };
        let first = run(&mut h, request, &limits());
        assert_eq!(first.memory, serde_json::json!({"count": 1}));
        let request = Request {
            code,
            turns: None,
            note: None,
            version: 0,
            wake_on: &[],
            memory: &first.memory,
            plan: &NO_PLAN,
            probe: false,
        };
        let second = run(&mut h, request, &limits());
        assert_eq!(second.memory, serde_json::json!({"count": 2}));
    }

    /// A `memory` write that would exceed `Limits::memory_bytes` fails the whole run with a
    /// clear error instead of growing without bound (docs/design/agent-scripting.md, "The
    /// SDK": "An oversize write fails the script with a clear error"), and the caller's own
    /// `memory` is left exactly as it was -- the write never happened as far as the session is
    /// concerned.
    #[test]
    fn an_oversize_memory_write_fails_the_run_and_keeps_the_previous_memory() {
        let tiny = Limits { memory_bytes: 16, ..limits() };
        let mut h = host();
        let previous = serde_json::json!({"camp": "ok"});
        let request = Request {
            code: r#"memory.blob = "this is far too long to fit in sixteen bytes";"#,
            turns: None,
            note: None,
            version: 0,
            wake_on: &[],
            memory: &previous,
            plan: &NO_PLAN,
            probe: false,
        };
        let out = run(&mut h, request, &tiny);
        assert_eq!(out.reason, "error");
        assert!(out.why.as_deref().unwrap().contains("cap"), "{:?}", out.why);
        assert_eq!(out.memory, previous);
    }

    /// Rebinding `memory` to something that isn't a map at all is the same kind of failure: a
    /// clear error, and the previous `memory` kept.
    #[test]
    fn rebinding_memory_to_a_non_map_fails_the_run_and_keeps_the_previous_memory() {
        let mut h = host();
        let previous = serde_json::json!({"camp": "ok"});
        let request = Request {
            code: "memory = 42;",
            turns: None,
            note: None,
            version: 0,
            wake_on: &[],
            memory: &previous,
            plan: &NO_PLAN,
            probe: false,
        };
        let out = run(&mut h, request, &limits());
        assert_eq!(out.reason, "error");
        assert!(out.why.as_deref().unwrap().contains("map"), "{:?}", out.why);
        assert_eq!(out.memory, previous);
    }
}

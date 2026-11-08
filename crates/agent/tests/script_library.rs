//! The saved script library, `memory` and lab notes end-to-end (#479, #480, docs/design/
//! agent-scripting.md, "Memory across sessions"): `save_script`/`scripts`/`notes` through
//! [`Tools::call`] exactly as `run_script` is, `memory` persisting across `run_script` calls in
//! a session, the run log's own `Script`/`Wake`/`Note` lines (ignored by replay, so a
//! `--resume` session can read them back to rebuild its library, `memory` and notes without
//! changing what the game itself replays to) -- including a `save_script` that was never
//! actually run (#480: `save_script` logs its own `Script` line right away, not only once
//! `run_script {name}` first runs it).

use fn_agent::runlog::Writer;
use fn_agent::script::Limits;
use fn_agent::tools::Tools;
use fn_tui::surface::{EventKind, Host, LogLine};
use fn_tui::watch;
use serde_json::json;

const SEED: u64 = 7;
const TURN_SECONDS: u32 = 10;

fn limits() -> Limits {
    Limits {
        default_turns: 15,
        max_turns: 180,
        max_operations: 500_000,
        log_tail: 30,
        default_wake: vec![EventKind::Death, EventKind::Need, EventKind::Endeavor],
        memory_bytes: 8192,
        plan_bytes: 8192,
    }
}

fn host() -> Host {
    let nation = fn_tui::cli::parse_nation("fire").unwrap();
    Host::new(SEED, nation, None, None, None, false).unwrap().with_turn_seconds(TURN_SECONDS)
}

fn tools() -> Tools<Vec<u8>> {
    Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(limits())
}

/// `save_script` starts a new name at version 1 and bumps it on every further call, and
/// `scripts {}` lists it with no outcome yet (it hasn't been run).
#[test]
fn save_script_bumps_its_version_and_scripts_lists_it_unrun() {
    let mut t = tools();
    let first = t.call("save_script", json!({"name": "keep-fed", "code": "1;"})).unwrap();
    assert_eq!(first["version"], 1);
    let second = t.call("save_script", json!({"name": "keep-fed", "code": "2;"})).unwrap();
    assert_eq!(second["version"], 2);

    let listed = t.call("scripts", json!({})).unwrap();
    let scripts = listed["scripts"].as_array().unwrap();
    assert_eq!(scripts.len(), 1);
    assert_eq!(scripts[0]["name"], "keep-fed");
    assert_eq!(scripts[0]["version"], 2);
    assert!(scripts[0]["last_outcome"].is_null());
}

/// `save_script` logs its own `Script` line right away (#480), with `turns: 0` as this line's
/// own "never run" marker -- distinct from any real `run_script {name}` call's own `Script`
/// line, since `ScriptLimits::resolve_turns` always clamps a run's own `turns` to at least 1.
#[test]
fn save_script_logs_a_script_line_with_no_turns() {
    let mut t = tools();
    t.call("save_script", json!({"name": "keep-fed", "code": "1;"})).unwrap();
    let log = String::from_utf8(t.log().get_ref().clone()).unwrap();
    let lines: Vec<LogLine> = log.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let LogLine::Script { name, version, turns, .. } =
        lines.iter().find(|l| matches!(l, LogLine::Script { .. })).expect("a Script line")
    else {
        unreachable!()
    };
    assert_eq!(name, "keep-fed");
    assert_eq!(*version, 1);
    assert_eq!(*turns, 0, "a save-only Script line carries no turns");
}

/// `run_script {name}` runs a saved script's own latest version, tags its log lines
/// `[name vN]`, and `scripts {}` picks up its own last outcome and turns run afterwards.
#[test]
fn run_script_by_name_runs_the_saved_code_and_updates_its_own_last_outcome() {
    let mut t = tools();
    t.call("save_script", json!({"name": "keep-fed", "code": r#"wait(2); "ok""#})).unwrap();

    let out = t.call("run_script", json!({"name": "keep-fed"})).unwrap();
    assert_eq!(out["reason"], "done", "{out}");
    assert_eq!(out["script"], "keep-fed");
    assert_eq!(out["version"], 1);

    let listed = t.call("scripts", json!({})).unwrap();
    let scripts = listed["scripts"].as_array().unwrap();
    assert_eq!(scripts[0]["last_outcome"], "done");
    assert_eq!(scripts[0]["last_turns_run"], 2);
}

/// `run_script` with a `name` nobody ever `save_script`d is a plain tool error naming the
/// missing name, not a panic or a silent no-op.
#[test]
fn run_script_by_an_unknown_name_is_a_tool_error() {
    let mut t = tools();
    let err = t.call("run_script", json!({"name": "nope"})).unwrap_err();
    assert!(err.contains("nope"), "{err}");
}

/// `run_script` needs exactly one of `name` or `code`: neither, or both, is refused outright.
#[test]
fn run_script_needs_exactly_one_of_name_or_code() {
    let mut t = tools();
    assert!(t.call("run_script", json!({})).is_err());
    t.call("save_script", json!({"name": "a", "code": "1;"})).unwrap();
    assert!(t.call("run_script", json!({"name": "a", "code": "2;"})).is_err());
}

/// `memory` persists across separate `run_script` calls in the same session: a second run sees
/// whatever the first one left there.
#[test]
fn memory_persists_across_run_script_calls_in_a_session() {
    let mut t = tools();
    let code = r#"let n = memory.count ?? 0; memory.count = n + 1; memory.count"#;
    let first = t.call("run_script", json!({"code": code})).unwrap();
    assert_eq!(first["memory"], json!({"count": 1}));
    let second = t.call("run_script", json!({"code": code})).unwrap();
    assert_eq!(second["memory"], json!({"count": 2}));
    assert_eq!(second["value"], 2);
}

/// A `memory` write over `data/agent.toml`'s own `memory_bytes` cap fails the run with a clear
/// error, through the same `run_script` tool a model actually calls.
#[test]
fn an_oversize_memory_write_through_run_script_is_a_clear_error() {
    let mut t = Tools::new(host(), Writer::new(Vec::new()))
        .unwrap()
        .with_scripts(Limits { memory_bytes: 16, ..limits() });
    let out = t
        .call("run_script", json!({"code": r#"memory.blob = "far too long for sixteen bytes";"#}))
        .unwrap();
    assert_eq!(out["reason"], "error", "{out}");
    assert!(out["why"].as_str().unwrap().contains("cap"), "{out}");
}

/// `notes {}` starts empty; `notes {append}` adds a line, logs it, and every call keeps
/// returning the full list, oldest first.
#[test]
fn notes_append_grows_the_list_and_logs_each_line() {
    let mut t = tools();
    assert_eq!(t.call("notes", json!({})).unwrap()["notes"], json!([]));
    t.call("notes", json!({"append": "keep-fed stalls once berries run out"})).unwrap();
    let after = t.call("notes", json!({"append": "fix: also harvest from stockpiles"})).unwrap();
    assert_eq!(
        after["notes"],
        json!(["keep-fed stalls once berries run out", "fix: also harvest from stockpiles",])
    );

    let lines = t.log().get_ref().clone();
    let text = String::from_utf8(lines).unwrap();
    let notes: Vec<LogLine> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .filter(|l| matches!(l, LogLine::Note { .. }))
        .collect();
    assert_eq!(
        notes,
        vec![
            LogLine::Note { text: "keep-fed stalls once berries run out".into() },
            LogLine::Note { text: "fix: also harvest from stockpiles".into() },
        ]
    );
}

/// A `run_script` call's own `Script` and `Wake` log lines bracket the `Keys`/`Turn` lines it
/// actually made, in the order they happened: a watcher or `fn_agent keys` reading the log
/// sees `Script`, then the run's own acts and turns, then `Wake` -- not `Wake` before the acts
/// it's reporting on.
#[test]
fn script_and_wake_lines_bracket_the_runs_own_keys_and_turn_lines_in_order() {
    let mut t = tools();
    t.call("save_script", json!({"name": "scout", "code": r#"keys("<right>"); wait(1);"#}))
        .unwrap();
    let saved = t.log().get_ref().len();
    t.call("run_script", json!({"name": "scout"})).unwrap();
    let lines = t.log().get_ref()[saved..].to_vec();
    let text = String::from_utf8(lines).unwrap();
    let types: Vec<String> = text
        .lines()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(types, vec!["call", "script", "keys", "turn", "wake"], "{types:?}");
}

/// `fn_agent serve --resume`'s own job (`Tools::resume`): a session that saved a script, ran it
/// by name, mutated `memory` and appended a note, resumed from its own log, restores exactly
/// the same `scripts {}`, `notes {}` and `memory` a continuing live session would have.
#[test]
fn a_resumed_session_restores_the_same_scripts_notes_and_memory() {
    let path = std::env::temp_dir()
        .join(format!("fn_agent_script_library_resume_test_{}.jsonl", std::process::id()));

    let (want_scripts, want_notes) = {
        let writer = Writer::create(&path).unwrap();
        let mut t = Tools::new(host(), writer).unwrap().with_scripts(limits());
        t.call("save_script", json!({"name": "keep-fed", "code": r#"memory.camp = 1; wait(1);"#}))
            .unwrap();
        t.call("run_script", json!({"name": "keep-fed"})).unwrap();
        t.call("notes", json!({"append": "first run looked fine"})).unwrap();
        t.end("stdin closed").unwrap();
        (t.call("scripts", json!({})).unwrap(), t.call("notes", json!({})).unwrap())
    };

    let (header, lines) = watch::read_file(&path).unwrap();
    let host = Host::resume(&header, &lines).unwrap();
    let writer = Writer::append(&path).unwrap();
    let mut resumed = Tools::resume(host, writer, &lines).with_scripts(limits());
    std::fs::remove_file(&path).ok();

    assert_eq!(resumed.call("scripts", json!({})).unwrap(), want_scripts);
    assert_eq!(resumed.call("notes", json!({})).unwrap(), want_notes);

    // `memory` isn't exposed as its own tool, so prove it round-tripped by reading it back
    // through a no-op script and comparing to what the original session's last run reported.
    let readback = resumed.call("run_script", json!({"code": "memory"})).unwrap();
    assert_eq!(readback["value"], json!({"camp": 1}));
}

/// A script that's `save_script`d but never actually `run_script`d in a session is still
/// restored by `--resume` (#480's own gap left by #479): `scripts {}` shows it with its own
/// name and version, and no outcome, exactly as a live session's own `scripts {}` would.
#[test]
fn a_saved_but_never_run_script_is_restored_by_resume() {
    let path = std::env::temp_dir().join(format!(
        "fn_agent_script_library_save_only_resume_test_{}.jsonl",
        std::process::id()
    ));

    let want_scripts = {
        let writer = Writer::create(&path).unwrap();
        let mut t = Tools::new(host(), writer).unwrap().with_scripts(limits());
        t.call("save_script", json!({"name": "keep-fed", "code": r#"wait(1);"#})).unwrap();
        t.end("stdin closed").unwrap();
        t.call("scripts", json!({})).unwrap()
    };

    let (header, lines) = watch::read_file(&path).unwrap();
    assert!(
        lines.iter().any(|l| matches!(l, LogLine::Script { .. })),
        "save_script should have logged a Script line even though it was never run: {lines:?}"
    );

    let host = Host::resume(&header, &lines).unwrap();
    let writer = Writer::append(&path).unwrap();
    let mut resumed = Tools::resume(host, writer, &lines).with_scripts(limits());
    std::fs::remove_file(&path).ok();

    let got_scripts = resumed.call("scripts", json!({})).unwrap();
    assert_eq!(got_scripts, want_scripts);
    assert_eq!(got_scripts["scripts"][0]["name"], "keep-fed");
    assert_eq!(got_scripts["scripts"][0]["version"], 1);
    assert!(got_scripts["scripts"][0]["last_outcome"].is_null(), "{got_scripts}");

    // The resumed session's own library really has the code, not just the bookkeeping:
    // `run_script {name}` can run it.
    let ran = resumed.call("run_script", json!({"name": "keep-fed"})).unwrap();
    assert_eq!(ran["reason"], "done", "{ran}");
}

/// `Script`, `Wake` and `Note` lines are #479's own commentary, never replayed
/// (`fn_tui::surface::LogLine`'s own doc comment): a scripted log with them stripped out
/// resumes to exactly the same frame as the one that still has them.
#[test]
fn a_scripted_log_resumes_to_the_same_frame_with_its_script_wake_and_note_lines_stripped() {
    let path = std::env::temp_dir()
        .join(format!("fn_agent_script_library_strip_test_{}.jsonl", std::process::id()));
    {
        let writer = Writer::create(&path).unwrap();
        let mut t = Tools::new(host(), writer).unwrap().with_scripts(limits());
        t.call("save_script", json!({"name": "keep-fed", "code": r#"keys("<right>"); wait(1);"#}))
            .unwrap();
        t.call("run_script", json!({"name": "keep-fed"})).unwrap();
        t.call("notes", json!({"append": "looks fine"})).unwrap();
    }

    let (header, lines) = watch::read_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert!(
        lines.iter().any(|l| matches!(l, LogLine::Script { .. } | LogLine::Note { .. })),
        "the log should actually have commentary lines to strip"
    );

    let mut with_commentary = Host::resume(&header, &lines).unwrap();
    let stripped: Vec<LogLine> = lines
        .into_iter()
        .filter(|l| {
            !matches!(l, LogLine::Script { .. } | LogLine::Wake { .. } | LogLine::Note { .. })
        })
        .collect();
    let mut without_commentary = Host::resume(&header, &stripped).unwrap();

    let size = (100, 40);
    assert_eq!(with_commentary.frame(size.0, size.1), without_commentary.frame(size.0, size.1));
}

/// #557: `save_script` adds a one-line hint when the code has lines over the threshold, and
/// still saves; clean code gets no hint at all.
#[test]
fn save_script_hints_at_long_lines_but_always_saves() {
    let mut t = tools();
    let clean = t.call("save_script", json!({"name": "g", "code": "let a = 1;\n"})).unwrap();
    assert!(clean.get("hint").is_none(), "{clean}");

    let long = format!("let a = 1;\nlet b = \"{}\";\n", "x".repeat(120));
    let saved = t.call("save_script", json!({"name": "g", "code": long})).unwrap();
    assert_eq!(saved["version"], 2, "{saved}");
    let hint = saved["hint"].as_str().unwrap();
    assert!(hint.contains("1 line over 100 characters"), "{hint}");
    assert!(hint.contains("longest 131"), "{hint}");
    assert!(!hint.contains('\n'), "{hint}");
    assert_eq!(t.call("scripts", json!({})).unwrap()["scripts"][0]["version"], 2);
}

/// `scripts {name}` returns a saved script's current source (#560), after a re-save the new
/// one, with a one-line call summary naming the script and never the source; the plain list
/// stays source-free; an unknown name is an error naming what exists; and a `--resume` session
/// reads the same source back.
#[test]
fn scripts_with_a_name_reads_the_source() {
    let mut t = tools();
    let code = "// keep fed\nlet f = observe();\nwait(1);\n";
    t.call("save_script", json!({"name": "governor", "code": "old;"})).unwrap();
    t.call("save_script", json!({"name": "governor", "code": code})).unwrap();
    t.call("save_script", json!({"name": "aux", "code": "wait(2);"})).unwrap();

    let got = t.call("scripts", json!({"name": "governor"})).unwrap();
    assert_eq!(got["source"], code);
    assert_eq!(got["version"], 2);
    assert_eq!(got["lines"], 3);
    let listed = t.call("scripts", json!({})).unwrap();
    assert!(!listed.to_string().contains("keep fed"), "the list carries no source: {listed}");
    assert_eq!(listed["scripts"][1]["lines"], 3);

    let err = t.call("scripts", json!({"name": "govenor"})).unwrap_err();
    assert!(err.contains("govenor") && err.contains("aux, governor"), "{err}");

    // Logged like any call, the name only.
    let log = String::from_utf8(t.log().get_ref().clone()).unwrap();
    let lines: Vec<LogLine> = log.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert!(lines.iter().any(|l| matches!(
        l,
        LogLine::Call { tool, summary } if tool == "scripts" && summary == "governor"
    )));
    assert!(!log.contains("\"summary\":\"// keep fed"));

    // A resumed session restores the latest source, not the first.
    let resumed_lines: Vec<LogLine> = lines.into_iter().skip(1).collect();
    let mut resumed =
        Tools::resume(host(), Writer::new(Vec::new()), &resumed_lines).with_scripts(limits());
    assert_eq!(resumed.call("scripts", json!({"name": "governor"})).unwrap()["source"], code);
}

/// Without `--scripts`, `scripts {name}` is refused like the rest of the script tools.
#[test]
fn reading_a_script_needs_scripts() {
    let mut plain = Tools::new(host(), Writer::new(Vec::new())).unwrap();
    let err = plain.call("scripts", json!({"name": "governor"})).unwrap_err();
    assert!(err.contains("--scripts"), "{err}");
}

//! The `plan` tool and the `plan()` binding end-to-end (#524, docs/design/agent-hillclimb.md,
//! "Within a run: a governor and a plan"): set, merge and the byte cap through [`Tools::call`],
//! the read-only `plan()` a script sees, the `plan` log line, and `--resume` restoring the
//! latest plan while replay ignores the line.

use fn_agent::runlog::Writer;
use fn_agent::script::Limits;
use fn_agent::tools::Tools;
use fn_tui::surface::{EventKind, Host, LogLine};
use fn_tui::watch;
use serde_json::json;

const SEED: u64 = 7;

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
    let nation = fn_tui::cli::parse_nation("earth").unwrap();
    Host::new(SEED, nation, None, None, None, false).unwrap()
}

fn tools() -> Tools<Vec<u8>> {
    Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(limits())
}

/// `plan` is a `--scripts` tool, offered and callable only then.
#[test]
fn plan_is_only_offered_with_scripts() {
    let mut plain = Tools::new(host(), Writer::new(Vec::new())).unwrap();
    assert!(!plain.list().iter().any(|t| t["name"] == "plan"));
    let err = plain.call("plan", json!({})).unwrap_err();
    assert!(err.contains("--scripts"), "{err}");
    assert!(tools().list().iter().any(|t| t["name"] == "plan"));
}

/// A fresh session's plan is `{}`; `set` replaces it whole; with no arguments the tool reads.
#[test]
fn set_replaces_the_plan_and_no_arguments_reads_it() {
    let mut t = tools();
    assert_eq!(t.call("plan", json!({})).unwrap(), json!({"plan": {}}));

    let set = t.call("plan", json!({"set": {"goal": "nine signs", "order": ["beds"]}})).unwrap();
    assert_eq!(set, json!({"plan": {"goal": "nine signs", "order": ["beds"]}}));

    let replaced = t.call("plan", json!({"set": {"goal": "food first"}})).unwrap();
    assert_eq!(replaced, json!({"plan": {"goal": "food first"}}));
    assert_eq!(t.call("plan", json!({})).unwrap(), replaced);
}

/// `merge` shallow-merges top-level keys: other keys stay, a merged key is replaced whole.
#[test]
fn merge_shallow_merges_top_level_keys() {
    let mut t = tools();
    t.call("plan", json!({"set": {"goal": "g", "targets": {"max_idle": 3, "stone_cap": 220}}}))
        .unwrap();
    let merged = t
        .call(
            "plan",
            json!({"merge": {"targets": {"max_idle": 5}, "build": [{"building": "Igloo"}]}}),
        )
        .unwrap();
    assert_eq!(
        merged,
        json!({"plan": {
            "goal": "g",
            "targets": {"max_idle": 5},
            "build": [{"building": "Igloo"}],
        }})
    );
}

/// Bad calls are tool errors that leave the plan alone: both `set` and `merge`, a non-object.
#[test]
fn a_bad_call_is_an_error_and_changes_nothing() {
    let mut t = tools();
    t.call("plan", json!({"set": {"goal": "g"}})).unwrap();
    assert!(t.call("plan", json!({"set": {}, "merge": {}})).is_err());
    assert!(t.call("plan", json!({"set": [1, 2]})).is_err());
    assert_eq!(t.call("plan", json!({})).unwrap(), json!({"plan": {"goal": "g"}}));
}

/// A plan over `plan_bytes` fails with a clear error, for `set` and `merge` alike, and the plan
/// from before (and the log) are untouched.
#[test]
fn a_plan_over_the_cap_fails_with_a_clear_error() {
    let mut t = Tools::new(host(), Writer::new(Vec::new()))
        .unwrap()
        .with_scripts(Limits { plan_bytes: 64, ..limits() });
    t.call("plan", json!({"set": {"goal": "short"}})).unwrap();

    let err = t.call("plan", json!({"set": {"blob": "x".repeat(100)}})).unwrap_err();
    assert!(err.contains("64-byte cap"), "{err}");
    let err = t.call("plan", json!({"merge": {"blob": "x".repeat(100)}})).unwrap_err();
    assert!(err.contains("64-byte cap"), "{err}");

    assert_eq!(t.call("plan", json!({})).unwrap(), json!({"plan": {"goal": "short"}}));
    let log = String::from_utf8(t.log().get_ref().clone()).unwrap();
    assert_eq!(log.matches(r#""type":"plan""#).count(), 1, "only the good set is logged");
}

/// `plan()` in a script returns the plan as a map, `{}` with none set; mutating the copy a
/// script got changes nothing for the next call or the next run.
#[test]
fn the_plan_binding_reads_the_plan_and_is_read_only() {
    let mut t = tools();
    let empty = t.call("run_script", json!({"code": "plan()"})).unwrap();
    assert_eq!(empty["value"], json!({}));

    t.call("plan", json!({"set": {"goal": "g", "targets": {"max_idle": 3}}})).unwrap();
    let read =
        t.call("run_script", json!({"code": r#"let p = plan(); p.targets.max_idle + 1"#})).unwrap();
    assert_eq!(read["value"], json!(4), "{read}");

    // A script writing to its copy changes neither a second `plan()` in the same run nor the tool.
    let scribble = t
        .call("run_script", json!({"code": r#"let p = plan(); p.goal = "changed"; plan().goal"#}))
        .unwrap();
    assert_eq!(scribble["value"], json!("g"), "{scribble}");
    assert_eq!(t.call("plan", json!({})).unwrap()["plan"]["goal"], "g");
}

/// Every change writes one `plan` line carrying the whole new plan; a read writes none.
#[test]
fn every_change_writes_a_plan_log_line() {
    let mut t = tools();
    t.call("plan", json!({})).unwrap();
    t.call("plan", json!({"set": {"goal": "g"}})).unwrap();
    t.call("plan", json!({"merge": {"order": ["beds"]}})).unwrap();

    let log = String::from_utf8(t.log().get_ref().clone()).unwrap();
    let plans: Vec<LogLine> = log
        .lines()
        .map(|l| serde_json::from_str::<LogLine>(l).unwrap())
        .filter(|l| matches!(l, LogLine::Plan { .. }))
        .collect();
    assert_eq!(
        plans,
        vec![
            LogLine::Plan { plan: json!({"goal": "g"}) },
            LogLine::Plan { plan: json!({"goal": "g", "order": ["beds"]}) },
        ]
    );
}

/// The call summary says which half was used and never the plan itself.
#[test]
fn the_call_line_never_carries_the_plan() {
    let mut t = tools();
    t.call("plan", json!({"set": {"goal": "secret sauce"}})).unwrap();
    let log = String::from_utf8(t.log().get_ref().clone()).unwrap();
    let call = log
        .lines()
        .map(|l| serde_json::from_str::<LogLine>(l).unwrap())
        .find(|l| matches!(l, LogLine::Call { tool, .. } if tool == "plan"))
        .unwrap();
    assert_eq!(call, LogLine::Call { tool: "plan".into(), summary: "set".into() });
}

/// Resuming from the log restores the latest plan (not an earlier one), and a script in the
/// resumed session sees it through `plan()`.
#[test]
fn a_resumed_session_restores_the_latest_plan() {
    let path = std::env::temp_dir()
        .join(format!("fn_agent_plan_resume_test_{}.jsonl", std::process::id()));
    {
        let writer = Writer::create(&path).unwrap();
        let mut t = Tools::new(host(), writer).unwrap().with_scripts(limits());
        t.call("plan", json!({"set": {"goal": "old"}})).unwrap();
        t.call("plan", json!({"set": {"goal": "new", "research": ["farming"]}})).unwrap();
        t.call("plan", json!({"merge": {"targets": {"max_idle": 2}}})).unwrap();
        t.end("stdin closed").unwrap();
    }

    let (header, lines) = watch::read_file(&path).unwrap();
    let host = Host::resume(&header, &lines).unwrap();
    let writer = Writer::append(&path).unwrap();
    let mut resumed = Tools::resume(host, writer, &lines).with_scripts(limits());
    std::fs::remove_file(&path).ok();

    let want = json!({"goal": "new", "research": ["farming"], "targets": {"max_idle": 2}});
    assert_eq!(resumed.call("plan", json!({})).unwrap(), json!({"plan": want}));
    let seen = resumed.call("run_script", json!({"code": "plan()"})).unwrap();
    assert_eq!(seen["value"], want);
}

/// Replay ignores the line: a log with its `plan` lines stripped resumes to the same frame.
#[test]
fn replay_ignores_plan_lines() {
    let path = std::env::temp_dir()
        .join(format!("fn_agent_plan_replay_test_{}.jsonl", std::process::id()));
    {
        let writer = Writer::create(&path).unwrap();
        let mut t = Tools::new(host(), writer).unwrap().with_scripts(limits());
        t.call("plan", json!({"set": {"goal": "g"}})).unwrap();
        t.call("run_script", json!({"code": r#"keys("<right>"); wait(1);"#})).unwrap();
        t.call("plan", json!({"merge": {"order": []}})).unwrap();
    }
    let (header, lines) = watch::read_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert!(lines.iter().any(|l| matches!(l, LogLine::Plan { .. })));

    let mut with_plan = Host::resume(&header, &lines).unwrap();
    let stripped: Vec<LogLine> =
        lines.iter().filter(|l| !matches!(l, LogLine::Plan { .. })).cloned().collect();
    let mut without_plan = Host::resume(&header, &stripped).unwrap();
    assert_eq!(with_plan.frame(100, 40), without_plan.frame(100, 40));
    // `--keys` replay (`fn_agent keys`) skips it too.
    assert_eq!(watch::to_keys_script(&header, &lines), watch::to_keys_script(&header, &stripped));
}

/// `sdk_reference` documents the tool, the conventional keys and the governor frame, and lists
/// `plan()` among the plumbing.
#[test]
fn sdk_reference_documents_the_plan() {
    let mut t = tools();
    let r = t.call("sdk_reference", json!({})).unwrap();
    assert!(r["plan"]["tool"].as_str().unwrap().contains("plan {set?, merge?}"), "{r}");
    for key in ["goal", "order", "research", "build", "targets"] {
        assert!(r["plan"]["keys"][key].is_string(), "{key} missing from {}", r["plan"]["keys"]);
    }
    let frame = r["plan"]["governor"]["frame"].as_str().unwrap();
    assert!(frame.contains("let p = plan();") && frame.contains("fn beds_duty("), "{frame}");
    let signatures: Vec<&str> = r["plumbing"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["signature"].as_str().unwrap())
        .collect();
    assert!(signatures.contains(&"plan()"), "{signatures:?}");
}

/// #557: the example governor `sdk_reference` shows is a real, runnable script: saved and run
/// against a plan it reads, it runs its turns to the budget and its duties act.
#[test]
fn the_example_governor_runs_under_run_script() {
    let mut t = tools();
    let reference = t.call("sdk_reference", json!({})).unwrap();
    let frame = reference["plan"]["governor"]["frame"].as_str().unwrap().to_owned();
    let plan = json!({"research": [], "targets": {"beds_ahead": 2}, "build": []});
    t.call("plan", json!({"set": plan})).unwrap();
    t.call("save_script", json!({"name": "governor", "code": frame})).unwrap();
    let r = t.call("run_script", json!({"name": "governor", "turns": 3})).unwrap();
    assert_eq!(r["reason"], "budget", "{r}");
    assert!(r["acts"]["ok"].as_u64().unwrap() > 0, "it did its duties: {r}");
    assert!(r["turns_run"].as_u64().unwrap() >= 3, "{r}");
}

/// #557: the layout rules reach `sdk_reference` and the `save_script` tool description.
#[test]
fn layout_rules_reach_sdk_reference_and_save_script() {
    let mut t = tools();
    let r = t.call("sdk_reference", json!({})).unwrap();
    let layout = r["plan"]["governor"]["layout"].as_str().unwrap();
    let tool = t.list().into_iter().find(|t| t["name"] == "save_script").unwrap();
    let description = tool["description"].as_str().unwrap();
    for text in [layout, description] {
        for phrase in [
            "one statement per line",
            "4-space indent",
            "under 80 characters",
            "own named fn",
            "change only what needs changing",
        ] {
            assert!(text.contains(phrase), "missing {phrase:?} in: {text}");
        }
    }
}

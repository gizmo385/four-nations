//! `fn_agent serve --carry-in FILE` end-to-end (#530): a fresh session preloaded with a
//! handoff has the scripts, plan and notes in place before its first call, writes them to the log
//! as `script`, `note` and `plan` lines, and a `--resume` of that log rebuilds the same state.

use fn_agent::runlog::Writer;
use fn_agent::script::Limits;
use fn_agent::tools::Tools;
use fn_tui::surface::{EventKind, Host, LogLine};
use serde_json::json;

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
    let nation = fn_tui::cli::parse_nation("water").unwrap();
    Host::new(7, nation, None, None, None, false).unwrap().with_turn_seconds(10)
}

const HANDOFF: &str = r#"{
    "scripts": {"governor": {"version": 9, "code": "let p = plan(); wait(p.pause); \"ok\""}},
    "notes": ["designations drain", "keep the stockpile free"],
    "plan": {"pause": 2, "goal": "all nine signs"}
}"#;

fn log_lines(t: &Tools<Vec<u8>>) -> Vec<LogLine> {
    let log = String::from_utf8(t.log().get_ref().clone()).unwrap();
    log.lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

#[test]
fn a_carried_in_session_has_its_library_plan_and_notes_before_the_first_call() {
    let mut t = Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(limits());
    t.carry_in(HANDOFF).unwrap();

    let listed = t.call("scripts", json!({})).unwrap();
    assert_eq!(listed["scripts"][0]["name"], "governor");
    assert_eq!(listed["scripts"][0]["version"], 9);
    assert_eq!(t.call("plan", json!({})).unwrap()["plan"]["goal"], "all nine signs");
    let notes = t.call("notes", json!({})).unwrap();
    assert_eq!(notes["notes"], json!(["designations drain", "keep the stockpile free"]));

    // The preloaded script runs by name, reads the preloaded plan, and a re-save goes on from v9.
    let out = t.call("run_script", json!({"name": "governor"})).unwrap();
    assert_eq!(out["reason"], "done", "{out}");
    assert_eq!(out["version"], 9);
    assert_eq!(out["turns_run"], 2, "the governor waited the plan's two turns: {out}");
    let saved = t.call("save_script", json!({"name": "governor", "code": "wait(1);"})).unwrap();
    assert_eq!(saved["version"], 10);
}

#[test]
fn the_log_carries_the_preload_as_ordinary_lines() {
    let mut t = Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(limits());
    t.carry_in(HANDOFF).unwrap();
    let lines = log_lines(&t);
    assert!(matches!(lines[0], LogLine::Header(_)));
    assert!(matches!(
        &lines[1],
        LogLine::Script { name, version: 9, turns: 0, .. } if name == "governor"
    ));
    assert!(matches!(&lines[2], LogLine::Note { text } if text == "designations drain"));
    assert!(matches!(&lines[4], LogLine::Plan { plan } if plan["pause"] == 2));
    assert_eq!(lines.len(), 5);

    // --resume reads them back like any others.
    let rebuilt =
        Tools::resume(host(), Writer::new(Vec::new()), &lines[1..]).with_scripts(limits());
    let mut rebuilt = rebuilt;
    assert_eq!(rebuilt.call("scripts", json!({})).unwrap()["scripts"][0]["version"], 9);
    assert_eq!(rebuilt.call("plan", json!({})).unwrap()["plan"]["pause"], 2);
}

#[test]
fn carry_in_needs_scripts_and_a_handoff() {
    let mut plain = Tools::new(host(), Writer::new(Vec::new())).unwrap();
    assert!(plain.carry_in(HANDOFF).unwrap_err().contains("--scripts"));
    let mut t = Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(limits());
    assert!(t.carry_in("[1, 2]").unwrap_err().contains("handoff"));
    assert_eq!(log_lines(&t).len(), 1, "a refused handoff writes nothing");
}

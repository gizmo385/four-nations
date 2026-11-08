//! Inline `run_script {code}` is a read-only probe (#527, decision #522, docs/design/
//! agent-hillclimb.md, "Probes look, they don't act"): every read works, `act`, `keys`, `wait`
//! and the helpers built on them fail with a fixed message and press nothing, and the same code
//! saved and run by name acts normally.

use fn_agent::runlog::Writer;
use fn_agent::script::Limits;
use fn_agent::tools::Tools;
use fn_tui::surface::{EventKind, Host};
use serde_json::{Value, json};

const REFUSAL: &str = "probes can't act: put this in the governor, or in the plan";

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

fn tools() -> Tools<Vec<u8>> {
    let nation = fn_tui::cli::parse_nation("earth").unwrap();
    let host = Host::new(7, nation, None, None, None, false).unwrap();
    Tools::new(host, Writer::new(Vec::new())).unwrap().with_scripts(limits())
}

/// The `type` of every log line written so far.
fn log_types(t: &Tools<Vec<u8>>) -> Vec<String> {
    let text = String::from_utf8(t.log().get_ref().clone()).unwrap();
    text.lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap()["type"].as_str().unwrap().to_owned())
        .collect()
}

fn probe(t: &mut Tools<Vec<u8>>, code: &str) -> Value {
    t.call("run_script", json!({"code": code, "turns": 5})).unwrap()
}

/// Every read binding works in a probe, including `plan()`, `log` and reading `memory`.
#[test]
fn a_probe_can_call_every_read_binding() {
    let mut t = tools();
    t.call("plan", json!({"set": {"goal": "survive"}})).unwrap();
    let code = r#"
        let o = observe();
        look(90, 60, 8, 8);
        cells(90, 60, 4, 4);
        inspect(94, 64);
        can_build("Bed", 94, 64);
        screen("build");
        events();
        log("surveyed");
        let m = memory;
        find("tree", #{x: 94, y: 64}, 6);
        store("Wood");
        idle();
        need("Thirsty");
        plan().goal
    "#;
    let result = probe(&mut t, code);
    assert_eq!(result["reason"], "done", "{result}");
    assert_eq!(result["value"], "survive");
    assert_eq!(result["turns_run"], 0);
    assert_eq!(result["log"], json!(["surveyed"]));
}

/// A probe that tries to act, key or wait fails with the fixed message and leaves the log with no
/// `keys` or `turn` line and the clock where it was.
#[test]
fn a_probe_that_acts_presses_nothing_and_passes_no_turn() {
    for code in [
        r#"act([#{step: "cursor", x: 5, y: 5}])"#,
        r#"act([#{step: "cursor", x: 5, y: 5}], "note")"#,
        r#"keys("<right>")"#,
        r#"keys("<right>", "note")"#,
        "wait(1)",
        r#"designate_near("harvest", "tree", #{x: 94, y: 64}, 6, false)"#,
        r#"build_near("Bed", #{x: 94, y: 64})"#,
        "wait_until(|| false, 3)",
    ] {
        let mut t = tools();
        let before = t.call("observe", json!({})).unwrap()["turn"].clone();
        let result = probe(&mut t, code);
        assert_eq!(result["reason"], "error", "{code}: {result}");
        let why = result["why"].as_str().unwrap();
        assert!(why.contains(REFUSAL), "{code}: {why}");
        assert_eq!(result["turns_run"], 0, "{code}");
        assert_eq!(result["observe"]["turn"], before, "{code}");
        let types = log_types(&t);
        assert!(!types.iter().any(|ty| ty == "keys" || ty == "turn"), "{code}: {types:?}");
    }
}

/// The same code, saved and run by name, acts normally.
#[test]
fn the_same_code_saved_and_run_by_name_acts() {
    let code = r#"keys("<right>"); wait(1);"#;
    let mut t = tools();
    let result = probe(&mut t, code);
    assert_eq!(result["reason"], "error", "{result}");

    t.call("save_script", json!({"name": "scout", "code": code})).unwrap();
    let result = t.call("run_script", json!({"name": "scout", "turns": 5})).unwrap();
    assert_eq!(result["reason"], "done", "{result}");
    assert_eq!(result["turns_run"], 1);
    let types = log_types(&t);
    assert!(types.iter().any(|ty| ty == "keys"), "{types:?}");
    assert!(types.iter().any(|ty| ty == "turn"), "{types:?}");
}

/// `sdk_reference` and `briefing` both say which calls a probe may make.
#[test]
fn sdk_reference_and_briefing_name_the_probe_rule() {
    let mut t = tools();
    let reference = t.call("sdk_reference", json!({})).unwrap();
    let note = reference["probes"].as_str().unwrap();
    assert!(note.contains("can_build") && note.contains(REFUSAL), "{note}");
    let briefing = t.call("briefing", json!({})).unwrap();
    assert!(briefing["probes"].as_str().unwrap().contains(REFUSAL), "{briefing}");
}

/// With `--scripts`, `briefing` carries the score rule (#531), the same text `score_rule`
/// generates from `data/score.toml`.
#[test]
fn scripts_briefing_states_the_score_rule() {
    let mut t = tools();
    let briefing = t.call("briefing", json!({})).unwrap();
    let rule = briefing["scoring"].as_str().expect("a scoring paragraph");
    assert_eq!(rule, fn_agent::scorecard::bundled_score_rule());
    assert!(rule.contains("progress times care"), "{rule}");
}

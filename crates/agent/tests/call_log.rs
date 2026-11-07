//! `LogLine::Call` (#497, docs/design/agent-studio.md, "A new log line: `call`"): one `call` line
//! per `tools/call`, written ahead of that same call's own lines, ignored like `Script`/`Wake`/
//! `Note` by everything that rebuilds a session rather than displays it.

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
    }
}

fn host() -> Host {
    let nation = fn_tui::cli::parse_nation("fire").unwrap();
    Host::new(SEED, nation, None, None, None, false).unwrap().with_turn_seconds(TURN_SECONDS)
}

fn tools() -> Tools<Vec<u8>> {
    Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(limits())
}

fn log_types(t: &Tools<Vec<u8>>) -> Vec<String> {
    String::from_utf8(t.log().get_ref().clone())
        .unwrap()
        .lines()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

/// Every one of `observe`, `act`, `end_turn`, `save_script` and `run_script` gets its own `call`
/// line right before any line that call's own dispatch makes (its own `keys`/`turn`/`script`/
/// `wake`), exactly the order a live watcher reads them in.
#[test]
fn a_call_line_lands_before_that_calls_own_lines_for_every_tool() {
    let mut t = tools();
    t.call("observe", json!({})).unwrap();
    t.call("act", json!({"steps": [{"step": "cursor", "x": 2, "y": 2}], "note": "scout"})).unwrap();
    t.call("end_turn", json!({"turns": 1})).unwrap();
    t.call("save_script", json!({"name": "keep-fed", "code": "wait(1);"})).unwrap();
    t.call("run_script", json!({"name": "keep-fed", "turns": 1})).unwrap();

    let types = log_types(&t);
    assert_eq!(
        types,
        vec![
            "header", "call", "call", "keys", "call", "turn", "call", "script", "call", "script",
            "turn", "wake",
        ],
        "{types:?}"
    );
}

/// Stripping every `call` line out of a served log changes nothing a replay reads: `fn_agent
/// keys`'s own script (`watch::to_keys_script`) is identical with or without them, and a
/// `Host::resume` built from either set of lines reaches the exact same frame (#497's acceptance
/// criteria).
#[test]
fn stripping_call_lines_changes_neither_the_keys_script_nor_the_resumed_frame() {
    let mut t = tools();
    t.call("look", json!({"x": 0, "y": 0, "w": 4, "h": 4})).unwrap();
    t.call("act", json!({"steps": [{"step": "cursor", "x": 5, "y": 5}], "note": "scout"})).unwrap();
    t.call("end_turn", json!({"turns": 1})).unwrap();
    t.call("save_script", json!({"name": "keep-fed", "code": "wait(1);"})).unwrap();
    t.call("run_script", json!({"name": "keep-fed", "turns": 1})).unwrap();
    t.call("notes", json!({"append": "looks fine so far"})).unwrap();

    let header = t.log().get_ref().clone();
    let text = String::from_utf8(header).unwrap();
    let lines: Vec<LogLine> =
        text.lines().skip(1).map(|l| serde_json::from_str(l).unwrap()).collect();
    assert!(lines.iter().any(|l| matches!(l, LogLine::Call { .. })), "fixture has call lines");

    let stripped: Vec<LogLine> =
        lines.iter().filter(|l| !matches!(l, LogLine::Call { .. })).cloned().collect();
    assert!(stripped.len() < lines.len());

    let header_line: LogLine = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    let LogLine::Header(header) = header_line else { panic!("first line isn't a header") };

    let with_calls_script = watch::to_keys_script(&header, &lines);
    let without_calls_script = watch::to_keys_script(&header, &stripped);
    assert_eq!(with_calls_script, without_calls_script);

    let mut with_calls_host = Host::resume(&header, &lines).unwrap();
    let mut without_calls_host = Host::resume(&header, &stripped).unwrap();
    assert_eq!(with_calls_host.frame(100, 40), without_calls_host.frame(100, 40));
    assert_eq!(with_calls_host.log(), without_calls_host.log());
}

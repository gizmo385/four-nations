//! `run_script` end-to-end (docs/design/agent-scripting.md, #477): `fn_agent serve --scripts`
//! adds the one tool, dispatched through [`Tools::call`] exactly as `act`/`end_turn` are, and a
//! scripted log reads back the same way an MCP-only one does -- `fn_agent keys`'s own
//! `--keys` script, replayed from scratch, lands on the same frame.

use std::io::BufReader;

use fn_agent::runlog::Writer;
use fn_agent::script::Limits;
use fn_agent::server;
use fn_agent::tools::Tools;
use fn_tui::surface::{ActionStep, EventKind, Host};
use fn_tui::watch;
use serde_json::json;

const SEED: u64 = 7;
const TURN_SECONDS: u32 = 10;
const SIZE: (u16, u16) = (100, 40);

const SCRIPT: &str = r#"
    act([#{step: "cursor", x: 5, y: 5}], "scout the east");
    wait(1);
"#;

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

/// `run_script` is only offered once a session is built `with_scripts` (`fn_agent serve
/// --scripts`): a sandboxed racer with no flag never sees it in `tools/list`.
#[test]
fn run_script_is_only_offered_once_with_scripts_is_called() {
    let plain = Tools::new(host(), Writer::new(Vec::new())).unwrap();
    assert!(!plain.list().iter().any(|t| t["name"] == "run_script"));

    let scripted = Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(limits());
    assert!(scripted.list().iter().any(|t| t["name"] == "run_script"));
}

/// Calling `run_script` on a session that wasn't started `with_scripts` is an ordinary tool
/// error, naming the flag it's missing, not a panic or a silent no-op.
#[test]
fn run_script_without_scripts_is_a_tool_error() {
    let mut tools = Tools::new(host(), Writer::new(Vec::new())).unwrap();
    let err = tools.call("run_script", json!({"code": "1"})).unwrap_err();
    assert!(err.contains("--scripts"), "{err}");
}

/// `tools/call run_script` reports `done`, its own return value, the turns it ran and the
/// `observe` the game ended on -- the same shape a direct `fn_agent::script::run` call returns,
/// now reached through the JSON-RPC path a real `fn_agent serve --scripts` speaks.
#[test]
fn tools_call_run_script_round_trips_over_json_rpc() {
    let mut tools = Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(limits());
    tools.call("save_script", json!({"name": "keep-fed", "code": SCRIPT})).unwrap();
    let mut out = Vec::new();
    let script = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"run_script","arguments":{"name":"keep-fed","turns":5}}}"#;
    server::serve(BufReader::new(script.as_bytes()), &mut out, &mut tools);
    let response: serde_json::Value =
        serde_json::from_str(String::from_utf8(out).unwrap().lines().next().unwrap()).unwrap();
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["reason"], "done", "{result}");
    assert_eq!(result["turns_run"], 1);
    assert_eq!(result["observe"]["turn"], 1);
}

/// A `run_script` call's every `act`/`keys` step is a normal `keys` log line
/// (docs/design/agent-scripting.md's "Determinism, replay and watching"): the `--keys` script
/// `fn_agent keys` would print for that log, replayed from scratch through a fresh `Shell`,
/// lands on the same frame a plain MCP `act`/`end_turn` session reaches doing the same thing --
/// the same guarantee `watch_parity.rs` already checks for an MCP-only run.
#[test]
fn fn_agent_keys_on_a_scripted_log_reproduces_the_same_frame_as_an_mcp_only_run() {
    let want = {
        let mut h = host();
        let steps: Vec<ActionStep> =
            serde_json::from_value(json!([{"step": "cursor", "x": 5, "y": 5}])).unwrap();
        h.act(&steps, Some("scout the east".into()));
        h.end_turn(1);
        h.frame(SIZE.0, SIZE.1)
    };

    let path =
        std::env::temp_dir().join(format!("fn_agent_run_script_test_{}.jsonl", std::process::id()));
    {
        let writer = Writer::create(&path).unwrap();
        let mut tools = Tools::new(host(), writer).unwrap().with_scripts(limits());
        tools.call("save_script", json!({"name": "scout", "code": SCRIPT})).unwrap();
        let result = tools.call("run_script", json!({"name": "scout", "turns": 5})).unwrap();
        assert_eq!(result["reason"], "done", "{result}");
    }

    let (header, lines) = watch::read_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    let script = watch::to_keys_script(&header, &lines);

    let args = fn_tui::cli::Args {
        seed: Some(SEED),
        nation: Some(fn_tui::cli::parse_nation("fire").unwrap()),
        frame: false,
        size: fn_tui::cli::DEFAULT_SIZE,
        ticks: 0,
        keys: Vec::new(),
        ascii: false,
        ansi: false,
        color: None,
        embark: false,
        start: None,
        map_size: None,
        rough: false,
        difficulty: None,
        fast: false,
        scenario: None,
        watch: None,
        pace: None,
        at: None,
        record: None,
        fps: None,
    };
    let mut replay = fn_tui::shell::Shell::for_args(&args, fn_tui::cli::DEFAULT_SIZE, None, 0)
        .expect("a header built by this test's own Host always replays");
    let steps = fn_tui::keys::parse(&script).unwrap();
    replay.play(&steps);
    let palette = fn_tui::color::Palette::new(
        replay.defs(),
        replay.nation(),
        fn_tui::color::ColorDepth::TrueColor,
        true,
    );
    let got = fn_tui::frame::to_text(&fn_tui::frame::render_shell(&mut replay, &palette, SIZE));
    assert_eq!(got, want);
}

/// Keys drive a replay, not budgets (#564): a log made under a roomy operation budget -- a pass
/// of 1.2 million operations before its first `wait`, a runaway under the bundled limit --
/// replays to the very game the live run ended in, because replaying a log runs no script and so
/// never asks what the budget was.
#[test]
fn a_log_made_under_a_bigger_budget_replays_to_the_same_game() {
    let heavy = r#"
        let x = 0;
        for i in 0..400000 { x += 1; }
        act([#{step: "cursor", x: 5, y: 5}], "after a long think");
        wait(2);
    "#;
    // The bundled budget refuses that pass...
    let mut bundled =
        Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(Limits::bundled());
    bundled.call("save_script", json!({"name": "heavy", "code": heavy})).unwrap();
    let refused = bundled.call("run_script", json!({"name": "heavy", "turns": 5})).unwrap();
    assert_eq!(refused["reason"], "runaway", "{refused}");

    // ...a session with a bigger one plays it, and its log replays to the same game.
    let path =
        std::env::temp_dir().join(format!("fn_agent_budget_replay_{}.jsonl", std::process::id()));
    let live = {
        let mut roomy = limits();
        roomy.max_operations = 50_000_000;
        let writer = Writer::create(&path).unwrap();
        let mut tools = Tools::new(host(), writer).unwrap().with_scripts(roomy);
        tools.call("save_script", json!({"name": "heavy", "code": heavy})).unwrap();
        let done = tools.call("run_script", json!({"name": "heavy", "turns": 5})).unwrap();
        assert_eq!(done["reason"], "done", "{done}");
        assert_eq!(done["turns_run"], 2);
        done["observe"].clone()
    };
    let (header, lines) = watch::read_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    let replayed = Host::resume(&header, &lines).expect("the log replays");
    assert_eq!(serde_json::to_value(replayed.observe()).unwrap(), live);
}

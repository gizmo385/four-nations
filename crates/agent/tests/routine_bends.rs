//! #377 end-to-end, through the MCP-facing `Tools` (not `Host` directly, #313): a Fire
//! settlement designates Hearthflame on its own spawn tile, and the `end_turn {turns: 30}` an
//! agent would ask for runs every one of those turns, because a routine finished bend no longer
//! counts as "worth stopping for" (`fn_ui::alerts::Entry::worth_stopping`).

use fn_agent::runlog::Writer;
use fn_agent::tools::Tools;
use fn_tui::surface::Host;
use serde_json::{Value, json};

const SEED: u64 = 7;
const TURN_SECONDS: u32 = 5;

fn fresh_host() -> Host {
    let nation = fn_tui::cli::parse_nation("fire").unwrap();
    Host::new(SEED, nation, None, None, None, false).unwrap().with_turn_seconds(TURN_SECONDS)
}

#[test]
fn end_turn_runs_all_30_turns_with_hearthflame_designated() {
    let host = fresh_host();
    // Hearthflame is Fire's signature ability (no research needed), and the cursor starts on
    // the first settler's own spawn tile (`UiState::new`) -- always open ground, so it's a
    // valid target without hunting the map for one.
    let pos = host.observe().cursor;

    let path = std::env::temp_dir()
        .join(format!("fn_agent_routine_bends_test_{}.jsonl", std::process::id()));
    let writer = Writer::create(&path).unwrap();
    let mut tools = Tools::new(host, writer).unwrap();

    let act = tools
        .call(
            "act",
            json!({
                "steps": [{
                    "step": "designate",
                    "tool": "bend",
                    "from": {"x": pos.x, "y": pos.y},
                    "to": {"x": pos.x, "y": pos.y},
                }],
                "note": "kindle a hearthflame",
            }),
        )
        .unwrap();
    assert_eq!(act["error"], Value::Null, "the designation should have planned: {act}");

    let obs = tools.call("end_turn", json!({"turns": 30})).unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(obs["turn"], 30, "a settlement that only bends should run every turn asked: {obs}");
    assert_eq!(obs["stopped"], Value::Null, "{obs}");
}

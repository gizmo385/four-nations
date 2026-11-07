//! `sdk_reference` end-to-end (#480, docs/design/agent-scripting.md, "The SDK"): offered only
//! once a session is built `with_scripts`, and its result names both layers -- the plumbing
//! `register` adds to the engine, and the porcelain `prelude.rhai` defines on top of it --
//! through [`Tools::call`] exactly as `run_script` is.

use fn_agent::runlog::Writer;
use fn_agent::script::Limits;
use fn_agent::tools::Tools;
use fn_tui::surface::{EventKind, Host};
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
    }
}

fn host() -> Host {
    let nation = fn_tui::cli::parse_nation("earth").unwrap();
    Host::new(SEED, nation, None, None, None, false).unwrap()
}

/// `sdk_reference` is only offered once a session is built `with_scripts` (`fn_agent serve
/// --scripts`), the same gate every other script tool shares.
#[test]
fn sdk_reference_is_only_offered_once_with_scripts_is_called() {
    let plain = Tools::new(host(), Writer::new(Vec::new())).unwrap();
    assert!(!plain.list().iter().any(|t| t["name"] == "sdk_reference"));

    let scripted = Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(limits());
    assert!(scripted.list().iter().any(|t| t["name"] == "sdk_reference"));
}

/// Calling `sdk_reference` on a session that wasn't started `with_scripts` is an ordinary tool
/// error naming the missing flag, not a panic.
#[test]
fn sdk_reference_without_scripts_is_a_tool_error() {
    let mut tools = Tools::new(host(), Writer::new(Vec::new())).unwrap();
    let err = tools.call("sdk_reference", json!({})).unwrap_err();
    assert!(err.contains("--scripts"), "{err}");
}

/// The result names both layers: every plumbing call (`observe`, `cells`, `act`, `wait`, ...)
/// with its own `#{signature, doc}`, and every porcelain call (`find`, `designate_near`,
/// `build_near`, `wait_until`, `stalled`, `store`, `idle`, `need`) with its own
/// `#{name, params, doc}` -- read back from the registered functions and the compiled
/// `prelude.rhai`, not a hand-kept copy that could drift from either.
#[test]
fn sdk_reference_names_the_plumbing_and_porcelain_layers() {
    let mut tools = Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(limits());
    let result = tools.call("sdk_reference", json!({})).unwrap();

    let plumbing = result["plumbing"].as_array().expect("plumbing is a list");
    assert!(!plumbing.is_empty());
    for entry in plumbing {
        assert!(!entry["signature"].as_str().unwrap().is_empty(), "{entry}");
        assert!(!entry["doc"].as_str().unwrap().is_empty(), "{entry}: no doc line");
    }
    let plumbing_signatures: Vec<&str> =
        plumbing.iter().map(|e| e["signature"].as_str().unwrap()).collect();
    for want in ["observe()", "cells(_, _, _, _)", "act(_)", "wait(_)"] {
        assert!(
            plumbing_signatures.contains(&want),
            "{want:?} missing from {plumbing_signatures:?}"
        );
    }

    let porcelain = result["porcelain"].as_array().expect("porcelain is a list");
    let porcelain_names: Vec<&str> =
        porcelain.iter().map(|e| e["name"].as_str().unwrap()).collect();
    for want in
        ["find", "designate_near", "build_near", "wait_until", "stalled", "store", "idle", "need"]
    {
        assert!(porcelain_names.contains(&want), "{want:?} missing from {porcelain_names:?}");
    }
    for entry in porcelain {
        assert!(!entry["doc"].as_str().unwrap().is_empty(), "{entry}: no doc line");
    }
}

/// `sdk_reference` changes nothing in the world and leaves no line of its own beyond the `call`
/// every tool gets (#497): calling it mid-session leaves the world and the run log otherwise
/// exactly as they were, the same as any other read-only tool.
#[test]
fn sdk_reference_changes_nothing_and_logs_nothing_of_its_own() {
    let mut tools = Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(limits());
    let before = tools.call("observe", json!({})).unwrap();
    tools.call("sdk_reference", json!({})).unwrap();
    let after = tools.call("observe", json!({})).unwrap();
    assert_eq!(before["turn"], after["turn"]);

    let log = String::from_utf8(tools.log().get_ref().clone()).unwrap();
    let types: Vec<serde_json::Value> =
        log.lines().map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()).collect();
    assert!(types.iter().all(|l| l["type"] != "sdk_reference"), "{types:?}");
}

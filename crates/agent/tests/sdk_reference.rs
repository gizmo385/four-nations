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
        plan_bytes: 8192,
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
/// `prelude.rhai`, not a hand-kept copy that could drift from either. It also names every
/// `act` step shape (#508) as a `#{step, fields, example}`.
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
    for want in ["observe()", "cells(_, _, _, _)", "can_build(_, _, _)", "act(_)", "wait(_)"] {
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

    let action_steps = result["action_steps"].as_array().expect("action_steps is a list");
    let step_names: Vec<&str> = action_steps.iter().map(|e| e["step"].as_str().unwrap()).collect();
    for want in ["designate", "build", "select", "research", "cursor", "keys"] {
        assert!(step_names.contains(&want), "{want:?} missing from {step_names:?}");
    }
    for entry in action_steps {
        assert!(!entry["fields"].as_str().unwrap().is_empty(), "{entry}: no fields");
        assert!(!entry["example"].as_str().unwrap().is_empty(), "{entry}: no example");
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

/// #586: nothing an agent is handed as documentation names a bend ability. Each nation's
/// abilities are its own (Bore Tunnel is Earth's alone), and a name used as an example sent a
/// Water attempt chasing one it could never cast. The `bend` screen is where a script reads the
/// names it can use, so the tool descriptions, the SDK reference, the briefing and the example
/// governor point to that instead of giving one.
#[test]
fn no_documentation_names_a_bend_ability() {
    let session = fn_ui::Session::new(SEED, fn_tui::cli::parse_nation("earth").unwrap(), 48, 32);
    let names: Vec<String> = session.defs.abilities.iter().map(|a| a.name.clone()).collect();
    assert!(names.iter().any(|n| n == "Bore Tunnel"), "{names:?}");

    let mut tools = Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(limits());
    let texts = [
        ("tools/list", serde_json::to_string(&tools.list()).unwrap()),
        ("sdk_reference", tools.call("sdk_reference", json!({})).unwrap().to_string()),
        ("briefing", tools.call("briefing", json!({})).unwrap().to_string()),
        ("example governor", fn_agent::plan::GOVERNOR_FRAME.to_owned()),
    ];
    for (what, text) in &texts {
        for name in &names {
            assert!(!text.contains(name.as_str()), "{what} names the ability {name:?}");
        }
    }
    // And the list is documented where a script looks for it.
    let reference = texts[1].1.as_str();
    assert!(reference.contains("screen(\\\"bend\\\")"), "{reference}");
}

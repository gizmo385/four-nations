//! `fn_agent serve --carry-in FILE` (#530, docs/design/agent-hillclimb.md, "The attempt loop"):
//! a fresh session that starts with a strategy already in hand. FILE is a handoff, the JSON
//! `cargo xtask agent-run --carry` writes: saved scripts (`name` to `{version, code}`), lab
//! `notes` and the `plan`. [`lines`] turns it into the `script`, `note` and `plan` log lines a
//! session would have written had the model made those calls itself, and
//! [`Tools::carry_in`](crate::tools::Tools::carry_in) writes them ahead of the first call. The
//! library, notes and plan are then in place for `run_script {name}`, `notes` and `plan`, and
//! `--resume` rebuilds them from the log like any others. This replaces asking the model to
//! retype every script.

use std::collections::BTreeMap;

use fn_tui::surface::LogLine;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct Entry {
    #[serde(default)]
    version: u32,
    code: String,
}

#[derive(Debug, Deserialize)]
struct Handoff {
    #[serde(default)]
    scripts: BTreeMap<String, Entry>,
    #[serde(default)]
    notes: Vec<String>,
    #[serde(default)]
    plan: Option<Value>,
}

/// The log lines that preload `text` (a handoff's JSON): a never-run `script` line per saved
/// script, in name order, keeping its version (so the next `save_script` continues from it), a
/// `note` line per note, and a `plan` line when there is a plan. The plan is checked against
/// `plan_cap` bytes, the way a `plan {set}` call is.
pub fn lines(text: &str, plan_cap: usize) -> Result<Vec<LogLine>, String> {
    let handoff: Handoff =
        serde_json::from_str(text).map_err(|e| format!("--carry-in isn't a handoff: {e}"))?;
    let mut out = Vec::new();
    for (name, entry) in handoff.scripts {
        if name.trim().is_empty() {
            return Err("--carry-in: a script has an empty name".to_owned());
        }
        out.push(LogLine::Script {
            name,
            // Version 0 means "never saved" to a log reader, which would skip the script.
            version: entry.version.max(1),
            code: entry.code,
            turns: 0,
            wake_on: Vec::new(),
        });
    }
    for text in handoff.notes.into_iter().filter(|n| !n.is_empty()) {
        out.push(LogLine::Note { text });
    }
    if let Some(plan) = handoff.plan {
        let plan = crate::plan::apply(&serde_json::json!({}), Some(plan), None, plan_cap)
            .map_err(|e| format!("--carry-in: {e}"))?;
        if plan.as_object().is_some_and(|m| !m.is_empty()) {
            out.push(LogLine::Plan { plan });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handoff_becomes_script_note_and_plan_lines() {
        let text = r#"{"scripts": {"governor": {"version": 9, "code": "wait(1);"},
                                   "aux": {"version": 2, "code": "wait(2);"}},
                       "notes": ["a", "", "b"], "plan": {"goal": "x"}}"#;
        let got = lines(text, 8192).unwrap();
        assert_eq!(got.len(), 5);
        assert!(
            matches!(&got[0], LogLine::Script { name, version: 2, turns: 0, .. } if name == "aux")
        );
        assert!(matches!(
            &got[1],
            LogLine::Script { name, version: 9, code, .. } if name == "governor" && code == "wait(1);"
        ));
        assert_eq!(got[2], LogLine::Note { text: "a".into() });
        assert_eq!(got[3], LogLine::Note { text: "b".into() });
        assert_eq!(got[4], LogLine::Plan { plan: serde_json::json!({"goal": "x"}) });
    }

    #[test]
    fn an_empty_plan_and_a_missing_version_are_tolerated() {
        let got = lines(r#"{"scripts": {"g": {"code": "x"}}, "plan": {}}"#, 100).unwrap();
        assert_eq!(got.len(), 1);
        assert!(matches!(&got[0], LogLine::Script { version: 1, .. }));
    }

    #[test]
    fn bad_input_names_the_problem() {
        assert!(lines("nope", 100).unwrap_err().contains("handoff"));
        let big = format!(r#"{{"plan": {{"k": "{}"}}}}"#, "x".repeat(200));
        assert!(lines(&big, 100).unwrap_err().contains("over the 100-byte cap"));
        assert!(lines(r#"{"plan": [1]}"#, 100).unwrap_err().contains("JSON object"));
    }
}

//! The handoff: `cargo xtask agent-run --carry DIR` writes `DIR/handoff.json` from a run's
//! final script library, lab notes and plan, and `fn_agent serve --carry-in FILE` and the next
//! run's prompt read it back.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One saved script. `last_outcome` and `last_turns_run` are `xtask`'s orientation text for a
/// resumed chunk; `fn_agent`'s `--carry-in` reads them and ignores them.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptEntry {
    /// 0 (or absent) means never saved.
    #[serde(default)]
    pub version: u32,
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_turns_run: Option<u32>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Handoff {
    #[serde(default)]
    pub scripts: BTreeMap<String, ScriptEntry>,
    #[serde(default)]
    pub notes: Vec<String>,
    /// The `plan` tool's latest map (#524); absent when none was ever set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_version_and_extra_fields_read() {
        let h: Handoff =
            serde_json::from_str(r#"{"scripts": {"g": {"code": "x"}}, "plan": {}, "other": 1}"#)
                .unwrap();
        assert_eq!(h.scripts["g"].version, 0);
        assert!(h.notes.is_empty());
    }

    #[test]
    fn it_round_trips_without_the_optional_fields() {
        let h = Handoff::default();
        assert_eq!(serde_json::to_string(&h).unwrap(), r#"{"scripts":{},"notes":[]}"#);
    }
}

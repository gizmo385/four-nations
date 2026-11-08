//! The saved script library (#479, docs/design/agent-scripting.md, "The trampoline": "`save_script
//! {name, code}` stores a script and bumps its version"): `save_script` stores a script's own
//! code under a name, bumping its version every time; `run_script {name}` runs whatever's there
//! now; `scripts {}` lists every saved name with its version and its last `run_script` call's
//! own outcome and turns run.
//!
//! This is purely a session's own bookkeeping -- `fn_agent::tools::Tools` owns the one
//! [`Library`] a session has. Nothing here reaches a file: a resumed session rebuilds one from
//! the run log's own `Script`/`Turn`/`Wake` lines instead (`fn_agent::tools::rebuild_session`,
//! "Memory across sessions").

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// One saved script: its own code, current version, and what its last `run_script` call
/// reported (`None` until it's actually been run at least once).
#[derive(Clone, Debug, Default)]
struct Entry {
    version: u32,
    code: String,
    last_outcome: Option<String>,
    last_turns_run: Option<u32>,
}

/// Every script `save_script` has stored this session, keyed by name (`scripts {}`'s own
/// list, in name order).
#[derive(Clone, Debug, Default)]
pub struct Library {
    scripts: BTreeMap<String, Entry>,
}

impl Library {
    pub fn new() -> Self {
        Self::default()
    }

    /// `save_script {name, code}`: stores `code` under `name`, bumping its version (1 for a new
    /// name, one more than whatever was there otherwise, even if `code` is unchanged). Returns
    /// the new version.
    pub fn save(&mut self, name: &str, code: &str) -> u32 {
        let entry = self.scripts.entry(name.to_owned()).or_default();
        entry.version += 1;
        entry.code = code.to_owned();
        entry.version
    }

    /// `name`'s own code and current version, for `run_script {name}` to run: `None` if nothing
    /// was ever saved under that name.
    pub fn get(&self, name: &str) -> Option<(&str, u32)> {
        self.scripts.get(name).map(|e| (e.code.as_str(), e.version))
    }

    /// Records a `run_script {name}` call's own outcome and turns run against `name`
    /// (`scripts {}`'s own "last outcome and turns run"): a no-op if `name` was never
    /// `save_script`d (an ad hoc `run_script {code}` call never reaches this).
    pub fn record_run(&mut self, name: &str, outcome: &str, turns_run: u32) {
        if let Some(entry) = self.scripts.get_mut(name) {
            entry.last_outcome = Some(outcome.to_owned());
            entry.last_turns_run = Some(turns_run);
        }
    }

    /// Sets `name`'s own saved code and version directly, inserting it if it's new: how
    /// `rebuild_session` replays the run log's own `Script` lines (which already carry whatever
    /// version a live `save_script`/`run_script` call reached) back into a resumed session's
    /// library, in the same chronological order they were originally run.
    pub fn restore(&mut self, name: &str, version: u32, code: &str) {
        let entry = self.scripts.entry(name.to_owned()).or_default();
        entry.version = version;
        entry.code = code.to_owned();
    }

    /// `scripts {}`'s own result: every saved script's name, version, last outcome and turns
    /// run (`null` for either until it's been run at least once), in name order.
    pub fn list(&self) -> Value {
        let scripts: Vec<Value> = self
            .scripts
            .iter()
            .map(|(name, e)| {
                json!({
                    "name": name,
                    "version": e.version,
                    "last_outcome": e.last_outcome,
                    "last_turns_run": e.last_turns_run,
                    "lines": e.code.lines().count(),
                })
            })
            .collect();
        json!({ "scripts": scripts })
    }

    /// `scripts {name}`'s own result (#560): that one script's list entry plus its current
    /// `source`, so a model can revise it instead of rewriting it. An unknown name is an `Err`
    /// naming the scripts that do exist.
    pub fn read(&self, name: &str) -> Result<Value, String> {
        let Some(e) = self.scripts.get(name) else {
            let known: Vec<&str> = self.scripts.keys().map(String::as_str).collect();
            return Err(if known.is_empty() {
                format!("scripts: no script named {name:?}; none are saved yet")
            } else {
                format!("scripts: no script named {name:?}; saved: {}", known.join(", "))
            });
        };
        Ok(json!({
            "name": name,
            "version": e.version,
            "last_outcome": e.last_outcome,
            "last_turns_run": e.last_turns_run,
            "lines": e.code.lines().count(),
            "source": e.code,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Saving a new name starts at version 1; saving the same name again bumps it, even with
    /// identical code.
    #[test]
    fn save_starts_at_one_and_bumps_on_every_call() {
        let mut lib = Library::new();
        assert_eq!(lib.save("keep-fed", "code v1"), 1);
        assert_eq!(lib.save("keep-fed", "code v1"), 2);
        assert_eq!(lib.save("keep-fed", "code v3"), 3);
        assert_eq!(lib.get("keep-fed"), Some(("code v3", 3)));
    }

    /// Two different names keep their own, independent versions.
    #[test]
    fn different_names_have_independent_versions() {
        let mut lib = Library::new();
        lib.save("keep-fed", "a");
        lib.save("scout", "b");
        lib.save("keep-fed", "a2");
        assert_eq!(lib.get("keep-fed"), Some(("a2", 2)));
        assert_eq!(lib.get("scout"), Some(("b", 1)));
    }

    /// A name nobody ever saved has no entry.
    #[test]
    fn get_is_none_for_an_unsaved_name() {
        assert_eq!(Library::new().get("nope"), None);
    }

    /// `scripts {}` lists a freshly saved script with no outcome yet, then its own outcome and
    /// turns run once `record_run` reports one.
    #[test]
    fn list_shows_no_outcome_until_a_run_is_recorded() {
        let mut lib = Library::new();
        lib.save("keep-fed", "code");
        let before = lib.list();
        assert_eq!(before["scripts"][0]["name"], "keep-fed");
        assert_eq!(before["scripts"][0]["version"], 1);
        assert!(before["scripts"][0]["last_outcome"].is_null());

        lib.record_run("keep-fed", "event", 23);
        let after = lib.list();
        assert_eq!(after["scripts"][0]["last_outcome"], "event");
        assert_eq!(after["scripts"][0]["last_turns_run"], 23);
    }

    /// `read` returns a saved script's own source with its list fields, and names the saved
    /// scripts when the name is unknown.
    #[test]
    fn read_returns_the_source_or_names_what_exists() {
        let mut lib = Library::new();
        assert!(lib.read("nope").unwrap_err().contains("none are saved"));
        lib.save("governor", "a;\nb;\n");
        lib.save("aux", "c;");
        lib.record_run("governor", "done", 4);
        let got = lib.read("governor").unwrap();
        assert_eq!(got["source"], "a;\nb;\n");
        assert_eq!(got["version"], 1);
        assert_eq!(got["lines"], 2);
        assert_eq!(got["last_outcome"], "done");
        let err = lib.read("gov").unwrap_err();
        assert!(err.contains("\"gov\"") && err.contains("aux, governor"), "{err}");
        // The list never carries sources.
        assert!(lib.list()["scripts"][0].get("source").is_none());
    }

    /// Recording a run against a name nobody saved is a harmless no-op (an ad hoc
    /// `run_script {code}` call never names a saved script to record against).
    #[test]
    fn record_run_on_an_unsaved_name_does_nothing() {
        let mut lib = Library::new();
        lib.record_run("nope", "done", 5);
        assert!(lib.list()["scripts"].as_array().unwrap().is_empty());
    }

    /// `restore` sets a name's code and version directly, overwriting whatever was there --
    /// `rebuild_session`'s own way of replaying the log's `Script` lines back into a fresh
    /// library in order.
    #[test]
    fn restore_sets_the_code_and_version_directly() {
        let mut lib = Library::new();
        lib.restore("keep-fed", 1, "v1 code");
        lib.restore("keep-fed", 2, "v2 code");
        assert_eq!(lib.get("keep-fed"), Some(("v2 code", 2)));
    }
}

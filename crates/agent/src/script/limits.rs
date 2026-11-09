//! `data/agent.toml`'s `[scripts]` table (docs/design/agent-scripting.md, "The trampoline"):
//! read by `fn_agent` only, never by `fn_sim` -- the same as `data/tui.toml` is `fn_tui`'s own
//! (`fn_tui::glyphs::Glyphs::bundled`).

use fn_tui::surface::EventKind;
use serde::Deserialize;

/// Limits on one `run_script` call, from `data/agent.toml`'s `[scripts]` table.
#[derive(Clone, Debug, Deserialize)]
pub struct Limits {
    /// A `run_script` call's own turn budget when the caller doesn't give one.
    pub default_turns: u32,
    /// No call may ask for more turns than this, whatever `turns` it's given.
    pub max_turns: u32,
    /// The work a script may do between two `wait`s (#564), counted in Rhai operations plus a
    /// fixed cost per native call (`bindings::cost`): past it the script stops
    /// deterministically ("runaway") instead of running on. Every `wait` starts the count again,
    /// so a loop that waits never reaches it, however long it runs.
    pub max_operations: u64,
    /// How many of the script's own log lines a result keeps, most recent last.
    pub log_tail: usize,
    /// `run_script`'s own `wake_on` when the caller doesn't give one (#478,
    /// docs/design/agent-scripting.md, "`run_script`": "The default `wake_on` is `death`,
    /// `need` and `endeavor`").
    pub default_wake: Vec<EventKind>,
    /// The `memory` map's own cap, in bytes of its JSON encoding (#479, docs/design/
    /// agent-scripting.md, "The SDK": "8 KB cap"). A write that would exceed this fails the
    /// script with a clear error instead of persisting (`runner::finalize_memory`).
    pub memory_bytes: usize,
    /// The `plan` tool's own cap, in bytes of the plan's JSON encoding (#524,
    /// docs/design/agent-hillclimb.md, "Within a run: a governor and a plan": "capped at 8 KB
    /// like `memory`"). A `set` or `merge` that would exceed it is a tool error and changes
    /// nothing.
    #[serde(default = "default_plan_bytes")]
    pub plan_bytes: usize,
}

fn default_plan_bytes() -> usize {
    8192
}

/// What one campaign attempt may use, from `data/agent.toml`'s `[attempt]` table (#570).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub struct AttemptLimits {
    /// How many times one attempt may call `test_governor`, rejected calls included, whatever
    /// the number of chunks it takes.
    pub max_tests: u32,
}

/// `data/agent.toml`'s own shape: `[scripts]` (#477) and `[attempt]` (#570).
#[derive(Deserialize)]
struct File {
    scripts: Limits,
    attempt: AttemptLimits,
}

impl AttemptLimits {
    /// The attempt limits shipped in `data/agent.toml`, embedded at compile time.
    pub fn bundled() -> Self {
        toml::from_str::<File>(include_str!("../../../../data/agent.toml"))
            .expect("bundled data/agent.toml must be valid; run `cargo test -p fn_agent`")
            .attempt
    }
}

impl Limits {
    /// The limits shipped in `data/agent.toml`, embedded at compile time.
    pub fn bundled() -> Self {
        Self::from_toml(include_str!("../../../../data/agent.toml"))
            .expect("bundled data/agent.toml must be valid; run `cargo test -p fn_agent`")
    }

    pub fn from_toml(src: &str) -> Result<Self, toml::de::Error> {
        toml::from_str::<File>(src).map(|f| f.scripts)
    }

    /// `turns` (`run_script`'s own, optional `turns`), defaulted to [`Self::default_turns`] and
    /// clamped to at least 1 and at most [`Self::max_turns`].
    pub fn resolve_turns(&self, turns: Option<u32>) -> u32 {
        turns.unwrap_or(self.default_turns).clamp(1, self.max_turns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_file_is_valid_and_sane() {
        let l = Limits::bundled();
        assert!(l.default_turns >= 1);
        assert!(l.default_turns <= l.max_turns);
        assert!(l.max_operations > 0);
        assert!(l.log_tail > 0);
        assert!(!l.default_wake.is_empty());
        assert!(l.memory_bytes > 0);
        assert!(l.plan_bytes > 0);
        assert!(AttemptLimits::bundled().max_tests >= 30, "#570: at least 30 tests");
    }

    fn limits() -> Limits {
        Limits {
            default_turns: 15,
            max_turns: 180,
            max_operations: 1000,
            log_tail: 10,
            default_wake: vec![EventKind::Death, EventKind::Need, EventKind::Endeavor],
            memory_bytes: 8192,
            plan_bytes: 8192,
        }
    }

    #[test]
    fn resolve_turns_defaults_when_none_is_given() {
        assert_eq!(limits().resolve_turns(None), 15);
    }

    #[test]
    fn resolve_turns_clamps_to_at_least_one_and_at_most_max_turns() {
        assert_eq!(limits().resolve_turns(Some(0)), 1);
        assert_eq!(limits().resolve_turns(Some(999)), 180);
        assert_eq!(limits().resolve_turns(Some(50)), 50);
    }
}

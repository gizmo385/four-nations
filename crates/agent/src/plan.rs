//! The `plan` tool's own rules (#524, docs/design/agent-hillclimb.md, "Within a run: a governor
//! and a plan"): the plan is the model's intent held as data, a JSON map scripts read through
//! `plan()`. `memory` is what scripts remember; the plan is what the model wants. [`apply`] is
//! the one place a `set` or `merge` is checked and combined; [`reference`] is the convention and
//! the governor frame `sdk_reference` shows.

use serde_json::{Map, Value, json};

/// The plan after a `plan {set?, merge?}` call, or the reason it can't change.
/// - Neither argument: the plan unchanged (a read).
/// - `set` (a JSON object) replaces the plan.
/// - `merge` (a JSON object) shallow-merges its top-level keys into the plan; a `null` value
///   removes that key.
/// - Both is an error, as is a non-object, or a result whose JSON encoding is over `cap` bytes
///   (`data/agent.toml` `[scripts] plan_bytes`): the plan is left as it was.
pub fn apply(
    current: &Value,
    set: Option<Value>,
    merge: Option<Value>,
    cap: usize,
) -> Result<Value, String> {
    let next = match (set, merge) {
        (None, None) => return Ok(current.clone()),
        (Some(_), Some(_)) => return Err("plan: give set or merge, not both".to_owned()),
        (Some(set), None) => as_map("set", set)?,
        (None, Some(merge)) => {
            let mut map = current.as_object().cloned().unwrap_or_default();
            for (key, value) in as_map("merge", merge)? {
                if value.is_null() {
                    map.remove(&key);
                } else {
                    map.insert(key, value);
                }
            }
            map
        }
    };
    let next = Value::Object(next);
    let bytes = next.to_string().len();
    if bytes > cap {
        return Err(format!(
            "plan: that would make the plan {bytes} bytes, over the {cap}-byte cap \
             (plan_bytes); the plan is unchanged. Trim it: shorter lists, fewer keys."
        ));
    }
    Ok(next)
}

fn as_map(arg: &str, value: Value) -> Result<Map<String, Value>, String> {
    match value {
        Value::Object(map) => Ok(map),
        other => Err(format!("plan: {arg} must be a JSON object, not {}", kind(&other))),
    }
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// The governor frame from docs/design/agent-hillclimb.md: a shape, not a strategy. The helper
/// functions it calls are the agent's own, written and kept in the same script.
pub const GOVERNOR_FRAME: &str = r#"// governor: one loop of duties. Intent lives in plan(); this is the how.
let camp = memory.camp ?? observe().cursor;
memory.camp = camp;
loop {
    let p = plan();
    let g = observe();
    if g.research.label.contains("none set") {
        for t in p.research { if act([#{step: "research", tech: t}], "plan").ok { break; } }
    }
    food_duty(p.targets, camp);          // helpers the agent writes and keeps in this script
    beds_duty(p.targets, camp);
    build_queue_duty(p.build, camp);     // one entry at a time, retried until it stands
    sign_duty(p.order);                  // the first unmet sign in the plan's order
    if idle() > p.targets.max_idle { wake(`${idle()} idle: the plan has no work for them`); }
    wait(3);
}
"#;

/// `sdk_reference`'s `plan` section: what the tool does, the conventional keys, and the governor
/// frame. The keys are the agent's own to extend; the convention is what the scorecard and the
/// studio display.
pub fn reference() -> Value {
    json!({
        "tool": "plan {set?, merge?}: with neither, returns the current plan (a JSON map). \
                 `set` replaces it; `merge` shallow-merges top-level keys (a null value removes \
                 a key). Capped at data/agent.toml's plan_bytes; a bigger plan fails with an \
                 error and changes nothing. Every change is logged and restored by --resume. \
                 Scripts read it with plan() (a read-only copy, as of when the run started).",
        "keys": {
            "goal": "one sentence: what this settlement is for, e.g. \"all nine signs by \
                     minute 150, nobody past Hungry\".",
            "order": "the signs/objectives in the order to pursue them, e.g. [\"beds\", \
                      \"stone\", \"scrolls\", \"population\", ...].",
            "research": "the research queue, tech names in order.",
            "build": "the build queue: [{building, near?}, ...], one entry at a time until it \
                      stands.",
            "targets": "the governor's numbers, e.g. {beds_ahead, food_per_head, \
                        stockpile_free, max_idle, stone_cap}.",
        },
        "governor": {
            "convention": "By convention one saved script named `governor` owns the whole \
                           settlement: one loop of duties (water, food, beds, stockpile, \
                           research, the build queue, the next sign in plan().order, idle \
                           hands), each reading its numbers from the plan. Revise it, don't \
                           replace it. Most wakes become plan edits: \"build a dock\" is \
                           plan {merge: {build: [...]}}, and \"food is falling\" is raising \
                           targets.food_per_head. Write code only when the governor can't do \
                           something at all. Re-run the governor after a plan edit: a running \
                           script sees the plan as of its start.",
            "frame": GOVERNOR_FRAME,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: usize = 8192;

    #[test]
    fn neither_argument_reads_the_plan() {
        let plan = json!({"goal": "x"});
        assert_eq!(apply(&plan, None, None, CAP).unwrap(), plan);
    }

    #[test]
    fn set_replaces_the_whole_plan() {
        let plan = json!({"goal": "x", "order": ["a"]});
        let next = apply(&plan, Some(json!({"goal": "y"})), None, CAP).unwrap();
        assert_eq!(next, json!({"goal": "y"}));
    }

    #[test]
    fn merge_replaces_top_level_keys_and_keeps_the_rest() {
        let plan = json!({"goal": "x", "targets": {"a": 1, "b": 2}});
        let next = apply(&plan, None, Some(json!({"targets": {"a": 9}, "order": []})), CAP);
        // Shallow: `targets` is replaced whole, not merged key by key.
        assert_eq!(next.unwrap(), json!({"goal": "x", "targets": {"a": 9}, "order": []}));
    }

    #[test]
    fn merge_null_removes_a_key() {
        let plan = json!({"goal": "x", "order": []});
        let next = apply(&plan, None, Some(json!({"order": null})), CAP).unwrap();
        assert_eq!(next, json!({"goal": "x"}));
    }

    #[test]
    fn set_and_merge_together_are_refused() {
        let err = apply(&json!({}), Some(json!({})), Some(json!({})), CAP).unwrap_err();
        assert!(err.contains("not both"), "{err}");
    }

    #[test]
    fn a_non_object_is_refused() {
        let err = apply(&json!({}), Some(json!([1])), None, CAP).unwrap_err();
        assert!(err.contains("set must be a JSON object"), "{err}");
        let err = apply(&json!({}), None, Some(json!("x")), CAP).unwrap_err();
        assert!(err.contains("merge must be a JSON object"), "{err}");
    }

    #[test]
    fn a_plan_over_the_cap_fails_with_a_clear_error() {
        let err = apply(&json!({}), Some(json!({"blob": "x".repeat(100)})), None, 50).unwrap_err();
        assert!(err.contains("cap") && err.contains("unchanged"), "{err}");
        // A merge that pushes an existing plan over the cap fails too.
        let plan = json!({"a": "x".repeat(30)});
        assert!(apply(&plan, None, Some(json!({"b": "y".repeat(30)})), 50).is_err());
    }

    /// The frame has to be real Rhai, or it teaches a script that doesn't compile.
    #[test]
    fn the_governor_frame_compiles() {
        crate::script::prelude::compile_check(GOVERNOR_FRAME).expect("the governor frame parses");
    }
}

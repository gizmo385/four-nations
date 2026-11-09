//! `sdk_reference`'s own action-step half (#508, docs/design/agent-play.md, "Action"): one
//! `#{step, fields, example}` per [`ActionStep`] variant's own serde tag (`#[serde(tag =
//! "step")]`), so a script author can see every step `act` accepts, and one example of each,
//! without reading `fn_tui::surface`'s definition or guessing from an error. Hand-kept text --
//! unlike [`crate::script::prelude::reference`]'s `///` comments, a native Rust enum carries no
//! doc comment [`reference`] could read back -- checked against the enum's own variants by
//! `every_action_step_variant_has_a_reference_entry` below, in the same spirit as
//! `crate::script::bindings`'s `every_registered_plumbing_function_has_a_non_empty_doc_line`:
//! a variant with no entry here fails that test, and `step_tag`'s own exhaustive match fails to
//! even compile once a new variant lands, so both sides catch the drift.

use serde_json::{Value, json};

/// One [`ActionStep`] variant's own entry.
struct ActionStepDoc {
    /// The serde tag `act`'s own `{"step": ...}` shape uses.
    step: &'static str,
    /// The fields a script actually passes, in prose (`docs/design/agent-play.md`'s own
    /// "Action" table, condensed to one line).
    fields: &'static str,
    /// One `act` call using this step, as a script would write it.
    example: &'static str,
}

const ACTION_STEP_DOCS: &[ActionStepDoc] = &[
    ActionStepDoc {
        step: "designate",
        fields: "{tool, from, to, standing?, teardown?, ability?}: tool is \"harvest\", \
                 \"cancel\", \"clear\", \"stockpile\", \"bend\" or \"sow\", from/to a \
                 rectangle. standing (default false) only applies when tool is \"harvest\": \
                 true re-queues the designation once the resource ripens again. teardown \
                 (default false) only applies when tool is \"cancel\": without it, cancel \
                 withdraws only job designations (a harvest, a bend, a sow or a clear mark) \
                 and leaves a build site or stockpile tile alone; with it, those go too. \
                 ability only applies when tool is \"bend\": the ability's own name (as the \
                 Bend tool's own banner shows it, as screen(\"bend\") lists them -- each nation's \
                 abilities are its own, so read the list rather than guessing a name); with \
                 none given, the \
                 Bend tool is picked (or kept) without changing which ability is active. Any \
                 of these three set on the wrong tool, or an ability that's unknown or not yet \
                 usable, fails to plan.",
        example: "act([#{step: \"designate\", tool: \"sow\", from: #{x: 10, y: 10}, \
                   to: #{x: 12, y: 10}}], \"sow the near field\")",
    },
    ActionStepDoc {
        step: "build",
        fields: "{building, from, to}: building is the Build modal's own name (as look()'s \
                 legend and screen(\"build\")'s rows list it), and the rectangle to place it \
                 over.",
        example: "act([#{step: \"build\", building: \"Bed\", from: #{x: 40, y: 22}, \
                   to: #{x: 40, y: 22}}], \"a bed near camp\")",
    },
    ActionStepDoc {
        step: "select",
        fields: "{x, y} or {name}, not both and not neither: the unit at a tile, or the \
                 settler with that name (screen(\"nation\")'s own roster).",
        example: "act([#{step: \"select\", name: \"Toph\"}])",
    },
    ActionStepDoc {
        step: "research",
        fields: "{tech}: the tech's own name (screen(\"research\")'s cards).",
        example: "act([#{step: \"research\", tech: \"Irrigation\"}])",
    },
    ActionStepDoc {
        step: "cursor",
        fields: "{x, y}: moves the cursor there, with no Enter, so observe()'s cursor field \
                 reads it without designating or selecting anything.",
        example: "act([#{step: \"cursor\", x: 40, y: 22}])",
    },
    ActionStepDoc {
        step: "keys",
        fields: "{script}: a --keys token script, pressed directly, without <tick N> \
                 (wait(turns) is how a script spends game time).",
        example: "act([#{step: \"keys\", script: \"h <right*2> <enter>\"}])",
    },
];

/// `sdk_reference`'s own action-step half: `ACTION_STEP_DOCS` as `#{step, fields, example}`
/// maps, in the order `fn_tui::surface::ActionStep` defines its variants.
pub(crate) fn reference() -> Vec<Value> {
    ACTION_STEP_DOCS
        .iter()
        .map(|d| json!({"step": d.step, "fields": d.fields, "example": d.example}))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fn_tui::surface::{ActionStep, Coord, DesignateTool};

    /// `step`'s own serde tag: an exhaustive match, so a variant `fn_tui::surface::ActionStep`
    /// gains without a matching arm here fails to compile rather than silently missing from
    /// this module's own test coverage.
    fn step_tag(step: &ActionStep) -> &'static str {
        match step {
            ActionStep::Designate { .. } => "designate",
            ActionStep::Build { .. } => "build",
            ActionStep::Select { .. } => "select",
            ActionStep::Research { .. } => "research",
            ActionStep::Cursor { .. } => "cursor",
            ActionStep::Keys { .. } => "keys",
        }
    }

    /// One sample of every `ActionStep` variant, for `every_action_step_variant_has_a_
    /// reference_entry` below.
    fn samples() -> Vec<ActionStep> {
        vec![
            ActionStep::Designate {
                tool: DesignateTool::Harvest,
                from: Coord { x: 0, y: 0 },
                to: Coord { x: 0, y: 0 },
                standing: false,
                teardown: false,
                ability: None,
            },
            ActionStep::Build {
                building: "Bed".into(),
                from: Coord { x: 0, y: 0 },
                to: Coord { x: 0, y: 0 },
            },
            ActionStep::Select { x: None, y: None, name: Some("Anyone".into()) },
            ActionStep::Research { tech: "Irrigation".into() },
            ActionStep::Cursor { x: 0, y: 0 },
            ActionStep::Keys { script: "h".into() },
        ]
    }

    /// Every `ActionStep` variant shows up in [`ACTION_STEP_DOCS`]: the moment a new variant
    /// lands with no matching entry here, this fails rather than `sdk_reference` quietly
    /// omitting it (`crate::script::bindings`'s own
    /// `every_registered_plumbing_function_has_a_non_empty_doc_line` does the same for the
    /// plumbing layer).
    #[test]
    fn every_action_step_variant_has_a_reference_entry() {
        let docs: Vec<&str> = ACTION_STEP_DOCS.iter().map(|d| d.step).collect();
        for s in &samples() {
            let tag = step_tag(s);
            assert!(docs.contains(&tag), "{tag:?} missing from ACTION_STEP_DOCS: {docs:?}");
        }
    }

    /// Every entry's `fields` and `example` text is non-empty.
    #[test]
    fn every_action_step_doc_has_fields_and_an_example() {
        for v in reference() {
            assert!(!v["fields"].as_str().unwrap().is_empty(), "{v}");
            assert!(!v["example"].as_str().unwrap().is_empty(), "{v}");
        }
    }

    /// Each sample actually serializes under the tag `step_tag` claims for it: `ACTION_STEP_
    /// DOCS`'s own `step` names track `act`'s real wire shape, not just this test's guess.
    #[test]
    fn step_tag_matches_the_real_serde_tag() {
        for s in &samples() {
            let v = serde_json::to_value(s).unwrap();
            assert_eq!(v["step"].as_str().unwrap(), step_tag(s), "{v}");
        }
    }
}

//! The title screen's words (#880): what each step says besides the data's own names, so the
//! terminal (`fn_tui::title`) and the window (`fn_gui::title`) print the same strings. The state
//! and the keys are `fn_tui::title`'s: this is only wording.

use fn_sim::embark::{MapSizeDef, SurveyMode};

/// The menu's welcome line.
pub const WELCOME: &str = "Welcome to the Four Nations.";
/// The nation step's prompt.
pub const NATION_PROMPT: &str = "Choose a nation (arrows or 1-4):";
/// The Load step with nothing saved.
pub const NO_SAVES: &str = "No saves yet.";

/// The seed step's heading: `Fire Nation — new game:`.
pub fn seed_heading(nation_name: &str) -> String {
    format!("{nation_name} — new game:")
}

/// The seed step's four row labels, without their colon.
pub const SEED_LABEL: &str = "Seed";
pub const SIZE_LABEL: &str = "Size";
pub const SURVEY_LABEL: &str = "Survey";
pub const DIFFICULTY_LABEL: &str = "Difficulty";

/// What the Survey row's value reads.
pub fn survey_name(mode: SurveyMode) -> &'static str {
    match mode {
        SurveyMode::Exact => "Exact",
        SurveyMode::Rough => "Rough",
    }
}

/// The hint under the seed step while the Seed row is highlighted.
pub const SEED_HINT: &str = "Type digits, Backspace to edit.";

/// The hint while the Size row is highlighted.
pub fn size_hint(size: &MapSizeDef) -> String {
    format!("{}: {}x{} tiles.", size.name, size.width, size.height)
}

/// The hint while the Survey row is highlighted.
pub fn survey_hint(mode: SurveyMode) -> &'static str {
    match mode {
        SurveyMode::Exact => "Exact: you'll see exactly what's at every site.",
        SurveyMode::Rough => "Rough: you won't know exactly what's there until you arrive.",
    }
}

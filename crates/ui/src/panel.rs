//! The Seal side panel as data (docs/design/side-panel-v2.md, #416; #873): its header and
//! sections, each a title, a right-hand key and rows of toned text ([`crate::rows::Row`]), built
//! from a session and the frontend's own [`UiState`] with no width in sight. A frontend lays the
//! model out: `fn_tui::panel` wraps, fits and trims it to a column of cells; a window frontend
//! paints the same words in its own way.
//!
//! What is *not* here is everything that depends on room: the wrapping of an alert or of a
//! tool's description, how many materials share a row, what folds into "and 3 more, press L",
//! and which rows a short panel gives up (docs/design/side-panel-v2.md "Zone 1"). The model keeps
//! the text whole and says which parts are which (an alert is an [`AlertEntry`], a paragraph is
//! [`ToolLine::Text`]); the inspector, the panel's last section, is `crate::inspector`'s (#882).

use fn_sim::building::BuildingId;

use crate::alerts::Category;
use crate::bend;
use crate::build;
use crate::endeavor::{self, EndeavorStatus};
use crate::rows::{Marks, Row, Tone, clock, meter};
use crate::screens::kind_tone;
use crate::session::Session;
use crate::state::UiState;
use crate::toolpanel::{self, Emphasis, Line, ToolState};
use crate::tools::Tool;
use crate::view::{
    Activity, SeasonView, SideView, activity_counts, beast_count, food_in_stores, materials,
    need_flags, season_view, side_view,
};

/// A research bar is this many cells.
pub const BAR_CELLS: usize = 10;

/// The Settlement section's heading and its key.
pub const SETTLEMENT_TITLE: &str = "SETTLEMENT";
pub const SETTLEMENT_KEY: &str = "[N]";
/// The Alerts section's heading.
pub const ALERTS_TITLE: &str = "ALERTS";

/// The whole panel but the inspector (`crate::inspector`).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SidePanel {
    /// The frame's top edge: the nation between two seals ("◆ EARTH KINGDOM ◆"), no padding.
    pub title: String,
    /// The frame's bottom edge: "seed 7".
    pub footer: String,
    /// Two rows (#654): run/pause and speed, then the clock.
    pub header: Vec<Row>,
    pub settlement: Settlement,
    pub alerts: AlertsSection,
    /// `None` in Select mode with the tool list closed: there is no section to show.
    pub tool: Option<ToolSection>,
    /// The building the Build tool would place, for the inspector (`side_view`'s own choice).
    pub choice: Option<BuildingId>,
}

/// The Settlement glance (#416, docs/design/side-panel-v2.md "Zone 1"): never empty, never
/// trimmed, so its heading is always on screen.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Settlement {
    pub title: String,
    pub right: String,
    /// "People N" left, "Idle N" right (#416).
    pub people: Row,
    /// "Beasts 2", "Hungry 1" in Warn on the right; `None` with no beast (#699).
    pub beasts: Option<Row>,
    /// "Hungry 2  Exhausted 1", each word in its own tone; `None` with nobody flagged (#310).
    pub need_flags: Option<Row>,
    /// "Food 159 · 13 each" (#416, #663).
    pub food: Row,
    /// "Winter: mild"; `None` with seasons off (#663).
    pub winter: Option<Row>,
    /// Non-food items in the item data's order, each "Wood 149", for the frontend to pack into
    /// as many rows as it has room for ([`fold_kinds`] says what a dropped tail is called).
    pub materials: Vec<String>,
    /// Target, bar and percent, or "Research: none set" in Warn (#416).
    pub research: Row,
    /// Under it, indented, while the front tech's item bill is short (#729) or it waits on a
    /// condition that fell (`research_notes`).
    pub research_bill: Vec<Row>,
    /// "Gate 3" with `[G]`, then what is still needed (#235, #242, #663); empty with no
    /// endeavor in the data.
    pub gate: Vec<Row>,
}

/// The Alerts section (#656): the open alerts, most urgent first, unwrapped.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct AlertsSection {
    pub title: String,
    /// "[L]", or "2 · [L]" with two alerts open.
    pub right: String,
    pub entries: Vec<AlertEntry>,
}

/// One open alert, or a notice (news that is not a problem), in full.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct AlertEntry {
    pub text: String,
    pub tone: Tone,
    /// Counted in the heading; a notice is not.
    pub alert: bool,
}

/// The Tool section (#657): the active tool's groups of lines, or the tool list (`/`, #664).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ToolSection {
    /// The tool's name, "Harvest · repeating" while its `O` toggle is on, or "DESIGNATE" for
    /// the list.
    pub title: String,
    /// "[H]", or "[/]" for the list.
    pub right: String,
    pub tool: Tool,
    /// The tool list is open: the section is that list, in the plain heading tone.
    pub list: bool,
    /// The standing-order or teardown toggle is on: the title says so, and Harvest's tint
    /// follows.
    pub repeating: bool,
    /// Groups of lines, blank rows between them when there is room; the Build tool's building
    /// or the Bend tool's ability is the second group.
    pub blocks: Vec<Vec<ToolLine>>,
    /// How many of the first groups are never dropped whole: what the tool does, and the
    /// building or ability it is set to.
    pub keep: usize,
}

/// One line of the Tool section, with every tone decided.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum ToolLine {
    /// A paragraph, wrapped to the room there is.
    Text(String, Tone),
    /// A toggle: "Standing order: off", `value` in its own tone; wrapped as plain text when the
    /// pair is too wide ([`ToolLine::setting_text`]).
    Setting { label: String, value: String, value_tone: Tone },
    /// A row of the tool list (#664): the key, the name, and what it is for (cut to the room),
    /// in the tones of [`ChoiceTones`].
    Choice { selected: bool, key: char, name: String, what: String, tones: ChoiceTones },
    /// The Build tool's chosen building, its cost on the right: a frontend too narrow for both
    /// halves puts them on two rows.
    Building(Row),
    /// The Bend tool's current ability and its casters on the right: one row.
    Ability(Row),
}

impl ToolLine {
    /// A setting's "label: value" as one string.
    pub fn setting_text(label: &str, value: &str) -> String {
        format!("{label}: {value}")
    }
}

/// The tones of a tool-list row's parts: the pointer (`head`), key, name and description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ChoiceTones {
    pub head: Tone,
    pub key: Tone,
    pub name: Tone,
    pub what: Tone,
}

impl ChoiceTones {
    /// A tool that can't be used yet is dim from end to end; the chosen row is the title.
    pub fn of(usable: bool, selected: bool) -> Self {
        let head = if selected { Tone::Title } else { Tone::Text };
        let (key, name, what) = match (usable, selected) {
            (false, _) => (Tone::Dim, Tone::Dim, Tone::Dim),
            (true, true) => (Tone::Accent, Tone::Title, Tone::Text),
            (true, false) => (Tone::Accent, Tone::Text, Tone::Dim),
        };
        Self { head, key, name, what }
    }
}

/// What a dropped tail of materials is called on the last row shown: "  +3".
pub fn fold_kinds(dropped: usize) -> String {
    format!("  +{dropped}")
}

/// The last line of the Alerts section when some are folded away.
pub fn fold_alerts(hidden: usize) -> String {
    format!("and {hidden} more, press L")
}

impl SidePanel {
    /// The panel for `session` as `ui` has it. `winter_food_per_head` is the presentation
    /// threshold below which autumn's and winter's food warns (`data/tui.toml`).
    pub fn build(
        session: &Session,
        ui: &UiState,
        marks: &Marks,
        winter_food_per_head: u32,
    ) -> Self {
        let view = side_view(session, ui.tool, &ui.notice, ui.choice, &ui.alerts);
        let season = season_view(session, winter_food_per_head);
        Self {
            title: format!("{0} {1} {0}", marks.seal, view.nation.to_uppercase()),
            footer: format!("seed {}", view.seed),
            header: header(&view, marks),
            settlement: settlement(session, &view, season.as_ref(), marks),
            alerts: alerts(ui, marks),
            tool: tool_section(session, ui, &view, marks),
            choice: view.choice,
        }
    }
}

/// The People section's header count (#248, docs/design/endgame.md "The transformation"): the
/// plain total while the settlement is one nation, else the total with the breakdown by nation
/// ("12 (Water 7, Earth 3, Fire 2)"), in `Nation::ALL`'s order (`view::population_by_nation`).
fn population_label(view: &SideView) -> String {
    if view.population.len() <= 1 {
        return view.people.to_string();
    }
    let by_nation: Vec<String> = view.population.iter().map(|(n, c)| format!("{n} {c}")).collect();
    format!("{} ({})", view.people, by_nation.join(", "))
}

/// The header (docs/design/side-panel.md "Sections, top to bottom", #654): two rows with the same
/// shape from the first tick on. The run state and the speed ("▶ Running x1", "‖ Paused x1"),
/// then the game's one clock (`crate::time::clock_text`): the season, the year and the time left
/// in it ("Spring Y2 · 15:00 left"), or the day with seasons off. The pair doesn't fit one row of
/// a 32-column panel. The seed is the panel's own bottom edge, and `Space + -` is the status
/// line's and the key help's.
fn header(view: &SideView, marks: &Marks) -> Vec<Row> {
    let (icon, tone) =
        if view.paused { (marks.pause, Tone::Paused) } else { (marks.run, Tone::Running) };
    let word = if view.paused { "Paused" } else { "Running" };
    vec![
        Row::text(format!("{icon} {word} x{}", view.speed), tone),
        Row::text(clock(&view.clock, marks), Tone::Text),
    ]
}

/// The Settlement glance's People row (#416, docs/design/side-panel-v2.md "Zone 1"): "People N"
/// left, "Idle N" right; the activity breakdown (Haul, Craft, ...) moved to the Nation screen
/// (`N`, #308), which already groups settlers the same way (`view::activity_counts`).
fn people_row(session: &Session, view: &SideView) -> Row {
    let idle = activity_counts(session)
        .into_iter()
        .find(|&(a, _)| a == Activity::Idle)
        .map_or(0, |(_, n)| n);
    Row::pair(
        format!("People {}", population_label(view)),
        Tone::Text,
        format!("Idle {idle}"),
        Tone::Text,
    )
}

/// The Settlement glance's Beasts row (#699): "Beasts 2" left, and "Hungry 1" right in the warn
/// tone while any rests at home with an empty bunker. `None` with no beast, which hides the row.
fn beasts_row(session: &Session) -> Option<Row> {
    let count = beast_count(session);
    if count.total == 0 {
        return None;
    }
    let left = format!("Beasts {}", count.total);
    Some(if count.hungry == 0 {
        Row::text(left, Tone::Text)
    } else {
        Row::pair(left, Tone::Text, format!("Hungry {}", count.hungry), Tone::Warn)
    })
}

/// The tone a need-flag word wears (#310): the "bad" half of each `view::need_word` pair is
/// `Tone::Bad`, the "warn" half `Tone::Warn`.
fn need_flag_tone(word: &str) -> Tone {
    match word {
        "Starving" | "Parched" | "Exhausted" | "Dying" => Tone::Bad,
        _ => Tone::Warn,
    }
}

/// The People section's need-flags row (#310): "Hungry 2  Exhausted 1", each word in its own
/// tone. `None` when nobody is flagged, which hides the row.
fn need_flags_row(session: &Session) -> Option<Row> {
    let flags = need_flags(session);
    if flags.is_empty() {
        return None;
    }
    let mut spans = Vec::new();
    for (i, (word, n)) in flags.iter().enumerate() {
        if i > 0 {
            spans.push(("  ".to_owned(), Tone::Text));
        }
        spans.push((format!("{word} {n}"), need_flag_tone(word)));
    }
    Some(Row::spans(spans))
}

/// The Settlement glance's Food row (#416, #663, docs/design/side-panel-v2.md "Zone 1"): the
/// total in stores and each settler's share of it ("Food 159 · 13 each").
fn food_row(session: &Session, view: &SideView, marks: &Marks) -> Row {
    let food = food_in_stores(session);
    let each = food / (view.people as u32).max(1);
    Row::text(format!("Food {food} {} {each} each", marks.dot), Tone::Text)
}

/// The Settlement glance's winter row (#416, #663), with seasons on: the climate band's winter
/// effects, labelled ("Winter: mild", "Winter: cold, frost"), Warn-toned exactly when
/// `season.food_warn` is (autumn or winter, below `winter_food_per_head`). Its own row, since a
/// labelled outlook no longer fits beside the Food row. `None` with seasons off.
fn winter_row(season: Option<&SeasonView>) -> Option<Row> {
    let s = season.filter(|s| !s.outlook.is_empty())?;
    let tone = if s.food_warn { Tone::Warn } else { Tone::Text };
    Some(Row::text(format!("Winter: {}", s.outlook), tone))
}

/// The Settlement glance's Research row (#416, docs/design/side-panel-v2.md "Zone 1"): the
/// target's name, its bar and its percent, all on one row ("Masonry ██████░░░░ 68%"). With no
/// target, "Research: none set" in Warn -- the state that most needs the player's attention, not
/// a dim aside. With an Archive standing and no target it reads the scribing progress instead
/// ("Copying scrolls: 90/150").
fn research_row(view: &SideView, marks: &Marks) -> Row {
    let r = &view.research;
    // An Archive with no target: the scribing counter takes the row (#712).
    if let Some(text) = &r.copying {
        // The paused text fills the panel's width, so it goes without its key.
        return if text.chars().count() > 23 {
            Row::text(text.clone(), Tone::Text)
        } else {
            Row::pair(text.clone(), Tone::Text, "[T]", Tone::Dim)
        };
    }
    if r.frac == 0.0 && r.label == "No target set" {
        return Row::pair("Research: none set", Tone::Warn, "[T]", Tone::Dim);
    }
    let [full, empty, flag] = meter(r.frac, BAR_CELLS, Tone::Accent, false, marks);
    Row::spans(vec![
        (format!("{} ", r.label), Tone::Strong),
        full,
        empty,
        flag,
        (format!("{:>3.0}%", r.frac * 100.0), Tone::Text),
    ])
}

/// The Settlement glance's Gate rows (#416, #235, #242, #663): the signs met ("Gate 3 of 9
/// signs") with `[G]`, then under it what the gate still needs ("Needs 4 more people",
/// `endeavor::gate_detail`) while any sign is missing; once nothing is missing, the line the old
/// Endeavor section's bar used to sit above (signs met, Ready, or the endeavor's own stage).
fn gate_rows(e: &EndeavorStatus) -> Vec<Row> {
    let tone = if e.next.is_empty() { Tone::Text } else { Tone::Warn };
    vec![
        Row::pair(format!("Gate {}", e.count), tone, "[G]", Tone::Dim),
        Row::text(endeavor::gate_detail(&e.next, &e.line), tone),
    ]
}

fn settlement(
    session: &Session,
    view: &SideView,
    season: Option<&SeasonView>,
    marks: &Marks,
) -> Settlement {
    Settlement {
        title: SETTLEMENT_TITLE.to_owned(),
        right: SETTLEMENT_KEY.to_owned(),
        people: people_row(session, view),
        beasts: beasts_row(session),
        need_flags: need_flags_row(session),
        food: food_row(session, view, marks),
        winter: winter_row(season),
        materials: materials(session)
            .into_iter()
            .map(|(name, count)| format!("{name} {count}"))
            .collect(),
        research: research_row(view, marks),
        research_bill: research_notes(&view.research),
        gate: view.endeavor.as_ref().map(gate_rows).unwrap_or_default(),
    }
}

/// The rows under the Research row: the bill while it is short, or the condition the front tech
/// waits on, broken before its live count so it fits the panel:
/// `Waiting: 3 Fire residents` and `(2 live here)` (docs/design/horizons-phase-6.md, "Mockups").
fn research_notes(status: &crate::research::ResearchStatus) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(text) = &status.bill {
        let tone = if text.starts_with("Waiting") { Tone::Warn } else { Tone::Dim };
        rows.push(Row::text(format!("  {text}"), tone));
    }
    if let Some(text) = &status.waiting {
        match text.split_once(" (") {
            Some((head, count)) => {
                rows.push(Row::text(format!("  {head}"), Tone::Warn));
                rows.push(Row::text(format!("  ({count}"), Tone::Warn));
            }
            None => rows.push(Row::text(format!("  {text}"), Tone::Warn)),
        }
    }
    rows
}

/// The open alerts (`Alerts::open`, #656), most urgent first, with how many are open and the key
/// that lists them all in the heading.
fn alerts(ui: &UiState, marks: &Marks) -> AlertsSection {
    let entries: Vec<AlertEntry> = ui
        .alerts
        .open()
        .into_iter()
        .map(|t| {
            let alert = t.category == Category::Alert;
            let tone = if alert { Tone::Warn } else { kind_tone(t.kind) };
            AlertEntry { text: t.text, tone, alert }
        })
        .collect();
    let open = entries.iter().filter(|a| a.alert).count();
    let right = if open == 0 { "[L]".to_owned() } else { format!("{open} {} [L]", marks.dot) };
    AlertsSection { title: ALERTS_TITLE.to_owned(), right, entries }
}

/// The Tool section's one row for the Build tool's current choice (#111, #285): its name marked
/// and its cost on the right; dimmed with the tech it still needs in place of the cost if it's
/// somehow locked (the Build modal only ever lets the player choose an unlocked one).
fn picker_row(row: &build::PickerRow, chosen: Option<BuildingId>, marks: &Marks) -> Row {
    let mark = if Some(row.id) == chosen { marks.pointer } else { " " };
    let (name_tone, right, right_tone) = match &row.requires {
        Some(requires) => (Tone::Dim, requires.clone(), Tone::Warn),
        None if Some(row.id) == chosen => (Tone::Title, row.cost.clone(), Tone::Dim),
        None => (Tone::Text, row.cost.clone(), Tone::Dim),
    };
    Row::pair(format!("{mark}{}", row.name), name_tone, right, right_tone)
}

/// The Tool section's one row while the Bend tool is active (#418, docs/design/side-panel-v2.md
/// "Zone 3"): the current ability (the one `B` would channel next, or cycles to, #248) and how
/// many in the settlement could cast it ("Hearthflame 20 benders", `World::caster_count`), or,
/// for a rank-gated ability, how many hold the rank itself ("2 adepts", "no master",
/// `text::caster_label`). `None` with nothing castable.
fn bend_ability_row(session: &Session, marks: &Marks) -> Option<Row> {
    let view = bend::view(session);
    let current = view.current()?;
    Some(Row::pair(
        format!("{}{}", marks.pointer, current.name),
        Tone::Title,
        current.casters.clone(),
        Tone::Dim,
    ))
}

fn tool_line(line: &Line) -> ToolLine {
    match line {
        Line::Text(text, emphasis) => {
            let tone = if *emphasis == Emphasis::Dim { Tone::Dim } else { Tone::Text };
            ToolLine::Text(text.clone(), tone)
        }
        Line::Setting { label, value } => ToolLine::Setting {
            label: label.clone(),
            value: value.clone(),
            value_tone: if value == "on" { Tone::Ok } else { Tone::Dim },
        },
        Line::Choice { selected, key, name, what, usable } => ToolLine::Choice {
            selected: *selected,
            key: *key,
            name: name.clone(),
            what: what.clone(),
            tones: ChoiceTones::of(*usable, *selected),
        },
    }
}

/// The Tool section (#657): `toolpanel`'s groups, with the Build tool's building or the Bend
/// tool's ability under the description, and the Gate tool's last result (the one tool with no
/// rectangle to report on) after them. `None` in Select mode with the tool list closed.
fn tool_section(
    session: &Session,
    ui: &UiState,
    view: &SideView,
    marks: &Marks,
) -> Option<ToolSection> {
    let list = ui.tool_list.is_some();
    if view.tool == Tool::Select && !list {
        return None;
    }
    let state = ToolState {
        tool: ui.tool,
        anchor: ui.anchor,
        cursor: ui.cursor,
        repeating: ui.repeating,
        teardown: ui.teardown,
        choice: ui.choice,
        marked: ui.marked_now(),
        list: ui.tool_list,
    };
    let mut blocks: Vec<Vec<ToolLine>> = toolpanel::blocks(session, &state)
        .iter()
        .map(|b| b.iter().map(tool_line).collect())
        .collect();
    let mut keep = usize::from(!blocks.is_empty());
    if !blocks.is_empty() {
        // The list is the whole section: no building or ability row under it.
        let picker = if list {
            None
        } else if ui.tool == Tool::Bend {
            bend_ability_row(session, marks).map(ToolLine::Ability)
        } else {
            view.picker
                .iter()
                .find(|row| Some(row.id) == view.choice)
                .map(|row| ToolLine::Building(picker_row(row, view.choice, marks)))
        };
        if let Some(row) = picker {
            blocks.insert(1, vec![row]);
            keep = 2;
        }
        if ui.tool == Tool::Gate && !list && !view.notice.is_empty() {
            blocks.push(vec![ToolLine::Text(view.notice.clone(), Tone::Text)]);
        }
    }
    let label = ui.repeating_label();
    let repeating = label.is_some();
    let (title, right) = if list {
        ("DESIGNATE".to_owned(), "[/]".to_owned())
    } else {
        (label.unwrap_or_else(|| view.tool.name().to_owned()), format!("[{}]", view.tool.key()))
    };
    Some(ToolSection { title, right, tool: view.tool, list, repeating, blocks, keep })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fn_sim::Nation;

    /// The Build tool's picker row (#111, #275): the chosen building is marked and keeps its
    /// cost; a locked one is dimmed, with the tech it still needs in place of the cost.
    #[test]
    fn a_picker_row_marks_the_chosen_building_and_dims_a_locked_one() {
        let session = Session::new(7, Nation::Earth, 48, 32);
        let rows = build::picker_rows(&session);
        let marks = Marks::new(false);
        let bed = rows.iter().find(|r| r.name == "Bed").unwrap();
        let chosen = picker_row(bed, Some(bed.id), &marks);
        assert_eq!(chosen.plain(), format!("{}Bed {}", marks.pointer, bed.cost));
        assert_eq!(chosen.left[0].1, Tone::Title);

        let unchosen = picker_row(bed, None, &marks);
        assert!(unchosen.plain().starts_with(' '), "{}", unchosen.plain());

        let locked = rows.iter().find(|r| r.locked()).expect("Earth has a locked building");
        let row = picker_row(locked, None, &marks);
        assert_eq!(row.left[0].1, Tone::Dim);
        assert_eq!(row.right.as_ref().unwrap().0, *locked.requires.as_ref().unwrap());
        assert_eq!(row.right.as_ref().unwrap().1, Tone::Warn);
    }
}

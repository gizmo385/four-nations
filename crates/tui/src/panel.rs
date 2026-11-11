//! The Seal side panel (docs/design/side-panel-v2.md, #416, which revises
//! docs/design/side-panel.md): a framed column with the nation's name on the top edge, the seed
//! on the bottom edge, and three zones, each a divider heading joined to the frame
//! (`crate::widgets::heading_row`), with no blank rows between them.
//!
//! **Zone 1, the header and the Settlement glance (fixed; never trimmed but for its one optional
//! row):** the header (two rows, #654: run/pause and speed, then the season, year and time
//! left, or the day with seasons off); one `SETTLEMENT` heading; People (the
//! total and Idle, #310 -- the activity breakdown moved to the Nation screen, `N`, #308); need
//! flags (hidden with nobody flagged); Food (the total and each settler's share); the winter row;
//! Materials (non-food items, packed, `+N` fold, a second row with spare height); Research
//! (target, bar, percent, one row; "Research: none set" in Warn with no target); the Gate (the
//! signs met and what is still needed, #235, #242; hidden with no endeavor in the
//! data). The only thing this zone ever grows into spare height is the Materials section's
//! second row; nothing in it is ever given up first.
//!
//! **Zone 2, Alerts** (one line each, newest last; always shows at least its heading and the
//! newest toast, #415). A routine success (an ability cast) is log-only, never a toast (#417);
//! repeats of the same kind and subject coalesce into one line with a count
//! (`fn_ui::alerts::Alerts::toasts`); a toast's own text drops map coordinates, which stay in
//! the log entry. Spare height goes here first, up to [`ALERTS_SHOWN`] rows, before anything
//! else in the panel grows.
//!
//! **Zone 3, the Tool and the inspector** (`crate::inspector`, #653): Tool (the active tool's
//! name in its own tint; hidden off a tool), then the inspector, the last section, which takes
//! every row left. The Tool section (#657, `fn_ui::toolpanel`) is groups of rows: what the tool
//! does; the Build tool's chosen building or, for Bend, the current ability and its bender count;
//! with a corner set, the rectangle's size and what applying it would mark, and once applied,
//! what it marked ("Marked 7 trees.") until the cursor moves or a new corner is set; and the
//! tool's toggles ("Standing order: off"). The inspector's heading names what it describes: the
//! selected settler (name and rank, with its task, carrying, meters, rank progress and tile in
//! the section, where the selection bar used to be), else whatever is under the keyboard cursor (a
//! settler, a building, "STOCKPILE", the terrain) with the cursor's position on the heading. The
//! Build tool's full picker moved to its own modal (`U`, `crate::build`, #285); the panel names
//! only the choice. The nation's modifiers moved to the Nation screen's Traits (`N`, #308). What
//! `Enter` does next in a tool is the active tool banner's own line (#307, `crate::banner`) and
//! the status line's keys, not the panel's.
//!
//! The panel is a fixed height, so a terminal too short for everything gives up rows in this
//! order: alerts beyond the newest, then the Tool section's groups from the last (never what the
//! tool does, nor the building or ability), and the inspector is left one row while a tool is
//! active, since the Tool section has first call on rows then. The header, the Settlement glance
//! in full, every heading and the inspector's first rows (a settler's task and meters, three rows
//! of a tile) always stay with no tool; the inspector drops its own rows from the bottom
//! (`Inspector::fitted`).

use fn_sim::building::BuildingId;
use fn_sim::world::Unit;
use fn_ui::bend;
use fn_ui::build;
use fn_ui::endeavor::EndeavorStatus;
use fn_ui::inspect::Hint;
use fn_ui::toolpanel::{self, Emphasis, Line as ToolLine, ToolState};
use fn_ui::view::{
    Activity, SeasonView, SideView, activity_counts, food_in_stores, materials, need_flags,
    season_view, side_view,
};
use fn_ui::{Session, Tool};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::color::{Palette, Tint};
use crate::inspector::{Inspector, fit_blocks, inspect};
use crate::state::UiState;
use crate::widgets::{Marks, Painter, Row, TitleAt, Tone, draw_frame, meter, wrap};

/// How an action hint reads in the terminal, if it can be taken yet.
///
/// * Selecting a unit names its key.
/// * Starting a tool names its key.
/// * Applying the active tool is the rectangle line ([`rect_hint`]), which the active tool
///   banner shows in every tool (#307, `crate::banner`), so the tooltip's own hint adds nothing.
pub fn hint_text(hint: Hint) -> Option<String> {
    match hint {
        Hint::SelectUnit => Some("Press Enter to select".to_owned()),
        Hint::ApplyTool(_) => None,
        Hint::StartTool(tool) => Some(format!("Press {} for the {} tool", tool.key(), tool.name())),
    }
}

/// What `Enter` does next in the active tool: set a corner, or apply the tool to the rectangle
/// ("Enter: designate 5x3"). `None` in Select mode.
pub fn rect_hint(ui: &UiState) -> Option<String> {
    if ui.tool == Tool::Select {
        return None;
    }
    // The gate has no rectangle (#643): one `Enter` places it.
    if ui.tool == Tool::Gate {
        return Some("Enter: place the gate".to_owned());
    }
    Some(match ui.rect_size() {
        None => "Enter: set a corner".to_owned(),
        Some((w, h)) => format!("Enter: {} {w}x{h}", apply_verb(ui.tool)),
    })
}

/// The verb for applying a tool to a rectangle.
fn apply_verb(tool: Tool) -> &'static str {
    match tool {
        Tool::Harvest | Tool::Stockpile | Tool::Sow => "designate",
        Tool::CancelHarvest => "cancel",
        Tool::Clear | Tool::Deconstruct => "clear",
        Tool::Bend => "bend",
        Tool::Build => "build",
        Tool::Gate => "place",
        Tool::Select => "select",
    }
}

/// The most lines one paragraph of the Tool section wraps to (the description, a toggle): it
/// wraps rather than cuts, and the fitting drops whole groups of rows when the panel is short.
const TOOL_LINES: usize = 5;
/// One alert is one line (docs/design/side-panel.md "Sections, top to bottom"): the full text is
/// in the log (`L`).
const ALERT_LINES: usize = 1;
/// At most this many alert rows show at once, even with room for more (#417,
/// docs/design/side-panel-v2.md "Fitting": "Spare rows go, in order: Alerts up to 6..."). Alerts
/// are already given this much room before the fitting loop runs (`draw_panel`'s own `k.alerts`
/// starts at `d.alerts.len().min(ALERTS_SHOWN)`), so the Settlement glance's own growth (a
/// second Materials row) and Here only ever see what's left afterward.
const ALERTS_SHOWN: usize = 6;
/// At most this many Materials rows show at once, even with room for more (#416,
/// docs/design/side-panel-v2.md "Zone 1": "Spare height adds a second materials row").
const MATERIALS_ROWS_MAX: usize = 2;
/// A research bar is this many cells.
const BAR_CELLS: usize = 10;

/// A heading (`crate::widgets::heading_row`) and the rows under it.
struct Section {
    title: String,
    right: String,
    rows: Vec<Row>,
    /// Shown even with no rows (People, Endeavor, Tool, Here): their heading is still useful
    /// on its own (docs/design/side-panel.md "Sections, top to bottom").
    always: bool,
    /// The heading's title tone: `Tone::Heading` for every section but Tool, which wears the
    /// active tool's own tint instead (`Tone::Custom`).
    title_tone: Tone,
}

impl Section {
    fn new(title: impl Into<String>, right: impl Into<String>, rows: Vec<Row>) -> Self {
        Self {
            title: title.into(),
            right: right.into(),
            rows,
            always: false,
            title_tone: Tone::Heading,
        }
    }

    fn always(mut self) -> Self {
        self.always = true;
        self
    }

    fn tint(mut self, tone: Tone) -> Self {
        self.title_tone = tone;
        self
    }

    fn visible(&self) -> bool {
        self.always || !self.rows.is_empty()
    }
}

/// Everything the sections show, computed once; the fitting loop picks how much of it to use.
struct Data {
    view: SideView,
    /// The Settlement glance's People row (#416): "People N" left, "Idle N" right.
    people: Row,
    /// The need-flags row (#310): `None` when nobody is flagged, which hides the row.
    need_flags_row: Option<Row>,
    /// The Settlement glance's Food row (#416): the total and each settler's share.
    food: Row,
    /// The winter row (#663): the climate band's winter effects, labelled; `None` with seasons
    /// off.
    winter: Option<Row>,
    /// The Settlement glance's Materials rows (#416): non-food items packed into as few rows as
    /// fit the panel's width, each paired with how many distinct kinds it and every row before
    /// it account for, so a dropped tail folds into `+N` kinds. At most [`MATERIALS_ROWS_MAX`]
    /// of them.
    materials: Vec<(Row, usize)>,
    /// The Settlement glance's Research row (#416): short shape (target, bar, percent), or
    /// "Research: none set" in Warn with no target.
    research: Row,
    /// The Settlement glance's Gate rows (#416, #235, #242, #663): the signs met, then what is
    /// still needed; empty with no endeavor in the data.
    gate: Vec<Row>,
    /// The Tool section's rows (#657), in groups: what the tool does, the Build tool's chosen
    /// building or the Bend tool's current ability, then the rectangle's preview or what the
    /// last one marked, then the tool's toggles. Empty off a tool.
    tool: ToolRows,
    alerts: Vec<Row>,
    /// The last section (#653): what the player is looking at, named in its own heading.
    inspector: Inspector,
    /// "Harvest · repeating" while the standing-order toggle is on, "Cancel · teardown" while
    /// the Cancel tool's own toggle is (`UiState::repeating_label`, #277/#335); `None` the rest
    /// of the time.
    repeating_label: Option<String>,
}

/// How much of each trimmable block to show. The Settlement glance (#416) isn't knobbed: it
/// never gives up a row, and `materials` is the one thing it ever grows (`draw_panel`'s own
/// growth pass, after the shrink loop below settles).
#[derive(Clone, Copy)]
struct Knobs {
    alerts: usize,
    /// How many rows of `Data::tool` to show ([`fit_blocks`]).
    tool: usize,
    /// How many of `Data::materials` to show, up to [`MATERIALS_ROWS_MAX`]; the rest fold into
    /// `+N` kinds on the last shown row.
    materials: usize,
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
/// then the game's one clock (`fn_ui::time::clock_text`): the season, the year and the time left
/// in it ("Spring Y2 · 15:00 left"), or the day with seasons off. The pair doesn't fit one row of
/// a 32-column panel. The seed is the panel's own bottom edge, and `Space + -` is the status
/// line's and the key help's.
fn header(view: &SideView, marks: &Marks) -> Vec<Row> {
    let (icon, tone) =
        if view.paused { (marks.pause, Tone::Paused) } else { (marks.run, Tone::Running) };
    let word = if view.paused { "Paused" } else { "Running" };
    vec![
        Row::text(format!("{icon} {word} x{}", view.speed), tone),
        Row::text(crate::widgets::clock(&view.clock, marks), Tone::Text),
    ]
}

/// A channelling unit's status in place of the plain task text ("Channelling Hearthflame 1.2s"):
/// its ability and the time left, while it stands on the tile and bends (`World::bend_channels`,
/// #278). `None` while it walks there or does anything else, which keeps the plain task (the
/// inspector's own use, `crate::inspector`; the Nation screen's roster, `N`, #308, has its own).
pub fn channel_text(session: &Session, unit: &Unit) -> Option<String> {
    let defs = &session.defs;
    session.world.bend_channels(defs).into_iter().find(|c| c.unit == unit.id).map(|c| {
        format!("Channelling {} {:.1}s", defs.ability(c.ability).name, c.remaining_secs.max(0.0))
    })
}

/// The Settlement glance's People row (#416, docs/design/side-panel-v2.md "Zone 1"): "People N"
/// left, "Idle N" right; the activity breakdown (Haul, Craft, ...) moved to the Nation screen
/// (`N`, #308), which already groups settlers the same way (`fn_ui::view::activity_counts`).
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

/// The tone a need-flag word wears (#310): the "bad" half of each `fn_ui::view::need_word` pair
/// is `Tone::Bad`, the "warn" half `Tone::Warn`.
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

/// The Settlement glance's Materials rows (#416): non-food items ("Wood 149  Stone 395",
/// `fn_ui::view::materials`, in the item data's own order), packed into as few rows as fit
/// `width`, each paired with how many distinct kinds it and every row before it account for, so
/// a dropped tail folds into `+N` kinds (`sections`'s own job, since the fold has to land on
/// whichever row fitting actually keeps).
fn materials_rows(session: &Session, width: usize) -> Vec<(Row, usize)> {
    let mut rows = Vec::new();
    let mut line = String::new();
    let mut kinds = 0usize;
    let mut cumulative = 0usize;
    for (name, count) in materials(session) {
        let entry = format!("{name} {count}");
        if !line.is_empty() && line.chars().count() + 2 + entry.chars().count() > width {
            cumulative += kinds;
            rows.push((Row::text(std::mem::take(&mut line), Tone::Text), cumulative));
            kinds = 0;
        }
        if !line.is_empty() {
            line.push_str("  ");
        }
        line.push_str(&entry);
        kinds += 1;
    }
    if !line.is_empty() {
        cumulative += kinds;
        rows.push((Row::text(line, Tone::Text), cumulative));
    }
    rows
}

/// The Settlement glance's Research row (#416, docs/design/side-panel-v2.md "Zone 1"): the
/// target's name, its bar and its percent, all on one row ("Masonry ██████░░░░ 68%"). With no
/// target, "Research: none set" in Warn -- the state that most needs the player's attention, not
/// a dim aside (unlike the old two-row shape's "No target set" / "N pts banked").
fn research_row(view: &SideView, marks: &Marks) -> Row {
    let r = &view.research;
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
/// `fn_ui::endeavor::gate_detail`) while any sign is missing; once nothing is missing, the line
/// the old Endeavor section's bar used to sit above (signs met, Ready, or the endeavor's own
/// stage). Two rows, since the panel's width has no room for both on one. No endeavor in the data
/// hides them.
fn gate_rows(e: &EndeavorStatus) -> Vec<Row> {
    let tone = if e.next.is_empty() { Tone::Text } else { Tone::Warn };
    vec![
        Row::pair(format!("Gate {}", e.count), tone, "[G]", Tone::Dim),
        Row::text(fn_ui::endeavor::gate_detail(&e.next, &e.line), tone),
    ]
}

/// The Tool section's one row for the Build tool's current choice (#111, #285): its name marked
/// and its cost on the right; dimmed with the tech it still needs in place of the cost if it's
/// somehow locked (the Build modal only ever lets the player choose an unlocked one). A name and
/// cost too wide to share `width` ("Mine Shaft" and "10 Stone, 6 Wood, 2 Tool, 2x2") take a
/// row each, the cost right-aligned under the name, rather than the cost crowding the name out.
fn picker_rows(
    row: &build::PickerRow,
    chosen: Option<BuildingId>,
    marks: &Marks,
    width: usize,
) -> Vec<Row> {
    let mark = if Some(row.id) == chosen { marks.pointer } else { " " };
    let (name_tone, right, right_tone) = match &row.requires {
        Some(requires) => (Tone::Dim, requires.clone(), Tone::Warn),
        None if Some(row.id) == chosen => (Tone::Title, row.cost.clone(), Tone::Dim),
        None => (Tone::Text, row.cost.clone(), Tone::Dim),
    };
    let name = format!("{mark}{}", row.name);
    if name.chars().count() + 1 + right.chars().count() <= width {
        return vec![Row::pair(name, name_tone, right, right_tone)];
    }
    vec![Row::text(name, name_tone), Row::pair("", Tone::Dim, right, right_tone)]
}

/// The Tool section's one row while the Bend tool is active (#418, docs/design/side-panel-v2.md
/// "Zone 3"): the current ability (the one `B` would channel next, or cycles to, #248) and how
/// many in the settlement could cast it ("Hearthflame 20 benders", `World::caster_count`), or,
/// for a rank-gated ability, how many hold the rank itself ("2 adepts", "no master",
/// `fn_ui::text::caster_label`, docs/design/bending.md "Seeing it", #361) -- it stays listed
/// here even while nobody holds the rank (`usable_abilities` only gates on nation and tech, not
/// rank), so the player can see what's coming and cycle to it with `B`. The same shape the
/// Build tool's `picker_rows` gives its one chosen building. `B` already cycles, and the active
/// tool banner already says `B: next ability` (#307), so the rest of the list moved out rather
/// than grow a row per ability learned. Empty off the Bend tool, or with nothing castable, which
/// keeps the Build tool's `picker_rows` in the same slot.
fn bend_ability_rows(session: &Session, marks: &Marks) -> Vec<Row> {
    let view = bend::view(session);
    let Some(current) = view.current() else { return Vec::new() };
    vec![Row::pair(
        format!("{}{}", marks.pointer, current.name),
        Tone::Title,
        current.casters.clone(),
        Tone::Dim,
    )]
}

/// The Tool section's rows, in groups (`fn_ui::toolpanel`, #657), before the panel knows how
/// many it gets.
struct ToolRows {
    blocks: Vec<Vec<Row>>,
    /// How many of the first groups are never dropped whole: what the tool does, and the
    /// building or ability it is set to.
    keep: usize,
}

impl ToolRows {
    /// Every row, blank rows between the groups included.
    fn full(&self) -> usize {
        fit_blocks(&self.blocks, self.keep, usize::MAX).len()
    }

    /// The rows the section is never left without: the first `keep` groups.
    fn floor(&self) -> usize {
        self.blocks.iter().take(self.keep).map(Vec::len).sum()
    }
}

/// One line of the Tool section as rows `width` cells wide: text wraps, a toggle reads
/// "Standing order: off" with its state in the tone of on or off.
fn tool_line_rows(line: &ToolLine, width: usize, marks: &Marks) -> Vec<Row> {
    match line {
        ToolLine::Text(text, emphasis) => {
            let tone = if *emphasis == Emphasis::Dim { Tone::Dim } else { Tone::Text };
            wrap(text, width, TOOL_LINES, marks.ellipsis)
                .into_iter()
                .map(|l| Row::text(l, tone))
                .collect()
        }
        ToolLine::Setting { label, value } => {
            let text = format!("{label}: {value}");
            if text.chars().count() > width {
                return wrap(&text, width, TOOL_LINES, marks.ellipsis)
                    .into_iter()
                    .map(|l| Row::text(l, Tone::Text))
                    .collect();
            }
            let tone = if value == "on" { Tone::Ok } else { Tone::Dim };
            vec![Row::spans(vec![(format!("{label}: "), Tone::Text), (value.clone(), tone)])]
        }
    }
}

/// The Tool section's rows for the active tool: `fn_ui::toolpanel`'s groups as rows, with the
/// Build tool's building or the Bend tool's ability (`picker`) under the description, and the
/// Gate tool's last result (`notice`, the one tool with no rectangle to report on) after them.
fn tool_rows(
    session: &Session,
    ui: &UiState,
    picker: Vec<Row>,
    notice: &str,
    width: usize,
    marks: &Marks,
) -> ToolRows {
    let state = ToolState {
        tool: ui.tool,
        anchor: ui.anchor,
        cursor: ui.cursor,
        repeating: ui.repeating,
        teardown: ui.teardown,
        choice: ui.choice,
        marked: ui.marked_now(),
    };
    let mut blocks: Vec<Vec<Row>> = toolpanel::blocks(session, &state)
        .iter()
        .map(|b| b.iter().flat_map(|l| tool_line_rows(l, width, marks)).collect())
        .collect();
    if blocks.is_empty() {
        return ToolRows { blocks, keep: 0 };
    }
    let keep = if picker.is_empty() { 1 } else { 2 };
    if !picker.is_empty() {
        blocks.insert(1, picker);
    }
    if ui.tool == Tool::Gate && !notice.is_empty() {
        let text = ToolLine::Text(notice.to_owned(), Emphasis::Plain);
        blocks.push(tool_line_rows(&text, width, marks));
    }
    ToolRows { blocks, keep }
}

fn gather(
    session: &Session,
    ui: &UiState,
    marks: &Marks,
    width: usize,
    winter_food_per_head: u32,
) -> Data {
    let view = side_view(session, ui.tool, &ui.notice, ui.choice, &ui.alerts);
    let ellipsis = marks.ellipsis;
    // Just the chosen building (#285 moved the rest of the picker to the Build modal); the Bend
    // tool uses the same slot for its current ability's one row instead (#248, #418). They sit
    // in the Tool section under what the tool does (#657).
    let picker: Vec<Row> = if ui.tool == Tool::Bend {
        bend_ability_rows(session, marks)
    } else {
        view.picker
            .iter()
            .find(|row| Some(row.id) == view.choice)
            .map(|row| picker_rows(row, view.choice, marks, width))
            .unwrap_or_default()
    };
    let tool = tool_rows(session, ui, picker, &view.notice, width, marks);
    let alerts = ui
        .alerts
        .toasts()
        .into_iter()
        .flat_map(|e| {
            let tone = crate::screens::kind_tone(e.kind);
            wrap(&e.text, width, ALERT_LINES, ellipsis).into_iter().map(move |l| Row::text(l, tone))
        })
        .collect();
    let season = season_view(session, winter_food_per_head);
    let people = people_row(session, &view);
    let food = food_row(session, &view, marks);
    let winter = winter_row(season.as_ref());
    let research = research_row(&view, marks);
    let gate = view.endeavor.as_ref().map(gate_rows).unwrap_or_default();
    Data {
        people,
        need_flags_row: need_flags_row(session),
        food,
        winter,
        materials: materials_rows(session, width),
        research,
        gate,
        tool,
        alerts,
        inspector: inspect(session, ui, view.choice, width, marks),
        repeating_label: ui.repeating_label(),
        view,
    }
}

/// The Settlement glance's own rows (#416, docs/design/side-panel-v2.md "Zone 1"), in order:
/// People, need flags (if any), Food, Materials (`k.materials` of them, the rest folded into
/// `+N` kinds on the last one shown), Research, and the Gate (if the data has an endeavor).
/// Never empty, so [`Section::visible`] always keeps the SETTLEMENT heading on screen.
fn settlement_rows(d: &Data, k: Knobs) -> Vec<Row> {
    let mut rows = vec![d.people.clone()];
    if let Some(flags) = &d.need_flags_row {
        rows.push(flags.clone());
    }
    rows.push(d.food.clone());
    rows.extend(d.winter.clone());
    let shown = k.materials.min(d.materials.len());
    rows.extend(d.materials.iter().take(shown).map(|(row, _)| row.clone()));
    if shown < d.materials.len() {
        let accounted = if shown == 0 { 0 } else { d.materials[shown - 1].1 };
        let total = d.materials.last().map_or(0, |&(_, n)| n);
        let dropped = total.saturating_sub(accounted);
        match rows.last_mut() {
            Some(last) => last.left.push((format!("  +{dropped}"), Tone::Dim)),
            None => rows.push(Row::text(format!("+{dropped}"), Tone::Dim)),
        }
    }
    rows.push(d.research.clone());
    rows.extend(d.gate.iter().cloned());
    rows
}

/// The active tool's tint ([`crate::banner::tint`]'s own logic, over `Data` rather than
/// `UiState`, since `sections` doesn't otherwise need the latter): `None` off a tool (Select).
fn tool_tint(d: &Data) -> Option<Tint> {
    if d.view.tool == Tool::Harvest && d.repeating_label.is_some() {
        Some(Tint::RepeatingHarvest)
    } else {
        Tint::for_tool(d.view.tool)
    }
}

/// The sections as the knobs allow, in order, with the hidden ones left out.
fn sections(d: &Data, k: Knobs, palette: &Palette) -> Vec<Section> {
    // One `SETTLEMENT` heading, `[N]` on the right, replaces the old PEOPLE, season, STORES,
    // RESEARCH and ENDEAVOR headings (#416): the zone never gives up a row, so it's always
    // visible and never knobbed down past `settlement_rows`'s own growth (the Materials section's
    // second row).
    let settlement = Section::new("SETTLEMENT", "[N]", settlement_rows(d, k)).always();
    // The newest alerts are the last rows; a short panel keeps those.
    let skip = d.alerts.len().saturating_sub(k.alerts);
    let alerts = Section::new("ALERTS", "[L]", d.alerts.iter().skip(skip).cloned().collect());
    // Just above Here, in the tool's own tint (docs/design/side-panel.md "Headings"). Select
    // mode has no tool to talk about.
    let tool = (d.view.tool != Tool::Select).then(|| {
        let rows = fit_blocks(&d.tool.blocks, d.tool.keep, k.tool);
        // "Harvest · repeating" or "Cancel · teardown" while the tool's own `O` toggle is on
        // (#277/#335), else just the tool's name.
        let name = d.repeating_label.clone().unwrap_or_else(|| d.view.tool.name().to_owned());
        let tone = tool_tint(d)
            .map_or(Tone::Heading, |t| Tone::Custom(palette.color(palette.tint_rgb(t))));
        Section::new(name, format!("[{}]", d.view.tool.key()), rows).always().tint(tone)
    });
    // The inspector, which comes last and takes every row left (#653), is drawn after these.
    let all = [Some(settlement), Some(alerts), tool];
    all.into_iter().flatten().filter(Section::visible).collect()
}

/// The rows the header and `sections` take, and the inspector's own heading and the rows it is
/// never left without ([`Inspector::floor`]): one row per heading, no blank rows between them
/// (docs/design/side-panel.md "Decisions in brief": "the blank row between sections goes").
fn height(header_rows: usize, sections: &[Section], inspector: &Inspector, tool: bool) -> usize {
    // With a tool active the Tool section has first call on rows, and the inspector is left
    // one (#657).
    let floor = if tool { inspector.floor_under_tool() } else { inspector.floor() };
    header_rows + sections.iter().map(|s| 1 + s.rows.len()).sum::<usize>() + 1 + floor
}

/// Draws the panel into `area`: the frame (the nation on top, the seed on the bottom edge), then
/// the header and the sections, each a divider heading (`Painter::heading`) joined to the frame.
pub fn draw_panel(
    session: &Session,
    ui: &UiState,
    palette: &Palette,
    area: Rect,
    buf: &mut Buffer,
) {
    let marks = Marks::new(palette.ascii);
    let inner = Rect::new(area.left() + 2, area.top() + 1, area.width - 4, area.height - 2);
    let winter_food_per_head = palette.glyphs.season.winter_food_per_head;
    let d = gather(session, ui, &marks, usize::from(inner.width), winter_food_per_head);
    let title = format!(" {0} {1} {0} ", marks.seal, d.view.nation.to_uppercase());
    let footer = format!("seed {}", d.view.seed);
    draw_frame(buf, area, palette, &title, TitleAt::Centre, &footer);

    // Fit: the Settlement glance (#416) never gives up a row, so only Alerts and the Tool's
    // groups (from the last) shrink here, until the inspector keeps the rows it is never left
    // without; the glance's own growth -- a second Materials row -- is a separate pass below,
    // once this settles.
    let head = header(&d.view, &marks);
    let materials_cap = d.materials.len().min(MATERIALS_ROWS_MAX);
    // Fitting gives up alerts beyond the newest, but never the newest itself while at least one
    // is live (#415): the ALERTS heading on its own, with every row gone, would otherwise read as
    // "no alerts" instead of "a live one that got clipped".
    let alerts_floor = usize::from(!d.alerts.is_empty());
    let mut k = Knobs {
        alerts: d.alerts.len().min(ALERTS_SHOWN),
        tool: d.tool.full(),
        materials: materials_cap.min(1),
    };
    let tool_active = d.view.tool != Tool::Select;
    let needs = |k: Knobs| height(head.len(), &sections(&d, k, palette), &d.inspector, tool_active);
    while needs(k) > usize::from(inner.height) {
        if k.alerts > alerts_floor {
            k.alerts -= 1;
        } else if k.tool > d.tool.floor() {
            k.tool -= 1;
        } else {
            break;
        }
    }
    // Spare height adds the Materials section's second row before anything else in this zone
    // grows (#416): the only growth `settlement_rows` ever has, since Alerts' own growth beyond
    // today's cap is #417. Whatever it leaves is the inspector's.
    if k.materials < materials_cap {
        let mut grown = k;
        grown.materials += 1;
        if needs(grown) <= usize::from(inner.height) {
            k = grown;
        }
    }

    let mut p = Painter::new(buf, inner, palette);
    for row in &head {
        p.row(row);
    }
    for section in sections(&d, k, palette) {
        p.heading(area, &section.title, &section.right, section.title_tone);
        for row in &section.rows {
            p.row(row);
        }
    }
    // The inspector: its heading names what it describes, and it gets every row left (#653).
    let left = usize::from(inner.height.saturating_sub(p.row + 1));
    let fitted = d.inspector.fitted(left);
    p.heading(area, &fitted.title, &fitted.right, Tone::Heading);
    for row in &fitted.rows {
        p.row(row);
    }
}

/// The `Color` of the panel background, for tests.
pub fn panel_bg(palette: &Palette) -> Color {
    palette.token(palette.theme.panel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rectangle_hint_follows_the_corner() {
        use fn_sim::{Nation, Pos};
        let mut ui = UiState::new(&Session::new(7, Nation::Earth, 48, 32), (120, 40));
        assert_eq!(rect_hint(&ui), None, "Select mode has none");
        ui.set_tool(Tool::Harvest);
        assert_eq!(rect_hint(&ui).as_deref(), Some("Enter: set a corner"));
        ui.anchor = Some(Pos::new(3, 3));
        ui.cursor = Pos::new(7, 5);
        assert_eq!(rect_hint(&ui).as_deref(), Some("Enter: designate 5x3"));
        ui.set_tool(Tool::CancelHarvest);
        assert_eq!(rect_hint(&ui).as_deref(), Some("Enter: set a corner"), "a new tool drops it");
    }

    #[test]
    fn selecting_a_unit_has_a_hint() {
        assert_eq!(hint_text(Hint::SelectUnit).as_deref(), Some("Press Enter to select"));
        assert_eq!(hint_text(Hint::ApplyTool(Tool::Harvest)), None);
        assert_eq!(
            hint_text(Hint::StartTool(Tool::Harvest)).as_deref(),
            Some("Press H for the Harvest tool")
        );
    }

    /// The Build tool's picker row (#111, #275): the chosen building is marked and keeps its
    /// cost; a locked one is dimmed, with the tech it still needs in place of the cost.
    #[test]
    fn a_picker_row_marks_the_chosen_building_and_dims_a_locked_one() {
        use fn_sim::Nation;
        let session = Session::new(7, Nation::Earth, 48, 32);
        let rows = build::picker_rows(&session);
        let marks = Marks::new(false);
        let bed = rows.iter().find(|r| r.name == "Bed").unwrap();
        let one = |r, chosen| {
            let mut rows = picker_rows(r, chosen, &marks, 60);
            assert_eq!(rows.len(), 1);
            rows.remove(0)
        };
        let chosen = one(bed, Some(bed.id));
        assert_eq!(chosen.plain(), format!("{}Bed {}", marks.pointer, bed.cost));
        assert_eq!(chosen.left[0].1, Tone::Title);

        let unchosen = one(bed, None);
        assert!(unchosen.plain().starts_with(' '), "{}", unchosen.plain());

        let locked = rows.iter().find(|r| r.locked()).expect("Earth has a locked building");
        let row = one(locked, None);
        assert_eq!(row.left[0].1, Tone::Dim);
        assert_eq!(row.right.as_ref().unwrap().0, *locked.requires.as_ref().unwrap());
        assert_eq!(row.right.as_ref().unwrap().1, Tone::Warn);
    }

    /// A name and cost too wide to share the panel take a row each, so the name isn't crowded
    /// out by the cost (the Mine Shaft's "10 Stone, 6 Wood, 2 Tool, 2x2").
    #[test]
    fn a_picker_row_too_wide_for_the_panel_puts_the_cost_on_its_own_row() {
        use fn_sim::Nation;
        let session = Session::new(7, Nation::Earth, 48, 32);
        let rows = build::picker_rows(&session);
        let marks = Marks::new(false);
        let bed = rows.iter().find(|r| r.name == "Bed").unwrap();
        let width = 1 + "Bed".len() + 1 + bed.cost.len() - 1;
        let split = picker_rows(bed, Some(bed.id), &marks, width);
        assert_eq!(split.len(), 2);
        assert_eq!(split[0].plain(), format!("{}Bed", marks.pointer));
        assert_eq!(split[1].right.as_ref().unwrap().0, bed.cost);
    }

    /// A unit standing on its tile and channelling (#278) names the ability and the time left;
    /// one that is still walking there, or doing anything else, keeps the plain task.
    #[test]
    fn a_channelling_unit_names_its_ability_and_the_time_left() {
        use fn_sim::Nation;
        use fn_sim::world::Command;

        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let pos = s.world.units[0].pos;
        let id = s.world.units[0].id;
        let ability = fn_ui::tools::ability_for_nation(&s.defs, Nation::Fire).unwrap();
        s.apply(Command::Bend { ability, pos }).unwrap();

        let idle = s.world.units.iter().find(|u| u.id != id).unwrap();
        assert_eq!(channel_text(&s, idle), None, "not channelling at all");

        for _ in 0..2000 {
            s.tick();
            if s.world.bend_channels(&s.defs).iter().any(|c| c.unit == id) {
                break;
            }
        }
        let unit = s.world.units.iter().find(|u| u.id == id).unwrap();
        let text = channel_text(&s, unit).expect("the bender should be channelling by now");
        assert!(text.starts_with("Channelling Hearthflame "), "{text}");
        assert!(text.ends_with('s'), "{text}");
        let left: f32 = text["Channelling Hearthflame ".len()..text.len() - 1].parse().unwrap();
        assert!((0.0..=4.0).contains(&left), "{text}");
    }
}

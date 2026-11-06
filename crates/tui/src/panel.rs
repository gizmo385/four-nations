//! The Seal side panel (docs/design/side-panel.md, which revises docs/design/tui.md): a framed
//! column with the nation's name on the top edge, the seed on the bottom edge, and sections in
//! the doc's order, each a divider heading joined to the frame (`crate::widgets::heading_row`)
//! plus a short, fixed shape, with no blank rows between them: the header (one row: run/pause,
//! speed and the clock), People (activity counts and need flags, #310, not a line per settler --
//! the roster is the Nation screen's, `N`, #308), Season (#299, under People: food per head and
//! the band's winter effects; hidden with seasons off), Stores (at most two rows), Research
//! (target, bar, rate; one row when short), Endeavor (the end game's progress, #235, #242, its
//! count on the heading; hidden with no endeavor in the data), Alerts (one line each, newest
//! last), Tool (just above Cursor: the active tool's name in its own tint, the Build tool's
//! chosen building or the Bend abilities, then the last rectangle's result; hidden off a tool)
//! and Cursor (what is under the keyboard cursor). The Build tool's full picker moved to its own
//! modal (`U`, `crate::build`, #285); the panel names only the choice. The nation's modifiers
//! moved to the Nation screen's Traits (`N`, #308). What `Enter` does next in a tool, and the
//! tool's own hint, are the active tool banner's own lines (#307, `crate::banner`) and the status
//! line's, not the panel's.
//!
//! The panel is a fixed height, so a terminal that is too short for every section gives up rows
//! in docs/design/side-panel.md's own order: Endeavor's one row, alerts beyond the newest,
//! Research's second row, Stores' second row, People's group rows past the first (folded into
//! `+N`), Season's "Winter here" line, the Tool section's result beyond two lines, then the Bend
//! ability list down to the current ability. The header, every section's heading and three
//! Cursor rows always stay.

use fn_sim::Pos;
use fn_sim::building::BuildingId;
use fn_sim::world::{CommandError, Unit};
use fn_ui::build;
use fn_ui::inspect::{Hint, Line, tooltip_lines};
use fn_ui::text::describe_stack;
use fn_ui::tools::{bend_ability, usable_abilities};
use fn_ui::view::{
    SeasonView, SideView, activity_counts, need_flags, season_view, side_view, stores,
};
use fn_ui::{Session, Tool};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::color::{Palette, Tint};
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
        Tool::Bend => "bend",
        Tool::Build => "build",
        Tool::Select => "select",
    }
}

/// The most lines the Tool section's last result (`notice`) wraps to before fitting trims it
/// (down to [`NOTICE_FLOOR`]). The tool's own hint is the status line's and the active tool
/// banner's now (#307), not a row here.
const NOTICE_LINES: usize = 5;
/// Fitting never takes the Tool section's result below this many lines (docs/design/side-panel.md
/// "Fitting": "the Tool notice past two lines").
const NOTICE_FLOOR: usize = 2;
/// One alert is one line (docs/design/side-panel.md "Sections, top to bottom"): the full text is
/// in the log (`L`).
const ALERT_LINES: usize = 1;
/// At most this many alert rows show at once, even with room for more.
const ALERTS_SHOWN: usize = 3;
/// At most this many stores rows show at once, even with room for more.
const STORES_SHOWN: usize = 2;
/// At most this many lines a Cursor Detail wraps to (docs/design/side-panel.md: Cursor "gets the
/// spare rows"), so a nation name plus a position ("Air Nomads - Foraging at (79, 75)") never
/// silently loses its tail to [`crate::widgets::fit`]'s ellipsis instead of wrapping (#349 hit
/// this once boulders gave foothill pathing a reason to differ from before).
const CURSOR_DETAIL_LINES: usize = 2;
/// A research bar is this many cells.
const BAR_CELLS: usize = 10;
/// The People section never shows more activity-count rows than this, however wide the panel or
/// however many distinct activities are in play (docs/design/side-panel.md "People: counts, not
/// names": "The groups pack into at most three rows").
const ACTIVITY_ROWS_MAX: usize = 3;

/// A heading (`crate::widgets::heading_row`) and the rows under it.
struct Section {
    title: String,
    right: String,
    rows: Vec<Row>,
    /// Shown even with no rows (People, Endeavor, Tool, Cursor): their heading is still useful
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
    /// The People section's activity-count rows (#310, docs/design/side-panel.md "People:
    /// counts, not names"), each paired with the settlers it and every row before it account
    /// for, so fitting can say how many a dropped tail folds into (`+N`). At most
    /// [`ACTIVITY_ROWS_MAX`] of them; never empty for a settlement with at least one unit (Idle
    /// is the fallback group).
    activity_rows: Vec<(Row, usize)>,
    /// The need-flags row (#310): `None` when nobody is flagged, which hides the row.
    need_flags_row: Option<Row>,
    /// The Season section (#299), under People; `None` with the chosen difficulty preset's
    /// seasons off, which hides the whole section.
    season: Option<SeasonView>,
    stores: Vec<Row>,
    /// Research's full, two-row shape (target and rate, then the bar).
    research: Vec<Row>,
    /// Research's one-row shape, used once `Knobs::research` drops to 1
    /// (docs/design/side-panel.md "Sections, top to bottom": `Masonry ██████░░░░ 68%`).
    research_short: Row,
    /// The Endeavor section's one row (#235, #242); empty with no endeavor in the data. The
    /// heading carries the `met/total` count (`SideView::endeavor`'s own `count`).
    endeavor: Vec<Row>,
    /// The Tool section's rows for the Build tool's chosen building (#111; its full picker moved
    /// to the Build modal, #285), or every usable ability for the Bend tool (#248,
    /// `bend_ability_rows`, the current one first so fitting can keep it alone); empty for every
    /// other tool, or with nothing placeable or castable.
    picker: Vec<Row>,
    /// The Tool section's last result, wrapped to at most [`NOTICE_LINES`].
    notice: Vec<Row>,
    alerts: Vec<Row>,
    cursor: Vec<Row>,
    cursor_pos: String,
    /// "Harvest · repeating" while the standing-order toggle is on (`UiState::repeating_label`,
    /// #277); `None` the rest of the time.
    repeating_label: Option<String>,
}

/// How much of each trimmable block to show.
#[derive(Clone, Copy)]
struct Knobs {
    /// How many of `Data::activity_rows` to show; the rest fold into `+N` (#310). Never 0 for a
    /// live settlement: fitting only takes rows past the first.
    units: usize,
    alerts: usize,
    picker: usize,
    stores: usize,
    /// 2 for Research's full shape, 1 for its short one (`Data::research_short`).
    research: usize,
    /// 2 rows (food, then "Winter here") or 1 (food alone); meaningless with `Data::season`
    /// `None`.
    season: usize,
    notice: usize,
    /// Whether the Endeavor section's one row shows; its heading stays either way
    /// (docs/design/side-panel.md "Sections, top to bottom": "When short, the heading alone
    /// shows").
    endeavor: bool,
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

/// The header (docs/design/side-panel.md "Sections, top to bottom"): one row, the run/pause mark,
/// word and speed on the left, the clock on the right. The seed is the panel's own bottom edge
/// now, and `Space + -` is the status line's and the key help's.
fn header(view: &SideView, marks: &Marks) -> Vec<Row> {
    let (icon, word, tone) = if view.paused {
        (marks.pause, "Paused", Tone::Paused)
    } else {
        (marks.run, "Running", Tone::Running)
    };
    vec![Row::pair(format!("{icon} {word} x{}", view.speed), tone, view.clock.clone(), Tone::Text)]
}

/// A channelling unit's status in place of the plain task text ("Channelling Hearthflame 1.2s"):
/// its ability and the time left, while it stands on the tile and bends (`World::bend_channels`,
/// #278). `None` while it walks there or does anything else, which keeps the plain task (the
/// selection bar's own use, `crate::bar`; the Nation screen's roster, `N`, #308, has its own).
pub fn channel_text(session: &Session, unit: &Unit) -> Option<String> {
    let defs = &session.defs;
    session.world.bend_channels(defs).into_iter().find(|c| c.unit == unit.id).map(|c| {
        format!("Channelling {} {:.1}s", defs.ability(c.ability).name, c.remaining_secs.max(0.0))
    })
}

/// The People section's activity-count rows (#310, docs/design/side-panel.md "People: counts,
/// not names"): `fn_ui::view::activity_counts` as "Label N" entries, packed into as few rows as
/// fit `width`, each paired with the settlers it and every row before it account for (so a
/// dropped tail's count is `total - last shown row's`).
fn activity_rows(session: &Session, width: usize) -> Vec<(Row, usize)> {
    let mut rows = Vec::new();
    let mut line = String::new();
    let mut line_total = 0usize;
    let mut cumulative = 0usize;
    for (activity, n) in activity_counts(session) {
        let entry = format!("{} {n}", activity.label());
        if !line.is_empty() && line.chars().count() + 2 + entry.chars().count() > width {
            cumulative += line_total;
            rows.push((Row::text(std::mem::take(&mut line), Tone::Text), cumulative));
            line_total = 0;
        }
        if !line.is_empty() {
            line.push_str("  ");
        }
        line.push_str(&entry);
        line_total += n;
    }
    if !line.is_empty() {
        cumulative += line_total;
        rows.push((Row::text(line, Tone::Text), cumulative));
    }
    rows
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

/// What the stockpiles hold, "Wood 12  Stone 4", packed into as few lines as fit `width`.
fn store_rows(session: &Session, width: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut line = String::new();
    for (name, count) in stores(session) {
        let entry = format!("{name} {count}");
        if !line.is_empty() && line.chars().count() + 2 + entry.chars().count() > width {
            rows.push(Row::text(std::mem::take(&mut line), Tone::Text));
        }
        if !line.is_empty() {
            line.push_str("  ");
        }
        line.push_str(&entry);
    }
    if !line.is_empty() {
        rows.push(Row::text(line, Tone::Text));
    }
    rows
}

/// The target and how far along it is, in two rows (docs/design/side-panel.md "Sections, top to
/// bottom"): the tech and the rate, then the bar, the percentage and the points. With no target,
/// the points banked.
fn research_rows(view: &SideView, marks: &Marks) -> Vec<Row> {
    let r = &view.research;
    if r.frac == 0.0 && r.label == "No target set" {
        return vec![
            Row::pair(r.label.clone(), Tone::Dim, r.rate.clone(), Tone::Dim),
            Row::text(r.pool.clone(), Tone::Dim),
        ];
    }
    let label =
        if r.more_queued > 0 { format!("{} +{}", r.label, r.more_queued) } else { r.label.clone() };
    let (full, empty) = meter(r.frac, BAR_CELLS, marks.full, marks.empty);
    vec![
        Row::pair(label, Tone::Strong, r.rate.clone(), Tone::Dim),
        Row::spans(vec![
            (full, Tone::Accent),
            (empty, Tone::Dim),
            (format!(" {:>3.0}% ", r.frac * 100.0), Tone::Text),
            (r.pool.clone(), Tone::Dim),
        ]),
    ]
}

/// Research's short, one-row shape (docs/design/side-panel.md "Sections, top to bottom"): the
/// target's name, its bar and its percent, all on one row ("Masonry ██████░░░░ 68%"). With no
/// target, the label and the points banked.
fn research_row_short(view: &SideView, marks: &Marks) -> Row {
    let r = &view.research;
    if r.frac == 0.0 && r.label == "No target set" {
        return Row::pair(r.label.clone(), Tone::Dim, r.pool.clone(), Tone::Dim);
    }
    let (full, empty) = meter(r.frac, BAR_CELLS, marks.full, marks.empty);
    Row::spans(vec![
        (format!("{} ", r.label), Tone::Strong),
        (full, Tone::Accent),
        (empty, Tone::Dim),
        (format!(" {:>3.0}%", r.frac * 100.0), Tone::Text),
    ])
}

/// The side panel's Endeavor section (#235, #242), under Research: the heading carries
/// `met/total` (`EndeavorStatus::count`); the one row under it names the first missing sign
/// (docs/design/side-panel.md "Sections, top to bottom": "only the first missing sign"), or, with
/// nothing missing, the line the bar used to sit above (signs met, or the endeavor's own stage).
/// Empty with no endeavor in the data, which hides the whole section.
fn endeavor_rows(view: &SideView) -> Vec<Row> {
    let Some(e) = &view.endeavor else { return Vec::new() };
    vec![match e.next.first() {
        Some(next) => Row::pair(format!("Next: {next}"), Tone::Warn, "[G]", Tone::Dim),
        None => Row::pair(e.line.clone(), Tone::Dim, "[G]", Tone::Dim),
    }]
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

/// The Tool section's rows while the Bend tool is active (#248, docs/design/endgame.md "The
/// transformation"): every ability someone in the settlement can cast, the one `B` would channel
/// next marked, with how many could cast it ("Raise Earth: 14 benders", `World::caster_count`),
/// the current ability first so fitting ("the Bend ability list down to the current ability",
/// docs/design/side-panel.md) can drop everyone else and still leave it on screen. Empty off the
/// Bend tool, which keeps the Build tool's single-row `picker_row` in its slot.
fn bend_ability_rows(session: &Session, marks: &Marks) -> Vec<Row> {
    let defs = &session.defs;
    let current = bend_ability(session);
    let mut abilities = usable_abilities(session);
    if let Some(at) = current.and_then(|c| abilities.iter().position(|&id| id == c)) {
        abilities.swap(0, at);
    }
    abilities
        .into_iter()
        .map(|id| {
            let def = defs.ability(id);
            let mark = if Some(id) == current { marks.pointer } else { " " };
            let casters = session.world.caster_count(id, defs);
            let word = if casters == 1 { "bender" } else { "benders" };
            let name_tone = if Some(id) == current { Tone::Title } else { Tone::Text };
            Row::pair(
                format!("{mark}{}", def.name),
                name_tone,
                format!("{casters} {word}"),
                Tone::Dim,
            )
        })
        .collect()
}

/// Whether the building the Build tool would place fits the cursor's tile, or the reason it
/// doesn't (`fn_ui::build::placement`). `None` off the Build tool, or with nothing placeable.
fn placement_row(session: &Session, pos: Pos, choice: Option<BuildingId>) -> Option<Row> {
    let placement = build::placement(session, pos, choice?);
    Some(match placement.refused {
        None => Row::text("Can build here", Tone::Ok),
        Some(why) => {
            let why = build::describe_error(session, CommandError::Build(why));
            Row::text(format!("Can't build here: {why}"), Tone::Warn)
        }
    })
}

/// The tile under the cursor: whether the Build tool's choice would fit there, then the
/// tooltip, then anything else lying on it. What `Enter` does next in a tool is the active tool
/// banner's own line now (#307, `crate::banner`), not repeated here.
///
/// A Detail line wraps (at most [`CURSOR_DETAIL_LINES`]) instead of fitting with an ellipsis:
/// the Cursor section is always last and "gets the spare rows" (docs/design/side-panel.md), so
/// unlike every other section's rows, it can afford to grow rather than lose a tail a player (or
/// an agent reading the panel, #320) might need.
fn cursor_rows(
    session: &Session,
    ui: &UiState,
    choice: Option<BuildingId>,
    width: usize,
    ellipsis: char,
) -> Vec<Row> {
    let pos = ui.cursor;
    let mut rows = Vec::new();
    if ui.tool == Tool::Build
        && let Some(row) = placement_row(session, pos, choice)
    {
        rows.push(row);
    }
    let mut lines = tooltip_lines(session, ui.tool, pos).unwrap_or_default();
    // The tooltip names the top thing on the tile; the section lists what else is there.
    if let Some((_, stack)) = session.world.ground_items().find(|&(q, _)| q == pos) {
        lines.push(Line::Detail(format!(
            "On the ground: {}",
            describe_stack(&session.defs, Some(stack))
        )));
    }
    if let Some(stack) = session.world.stored_at(pos) {
        lines.push(Line::Detail(format!("Stored: {}", describe_stack(&session.defs, Some(stack)))));
    }
    if session.world.stockpile_zones().any(|q| q == pos) {
        lines.push(Line::Detail("Stockpile".into()));
    }
    for line in lines {
        match line {
            Line::Title(s) => rows.push(Row::text(s, Tone::Strong)),
            Line::Detail(s) => rows.extend(
                wrap(&s, width, CURSOR_DETAIL_LINES, ellipsis)
                    .into_iter()
                    .map(|l| Row::text(l, Tone::Text)),
            ),
            Line::Hint(h) => {
                if let Some(s) = hint_text(h) {
                    rows.push(Row::text(s, Tone::Dim));
                }
            }
        }
    }
    // A building with anyone inside lists them last ("Mai, studying", #226, #229): `Enter`
    // cycles onto one the same way it cycles several units sharing a tile (`UiState::select_here`).
    for occupant in build::occupant_rows(session, pos) {
        rows.push(Row::text(occupant.text(), Tone::Text));
    }
    rows
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
    // tool uses the same slot for every usable ability instead (#248). The tool's own hint
    // (`UiState::tool_hint`) is the status line's and the active tool banner's now (#307), not a
    // row here (docs/design/side-panel.md "Sections, top to bottom").
    let picker: Vec<Row> = if ui.tool == Tool::Bend {
        bend_ability_rows(session, marks)
    } else {
        view.picker
            .iter()
            .find(|row| Some(row.id) == view.choice)
            .map(|row| vec![picker_row(row, view.choice, marks)])
            .unwrap_or_default()
    };
    let notice = wrap(&view.notice, width, NOTICE_LINES, ellipsis)
        .into_iter()
        .map(|l| Row::text(l, Tone::Text))
        .collect();
    let alerts = ui
        .alerts
        .toasts()
        .into_iter()
        .flat_map(|e| {
            let tone = crate::screens::kind_tone(e.kind);
            wrap(&e.text, width, ALERT_LINES, ellipsis).into_iter().map(move |l| Row::text(l, tone))
        })
        .collect();
    Data {
        activity_rows: activity_rows(session, width).into_iter().take(ACTIVITY_ROWS_MAX).collect(),
        need_flags_row: need_flags_row(session),
        season: season_view(session, winter_food_per_head),
        stores: store_rows(session, width),
        research: research_rows(&view, marks),
        research_short: research_row_short(&view, marks),
        endeavor: endeavor_rows(&view),
        picker,
        notice,
        alerts,
        cursor: cursor_rows(session, ui, view.choice, width, ellipsis),
        cursor_pos: format!("({}, {})", ui.cursor.x, ui.cursor.y),
        repeating_label: ui.repeating_label(),
        view,
    }
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
    let people = {
        // At least one row always shows (`Knobs::units` never reaches 0): fitting only takes
        // rows past the first (docs/design/side-panel.md "Fitting").
        let shown = k.units.min(d.activity_rows.len()).max(1);
        let mut rows: Vec<Row> =
            d.activity_rows.iter().take(shown).map(|(row, _)| row.clone()).collect();
        if shown < d.activity_rows.len() {
            let accounted = d.activity_rows[shown - 1].1;
            rows.push(Row::text(
                format!("+{}", d.view.people.saturating_sub(accounted)),
                Tone::Dim,
            ));
        }
        if let Some(flags) = &d.need_flags_row {
            rows.push(flags.clone());
        }
        // `[N]` sits at the right end of the last row, flags or not (docs/design/side-panel.md
        // "People: counts, not names").
        if let Some(last) = rows.last_mut() {
            last.right = Some(("[N]".to_owned(), Tone::Dim));
        }
        Section::new("PEOPLE", population_label(&d.view), rows).always()
    };
    // Under People, shortened to fit the heading's own width (docs/design/side-panel.md "How it
    // fits #299 and #306"); hidden with the chosen difficulty preset's seasons off
    // (`Data::season` `None`).
    let season = d.season.as_ref().map(|s| {
        let mut rows = vec![Row::pair(
            "Food in stores",
            Tone::Dim,
            s.food.clone(),
            if s.food_warn { Tone::Warn } else { Tone::Text },
        )];
        if k.season >= 2 {
            rows.push(Row::text(s.here.clone(), Tone::Dim));
        }
        Section::new(s.heading.clone(), s.left.clone(), rows)
    });
    let stores = Section::new("STORES", "", d.stores.iter().take(k.stores).cloned().collect());
    let research_rows =
        if k.research >= 2 { d.research.clone() } else { vec![d.research_short.clone()] };
    let research = Section::new("RESEARCH", "", research_rows);
    // The heading carries `met/total` either way; the one row under it (the first missing sign)
    // is the one thing fitting gives up first (docs/design/side-panel.md "Sections, top to
    // bottom": "When short, the heading alone shows").
    let endeavor = d.view.endeavor.as_ref().map(|e| {
        let rows: Vec<Row> = d.endeavor.iter().take(usize::from(k.endeavor)).cloned().collect();
        Section::new("ENDEAVOR", e.count.clone(), rows).always()
    });
    // The newest alerts are the last rows; a short panel keeps those.
    let skip = d.alerts.len().saturating_sub(k.alerts);
    let alerts = Section::new("ALERTS", "[L]", d.alerts.iter().skip(skip).cloned().collect());
    // Just above Cursor, in the tool's own tint (docs/design/side-panel.md "Headings"). Select
    // mode has no tool to talk about.
    let tool = (d.view.tool != Tool::Select).then(|| {
        let mut rows: Vec<Row> = d.picker.iter().take(k.picker).cloned().collect();
        rows.extend(d.notice.iter().take(k.notice).cloned());
        // "Harvest · repeating" while the standing-order toggle is on (#277), else just the
        // tool's name.
        let name = d.repeating_label.clone().unwrap_or_else(|| d.view.tool.name().to_owned());
        let tone = tool_tint(d)
            .map_or(Tone::Heading, |t| Tone::Custom(palette.color(palette.tint_rgb(t))));
        Section::new(name, format!("[{}]", d.view.tool.key()), rows).always().tint(tone)
    });
    let cursor = Section::new("CURSOR", d.cursor_pos.clone(), d.cursor.clone()).always();
    let all = [Some(people), season, Some(stores), Some(research), endeavor, Some(alerts), tool];
    all.into_iter().flatten().chain([cursor]).filter(Section::visible).collect()
}

/// The rows the header and `sections` take: one row per heading, no blank rows between them
/// (docs/design/side-panel.md "Decisions in brief": "the blank row between sections goes").
fn height(header_rows: usize, sections: &[Section]) -> usize {
    header_rows + sections.iter().map(|s| 1 + s.rows.len()).sum::<usize>()
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

    // Fit: give up the least useful rows first, in docs/design/side-panel.md's own order, until
    // the sections fit the panel.
    let head = header(&d.view, &marks);
    let notice_floor = d.notice.len().min(NOTICE_FLOOR);
    let picker_floor = usize::from(!d.picker.is_empty());
    // Fitting gives up alerts beyond the newest, but never the newest itself while at least one
    // is live (#415): the ALERTS heading on its own, with every row gone, would otherwise read as
    // "no alerts" instead of "a live one that got clipped".
    let alerts_floor = usize::from(!d.alerts.is_empty());
    let mut k = Knobs {
        units: d.activity_rows.len(),
        alerts: d.alerts.len().min(ALERTS_SHOWN),
        picker: d.picker.len(),
        stores: d.stores.len().min(STORES_SHOWN),
        research: 2,
        season: 2,
        notice: d.notice.len(),
        endeavor: true,
    };
    while height(head.len(), &sections(&d, k, palette)) > usize::from(inner.height) {
        if k.endeavor {
            k.endeavor = false;
        } else if k.alerts > alerts_floor {
            k.alerts -= 1;
        } else if k.research > 1 {
            k.research -= 1;
        } else if k.stores > 1 {
            k.stores -= 1;
        } else if k.units > 1 {
            k.units -= 1;
        } else if k.season > 1 {
            k.season -= 1;
        } else if k.notice > notice_floor {
            k.notice -= 1;
        } else if k.picker > picker_floor {
            k.picker -= 1;
        } else {
            break;
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

    /// The Cursor section's placement line (#275): `Can build here` where the chosen building
    /// fits, and `fn_ui::build::describe_error`'s reason where it doesn't; `None` with nothing
    /// chosen.
    #[test]
    fn the_placement_row_says_whether_the_chosen_building_fits() {
        use fn_sim::Nation;
        let session = Session::new(7, Nation::Earth, 48, 32);
        let bed = session.defs.building_id("bed").unwrap();
        assert_eq!(placement_row(&session, Pos::new(-1, -1), None), None);

        let free = session
            .world
            .map
            .iter()
            .find(|(p, t)| {
                t.terrain == fn_sim::Terrain::Land
                    && t.resource.is_none()
                    && !session.world.units.iter().any(|u| u.pos == *p)
            })
            .map(|(p, _)| p)
            .unwrap();
        let ok = placement_row(&session, free, Some(bed)).unwrap();
        assert_eq!(ok.plain(), "Can build here");

        let off_map = placement_row(&session, Pos::new(-1, -1), Some(bed)).unwrap();
        assert!(off_map.plain().starts_with("Can't build here: "), "{}", off_map.plain());
        assert!(off_map.plain().contains("off the map"), "{}", off_map.plain());
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

    /// The Cursor section lists a building's occupants last (#226, #229), "Mai, idle" until
    /// #227 gives them something to do; none off a building or on one with nobody inside.
    #[test]
    fn the_cursor_section_lists_a_building_s_occupants_last() {
        use fn_sim::Nation;
        use fn_sim::building::Building;
        use fn_sim::geom::Footprint;
        let mut s = Session::new(7, Nation::Fire, 64, 64);
        let hall = s.defs.building_id("study_hall").unwrap();
        let passable = s.defs.building(hall).passable;
        let at = Pos::new(30, 30);
        for dy in 0..3 {
            for dx in 0..3 {
                let t = s.world.map.get_mut(Pos::new(at.x + dx, at.y + dy)).unwrap();
                t.terrain = fn_sim::Terrain::Land;
                t.resource = None;
            }
        }
        let foot = Footprint::new(at, 2, 2);
        let ui = UiState::new(&s, (120, 40));
        assert!(cursor_rows(&s, &ui, None, 28, '…').iter().all(|r| !r.plain().contains(", idle")));

        for p in foot.tiles() {
            let part = [(p.x - at.x) as u8, (p.y - at.y) as u8];
            let b = Building { part, size: [2, 2], ..Building::new(hall, passable, false) };
            s.world.map.get_mut(p).unwrap().building = Some(b);
        }
        let door = foot.entrance();
        s.world.units[0].pos = door;
        let name = s.world.units[0].name.clone();
        let mut ui = UiState::new(&s, (120, 40));
        ui.cursor = at;
        assert!(
            cursor_rows(&s, &ui, None, 28, '…').iter().all(|r| !r.plain().contains(", idle")),
            "nobody inside yet"
        );

        assert!(s.world.enter_building(0, at, &s.defs));
        let rows = cursor_rows(&s, &ui, None, 28, '…');
        assert_eq!(rows.last().map(Row::plain), Some(format!("{name}, idle")));
    }
}

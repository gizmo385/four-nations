//! The Seal side panel (docs/design/side-panel-v2.md, #416, which revises
//! docs/design/side-panel.md): a framed column with the nation's name on the top edge, the seed
//! on the bottom edge, and three zones, each a divider heading joined to the frame
//! (`crate::widgets::heading_row`), with no blank rows between them.
//!
//! **Zone 1, the header and the Settlement glance (fixed; never trimmed but for its one optional
//! row):** the header (run/pause and speed left, the season with its time left right while
//! seasons are on, else the elapsed clock as before); one `SETTLEMENT` heading; People (the
//! total and Idle, #310 -- the activity breakdown moved to the Nation screen, `N`, #308); need
//! flags (hidden with nobody flagged); Food (the total, per head, and the winter outlook word);
//! Materials (non-food items, packed, `+N` fold, a second row with spare height); Research
//! (target, bar, percent, one row; "Research: none set" in Warn with no target); the Gate (the
//! signs met and the first missing one's short count, #235, #242; hidden with no endeavor in the
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
//! **Zone 3, Here** (named CURSOR before #418): Tool (just above Here: the active tool's name in
//! its own tint, the Build tool's chosen building or, for Bend, the current ability and its
//! bender count, one row either way, then the last rectangle's result; hidden off a tool) and
//! Here (what is under the keyboard cursor). A unit standing there shows its name, its task and,
//! on the Select tool, `Enter: select`; the own-nation prefix shows only once the settlement has
//! more than one nation (#248 "the transformation"), Carrying only with something to carry, and
//! finished bends don't show at all -- they're the selection bar's own line now (`crate::bar`,
//! `fn_ui::text::mastery_line`). While the selection bar is open for that same unit, Here shows the
//! tile instead (`fn_ui::inspect::tile_lines`), so nothing repeats. The Build tool's full picker
//! moved to its own modal (`U`, `crate::build`, #285); the panel names only the choice. The
//! nation's modifiers moved to the Nation screen's Traits (`N`, #308). What `Enter` does next in
//! a tool, and the tool's own hint, are the active tool banner's own lines (#307, `crate::banner`)
//! and the status line's, not the panel's.
//!
//! The panel is a fixed height, so a terminal too short for everything gives up rows in this
//! order: alerts beyond the newest, then the Tool section's result beyond two lines. The header,
//! the Settlement glance in full, every heading and three Here rows always stay.

use fn_sim::Pos;
use fn_sim::building::BuildingId;
use fn_sim::world::{CommandError, Unit};
use fn_ui::bend;
use fn_ui::build;
use fn_ui::endeavor::EndeavorStatus;
use fn_ui::inspect::{Hint, Line, tile_lines, tooltip_lines};
use fn_ui::text::describe_stack;
use fn_ui::view::{
    Activity, SeasonView, SideView, activity_counts, food_in_stores, materials, need_flags,
    season_view, side_view,
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
        Tool::Clear => "clear",
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
/// At most this many alert rows show at once, even with room for more (#417,
/// docs/design/side-panel-v2.md "Fitting": "Spare rows go, in order: Alerts up to 6..."). Alerts
/// are already given this much room before the fitting loop runs (`draw_panel`'s own `k.alerts`
/// starts at `d.alerts.len().min(ALERTS_SHOWN)`), so the Settlement glance's own growth (a
/// second Materials row) and Here only ever see what's left afterward.
const ALERTS_SHOWN: usize = 6;
/// At most this many Materials rows show at once, even with room for more (#416,
/// docs/design/side-panel-v2.md "Zone 1": "Spare height adds a second materials row").
const MATERIALS_ROWS_MAX: usize = 2;
/// At most this many lines a Here Detail wraps to (docs/design/side-panel.md: Cursor, now Here,
/// "gets the spare rows"), so a nation name plus a position ("Air Nomads - Foraging at (79, 75)")
/// never silently loses its tail to [`crate::widgets::fit`]'s ellipsis instead of wrapping (#349
/// hit this once boulders gave foothill pathing a reason to differ from before).
const HERE_DETAIL_LINES: usize = 2;
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
    /// The chosen difficulty preset's season data (#299); `None` with seasons off, which drops
    /// the header's season clock and the Food row's winter outlook word.
    season: Option<SeasonView>,
    /// The Settlement glance's Food row (#416): the total, per head, and (with seasons on) the
    /// winter outlook word.
    food: Row,
    /// The Settlement glance's Materials rows (#416): non-food items packed into as few rows as
    /// fit the panel's width, each paired with how many distinct kinds it and every row before
    /// it account for, so a dropped tail folds into `+N` kinds. At most [`MATERIALS_ROWS_MAX`]
    /// of them.
    materials: Vec<(Row, usize)>,
    /// The Settlement glance's Research row (#416): short shape (target, bar, percent), or
    /// "Research: none set" in Warn with no target.
    research: Row,
    /// The Settlement glance's Gate row (#416, #235, #242): the signs met and the first missing
    /// one's short count; `None` with no endeavor in the data.
    gate: Option<Row>,
    /// The Tool section's one row: the Build tool's chosen building (#111; its full picker moved
    /// to the Build modal, #285), or the Bend tool's current ability and bender count (#418,
    /// `bend_ability_rows`); empty for every other tool, or with nothing placeable or castable.
    picker: Vec<Row>,
    /// The Tool section's last result, wrapped to at most [`NOTICE_LINES`].
    notice: Vec<Row>,
    alerts: Vec<Row>,
    cursor: Vec<Row>,
    cursor_pos: String,
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
    picker: usize,
    notice: usize,
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

/// The header (docs/design/side-panel.md "Sections, top to bottom"): one row, the run/pause mark,
/// word and speed on the left, the clock on the right. The seed is the panel's own bottom edge
/// now, and `Space + -` is the status line's and the key help's.
fn header(view: &SideView, season: Option<&SeasonView>, marks: &Marks) -> Vec<Row> {
    let (icon, tone) =
        if view.paused { (marks.pause, Tone::Paused) } else { (marks.run, Tone::Running) };
    match season {
        // Seasons on (owner decision 4, #414): the clock gives way to the season and its time
        // left, which the header otherwise has no room for; the elapsed clock moves to the
        // Nation screen's header line (`nation::draw`, already there) and the Esc menu
        // (`crate::menu::draw`).
        Some(s) => {
            let left = if view.paused {
                format!("{icon} Paused")
            } else {
                format!("{icon} x{}", view.speed)
            };
            let right = format!("{} {}", s.heading.replace(" · ", " "), s.left);
            vec![Row::pair(left, tone, right, Tone::Text)]
        }
        None => {
            let word = if view.paused { "Paused" } else { "Running" };
            vec![Row::pair(
                format!("{icon} {word} x{}", view.speed),
                tone,
                view.clock.clone(),
                Tone::Text,
            )]
        }
    }
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

/// The Settlement glance's Food row (#416, docs/design/side-panel-v2.md "Zone 1"): the total in
/// stores, per head, and (with seasons on) the winter outlook word -- the climate band's winter
/// effects, Warn-toned exactly when `season.food_warn` is (autumn or winter, below
/// `winter_food_per_head`). With seasons off there is no outlook to show, same as today.
fn food_row(session: &Session, view: &SideView, season: Option<&SeasonView>, marks: &Marks) -> Row {
    let food = food_in_stores(session);
    let per_head = food / (view.people as u32).max(1);
    let base = format!("Food {food} {} {per_head}/head", marks.dot);
    match season {
        Some(s) if !s.outlook.is_empty() => {
            let tone = if s.food_warn { Tone::Warn } else { Tone::Text };
            Row::spans(vec![(format!("{base}  "), Tone::Text), (s.outlook.clone(), tone)])
        }
        _ => Row::text(base, Tone::Text),
    }
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
    let (full, empty) = meter(r.frac, BAR_CELLS, marks.full, marks.empty);
    Row::spans(vec![
        (format!("{} ", r.label), Tone::Strong),
        (full, Tone::Accent),
        (empty, Tone::Dim),
        (format!(" {:>3.0}%", r.frac * 100.0), Tone::Text),
    ])
}

/// The Settlement glance's Gate row (#416, #235, #242): the signs met and, while any is still
/// missing, the first one's short count (`SignDef::short`, "3 scrolls"); once nothing is
/// missing, the line the old Endeavor section's bar used to sit above (signs met, Ready, or the
/// endeavor's own stage). `None` with no endeavor in the data, which hides the row.
fn gate_row(e: &EndeavorStatus, marks: &Marks) -> Row {
    let dot = marks.dot;
    match e.next.first() {
        Some(next) => {
            Row::pair(format!("Gate {} {dot} {next}", e.count), Tone::Warn, "[G]", Tone::Dim)
        }
        None => {
            Row::pair(format!("Gate {} {dot} {}", e.count, e.line), Tone::Text, "[G]", Tone::Dim)
        }
    }
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

/// Whether the building the Build tool would place fits the cursor's tile, or the reason it
/// doesn't (`fn_ui::build::placement`), wrapped like the section's Detail lines so a long reason
/// ("would block the Academy's door") isn't cut short. Empty off the Build tool, or with nothing
/// placeable.
fn placement_rows(
    session: &Session,
    pos: Pos,
    choice: Option<BuildingId>,
    width: usize,
    ellipsis: char,
) -> Vec<Row> {
    let Some(choice) = choice else { return Vec::new() };
    match build::placement(session, pos, choice).refused {
        None => vec![Row::text("Can build here", Tone::Ok)],
        Some(why) => {
            let why = build::describe_error(session, CommandError::Build(why));
            wrap(&format!("Can't build here: {why}"), width, HERE_DETAIL_LINES, ellipsis)
                .into_iter()
                .map(|l| Row::text(l, Tone::Warn))
                .collect()
        }
    }
}

/// What is under the cursor: whether the Build tool's choice would fit there, then the tile's
/// own tooltip, then anything else lying on it. What `Enter` does next in a tool is the active
/// tool banner's own line now (#307, `crate::banner`), not repeated here.
///
/// The unit standing there, if any, is the tooltip's own job (`fn_ui::inspect::tooltip_lines`)
/// -- unless it's the very unit the selection bar already has open, in which case Here falls
/// back to the tile's own lines instead (`fn_ui::inspect::tile_lines`, #418): the bar already
/// names it, so Here naming it again would just repeat the name, the task and (while #226 keeps
/// an occupant off the map) nothing else.
///
/// A Detail line wraps (at most [`HERE_DETAIL_LINES`]) instead of fitting with an ellipsis:
/// the Here section is always last and "gets the spare rows" (docs/design/side-panel.md), so
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
    if ui.tool == Tool::Build {
        rows.extend(placement_rows(session, pos, choice, width, ellipsis));
    }
    let unit_here = session.world.units_at(pos).next();
    let selected_here = unit_here.is_some_and(|u| Some(u.id) == ui.selected);
    let mut lines = if selected_here {
        tile_lines(session, ui.tool, pos).unwrap_or_default()
    } else {
        tooltip_lines(session, ui.tool, pos).unwrap_or_default()
    };
    // The tooltip names the top thing on the tile; the section lists what else is there.
    if let Some((_, stack)) = session.world.ground_items().find(|&(q, _)| q == pos) {
        lines.push(Line::Detail(format!(
            "On the ground: {}",
            describe_stack(&session.defs, Some(stack))
        )));
    }
    // Stored and the zone itself merge into one line now (#418): "Stockpile: 9 Wood", or
    // "Stockpile: empty" with nothing stored there yet. A plain tile's own tooltip already
    // names this (`fn_ui::inspect::stockpile_line`), so this only still matters for a resource
    // tile a stockpile zone sits under (`PathRules::resources_block` lets some nations zone
    // over one), which returns before reaching that line.
    let named_already =
        lines.iter().any(|l| matches!(l, Line::Detail(d) if d.starts_with("Stockpile")));
    if !named_already {
        if let Some(stack) = session.world.stored_at(pos) {
            lines.push(Line::Detail(format!(
                "Stockpile: {}",
                describe_stack(&session.defs, Some(stack))
            )));
        } else if session.world.stockpile_zones().any(|q| q == pos) {
            lines.push(Line::Detail("Stockpile: empty".into()));
        }
    }
    for line in lines {
        match line {
            Line::Title(s) => rows.push(Row::text(s, Tone::Strong)),
            Line::Detail(s) => rows.extend(
                wrap(&s, width, HERE_DETAIL_LINES, ellipsis)
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
    // tool uses the same slot for its current ability's one row instead (#248, #418). The tool's
    // own hint (`UiState::tool_hint`) is the status line's and the active tool banner's now
    // (#307), not a row here (docs/design/side-panel.md "Sections, top to bottom").
    let picker: Vec<Row> = if ui.tool == Tool::Bend {
        bend_ability_rows(session, marks)
    } else {
        view.picker
            .iter()
            .find(|row| Some(row.id) == view.choice)
            .map(|row| picker_rows(row, view.choice, marks, width))
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
    let season = season_view(session, winter_food_per_head);
    let people = people_row(session, &view);
    let food = food_row(session, &view, season.as_ref(), marks);
    let research = research_row(&view, marks);
    let gate = view.endeavor.as_ref().map(|e| gate_row(e, marks));
    Data {
        people,
        need_flags_row: need_flags_row(session),
        season,
        food,
        materials: materials_rows(session, width),
        research,
        gate,
        picker,
        notice,
        alerts,
        cursor: cursor_rows(session, ui, view.choice, width, ellipsis),
        cursor_pos: format!("({}, {})", ui.cursor.x, ui.cursor.y),
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
    if let Some(gate) = &d.gate {
        rows.push(gate.clone());
    }
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
        let mut rows: Vec<Row> = d.picker.iter().take(k.picker).cloned().collect();
        rows.extend(d.notice.iter().take(k.notice).cloned());
        // "Harvest · repeating" or "Cancel · teardown" while the tool's own `O` toggle is on
        // (#277/#335), else just the tool's name.
        let name = d.repeating_label.clone().unwrap_or_else(|| d.view.tool.name().to_owned());
        let tone = tool_tint(d)
            .map_or(Tone::Heading, |t| Tone::Custom(palette.color(palette.tint_rgb(t))));
        Section::new(name, format!("[{}]", d.view.tool.key()), rows).always().tint(tone)
    });
    // CURSOR until #418: HERE, keeping the coordinates on the heading.
    let here = Section::new("HERE", d.cursor_pos.clone(), d.cursor.clone()).always();
    let all = [Some(settlement), Some(alerts), tool];
    all.into_iter().flatten().chain([here]).filter(Section::visible).collect()
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

    // Fit: the Settlement glance (#416) never gives up a row, so only Alerts and Here (the Tool
    // result, then the Bend ability list) shrink here; the glance's own growth -- a second
    // Materials row -- is a separate pass below, once this settles.
    let head = header(&d.view, d.season.as_ref(), &marks);
    let notice_floor = d.notice.len().min(NOTICE_FLOOR);
    let picker_floor = usize::from(!d.picker.is_empty());
    let materials_cap = d.materials.len().min(MATERIALS_ROWS_MAX);
    // Fitting gives up alerts beyond the newest, but never the newest itself while at least one
    // is live (#415): the ALERTS heading on its own, with every row gone, would otherwise read as
    // "no alerts" instead of "a live one that got clipped".
    let alerts_floor = usize::from(!d.alerts.is_empty());
    let mut k = Knobs {
        alerts: d.alerts.len().min(ALERTS_SHOWN),
        picker: d.picker.len(),
        notice: d.notice.len(),
        materials: materials_cap.min(1),
    };
    while height(head.len(), &sections(&d, k, palette)) > usize::from(inner.height) {
        if k.alerts > alerts_floor {
            k.alerts -= 1;
        } else if k.notice > notice_floor {
            k.notice -= 1;
        } else if k.picker > picker_floor {
            k.picker -= 1;
        } else {
            break;
        }
    }
    // Spare height adds the Materials section's second row before anything else in this zone
    // grows (#416): the only growth `settlement_rows` ever has, since Alerts' and Here's own
    // growth beyond today's caps are #417/#418.
    if k.materials < materials_cap {
        let mut grown = k;
        grown.materials += 1;
        if height(head.len(), &sections(&d, grown, palette)) <= usize::from(inner.height) {
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

    /// The Here section's placement line (#275): `Can build here` where the chosen building
    /// fits, and `fn_ui::build::describe_error`'s reason where it doesn't; `None` with nothing
    /// chosen.
    #[test]
    fn the_placement_row_says_whether_the_chosen_building_fits() {
        use fn_sim::Nation;
        let session = Session::new(7, Nation::Earth, 48, 32);
        let bed = session.defs.building_id("bed").unwrap();
        assert!(placement_rows(&session, Pos::new(-1, -1), None, 30, '…').is_empty());

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
        let ok = placement_rows(&session, free, Some(bed), 30, '…');
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].plain(), "Can build here");

        let off_map = placement_rows(&session, Pos::new(-1, -1), Some(bed), 60, '…');
        assert_eq!(off_map.len(), 1);
        assert_eq!(off_map[0].plain(), "Can't build here: off the map");

        // Too long for the panel, the reason wraps onto another row instead of losing its tail.
        let narrow = placement_rows(&session, Pos::new(-1, -1), Some(bed), 20, '…');
        let text: Vec<_> = narrow.iter().map(Row::plain).collect();
        assert_eq!(text.join(" "), "Can't build here: off the map", "{text:?}");
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

    /// The Here section lists a building's occupants last (#226, #229), "Mai, idle" until
    /// #227 gives them something to do; none off a building or on one with nobody inside.
    #[test]
    fn the_here_section_lists_a_building_s_occupants_last() {
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

    /// Once the selection bar is open for the unit under the cursor, Here shows the tile only,
    /// not the unit again (#418): the bar already names it, its task and `Bends N of M`, so
    /// repeating the name here is only noise. Moving the cursor off it, or deselecting, brings
    /// the unit's own lines straight back.
    #[test]
    fn the_here_section_shows_the_tile_not_the_unit_once_its_bar_is_open() {
        use fn_sim::Nation;
        let s = Session::new(7, Nation::Earth, 48, 32);
        let unit = s.world.units[0].clone();
        let mut ui = UiState::new(&s, (120, 40));
        ui.cursor = unit.pos;

        let unselected = cursor_rows(&s, &ui, None, 28, '…');
        assert_eq!(unselected.first().map(Row::plain), Some(unit.name.clone()), "{unselected:?}");

        ui.select(Some(unit.id));
        let selected = cursor_rows(&s, &ui, None, 28, '…');
        assert_ne!(
            selected.first().map(Row::plain),
            Some(unit.name.clone()),
            "the bar already names it: {selected:?}"
        );

        ui.select(None);
        let deselected = cursor_rows(&s, &ui, None, 28, '…');
        assert_eq!(deselected.first().map(Row::plain), Some(unit.name), "{deselected:?}");
    }

    /// A stockpile zone's own Here rows merge into one `Stockpile: ...` line (#418), not a
    /// `Stored:` line plus a bare `Stockpile` row -- and not two `Stockpile:` lines either, the
    /// tile's own tooltip (`fn_ui::inspect::stockpile_line`) already naming an empty zone the
    /// same way.
    #[test]
    fn the_stockpile_and_stored_lines_merge_into_one() {
        use fn_sim::Nation;
        use fn_sim::world::Command;
        let mut s = Session::new(7, Nation::Earth, 48, 32);
        let pos = s
            .world
            .map
            .iter()
            .find(|(p, t)| {
                t.terrain == fn_sim::Terrain::Land
                    && t.resource.is_none()
                    && !s.world.units.iter().any(|u| u.pos == *p)
            })
            .map(|(p, _)| p)
            .unwrap();
        s.apply(Command::DesignateStockpile { pos }).unwrap();
        let mut ui = UiState::new(&s, (120, 40));
        ui.cursor = pos;
        let rows = cursor_rows(&s, &ui, None, 28, '…');
        let stockpile_rows: Vec<_> =
            rows.iter().filter(|r| r.plain().starts_with("Stockpile")).collect();
        assert_eq!(
            stockpile_rows.iter().map(|r| r.plain()).collect::<Vec<_>>(),
            vec!["Stockpile: empty".to_owned()],
            "{rows:?}"
        );
    }
}

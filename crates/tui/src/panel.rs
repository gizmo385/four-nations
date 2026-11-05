//! The Seal side panel (docs/design/ui.md, docs/design/tui.md): a framed column with the nation's
//! name on the top edge, then sections in ui.md's order, each hidden while it has nothing to show:
//! the header (seed, clock, speed), People (the count, then a line per unit while they fit),
//! Stores, Research (target, bar, rate), Endeavor (the end game's progress, #235, #242; hidden
//! with no endeavor in the data), the nation's modifiers, Tool (the active designation tool, its
//! hint, the Build tool's chosen building and the last rectangle's result), Alerts and Cursor
//! (what is under the keyboard cursor). The Build tool's full picker moved to its own modal
//! (`U`, `crate::build`, #285); the panel names only the choice. What `Enter` does next in a
//! tool is the active tool banner's own line (#307, `crate::banner`), not the Cursor section's.
//!
//! The panel is a fixed height, so a terminal that is too short for every section gives up the
//! least useful lines first: the tool's hint and the Build tool's chosen-building line (the
//! status line names it too), the modifiers, the whole Endeavor section, the unit lines, the
//! older alerts and all but one line of stores. The header, Research and Cursor always stay.

use fn_sim::Pos;
use fn_sim::building::BuildingId;
use fn_sim::world::CommandError;
use fn_sim::world::Unit;
use fn_ui::build;
use fn_ui::inspect::{Hint, Line, tooltip_lines};
use fn_ui::text::{describe_stack, describe_task};
use fn_ui::tools::{bend_ability, usable_abilities};
use fn_ui::view::{SideView, side_view, stores};
use fn_ui::{Session, Tool};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::color::Palette;
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

/// The most lines the Tool section gives its hint (a bend ability's takes four) and the last
/// result.
const HINT_LINES: usize = 4;
const NOTICE_LINES: usize = 5;
/// The most lines one alert takes.
const ALERT_LINES: usize = 2;
/// A research bar is this many cells.
const BAR_CELLS: usize = 10;

/// What a task says in a unit's line: the phrase before the place and the progress.
pub fn short_task(task: &str) -> &str {
    let cut = [" at (", " to ("].iter().filter_map(|m| task.find(m)).min();
    cut.map_or(task, |i| &task[..i])
}

/// A titled block of rows.
struct Section {
    title: &'static str,
    right: String,
    rows: Vec<Row>,
    /// Shown even with no rows (People, Cursor).
    always: bool,
}

impl Section {
    fn new(title: &'static str, right: impl Into<String>, rows: Vec<Row>) -> Self {
        Self { title, right: right.into(), rows, always: false }
    }

    fn always(mut self) -> Self {
        self.always = true;
        self
    }

    fn visible(&self) -> bool {
        self.always || !self.rows.is_empty()
    }
}

/// Everything the sections show, computed once; the fitting loop picks how much of it to use.
struct Data {
    view: SideView,
    units: Vec<Row>,
    stores: Vec<Row>,
    research: Vec<Row>,
    /// The Endeavor section's rows (#235, #242); empty with no endeavor in the data.
    endeavor: Vec<Row>,
    modifiers: Vec<Row>,
    hint: Vec<Row>,
    /// The Tool section's one row for the Build tool's chosen building (#111; its full picker
    /// moved to the Build modal, #285), or every usable ability for the Bend tool (#248,
    /// `bend_ability_rows`); empty for every other tool, or with nothing placeable or castable.
    picker: Vec<Row>,
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
    units: usize,
    modifiers: usize,
    alerts: usize,
    hint: usize,
    picker: usize,
    stores: usize,
    /// Whether the Endeavor section shows at all: unlike Research and Cursor it gives way
    /// whole, before the unit lines, on a terminal too short for everything (#242).
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

fn header(view: &SideView, marks: &Marks) -> Vec<Row> {
    let (icon, word, tone) = if view.paused {
        (marks.pause, "Paused", Tone::Paused)
    } else {
        (marks.run, "Running", Tone::Running)
    };
    vec![
        Row::pair(format!("Seed {}", view.seed), Tone::Dim, view.clock.clone(), Tone::Text),
        Row::pair(format!("{icon} {word} x{}", view.speed), tone, "Space  + -", Tone::Dim),
    ]
}

/// A channelling unit's line in place of the plain task text ("Channelling Hearthflame 1.2s"):
/// its ability and the time left, while it stands on the tile and bends (`World::bend_channels`,
/// #278). `None` while it walks there or does anything else, which keeps [`describe_task`].
pub fn channel_text(session: &Session, unit: &Unit) -> Option<String> {
    let defs = &session.defs;
    session.world.bend_channels(defs).into_iter().find(|c| c.unit == unit.id).map(|c| {
        format!("Channelling {} {:.1}s", defs.ability(c.ability).name, c.remaining_secs.max(0.0))
    })
}

/// What a unit's line says it is doing: [`channel_text`] while it channels, else the plain task.
fn task_text(session: &Session, unit: &Unit) -> String {
    channel_text(session, unit)
        .unwrap_or_else(|| describe_task(&session.world, &session.defs, unit))
}

/// A line per unit: its name, then what it is doing. The selected unit is marked, so it can be
/// told without colour.
fn unit_rows(session: &Session, ui: &UiState, marks: &Marks) -> Vec<Row> {
    let units = &session.world.units;
    let name_width = units.iter().map(|u| u.name.chars().count()).max().unwrap_or(0);
    units
        .iter()
        .map(|u| {
            let selected = ui.selected == Some(u.id);
            let mark = if selected { if marks.h == '-' { ">" } else { "▸" } } else { " " };
            let task = task_text(session, u);
            Row::spans(vec![
                (
                    format!("{mark}{:<name_width$} ", u.name),
                    if selected { Tone::Title } else { Tone::Text },
                ),
                (short_task(&task).to_owned(), Tone::Dim),
            ])
        })
        .collect()
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

/// The target and how far along it is: the tech and the rate, then the bar, the percentage and
/// the points. With no target, the points banked.
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
    let (full, empty) = meter(r.frac, BAR_CELLS, marks);
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

/// The side panel's Endeavor section (#235, #242), under Research: `met/total`, a bar, the line
/// under it (what `G` is for, or that the endeavor is Ready), and up to three missing things.
/// Empty with no endeavor in the data, which hides the whole section.
fn endeavor_rows(view: &SideView, marks: &Marks) -> Vec<Row> {
    let Some(e) = &view.endeavor else { return Vec::new() };
    let (full, empty) = meter(e.frac, BAR_CELLS, marks);
    let mut rows = vec![
        Row::pair(e.name.clone(), Tone::Strong, e.count.clone(), Tone::Dim),
        Row::spans(vec![(full, Tone::Accent), (empty, Tone::Dim)]),
        Row::text(e.line.clone(), Tone::Dim),
    ];
    if !e.next.is_empty() {
        rows.push(Row::text(format!("Next: {}", e.next.join(", ")), Tone::Warn));
    }
    rows
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
/// next marked, with how many could cast it ("Raise Earth: 14 benders", `World::caster_count`).
/// Empty off the Bend tool, which keeps the Build tool's single-row `picker_row` in its slot.
fn bend_ability_rows(session: &Session, marks: &Marks) -> Vec<Row> {
    let defs = &session.defs;
    let current = bend_ability(session);
    usable_abilities(session)
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
fn cursor_rows(session: &Session, ui: &UiState, choice: Option<BuildingId>) -> Vec<Row> {
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
            Line::Detail(s) => rows.push(Row::text(s, Tone::Text)),
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

fn gather(session: &Session, ui: &UiState, marks: &Marks, width: usize) -> Data {
    let view = side_view(session, ui.tool, &ui.notice, ui.choice, &ui.alerts);
    let ellipsis = marks.ellipsis;
    let modifiers = view
        .modifiers
        .iter()
        .map(|(label, value)| Row::pair(label.clone(), Tone::Dim, value.clone(), Tone::Text))
        .collect();
    // `UiState::tool_hint`, not `view.tool_hint`: every other tool's is `fn_ui`'s own (the same
    // text either way), but the Build tool's names the TUI's own choice and `U` key (#285),
    // which the shared text can't (the choice belongs to the frontend, not the `Session`).
    let hint = wrap(&ui.tool_hint(session), width, HINT_LINES, ellipsis)
        .into_iter()
        .map(|l| Row::text(l, Tone::Dim))
        .collect();
    // Just the chosen building (#285 moved the rest of the picker to the Build modal); the Bend
    // tool uses the same slot for every usable ability instead (#248).
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
            let tone = if e.kind == fn_ui::alerts::Kind::Death { Tone::Bad } else { Tone::Text };
            wrap(&e.text, width, ALERT_LINES, ellipsis).into_iter().map(move |l| Row::text(l, tone))
        })
        .collect();
    Data {
        units: unit_rows(session, ui, marks),
        stores: store_rows(session, width),
        research: research_rows(&view, marks),
        endeavor: endeavor_rows(&view, marks),
        modifiers,
        hint,
        picker,
        notice,
        alerts,
        cursor: cursor_rows(session, ui, view.choice),
        cursor_pos: format!("({}, {})", ui.cursor.x, ui.cursor.y),
        repeating_label: ui.repeating_label(),
        view,
    }
}

/// The sections as the knobs allow, in order, with the hidden ones left out.
fn sections(d: &Data, k: Knobs) -> Vec<Section> {
    let people = {
        let n = d.units.len();
        let mut rows: Vec<Row> = d.units.iter().take(k.units).cloned().collect();
        if k.units < n && k.units > 0 {
            rows.push(Row::pair(format!("+{} more", n - k.units), Tone::Dim, "[Tab]", Tone::Dim));
        }
        Section::new("PEOPLE", population_label(&d.view), rows).always()
    };
    let stores = Section::new("STORES", "", d.stores.iter().take(k.stores).cloned().collect());
    let research = Section::new("RESEARCH", "", d.research.clone());
    // Under Research (docs/design/endgame.md, "Showing progress"); hides itself with no
    // endeavor in the data (`d.endeavor` is then empty) or when `k.endeavor` gives it up whole.
    let endeavor = k.endeavor.then(|| Section::new("ENDEAVOR", "", d.endeavor.clone()));
    let modifiers =
        Section::new("MODIFIERS", "", d.modifiers.iter().take(k.modifiers).cloned().collect());
    // Select mode has no tool to talk about.
    let tool = (d.view.tool != Tool::Select).then(|| {
        let mut rows: Vec<Row> = d.hint.iter().take(k.hint).cloned().collect();
        rows.extend(d.picker.iter().take(k.picker).cloned());
        rows.extend(d.notice.iter().cloned());
        // "Harvest · repeating" while the standing-order toggle is on (#277), else just the
        // tool's name.
        let name = d.repeating_label.clone().unwrap_or_else(|| d.view.tool.name().to_owned());
        Section::new("TOOL", format!("{name} [{}]", d.view.tool.key()), rows).always()
    });
    // The newest alerts are the last rows; a short panel keeps those.
    let skip = d.alerts.len().saturating_sub(k.alerts);
    let alerts = Section::new("ALERTS", "", d.alerts.iter().skip(skip).cloned().collect());
    let cursor = Section::new("CURSOR", d.cursor_pos.clone(), d.cursor.clone()).always();
    let all =
        [Some(people), Some(stores), Some(research), endeavor, Some(modifiers), tool, Some(alerts)];
    all.into_iter().flatten().chain([cursor]).filter(Section::visible).collect()
}

/// The rows the header and `sections` take, with a blank row before each section.
fn height(header_rows: usize, sections: &[Section]) -> usize {
    header_rows + sections.iter().map(|s| 2 + s.rows.len()).sum::<usize>()
}

/// Draws the panel into `area`: the frame, then the sections.
pub fn draw_panel(
    session: &Session,
    ui: &UiState,
    palette: &Palette,
    area: Rect,
    buf: &mut Buffer,
) {
    let marks = Marks::new(palette.ascii);
    let inner = Rect::new(area.left() + 2, area.top() + 1, area.width - 4, area.height - 2);
    let d = gather(session, ui, &marks, usize::from(inner.width));
    let title = format!(" {0} {1} {0} ", marks.seal, d.view.nation.to_uppercase());
    draw_frame(buf, area, palette, &title, TitleAt::Centre, "");

    // Fit: give up the least useful lines first until the sections fit the panel.
    let head = header(&d.view, &marks);
    let mut k = Knobs {
        units: d.units.len(),
        modifiers: d.modifiers.len(),
        alerts: d.alerts.len(),
        hint: d.hint.len(),
        picker: d.picker.len(),
        stores: d.stores.len(),
        endeavor: true,
    };
    while height(head.len(), &sections(&d, k)) > usize::from(inner.height) {
        if k.hint > 0 {
            k.hint -= 1;
        } else if k.picker > 0 {
            k.picker -= 1;
        } else if k.modifiers > 0 {
            k.modifiers -= 1;
        } else if k.endeavor {
            k.endeavor = false;
        } else if k.units > 0 {
            k.units -= 1;
        } else if k.alerts > 0 {
            k.alerts -= 1;
        } else if k.stores > 1 {
            k.stores -= 1;
        } else {
            break;
        }
    }

    let mut p = Painter::new(buf, inner, palette);
    for row in &head {
        p.row(row);
    }
    for section in sections(&d, k) {
        p.blank();
        let head = format!("{} {}", p.marks.bullet, section.title);
        let right = (!section.right.is_empty()).then(|| (section.right.clone(), Tone::Text));
        p.row(&Row { left: vec![(head, Tone::Title)], right });
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
    fn a_unit_line_keeps_the_phrase_and_drops_the_place() {
        assert_eq!(short_task("Chopping Oak Tree at (76, 64), 40% done"), "Chopping Oak Tree");
        assert_eq!(short_task("Hauling 5 Wood to (3, 4)"), "Hauling 5 Wood");
        assert_eq!(short_task("Sleeping"), "Sleeping");
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
        assert!(cursor_rows(&s, &ui, None).iter().all(|r| !r.plain().contains(", idle")));

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
            cursor_rows(&s, &ui, None).iter().all(|r| !r.plain().contains(", idle")),
            "nobody inside yet"
        );

        assert!(s.world.enter_building(0, at, &s.defs));
        let rows = cursor_rows(&s, &ui, None);
        assert_eq!(rows.last().map(Row::plain), Some(format!("{name}, idle")));
    }
}

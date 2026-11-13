//! The Seal side panel (docs/design/side-panel-v2.md, #416, which revises
//! docs/design/side-panel.md): a framed column with the nation's name on the top edge, the seed
//! on the bottom edge, and three zones, each a divider heading joined to the frame
//! (`crate::widgets::heading_row`), with no blank rows between them.
//!
//! **Zone 1, the header and the Settlement glance (fixed; never trimmed but for its one optional
//! row):** the header (two rows, #654: run/pause and speed, then the season, year and time
//! left, or the day with seasons off); one `SETTLEMENT` heading; People (the
//! total and Idle, #310 -- the activity breakdown moved to the Nation screen, `N`, #308); Beasts
//! (the count and how many are hungry, #699; hidden with no beast); need
//! flags (hidden with nobody flagged); Food (the total and each settler's share); the winter row;
//! Materials (non-food items, packed, `+N` fold, a second row with spare height); Research
//! (target, bar, percent, one row; "Research: none set" in Warn with no target); the Gate (the
//! signs met and what is still needed, #235, #242; hidden with no endeavor in the
//! data). The only thing this zone ever grows into spare height is the Materials section's
//! second row; nothing in it is ever given up first.
//!
//! **Zone 2, Alerts** (#656; always shows at least its heading and the first alert while any is
//! open, #415). The heading counts the open alerts. Each alert is wrapped in full, with a blank
//! row between them, and alerts of one cause are one entry (`fn_ui::alerts::Alerts::open`:
//! "Stockpiles are full. 442 stone, 17 fish and 15 fruit have nowhere to go."). Most urgent
//! first; they take at most half the rows below the glance, and the rest fold into "and 3 more, press L". The
//! news that is not an alert (a learned tech, a newcomer) shows after them while its toast
//! lasts, and folds first. A routine success (an ability cast) is log-only, never a toast
//! (#417); a toast's own text drops map coordinates, which stay in the log entry.
//!
//! **Zone 3, the Tool and the inspector** (`crate::inspector`, #653): Tool (the active tool's
//! name in its own tint; hidden off a tool), then the inspector, the last section, which takes
//! every row left. The Tool section (#657, `fn_ui::toolpanel`) is groups of rows: what the tool
//! does; the Build tool's chosen building or, for Bend, the current ability and its bender count;
//! with a corner set, the rectangle's size and what applying it would mark, and once applied,
//! what it marked ("Marked 7 trees.") until the cursor moves or a new corner is set; and the
//! tool's toggles ("Standing order: off"). With the tool list open (`/`, #664) the section is that
//! list alone, under a DESIGNATE heading, with a tool active or not. The inspector's heading names what it describes: the
//! selected settler (name and rank, with its task, carrying, meters, rank progress and tile in
//! the section, where the selection bar used to be), else whatever is under the keyboard cursor (a
//! settler, a building, "STOCKPILE", the terrain) with the cursor's position on the heading. The
//! Build tool's full picker moved to its own modal (`U`, `crate::build`, #285); the panel names
//! only the choice. The nation's modifiers moved to the Nation screen's Traits (`N`, #308). What
//! `Enter` does next in a tool is the active tool banner's own line (#307, `crate::banner`) and
//! the status line's keys, not the panel's.
//!
//! The panel is a fixed height, so a terminal too short for everything gives up rows in this
//! order: alerts beyond the first, then the Tool section's groups from the last (never what the
//! tool does, nor the building or ability), and the inspector is left one row while a tool is
//! active, since the Tool section has first call on rows then. The header, the Settlement glance
//! in full, every heading and the inspector's first rows (a settler's task and meters, three rows
//! of a tile) always stay with no tool; the inspector drops its own rows from the bottom
//! (`Inspector::fitted`).

use fn_ui::panel::{ChoiceTones, SidePanel, ToolLine, ToolSection, fold_alerts, fold_kinds};
use fn_ui::{Session, Tool};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::color::{Palette, Tint};
use crate::inspector::{Inspector, fit_blocks, inspect};
use crate::state::UiState;
use crate::widgets::{Marks, Painter, Row, TitleAt, Tone, draw_frame, wrap};
pub use fn_ui::view::channel_text;

pub use fn_ui::inspector::hint_text;

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
        Tool::Remove => "remove",
        Tool::Clear => "clear",
        Tool::Bend => "bend",
        Tool::Build => "build",
        Tool::Gate => "place",
        Tool::Select => "select",
    }
}

/// The most lines one paragraph of the Tool section wraps to (the description, a toggle): it
/// wraps rather than cuts, and the fitting drops whole groups of rows when the panel is short.
const TOOL_LINES: usize = 5;
/// At most this many Materials rows show at once, even with room for more (#416,
/// docs/design/side-panel-v2.md "Zone 1": "Spare height adds a second materials row").
const MATERIALS_ROWS_MAX: usize = 2;

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
    /// The words (`fn_ui::panel`): the header, the Settlement glance, the alerts and the Tool
    /// section, built with no width; the rest of this struct is those words laid out for it.
    model: SidePanel,
    /// The Settlement glance's Materials rows (#416): non-food items packed into as few rows as
    /// fit the panel's width, each paired with how many distinct kinds it and every row before
    /// it account for, so a dropped tail folds into `+N` kinds. At most [`MATERIALS_ROWS_MAX`]
    /// of them.
    materials: Vec<(Row, usize)>,
    /// The Tool section's rows (#657), in groups: what the tool does, the Build tool's chosen
    /// building or the Bend tool's current ability, then the rectangle's preview or what the
    /// last one marked, then the tool's toggles. Empty off a tool.
    tool: ToolRows,
    /// The open alerts (`fn_ui::alerts::Alerts::open`, #656), most urgent first, each wrapped in
    /// full.
    alerts: Vec<AlertItem>,
    /// The last section (#653): what the player is looking at, named in its own heading.
    inspector: Inspector,
}

/// One open alert in the Alerts section (#656): every row it wraps to, never cut.
struct AlertItem {
    rows: Vec<Row>,
}

/// How much of each trimmable block to show. The Settlement glance (#416) isn't knobbed: it
/// never gives up a row, and `materials` is the one thing it ever grows (`draw_panel`'s own
/// growth pass, after the shrink loop below settles).
#[derive(Clone, Copy)]
struct Knobs {
    /// How many of `Data::alerts` show, most urgent first; the rest fold into one last row.
    alerts: usize,
    /// How many rows of `Data::tool` to show ([`fit_blocks`]).
    tool: usize,
    /// How many of `Data::materials` to show, up to [`MATERIALS_ROWS_MAX`]; the rest fold into
    /// `+N` kinds on the last shown row.
    materials: usize,
}

/// The Settlement glance's Materials rows (#416): the model's items ("Wood 149", in the item
/// data's own order), packed into as few rows as fit `width`, each paired with how many distinct
/// kinds it and every row before it account for, so a dropped tail folds into `+N` kinds
/// (`settlement_rows`'s own job, since the fold has to land on whichever row fitting actually
/// keeps).
fn materials_rows(entries: &[String], width: usize) -> Vec<(Row, usize)> {
    let mut rows = Vec::new();
    let mut line = String::new();
    let mut kinds = 0usize;
    let mut cumulative = 0usize;
    for entry in entries {
        if !line.is_empty() && line.chars().count() + 2 + entry.chars().count() > width {
            cumulative += kinds;
            rows.push((Row::text(std::mem::take(&mut line), Tone::Text), cumulative));
            kinds = 0;
        }
        if !line.is_empty() {
            line.push_str("  ");
        }
        line.push_str(entry);
        kinds += 1;
    }
    if !line.is_empty() {
        cumulative += kinds;
        rows.push((Row::text(line, Tone::Text), cumulative));
    }
    rows
}

/// The Tool section's rows for the Build tool's current choice (`fn_ui::panel`'s `picker_row`):
/// a name and cost too wide to share `width` ("Mine Shaft" and "10 Stone, 6 Wood, 2 Tool, 2x2")
/// take a row each, the cost right-aligned under the name, rather than the cost crowding the name
/// out.
fn picker_rows(row: &Row, width: usize) -> Vec<Row> {
    let (Some((name, name_tone)), Some((right, right_tone))) = (row.left.first(), &row.right)
    else {
        return vec![row.clone()];
    };
    if name.chars().count() + 1 + right.chars().count() <= width {
        return vec![row.clone()];
    }
    vec![Row::text(name.clone(), *name_tone), Row::pair("", Tone::Dim, right.clone(), *right_tone)]
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
        ToolLine::Text(text, tone) => wrap(text, width, TOOL_LINES, marks.ellipsis)
            .into_iter()
            .map(|l| Row::text(l, *tone))
            .collect(),
        ToolLine::Setting { label, value, value_tone } => {
            let text = ToolLine::setting_text(label, value);
            if text.chars().count() > width {
                return wrap(&text, width, TOOL_LINES, marks.ellipsis)
                    .into_iter()
                    .map(|l| Row::text(l, Tone::Text))
                    .collect();
            }
            vec![Row::spans(vec![(format!("{label}: "), Tone::Text), (value.clone(), *value_tone)])]
        }
        ToolLine::Choice { selected, key, name, what, tones } => {
            vec![choice_row(*selected, *key, name, what, *tones, width, marks)]
        }
        ToolLine::Building(row) => picker_rows(row, width),
        ToolLine::Ability(row) => vec![row.clone()],
    }
}

/// How wide the tool list's name column is: "Stockpile".
const LIST_NAME: usize = 9;

/// One row of the tool list (`/`, #664): the pointer on the chosen row, the tool's letter, its
/// name in a column, and what it is for, cut to the room (a row never wraps, so the list keeps
/// its shape). A tool that can't be used yet is dim from end to end, its reason in place of
/// what it is for.
fn choice_row(
    selected: bool,
    key: char,
    name: &str,
    what: &str,
    tones: ChoiceTones,
    width: usize,
    marks: &Marks,
) -> Row {
    let mark = if selected { marks.pointer } else { " " };
    let head = format!("{mark} ");
    let room = width.saturating_sub(head.chars().count() + 2 + LIST_NAME + 2);
    let what = wrap(what, room.max(1), 1, marks.ellipsis).into_iter().next().unwrap_or_default();
    Row::spans(vec![
        (head, tones.head),
        (format!("{key} "), tones.key),
        (format!("{name:<LIST_NAME$}  "), tones.name),
        (what, tones.what),
    ])
}

/// The Tool section's rows for the active tool: the model's groups as rows `width` cells wide.
fn tool_rows(tool: Option<&ToolSection>, width: usize, marks: &Marks) -> ToolRows {
    let Some(tool) = tool else { return ToolRows { blocks: Vec::new(), keep: 0 } };
    let blocks = tool
        .blocks
        .iter()
        .map(|b| b.iter().flat_map(|l| tool_line_rows(l, width, marks)).collect())
        .collect();
    ToolRows { blocks, keep: tool.keep }
}

fn gather(
    session: &Session,
    ui: &UiState,
    marks: &Marks,
    width: usize,
    winter_food_per_head: u32,
) -> Data {
    let model = SidePanel::build(session, ui, marks, winter_food_per_head);
    let tool = tool_rows(model.tool.as_ref(), width, marks);
    // Every alert wraps across as many rows as it needs (#656); what doesn't fit folds away
    // whole, never cut.
    let alerts = model
        .alerts
        .entries
        .iter()
        .map(|a| {
            let rows = wrap(&a.text, width, usize::MAX, marks.ellipsis)
                .into_iter()
                .map(|l| Row::text(l, a.tone));
            AlertItem { rows: rows.collect() }
        })
        .collect();
    Data {
        materials: materials_rows(&model.settlement.materials, width),
        tool,
        alerts,
        inspector: inspect(session, ui, model.choice, width, marks),
        model,
    }
}

/// The Settlement glance's own rows (#416, docs/design/side-panel-v2.md "Zone 1"), in order:
/// People, Beasts (if any), need flags (if any), Food, the winter row (with seasons on),
/// Materials (`k.materials` of them, the rest folded into `+N` kinds on the last one shown),
/// Research, and the Gate (if the data has an endeavor). Never empty, so [`Section::visible`]
/// always keeps the SETTLEMENT heading on screen.
fn settlement_rows(d: &Data, k: Knobs) -> Vec<Row> {
    let s = &d.model.settlement;
    let mut rows = vec![s.people.clone()];
    rows.extend(s.beasts.clone());
    rows.extend(s.need_flags.clone());
    rows.push(s.food.clone());
    rows.extend(s.winter.clone());
    let shown = k.materials.min(d.materials.len());
    rows.extend(d.materials.iter().take(shown).map(|(row, _)| row.clone()));
    if shown < d.materials.len() {
        let accounted = if shown == 0 { 0 } else { d.materials[shown - 1].1 };
        let total = d.materials.last().map_or(0, |&(_, n)| n);
        let dropped = total.saturating_sub(accounted);
        match rows.last_mut() {
            Some(last) => last.left.push((fold_kinds(dropped), Tone::Dim)),
            None => rows.push(Row::text(format!("+{dropped}"), Tone::Dim)),
        }
    }
    rows.push(s.research.clone());
    rows.extend(s.research_bill.iter().cloned());
    rows.extend(s.gate.iter().cloned());
    rows
}

/// The active tool's tint ([`crate::banner::tint`]'s own logic, over the model rather than
/// `UiState`): `None` off a tool (Select).
pub fn tool_tint(t: &ToolSection) -> Option<Tint> {
    if t.tool == Tool::Harvest && t.repeating {
        Some(Tint::RepeatingHarvest)
    } else {
        Tint::for_tool(t.tool)
    }
}

/// The Alerts section's rows (#656): the first `shown` alerts, a blank row between them, and
/// when the rest don't fit, one last line saying how many are folded away. Most urgent first, so
/// what folds is the least urgent and, within that, the oldest.
fn alert_rows(alerts: &[AlertItem], shown: usize) -> Vec<Row> {
    let shown = shown.min(alerts.len());
    let mut rows: Vec<Row> = Vec::new();
    for (i, alert) in alerts.iter().take(shown).enumerate() {
        if i > 0 {
            rows.push(Row::text("", Tone::Text));
        }
        rows.extend(alert.rows.iter().cloned());
    }
    if shown < alerts.len() {
        rows.push(Row::text(fold_alerts(alerts.len() - shown), Tone::Dim));
    }
    rows
}

/// How many alerts to show in at most `budget` rows: as many as fit with the fold line, but
/// never fewer than the first (an alert is shown whole or folded, and the newest of the most
/// urgent is never folded while any are open).
fn alerts_within(alerts: &[AlertItem], budget: usize) -> usize {
    let first = usize::from(!alerts.is_empty());
    (first..=alerts.len()).rev().find(|&n| alert_rows(alerts, n).len() <= budget).unwrap_or(first)
}

/// The sections as the knobs allow, in order, with the hidden ones left out.
fn sections(d: &Data, k: Knobs, palette: &Palette) -> Vec<Section> {
    // One `SETTLEMENT` heading, `[N]` on the right, replaces the old PEOPLE, season, STORES,
    // RESEARCH and ENDEAVOR headings (#416): the zone never gives up a row, so it's always
    // visible and never knobbed down past `settlement_rows`'s own growth (the Materials section's
    // second row).
    let s = &d.model.settlement;
    let settlement = Section::new(&s.title, &s.right, settlement_rows(d, k)).always();
    let a = &d.model.alerts;
    let alerts = Section::new(&a.title, &a.right, alert_rows(&d.alerts, k.alerts));
    // Just above Here, in the tool's own tint (docs/design/side-panel.md "Headings"). Select
    // mode has no tool to talk about.
    let tool = d.model.tool.as_ref().map(|t| {
        let rows = fit_blocks(&d.tool.blocks, d.tool.keep, k.tool);
        // The list has its own heading, in the plain heading tone: it is about no one tool.
        if t.list {
            return Section::new(&t.title, &t.right, rows).always();
        }
        let tone = tool_tint(t).map_or(Tone::Heading, |tint| Tone::Custom(palette.tint_rgb(tint)));
        Section::new(&t.title, &t.right, rows).always().tint(tone)
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
    let title = format!(" {} ", d.model.title);
    let footer = d.model.footer.clone();
    draw_frame(buf, area, palette, &title, TitleAt::Centre, &footer);

    // Fit: the Settlement glance (#416) never gives up a row, so only Alerts and the Tool's
    // groups (from the last) shrink here, until the inspector keeps the rows it is never left
    // without; the glance's own growth -- a second Materials row -- is a separate pass below,
    // once this settles.
    let head = d.model.header.clone();
    let materials_cap = d.materials.len().min(MATERIALS_ROWS_MAX);
    // Fitting gives up alerts beyond the newest, but never the newest itself while at least one
    // is live (#415): the ALERTS heading on its own, with every row gone, would otherwise read as
    // "no alerts" instead of "a live one that got clipped".
    let alerts_floor = usize::from(!d.alerts.is_empty());
    let mut k = Knobs { alerts: 0, tool: d.tool.full(), materials: materials_cap.min(1) };
    // Alerts take at most half the rows below the glance (#656); the oldest, least urgent are
    // folded into a last line past that. The rest of the panel (the Tool section, the
    // inspector) shares what is left.
    let glance = head.len() + 1 + settlement_rows(&d, k).len();
    let below = usize::from(inner.height).saturating_sub(glance);
    k.alerts = alerts_within(&d.alerts, below / 2);
    let tool_active = d.model.tool.is_some();
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
    // grows (#416): the only growth `settlement_rows` ever has. Whatever it leaves is the
    // inspector's.
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
    use fn_ui::inspect::Hint;

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
        ui.set_tool(Tool::Remove);
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

    /// A name and cost too wide to share the panel take a row each, so the name isn't crowded
    /// out by the cost (the Mine Shaft's "10 Stone, 6 Wood, 2 Tool, 2x2"); one that fits stays
    /// one row.
    #[test]
    fn a_picker_row_too_wide_for_the_panel_puts_the_cost_on_its_own_row() {
        let marks = Marks::new(false);
        let cost = "5 Wood, 1x1";
        let row = Row::pair(format!("{}Bed", marks.pointer), Tone::Title, cost, Tone::Dim);
        let width = 1 + "Bed".len() + 1 + cost.len();
        assert_eq!(picker_rows(&row, width), vec![row.clone()]);
        let split = picker_rows(&row, width - 1);
        assert_eq!(split.len(), 2);
        assert_eq!(split[0].plain(), format!("{}Bed", marks.pointer));
        assert_eq!(split[0].left[0].1, Tone::Title);
        assert_eq!(split[1].right.as_ref().unwrap(), &(cost.to_owned(), Tone::Dim));
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

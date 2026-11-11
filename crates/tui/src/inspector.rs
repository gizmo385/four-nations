//! The side panel's last section, the inspector (docs/design/side-panel-v2.md "The inspector",
//! #653): what the player is looking at, in the panel, whatever it is. Its heading names the
//! thing -- a settler's name with its rank, a building, "STOCKPILE" or the terrain -- and it is
//! given every row the sections above it leave, so its text wraps and is never cut while a row
//! is free.
//!
//! * **A selected settler** (`Enter`, `Tab`) is its task, what it carries, the five meters as
//!   10-cell bars in the `ok`, `warn` and `bad` tokens with the word "low" after any that is not
//!   ok (so they read without colour), its progress to the next rank, and the tile it stands on.
//!   Chi is in the accent and never warns: it can't hurt the unit. These were the selection
//!   bar's rows (`crate::bar` until #653), which drew over the bottom of the map.
//! * **Anything else** is what is under the cursor: the tile's own lines
//!   (`fn_ui::inspect::tooltip_lines`), a settler standing there shown with `Enter` to select it.
//!   While a tool is active the cursor is what the tool acts on, so the inspector describes it
//!   (with the Build tool's "Can build here" first) even with a settler still selected.
//!
//! The cursor's position is said once: in the heading for a tile, on the "Standing on" line for
//! a settler. With too little room, rows go from the bottom: the tile line first, then the rank
//! progress, then the meters from the last up; blank rows between groups go before any text.

use fn_sim::Pos;
use fn_sim::building::BuildingId;
use fn_sim::world::{BuildError, CommandError, EndeavorError, Task, Unit};
use fn_ui::build;
use fn_ui::inspect::{Line, ground_phrase, tile_lines, tooltip_lines};
use fn_ui::text::describe_stack;
use fn_ui::view::{Status, UnitView, unit_view};
use fn_ui::{Session, Tool};

use crate::panel::hint_text;
use crate::state::UiState;
use crate::widgets::{Marks, Row, Tone, fit, meter, wrap};

/// A meter bar is this many cells.
const BAR_CELLS: usize = 10;
/// The label column of a meter ("Hunger").
const LABEL: usize = 6;
/// The most lines a paragraph wraps to. Generous: the inspector wraps rather than cuts, and the
/// fitting drops whole groups of rows when the panel is short.
const WRAP_LINES: usize = 8;
/// The most rows of a tile the sections above leave it before they give up their own.
const TILE_FLOOR: usize = 3;

/// One meter as a row: `Hunger ███████░░░  72%`, then ` low` in the warning's tone when it is
/// not ok. The cells are the game's one meter (`crate::widgets::meter`, #654): the whole bar
/// wears the warning while low. Its `!` flag column stays a space here, since the word says it.
pub fn meter_row(label: &str, frac: f32, tone: Tone, low: bool, marks: &Marks) -> Row {
    let [full, empty, _flag] = meter(frac, BAR_CELLS, tone, low, marks);
    let mut spans = vec![
        (format!("{label:<LABEL$} "), Tone::Dim),
        full,
        empty,
        (" ".to_owned(), Tone::Text),
        (format!("{:>4}", format!("{:.0}%", frac * 100.0)), Tone::Text),
    ];
    if low {
        spans.push((" low".to_owned(), tone));
    }
    Row::spans(spans)
}

/// The meters, top to bottom: health, hunger, thirst, energy, then chi.
pub fn meter_rows(view: &UnitView, marks: &Marks) -> Vec<Row> {
    let mut rows: Vec<Row> = view
        .needs
        .iter()
        .map(|n| meter_row(n.label, n.frac, Tone::of(n.status), n.status != Status::Ok, marks))
        .collect();
    rows.push(meter_row("Chi", view.chi_frac, Tone::Accent, false, marks));
    rows
}

/// What the inspector shows, before it knows how many rows it gets.
pub struct Inspector {
    /// What the heading names.
    pub title: String,
    /// The heading's right-hand value (the tile's position, a settler's rank); empty for none.
    pub right: String,
    /// Groups of rows, in order. A blank row separates neighbours when there is room.
    blocks: Vec<Vec<Row>>,
    /// The body rows the sections above leave it before they give up their own.
    floor: usize,
    /// How many of the first groups are never dropped whole: a short panel cuts rows from the
    /// last of them instead.
    keep: usize,
}

/// The inspector once it is fitted to its rows.
pub struct Fitted {
    pub title: String,
    pub right: String,
    pub rows: Vec<Row>,
}

impl Inspector {
    /// A heading, its right-hand value and the groups of rows under it, for a panel whose rows
    /// are `width` cells (its heading is four wider: the frame and its padding). A heading too
    /// narrow for its value gives it to the first row instead; one too narrow for its title
    /// gives that to the rows too, cut to fit the heading.
    fn new(
        title: String,
        right: String,
        mut blocks: Vec<Vec<Row>>,
        width: usize,
        (floor, keep): (usize, usize),
        marks: &Marks,
    ) -> Self {
        let (t, v) = (title.chars().count(), right.chars().count());
        let room = width + 4;
        let (title, right, moved) = if t + 3 <= room - 2 && (right.is_empty() || t + v + 9 <= room)
        {
            (title, right, Vec::new())
        } else if t + 3 <= room - 2 {
            (title, String::new(), vec![Row::text(right, Tone::Dim)])
        } else {
            let mut rows: Vec<Row> = wrap(&title, width, WRAP_LINES, marks.ellipsis)
                .into_iter()
                .map(|l| Row::text(l, Tone::Strong))
                .collect();
            if !right.is_empty() {
                rows.push(Row::text(right, Tone::Dim));
            }
            (fit(&title, room - 5, marks.ellipsis), String::new(), rows)
        };
        let (floor, keep) = (floor + moved.len(), keep + usize::from(!moved.is_empty()));
        if !moved.is_empty() {
            blocks.insert(0, moved);
        }
        Self { title, right, blocks, floor, keep }
    }

    /// How many body rows (not counting the heading) the panel's other sections leave it before
    /// they give up their own.
    pub fn floor(&self) -> usize {
        self.floor
    }

    /// All the body rows, with a blank between groups: what it shows with rows to spare.
    pub fn all_rows(&self) -> Vec<Row> {
        self.fitted(usize::MAX).rows
    }

    /// The heading and as many body rows as fit in `rows`. Spare rows become blanks between
    /// groups; a short panel loses the blanks first, then whole groups from the bottom, then (if
    /// even the first group is too long) rows from its bottom.
    pub fn fitted(&self, rows: usize) -> Fitted {
        let rows = fit_blocks(&self.blocks, self.keep, rows);
        Fitted { title: self.title.clone(), right: self.right.clone(), rows }
    }

    /// How many body rows it keeps while a tool is active: the Tool section has first call on
    /// the panel's rows then (#657), so the inspector is left one, which for the Build tool is
    /// the start of its "Can build here" line.
    pub fn floor_under_tool(&self) -> usize {
        self.floor.min(1)
    }
}

/// At most `rows` rows of `blocks`, groups of rows with a blank row between neighbours when
/// there is room: spare rows become blanks, a short panel loses the blanks first, then whole
/// groups from the bottom (never one of the first `keep`), then (if even those are too long)
/// rows from the bottom of what is left. Shared by the inspector and the Tool section (#657).
pub fn fit_blocks(blocks: &[Vec<Row>], keep: usize, rows: usize) -> Vec<Row> {
    let mut blocks: Vec<&Vec<Row>> = blocks.iter().filter(|b| !b.is_empty()).collect();
    let len = |bs: &[&Vec<Row>]| bs.iter().map(|b| b.len()).sum::<usize>();
    while blocks.len() > keep.max(1) && len(&blocks) > rows {
        blocks.pop();
    }
    let spare = rows.saturating_sub(len(&blocks));
    let gaps = blocks.len().saturating_sub(1).min(spare);
    let mut out: Vec<Row> = Vec::new();
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 && i <= gaps {
            out.push(Row::text("", Tone::Text));
        }
        out.extend(b.iter().cloned());
    }
    out.truncate(rows);
    out
}

/// The inspector for the current selection, cursor and tool, with rows `width` cells wide.
pub fn inspect(
    session: &Session,
    ui: &UiState,
    choice: Option<BuildingId>,
    width: usize,
    marks: &Marks,
) -> Inspector {
    let settler = if ui.tool == Tool::Select { ui.selected_unit(session) } else { None };
    match settler {
        Some(unit) if unit.is_beast() => beast_inspector(session, unit, width, marks),
        Some(unit) => settler_inspector(session, unit, width, marks),
        None => tile_inspector(session, ui, choice, width, marks),
    }
}

/// Wraps `text` and puts `tail` on its last line if it fits there, else on a line of its own,
/// so a position like `(52, 56)` is never split between lines.
fn wrap_with_tail(text: &str, tail: &str, width: usize, ellipsis: char) -> Vec<String> {
    let mut lines = wrap(text, width, WRAP_LINES, ellipsis);
    match lines.last_mut() {
        Some(last) if last.chars().count() + 1 + tail.chars().count() <= width => {
            last.push(' ');
            last.push_str(tail);
        }
        _ => lines.push(tail.to_owned()),
    }
    lines
}

/// The look of a task line: the Avatar holding the Harmony Rite (#238, #242) is bold on the
/// nation's accent, so the Avatar State reads apart from anything else he could be doing.
fn task_tone(is_avatar: bool, task: Option<Task>) -> Tone {
    if is_avatar && task == Some(Task::Rite) { Tone::Avatar } else { Tone::Text }
}

/// What the inspector says about `unit`, and the Nation screen's detail strip too (#660): the
/// plain `unit_view`, except that while the unit channels a bend its task line names the ability
/// and the time left (#278).
pub fn settler_view(session: &Session, unit: &Unit) -> UnitView {
    let mut view = unit_view(session, unit);
    if let Some(text) = crate::panel::channel_text(session, unit) {
        view.task = text;
    }
    view
}

/// A selected settler: the old selection bar's rows (task, carrying, meters, rank progress and
/// the tile) as the inspector's groups.
fn settler_inspector(session: &Session, unit: &Unit, width: usize, marks: &Marks) -> Inspector {
    let view = settler_view(session, unit);
    let paragraph = |text: &str, tone: Tone| -> Vec<Row> {
        wrap(text, width, WRAP_LINES, marks.ellipsis)
            .into_iter()
            .map(|l| Row::text(l, tone))
            .collect()
    };
    let mut doing = Vec::new();
    // The Avatar belongs to no nation of his own, so the panel's nation is not his: say whose
    // guest he is.
    if unit.is_avatar() {
        let mut guest = view.nation.clone();
        if let Some(first) = guest.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        doing.extend(paragraph(&guest, Tone::Dim));
    }
    doing.extend(paragraph(&view.task, task_tone(unit.is_avatar(), unit.task())));
    doing.push(Row::text(format!("Carrying {}", view.carrying), Tone::Text));
    let meters = meter_rows(&view, marks);
    let floor = doing.len() + meters.len();
    let rank = paragraph(&view.mastery_line, Tone::Text);
    let tail = format!("({}, {})", view.tile.x, view.tile.y);
    let standing = match &view.inside {
        Some(building) => format!("In the {building}"),
        None => {
            let ground = ground_phrase(session, view.tile).unwrap_or_default().to_lowercase();
            format!("Standing on {ground}")
        }
    };
    let standing: Vec<Row> = wrap_with_tail(&standing, &tail, width, marks.ellipsis)
        .into_iter()
        .map(|l| Row::text(l, Tone::Dim))
        .collect();
    Inspector::new(
        view.name.clone(),
        view.rank_title.clone(),
        vec![doing, meters, rank, standing],
        width,
        (floor, 2),
        marks,
    )
}

/// A selected beast (#699): its row's name, what it is doing and carrying, its home and the
/// home's food, and the tile it stands on. It has no meters, rank or chi to show: a beast has no
/// needs, and never bends or learns.
fn beast_inspector(session: &Session, unit: &Unit, width: usize, marks: &Marks) -> Inspector {
    let paragraph = |text: &str, tone: Tone| -> Vec<Row> {
        wrap(text, width, WRAP_LINES, marks.ellipsis)
            .into_iter()
            .map(|l| Row::text(l, tone))
            .collect()
    };
    let view = unit_view(session, unit);
    let mut doing = paragraph(&view.task, Tone::Text);
    doing.push(Row::text(format!("Carrying {}", view.carrying), Tone::Text));
    let home: Vec<Row> = fn_ui::inspect::beast_home_lines(session, unit)
        .iter()
        .flat_map(|line| paragraph(line, Tone::Text))
        .collect();
    let tail = format!("({}, {})", view.tile.x, view.tile.y);
    let ground = ground_phrase(session, view.tile).unwrap_or_default().to_lowercase();
    let standing: Vec<Row> =
        wrap_with_tail(&format!("Standing on {ground}"), &tail, width, marks.ellipsis)
            .into_iter()
            .map(|l| Row::text(l, Tone::Dim))
            .collect();
    let floor = doing.len() + home.len();
    Inspector::new(
        view.name,
        "Beast".to_owned(),
        vec![doing, home, standing],
        width,
        (floor, 2),
        marks,
    )
}

/// What the Build tool's choice would do on the cursor's tile: whether it fits, or the reason it
/// doesn't (`fn_ui::build::placement`), wrapped so a long reason ("would block the Academy's
/// door") isn't cut short. Empty off the Build tool, or with nothing placeable.
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
            wrap(&format!("Can't build here: {why}"), width, WRAP_LINES, ellipsis)
                .into_iter()
                .map(|l| Row::text(l, Tone::Warn))
                .collect()
        }
    }
}

/// Whether the gate fits with the cursor on `pos` (#643, `fn_ui::gate::placement`), or why not,
/// as [`placement_rows`] says it for the Build tool's choice.
fn gate_placement_rows(session: &Session, pos: Pos, width: usize, ellipsis: char) -> Vec<Row> {
    let Some(placement) = fn_ui::gate::placement(session, pos) else { return Vec::new() };
    let why = match placement.refused {
        None => return vec![Row::text("Can place the gate here", Tone::Ok)],
        Some(BuildError::CannotBuild { at, why }) => {
            fn_ui::gate::describe_error(session, EndeavorError::CannotBuild { at, why })
        }
        Some(_) => fn_ui::gate::describe_error(session, EndeavorError::NotReady),
    };
    wrap(&format!("Can't place the gate here: {why}"), width, WRAP_LINES, ellipsis)
        .into_iter()
        .map(|l| Row::text(l, Tone::Warn))
        .collect()
}

/// What a stockpile zone on `pos` holds, in words: "Wood 20 of 20, full", "Wood 12 of 20" or
/// "Empty". `None` where there is no zone and nothing stored.
fn stockpile_text(session: &Session, pos: Pos) -> Option<String> {
    let (world, defs) = (&session.world, &session.defs);
    let cap = defs.jobs.stockpile_stack;
    match world.stored_at(pos) {
        Some(s) => {
            let full = if s.count >= cap { ", full" } else { "" };
            Some(format!("{} {} of {cap}{full}", defs.item(s.item).name, s.count))
        }
        None if world.stockpile_zones().any(|q| q == pos) => Some("Empty".to_owned()),
        None => None,
    }
}

/// What is under the cursor: whether the Build tool's choice would fit there, then the tile's
/// own tooltip, then anything else lying on it. What `Enter` does next in a tool is the active
/// tool banner's own line (#307, `crate::banner`), not repeated here.
///
/// The heading is the first thing the tooltip names -- a settler standing there, a building, a
/// resource or the terrain -- and the cursor's position; a stockpile zone takes the heading
/// itself ("STOCKPILE") with what it holds first and the terrain under it after. The unit
/// standing there, if any, is the tooltip's own job (`tooltip_lines`) -- unless it is the unit
/// already selected, which falls back to the tile's own lines (`tile_lines`, #418).
fn tile_inspector(
    session: &Session,
    ui: &UiState,
    choice: Option<BuildingId>,
    width: usize,
    marks: &Marks,
) -> Inspector {
    let pos = ui.cursor;
    let ellipsis = marks.ellipsis;
    let paragraph = |text: &str, tone: Tone| -> Vec<Row> {
        wrap(text, width, WRAP_LINES, ellipsis).into_iter().map(|l| Row::text(l, tone)).collect()
    };
    let mut lead = Vec::new();
    if ui.tool == Tool::Build {
        lead.extend(placement_rows(session, pos, choice, width, ellipsis));
    }
    if ui.tool == Tool::Gate {
        lead.extend(gate_placement_rows(session, pos, width, ellipsis));
    }
    let unit_here = session.world.units_at(pos).next();
    let selected_here = unit_here.is_some_and(|u| Some(u.id) == ui.selected);
    let mut lines = if selected_here {
        tile_lines(session, ui.tool, pos).unwrap_or_default()
    } else {
        tooltip_lines(session, ui.tool, pos).unwrap_or_default()
    };
    let shows_unit = unit_here.is_some() && !selected_here;
    // The tile's own stockpile line is said in words below, and a building's "Built (x, y)" (its
    // anchor, for the agent's tooltip) is a position again: the heading has the cursor's.
    lines.retain(
        |l| !matches!(l, Line::Detail(d) if d.starts_with("Stockpile") || d.starts_with("Built (")),
    );
    // The tooltip names the top thing on the tile; the section lists what else is there.
    if let Some((_, stack)) = session.world.ground_items().find(|&(q, _)| q == pos) {
        lines.push(Line::Detail(format!(
            "On the ground: {}",
            describe_stack(&session.defs, Some(stack))
        )));
    }
    let mut title = None;
    let mut stock_rows = Vec::new();
    if let Some(text) = stockpile_text(session, pos) {
        if shows_unit {
            // A settler's tooltip owns the heading; the zone is one more line.
            lines.push(Line::Detail(format!("Stockpile: {text}")));
        } else {
            title = Some("Stockpile".to_owned());
            stock_rows = paragraph(&text, Tone::Text);
        }
    }
    // The first title is the heading, unless the stockpile has taken it: then it is the terrain
    // line under what the zone holds.
    let mut rows = Vec::new();
    for line in lines {
        match line {
            Line::Title(s) if title.is_none() => title = Some(s),
            Line::Title(s) => rows.extend(paragraph(&s, Tone::Strong)),
            Line::Detail(s) => rows.extend(paragraph(&s, Tone::Text)),
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
    let head: Vec<Row> = lead.into_iter().chain(stock_rows).collect();
    let blocks = if head.is_empty() { vec![rows] } else { vec![head, rows] };
    let right = format!("({}, {})", pos.x, pos.y);
    let floor = blocks.iter().map(Vec::len).sum::<usize>().min(TILE_FLOOR);
    let title = title.unwrap_or_else(|| "Nothing".to_owned());
    Inspector::new(title, right, blocks, width, (floor, 1), marks)
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;

    #[test]
    fn a_meter_row_is_label_bar_percent_and_the_word_low_only_when_not_ok() {
        let m = Marks::new(true);
        let ok = meter_row("Hunger", 0.72, Tone::Ok, false, &m);
        assert_eq!(ok.plain(), "Hunger #######...  72%");
        let low = meter_row("Energy", 0.23, Tone::Bad, true, &m);
        assert_eq!(low.plain(), "Energy ##........  23% low");
        let full = meter_row("Health", 1.0, Tone::Ok, false, &m);
        assert_eq!(full.plain(), "Health ########## 100%");
        // An ok bar wears the status and its empty cells are dim; a low one wears the warning
        // throughout, the word included.
        assert_eq!((ok.left[1].1, ok.left[2].1), (Tone::Ok, Tone::Dim));
        assert_eq!((low.left[1].1, low.left[2].1), (Tone::Bad, Tone::Bad));
        assert_eq!(low.left[5], (" low".to_owned(), Tone::Bad));
    }

    #[test]
    fn a_unit_s_meters_flag_exactly_the_ones_that_are_not_ok() {
        let mut s = Session::new(7, Nation::Earth, 48, 32);
        let id = s.world.units[0].id;
        s.world.units[0].needs.energy = 0.0;
        let unit = s.world.units.iter().find(|u| u.id == id).unwrap();
        let view = unit_view(&s, unit);
        let rows = meter_rows(&view, &Marks::new(false));
        assert_eq!(rows.len(), 5, "health, hunger, thirst, energy and chi");
        let flagged: Vec<&str> = view
            .needs
            .iter()
            .zip(&rows)
            .filter(|(_, r)| r.plain().ends_with(" low"))
            .map(|(n, _)| n.label)
            .collect();
        assert_eq!(flagged, ["Energy"]);
        assert!(!rows[4].plain().ends_with(" low"), "chi never warns");
        assert!(rows[4].plain().starts_with("Chi"));
    }

    /// The heading names the settler and its rank's own title (`docs/design/bending.md` "Seeing
    /// it", #361); the rows are task, carrying, meters, rank progress and where it stands.
    #[test]
    fn a_selected_settler_is_task_carrying_meters_rank_and_tile() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let mut ui = UiState::new(&s, (120, 40));
        ui.select(Some(s.world.units[0].id));
        let view = unit_view(&s, &s.world.units[0]);
        let i = inspect(&s, &ui, None, 28, &Marks::new(true));
        assert_eq!((i.title.as_str(), i.right.as_str()), (view.name.as_str(), "Student"));
        let text: Vec<String> = i.all_rows().iter().map(Row::plain).collect();
        assert_eq!(text[0], view.task);
        assert_eq!(text[1], "Carrying nothing");
        assert_eq!(text[2], "");
        assert!(text[3].starts_with("Health "), "{text:?}");
        assert!(text[7].starts_with("Chi "), "{text:?}");
        let tail = format!("({}, {})", view.tile.x, view.tile.y);
        assert!(text.last().unwrap().ends_with(&tail), "{text:?}");
        assert!(text.iter().any(|l| l.starts_with("Standing on ")), "{text:?}");
        assert!(text.iter().any(|l| l.starts_with("To adept: ")), "{text:?}");
        // The position is said once.
        assert_eq!(text.join("\n").matches(&tail).count(), 1, "{text:?}");
    }

    /// A short panel drops rows from the bottom: the tile line, then the rank progress, and only
    /// then a meter.
    #[test]
    fn a_short_inspector_drops_the_tile_then_the_rank_then_the_meters() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let mut ui = UiState::new(&s, (120, 40));
        ui.select(Some(s.world.units[0].id));
        let i = inspect(&s, &ui, None, 28, &Marks::new(true));
        let text = |rows| i.fitted(rows).rows.iter().map(Row::plain).collect::<Vec<_>>().join("\n");
        let all = text(usize::MAX);
        assert!(all.contains("Standing on") && all.contains("To adept") && all.contains("Chi"));
        // Seven rows: the task, carrying and the five meters, nothing else.
        let tight = text(7);
        assert!(tight.contains("Chi") && tight.contains("Health"), "{tight}");
        assert!(!tight.contains("To adept") && !tight.contains("Standing on"), "{tight}");
        assert!(!tight.contains("\n\n"), "no blank rows while rows are short: {tight}");
        // Between: the rank progress goes after the tile line has.
        let rank_rows = text(usize::MAX).lines().filter(|l| !l.is_empty()).count() - 7 - 2;
        let mid = text(7 + rank_rows);
        assert!(mid.contains("To adept") && !mid.contains("Standing on"), "{mid}");
        // Fewer than the meters take: the last meters go first.
        let cut = text(5);
        assert!(cut.contains("Hunger") && !cut.contains("Chi"), "{cut}");
    }

    /// The Harmony Rite's Avatar State (#238, #242) marks the task line apart: bold, on the
    /// nation's accent; anything else keeps the plain look.
    #[test]
    fn the_avatar_state_marks_the_task_line_apart() {
        assert_eq!(task_tone(true, Some(Task::Rite)), Tone::Avatar);
        assert_eq!(task_tone(true, None), Tone::Text);
        assert_eq!(task_tone(false, Some(Task::Rite)), Tone::Text);
    }

    /// A unit inside a building (#226, #229) reads "In the Academy" in place of its ground.
    #[test]
    fn a_settler_inside_a_building_names_it_instead_of_its_ground() {
        use fn_sim::geom::Footprint;
        let mut s = Session::new(7, Nation::Fire, 64, 64);
        let hall = s.defs.building_id("study_hall").unwrap();
        let at = Pos::new(30, 30);
        for dy in 0..3 {
            for dx in 0..3 {
                let t = s.world.map.get_mut(Pos::new(at.x + dx, at.y + dy)).unwrap();
                t.terrain = fn_sim::Terrain::Land;
                t.resource = None;
            }
        }
        let foot = Footprint::new(at, 2, 2);
        s.world.put_building(foot, hall, &s.defs);
        s.world.units[0].pos = foot.entrance();
        assert!(s.world.enter_building(0, at, &s.defs));
        let mut ui = UiState::new(&s, (120, 40));
        ui.select(Some(s.world.units[0].id));
        let rows = inspect(&s, &ui, None, 40, &Marks::new(false)).all_rows();
        assert!(rows.last().unwrap().plain().starts_with("In the Academy ("), "{rows:?}");
    }

    /// A tile's heading names it and its position, and the rows do not say the position again.
    #[test]
    fn a_tile_is_named_in_the_heading_with_its_position_once() {
        let s = Session::new(7, Nation::Earth, 48, 32);
        let mut ui = UiState::new(&s, (120, 40));
        let pos = s
            .world
            .map
            .iter()
            .find(|(p, t)| {
                t.resource.is_none()
                    && t.building.is_none()
                    && t.terrain.is_walkable()
                    && !s.world.units.iter().any(|u| u.pos == *p)
            })
            .map(|(p, _)| p)
            .unwrap();
        ui.cursor = pos;
        let i = inspect(&s, &ui, None, 40, &Marks::new(false));
        let at = format!("({}, {})", pos.x, pos.y);
        assert_eq!(i.right, at);
        let rows: Vec<String> = i.all_rows().iter().map(Row::plain).collect();
        assert!(rows.iter().any(|r| r.starts_with("Elevation ")), "{rows:?}");
        assert!(rows.iter().all(|r| !r.contains(&at)), "{rows:?}");
    }

    /// A heading too narrow for its value gives the value to the first row, and one too narrow
    /// for its name gives that to the rows as well.
    #[test]
    fn a_heading_that_does_not_fit_hands_its_words_to_the_rows() {
        let m = Marks::new(false);
        let body = || vec![vec![Row::text("body", Tone::Text)]];
        let short = Inspector::new("Wood".into(), "(94, 64)".into(), body(), 28, (1, 1), &m);
        assert_eq!((short.title.as_str(), short.right.as_str()), ("Wood", "(94, 64)"));
        let long =
            Inspector::new("Plains on the coast".into(), "(94, 64)".into(), body(), 28, (1, 1), &m);
        assert_eq!((long.title.as_str(), long.right.as_str()), ("Plains on the coast", ""));
        assert_eq!(long.all_rows()[0].plain(), "(94, 64)");
        assert_eq!(long.floor(), 2);
        let huge = Inspector::new(
            "A very long name that does not fit".into(),
            "".into(),
            body(),
            20,
            (1, 1),
            &m,
        );
        assert!(huge.title.ends_with('…') && huge.title.chars().count() <= 20, "{}", huge.title);
        assert_eq!(huge.all_rows()[0].plain(), "A very long name");
    }

    /// A stockpile zone takes the heading; what it holds comes first, the terrain after a blank.
    #[test]
    fn a_stockpile_zone_is_the_heading_and_the_terrain_comes_after_it() {
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
        let i = inspect(&s, &ui, None, 28, &Marks::new(false));
        assert_eq!(i.title, "Stockpile");
        let rows: Vec<String> = i.all_rows().iter().map(Row::plain).collect();
        assert_eq!(rows[0], "Empty", "{rows:?}");
        assert_eq!(rows[1], "", "{rows:?}");
        assert!(rows.iter().all(|r| !r.starts_with("Stockpile")), "{rows:?}");
    }

    /// A building's occupants come last (#226, #229), "Mai, idle" until #227 gives them
    /// something to do; none off a building or on one with nobody inside.
    #[test]
    fn a_building_s_inspector_lists_its_occupants_last() {
        use fn_sim::geom::Footprint;
        let mut s = Session::new(7, Nation::Fire, 64, 64);
        let hall = s.defs.building_id("study_hall").unwrap();
        let at = Pos::new(30, 30);
        for dy in 0..3 {
            for dx in 0..3 {
                let t = s.world.map.get_mut(Pos::new(at.x + dx, at.y + dy)).unwrap();
                t.terrain = fn_sim::Terrain::Land;
                t.resource = None;
            }
        }
        let foot = Footprint::new(at, 2, 2);
        let rows = |s: &Session, cursor: Pos| {
            let mut ui = UiState::new(s, (120, 40));
            ui.cursor = cursor;
            inspect(s, &ui, None, 28, &Marks::new(false)).all_rows()
        };
        assert!(rows(&s, at).iter().all(|r| !r.plain().contains(", idle")));

        s.world.put_building(foot, hall, &s.defs);
        s.world.units[0].pos = foot.entrance();
        let name = s.world.units[0].name.clone();
        assert!(rows(&s, at).iter().all(|r| !r.plain().contains(", idle")), "nobody inside yet");

        assert!(s.world.enter_building(0, at, &s.defs));
        assert_eq!(rows(&s, at).last().map(Row::plain), Some(format!("{name}, idle")));
    }

    /// With a settler selected and a tool active, the inspector describes the cursor's tile, which
    /// is what the tool acts on; the settler comes back when the tool is left.
    #[test]
    fn an_active_tool_points_the_inspector_at_the_cursor_not_the_settler() {
        let s = Session::new(7, Nation::Earth, 48, 32);
        let mut ui = UiState::new(&s, (120, 40));
        ui.select(Some(s.world.units[0].id));
        let name = s.world.units[0].name.clone();
        assert_eq!(inspect(&s, &ui, None, 28, &Marks::new(false)).title, name);
        ui.set_tool(Tool::Harvest);
        ui.cursor = Pos::new(1, 1);
        let i = inspect(&s, &ui, None, 28, &Marks::new(false));
        assert_ne!(i.title, name);
        assert_eq!(i.right, "(1, 1)");
    }

    /// The inspector's placement line (#275): `Can build here` where the chosen building
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
}

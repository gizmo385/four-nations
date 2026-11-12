//! The inspector as data (docs/design/side-panel-v2.md "The inspector", #653, #882): what the
//! player is looking at, said once for every frontend, with no width in sight. It is an
//! [`Inspected`]: a heading (the thing's name and a right-hand value) and groups of [`Part`]s.
//! A frontend lays it out: `fn_tui::inspector` wraps its text to a column of cells, draws each
//! meter as a row of cells and fits the groups into the rows it has; a window frontend wraps
//! with its own labels, draws a meter as a bar and scrolls instead of dropping a group.
//!
//! * **A selected settler** (`Enter`, `Tab`) is its task, what it carries, the five meters (the
//!   word "low" after any that is not ok, so they read without colour), its progress to the next
//!   rank, and the tile it stands on. Chi is in the accent and never warns: it can't hurt the
//!   unit.
//! * **Anything else** is what is under the cursor: the tile's own lines
//!   ([`tooltip_lines`]), a settler standing there shown with `Enter` to select it. While a tool
//!   is active the cursor is what the tool acts on, so the inspector describes it (with the
//!   Build tool's "Can build here" first) even with a settler still selected.
//!
//! The cursor's position is said once: in the heading for a tile, on the "Standing on" line for
//! a settler.

use fn_sim::Pos;

use fn_sim::building::BuildingId;
use fn_sim::world::{BuildError, CommandError, EndeavorError, Task, Unit, UnitId};

use crate::build;
use crate::inspect::{Hint, Line, ground_phrase, tile_lines, tooltip_lines};
use crate::rows::Tone;
use crate::session::Session;
use crate::state::UiState;
use crate::text::describe_stack;
use crate::tools::Tool;
use crate::view::{Status, UnitView, settler_view, unit_view};

/// The most rows of a tile a short panel always keeps ([`Floor::Few`]).
pub const TILE_FLOOR: usize = 3;

/// One piece of a group.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    /// A paragraph, for the frontend to wrap.
    Text(String, Tone),
    /// A line that is never wrapped ("Carrying nothing", a hint).
    Line(String, Tone),
    /// A dim paragraph that ends with a position: the frontend keeps `tail` in one piece, on the
    /// last line if it fits there and on a line of its own if not.
    Standing { text: String, tail: String },
    /// A meter: `Hunger 72%`, then "low" in the warning's tone when it is not ok.
    Meter { label: &'static str, frac: f32, tone: Tone, low: bool },
}

/// How many rows a short panel must always leave the inspector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Floor {
    /// Every row of the groups that are never dropped ([`Inspected::keep`]).
    Keep,
    /// At most this many rows altogether.
    Few(usize),
}

/// What the inspector shows, before a frontend knows how much room it gets.
#[derive(Clone, Debug, PartialEq)]
pub struct Inspected {
    /// What the heading names.
    pub title: String,
    /// The heading's right-hand value (the tile's position, a settler's rank); empty for none.
    pub right: String,
    /// Groups of parts, in order. A frontend puts a blank row between neighbours when there is
    /// room.
    pub groups: Vec<Vec<Part>>,
    /// How many of the first groups are never dropped whole: a short panel cuts rows from the
    /// last of them instead.
    pub keep: usize,
    /// What a short panel must always leave.
    pub floor: Floor,
    /// The settler (or beast) it describes; `None` for a tile.
    pub unit: Option<UnitId>,
}

impl Inspected {
    /// Every text of the model, in order: the heading, then each part (a meter as its label).
    /// For tests.
    pub fn plain(&self) -> Vec<String> {
        let mut out = vec![self.title.clone(), self.right.clone()];
        for part in self.groups.iter().flatten() {
            out.push(match part {
                Part::Text(t, _) | Part::Line(t, _) => t.clone(),
                Part::Standing { text, tail } => format!("{text} {tail}"),
                Part::Meter { label, .. } => (*label).to_owned(),
            });
        }
        out
    }
}

/// The meters, top to bottom: health, hunger, thirst, energy, then chi (always in the accent).
pub fn meters(view: &UnitView) -> Vec<Part> {
    let mut parts: Vec<Part> = view
        .needs
        .iter()
        .map(|n| Part::Meter {
            label: n.label,
            frac: n.frac,
            tone: Tone::of(n.status),
            low: n.status != Status::Ok,
        })
        .collect();
    parts.push(Part::Meter { label: "Chi", frac: view.chi_frac, tone: Tone::Accent, low: false });
    parts
}

/// The inspector for the current selection, cursor and tool.
pub fn inspect(session: &Session, ui: &UiState, choice: Option<BuildingId>) -> Inspected {
    let settler = if ui.tool == Tool::Select { ui.selected_unit(session) } else { None };
    match settler {
        Some(unit) if unit.is_beast() => beast(session, unit),
        Some(unit) => settler_parts(session, unit),
        None => tile(session, ui, choice),
    }
}

/// The look of a task line: the Avatar holding the Harmony Rite (#238, #242) is bold on the
/// nation's accent, so the Avatar State reads apart from anything else he could be doing.
pub fn task_tone(is_avatar: bool, task: Option<Task>) -> Tone {
    if is_avatar && task == Some(Task::Rite) { Tone::Avatar } else { Tone::Text }
}

/// A selected settler: its task, carrying, meters, rank progress and the tile it stands on.
fn settler_parts(session: &Session, unit: &Unit) -> Inspected {
    let view = settler_view(session, unit);
    let mut doing = Vec::new();
    // The Avatar belongs to no nation of his own, so the panel's nation is not his: say whose
    // guest he is.
    if unit.is_avatar() {
        let mut guest = view.nation.clone();
        if let Some(first) = guest.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        doing.push(Part::Text(guest, Tone::Dim));
    }
    doing.push(Part::Text(view.task.clone(), task_tone(unit.is_avatar(), unit.task())));
    doing.push(Part::Line(format!("Carrying {}", view.carrying), Tone::Text));
    let rank = vec![Part::Text(view.mastery_line.clone(), Tone::Text)];
    let tail = format!("({}, {})", view.tile.x, view.tile.y);
    let standing = match &view.inside {
        Some(building) => format!("In the {building}"),
        None => {
            let ground = ground_phrase(session, view.tile).unwrap_or_default().to_lowercase();
            format!("Standing on {ground}")
        }
    };
    Inspected {
        title: view.name.clone(),
        right: view.rank_title.clone(),
        groups: vec![doing, meters(&view), rank, vec![Part::Standing { text: standing, tail }]],
        keep: 2,
        floor: Floor::Keep,
        unit: Some(unit.id),
    }
}

/// A selected beast (#699): its name, what it is doing and carrying, its home and the home's
/// food, and the tile it stands on. It has no meters, rank or chi to show: a beast has no needs,
/// and never bends or learns.
fn beast(session: &Session, unit: &Unit) -> Inspected {
    let view = unit_view(session, unit);
    let doing = vec![
        Part::Text(view.task.clone(), Tone::Text),
        Part::Line(format!("Carrying {}", view.carrying), Tone::Text),
    ];
    let home: Vec<Part> = crate::inspect::beast_home_lines(session, unit)
        .into_iter()
        .map(|line| Part::Text(line, Tone::Text))
        .collect();
    let tail = format!("({}, {})", view.tile.x, view.tile.y);
    let ground = ground_phrase(session, view.tile).unwrap_or_default().to_lowercase();
    let standing = vec![Part::Standing { text: format!("Standing on {ground}"), tail }];
    Inspected {
        title: view.name,
        right: "Beast".to_owned(),
        groups: vec![doing, home, standing],
        keep: 2,
        floor: Floor::Keep,
        unit: Some(unit.id),
    }
}

/// What the Build tool's choice would do on the cursor's tile: whether it fits, or the reason it
/// doesn't (`build::placement`). Empty with nothing placeable.
fn placement_parts(session: &Session, pos: Pos, choice: Option<BuildingId>) -> Vec<Part> {
    let Some(choice) = choice else { return Vec::new() };
    match build::placement(session, pos, choice).refused {
        None => vec![Part::Line("Can build here".to_owned(), Tone::Ok)],
        Some(why) => {
            let why = build::describe_error(session, CommandError::Build(why));
            vec![Part::Text(format!("Can't build here: {why}"), Tone::Warn)]
        }
    }
}

/// Whether the gate fits with the cursor on `pos` (#643), or why not, as [`placement_parts`] says
/// it for the Build tool's choice.
fn gate_placement_parts(session: &Session, pos: Pos) -> Vec<Part> {
    let Some(placement) = crate::gate::placement(session, pos) else { return Vec::new() };
    let why = match placement.refused {
        None => return vec![Part::Line("Can place the gate here".to_owned(), Tone::Ok)],
        Some(BuildError::CannotBuild { at, why }) => {
            crate::gate::describe_error(session, EndeavorError::CannotBuild { at, why })
        }
        Some(_) => crate::gate::describe_error(session, EndeavorError::NotReady),
    };
    vec![Part::Text(format!("Can't place the gate here: {why}"), Tone::Warn)]
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

/// How an action hint reads, if it can be taken yet.
///
/// * Selecting a unit names its key.
/// * Starting a tool names its key.
/// * Applying the active tool is the rectangle line, which the active tool banner shows in every
///   tool (#307), so the tooltip's own hint adds nothing.
pub fn hint_text(hint: Hint) -> Option<String> {
    match hint {
        Hint::SelectUnit => Some("Press Enter to select".to_owned()),
        Hint::ApplyTool(_) => None,
        Hint::StartTool(tool) => Some(format!("Press {} for the {} tool", tool.key(), tool.name())),
    }
}

/// What is under the cursor: whether the Build tool's choice would fit there, then the tile's
/// own tooltip, then anything else lying on it.
///
/// The heading is the first thing the tooltip names -- a settler standing there, a building, a
/// resource or the terrain -- and the cursor's position; a stockpile zone takes the heading
/// itself ("STOCKPILE") with what it holds first and the terrain under it after. The unit
/// standing there, if any, is the tooltip's own job (`tooltip_lines`) -- unless it is the unit
/// already selected, which falls back to the tile's own lines (`tile_lines`, #418).
fn tile(session: &Session, ui: &UiState, choice: Option<BuildingId>) -> Inspected {
    let pos = ui.cursor;
    let mut lead = Vec::new();
    if ui.tool == Tool::Build {
        lead.extend(placement_parts(session, pos, choice));
    }
    if ui.tool == Tool::Gate {
        lead.extend(gate_placement_parts(session, pos));
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
    let mut stock = Vec::new();
    if let Some(text) = stockpile_text(session, pos) {
        if shows_unit {
            // A settler's tooltip owns the heading; the zone is one more line.
            lines.push(Line::Detail(format!("Stockpile: {text}")));
        } else {
            title = Some("Stockpile".to_owned());
            stock = vec![Part::Text(text, Tone::Text)];
        }
    }
    // The first title is the heading, unless the stockpile has taken it: then it is the terrain
    // line under what the zone holds.
    let mut rows = Vec::new();
    for line in lines {
        match line {
            Line::Title(s) if title.is_none() => title = Some(s),
            Line::Title(s) => rows.push(Part::Text(s, Tone::Strong)),
            Line::Detail(s) => rows.push(Part::Text(s, Tone::Text)),
            Line::Hint(h) => {
                if let Some(s) = hint_text(h) {
                    rows.push(Part::Line(s, Tone::Dim));
                }
            }
        }
    }
    // A building with anyone inside lists them last ("Mai, studying", #226, #229): `Enter`
    // cycles onto one the same way it cycles several units sharing a tile (`UiState::select_here`).
    for occupant in build::occupant_rows(session, pos) {
        rows.push(Part::Line(occupant.text(), Tone::Text));
    }
    let head: Vec<Part> = lead.into_iter().chain(stock).collect();
    let groups = if head.is_empty() { vec![rows] } else { vec![head, rows] };
    Inspected {
        title: title.unwrap_or_else(|| "Nothing".to_owned()),
        right: format!("({}, {})", pos.x, pos.y),
        groups,
        keep: 1,
        floor: Floor::Few(TILE_FLOOR),
        unit: None,
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;

    #[test]
    fn a_unit_s_meters_flag_exactly_the_ones_that_are_not_ok() {
        let mut s = Session::new(7, Nation::Earth, 48, 32);
        let id = s.world.units[0].id;
        s.world.units[0].needs.energy = 0.0;
        let unit = s.world.units.iter().find(|u| u.id == id).unwrap();
        let parts = meters(&unit_view(&s, unit));
        assert_eq!(parts.len(), 5, "health, hunger, thirst, energy and chi");
        let low: Vec<&str> = parts
            .iter()
            .filter_map(|p| match p {
                Part::Meter { label, low: true, .. } => Some(*label),
                _ => None,
            })
            .collect();
        assert_eq!(low, ["Energy"]);
        assert!(matches!(
            parts[4],
            Part::Meter { label: "Chi", tone: Tone::Accent, low: false, .. }
        ));
    }

    #[test]
    fn a_selected_settler_is_task_carrying_meters_rank_and_tile() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let mut ui = UiState::new(&s, (120, 40));
        ui.select(Some(s.world.units[0].id));
        let view = unit_view(&s, &s.world.units[0]);
        let i = inspect(&s, &ui, None);
        assert_eq!((i.title.as_str(), i.right.as_str()), (view.name.as_str(), "Student"));
        assert_eq!(i.unit, Some(s.world.units[0].id));
        assert_eq!(i.groups.len(), 4);
        assert_eq!(i.groups[0][0], Part::Text(view.task, Tone::Text));
        assert_eq!(i.groups[0][1], Part::Line("Carrying nothing".to_owned(), Tone::Text));
        assert_eq!(i.groups[1].len(), 5);
        let Part::Standing { text, tail } = &i.groups[3][0] else { panic!("{:?}", i.groups[3]) };
        assert!(text.starts_with("Standing on "), "{text}");
        assert_eq!(*tail, format!("({}, {})", view.tile.x, view.tile.y));
    }

    #[test]
    fn the_avatar_state_marks_the_task_line_apart() {
        assert_eq!(task_tone(true, Some(Task::Rite)), Tone::Avatar);
        assert_eq!(task_tone(true, None), Tone::Text);
        assert_eq!(task_tone(false, Some(Task::Rite)), Tone::Text);
    }

    #[test]
    fn a_tile_is_named_with_its_position_once() {
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
        let i = inspect(&s, &ui, None);
        let at = format!("({}, {})", pos.x, pos.y);
        assert_eq!((i.right.as_str(), i.unit), (at.as_str(), None));
        assert!(i.plain()[2..].iter().all(|t| !t.contains(&at)), "{:?}", i.plain());
    }

    #[test]
    fn the_placement_part_says_whether_the_chosen_building_fits() {
        let session = Session::new(7, Nation::Earth, 48, 32);
        let bed = session.defs.building_id("bed").unwrap();
        assert!(placement_parts(&session, Pos::new(-1, -1), None).is_empty());
        let off = placement_parts(&session, Pos::new(-1, -1), Some(bed));
        assert_eq!(off, [Part::Text("Can't build here: off the map".to_owned(), Tone::Warn)]);
    }
}

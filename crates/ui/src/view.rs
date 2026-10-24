//! What the panels show, as plain data: the side panel and the selected unit. Pure functions over
//! the `Session`, so a frontend can compare a view with last frame's and redraw only when it
//! changed, and so the tests cover what the panels say.

use fn_sim::Pos;
use fn_sim::building::BuildingId;
use fn_sim::nation::Modifiers;
use fn_sim::needs::NeedDef;
use fn_sim::world::{TICKS_PER_SECOND, Unit, UnitId};

use crate::alerts::Alerts;
use crate::build::{self, PickerRow};
use crate::research::{ResearchStatus, research_status};
use crate::session::Session;
use crate::text::{describe_stack, describe_task};
use crate::tools::Tool;

/// Everything the side panel shows.
#[derive(Clone, PartialEq, Debug)]
pub struct SideView {
    pub nation: String,
    pub seed: u64,
    pub people: usize,
    /// In-game time, `MM:SS` (`format_clock`).
    pub clock: String,
    pub paused: bool,
    /// The clock's speed as a multiple of real time.
    pub speed: u32,
    pub tool: Tool,
    /// What the current tool does, shown under its buttons. Generic for most tools; for `Bend`
    /// it names the nation's actual ability (#72).
    pub tool_hint: String,
    pub notice: String,
    /// The building picker's rows, listed while the Build tool is active (#111); empty otherwise.
    pub picker: Vec<PickerRow>,
    /// The building the Build tool would place, if it can place any.
    pub choice: Option<BuildingId>,
    /// The nation's modifiers that differ from 1.0, as (label, value).
    pub modifiers: Vec<(String, String)>,
    /// The tech the knowledge pool is currently paying for, its progress and rate (#23).
    pub research: ResearchStatus,
    /// The alert toasts showing now, oldest first. Empty most of the time, which hides the
    /// section (ui.md: "a section with nothing to show hides itself").
    pub alerts: Vec<String>,
}

/// The side panel for `session`. `notice` is the last designation's result and `choice` the
/// Build tool's building; both belong to the frontend.
pub fn side_view(
    session: &Session,
    tool: Tool,
    notice: &str,
    choice: Option<BuildingId>,
    alerts: &Alerts,
) -> SideView {
    let (picker, choice) = if tool == Tool::Build {
        let rows = build::picker_rows(session);
        let choice = build::valid_choice(&rows, choice);
        (rows, choice)
    } else {
        (Vec::new(), None)
    };
    let (world, defs) = (&session.world, &session.defs);
    SideView {
        nation: defs.nation(world.nation).name.clone(),
        seed: world.seed,
        people: world.units.len(),
        clock: format_clock(world.tick),
        paused: session.clock.paused,
        speed: session.clock.speed(),
        tool,
        tool_hint: tool.hint(session),
        notice: notice.to_owned(),
        picker,
        choice,
        modifiers: modifier_rows(world.modifiers()),
        research: research_status(world.research(), defs),
        alerts: alerts.toast_lines(),
    }
}

/// What the stockpiles hold, as (item name, count), in the order of `data/items.toml`. Items with
/// none stored are left out, so a settlement with no stockpile has an empty list.
pub fn stores(session: &Session) -> Vec<(String, u32)> {
    let (world, defs) = (&session.world, &session.defs);
    defs.items
        .iter()
        .filter_map(|item| {
            let total = world.stored_total(defs.item_id(&item.id)?);
            (total > 0).then(|| (item.name.clone(), total))
        })
        .collect()
}

/// In-game time since the start as `MM:SS`. There is no calendar yet.
pub fn format_clock(tick: u64) -> String {
    let secs = tick / u64::from(TICKS_PER_SECOND);
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// The modifiers that aren't 1.0, as (label, "x1.6"). A modifier at its baseline says nothing.
pub fn modifier_rows(m: &Modifiers) -> Vec<(String, String)> {
    Modifiers::NAMES
        .iter()
        .filter_map(|name| Some((*name, m.get(name)?)))
        .filter(|(_, v)| (v - 1.0).abs() > 0.005)
        .map(|(name, v)| (humanize(name), multiplier(v)))
        .collect()
}

/// 1.6 becomes "x1.6" and 2.0 becomes "x2".
pub fn multiplier(v: f32) -> String {
    let s = format!("{v:.2}");
    format!("x{}", s.trim_end_matches('0').trim_end_matches('.'))
}

/// "move_speed" becomes "Move speed".
pub fn humanize(name: &str) -> String {
    let mut s = name.replace('_', " ");
    if let Some(first) = s.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    s
}

/// How a meter reads: the same colours for every nation (`Theme::ok`, `warn`, `bad`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Ok,
    Warn,
    Bad,
}

/// A meter at `frac` of full is `Ok` down to `low` (the level where a unit goes to fix it),
/// `Warn` down to half of that, and `Bad` below.
pub fn need_status(frac: f32, low: f32) -> Status {
    if frac >= low {
        Status::Ok
    } else if frac >= low / 2.0 {
        Status::Warn
    } else {
        Status::Bad
    }
}

/// Health has no `low` in the data; it reads as a warning at half.
pub const HEALTH_LOW: f32 = 0.5;

#[derive(Clone, PartialEq, Debug)]
pub struct NeedView {
    pub label: &'static str,
    /// In `[0, 1]`.
    pub frac: f32,
    pub status: Status,
}

impl NeedView {
    pub fn percent(&self) -> String {
        format!("{:.0}%", self.frac * 100.0)
    }
}

/// Everything the selection bar shows for a unit, except the portrait (a frontend picks art from
/// the unit's `variant`).
#[derive(Clone, PartialEq, Debug)]
pub struct UnitView {
    pub id: UnitId,
    pub name: String,
    pub nation: String,
    pub tile: Pos,
    pub task: String,
    pub carrying: String,
    pub needs: Vec<NeedView>,
    /// Chi, as a fraction of `data/bending.toml`'s `chi.max`. Not a `NeedView`: chi never hurts
    /// the unit, so it has no "low" threshold or warning colour (docs/design/bending.md).
    pub chi_frac: f32,
}

pub fn unit_view(session: &Session, unit: &Unit) -> UnitView {
    let defs = &session.defs;
    let s = &defs.needs;
    let meter = |label, value: f32, max: f32, low: f32| {
        let frac = (value / max).clamp(0.0, 1.0);
        NeedView { label, frac, status: need_status(frac, low) }
    };
    let low = |n: NeedDef| n.low / s.max();
    UnitView {
        id: unit.id,
        name: unit.name.clone(),
        nation: defs.nation(unit.nation).name.clone(),
        tile: unit.pos,
        task: describe_task(&session.world, defs, unit),
        carrying: describe_stack(defs, unit.carrying),
        needs: vec![
            meter("Health", unit.needs.health, s.health.max, HEALTH_LOW),
            meter("Hunger", unit.needs.hunger, s.max(), low(s.hunger)),
            meter("Thirst", unit.needs.thirst, s.max(), low(s.thirst)),
            meter("Energy", unit.needs.energy, s.max(), low(s.energy)),
        ],
        chi_frac: (unit.chi / defs.chi.max).clamp(0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;

    #[test]
    fn need_status_thresholds() {
        // Hunger's `low` is 40% of full.
        assert_eq!(need_status(1.0, 0.4), Status::Ok);
        assert_eq!(need_status(0.4, 0.4), Status::Ok);
        assert_eq!(need_status(0.39, 0.4), Status::Warn);
        assert_eq!(need_status(0.2, 0.4), Status::Warn);
        assert_eq!(need_status(0.19, 0.4), Status::Bad);
        assert_eq!(need_status(0.0, 0.4), Status::Bad);
    }

    #[test]
    fn stores_lists_only_what_the_stockpiles_hold() {
        use fn_sim::world::Command;
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        assert!(stores(&s).is_empty(), "a new game has stored nothing");
        // Harvest what is near the settlers and give them a stockpile to fill.
        let from = s.world.units[0].pos;
        let near: Vec<Pos> = s
            .world
            .map
            .iter()
            .filter(|(p, t)| {
                (p.x - from.x).abs() < 12
                    && (p.y - from.y).abs() < 8
                    && t.resource.is_some_and(|r| s.defs.harvest(r).is_some())
            })
            .map(|(p, _)| p)
            .collect();
        for p in near {
            let _ = s.apply(Command::DesignateHarvest { pos: p });
        }
        let mut zone: Vec<Pos> = s
            .world
            .map
            .iter()
            .filter(|(_, t)| t.terrain == fn_sim::Terrain::Land && t.resource.is_none())
            .map(|(p, _)| p)
            .collect();
        zone.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
        for pos in zone.into_iter().take(4) {
            s.apply(Command::DesignateStockpile { pos }).unwrap();
        }
        assert!(stores(&s).is_empty(), "an empty zone is not a store");
        for _ in 0..40 {
            s.tick_n(100);
            if !stores(&s).is_empty() {
                break;
            }
        }
        let rows = stores(&s);
        assert!(!rows.is_empty(), "the settlers should have stored something");
        for (name, count) in rows {
            let item = s.defs.items.iter().find(|i| i.name == name).unwrap();
            assert_eq!(count, s.world.stored_total(s.defs.item_id(&item.id).unwrap()));
        }
    }

    #[test]
    fn clock_counts_minutes_and_seconds() {
        let tps = u64::from(TICKS_PER_SECOND);
        assert_eq!(format_clock(0), "00:00");
        assert_eq!(format_clock(59 * tps + tps - 1), "00:59");
        assert_eq!(format_clock(60 * tps), "01:00");
        assert_eq!(format_clock((100 * 60 + 5) * tps), "100:05");
    }

    #[test]
    fn modifier_rows_skip_baseline_and_format_values() {
        let defs = fn_sim::Defs::bundled();
        let m = defs.nation(Nation::Air).modifiers;
        let rows = modifier_rows(&m);
        assert!(rows.contains(&("Move speed".to_owned(), "x1.6".to_owned())), "{rows:?}");
        assert!(rows.contains(&("Power generation".to_owned(), "x2".to_owned())), "{rows:?}");
        assert!(rows.iter().all(|(label, _)| label != "Heating"), "baseline shown: {rows:?}");
    }

    #[test]
    fn humanize_capitalises_and_spaces() {
        assert_eq!(humanize("water_work"), "Water work");
        assert_eq!(humanize("study"), "Study");
    }

    #[test]
    fn the_build_tool_lists_the_picker_and_other_tools_do_not() {
        let s = Session::new(7, Nation::Earth, 64, 64);
        let alerts = Alerts::default();
        let build_view = side_view(&s, Tool::Build, "", None, &alerts);
        assert_eq!(build_view.picker.len(), s.defs.buildings.len());
        let bed = s.defs.building_id("bed").unwrap();
        // With no choice yet the first placeable building is shown as the choice.
        assert!(build_view.picker.iter().any(|r| Some(r.id) == build_view.choice && !r.locked()));
        assert_eq!(side_view(&s, Tool::Build, "", Some(bed), &alerts).choice, Some(bed));
        let harvest = side_view(&s, Tool::Harvest, "", Some(bed), &alerts);
        assert!(harvest.picker.is_empty() && harvest.choice.is_none());
    }

    #[test]
    fn the_side_view_follows_the_clock() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let alerts = Alerts::default();
        let v = side_view(&s, Tool::Select, "done", None, &alerts);
        assert_eq!((v.paused, v.speed), (false, 1));
        assert_eq!(v.notice, "done");
        assert_eq!(v.people, s.world.units.len());
        assert_eq!(v.seed, 7);
        s.clock.toggle_pause();
        s.clock.faster();
        let v = side_view(&s, Tool::Select, "", None, &alerts);
        assert_eq!((v.paused, v.speed), (true, 2));
    }

    #[test]
    fn unit_view_has_every_meter_in_range() {
        let s = Session::new(7, Nation::Water, 64, 64);
        let v = unit_view(&s, &s.world.units[0]);
        let labels: Vec<_> = v.needs.iter().map(|n| n.label).collect();
        assert_eq!(labels, ["Health", "Hunger", "Thirst", "Energy"]);
        // New units start full.
        assert!(v.needs.iter().all(|n| n.frac == 1.0 && n.status == Status::Ok), "{v:?}");
        assert_eq!(v.needs[0].percent(), "100%");
        assert_eq!(v.chi_frac, 1.0, "a fresh unit's chi starts full too");
    }

    #[test]
    fn a_meter_below_its_low_mark_warns() {
        let mut s = Session::new(7, Nation::Water, 64, 64);
        s.world.units[0].needs.hunger = 0.0;
        let v = unit_view(&s, &s.world.units[0]);
        assert_eq!(v.needs[1].status, Status::Bad);
        assert_eq!(v.needs[0].status, Status::Ok);
    }
}

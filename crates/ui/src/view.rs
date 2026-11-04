//! What the panels show, as plain data: the side panel and the selected unit. Pure functions over
//! the `Session`, so a frontend can compare a view with last frame's and redraw only when it
//! changed, and so the tests cover what the panels say.

use fn_sim::Pos;
use fn_sim::building::BuildingId;
use fn_sim::nation::Modifiers;
use fn_sim::needs::NeedDef;
use fn_sim::world::{TICKS_PER_SECOND, Task, Unit, UnitId};
use fn_sim::{Defs, Nation, World};

use crate::alerts::Alerts;
use crate::build::{self, PickerRow};
use crate::endeavor::{EndeavorStatus, endeavor_status};
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
    /// The population by nation (#248), `population_by_nation`'s own doc.
    pub population: Vec<(String, usize)>,
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
    /// The tech the knowledge pool is currently paying for, its progress and rate (#23).
    pub research: ResearchStatus,
    /// What the stockpiles hold, in the order of the item data (`store_rows`).
    pub stores: Vec<StoreRow>,
    /// The end-game endeavor's progress (#235), or `None` when the data has none.
    pub endeavor: Option<EndeavorStatus>,
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
        population: population_by_nation(world, defs),
        clock: format_clock(world.tick),
        paused: session.clock.paused,
        speed: session.clock.speed(),
        tool,
        tool_hint: tool.hint(session),
        notice: notice.to_owned(),
        picker,
        choice,
        research: research_status(world.research(), defs),
        stores: store_rows(session),
        endeavor: endeavor_status(session),
        alerts: alerts.toast_lines(),
    }
}

/// One line of the Stores section: an item the stockpiles hold.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StoreRow {
    pub name: String,
    /// The item's icon name (`ItemDef::icon`); no current frontend draws it.
    pub icon: String,
    pub count: u32,
}

/// What the stockpiles hold, in the order of the item data (`data/jobs.toml`, then the foods of
/// `data/needs.toml`). Items with none stored are left out, so a settlement with no stockpile has
/// an empty list.
pub fn store_rows(session: &Session) -> Vec<StoreRow> {
    let (world, defs) = (&session.world, &session.defs);
    defs.items
        .iter()
        .filter_map(|item| {
            let count = world.stored_total(defs.item_id(&item.id)?);
            (count > 0).then(|| StoreRow {
                name: item.name.clone(),
                icon: item.icon.clone(),
                count,
            })
        })
        .collect()
}

/// What the stockpiles hold, as (item name, count): `store_rows` without the icons, for frontends
/// that show text only.
pub fn stores(session: &Session) -> Vec<(String, u32)> {
    store_rows(session).into_iter().map(|r| (r.name, r.count)).collect()
}

/// The population by nation (#248, the United City's mixed settlement, #240): each nation with
/// at least one living unit, in `Nation::ALL`'s order, as (name, count). The Avatar (#237) has
/// no nation of his own and isn't tallied.
pub fn population_by_nation(world: &World, defs: &Defs) -> Vec<(String, usize)> {
    Nation::ALL
        .into_iter()
        .map(|n| {
            let count =
                world.units.iter().filter(|u| !u.is_avatar() && world.nation_of(u) == n).count();
            (defs.nation(n).name.clone(), count)
        })
        .filter(|&(_, count)| count > 0)
        .collect()
}

/// Settlers grouped by what they're doing (docs/design/side-panel.md "People: counts, not
/// names"), in a **fixed order** so a group's place never jitters tick to tick (sorting by count
/// would).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Activity {
    Idle,
    Haul,
    Harvest,
    Farm,
    Build,
    Bend,
    Craft,
    Study,
    Gate,
    Eat,
    Sleep,
}

impl Activity {
    pub const ALL: [Activity; 11] = [
        Self::Idle,
        Self::Haul,
        Self::Harvest,
        Self::Farm,
        Self::Build,
        Self::Bend,
        Self::Craft,
        Self::Study,
        Self::Gate,
        Self::Eat,
        Self::Sleep,
    ];

    /// The short verb the panel's own line and the Nation screen's heading show ("Idle 9"):
    /// short because `Harvesting 3` doesn't fit beside two other groups in the panel's own width.
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Haul => "Haul",
            Self::Harvest => "Harvest",
            Self::Farm => "Farm",
            Self::Build => "Build",
            Self::Bend => "Bend",
            Self::Craft => "Craft",
            Self::Study => "Study",
            Self::Gate => "Gate",
            Self::Eat => "Eat",
            Self::Sleep => "Sleep",
        }
    }

    /// Which group a unit's task falls into (docs/design/side-panel.md's own table), or `Idle`
    /// with none.
    pub fn of(task: Option<Task>) -> Self {
        match task {
            None => Self::Idle,
            Some(Task::Haul(_) | Task::Deliver) => Self::Haul,
            Some(Task::Harvest(_)) => Self::Harvest,
            Some(Task::Sow(_) | Task::Reap(_)) => Self::Farm,
            Some(Task::Build(_)) => Self::Build,
            Some(Task::Bend(_)) => Self::Bend,
            Some(Task::Craft(_)) => Self::Craft,
            Some(Task::Study(_)) => Self::Study,
            Some(Task::Endeavor(_) | Task::Attend(_) | Task::Rite) => Self::Gate,
            Some(Task::Eat(_) | Task::Drink) => Self::Eat,
            Some(Task::Sleep) => Self::Sleep,
        }
    }
}

/// Settlers grouped by task, in [`Activity::ALL`]'s fixed order; zero counts are left out, so the
/// groups that show pack into a few rows (docs/design/side-panel.md "People: counts, not names").
pub fn activity_counts(session: &Session) -> Vec<(Activity, usize)> {
    let mut counts = [0usize; Activity::ALL.len()];
    for unit in &session.world.units {
        let activity = Activity::of(unit.task());
        let i =
            Activity::ALL.iter().position(|&a| a == activity).expect("every activity is listed");
        counts[i] += 1;
    }
    Activity::ALL.into_iter().zip(counts).filter(|&(_, n)| n > 0).collect()
}

/// The word a need's trouble wears, in the doc's own pairs (docs/design/side-panel.md "People:
/// counts, not names"): `Status::Warn` and `Status::Bad` each have their own; `Status::Ok` has
/// none. `label` is a [`NeedView::label`] ("Hunger", "Thirst", "Energy" or "Health").
pub fn need_word(label: &str, status: Status) -> Option<&'static str> {
    let (warn, bad) = match label {
        "Hunger" => ("Hungry", "Starving"),
        "Thirst" => ("Thirsty", "Parched"),
        "Energy" => ("Tired", "Exhausted"),
        "Health" => ("Hurt", "Dying"),
        _ => return None,
    };
    match status {
        Status::Ok => None,
        Status::Warn => Some(warn),
        Status::Bad => Some(bad),
    }
}

/// Counts of units in trouble, by [`need_word`]'s word, in the doc's fixed order (Hunger, Thirst,
/// Energy, Health; warn before bad within one): hidden words (count 0) are left out, so the row
/// shows only when it matters (docs/design/side-panel.md "People: counts, not names"). The
/// thresholds are `need_status`'s own (data/needs.toml), the same ones [`unit_view`]'s meters use.
pub fn need_flags(session: &Session) -> Vec<(String, usize)> {
    const WORDS: [&str; 8] =
        ["Hungry", "Starving", "Thirsty", "Parched", "Tired", "Exhausted", "Hurt", "Dying"];
    let mut counts = [0usize; WORDS.len()];
    for unit in &session.world.units {
        for n in &unit_view(session, unit).needs {
            if let Some(word) = need_word(n.label, n.status)
                && let Some(i) = WORDS.iter().position(|&w| w == word)
            {
                counts[i] += 1;
            }
        }
    }
    WORDS.into_iter().zip(counts).filter(|&(_, n)| n > 0).map(|(w, n)| (w.to_owned(), n)).collect()
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
    /// The building's display name while the unit is inside it (#226, #229), for the selection
    /// bar to read "In the Academy" instead of its tile: `tile` still names the door it went in
    /// by (`Unit::inside`), which is less useful to the player than what it's inside.
    pub inside: Option<String>,
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
    // The Avatar (#237) reads "The Avatar - guest of <nation>", not his own name and nation
    // (#242, the closed client issue #243's own wording): he belongs to no nation of his own,
    // and `nation_of` already falls back to the settlement's for exactly this case.
    let host = defs.nation(session.world.nation_of(unit)).name.clone();
    let (name, nation) = if unit.is_avatar() {
        ("The Avatar".to_owned(), format!("guest of {host}"))
    } else {
        (unit.name.clone(), host)
    };
    UnitView {
        id: unit.id,
        name,
        nation,
        tile: unit.pos,
        inside: unit
            .inside()
            .and_then(|anchor| session.world.building_at(anchor))
            .map(|b| defs.building(b).display_name(session.world.nation).to_owned()),
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

/// Everything the Nation screen (`N`, docs/design/side-panel.md "The Nation screen") shows: every
/// settler grouped by [`Activity`], in `World::units`' own order within a group; every store, not
/// just the panel's own two rows' worth; and the nation's traits (today's panel's own Modifiers,
/// moved here, since they only change when research does).
#[derive(Clone, PartialEq, Debug)]
pub struct NationView {
    pub nation: String,
    pub seed: u64,
    pub population: usize,
    /// In-game time, `MM:SS` (`format_clock`).
    pub clock: String,
    /// Groups with nobody in them are left out, in [`Activity::ALL`]'s fixed order.
    pub groups: Vec<(Activity, Vec<UnitView>)>,
    pub stores: Vec<StoreRow>,
    /// The nation's modifiers that differ from 1.0, as (label, value) (`modifier_rows`).
    pub traits: Vec<(String, String)>,
}

pub fn nation_view(session: &Session) -> NationView {
    let (world, defs) = (&session.world, &session.defs);
    let mut groups: Vec<(Activity, Vec<UnitView>)> =
        Activity::ALL.into_iter().map(|a| (a, Vec::new())).collect();
    for unit in &world.units {
        let activity = Activity::of(unit.task());
        let i =
            Activity::ALL.iter().position(|&a| a == activity).expect("every activity is listed");
        groups[i].1.push(unit_view(session, unit));
    }
    groups.retain(|(_, units)| !units.is_empty());
    NationView {
        nation: defs.nation(world.nation).name.clone(),
        seed: world.seed,
        population: world.units.len(),
        clock: format_clock(world.tick),
        groups,
        stores: store_rows(session),
        traits: modifier_rows(world.modifiers()),
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
        let rows = store_rows(&s);
        assert!(!rows.is_empty(), "the settlers should have stored something");
        let order: Vec<_> = rows.iter().map(|r| &r.name).collect();
        let data_order: Vec<_> = s.defs.items.iter().map(|i| &i.name).collect();
        assert!(order.iter().all(|n| data_order.contains(n)));
        assert!(
            order.windows(2).all(|w| {
                let at = |n: &&String| data_order.iter().position(|d| d == n);
                at(&w[0]) < at(&w[1])
            }),
            "rows follow the order of the item data"
        );
        for row in &rows {
            let item = s.defs.items.iter().find(|i| i.name == row.name).unwrap();
            assert_eq!(row.icon, item.icon);
            assert_eq!(row.count, s.world.stored_total(s.defs.item_id(&item.id).unwrap()));
        }
        assert_eq!(side_view(&s, Tool::Select, "", None, &Alerts::default()).stores, rows);
        assert_eq!(stores(&s).len(), rows.len());
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
        let earth_wall = s.defs.building_id("earth_wall").unwrap();
        let orchard = s.defs.building_id("orchard").unwrap();
        let listed: Vec<_> = build_view.picker.iter().map(|r| r.id).collect();
        assert!(listed.contains(&earth_wall) && !listed.contains(&orchard));
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

    /// `population_by_nation` (#248): a fresh game is entirely the settlement's own nation; a
    /// newcomer of another nation (#240) shows up as its own entry, in `Nation::ALL`'s order,
    /// and the Avatar (no nation of his own) isn't tallied at all.
    #[test]
    fn population_by_nation_tallies_each_nation_present_and_skips_the_avatar() {
        let mut s = Session::new(7, Nation::Fire, 64, 64);
        assert_eq!(
            population_by_nation(&s.world, &s.defs),
            [("Fire Nation".to_owned(), s.world.units.len())]
        );

        s.world.units[0].nation = Some(Nation::Earth);
        let by_nation = population_by_nation(&s.world, &s.defs);
        let earth_name = s.defs.nation(Nation::Earth).name.clone();
        let fire_name = s.defs.nation(Nation::Fire).name.clone();
        assert_eq!(
            by_nation,
            [(earth_name, 1), (fire_name, s.world.units.len() - 1)],
            "{by_nation:?}"
        );

        s.world.units[1].guest = Some(fn_sim::world::Guest::Avatar);
        let total: usize = population_by_nation(&s.world, &s.defs).iter().map(|&(_, c)| c).sum();
        assert_eq!(total, s.world.units.len() - 1, "the Avatar isn't tallied");
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
        assert_eq!(v.inside, None, "standing outside names no building");
    }

    /// A unit that stepped inside a building (#226, #229) names it in `inside`, in place of its
    /// tile; one standing anywhere else has `None` (the case above).
    #[test]
    fn unit_view_names_the_building_a_unit_is_inside() {
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
        for p in foot.tiles() {
            let part = [(p.x - at.x) as u8, (p.y - at.y) as u8];
            let b = Building { part, size: [2, 2], ..Building::new(hall, passable, false) };
            s.world.map.get_mut(p).unwrap().building = Some(b);
        }
        s.world.units[0].pos = foot.entrance();
        assert!(s.world.enter_building(0, at, &s.defs));
        let v = unit_view(&s, &s.world.units[0]);
        assert_eq!(v.inside.as_deref(), Some("Academy"), "Fire's name for the study hall");
    }

    #[test]
    fn a_meter_below_its_low_mark_warns() {
        let mut s = Session::new(7, Nation::Water, 64, 64);
        s.world.units[0].needs.hunger = 0.0;
        let v = unit_view(&s, &s.world.units[0]);
        assert_eq!(v.needs[1].status, Status::Bad);
        assert_eq!(v.needs[0].status, Status::Ok);
    }

    /// The Avatar (#237, #242) reads "The Avatar - guest of <nation>" in place of his own name
    /// and nation, whatever nation hosts him; an ordinary settler keeps its own.
    #[test]
    fn the_avatar_reads_as_a_guest_of_the_settlement_s_nation() {
        use fn_sim::world::Guest;
        let mut s = Session::new(7, Nation::Water, 64, 64);
        s.world.units[0].guest = Some(Guest::Avatar);
        let v = unit_view(&s, &s.world.units[0]);
        assert_eq!(v.name, "The Avatar");
        assert_eq!(v.nation, "guest of Water Tribes");

        let settler = unit_view(&s, &s.world.units[1]);
        assert_eq!(settler.name, s.world.units[1].name);
        assert_eq!(settler.nation, "Water Tribes");
    }

    /// Every `Task` variant falls into the activity table's group (docs/design/side-panel.md
    /// "People: counts, not names"), and `None` is Idle.
    #[test]
    fn every_task_variant_falls_into_its_doc_table_group() {
        use fn_sim::jobs::JobId;
        use fn_sim::world::FoodSource;
        let id = JobId(0);
        let cases = [
            (None, Activity::Idle),
            (Some(Task::Haul(id)), Activity::Haul),
            (Some(Task::Deliver), Activity::Haul),
            (Some(Task::Harvest(id)), Activity::Harvest),
            (Some(Task::Sow(id)), Activity::Farm),
            (Some(Task::Reap(id)), Activity::Farm),
            (Some(Task::Build(id)), Activity::Build),
            (Some(Task::Bend(id)), Activity::Bend),
            (Some(Task::Craft(id)), Activity::Craft),
            (Some(Task::Study(id)), Activity::Study),
            (Some(Task::Endeavor(id)), Activity::Gate),
            (Some(Task::Attend(id)), Activity::Gate),
            (Some(Task::Rite), Activity::Gate),
            (Some(Task::Eat(FoodSource::Carried)), Activity::Eat),
            (Some(Task::Drink), Activity::Eat),
            (Some(Task::Sleep), Activity::Sleep),
        ];
        for (task, want) in cases {
            assert_eq!(Activity::of(task), want, "{task:?}");
        }
    }

    /// A fresh settlement is every settler Idle, in `Activity::ALL`'s order, with zero counts
    /// left out.
    #[test]
    fn activity_counts_tallies_a_fresh_settlement_as_idle() {
        let s = Session::new(7, Nation::Earth, 48, 32);
        let counts = activity_counts(&s);
        assert_eq!(counts, [(Activity::Idle, s.world.units.len())]);
    }

    /// `need_word` pairs a need's label with the doc's own words, and says nothing for `Ok`.
    #[test]
    fn need_word_follows_the_doc_s_pairs() {
        assert_eq!(need_word("Hunger", Status::Warn), Some("Hungry"));
        assert_eq!(need_word("Hunger", Status::Bad), Some("Starving"));
        assert_eq!(need_word("Thirst", Status::Warn), Some("Thirsty"));
        assert_eq!(need_word("Energy", Status::Bad), Some("Exhausted"));
        assert_eq!(need_word("Health", Status::Warn), Some("Hurt"));
        assert_eq!(need_word("Hunger", Status::Ok), None);
        assert_eq!(need_word("Chi", Status::Bad), None);
    }

    /// A settlement with nobody in trouble has no flags; one unit starving and another
    /// exhausted shows both, in the doc's fixed order.
    #[test]
    fn need_flags_counts_units_in_trouble_and_hides_the_rest() {
        let mut s = Session::new(7, Nation::Earth, 48, 32);
        assert!(need_flags(&s).is_empty(), "a fresh settlement has nobody in trouble");
        s.world.units[0].needs.hunger = 0.0;
        s.world.units[1].needs.energy = 0.0;
        assert_eq!(need_flags(&s), [("Starving".to_owned(), 1), ("Exhausted".to_owned(), 1)]);
    }

    /// `nation_view` groups every settler the same way `activity_counts` does, lists every
    /// store (not just two rows' worth) and carries the panel's own modifier rows as traits.
    #[test]
    fn nation_view_groups_settlers_lists_every_store_and_carries_the_traits() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let view = nation_view(&s);
        assert_eq!(view.nation, s.defs.nation(Nation::Fire).name);
        assert_eq!(view.seed, 7);
        assert_eq!(view.population, s.world.units.len());
        assert_eq!(view.groups, [(Activity::Idle, view.groups[0].1.clone())]);
        assert_eq!(view.groups[0].1.len(), s.world.units.len());
        assert_eq!(view.traits, modifier_rows(s.world.modifiers()));
        assert_eq!(view.stores, store_rows(&s));
    }
}

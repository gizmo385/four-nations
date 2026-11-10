//! What the panels show, as plain data: the side panel and the selected unit. Pure functions over
//! the `Session`, so a frontend can compare a view with last frame's and redraw only when it
//! changed, and so the tests cover what the panels say.

use fn_sim::Pos;
use fn_sim::building::BuildingId;
use fn_sim::jobs::ItemId;
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
        research: research_status(world, defs),
        stores: store_rows(session),
        endeavor: endeavor_status(session),
        alerts: alerts.toast_lines(),
    }
}

/// One line of the Stores section: an item the stockpiles hold.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StoreRow {
    pub name: String,
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
            (count > 0).then(|| StoreRow { name: item.name.clone(), count })
        })
        .collect()
}

/// How full the stockpiles are (#509): how many tiles are designated as a stockpile zone, how
/// many of them currently hold something, and the per-tile cap every one shares
/// (`defs.jobs.stockpile_stack`, the same number `fn_ui::inspect::stockpile_line`'s own "12/20
/// Wood" divides by). A script reading `store(name)` alone can't tell "we're low" from "we're
/// full and the haulers have stopped" (#485's own playtest note); this is the other half of that
/// same question, read with `observe()`. `cap` is reported even with no zone designated at all
/// (`tiles == 0`): it's a setting, not a fact about the world right now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StockpileCapacity {
    pub tiles: u32,
    pub used: u32,
    pub cap: u32,
}

/// [`StockpileCapacity`] for `session`, right now.
pub fn stockpile_capacity(session: &Session) -> StockpileCapacity {
    let (world, defs) = (&session.world, &session.defs);
    let tiles = world.stockpile_zones().count() as u32;
    let used = world.stockpile_zones().filter(|&p| world.stored_at(p).is_some()).count() as u32;
    StockpileCapacity { tiles, used, cap: defs.jobs.stockpile_stack }
}

/// Items lying on the ground with nowhere to go: every stockpile tile that could take them
/// already holds a different item, at its own full stack (#337, #378's own gap in the balance
/// harness -- `World::room_at`: a stockpile tile holds one item kind until it's emptied, so
/// whichever item fills every tile first strands every other one on the ground for good, with
/// nothing in the Stores section to show it since `store_rows` only lists what's actually
/// stored). Each entry is the item and how much of it sits stuck right now, in the item data's
/// own order. Empty with no stockpile designated at all: that's a different problem (nowhere to
/// store anything yet, not a full one).
pub fn stuck_items(session: &Session) -> Vec<(ItemId, u32)> {
    let (world, defs) = (&session.world, &session.defs);
    if world.stockpile_zones().next().is_none() {
        return Vec::new();
    }
    let cap = defs.jobs.stockpile_stack;
    let has_room = |item: ItemId, pos: Pos| match world.stored_at(pos) {
        None => true,
        Some(s) => s.item == item && s.count < cap,
    };
    defs.items
        .iter()
        .filter_map(|def| {
            let item = defs.item_id(&def.id)?;
            let count: u32 =
                world.ground_items().filter(|(_, s)| s.item == item).map(|(_, s)| s.count).sum();
            let stuck = count > 0 && !world.stockpile_zones().any(|pos| has_room(item, pos));
            stuck.then_some((item, count))
        })
        .collect()
}

/// What the stockpiles hold, as (item name, count): `store_rows` without the icons, for frontends
/// that show text only.
pub fn stores(session: &Session) -> Vec<(String, u32)> {
    store_rows(session).into_iter().map(|r| (r.name, r.count)).collect()
}

/// Non-food items the stockpiles hold (#416, docs/design/side-panel-v2.md "Zone 1"'s Materials
/// row): `stores`, minus every item with a `FoodDef` (the Food row already covers that, from
/// `food_in_stores`). In the item data's own order; empty items are left out, same as `stores`.
pub fn materials(session: &Session) -> Vec<(String, u32)> {
    let (world, defs) = (&session.world, &session.defs);
    defs.items
        .iter()
        .filter(|item| item.food.is_none())
        .filter_map(|item| {
            let count = world.stored_total(defs.item_id(&item.id)?);
            (count > 0).then(|| (item.name.clone(), count))
        })
        .collect()
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
    Heal,
}

impl Activity {
    pub const ALL: [Activity; 12] = [
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
        Self::Heal,
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
            Self::Heal => "Heal",
        }
    }

    /// Which group a unit's task falls into (docs/design/side-panel.md's own table), or `Idle`
    /// with none.
    pub fn of(task: Option<Task>) -> Self {
        match task {
            None => Self::Idle,
            Some(Task::Haul(_) | Task::Deliver) => Self::Haul,
            // Clearing ground (#424) groups with harvesting until the Clear tool gets its own
            // panel row (#426).
            Some(Task::Harvest(_) | Task::Clear(_)) => Self::Harvest,
            Some(Task::Sow(_) | Task::Reap(_)) => Self::Farm,
            Some(Task::Build(_)) => Self::Build,
            Some(Task::Bend(_)) => Self::Bend,
            Some(Task::Craft(_)) => Self::Craft,
            // Charting (#402) groups with studying until the Map Room gets its own panel row
            // (#403): both are a seat worked at the same low priority.
            Some(Task::Study(_) | Task::Chart(_)) => Self::Study,
            Some(Task::Endeavor(_) | Task::Attend(_) | Task::Rite) => Self::Gate,
            Some(Task::Eat(_) | Task::Drink) => Self::Eat,
            Some(Task::Sleep) => Self::Sleep,
            // Taking a remedy (issue #390) heals, like resting in a healing building: it groups
            // with Heal, not Eat, even though it runs through the same walk-then-consume code
            // Drink and Eat do.
            Some(Task::Healing(_) | Task::Remedy(_)) => Self::Heal,
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

/// Seconds as "4:10 left" (minutes with no leading zero, seconds zero-padded):
/// `docs/design/difficulty.md` ("TUI"), the Season section's heading.
fn clock_left(secs: f32) -> String {
    let secs = secs.max(0.0).round() as u64;
    format!("{}:{:02} left", secs / 60, secs % 60)
}

/// Food items in the stockpiles, summed across every `FoodDef` (`docs/design/difficulty.md`
/// "TUI": the Season section's "Food in stores" line, #299).
pub fn food_in_stores(session: &Session) -> u32 {
    let (world, defs) = (&session.world, &session.defs);
    defs.items
        .iter()
        .filter(|i| i.food.is_some())
        .filter_map(|i| defs.item_id(&i.id))
        .map(|id| world.stored_total(id))
        .sum()
}

/// A climate band's effects, lower case, in the doc's own pairs ("cold, frost"), or "mild" with
/// neither (`docs/design/difficulty.md` "TUI" and "Embark"; `embark.md`'s own report shares the
/// words, not the function, since it also names the band).
pub fn climate_effects(climate: &fn_sim::seasons::ClimateDef) -> String {
    let mut effects = Vec::new();
    if climate.cold {
        effects.push("cold");
    }
    if climate.frost {
        effects.push("frost");
    }
    if effects.is_empty() { "mild".to_owned() } else { effects.join(", ") }
}

/// The side panel's Season section (`docs/design/difficulty.md` "TUI",
/// `docs/design/side-panel.md` "How it fits #299 and #306"): shown under People whenever the
/// chosen difficulty preset has seasons on (#295); `None` otherwise, which hides the section.
#[derive(Clone, PartialEq, Debug)]
pub struct SeasonView {
    /// The heading's title: the season and the year ("Autumn · Y1"; the divider format upper
    /// cases it, docs/design/side-panel.md "Headings").
    pub heading: String,
    /// The heading's right-hand value: time left in the season ("4:10 left").
    pub left: String,
    /// "38 · 4/head": what's in the stockpiles against the population, the Season section's
    /// second row.
    pub food: String,
    /// Whether `food` should read as a warning: autumn or winter, below the caller's
    /// `winter_food_per_head` threshold (presentation, not a sim rule, so the caller -- which
    /// reads `data/tui.toml` -- decides the number).
    pub food_warn: bool,
    /// The climate band's winter effects at the settlement's start (or the map's centre, for a
    /// `World::new` with no embark site), lower case ("mild", "cold, frost"): the side panel's
    /// Food row appends this as the winter outlook word (#416, docs/design/side-panel-v2.md
    /// "Zone 1"), toned Warn exactly when `food_warn` is.
    pub outlook: String,
}

/// `SeasonView` for `session`'s world, or `None` with seasons off. `winter_food_per_head` is the
/// frontend's own threshold (`data/tui.toml` for the TUI), not a sim number.
pub fn season_view(session: &Session, winter_food_per_head: u32) -> Option<SeasonView> {
    let (world, defs) = (&session.world, &session.defs);
    if !world.knobs(defs).seasons {
        return None;
    }
    let season = world.season(defs);
    let year = world.year(defs);
    let n = defs.seasons.seasons.len();
    let idx = world.season_index(defs);
    let is_autumn = n > 0 && (idx + 1) % n == defs.seasons.winter_index();
    let food = food_in_stores(session);
    let people = (world.units.len() as u32).max(1);
    let per_head = food / people;
    let food_warn = (is_autumn || season.winter) && per_head < winter_food_per_head;
    let start =
        world.start().unwrap_or_else(|| Pos::new(world.map.width() / 2, world.map.height() / 2));
    let outlook = world.climate_at(start, defs).map(climate_effects).unwrap_or_default();
    Some(SeasonView {
        heading: format!("{} · Y{year}", season.name),
        left: clock_left(world.season_left(defs)),
        food: format!("{food} · {per_head}/head"),
        food_warn,
        outlook,
    })
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
pub use fn_sim::needs::HEALTH_LOW;

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

/// Everything the selection bar shows for a unit, except the portrait.
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
    /// What this nation calls a settler at its rank right now (`World::rank_of`,
    /// `data/bending.toml`'s `[mastery.titles]`, #360/#361): the selection bar's border
    /// ("Jee - Fire Nation - Firebender") and its progress line (`mastery_line`) share this, so
    /// the two can't drift apart.
    pub rank_title: String,
    /// The selection bar's fourth row (`crate::text::mastery_line`, #361): progress to the next
    /// rank, or the master's own line.
    pub mastery_line: String,
    /// The Nation screen's Bends and Forms columns (`crate::text::mastery_cells`, #346, #361):
    /// "9/10" and "2/3" below mastery, "12 *" and "4 *" once mastered.
    pub bends_cell: String,
    pub forms_cell: String,
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
    let own_nation = session.world.nation_of(unit);
    let host = defs.nation(own_nation).name.clone();
    let (name, nation) = if unit.is_avatar() {
        ("The Avatar".to_owned(), format!("guest of {host}"))
    } else {
        (unit.name.clone(), host)
    };
    let progress = session.world.rank_of(unit, defs);
    let rank_title = defs.mastery.title(own_nation, progress.rank).to_owned();
    let mastery_line = crate::text::mastery_line(progress, &rank_title, defs.mastery.master_chi);
    let (bends_cell, forms_cell) = crate::text::mastery_cells(
        progress,
        defs.mastery.master_bends_for(own_nation),
        defs.mastery.master_forms,
    );
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
        rank_title,
        mastery_line,
        bends_cell,
        forms_cell,
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
            assert_eq!(row.count, s.world.stored_total(s.defs.item_id(&item.id).unwrap()));
        }
        assert_eq!(side_view(&s, Tool::Select, "", None, &Alerts::default()).stores, rows);
        assert_eq!(stores(&s).len(), rows.len());
    }

    #[test]
    fn stockpile_capacity_counts_designated_tiles_and_how_many_hold_something() {
        use fn_sim::world::Command;
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let empty = stockpile_capacity(&s);
        assert_eq!((empty.tiles, empty.used), (0, 0), "no zone designated yet");
        assert_eq!(empty.cap, s.defs.jobs.stockpile_stack, "cap is reported even with no zone");

        let from = s.world.units[0].pos;
        let mut zone: Vec<Pos> = s
            .world
            .map
            .iter()
            .filter(|(_, t)| t.terrain == fn_sim::Terrain::Land && t.resource.is_none())
            .map(|(p, _)| p)
            .collect();
        zone.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
        for pos in zone.into_iter().take(3) {
            s.apply(Command::DesignateStockpile { pos }).unwrap();
        }
        let designated = stockpile_capacity(&s);
        assert_eq!(designated.tiles, 3, "three zone tiles now designated");
        assert_eq!(designated.used, 0, "nothing has reached them yet");

        // Harvest what is near the settlers so there is something to haul into the zone.
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
        for _ in 0..40 {
            s.tick_n(100);
            if stockpile_capacity(&s).used > 0 {
                break;
            }
        }
        let filled = stockpile_capacity(&s);
        assert!(filled.used > 0, "something should have reached a stockpile tile by now");
        assert!(filled.used <= filled.tiles);
        assert_eq!(filled.tiles, 3, "designating more never happened in this test");
    }

    /// `materials` (#416, the side panel's Materials row): `stores` minus every food item, same
    /// order.
    #[test]
    fn materials_excludes_food_items_but_keeps_stores_own_order() {
        use fn_sim::world::Command;
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        assert!(materials(&s).is_empty(), "a new game has stored nothing");
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
        for pos in zone.into_iter().take(6) {
            s.apply(Command::DesignateStockpile { pos }).unwrap();
        }
        for _ in 0..60 {
            s.tick_n(100);
            if !stores(&s).is_empty() {
                break;
            }
        }
        let all = stores(&s);
        assert!(!all.is_empty(), "the settlers should have stored something");
        let mats = materials(&s);
        for (name, _) in &mats {
            let item = s.defs.items.iter().find(|i| &i.name == name).unwrap();
            assert!(item.food.is_none(), "{name} is food, should be left out");
        }
        let expected: Vec<(String, u32)> = all
            .into_iter()
            .filter(|(name, _)| {
                s.defs.items.iter().find(|i| &i.name == name).unwrap().food.is_none()
            })
            .collect();
        assert_eq!(mats, expected);
    }

    /// #337, #378's own gap in the balance harness: a single stockpile tile holds one item kind
    /// until it's emptied, so once it holds any stone at all, wood chopped afterwards has
    /// nowhere to go and `stuck_items` says so, even though `store_rows` never will (nothing of
    /// it is ever stored).
    #[test]
    fn stuck_items_flags_wood_once_the_only_zone_already_holds_stone() {
        use fn_sim::defs::BiomeId;
        use fn_sim::world::Command;
        use fn_sim::{Grid, Terrain, Tile};
        let defs = Defs::bundled();
        let stone = defs.resource_id("stone").unwrap();
        let oak = defs.resource_id("oak_tree").unwrap();
        let (stone_item, wood_item) =
            (defs.item_id("stone").unwrap(), defs.item_id("wood").unwrap());
        let map = Grid::from_fn(10, 3, |p| Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: if p == Pos::new(3, 1) { Terrain::Mountain } else { Terrain::Land },
            biome: BiomeId(0),
            resource: match p {
                q if q == Pos::new(3, 1) => Some(stone),
                q if q == Pos::new(7, 1) => Some(oak),
                _ => None,
            },
            building: None,
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(0, 1)], &defs);
        let mut s =
            Session { world, defs, clock: crate::clock::Clock::default(), bend_choice: None };
        assert!(stuck_items(&s).is_empty(), "nothing designated yet");
        s.apply(Command::DesignateStockpile { pos: Pos::new(1, 1) }).unwrap();
        s.apply(Command::DesignateHarvest { pos: Pos::new(3, 1) }).unwrap();
        for _ in 0..3000 {
            s.tick();
            if s.world.stored_total(stone_item) > 0 {
                break;
            }
        }
        assert!(s.world.stored_total(stone_item) > 0, "stone should have reached the stockpile");
        s.apply(Command::DesignateHarvest { pos: Pos::new(7, 1) }).unwrap();
        for _ in 0..3000 {
            s.tick();
            if s.world.ground_items().any(|(_, st)| st.item == wood_item) {
                break;
            }
        }
        let stuck = stuck_items(&s);
        assert_eq!(stuck.len(), 1, "{stuck:?}");
        assert_eq!(stuck[0].0, wood_item);
        assert!(stuck[0].1 > 0, "{stuck:?}");
        assert_eq!(s.world.stored_total(wood_item), 0, "wood never reaches the full stockpile");
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

    /// `unit_view` plumbs `World::rank_of` and `data/bending.toml`'s titles through
    /// (`docs/design/bending.md` "Seeing it", #360/#361): a fresh unit is a novice, of Water's
    /// own title, and its cells read against Water's own bundled threshold (8, from
    /// `bends_per_nation`).
    #[test]
    fn unit_view_carries_rank_title_and_mastery_cells() {
        let s = Session::new(7, Nation::Water, 64, 64);
        let v = unit_view(&s, &s.world.units[0]);
        assert_eq!(v.rank_title, "Student");
        assert_eq!(v.bends_cell, "0/8");
        assert_eq!(v.forms_cell, "0/3");
        assert!(v.mastery_line.starts_with("Student: 0 of"), "{}", v.mastery_line);
    }

    /// A unit that stepped inside a building (#226, #229) names it in `inside`, in place of its
    /// tile; one standing anywhere else has `None` (the case above).
    #[test]
    fn unit_view_names_the_building_a_unit_is_inside() {
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
            (Some(Task::Healing(fn_sim::Pos::new(0, 0))), Activity::Heal),
            (Some(Task::Remedy(fn_sim::Pos::new(0, 0))), Activity::Heal),
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

    /// `climate_effects` lists the doc's own words, "cold, frost", in that order, and "mild"
    /// with neither.
    #[test]
    fn climate_effects_lists_cold_then_frost_or_mild_with_neither() {
        use fn_sim::seasons::ClimateDef;
        let band = |cold, frost| ClimateDef {
            id: String::new(),
            name: String::new(),
            max_temperature: 1.0,
            winter_growth: 1.0,
            cold,
            frost,
            outlook_points: 0,
        };
        assert_eq!(climate_effects(&band(true, true)), "cold, frost");
        assert_eq!(climate_effects(&band(true, false)), "cold");
        assert_eq!(climate_effects(&band(false, true)), "frost");
        assert_eq!(climate_effects(&band(false, false)), "mild");
    }

    /// With seasons off (no difficulty preset, as every test session starts), the Season
    /// section hides.
    #[test]
    fn season_view_is_none_with_seasons_off() {
        let s = Session::new(7, Nation::Earth, 48, 32);
        assert_eq!(season_view(&s, 4), None);
    }

    /// With Normal (seasons on), the heading names the season and the year, the left value
    /// counts down the season, and the food line warns in autumn or winter below the threshold
    /// but not in spring, however little is stored.
    #[test]
    fn season_view_names_the_season_and_warns_on_low_food_in_autumn_or_winter() {
        let mut s = Session::new(7, Nation::Earth, 48, 32);
        s.world.set_difficulty(Some("normal".to_owned()));
        let v = season_view(&s, 4).expect("Normal has seasons on");
        assert_eq!(v.heading, "Spring · Y1");
        assert!(v.left.ends_with(" left"), "{}", v.left);
        assert_eq!(food_in_stores(&s), 0, "nothing stockpiled yet");
        assert!(!v.food_warn, "spring never warns, however little is stored");
        assert!(!v.outlook.is_empty(), "{}", v.outlook);

        // Jump to autumn (index 2 of 4, the season right before winter): no food now warns.
        let season_ticks = (f64::from(s.defs.seasons.calendar.season_minutes)
            * 60.0
            * f64::from(TICKS_PER_SECOND)) as u64;
        s.world.tick = season_ticks * 2; // spring(0), summer(1), autumn(2)
        assert_eq!(s.world.season(&s.defs).id, "autumn");
        let v = season_view(&s, 4).expect("still Normal");
        assert!(v.food_warn, "no food in autumn should warn: {v:?}");
    }
}

//! The balance harness (#300, `docs/design/difficulty.md`, "Measuring it"): runs one
//! (preset, nation, seed) headlessly, with no input or with the standard opening
//! (`crate::autoplay::Opening`), and reports what the doc asks for. The `balance` binary drives
//! this over every seed, nation and preset a caller names; `crates/sim/tests/balance.rs` (#291's
//! soak split) drives it over one fixed case as a CI check.
//!
//! Everything here is a pure function of `RunConfig` and `Defs`: the same inputs always produce
//! the same `RunReport` (the harness's own determinism test, and `tests/balance.rs`'s), since
//! `World` draws no randomness but its own seeded streams and the opening reads only `World`.
//!
//! `RunConfig::bend_slots` switches on the mastery mode (#343, `docs/design/endgame.md`,
//! "Mastery pacing"): alongside the opening, `Opening::with_bending` keeps that many bends
//! designated near the anchor every in-game minute, so a settler's chi (almost) never sits idle.
//! `RunReport::top_bends` and `masters_minute` are what that mode adds to read off the pace:
//! the `balance` binary's own mastery sweep drives this over every nation and seed named, at
//! whatever `--minutes` checkpoints a caller wants (re-running fresh for each, since one run is
//! cheap, module doc above). `RunConfig::full_research` (#352) swaps `Opening`'s short
//! `RESEARCH_PLAN` for the settlement's whole visible tree; the mastery sweep can opt into it to
//! see whether Earth and Air cast more once their own branch (Bore Tunnel, Gliders) is reachable.
//!
//! `run_economy` (#352, `docs/design/materials.md` section 7) is a second entry point,
//! independent of `run`/`RunConfig`: it drives `autoplay::Economy` instead of `Opening`, and
//! reports the columns the doc asks for (stone, ore, wood, food per head, population, signs met)
//! once per season rather than once at the end, so a retune's effect on pacing shows up at every
//! point in a run, not just the last. It shares `run`'s determinism, for the same reason.

use std::collections::{BTreeMap, BTreeSet};

use crate::autoplay::{self, Economy, Opening};
use crate::building::BuildingId;
use crate::defs::Defs;
use crate::embark::Survey;
use crate::endeavor;
use crate::geom::Pos;
use crate::jobs::ItemId;
use crate::mapgen::MapGenConfig;
use crate::nation::Nation;
use crate::world::{
    EndeavorEventKind, FoodSource, SeasonEventKind, TICKS_PER_SECOND, Task, UnitId, World,
};

/// The settlement's starting headcount every run uses: the same six settlers the TUI starts a
/// new game with (`fn_ui::STARTING_UNITS`). `fn_sim` doesn't depend on `fn_ui`, so this is kept
/// in step by hand, as `World::found`'s own `debug_assert_eq!` against it already is.
pub const PEOPLE: u32 = 6;

/// The two 15-minute windows the walk-per-meal median is measured in
/// (`docs/design/difficulty.md`, "Measuring it"): early, before anything nearby has tired, and
/// after an hour and a half of picking.
const WALK_WINDOWS: [(f32, f32); 2] = [(15.0, 30.0), (90.0, 105.0)];

/// What one run of the harness varies.
#[derive(Debug, Clone, PartialEq)]
pub struct RunConfig {
    pub seed: u64,
    pub nation: Nation,
    /// A `data/difficulty.toml` preset id, or `None` for identity (today's rules, no seasons,
    /// strain or starting stores): `World::found`'s own `difficulty` meaning.
    pub preset: Option<String>,
    pub minutes: f32,
    /// Whether `autoplay::Opening` drives the settlement, instead of leaving it to forage, drink
    /// and sleep on its own.
    pub opening: bool,
    /// The mastery mode (module doc): bends the opening keeps designated near the anchor, so
    /// chi (almost) never sits idle. `None` leaves bending alone; ignored if `opening` is
    /// `false`, since there is then no `Opening` to drive it.
    pub bend_slots: Option<u32>,
    /// Research the settlement's whole visible tech tree instead of `Opening`'s short
    /// `RESEARCH_PLAN` (module doc; #352). Ignored if `opening` is `false`.
    pub full_research: bool,
    pub width: i32,
    pub height: i32,
}

/// What one run measured (`docs/design/difficulty.md`, "Measuring it"): one row of the
/// `balance` binary's output, and what `tests/balance.rs` asserts about.
#[derive(Debug, Clone, PartialEq)]
pub struct RunReport {
    /// The minute of the first death, if anyone died before `RunConfig::minutes` ran out.
    pub first_death_minute: Option<f32>,
    pub deaths: u32,
    /// Living settlers when the run ended.
    pub population: u32,
    /// Food per head on every winter's first tick the run reached, oldest first, as
    /// `(year, food per head)`. Empty under a preset with `seasons` off (Sandbox), or if the run
    /// never reaches a winter.
    pub winter_food_per_head: Vec<(u64, f32)>,
    /// The median path length (in tiles) a unit walked per meal, in the 15-minute window
    /// starting at minute 15, if any meal finished in it.
    pub walk_minute_15: Option<f32>,
    /// The same, for the window starting at minute 90.
    pub walk_minute_90: Option<f32>,
    /// The three highest `Unit::bends` among living settlers when the run ended, highest first,
    /// short if fewer than three settlers are alive (mastery mode, module doc).
    pub top_bends: Vec<u32>,
    /// The minute the `masters` sign (`data/endeavor.toml`) was first met, if ever within
    /// `cfg.minutes`: `World::endeavor_events`'s own `SignMet("masters")`, so the threshold and
    /// target come from the data, not a number this module hardcodes. `None` if it was never
    /// met, or the data has no such sign.
    pub masters_minute: Option<f32>,
}

/// Runs one `(preset, nation, seed)`: surveys `cfg`'s map, founds a world at the valid site
/// nearest the centre (`Survey::default_site`, the same choice embark's own "Exact" default
/// makes with no player input), with `cfg.preset`'s starting stores laid down
/// (`World::found`, #294) exactly as the embark flow would, and ticks it forward `cfg.minutes`,
/// driving `autoplay::Opening` at its fixed decision points when `cfg.opening` is set. Draws no
/// randomness beyond the world's own seeded streams, so the same `cfg` and `defs` always produce
/// an equal `RunReport`.
pub fn run(cfg: &RunConfig, defs: &Defs) -> RunReport {
    let map_cfg = MapGenConfig {
        seed: cfg.seed,
        width: cfg.width,
        height: cfg.height,
        ..MapGenConfig::default()
    };
    let survey = Survey::new(&map_cfg, defs);
    let start = survey.default_site(defs);
    let mut world = World::found(survey, cfg.nation, start, PEOPLE, cfg.preset.clone(), defs)
        .expect("Survey::default_site always names a site World::found accepts");
    let opening = cfg.opening.then(|| {
        let opening = match cfg.bend_slots {
            Some(slots) => Opening::with_bending(PEOPLE, slots),
            None => Opening::new(PEOPLE),
        };
        if cfg.full_research { opening.with_full_research() } else { opening }
    });

    let total_ticks = minutes_to_ticks(cfg.minutes);
    let mut walker = WalkTracker::default();
    let mut window_samples: [Vec<u32>; WALK_WINDOWS.len()] = Default::default();
    let mut winter_food_per_head = Vec::new();

    for tick in 0..total_ticks {
        if let Some(opening) = &opening
            && Opening::due(tick)
        {
            opening.step(&mut world, defs);
        }
        world.tick(defs);

        for ev in world.season_events().iter().rev().take_while(|e| e.tick == world.tick) {
            if let SeasonEventKind::Changed { season } = ev.kind
                && defs.seasons.seasons[season].winter
            {
                let population = world.units.len() as f32;
                let food = world.stored_food(defs) as f32;
                let per_head = if population > 0.0 { food / population } else { 0.0 };
                winter_food_per_head.push((world.year(defs), per_head));
            }
        }

        let minute = ticks_to_minutes(world.tick);
        for steps in walker.observe(&world) {
            for (window, &(start, end)) in window_samples.iter_mut().zip(WALK_WINDOWS.iter()) {
                if minute >= start && minute < end {
                    window.push(steps);
                }
            }
        }
    }

    let mut top_bends: Vec<u32> = world.units.iter().map(|u| u.bends()).collect();
    top_bends.sort_unstable_by(|a, b| b.cmp(a));
    top_bends.truncate(3);
    let masters_minute = world.endeavor_events().iter().find_map(|e| match &e.kind {
        EndeavorEventKind::SignMet(id) if id == "masters" => Some(ticks_to_minutes(e.tick)),
        _ => None,
    });

    RunReport {
        first_death_minute: world.deaths.first().map(|d| ticks_to_minutes(d.tick)),
        deaths: world.deaths.len() as u32,
        population: world.units.len() as u32,
        winter_food_per_head,
        walk_minute_15: median(window_samples[0].clone()),
        walk_minute_90: median(window_samples[1].clone()),
        top_bends,
        masters_minute,
    }
}

/// What one economy run varies (`run_economy`, module doc; #352). Independent of `RunConfig`:
/// `autoplay::Economy` plays differently enough from `Opening` (research plan, farms, quarrying)
/// that bolting it onto the same config would mean two unrelated sets of fields guarded by one
/// `opening: bool`.
#[derive(Debug, Clone, PartialEq)]
pub struct EconomyConfig {
    pub seed: u64,
    pub nation: Nation,
    /// A `data/difficulty.toml` preset id, or `None` for identity (`World::found`'s own
    /// `difficulty` meaning); `docs/design/materials.md`'s own targets assume `"normal"`.
    pub preset: Option<String>,
    pub minutes: f32,
    /// Till farm plots and raise an orchard (`Economy::farms`)? `false` is materials.md's
    /// `--no-farms` check (owner question 2).
    pub farms: bool,
    pub width: i32,
    pub height: i32,
}

/// One season's snapshot (`run_economy`, `docs/design/materials.md` section 7: "stone stored,
/// ore mined, food per head, wood stored, population, signs met, and the minute each sign was
/// first met").
#[derive(Debug, Clone, PartialEq)]
pub struct EconomySeason {
    pub year: u64,
    /// Index into `defs.seasons.seasons`; the caller names it (`SeasonDef::name`).
    pub season: usize,
    pub minute: f32,
    /// Summed over the "stone" sign's own items (`autoplay::sign_items`), so a retune that adds
    /// a second stone-like item needs no change here.
    pub stone_stored: u32,
    /// Lifetime mined, not currently stored: the "ore" sign's own kind
    /// (`SignKind::Tally(Tally::Harvested)`), summed the same generic way.
    pub ore_mined: u32,
    pub wood_stored: u32,
    pub food_per_head: f32,
    pub population: u32,
    /// Ids of every sign met by this point, in `data/endeavor.toml`'s order.
    pub signs_met: Vec<String>,
}

/// What `run_economy` reports (module doc; #352).
#[derive(Debug, Clone, PartialEq)]
pub struct EconomyReport {
    /// One entry at tick 0 (spring, year 1), plus one per further season boundary reached
    /// within `cfg.minutes`, oldest first: never empty, though with a preset that leaves
    /// `seasons` off (Sandbox) the tick-0 entry is the only one, since nothing marks a boundary
    /// without them (`World::tick_season`).
    pub seasons: Vec<EconomySeason>,
    /// Every sign in `data/endeavor.toml`'s order, with the minute it was first met within
    /// `cfg.minutes`, or `None` if it never was.
    pub sign_minutes: Vec<(String, Option<f32>)>,
    /// Every building the nation can reach (`endeavor::reachable_buildings`), in
    /// `data/buildings.toml`'s order, with the minute one first stood complete, or `None` if
    /// none ever did (#376: does the opening raise one of every building, the new ones too?).
    pub first_built: Vec<(String, Option<f32>)>,
    /// Every item some reachable building's recipe makes or uses (tools, brick, metal, wood,
    /// ...), plus the "ore" sign's items, with how many are in stores at the end of the run
    /// (#376: are the Workshop, forge and kiln kept running, and what ore is left for the
    /// gate?). In item order.
    pub recipe_stores: Vec<(String, u32)>,
}

/// Runs one `(preset, nation, seed)` with `autoplay::Economy` driving it (module doc), the same
/// way `run` does with `Opening`: `World::found` at `Survey::default_site`, ticked forward
/// `cfg.minutes`, reporting a season at a time. Draws no randomness beyond `World`'s own seeded
/// streams, so the same `cfg` and `defs` always produce an equal `EconomyReport`.
pub fn run_economy(cfg: &EconomyConfig, defs: &Defs) -> EconomyReport {
    let map_cfg = MapGenConfig {
        seed: cfg.seed,
        width: cfg.width,
        height: cfg.height,
        ..MapGenConfig::default()
    };
    let survey = Survey::new(&map_cfg, defs);
    let start = survey.default_site(defs);
    let mut world = World::found(survey, cfg.nation, start, PEOPLE, cfg.preset.clone(), defs)
        .expect("Survey::default_site always names a site World::found accepts");
    let economy = if cfg.farms { Economy::new(PEOPLE) } else { Economy::without_farms(PEOPLE) };

    let stone_items = autoplay::sign_items(defs, "stone");
    let ore_items = autoplay::sign_items(defs, "ore");
    let wood = defs.item_id("wood");

    let reachable: Vec<BuildingId> =
        endeavor::reachable_buildings(cfg.nation, &defs.techs, &defs.buildings).collect();
    let mut first_built: Vec<Option<f32>> = vec![None; reachable.len()];

    let total_ticks = minutes_to_ticks(cfg.minutes);
    // `World::tick_season` only ever logs a `Changed` event at a later boundary (`World::tick`
    // increments the tick before checking, so tick 0 itself never is one): spring, year 1 is
    // reported here instead, the same way `World::season_index` already treats a fresh world.
    let mut seasons = vec![economy_season(
        &world,
        defs,
        world.season_index(defs),
        &stone_items,
        &ore_items,
        wood,
    )];
    for tick in 0..total_ticks {
        if Opening::due(tick) {
            economy.step(&mut world, defs);
            if first_built.iter().any(Option::is_none) {
                let standing: BTreeSet<BuildingId> = world.buildings().map(|(_, b)| b).collect();
                for (slot, b) in first_built.iter_mut().zip(&reachable) {
                    if slot.is_none() && standing.contains(b) {
                        *slot = Some(ticks_to_minutes(world.tick));
                    }
                }
            }
        }
        world.tick(defs);

        for ev in world.season_events().iter().rev().take_while(|e| e.tick == world.tick) {
            if let SeasonEventKind::Changed { season } = ev.kind {
                seasons.push(economy_season(&world, defs, season, &stone_items, &ore_items, wood));
            }
        }
    }

    let sign_minutes = defs
        .endeavor
        .signs
        .iter()
        .map(|s| {
            let minute = world.endeavor_events().iter().find_map(|e| match &e.kind {
                EndeavorEventKind::SignMet(id) if *id == s.id => Some(ticks_to_minutes(e.tick)),
                _ => None,
            });
            (s.id.clone(), minute)
        })
        .collect();

    let first_built = reachable
        .iter()
        .zip(first_built)
        .map(|(&b, minute)| (defs.building(b).id.clone(), minute))
        .collect();
    let recipe_items: BTreeSet<ItemId> = reachable
        .iter()
        .filter_map(|&b| defs.building(b).recipe.as_ref())
        .flat_map(|r| r.inputs.iter().chain(std::iter::once(&r.output)).map(|c| c.item))
        .chain(ore_items.iter().copied())
        .collect();
    let recipe_stores = recipe_items
        .into_iter()
        .map(|item| (defs.item(item).id.clone(), world.stored_total(item)))
        .collect();

    EconomyReport { seasons, sign_minutes, first_built, recipe_stores }
}

/// One `EconomySeason` snapshot of `world` right now (`run_economy`).
fn economy_season(
    world: &World,
    defs: &Defs,
    season: usize,
    stone_items: &[ItemId],
    ore_items: &[ItemId],
    wood: Option<ItemId>,
) -> EconomySeason {
    let population = world.units.len() as u32;
    let signs_met: Vec<String> =
        world.endeavor_signs(defs).into_iter().filter(|p| p.met).map(|p| p.id.to_owned()).collect();
    EconomySeason {
        year: world.year(defs),
        season,
        minute: ticks_to_minutes(world.tick),
        stone_stored: stone_items.iter().map(|&i| world.stored_total(i)).sum(),
        ore_mined: ore_items.iter().map(|&i| world.tallies().harvested(i)).sum(),
        wood_stored: wood.map_or(0, |w| world.stored_total(w)),
        food_per_head: if population > 0 {
            world.stored_food(defs) as f32 / population as f32
        } else {
            0.0
        },
        population,
        signs_met,
    }
}

fn minutes_to_ticks(minutes: f32) -> u64 {
    (f64::from(minutes) * 60.0 * f64::from(TICKS_PER_SECOND)).round() as u64
}

fn ticks_to_minutes(tick: u64) -> f32 {
    tick as f32 / 60.0 / TICKS_PER_SECOND as f32
}

/// The middle value of `xs`, averaging the two middle values for an even count; `None` if empty.
fn median(mut xs: Vec<u32>) -> Option<f32> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_unstable();
    let n = xs.len();
    Some(if n % 2 == 1 { xs[n / 2] as f32 } else { (xs[n / 2 - 1] + xs[n / 2]) as f32 / 2.0 })
}

/// Watches every unit's `Task::Eat` each tick and reports the path length (steps taken while
/// still travelling to that meal) of every one that finishes, dies or is pre-empted this tick.
/// Reads only `World::units` and `Unit::task`/`pos`, both public, so no sim change was needed to
/// measure this: any frontend polling the same way would see what this sees.
#[derive(Debug, Clone, Default)]
struct WalkTracker {
    /// By unit: the meal it's walking to (its `FoodSource`), where it started, and how many
    /// tile-to-tile moves it has made since.
    active: BTreeMap<UnitId, (FoodSource, Pos, u32)>,
}

impl WalkTracker {
    /// Updates every living unit's walk and returns the step count of every meal that ended
    /// this tick (the unit's task is no longer `Eat` with the same source it started with, or it
    /// died).
    fn observe(&mut self, world: &World) -> Vec<u32> {
        let mut done = Vec::new();
        let mut seen = BTreeSet::new();
        for u in &world.units {
            seen.insert(u.id);
            match u.task() {
                Some(Task::Eat(source)) => {
                    let entry = self.active.entry(u.id).or_insert((source, u.pos, 0));
                    if entry.0 != source {
                        *entry = (source, u.pos, 0);
                    } else if entry.1 != u.pos {
                        entry.1 = u.pos;
                        entry.2 += 1;
                    }
                }
                _ => {
                    if let Some((_, _, steps)) = self.active.remove(&u.id) {
                        done.push(steps);
                    }
                }
            }
        }
        let finished: Vec<UnitId> =
            self.active.keys().copied().filter(|id| !seen.contains(id)).collect();
        for id in finished {
            if let Some((_, _, steps)) = self.active.remove(&id) {
                done.push(steps);
            }
        }
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::Defs;

    fn cfg(preset: &str, nation: Nation, opening: bool, minutes: f32) -> RunConfig {
        let defs = Defs::bundled();
        let size = defs.embark.map_size("small").unwrap();
        RunConfig {
            seed: 7,
            nation,
            preset: Some(preset.to_owned()),
            minutes,
            opening,
            bend_slots: None,
            full_research: false,
            width: size.width,
            height: size.height,
        }
    }

    fn bending_cfg(nation: Nation, minutes: f32, bend_slots: u32) -> RunConfig {
        RunConfig { bend_slots: Some(bend_slots), ..cfg("normal", nation, true, minutes) }
    }

    fn economy_cfg(nation: Nation, minutes: f32, farms: bool) -> EconomyConfig {
        let defs = Defs::bundled();
        let size = defs.embark.map_size("small").expect("bundled data always has a small map size");
        EconomyConfig {
            seed: 7,
            nation,
            preset: Some("normal".to_owned()),
            minutes,
            farms,
            width: size.width,
            height: size.height,
        }
    }

    #[test]
    fn two_runs_of_the_same_config_are_identical() {
        let defs = Defs::bundled();
        let c = cfg("normal", Nation::Earth, false, 20.0);
        assert_eq!(run(&c, &defs), run(&c, &defs));
    }

    #[test]
    fn opening_is_also_deterministic() {
        let defs = Defs::bundled();
        let c = cfg("hard", Nation::Water, true, 20.0);
        assert_eq!(run(&c, &defs), run(&c, &defs));
    }

    #[test]
    fn sandbox_has_no_winter_food_readings() {
        let defs = Defs::bundled();
        let c = cfg("sandbox", Nation::Air, false, 90.0);
        let report = run(&c, &defs);
        assert!(report.winter_food_per_head.is_empty());
    }

    #[test]
    fn normal_reaches_its_first_winter_within_90_minutes() {
        let defs = Defs::bundled();
        let c = cfg("normal", Nation::Earth, true, 90.0);
        let report = run(&c, &defs);
        assert!(!report.winter_food_per_head.is_empty());
    }

    /// The mastery mode (#343): two runs of the same bending config give the same report, as
    /// the module doc promises.
    #[test]
    fn bending_is_also_deterministic() {
        let defs = Defs::bundled();
        let c = bending_cfg(Nation::Fire, 20.0, 6);
        assert_eq!(run(&c, &defs), run(&c, &defs));
    }

    /// Fire's Hearthflame needs no tech and never runs out of valid ground to kindle, so bends
    /// pile up steadily (`docs/design/endgame.md`, "Mastery pacing").
    #[test]
    fn bending_designates_hearthflame_for_fire() {
        let defs = Defs::bundled();
        let c = bending_cfg(Nation::Fire, 30.0, 6);
        let report = run(&c, &defs);
        assert!(report.top_bends.first().copied().unwrap_or(0) > 0, "{report:?}");
    }

    /// Water's Water the Land needs no tech either, and wetting a patch never consumes the
    /// ground beside the river, so it never runs dry the way Earth's Raise Earth can.
    #[test]
    fn bending_designates_water_the_land_for_water() {
        let defs = Defs::bundled();
        let c = bending_cfg(Nation::Water, 30.0, 6);
        let report = run(&c, &defs);
        assert!(report.top_bends.first().copied().unwrap_or(0) > 0, "{report:?}");
    }

    /// `top_bends` is at most 3 entries and never more than the living population.
    #[test]
    fn top_bends_is_at_most_three() {
        let defs = Defs::bundled();
        let c = bending_cfg(Nation::Earth, 30.0, PEOPLE);
        let report = run(&c, &defs);
        assert!(report.top_bends.len() <= 3, "{report:?}");
    }

    /// With no bending mode, the opening never designates a bend, so `masters_minute` is never
    /// met and `top_bends` stays at zero: the pre-#343 behaviour is unchanged.
    #[test]
    fn with_no_bend_slots_nobody_bends() {
        let defs = Defs::bundled();
        let c = cfg("normal", Nation::Fire, true, 30.0);
        let report = run(&c, &defs);
        assert_eq!(report.top_bends, vec![0, 0, 0]);
        assert_eq!(report.masters_minute, None);
    }

    /// `run_economy` is as deterministic as `run` (module doc; #352): `Economy` reads only
    /// `World`, so two identical runs issue the same commands and end at the same state.
    #[test]
    fn economy_report_is_deterministic() {
        let defs = Defs::bundled();
        let c = economy_cfg(Nation::Earth, 60.0, true);
        assert_eq!(run_economy(&c, &defs), run_economy(&c, &defs));
    }

    /// Normal turns seasons on, so the very first tick is already a season boundary (spring,
    /// year 1): `run_economy` reports it, not just later ones.
    #[test]
    fn economy_report_starts_with_spring_of_year_one() {
        let defs = Defs::bundled();
        let c = economy_cfg(Nation::Water, 20.0, true);
        let report = run_economy(&c, &defs);
        let first = report.seasons.first().expect("at least one season within 20 minutes");
        assert_eq!((first.year, first.season, first.minute), (1, 0, 0.0));
    }

    /// `sign_minutes` always lists every sign in `data/endeavor.toml`, met or not, so a caller
    /// can print "none" for one that never was rather than it silently being absent.
    #[test]
    fn economy_report_lists_every_sign() {
        let defs = Defs::bundled();
        let c = economy_cfg(Nation::Fire, 20.0, true);
        let report = run_economy(&c, &defs);
        assert_eq!(report.sign_minutes.len(), defs.endeavor.signs.len());
    }

    /// #376: `first_built` names every building the nation can reach, in data order, so the
    /// binary can print "none" for one the opening never raised.
    #[test]
    fn economy_report_lists_every_reachable_building() {
        let defs = Defs::bundled();
        let c = economy_cfg(Nation::Earth, 20.0, true);
        let report = run_economy(&c, &defs);
        let reachable: Vec<String> =
            endeavor::reachable_buildings(Nation::Earth, &defs.techs, &defs.buildings)
                .map(|b| defs.building(b).id.clone())
                .collect();
        let listed: Vec<String> = report.first_built.iter().map(|(id, _)| id.clone()).collect();
        assert_eq!(listed, reachable);
        assert!(listed.contains(&"deep_mine".to_owned()));
    }

    /// `--no-farms` (`Economy::without_farms`) never sows a farm plot, so the food sign's path
    /// there is wild food alone: a `food_per_head` reading never above what `--no-farms`
    /// produces is the shape materials.md's owner question 2 wants, though the numbers
    /// themselves are a data question, not this harness's.
    #[test]
    fn economy_report_runs_with_no_farms() {
        let defs = Defs::bundled();
        let c = economy_cfg(Nation::Water, 20.0, false);
        let report = run_economy(&c, &defs);
        assert!(!report.seasons.is_empty());
    }
}

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

mod autopsy;
mod probe;
mod record;
mod site;

pub use autopsy::Autopsy;
pub use probe::{Detectors, Flag, FlagClass, Occupation, Probe, Raised, Sample, Watch};
pub use record::{CareSummary, Minute, RunRecord, RunSpec, run as run_record, run_observed};
pub use site::{BORE_LIMIT, OPEN_RADIUS, SiteProfile};

use crate::autoplay::{Economy, Opening, Player};
use crate::defs::Defs;
use crate::embark::Survey;
use crate::geom::Pos;
use crate::jobs::ItemId;
use crate::mapgen::MapGenConfig;
use crate::nation::Nation;
use crate::score::{ScoreDef, ScoreStart, Scorecard};
use crate::world::{
    EndeavorEventKind, FoodSource, SeasonEventKind, TICKS_PER_SECOND, Task, UnitId, World,
    minutes_to_ticks,
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
    /// Research the nation's own branch before the shared tier (`Economy::research`,
    /// `docs/design/pacing.md`'s own `--branch-first`, #434): `false` is the default, unchanged
    /// order.
    pub branch_first: bool,
    pub width: i32,
    pub height: i32,
}

/// One season's snapshot (`run_economy`, `docs/design/materials.md` section 7: "stone stored,
/// ore mined, food per head, wood stored, population, signs met, and the minute each sign was
/// first met").
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// One new thing to do, in minute order (`decision_events`, `docs/design/pacing.md`'s own
/// decision timeline, #434): a tech completion that unlocks a building, ability or job (read
/// from the tech's own `unlocks`, never a hard-coded list of techs), or a sign met for the first
/// time. `label` names what happened (the tech's name, or `"<sign name> met"`), for printing;
/// nothing here is keyed by id, since the timeline is read-only output, not state a caller looks
/// things up in.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DecisionEvent {
    pub minute: f32,
    pub label: String,
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
    /// Every tech completed within `cfg.minutes`, in completion order (`Research::completions`):
    /// `(tech id, completion minute)` (`docs/design/pacing.md`, "Measuring it"; #434).
    pub techs: Vec<(String, f32)>,
    /// Every decision event (`DecisionEvent`) in minute order, ties broken by `label`: each
    /// tech completion from `techs` whose `unlocks` names a building, ability or job, plus each
    /// sign from `sign_minutes` the first minute it was met (`docs/design/pacing.md`, "Measuring
    /// it"; #434).
    pub timeline: Vec<DecisionEvent>,
    /// The start and end minute of the longest stretch with no decision event, from minute 30
    /// to `ready_minute` (the latest minute every sign but `masters` and `scrolls` was met, or
    /// `cfg.minutes` if one of them never was): `None` if `ready_minute` is 30 or earlier, so
    /// there's no window to measure (`docs/design/pacing.md`, "Measuring it"; #434).
    pub longest_gap: Option<(f32, f32)>,
    /// How many of those same gaps are longer than 15 minutes (`docs/design/pacing.md`'s own
    /// pacing target; #434).
    pub gaps_over_15: u32,
    /// The share of living units with no task (`Unit::task()` is `None`), sampled on every
    /// `Opening::due` tick, per 15-minute window: `(window start minute, share)`, in order,
    /// covering every window `cfg.minutes` reaches (`docs/design/pacing.md`, "Stores and idle
    /// hands"; #434).
    pub idle_share: Vec<(f32, f32)>,
    /// The minute of the first death, if anyone died before `cfg.minutes` ran out
    /// (`docs/design/materials.md` section 4, owner question 2's `--no-farms` check).
    pub first_death_minute: Option<f32>,
    pub deaths: u32,
}

/// Runs one `(preset, nation, seed)` with `autoplay::Economy` driving it (module doc), the same
/// way `run` does with `Opening`: `World::found` at `Survey::default_site`, ticked forward
/// `cfg.minutes`, reporting a season at a time. Draws no randomness beyond `World`'s own seeded
/// streams, so the same `cfg` and `defs` always produce an equal `EconomyReport`.
pub fn run_economy(cfg: &EconomyConfig, defs: &Defs) -> EconomyReport {
    run_economy_world(cfg, defs).0
}

/// `run_economy`, scored (#526): the same run, plus the `Scorecard` of the world as it stands at
/// the end of `cfg.minutes`, with that end as the horizon (`score` module doc, "Compute at the
/// horizon"). The care tally comes from `World::care()`, so a run that kills or starves settlers
/// scores for it.
pub fn run_economy_scored(
    cfg: &EconomyConfig,
    defs: &Defs,
    score: &ScoreDef,
) -> (EconomyReport, Scorecard) {
    let (report, world) = run_economy_world(cfg, defs);
    let card = Scorecard::compute(
        &world,
        defs,
        score,
        &ScoreStart::default(),
        minutes_to_ticks(cfg.minutes),
    );
    (report, card)
}

fn run_economy_world(cfg: &EconomyConfig, defs: &Defs) -> (EconomyReport, World) {
    let economy = if cfg.farms { Economy::new(PEOPLE) } else { Economy::without_farms(PEOPLE) };
    let economy = if cfg.branch_first { economy.with_branch_first() } else { economy };
    let spec = RunSpec {
        seed: cfg.seed,
        nation: cfg.nation,
        preset: cfg.preset.clone(),
        minutes: cfg.minutes,
        width: cfg.width,
        height: cfg.height,
        player: Player::Economy(economy),
        detectors: Detectors::default(),
        step_offset: 0,
    };
    let (record, world) = record::run_world(&spec, defs);
    (record.economy_report(), world)
}

/// How wide an `idle_share` window is, in minutes (`EconomyReport::idle_share`'s own module doc;
/// #434).
const IDLE_WINDOW_MINUTES: f32 = 15.0;

/// Tallies one `Opening::due` sample of every living unit's task into the window `world.tick`
/// falls in, growing `windows` to cover it if this is the first sample to reach that far
/// (`EconomyReport::idle_share`).
fn sample_idle_share(world: &World, windows: &mut Vec<(u32, u32)>) {
    let minute = ticks_to_minutes(world.tick);
    let window = (minute / IDLE_WINDOW_MINUTES) as usize;
    if windows.len() <= window {
        windows.resize(window + 1, (0, 0));
    }
    let idle = world.units.iter().filter(|u| u.task().is_none()).count() as u32;
    windows[window].0 += idle;
    windows[window].1 += world.units.len() as u32;
}

/// The minute every sign but `masters` and `scrolls` was first met: the latest of their
/// `sign_minutes`, or `total_minutes` (the run's own end) if any of them never was
/// (`docs/design/pacing.md`, "Measuring it": "Ready, taken as every sign except masters and
/// scrolls", #434).
fn ready_minute(sign_minutes: &[(String, Option<f32>)], total_minutes: f32) -> f32 {
    let mut ready = 0.0_f32;
    for (id, minute) in sign_minutes {
        if id == "masters" || id == "scrolls" {
            continue;
        }
        match minute {
            Some(m) => ready = ready.max(*m),
            None => return total_minutes,
        }
    }
    ready
}

/// Every decision event (`DecisionEvent`'s own module doc) in minute order, ties broken by
/// label: each tech completion whose `unlocks` names a building, ability or job (read from the
/// tech's own data, never a hard-coded list), plus each met sign from `sign_minutes`
/// (`docs/design/pacing.md`, "Measuring it"; #434).
fn decision_events(
    world: &World,
    defs: &Defs,
    sign_minutes: &[(String, Option<f32>)],
) -> Vec<DecisionEvent> {
    let mut events: Vec<DecisionEvent> = world
        .research()
        .completions()
        .iter()
        .filter_map(|c| {
            let tech = defs.tech(c.tech);
            let unlocks_something = !tech.unlocks.buildings.is_empty()
                || !tech.unlocks.jobs.is_empty()
                || !tech.unlocks.abilities.is_empty();
            unlocks_something.then(|| DecisionEvent {
                minute: ticks_to_minutes(c.tick),
                label: tech.name.clone(),
            })
        })
        .collect();
    for (id, minute) in sign_minutes {
        let Some(minute) = minute else { continue };
        let name = defs
            .endeavor
            .signs
            .iter()
            .find(|s| &s.id == id)
            .map_or(id.as_str(), |s| s.name.as_str());
        events.push(DecisionEvent { minute: *minute, label: format!("{name} met") });
    }
    events.sort_by(|a, b| a.minute.total_cmp(&b.minute).then_with(|| a.label.cmp(&b.label)));
    events
}

/// The gaps between `timeline`'s events, from minute 30 to `ready`, with virtual boundary
/// events at 30 and `ready` themselves so a stretch with no decision right after minute 30, or
/// right before `ready`, counts as a gap too (`docs/design/pacing.md`, "Measuring it": "no gap
/// longer than 15 minutes ... from minute 30 to Ready", #434). Empty if `ready <= 30.0`, so
/// there's no window to measure. `timeline` is assumed sorted by minute (`decision_events`'s own
/// contract).
fn decision_gaps(timeline: &[DecisionEvent], ready: f32) -> Vec<(f32, f32)> {
    if ready <= 30.0 {
        return Vec::new();
    }
    let mut bounds: Vec<f32> = std::iter::once(30.0)
        .chain(timeline.iter().map(|e| e.minute).filter(|&m| m > 30.0 && m < ready))
        .chain(std::iter::once(ready))
        .collect();
    bounds.dedup();
    bounds.windows(2).map(|w| (w[0], w[1])).collect()
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

/// Which `autoplay` opening a [`ParPrelude`] runs: the same two a scenario's prelude can name
/// (`fn_tui::surface::PreludeOpening`, which `fn_sim` can't depend on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParOpening {
    /// `autoplay::Economy`.
    Economy,
    /// `autoplay::Economy` researching branch-first (`Economy::with_branch_first`).
    EconomyBranchFirst,
    /// The plainer `autoplay::Opening`.
    Opening,
}

/// A scenario prelude (`docs/design/agent-scripting.md`, "Scenario starts"): `opening`, run for
/// `minutes` game minutes before turn 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParPrelude {
    pub opening: ParOpening,
    pub minutes: f32,
}

/// A track's start and horizon (`par`; `docs/design/agent-hillclimb.md`, "Tracks: what is
/// comparable"): the fields of a run log's header that decide the start, plus how long to score.
#[derive(Debug, Clone, PartialEq)]
pub struct ParTrack {
    pub seed: u64,
    pub nation: Nation,
    /// A `data/difficulty.toml` preset id, or `None` for identity.
    pub preset: Option<String>,
    /// A `data/embark.toml` map size id (`None` is `"small"`). Read only with a `site`, as in
    /// the TUI: with no site the game starts on the default-size map, at the centre.
    pub map_size: Option<String>,
    /// The chosen settlement site, or `None` for the default start.
    pub site: Option<(i32, i32)>,
    pub prelude: Option<ParPrelude>,
    /// Turns after the prelude to score at.
    pub horizon_turns: u64,
    /// Game seconds per turn (a run log header's `turn_seconds`; 20 by default).
    pub turn_seconds: u32,
}

/// The par for `track` (#526, `docs/design/agent-hillclimb.md`, "Tracks: what is comparable"):
/// what `autoplay::Economy` scores from the same start at the same horizon, so a run's score
/// reads against a yardstick. Par is privileged (the opening reads `World`), context only, and
/// selection never uses it.
///
/// The start mirrors the TUI's (`fn_tui::shell::Shell::for_args`): `World::found` on the
/// `map_size` map at `site` if one is named, else `World::new` on the default map. A prelude
/// runs first for its own opening and minutes; `Economy` plays from there to the horizon.
/// `horizon_turns` count from turn 0, after the prelude, but the scorecard's ticks are the
/// world's own, so the horizon is `prelude ticks + horizon_turns × turn ticks`, and a milestone
/// met in the prelude carries its absolute tick. The scorecard is told where turn 0 is
/// ([`ScoreStart`], #548): a milestone met in the prelude earns full credit, the late-credit
/// slope runs from the end of the prelude, and care counts from there.
///
/// Errors on an unknown preset or map size, or a site `World::found` refuses. Deterministic:
/// the same `track` and `defs` always give an equal `Scorecard`.
pub fn par(track: &ParTrack, defs: &Defs, score: &ScoreDef) -> Result<Scorecard, String> {
    let mut world = par_start(track, defs)?;
    let prelude_ticks = track.prelude.map_or(0, |p| minutes_to_ticks(p.minutes));
    let turn_ticks = u64::from(track.turn_seconds.max(1)) * u64::from(TICKS_PER_SECOND);
    let horizon = prelude_ticks + track.horizon_turns * turn_ticks;
    let economy = Economy::new(PEOPLE);
    let branch_first = Economy::new(PEOPLE).with_branch_first();
    let plain = Opening::new(PEOPLE);
    let mut start = ScoreStart::default();
    for tick in 0..horizon {
        if tick == prelude_ticks && tick > 0 {
            start = ScoreStart::of(&world);
        }
        if Opening::due(tick) {
            match track.prelude {
                Some(ParPrelude { opening: ParOpening::Opening, .. }) if tick < prelude_ticks => {
                    plain.step(&mut world, defs)
                }
                Some(ParPrelude { opening: ParOpening::EconomyBranchFirst, .. })
                    if tick < prelude_ticks =>
                {
                    branch_first.step(&mut world, defs)
                }
                _ => economy.step(&mut world, defs),
            }
        }
        world.tick(defs);
    }
    if horizon <= prelude_ticks && prelude_ticks > 0 {
        // No turns after the prelude: turn 0 is the horizon.
        start = ScoreStart::of(&world);
    }
    Ok(Scorecard::compute(&world, defs, score, &start, horizon))
}

/// The world a [`ParTrack`] starts from, before any opening plays: the world a player or an
/// agent begins on for the same start fields. With no `site` that is `World::new` on the default
/// map (what plain `fn_tui SEED NATION` and `fn_agent play` without `--start` build, with the
/// centre-spiral spawn #582 made keep to one connected region); with a `site` it is
/// `World::found` on the `map_size` survey. The balance harness's own runs start from
/// `World::found` at `Survey::default_site` instead, so a par that has to sit beside an agent's
/// score is made from this and not from `run_economy`.
///
/// Errors on an unknown preset or map size, or a site `World::found` refuses.
pub fn par_start(track: &ParTrack, defs: &Defs) -> Result<World, String> {
    if let Some(id) = &track.preset
        && defs.difficulty.preset(id).is_none()
    {
        return Err(format!("unknown difficulty preset {id:?}"));
    }
    match track.site {
        None => {
            let cfg = MapGenConfig { seed: track.seed, ..MapGenConfig::default() };
            let mut world = World::new(&cfg, track.nation, PEOPLE, defs);
            world.set_difficulty(track.preset.clone());
            Ok(world)
        }
        Some((x, y)) => {
            let id = track.map_size.as_deref().unwrap_or("small");
            let size =
                defs.embark.map_size(id).ok_or_else(|| format!("unknown map size {id:?}"))?;
            let survey = Survey::new(&Survey::config(track.seed, size), defs);
            World::found(survey, track.nation, Pos::new(x, y), PEOPLE, track.preset.clone(), defs)
                .map_err(|p| format!("can't start at ({x}, {y}): {p:?}"))
        }
    }
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
            branch_first: false,
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
            crate::endeavor::reachable_buildings(Nation::Earth, &defs.techs, &defs.buildings)
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

    /// One 120-minute economy run checks the report's shape (these were four tests with a
    /// run each, ~45 s of CI between them, for checks no nation changes):
    /// - #434's own acceptance criterion: `timeline` is in minute order, and every tech event in
    ///   it names a tech with a visible unlock (a building, ability or job), never one that only
    ///   changes a modifier (`decision_events`'s own rule).
    /// - `techs` lists every completion `timeline` could possibly draw a tech event from, so a
    ///   caller never sees a tech in `timeline` that isn't also in `techs`.
    /// - `idle_share` samples every `Opening::due` tick (module doc; #434), so the run covers at
    ///   least the first 15-minute window, and every share is a fraction.
    /// - `longest_gap`, when present, is a well-formed interval inside the measured window
    ///   (minute 30 to `ready_minute`, which is at most `cfg.minutes`): its start comes no
    ///   earlier than minute 30 and its end no later than the run's own length.
    #[test]
    fn the_economy_report_is_well_formed() {
        let defs = Defs::bundled();
        let c = economy_cfg(Nation::Earth, 120.0, true);
        let report = run_economy(&c, &defs);

        // timeline: minute order, and tech events name techs with a visible unlock.
        assert!(!report.timeline.is_empty(), "{report:?}");
        assert!(
            report.timeline.windows(2).all(|w| w[0].minute <= w[1].minute),
            "expected the timeline in minute order: {:?}",
            report.timeline
        );
        let tech_names: std::collections::BTreeSet<&str> =
            defs.techs.iter().map(|t| t.name.as_str()).collect();
        let visible: std::collections::BTreeSet<&str> = defs
            .techs
            .iter()
            .filter(|t| {
                !t.unlocks.buildings.is_empty()
                    || !t.unlocks.jobs.is_empty()
                    || !t.unlocks.abilities.is_empty()
            })
            .map(|t| t.name.as_str())
            .collect();
        for event in &report.timeline {
            // A sign event's label ends in " met" and is never a tech's own name (no tech is
            // named that way in `data/research.toml`).
            if tech_names.contains(event.label.as_str()) {
                assert!(
                    visible.contains(event.label.as_str()),
                    "tech event {event:?} names a tech with no visible unlock"
                );
            }
        }

        // Every tech event in the timeline is also in techs.
        let completed: std::collections::BTreeSet<&str> = report
            .techs
            .iter()
            .map(|(id, _)| defs.tech(defs.tech_id(id).unwrap()).name.as_str())
            .collect();
        for event in &report.timeline {
            if !event.label.ends_with(" met") {
                assert!(
                    completed.contains(event.label.as_str()),
                    "timeline tech event {event:?} has no matching completion in techs"
                );
            }
        }

        // idle_share: at least one window, every share a fraction.
        assert!(!report.idle_share.is_empty(), "{report:?}");
        for &(start, share) in &report.idle_share {
            assert!(start >= 0.0, "{report:?}");
            assert!((0.0..=1.0).contains(&share), "{report:?}");
        }

        // longest_gap: inside the measured window.
        if let Some((start, end)) = report.longest_gap {
            assert!(start >= 30.0 && end <= 120.0 && start <= end, "{report:?}");
        }
        assert!(report.gaps_over_15 as usize <= report.timeline.len() + 1, "{report:?}");
    }

    /// `--branch-first` (`EconomyConfig::branch_first`) is as deterministic as the default order
    /// (module doc; #434), for the same reason `run_economy` already is.
    #[test]
    fn branch_first_is_also_deterministic() {
        let defs = Defs::bundled();
        let c = EconomyConfig { branch_first: true, ..economy_cfg(Nation::Earth, 60.0, true) };
        assert_eq!(run_economy(&c, &defs), run_economy(&c, &defs));
    }

    /// Seed 7, Earth, Normal, the small map's default site, 90 turns (30 game minutes).
    fn par_track() -> (ParTrack, Defs) {
        let defs = Defs::bundled();
        let size = defs.embark.map_size("small").unwrap();
        let survey = Survey::new(&Survey::config(7, size), &defs);
        let site = survey.default_site(&defs);
        let track = ParTrack {
            seed: 7,
            nation: Nation::Earth,
            preset: Some("normal".to_owned()),
            map_size: Some("small".to_owned()),
            site: Some((site.x, site.y)),
            prelude: None,
            horizon_turns: 90,
            turn_seconds: 20,
        };
        (track, defs)
    }

    /// Par for one small fixed case is pinned, so a scoring or opening change shows up as a
    /// diff here (#526). Re-pin the numbers when one is meant: they print on failure.
    #[test]
    fn par_for_a_small_fixed_case_is_pinned() {
        let (track, defs) = par_track();
        let card = par(&track, &defs, &ScoreDef::bundled()).unwrap();
        assert!(!card.past_horizon);
        assert_eq!(card.horizon_ticks, 90 * 200);
        let got = (
            (card.score * 1000.0).round() / 1000.0,
            (card.progress * 1e4).round() / 1e4,
            card.care.deaths,
            card.care.close_calls,
        );
        assert_eq!(got, (165.375, 0.1654, 0, 0), "{card:#?}");
    }

    /// `par` is the same run `run_economy_scored` makes for the same start and length.
    #[test]
    fn par_matches_the_economy_run_scored_at_the_same_horizon() {
        let (track, defs) = par_track();
        let size = defs.embark.map_size("small").unwrap();
        let cfg = EconomyConfig {
            seed: 7,
            nation: Nation::Earth,
            preset: Some("normal".to_owned()),
            minutes: 30.0,
            farms: true,
            branch_first: false,
            width: size.width,
            height: size.height,
        };
        let score = ScoreDef::bundled();
        let (_, scored) = run_economy_scored(&cfg, &defs, &score);
        assert_eq!(par(&track, &defs, &score).unwrap(), scored);
    }

    /// A prelude moves turn 0 to its end (#548): the card knows where it is, and a milestone met
    /// during the prelude is credited in full, the same as one met at turn 0.
    #[test]
    fn par_on_a_prelude_track_starts_at_the_end_of_the_prelude() {
        let (track, defs) = par_track();
        let score = ScoreDef::bundled();
        let prelude = ParPrelude { opening: ParOpening::Economy, minutes: 10.0 };
        let track = ParTrack { prelude: Some(prelude), horizon_turns: 60, ..track };
        let card = par(&track, &defs, &score).unwrap();
        let prelude_ticks = minutes_to_ticks(10.0);
        assert_eq!(card.start_tick, prelude_ticks);
        assert_eq!(card.horizon_ticks, prelude_ticks + 60 * 200);
        let early: Vec<_> = card
            .milestones
            .iter()
            .filter(|r| r.first_met.is_some_and(|t| t <= prelude_ticks))
            .collect();
        assert!(!early.is_empty(), "something is met within the first ten minutes: {card:#?}");
        for r in early {
            assert!((r.credit - 1.0).abs() < 1e-9, "{} met in the prelude: {r:?}", r.id);
        }
        // No prelude: turn 0 is tick 0.
        let (plain, _) = par_track();
        assert_eq!(par(&plain, &defs, &score).unwrap().start_tick, 0);
    }

    #[test]
    fn par_refuses_an_unknown_preset_or_map_size() {
        let (track, defs) = par_track();
        let score = ScoreDef::bundled();
        let bad = ParTrack { preset: Some("nope".to_owned()), ..track.clone() };
        assert!(par(&bad, &defs, &score).unwrap_err().contains("preset"));
        let bad = ParTrack { map_size: Some("nope".to_owned()), ..track };
        assert!(par(&bad, &defs, &score).unwrap_err().contains("map size"));
    }
}

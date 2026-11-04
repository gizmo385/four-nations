//! The live game state and its fixed-step update.
//!
//! The sim advances in discrete ticks (`World::tick`). Clients render whatever state the world is
//! in and may interpolate between ticks, but must never mutate it except through commands.

use std::collections::{BTreeMap, BTreeSet};

use rand::RngExt;
use rand::seq::SliceRandom;

use crate::bending::AbilityId;
use crate::building::BuildingDef;
use crate::defs::{Defs, ResourceId};
use crate::geom::Pos;
use crate::jobs::{Job, JobId, Stack};
use crate::mapgen::{self, MapGenConfig, TileMap};
use crate::nation::{Modifiers, Nation};
use crate::needs::{DeathCause, Needs};
use crate::path::MoveRules;
use crate::regions::Regions;
use crate::research::{self, Research, ResearchError, TechId};
use crate::rng::{self, SimRng};

mod bending;
mod build;
mod command;
mod endeavor;
mod found;
mod gate;
mod heat;
mod inside;
mod orchard;
mod population;
mod stride;
mod study;
mod tally;
mod work;

use bending::Patch;
pub use bending::{BendChannel, BendError, BendEvent, BendOutcome, Moved};
use build::Site;
pub use build::{Blocker, BuildError};
pub use command::{Command, CommandError, Recorded};
use endeavor::EndeavorLog;
pub use endeavor::{EndeavorEvent, EndeavorEventKind, EndeavorState, SignProgress};
pub use gate::{EndeavorError, GateSite, GateTask};
pub use heat::{Aura, Flame, Glow, HeatSource};
pub use population::Arrival;
pub use stride::{STRIDE_LOOKAHEAD, Stride};
pub use tally::Tallies;
pub use work::{DesignateError, FoodSource, Task};

/// Ticks per in-game second. Movement and needs are expressed per tick.
pub const TICKS_PER_SECOND: u32 = 10;

/// Ticks a baseline (move_speed 1.0) unit waits between steps.
const BASE_TICKS_PER_STEP: f32 = 6.0;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct UnitId(pub u32);

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Unit {
    pub id: UnitId,
    pub name: String,
    /// This unit's own nation, if it differs from the settlement's (`World::nation`): after the
    /// Harmony Gate, settlers of other nations live in one settlement
    /// (`docs/design/endgame.md`, "The transformation"). `None` (the default for every unit
    /// spawned today) means the world's nation; read it through `World::nation_of`, never this
    /// field directly, so a save from before #239 (whose units were always home-nation) keeps
    /// meaning the same thing. A bare nation id from such a save (no `Some`/`None` wrapper)
    /// still deserializes, as `Some` of it.
    #[serde(default, deserialize_with = "deserialize_unit_nation")]
    pub nation: Option<Nation>,
    pub pos: Pos,
    /// Which of the unit's own nation's sprite variants to draw. Purely cosmetic.
    pub variant: u8,
    /// Hunger, thirst, energy and health (data/needs.toml). At 0 health the unit dies.
    pub needs: Needs,
    /// Bending energy, from 0 to `data/bending.toml`'s `chi.max`. Regenerates every tick (faster
    /// asleep) and is spent when a bend completes. Never hurts the unit.
    pub chi: f32,
    /// What this unit is carrying to a stockpile (or has picked up to eat).
    pub carrying: Option<Stack>,
    /// A need it is acting on (eat, drink, sleep). Takes priority over `job_task`.
    need_task: Option<Task>,
    /// The job it has claimed. Set aside, not dropped, while a need is being met.
    job_task: Option<Task>,
    /// Steps left to the current task's destination, the next step last. `None` means the
    /// route needs planning (a new claim has just resumed, say).
    route: Option<Vec<Pos>>,
    /// Seconds into the current need task's work (eating, drinking).
    work: f32,
    /// The bed this unit is sleeping in or walking to, while its need task is `Task::Sleep`.
    /// Other units treat it as taken (`work.rs`, issue #58).
    #[serde(default)]
    bed: Option<Pos>,
    /// Tick before which a failed attempt to act on a need isn't repeated (hunger, thirst,
    /// energy).
    need_retry_at: [u64; 3],
    /// Tick before which an idle unit doesn't look for a job again.
    claim_retry_at: u64,
    /// Progress toward the next step; a step happens when this reaches `BASE_TICKS_PER_STEP`.
    move_progress: f32,
    /// Tick before which an idle unit stands still instead of deciding whether to wander
    /// (`data/jobs.toml`, `idle_pause_seconds`; issue #110).
    #[serde(default)]
    idle_until: u64,
    /// Steps left in the idle wander in progress; 0 when standing.
    #[serde(default)]
    wander_left: u8,
    /// Bends this unit has finished (failed ones don't count), for the bending-master sign
    /// (`docs/design/endgame.md`, #233). Read it through `bends()`.
    #[serde(default)]
    bends: u32,
    /// The building this unit stepped inside, if any: its anchor (docs/design/buildings.md,
    /// "Units inside buildings", #226). While set, `pos` stays on the entrance tile, so floods,
    /// regions and paths need no change; everything that asks "who is standing on the map here"
    /// skips a unit that is inside (`World::units_at`). Set and cleared by
    /// `World::enter_building`/`leave_building`.
    #[serde(default)]
    inside: Option<Pos>,
}

impl Unit {
    /// How many bends this unit has finished over its life. Failed bends don't count.
    pub fn bends(&self) -> u32 {
        self.bends
    }

    /// The building this unit is inside, if any (#226).
    pub fn inside(&self) -> Option<Pos> {
        self.inside
    }
}

/// Reads `Unit::nation`, accepting a pre-#239 save's bare `Nation` (every unit's nation, with no
/// `Option` wrapper) as well as the current `None`/`Some(..)` shapes. A one-variant untagged enum
/// buffers the token first, which is what makes a self-describing format like RON treat a bare
/// value as `Some` of it instead of demanding the `Some(..)` wrapper `Option`'s own `Deserialize`
/// requires; see the test below.
fn deserialize_unit_nation<'de, D: serde::Deserializer<'de>>(
    de: D,
) -> Result<Option<Nation>, D::Error> {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Nation(Option<Nation>),
    }
    <Repr as serde::Deserialize>::deserialize(de).map(|Repr::Nation(n)| n)
}

/// A unit that died, kept so the UI and tests can see what happened.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Death {
    pub id: UnitId,
    pub name: String,
    pub tick: u64,
    pub cause: DeathCause,
}

/// Where `count` starting units go: walkable tiles, spiralling out from the map center.
fn spawn_points(map: &TileMap, count: usize) -> Vec<Pos> {
    let center = Pos::new(map.width() / 2, map.height() / 2);
    let mut candidates: Vec<Pos> = map
        .iter()
        .filter(|(_, t)| t.terrain.is_walkable() && t.resource.is_none())
        .map(|(p, _)| p)
        .collect();
    candidates.sort_by_key(|p| ((p.x - center.x).pow(2) + (p.y - center.y).pow(2), *p));
    candidates.truncate(count);
    candidates
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct World {
    pub seed: u64,
    pub tick: u64,
    pub map: TileMap,
    pub nation: Nation,
    /// Living units, ordered by id. Dead units are removed (see `deaths`), so a unit's index
    /// here can change; use `UnitId` to refer to one.
    pub units: Vec<Unit>,
    research: Research,
    /// The nation's modifiers with every known tech applied. Recomputed when a tech completes;
    /// read it through `modifiers()`.
    modifiers: Modifiers,
    /// Units that have died, oldest first.
    pub deaths: Vec<Death>,
    /// Designated work that isn't finished, in creation order.
    jobs: Vec<Job>,
    next_job: u32,
    /// Commands applied so far (`World::apply`), for replay.
    command_log: Vec<Recorded>,
    /// Renewable resources that have been worked and are growing back, with the seconds of
    /// growth each still needs. They stay on the map but can't be worked until they ripen.
    resting: BTreeMap<Pos, f32>,
    /// Harvest tiles with a standing order (#81): when the resource here next ripens, a new
    /// `JobKind::Harvest` is queued automatically instead of waiting for the player to
    /// re-designate it. Cleared by `CancelHarvest`.
    standing_harvest: BTreeSet<Pos>,
    /// Item stacks lying on the ground, one stack per tile, waiting to be hauled.
    ground: BTreeMap<Pos, Stack>,
    /// Tiles designated as stockpile.
    zones: BTreeSet<Pos>,
    /// What the stockpile tiles hold, one kind of item per tile.
    stored: BTreeMap<Pos, Stack>,
    /// Build designations that aren't finished, by tile (`Command::DesignateBuild`, #64).
    sites: BTreeMap<Pos, Site>,
    /// Finished buildings' anchors, keyed `(y, x)` so iterating matches `buildings()`'s old
    /// row-major scan of the map. Maintained wherever a building finishes (`raise`, #292); not
    /// saved (`#[serde(skip)]`), so it's rebuilt by a map scan once after loading
    /// (`World::load`) rather than on every once-a-second endeavor check.
    #[serde(skip)]
    building_index: BTreeMap<(i32, i32), Pos>,
    /// How units walk. Every unit uses these for now; a nation that can cross water would get
    /// its own.
    rules: MoveRules,
    /// How a unit hauling a stack moves: `rules` plus rail (#135). Built from the building defs
    /// when the world is made, so a save from before rail existed has the plain `rules` here.
    #[serde(default)]
    cart_rules: MoveRules,
    /// Which tiles can reach which under `rules`, or `None` when the map has changed since it
    /// was computed. Recomputed on demand; see `regions.rs`.
    #[serde(skip)]
    regions: Option<Regions>,
    /// Every bend that has finished or failed, oldest first.
    bend_events: Vec<BendEvent>,
    /// Watered patches that haven't dried yet, in `(dries_at, centre)` order (Water the Land).
    patches: Vec<Patch>,
    /// Tiles that sprouted lately and the tick their fallow time ends. Expired entries are
    /// dropped as time passes.
    fallow: BTreeMap<Pos, u64>,
    /// Lit heat sources by tile: Hearthflames today, campfires later (`heat.rs`).
    heat: BTreeMap<Pos, HeatSource>,
    /// Warmth auras on units, by carrier (Inner Fire, #123; `heat.rs`).
    #[serde(default)]
    auras: BTreeMap<UnitId, Aura>,
    /// Finished orchards by tile, with what each has planted (`orchard.rs`, #142).
    #[serde(default)]
    orchards: BTreeMap<Pos, orchard::Orchard>,
    /// Orchard-planted fruit trees that haven't ripened for the first time yet (a subset of
    /// `resting`).
    #[serde(default)]
    saplings: BTreeSet<Pos>,
    /// Lifetime counts for the end-game signs: items harvested, scrolls read (`tally.rs`, #233).
    #[serde(default)]
    tallies: Tallies,
    /// Where the end game stands (`endeavor.rs`, #234): Dormant until every sign is met at once.
    #[serde(default)]
    endeavor: EndeavorState,
    /// Which signs have been met so far, and the endeavor's events.
    #[serde(default)]
    endeavor_log: EndeavorLog,
    /// Resources still underground (`ResourceDef::buried`, #133), by tile: off the map until a
    /// Seismic Sense or a Bore Tunnel uncovers them (`bending.rs`). Saves from before #133 have
    /// none.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    buried: BTreeMap<Pos, ResourceId>,
    /// Where this world's settlers began, if it was founded at a chosen site (`found`,
    /// `world/found.rs`, #289) rather than `new`'s map-centre spiral. `None` for `new` and for
    /// every save from before #289, so neither changes a byte. Read it through `start()`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start: Option<Pos>,
    /// The id the next unit gets, whether it is a newcomer (`population.rs`, #232) or some
    /// future runtime spawn: always past every id a unit (living or dead) has ever had, so none
    /// is reused. A save from before this counter existed deserializes it as 0; `World::load`
    /// fixes it up to the same invariant from the units and deaths already on file.
    #[serde(default)]
    next_unit: u32,
    /// Newcomers who have joined since the world began (`population.rs`, #232), oldest first.
    /// Read it through `arrivals()`.
    #[serde(default)]
    arrivals: Vec<Arrival>,
    /// Picks which tile an orchard plants next (`orchard.plant`).
    #[serde(default = "orchard::default_rng")]
    orchard_rng: SimRng,
    /// Picks which tiles of a dried patch sprout (`bending.sprout`).
    sprout_rng: SimRng,
    /// Names a newcomer (`population.names`). A save from before #232 loads this seeded by 0
    /// instead of the world seed (as `orchard_rng` does for #142); still deterministic.
    #[serde(default = "population::default_rng")]
    growth_rng: SimRng,
    rng: SimRng,
}

impl World {
    pub fn new(cfg: &MapGenConfig, nation: Nation, starting_units: u32, defs: &Defs) -> Self {
        let map = mapgen::generate(cfg, defs);
        let spawns = spawn_points(&map, starting_units as usize);
        Self::with_map(cfg.seed, map, nation, &spawns, defs)
    }

    /// A world on a given map, with one unit at each of `spawns`. For tests and tools that need a
    /// hand-built map; `new` is the usual way in.
    pub fn with_map(seed: u64, map: TileMap, nation: Nation, spawns: &[Pos], defs: &Defs) -> Self {
        let mut world = Self {
            seed,
            tick: 0,
            map,
            nation,
            units: Vec::new(),
            research: Research::new(nation, defs),
            modifiers: defs.nation(nation).modifiers,
            deaths: Vec::new(),
            jobs: Vec::new(),
            next_job: 0,
            command_log: Vec::new(),
            resting: BTreeMap::new(),
            standing_harvest: BTreeSet::new(),
            ground: BTreeMap::new(),
            zones: BTreeSet::new(),
            stored: BTreeMap::new(),
            sites: BTreeMap::new(),
            building_index: BTreeMap::new(),
            rules: MoveRules::default(),
            cart_rules: defs.cart_rules(MoveRules::default()),
            regions: None,
            bend_events: Vec::new(),
            patches: Vec::new(),
            fallow: BTreeMap::new(),
            heat: BTreeMap::new(),
            auras: BTreeMap::new(),
            orchards: BTreeMap::new(),
            saplings: BTreeSet::new(),
            tallies: Tallies::default(),
            endeavor: EndeavorState::Dormant,
            endeavor_log: EndeavorLog::default(),
            buried: BTreeMap::new(),
            start: None,
            next_unit: 0,
            arrivals: Vec::new(),
            orchard_rng: orchard::new_rng(seed),
            sprout_rng: rng::stream(seed, "bending.sprout"),
            growth_rng: population::new_rng(seed),
            rng: rng::stream(seed, "world.units"),
        };
        world.bury(defs);
        world.refresh_modifiers(defs);
        world.spawn_units(spawns, defs);
        world.next_unit = spawns.len() as u32;
        world
    }

    /// Takes every buried resource on the map off it, into `buried`. A world does this when it
    /// is made; a tool that puts one on the map afterwards (a demo stage) calls it again.
    pub fn bury(&mut self, defs: &Defs) {
        let found: Vec<(Pos, ResourceId)> = self
            .map
            .iter()
            .filter_map(|(p, t)| t.resource.filter(|&r| defs.resource(r).buried).map(|r| (p, r)))
            .collect();
        for (p, r) in found {
            self.map[p].resource = None;
            self.buried.insert(p, r);
        }
    }

    /// How many resources are still underground (#133). Where they are is not for the frontends
    /// to know.
    pub fn buried_count(&self) -> usize {
        self.buried.len()
    }

    /// Checks that a world read from a save is consistent enough to run: the things that would
    /// panic later (grid shape, research tables, unit placement), not every invariant.
    pub(crate) fn validate(&self, defs: &Defs) -> Result<(), String> {
        if !self.map.is_well_formed() {
            return Err("the map's size doesn't match its tiles".into());
        }
        if !self.research.fits(defs) {
            return Err("the research state doesn't match the tech tree".into());
        }
        if !self.units.windows(2).all(|w| w[0].id < w[1].id) {
            return Err("units are out of order".into());
        }
        if let Some(u) = self.units.iter().find(|u| !self.map.in_bounds(u.pos)) {
            return Err(format!("{} is off the map", u.name));
        }
        Ok(())
    }

    /// The settlement's modifiers systems must read when there is no particular unit to ask: the
    /// world's nation base times every completed tech's multipliers. A unit's own effective
    /// modifiers (which may differ, #239) are `modifiers_for`. Never read `NationDef::modifiers`
    /// directly, or techs do nothing.
    pub fn modifiers(&self) -> &Modifiers {
        &self.modifiers
    }

    /// `unit`'s own nation: `Unit::nation` if it has one, else the settlement's (`self.nation`).
    /// Every system that cares which nation a unit belongs to reads this, never `Unit::nation`
    /// directly.
    pub fn nation_of(&self, unit: &Unit) -> Nation {
        unit.nation.unwrap_or(self.nation)
    }

    /// `unit`'s effective modifiers: its own nation's base times every tech the settlement knows
    /// (teaching across nations is deferred, so a unit of another nation doesn't benefit from
    /// techs its own nation hasn't researched, but does ride on the settlement's; #239). Same
    /// `TechId` order as `modifiers()`, so a world where every unit is the settlement's own
    /// nation gets bit-for-bit the same values either way.
    pub fn modifiers_for(&self, unit: &Unit, defs: &Defs) -> Modifiers {
        research::effective_modifiers(self.nation_of(unit), self.research.known_techs(), defs)
    }

    /// How the effective value of the modifier `name` is built up for `unit`, for tooltips: its
    /// own nation's base, then each known tech that multiplies it, as `(source name, factor)`.
    /// Empty for an unknown name.
    pub fn modifier_breakdown<'d>(
        &self,
        unit: &Unit,
        name: &str,
        defs: &'d Defs,
    ) -> Vec<(&'d str, f32)> {
        research::modifier_breakdown(name, self.nation_of(unit), self.research.known_techs(), defs)
    }

    fn refresh_modifiers(&mut self, defs: &Defs) {
        self.modifiers =
            research::effective_modifiers(self.nation, self.research.known_techs(), defs);
        self.refresh_heat_radii(defs);
    }

    /// What the settlement has done over the whole game (`tally.rs`).
    pub fn tallies(&self) -> &Tallies {
        &self.tallies
    }

    pub fn research(&self) -> &Research {
        &self.research
    }

    /// Is something whose `unlocked_by` is `unlocked_by` available to this settlement right now:
    /// no tech locks it (`None`), or the nation knows the one that does (`Unlocks::buildings`,
    /// `Unlocks::jobs`, issue #22).
    pub fn is_unlocked(&self, unlocked_by: Option<TechId>) -> bool {
        unlocked_by.is_none_or(|t| self.research.is_known(t))
    }

    /// Sets the research target: the queue becomes `tech` plus its unknown prereqs. Banked
    /// knowledge is kept. Reached through `Command::SetResearch`.
    fn set_research(&mut self, tech: TechId, defs: &Defs) -> Result<(), ResearchError> {
        self.research.set_target(tech, self.nation, defs)
    }

    fn spawn_units(&mut self, spawns: &[Pos], defs: &Defs) {
        let nation = defs.nation(self.nation);
        let mut names = nation.names.clone();
        names.shuffle(&mut rng::stream(self.seed, "world.names"));
        for (i, &pos) in spawns.iter().enumerate() {
            let variant = self.rng.random_range(0..nation.characters.len() as u8);
            // Reuse the pool with a numeral suffix if there are more units than names.
            let name = match i / names.len() {
                0 => names[i % names.len()].clone(),
                n => format!("{} {}", names[i % names.len()], n + 1),
            };
            self.units.push(Unit {
                id: UnitId(i as u32),
                name,
                nation: None,
                pos,
                variant,
                needs: Needs::full(&defs.needs),
                chi: defs.chi.max,
                carrying: None,
                need_task: None,
                job_task: None,
                route: None,
                work: 0.0,
                bed: None,
                need_retry_at: [0; 3],
                claim_retry_at: 0,
                move_progress: 0.0,
                idle_until: 0,
                wander_left: 0,
                bends: 0,
                inside: None,
            });
        }
    }

    /// The living unit with this id, if it hasn't died.
    pub fn unit(&self, id: UnitId) -> Option<&Unit> {
        self.units.binary_search_by_key(&id, |u| u.id).ok().map(|i| &self.units[i])
    }

    /// Tells the world that `map` was edited, so cached connectivity can't be trusted. The world
    /// does this itself for changes it makes (clearing a resource); only code that writes to
    /// `map` directly needs to call it.
    pub fn map_changed(&mut self) {
        self.regions = None;
    }

    /// The connected regions of the current map, recomputed first if the map has changed.
    fn regions(&mut self) -> &Regions {
        let (map, rules) = (&self.map, &self.rules);
        self.regions.get_or_insert_with(|| Regions::compute(map, rules))
    }

    fn walkable(&self, p: Pos) -> bool {
        self.map.get(p).is_some_and(|t| t.terrain.is_walkable() && t.resource.is_none())
    }

    /// Advances the world by one tick: research, units' needs and chi, resource regrowth and
    /// drying patches and burning-out flames run (units whose health reaches 0 die and are removed, in id order),
    /// a newcomer may join (`population.rs`, #232), then each survivor acts, in id order: it
    /// meets a need if one is low, else carries on with its job, else claims one, else wanders.
    /// See `work.rs`. Last, once a second, the endeavor checks its signs (`endeavor.rs`).
    pub fn tick(&mut self, defs: &Defs) {
        self.tick += 1;
        let completed = self.research.completions().len();
        self.research.tick(self.tick, defs);
        if self.research.completions().len() != completed {
            self.refresh_modifiers(defs);
        }
        self.tick_needs(defs);
        self.tick_growth(defs);
        self.tick_population(defs);
        self.tick_orchards(defs);
        self.tick_irrigation(defs);
        self.tick_heat();
        for i in 0..self.units.len() {
            self.act(i, defs);
        }
        self.tick_endeavor(defs);
    }

    /// What a unit with nothing to do does (issue #110). It stands still for `idle_pause_seconds`;
    /// then, with chance `idle_wander_chance`, it wanders 1 to `idle_wander_steps` random steps
    /// (at a speed set by the `move_speed` modifier) before standing again. All draws come from
    /// the `world.units` stream. A unit inside a building (#226) never reaches here in practice
    /// (whatever put it inside holds its `job_task`), but this guards against moving a hidden
    /// unit's `pos` off its entrance if it ever did.
    fn wander(&mut self, i: usize, defs: &Defs) {
        if self.units[i].inside.is_some() || self.tick < self.units[i].idle_until {
            return;
        }
        let s = defs.jobs;
        if self.units[i].wander_left == 0 {
            if self.rng.random::<f32>() >= s.idle_wander_chance {
                self.rest_idle(i, defs);
                return;
            }
            self.units[i].wander_left = self.rng.random_range(1..=s.idle_wander_steps);
            self.units[i].move_progress = 0.0;
        }
        let speed = self.modifiers_for(&self.units[i], defs).move_speed;
        self.units[i].move_progress += speed;
        if self.units[i].move_progress < BASE_TICKS_PER_STEP {
            return;
        }
        self.units[i].move_progress -= BASE_TICKS_PER_STEP;
        let pos = self.units[i].pos;
        let options: Vec<Pos> =
            self.map.neighbors(pos, false).filter(|&q| self.walkable(q)).collect();
        if options.is_empty() {
            self.units[i].wander_left = 0;
        } else {
            self.units[i].pos = options[self.rng.random_range(0..options.len())];
            self.units[i].wander_left -= 1;
        }
        if self.units[i].wander_left == 0 {
            self.rest_idle(i, defs);
        }
    }

    /// Starts an idle pause: the unit stands still for `idle_pause_seconds`.
    fn rest_idle(&mut self, i: usize, defs: &Defs) {
        let pause = (defs.jobs.idle_pause_seconds * TICKS_PER_SECOND as f32).ceil() as u64;
        self.units[i].wander_left = 0;
        self.units[i].idle_until = self.tick + pause;
    }
}

impl World {
    fn tick_needs(&mut self, defs: &Defs) {
        let tick = self.tick;
        let mut dead = Vec::new();
        let chi_regen = self.modifiers.chi_regen;
        for mut u in std::mem::take(&mut self.units) {
            let rest = if u.is_sleeping() { self.sleep_factor_for(&u, defs) } else { 1.0 };
            u.needs.tick_scaled(&defs.needs, u.is_sleeping(), rest);
            u.chi = defs.chi.regen(u.chi, u.is_sleeping(), chi_regen);
            if u.needs.is_dead() {
                dead.push(u);
            } else {
                self.units.push(u);
            }
        }
        for u in dead {
            let cause = u.needs.depleted().unwrap_or(DeathCause::Starvation);
            self.deaths.push(Death { id: u.id, name: u.name.clone(), tick, cause });
            self.release(&u);
        }
    }
}

#[cfg(test)]
mod idle_tests;

#[cfg(test)]
mod rest_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::TechCompleted;

    fn world(nation: Nation) -> (World, Defs) {
        let defs = Defs::bundled();
        let cfg = MapGenConfig { seed: 7, ..Default::default() };
        (World::new(&cfg, nation, 4, &defs), defs)
    }

    #[test]
    fn units_spawn_on_walkable_tiles() {
        let (w, _) = world(Nation::Earth);
        assert_eq!(w.units.len(), 4);
        assert!(w.units.iter().all(|u| w.walkable(u.pos)));
    }

    #[test]
    fn units_get_unique_deterministic_names() {
        let (a, _) = world(Nation::Fire);
        let (b, _) = world(Nation::Fire);
        let names: Vec<_> = a.units.iter().map(|u| u.name.clone()).collect();
        assert_eq!(names, b.units.iter().map(|u| u.name.clone()).collect::<Vec<_>>());
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len());
    }

    #[test]
    fn units_stay_on_walkable_tiles() {
        let (mut w, defs) = world(Nation::Water);
        for _ in 0..500 {
            w.tick(&defs);
        }
        assert!(w.units.iter().all(|u| w.walkable(u.pos)));
    }

    #[test]
    fn airbenders_cover_more_ground_than_earthbenders() {
        // Count steps taken during a long wander: air's move_speed modifier is higher than earth's.
        let steps = |nation| {
            let (mut w, defs) = world(nation);
            for u in &mut w.units {
                u.wander_left = 200;
            }
            let mut moved = 0;
            for _ in 0..300 {
                let before: Vec<Pos> = w.units.iter().map(|u| u.pos).collect();
                w.tick(&defs);
                moved += w.units.iter().zip(&before).filter(|(u, b)| u.pos != **b).count();
            }
            moved
        };
        assert!(steps(Nation::Air) > steps(Nation::Earth));
    }

    fn tech(defs: &Defs, id: &str) -> TechId {
        defs.tech_id(id).unwrap()
    }

    fn spent_millis(w: &World, defs: &Defs) -> u64 {
        w.research().completions().iter().map(|c| u64::from(defs.tech(c.tech).cost) * 1000).sum()
    }

    #[test]
    fn research_starts_with_the_nations_starting_techs() {
        let (w, defs) = world(Nation::Fire);
        let known: Vec<_> = w.research().known_techs().collect();
        assert_eq!(known, [tech(&defs, "fire_making")]);
        assert!(w.research().queue().is_empty());
        assert_eq!(w.research().pool_millis(), 0);
    }

    #[test]
    fn passive_rate_is_converted_to_millis_per_tick() {
        // 3 and 9 points a minute at 10 ticks a second.
        assert_eq!(Defs::bundled().research.passive_millis_per_tick, 5);
        assert_eq!(Defs::bundled().research.study_millis_per_tick, 15);
    }

    #[test]
    fn fire_making_completes_when_the_pacing_math_says() {
        let (mut w, defs) = world(Nation::Earth);
        let fm = tech(&defs, "fire_making");
        w.set_research(fm, &defs).unwrap();
        // 25 points at 3 a minute is 500 seconds, so tick 5000.
        for _ in 0..4999 {
            w.tick(&defs);
        }
        assert!(!w.research().is_known(fm));
        w.tick(&defs);
        assert!(w.research().is_known(fm));
        assert_eq!(w.research().completions(), [TechCompleted { tech: fm, tick: 5000 }]);
        assert!(w.research().queue().is_empty());
        assert_eq!(w.research().pool_millis(), 0);
    }

    #[test]
    fn deep_tech_queues_its_unknown_prereq_chain() {
        let (mut w, defs) = world(Nation::Earth);
        let target = tech(&defs, "earthen_walls");
        w.set_research(target, &defs).unwrap();
        let mut expected: Vec<TechId> =
            ["shelter", "carpentry", "masonry", "earth_shaping", "earthen_walls"]
                .iter()
                .map(|n| tech(&defs, n))
                .collect();
        expected.sort();
        // stoneworking is a starting tech, so it isn't queued.
        assert_eq!(w.research().queue(), expected);
        assert_eq!(w.research().queue().last(), Some(&target));
    }

    #[test]
    fn set_research_rejects_other_nations_and_known_techs() {
        let (mut w, defs) = world(Nation::Earth);
        assert_eq!(
            w.set_research(tech(&defs, "lightning_generation"), &defs),
            Err(ResearchError::WrongNation)
        );
        assert_eq!(
            w.set_research(tech(&defs, "stoneworking"), &defs),
            Err(ResearchError::AlreadyKnown)
        );
        assert_eq!(w.set_research(TechId(u16::MAX), &defs), Err(ResearchError::UnknownTech));
        assert!(w.research().queue().is_empty());
    }

    #[test]
    fn idle_knowledge_caps_without_a_target_and_carries_over() {
        let (mut w, defs) = world(Nation::Earth);
        let cap = defs.research.idle_knowledge_cap_millis;
        assert!(cap > 0 && cap < 25_000, "the buffer should be a small amount, below any tech");
        // Passive trickle alone stalls at the idle cap (#284): no target means no reason to bank
        // further than a small buffer.
        for _ in 0..6000 {
            w.tick(&defs);
        }
        assert_eq!(w.research().pool_millis(), cap);
        w.set_research(tech(&defs, "shelter"), &defs).unwrap();
        w.tick(&defs);
        // What was banked carries over; the idle buffer alone can't pay for even the cheapest
        // tech, so nothing completes yet.
        assert_eq!(w.research().pool_millis(), cap + defs.research.passive_millis_per_tick);
        assert!(w.research().completions().is_empty());
    }

    #[test]
    fn the_pool_cap_stops_several_techs_completing_in_one_tick() {
        let (mut w, defs) = world(Nation::Earth);
        // Before #284, banking far ahead of any target let shelter (25) and carpentry (60)
        // complete together the moment both became affordable. The cap now trims the pool to
        // the queue's front cost before it is ever spent, so only shelter completes here.
        w.research.bank_for_test(100_000);
        w.set_research(tech(&defs, "carpentry"), &defs).unwrap();
        w.tick(&defs);
        let completed: Vec<_> = w.research().completions().iter().map(|c| c.tech).collect();
        assert_eq!(completed, [tech(&defs, "shelter")]);
        assert!(!w.research().is_known(tech(&defs, "carpentry")));
    }

    #[test]
    fn changing_target_loses_no_knowledge() {
        let (mut w, defs) = world(Nation::Earth);
        w.set_research(tech(&defs, "fire_making"), &defs).unwrap();
        for _ in 0..3000 {
            w.tick(&defs);
        }
        let before = w.research().pool_millis();
        w.set_research(tech(&defs, "masonry"), &defs).unwrap();
        assert_eq!(w.research().pool_millis(), before);
        for _ in 0..9000 {
            w.tick(&defs);
        }
        // Everything earned is either still banked or was spent on a completed tech.
        assert_eq!(w.research().pool_millis() + spent_millis(&w, &defs), 5 * 12_000);
        assert!(!w.research().completions().is_empty());
    }

    #[test]
    fn research_is_deterministic() {
        let run = || {
            let (mut w, defs) = world(Nation::Water);
            w.set_research(tech(&defs, "healing"), &defs).unwrap();
            for t in 0..20_000 {
                if t == 7_000 {
                    w.set_research(tech(&defs, "water_travel"), &defs).unwrap();
                }
                w.tick(&defs);
            }
            w.research().clone()
        };
        assert_eq!(run(), run());
    }

    /// All of `m`'s modifiers as raw bits, so tests can demand bitwise equality.
    fn bits(m: &Modifiers) -> Vec<u32> {
        Modifiers::NAMES.iter().map(|n| m.get(n).unwrap().to_bits()).collect()
    }

    #[test]
    fn modifiers_start_as_the_nations_base() {
        let (w, defs) = world(Nation::Earth);
        assert_eq!(w.modifiers(), &defs.nation(Nation::Earth).modifiers);
    }

    #[test]
    fn nation_of_falls_back_to_the_settlements_nation_but_a_units_own_wins() {
        let (mut w, _) = world(Nation::Earth);
        assert_eq!(w.units[0].nation, None);
        assert_eq!(w.nation_of(&w.units[0]), Nation::Earth);
        w.units[0].nation = Some(Nation::Water);
        assert_eq!(w.nation_of(&w.units[0]), Nation::Water);
    }

    #[test]
    fn modifiers_for_reads_the_units_own_nation_not_the_settlements() {
        let (mut w, defs) = world(Nation::Water);
        assert_eq!(w.modifiers_for(&w.units[0], &defs), *w.modifiers());
        w.units[0].nation = Some(Nation::Earth);
        assert_eq!(w.modifiers_for(&w.units[0], &defs), defs.nation(Nation::Earth).modifiers);
        assert_ne!(w.modifiers_for(&w.units[0], &defs), *w.modifiers());
    }

    /// `Unit::nation` deserializes a pre-#239 save's bare `Nation` (no `Option` wrapper) as
    /// `Some` of it, alongside today's `None`/`Some(..)` shapes and a missing field (`default`).
    #[test]
    fn unit_nation_reads_a_pre_239_saves_bare_value_as_well_as_todays_shapes() {
        #[derive(serde::Deserialize)]
        struct Holder {
            #[serde(default, deserialize_with = "deserialize_unit_nation")]
            nation: Option<Nation>,
        }
        let read = |s: &str| ron::from_str::<Holder>(s).unwrap().nation;
        assert_eq!(read("(nation:fire)"), Some(Nation::Fire));
        assert_eq!(read("(nation:None)"), None);
        assert_eq!(read("(nation:Some(fire))"), Some(Nation::Fire));
        assert_eq!(read("()"), None, "missing is the settlement's nation");
    }

    #[test]
    fn completing_a_tech_changes_the_relevant_modifier() {
        let (mut w, defs) = world(Nation::Earth);
        let base = defs.nation(Nation::Earth).modifiers;
        assert_eq!(w.modifiers(), &base);
        w.set_research(tech(&defs, "carpentry"), &defs).unwrap();
        // Passive only, so capped (#284) at shelter's cost (25) first, 5000 ticks, then
        // carpentry's (60), another 12000: 17000 total, both landing exactly on the tick math.
        for _ in 0..17_000 {
            w.tick(&defs);
        }
        assert!(w.research().is_known(tech(&defs, "carpentry")));
        assert_eq!(w.modifiers().construction, base.construction * 1.1);
        assert_eq!(w.modifiers(), &Modifiers { construction: base.construction * 1.1, ..base });
        let unit = &w.units[0];
        assert_eq!(
            w.modifier_breakdown(unit, "construction", &defs),
            [("Earth Kingdom", base.construction), ("Carpentry", 1.1)]
        );
        // A modifier no known tech touches has just the nation's entry.
        assert_eq!(
            w.modifier_breakdown(unit, "move_speed", &defs),
            [("Earth Kingdom", base.move_speed)]
        );
        assert!(w.modifier_breakdown(unit, "nonsense", &defs).is_empty());
    }

    #[test]
    fn research_order_does_not_change_the_modifiers() {
        let (w, defs) = world(Nation::Earth);
        let reach = |order: [&str; 2]| {
            let mut w = w.clone();
            for t in order {
                w.set_research(tech(&defs, t), &defs).unwrap();
                // Drive the whole chain to completion, one tech a tick: the production cap
                // (#284) is about pacing, not this test, which only checks that completion
                // order doesn't change the final modifiers.
                while !w.research().queue().is_empty() {
                    w.research.bank_for_test(300_000);
                    w.tick(&defs);
                }
            }
            let order: Vec<_> = w.research().completions().iter().map(|c| c.tech).collect();
            (order, w)
        };
        let (order_a, a) = reach(["masonry", "earth_shaping"]);
        let (order_b, b) = reach(["earth_shaping", "masonry"]);
        assert_ne!(order_a, order_b, "the two runs should complete techs in different orders");
        assert!(a.research().known_techs().eq(b.research().known_techs()));
        assert_eq!(bits(a.modifiers()), bits(b.modifiers()));
        // And both match a recomputation from the known set alone.
        let fresh = research::effective_modifiers(Nation::Earth, a.research().known_techs(), &defs);
        assert_eq!(bits(a.modifiers()), bits(&fresh));
    }

    #[test]
    fn units_without_food_die_and_are_removed_deterministically() {
        let run = || {
            let (mut w, defs) = world(Nation::Earth);
            // No wild food anywhere: the units can drink but never eat.
            let food: Vec<Pos> = w
                .map
                .iter()
                .filter(|(_, t)| {
                    t.resource.is_some_and(|r| {
                        defs.harvest(r).is_some_and(|h| defs.food_of(h.item).is_some())
                    })
                })
                .map(|(p, _)| p)
                .collect();
            for p in food {
                w.map[p].resource = None;
            }
            w.map_changed();
            let mut ticks = 0;
            while !w.units.is_empty() {
                w.tick(&defs);
                ticks += 1;
                assert!(ticks < 100_000, "units never died");
            }
            w
        };
        let (a, b) = (run(), run());
        assert_eq!(a.deaths.len(), 4);
        assert_eq!(a.deaths, b.deaths);
        assert!(a.deaths.windows(2).all(|w| w[0].tick <= w[1].tick));
    }

    #[test]
    fn dead_units_leave_lookup_and_the_rest_carry_on() {
        let (mut w, defs) = world(Nation::Fire);
        // Make only unit 1 fatally ill; the others are fine.
        w.units[1].needs.hunger = 0.0;
        w.units[1].needs.health = 0.01;
        w.tick(&defs);
        assert_eq!(w.units.len(), 3);
        assert!(w.unit(UnitId(1)).is_none());
        assert_eq!(w.unit(UnitId(2)).map(|u| u.id), Some(UnitId(2)));
        assert_eq!(w.deaths.len(), 1);
        assert_eq!(w.deaths[0].id, UnitId(1));
        assert_eq!(w.deaths[0].cause, DeathCause::Starvation);
    }
}

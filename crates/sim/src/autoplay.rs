//! A deterministic scripted opening (#300, `docs/design/difficulty.md`, "Measuring it"): plain
//! code over `Command`, so it doubles as a smoke test for the command API (if a command this
//! issues ever stops applying, `crates/sim/tests/balance.rs` notices).
//!
//! `Opening` carries no progress state of its own: every decision reads the `World` it's handed
//! fresh, so calling `step` on the same world twice in a row is harmless (everything it might
//! redo is already done, and `World::apply` refuses a repeat harmlessly), and the same opening
//! given two identical worlds always issues the same commands. That is what lets the `balance`
//! harness (`crate::balance`) assert two runs give identical output.
//!
//! It does five things, each read from world state rather than tracked by the script:
//! - a stockpile near the settlement's anchor, once there is none;
//! - a standing harvest order (`Command::DesignateRepeatingHarvest`) on every wild, renewable
//!   food resource within `FORAGE_RADIUS` tiles;
//! - a one-shot harvest on whatever nearby resource yields a bed's or a campfire's cost (wood,
//!   on the bundled data): without this, a preset with no `starting_stores` (Hard) would leave a
//!   bed designated with nothing to build it from, and a unit would sit there re-planning a
//!   route to fetch material that never arrives. `beds`/`campfire` only ever designate once
//!   enough of each cost item is already in stores or on the ground, so no building is ever
//!   designated that can't finish: the sim's own retry backoff (`data/jobs.toml`'s
//!   `retry_seconds`) would otherwise have a stuck unit re-planning a whole-map route every few
//!   seconds for as long as the opening runs, the exact cost `tests/balance.rs` and the
//!   `balance` binary need to stay cheap;
//! - a bed per person, built where there's room, one at a time;
//! - a campfire, once its tech is known (`BuildingDef::unlocked_by`), with no nation check: Fire
//!   starts knowing it, every other nation researches it first (`RESEARCH_PLAN`).
//!
//! and drives a short, shared research queue, skipping whatever a nation already knows or can't
//! research (`Command::SetResearch`'s own `WrongNation`/`AlreadyKnown` refusals), so the same
//! plan plays for every nation with no `if nation == ..`.
//!
//! **Bending** (`Opening::with_bending`, #343, `docs/design/endgame.md`, "Mastery pacing") is
//! off by default. When on, `step` also keeps up to `bend_slots` bends designated within
//! `FORAGE_RADIUS` tiles of the anchor: for every empty slot, the ability the settlement can
//! cast (`World::check_ability`) least often so far that has a valid target nearby (nearest tile
//! first, `World::apply`'s own validation deciding; ties go to the cheapest by chi), so chi
//! never sits idle and a settler picks up a mix of forms rather than one ability spammed forever
//! (`keep_bends_designated`, #360). This is generic over `data/bending.toml`, with no
//! `if nation == ..`: the `balance` harness's mastery mode is what exercises it.
//!
//! **`Opening::with_full_research`** (#352) swaps `RESEARCH_PLAN` for the settlement's whole
//! visible tech tree, in file order (`Defs::techs_for`: the shared tier, then the nation's own
//! branch, prereqs always before what needs them). `docs/design/materials.md`'s own finding is
//! why this exists: the short plan never reaches a nation's own branch, so Earth never researches
//! tunneling and Air never researches gliders, and `with_bending`'s bend step then runs out of
//! anything for them to cast once their one tech-free ability's targets near the anchor are used
//! up. `Economy` always researches this way; the `balance` binary's `--mastery` sweep opts in
//! with it to check whether that changes Earth's and Air's pace.
//!
//! ## Economy (#352)
//!
//! `Economy` is a second opening, written the same way as `Opening` and sharing its low-level
//! moves (the free functions below), that plays toward the Harmony Gate's signs
//! (`docs/design/materials.md`, section 7) instead of just keeping a settlement alive. Each step
//! it additionally:
//! - researches the whole visible tree (`with_full_research`'s reasoning above, always on here),
//!   shared tier first unless `with_branch_first` is set (`docs/design/pacing.md`, "Measuring
//!   it"; #434);
//! - keeps a one-shot harvest designated on every reachable stone and ore deposit within
//!   `DEPOSIT_RADIUS` tiles (outcrops, boulders, veins: whatever `data/resources.toml` holds),
//!   read generically from the "stone" and "ore" signs' `items` in `data/endeavor.toml`, so a new
//!   deposit kind or a second ore (#349, #134) needs no change here;
//! - keeps a bed one ahead of the living population (instead of a fixed headcount, so growth
//!   doesn't stall on a full house) and one of every other building the nation can reach
//!   (`endeavor::reachable_buildings`), each once its cost is affordable;
//! - (unless `farms` is false, `Economy::without_farms`, materials.md's own `--no-farms` check)
//!   tills one farm plot per `PEOPLE_PER_FARM_PLOT` people and sows each the moment it's empty,
//!   and raises an orchard where the nation has one;
//! - keeps up to `people` bends designated within `DEPOSIT_RADIUS` tiles of the anchor
//!   (`with_bending`'s own mechanism), so Earth's Bore Tunnel and Seismic Sense, once researched,
//!   work the same ground the quarry and mine orders do. A bend that makes an item some unbuilt
//!   building is short of (Cut Ice for the igloo, Metalbend for the Deep Mine) goes first (#376);
//! - keeps a one-shot harvest designated on every surface scroll and uncovered lost ruin within
//!   `SCROLL_RADIUS` tiles (`recover_scrolls`, #404, `docs/design/cartography.md`): raising the
//!   Map Room itself needs no extra code here, since `raise_buildings` already builds one of
//!   every reachable building once its tech is known, and a Map Room's own Chart job is what
//!   uncovers a buried ruin in the first place.
//!
//! Stateless and `Copy`, and deterministic for the reasons given above for `Opening`.

use crate::bending::{AbilityId, Effect};
use crate::building::{BuildingId, CostItem};
use crate::defs::Defs;
use crate::endeavor::{self, SignKind, Tally};
use crate::geom::{Footprint, Pos};
use crate::jobs::ItemId;
use crate::mapgen::Tile;
use crate::nation::Nation;
use crate::research::TechId;
use crate::world::{Command, PlotStage, TICKS_PER_SECOND, World};

/// How far from the settlement's anchor (`World::settlement_anchor`) the opening designates
/// standing harvest orders on wild food, and looks for the material a bed or a campfire costs
/// (`docs/design/difficulty.md`, "Measuring it").
pub const FORAGE_RADIUS: i32 = 15;

/// In-game minutes between the opening's decisions (`Opening::due`). `World::tick` runs
/// `TICKS_PER_SECOND * 60` ticks a minute either way; this only sets how often the opening looks
/// at the world, not how fast anything it designates gets done.
pub const STEP_MINUTES: f32 = 1.0;

/// The opening's research queue, in order: fire-making first (a campfire needs it, and only
/// Fire starts knowing it, `data/research.toml`), then farming (a hedge against tired land
/// outgrowing the start box, #296). Both are shared-tier techs, so the list is the same for every
/// nation; `Opening::research` skips an entry already known.
pub const RESEARCH_PLAN: &[&str] = &["fire_making", "farming"];

/// How many stockpile tiles the opening lays near the anchor, once there is no stockpile at all.
const STOCKPILE_TILES: usize = 4;

/// How many stockpile tiles `grow_stockpile` lays at a time, once something is stranded on the
/// ground with no room (#378).
const STOCKPILE_TILES_PER_ITEM: u32 = 3;

/// How far out the opening will look for room to build a bed or a campfire, in tiles, before
/// giving up for this step (and trying again next one).
const BUILD_SEARCH_RADIUS: i32 = 30;

/// How far out `Economy` looks for stone and ore deposits to quarry and mine, and the bend it
/// keeps up toward them (module doc, "Economy"): richer than `FORAGE_RADIUS`, since
/// `docs/design/materials.md` ("What the map holds") finds most of a seed's stone and ore only
/// past 20 tiles.
pub const DEPOSIT_RADIUS: i32 = 40;

/// How far out `Economy` looks for a surface scroll or an uncovered lost ruin to recover
/// (`recover_scrolls`, module doc; #404). `docs/design/cartography.md`'s own supply survey
/// (`balance --supply`) measures scrolls in reach out to 90 tiles, calling that "the whole
/// landmass the [default] site is on"; this reuses the same number so the opening never leaves
/// something reachable sitting uncollected.
pub const SCROLL_RADIUS: i32 = 90;

/// People per farm plot `Economy` keeps tilled (module doc, "Economy").
const PEOPLE_PER_FARM_PLOT: u32 = 3;

/// A scripted opening for one settlement. Stateless beyond `people`, `bend_slots` and
/// `full_research` (`Opening::step` reads everything else off the `World` it's given), so it's
/// `Copy` and safe to keep around for a whole run.
#[derive(Debug, Clone, Copy)]
pub struct Opening {
    /// Beds the opening keeps built: usually the settlement's starting headcount, but a harness
    /// that wants fewer or more may pass anything.
    pub people: u32,
    /// Bends the opening keeps designated near the anchor (module doc, "Bending"); `None` leaves
    /// bending alone, which is what every opening did before #343.
    pub bend_slots: Option<u32>,
    /// Research the settlement's whole visible tech tree instead of `RESEARCH_PLAN` (module
    /// doc, "Economy"; #352). Set with `with_full_research`.
    full_research: bool,
}

impl Opening {
    pub fn new(people: u32) -> Self {
        Self { people, bend_slots: None, full_research: false }
    }

    /// The same opening, but every `step` also keeps up to `bend_slots` bends designated near
    /// the anchor (module doc, "Bending"): the balance harness's mastery mode (#343).
    pub fn with_bending(people: u32, bend_slots: u32) -> Self {
        Self { people, bend_slots: Some(bend_slots), full_research: false }
    }

    /// The same opening, but `research` queues the settlement's whole visible tech tree in file
    /// order instead of `RESEARCH_PLAN` (module doc, "Economy"; #352).
    pub fn with_full_research(self) -> Self {
        Self { full_research: true, ..self }
    }

    /// Ticks between decisions (`STEP_MINUTES`), at least 1.
    fn step_ticks() -> u64 {
        ((STEP_MINUTES * 60.0 * TICKS_PER_SECOND as f32).round() as u64).max(1)
    }

    /// Whether `tick` is one of the opening's fixed decision points. A harness calls `step` when
    /// this is true, including at tick 0.
    pub fn due(tick: u64) -> bool {
        tick.is_multiple_of(Self::step_ticks())
    }

    /// Issues whatever the opening still wants done, reading `world` fresh. Call this once per
    /// `Opening::due` tick.
    pub fn step(&self, world: &mut World, defs: &Defs) {
        let anchor = world.settlement_anchor();
        ensure_stockpile(world, defs, anchor);
        forage_wild_food(world, defs, anchor);
        self.materials(world, defs, anchor);
        self.beds(world, defs, anchor);
        self.campfire(world, defs, anchor);
        self.research(world, defs);
        if let Some(slots) = self.bend_slots {
            keep_bends_designated(world, defs, anchor, FORAGE_RADIUS, slots, &[]);
        }
    }

    /// A one-shot harvest on any nearby resource that yields what a bed or a campfire costs
    /// (module doc). Not a standing order: these resources don't regrow.
    fn materials(&self, world: &mut World, defs: &Defs, anchor: Pos) {
        let Some(bed) = defs.building_id("bed") else { return };
        let Some(campfire) = defs.building_id("campfire") else { return };
        let items: Vec<ItemId> = defs
            .building(bed)
            .cost
            .iter()
            .chain(defs.building(campfire).cost.iter())
            .map(|c: &CostItem| c.item)
            .collect();
        gather_materials(world, defs, anchor, &items);
    }

    /// Builds a bed if there's room for one more and the headcount still needs it, one at a
    /// time (no second bed designated while one is already under way) and only once its full
    /// cost is already in stores or on the ground (module doc, and `ensure_building`).
    fn beds(&self, world: &mut World, defs: &Defs, anchor: Pos) {
        let Some(bed) = defs.building_id("bed") else { return };
        ensure_building(world, defs, anchor, bed, self.people, BUILD_SEARCH_RADIUS);
    }

    /// Builds one campfire, once fire-making (or whatever unlocks it) is known and its cost is
    /// already in stores or on the ground.
    fn campfire(&self, world: &mut World, defs: &Defs, anchor: Pos) {
        let Some(campfire) = defs.building_id("campfire") else { return };
        ensure_building(world, defs, anchor, campfire, 1, BUILD_SEARCH_RADIUS);
    }

    /// Queues the next tech the nation doesn't already know, once nothing is queued: from
    /// `RESEARCH_PLAN`, or from the whole visible tree in file order if `full_research` is set
    /// (module doc, "Economy"). Skips a tech belonging to another nation's branch, with no
    /// `if nation == ..`: `SetResearch`'s own refusal tells it to move on.
    fn research(&self, world: &mut World, defs: &Defs) {
        if self.full_research {
            queue_next_known_tech(world, defs, defs.techs_for(world.nation).map(|(id, _)| id));
        } else {
            queue_next_known_tech(
                world,
                defs,
                RESEARCH_PLAN.iter().filter_map(|id| defs.tech_id(id)),
            );
        }
    }
}

/// The economy opening (#352, `docs/design/materials.md` section 7): see the module doc,
/// "Economy", for everything it does beyond `Opening`'s basics.
#[derive(Debug, Clone, Copy)]
pub struct Economy {
    /// Bends `Economy` keeps designated near the anchor, same meaning as `Opening::bend_slots`
    /// but always on.
    pub people: u32,
    /// Till farm plots and raise an orchard (module doc, "Economy")? `true` unless built with
    /// `without_farms`.
    pub farms: bool,
    /// Research the nation's own branch before the shared tier, instead of the shared tier
    /// first (`docs/design/pacing.md`, "Measuring it"; #434's `--branch-first`). Set with
    /// `with_branch_first`; both orders research the whole visible tree either way
    /// (`research_plan`), so this only changes which tech is picked first when nothing is
    /// queued.
    pub branch_first: bool,
}

impl Economy {
    pub fn new(people: u32) -> Self {
        Self { people, farms: true, branch_first: false }
    }

    /// The same opening, but it tills no farm plots and raises no orchard
    /// (`docs/design/materials.md` section 7's `--no-farms`, owner question 2's check).
    pub fn without_farms(people: u32) -> Self {
        Self { people, farms: false, branch_first: false }
    }

    /// The same opening, but `research_plan` tries the nation's own branch before the shared
    /// tier (`docs/design/pacing.md`, "Measuring it"; #434), closer to how a person plays than
    /// the default shared-tier-first order.
    pub fn with_branch_first(self) -> Self {
        Self { branch_first: true, ..self }
    }

    /// `defs.techs_for(nation)`'s own set, reordered with the nation's branch before the shared
    /// tier if `self.branch_first`, else left in file order (shared tier first). Either order is
    /// just a tie-break for which tech `queue_next_known_tech` picks first when nothing is
    /// queued: `Research::set_target`'s own prereq closure (always ascending `TechId`) decides
    /// what's actually researched first regardless, so a branch tech whose prereqs are still
    /// shared-tier techs pulls those in ahead of itself either way.
    fn research_plan(&self, nation: Nation, defs: &Defs) -> Vec<TechId> {
        let mut techs: Vec<(TechId, bool)> =
            defs.techs_for(nation).map(|(id, t)| (id, t.nation.is_none())).collect();
        if self.branch_first {
            // A stable sort keeps each group (branch, then shared) in its own file order.
            techs.sort_by_key(|&(_, shared)| shared);
        }
        techs.into_iter().map(|(id, _)| id).collect()
    }

    /// Issues whatever the opening still wants done, reading `world` fresh. Call this once per
    /// `Opening::due` tick (the same fixed decision points; `Economy` doesn't need its own).
    pub fn step(&self, world: &mut World, defs: &Defs) {
        let anchor = world.settlement_anchor();
        let deposits = deposit_items(defs);
        ensure_stockpile(world, defs, anchor);
        grow_stockpile(world, defs, anchor);
        forage_wild_food(world, defs, anchor);
        gather_materials(world, defs, anchor, &self.build_material_items(world, defs, &deposits));
        self.quarry_and_mine(world, defs, anchor, &deposits);
        recover_scrolls(world, defs, anchor);
        queue_next_known_tech(world, defs, self.research_plan(world.nation, defs).into_iter());
        self.raise_buildings(world, defs, anchor);
        if self.farms {
            self.sow_farm_plots(world, defs);
        }
        let wanted = self.wanted_items(world, defs);
        keep_bends_designated(world, defs, anchor, DEPOSIT_RADIUS, self.people, &wanted);
    }

    /// Items some building `raise_buildings` still wants (its target unmet, its tech known)
    /// costs more of than the settlement holds (#376). A bend that makes one of these
    /// (Metalbend's metal for Earth's Deep Mine, Cut Ice's ice for Water's igloo) goes ahead of
    /// cheaper bends in `keep_bends_designated`, which would otherwise fill every slot with
    /// low-chi bends and never make the item at all.
    fn wanted_items(&self, world: &World, defs: &Defs) -> Vec<ItemId> {
        let mut wanted = Vec::new();
        for building in endeavor::reachable_buildings(world.nation, &defs.techs, &defs.buildings) {
            let def = defs.building(building);
            if def.unlocked_by.is_some_and(|t| !world.research().is_known(t))
                || building_count(world, building) >= self.building_target(world, defs, building)
            {
                continue;
            }
            for c in &def.cost {
                if !wanted.contains(&c.item) && !have_enough(world, std::slice::from_ref(c)) {
                    wanted.push(c.item);
                }
            }
        }
        wanted
    }

    /// Every item any building the nation can reach costs, other than `deposits` (stone and ore
    /// get their own standing harvest via `quarry_and_mine`; gathering them here too would be
    /// harmless but pointless). On the bundled data this is just wood, but nothing here names it.
    fn build_material_items(&self, world: &World, defs: &Defs, deposits: &[ItemId]) -> Vec<ItemId> {
        endeavor::reachable_buildings(world.nation, &defs.techs, &defs.buildings)
            .flat_map(|b| defs.building(b).cost.iter().map(|c: &CostItem| c.item))
            .filter(|item| !deposits.contains(item))
            .collect()
    }

    /// A one-shot harvest on every resource within `DEPOSIT_RADIUS` that yields one of
    /// `deposits` (module doc, "Economy"). Not a standing order: these resources don't regrow,
    /// but calling this every step is harmless either way (`AlreadyDesignated` is ignored).
    fn quarry_and_mine(&self, world: &mut World, defs: &Defs, anchor: Pos, deposits: &[ItemId]) {
        let targets =
            nearby(anchor, DEPOSIT_RADIUS, world, defs, |t, defs| yields_any(t, defs, deposits));
        for pos in targets {
            let _ = world.apply(Command::DesignateHarvest { pos }, defs);
        }
    }

    /// Raises one of every building the nation can reach (`endeavor::reachable_buildings`), each
    /// once its own target count is still unmet and its cost is affordable (`ensure_building`),
    /// with `building_target` giving bed, farm plot and orchard their own counts.
    fn raise_buildings(&self, world: &mut World, defs: &Defs, anchor: Pos) {
        let targets: Vec<BuildingId> =
            endeavor::reachable_buildings(world.nation, &defs.techs, &defs.buildings).collect();
        for building in targets {
            let target = self.building_target(world, defs, building);
            ensure_building(world, defs, anchor, building, target, BUILD_SEARCH_RADIUS);
        }
    }

    /// How many of `building` `Economy` wants standing right now (module doc, "Economy"): a bed
    /// kept one ahead of the living population so growth never stalls on a full house, a farm
    /// plot per `PEOPLE_PER_FARM_PLOT` people (none with `farms` off), an orchard if the nation
    /// has one and `farms` is on, one of everything else. Keyed by the building's own id, not
    /// the nation: every nation's farm plot and bed (if it has one) gets the same treatment.
    fn building_target(&self, world: &World, defs: &Defs, building: BuildingId) -> u32 {
        match defs.building(building).id.as_str() {
            "bed" => world.units.len() as u32 + 1,
            "farm_plot" => {
                if self.farms {
                    self.farm_plot_target(world)
                } else {
                    0
                }
            }
            "orchard" => u32::from(self.farms),
            _ => 1,
        }
    }

    /// One farm plot per `PEOPLE_PER_FARM_PLOT` living settlers, at least 1.
    fn farm_plot_target(&self, world: &World) -> u32 {
        (world.units.len() as u32).div_ceil(PEOPLE_PER_FARM_PLOT).max(1)
    }

    /// Sows every empty farm plot the settlement has built (`Command::Sow`): a plot sits empty
    /// until sown, then grows and is reaped on its own (`world/farm.rs`).
    fn sow_farm_plots(&self, world: &mut World, defs: &Defs) {
        let Some(farm_plot) = defs.building_id("farm_plot") else { return };
        let empty: Vec<Pos> = world
            .buildings()
            .filter(|&(_, id)| id == farm_plot)
            .map(|(footprint, _)| footprint.anchor)
            .filter(|&pos| world.plot_stage(pos) == Some(PlotStage::Empty))
            .collect();
        for pos in empty {
            let _ = world.apply(Command::Sow { pos }, defs);
        }
    }
}

/// The items the sign named `id` in `data/endeavor.toml` counts (a `Stored` or `Tally::Harvested`
/// sign's own `items`), read generically so a new deposit kind or a second ore (#349, #134)
/// needs no change here or in `crate::balance`'s economy report, which reads the same signs for
/// its own columns. Empty if there's no such sign, or its kind doesn't carry an item list.
pub(crate) fn sign_items(defs: &Defs, id: &str) -> Vec<ItemId> {
    defs.endeavor
        .signs
        .iter()
        .find(|s| s.id == id)
        .map(|s| match &s.kind {
            SignKind::Stored { items } => items.clone(),
            SignKind::Tally(Tally::Harvested(items)) => items.clone(),
            _ => Vec::new(),
        })
        .unwrap_or_default()
}

/// The items the "stone" and "ore" signs count between them: `Economy`'s own definition of "a
/// deposit" (module doc, "Economy").
fn deposit_items(defs: &Defs) -> Vec<ItemId> {
    sign_items(defs, "stone").into_iter().chain(sign_items(defs, "ore")).collect()
}

/// Lays a small stockpile near the anchor, once there is none at all.
fn ensure_stockpile(world: &mut World, defs: &Defs, anchor: Pos) {
    if world.stockpile_zones().next().is_some() {
        return;
    }
    let candidates = open_tiles_near(world, defs, anchor, BUILD_SEARCH_RADIUS);
    for pos in candidates.into_iter().take(STOCKPILE_TILES) {
        let _ = world.apply(Command::DesignateStockpile { pos }, defs);
    }
}

/// Grows the stockpile whenever something lying on the ground has nowhere to go: a stockpile
/// tile holds only one item kind until it's emptied (`World::room_at`), and nothing partitions
/// the tiles `Economy` has laid among item kinds, so whichever item is produced fastest can claim
/// every one of them and leave a slower one (stone, behind research and a long haul) stranded on
/// the ground for the length of a run (#378: on one seed, Economy's own untiring wood gathering
/// filled every tile it had, so quarried stone never reached stores at all). A target sized from
/// the item list (`defs.items.len() * STOCKPILE_TILES_PER_ITEM`, #352's original fix) guesses a
/// total that the fastest-growing item can still exhaust before the slowest one is ever stored;
/// reacting to an actual stranding instead means production, not a guess, decides how big the
/// stockpile needs to be, and the moment anything has nowhere to go, the next haul always finds
/// the new tile empty. Not needed by `Opening`, which never produces more than a couple of kinds
/// at once.
fn grow_stockpile(world: &mut World, defs: &Defs, anchor: Pos) {
    let cap = defs.jobs.stockpile_stack;
    let has_room = |item: ItemId, pos: Pos| match world.stored_at(pos) {
        None => true,
        Some(stored) => stored.item == item && stored.count < cap,
    };
    let stranded = world
        .ground_items()
        .any(|(_, stack)| !world.stockpile_zones().any(|pos| has_room(stack.item, pos)));
    if !stranded {
        return;
    }
    let candidates = open_tiles_near(world, defs, anchor, BUILD_SEARCH_RADIUS);
    for pos in candidates.into_iter().take(STOCKPILE_TILES_PER_ITEM as usize) {
        let _ = world.apply(Command::DesignateStockpile { pos }, defs);
    }
}

/// Standing harvest orders on every wild, renewable food resource within `FORAGE_RADIUS`.
/// Harmless to call every step: an already-designated tile just refuses the repeat.
fn forage_wild_food(world: &mut World, defs: &Defs, anchor: Pos) {
    let targets = nearby(anchor, FORAGE_RADIUS, world, defs, is_wild_food);
    for pos in targets {
        let _ = world.apply(Command::DesignateRepeatingHarvest { pos }, defs);
    }
}

/// A one-shot harvest within `FORAGE_RADIUS` on any resource that yields one of `items`.
fn gather_materials(world: &mut World, defs: &Defs, anchor: Pos, items: &[ItemId]) {
    let targets = nearby(anchor, FORAGE_RADIUS, world, defs, |t, defs| yields_any(t, defs, items));
    for pos in targets {
        let _ = world.apply(Command::DesignateHarvest { pos }, defs);
    }
}

/// A one-shot harvest on every surface scroll (`ancient_scroll`, always visible) and every
/// uncovered lost ruin (`lost_ruins`, buried until a Map Room's Chart job or Air's Glide
/// uncovers one) within `SCROLL_RADIUS` tiles (module doc; #404). Both resources' harvests yield
/// the `ancient_scroll` item (`data/jobs.toml`), so one item filter catches both without naming
/// either resource: a later find that also yields scrolls needs no change here. Not a standing
/// order: neither resource regrows. Reachability is left to the job itself (`run_harvest`'s own
/// routing, same as `quarry_and_mine`): a ruin across water is still designated, same as an
/// unreachable one still gets charted, so the player sees it listed rather than silently
/// ignored.
fn recover_scrolls(world: &mut World, defs: &Defs, anchor: Pos) {
    let Some(scroll) = defs.item_id("ancient_scroll") else { return };
    let items = [scroll];
    let targets = nearby(anchor, SCROLL_RADIUS, world, defs, |t, defs| yields_any(t, defs, &items));
    for pos in targets {
        let _ = world.apply(Command::DesignateHarvest { pos }, defs);
    }
}

/// Builds one more `building` if the headcount or sign still wants it (fewer than `target`
/// living or under construction), one at a time (no second site of this kind designated while
/// one is already under way), only once its unlocking tech (if any) is known and its full cost
/// is already in stores or on the ground: the generalization behind `Opening`'s beds and
/// campfire, and `Economy`'s whole-building sweep (#352). A `target` of 0 always skips.
fn ensure_building(
    world: &mut World,
    defs: &Defs,
    anchor: Pos,
    building: BuildingId,
    target: u32,
    search_radius: i32,
) {
    if target == 0 {
        return;
    }
    if let Some(tech) = defs.building(building).unlocked_by
        && !world.research().is_known(tech)
    {
        return;
    }
    if building_count(world, building) >= target {
        return;
    }
    if world.build_sites().any(|(_, id, _)| id == building) {
        return;
    }
    if !have_enough(world, &defs.building(building).cost) {
        return;
    }
    if let Some(pos) = nearest_build_site(world, defs, anchor, building, search_radius) {
        let _ = world.apply(Command::DesignateBuild { pos, building }, defs);
    }
}

/// Queues the first tech in `plan` the nation doesn't already know, once nothing is queued.
/// Skips a tech belonging to another nation's branch, with no `if nation == ..`: `SetResearch`'s
/// own refusal tells it to move on (harmless here since neither caller ever includes one).
fn queue_next_known_tech(world: &mut World, defs: &Defs, plan: impl Iterator<Item = TechId>) {
    if !world.research().queue().is_empty() {
        return;
    }
    for tech in plan {
        if world.research().is_known(tech) {
            continue;
        }
        if world.apply(Command::SetResearch { tech }, defs).is_ok() {
            return;
        }
    }
}

/// Keeps up to `slots` bends designated within `radius` tiles of `anchor` (module doc,
/// "Bending"): while fewer than `slots` are pending, designates the ability the settlement can
/// cast right now (`World::check_ability`) that has a valid target among those tiles, nearest
/// first, leaving `World::apply`'s own validation to decide what counts as valid. Any ability
/// whose output is in `wanted` goes first (`Economy`'s `wanted_items`, #376); failing that, the
/// one cast least often so far, settlement-wide (`Unit::bends_of`, summed): this is what gives a
/// settler a mix of forms instead of one ability spammed forever (#360, `docs/design/bending.md`,
/// "The route: novice, adept, master"), tied by the cheapest `AbilityDef::chi`. Stops for this
/// step once nothing can be added, so a nation with no ability available, or no target anywhere
/// nearby, is left alone rather than looping.
fn keep_bends_designated(
    world: &mut World,
    defs: &Defs,
    anchor: Pos,
    radius: i32,
    slots: u32,
    wanted: &[ItemId],
) {
    let mut abilities: Vec<AbilityId> = (0..defs.abilities.len() as u16)
        .map(AbilityId)
        .filter(|&a| world.check_ability(a, defs).is_ok())
        .collect();
    if abilities.is_empty() {
        return;
    }
    let makes_wanted = |a: AbilityId| match &defs.ability(a).effect {
        Effect::DrawItem { item, .. } => wanted.contains(item),
        Effect::Smelt { metal, .. } => wanted.contains(metal),
        _ => false,
    };
    let cast_so_far = |a: AbilityId| world.units.iter().map(|u| u.bends_of(a)).sum::<u32>();
    abilities.sort_by(|&a, &b| {
        makes_wanted(b)
            .cmp(&makes_wanted(a))
            .then(cast_so_far(a).cmp(&cast_so_far(b)))
            .then(defs.ability(a).chi.total_cmp(&defs.ability(b).chi))
    });
    let positions = positions_near(anchor, radius);
    loop {
        let pending =
            world.bend_designations().filter(|&(_, p)| p.chebyshev(anchor) <= radius).count()
                as u32;
        if pending >= slots {
            return;
        }
        let placed = abilities.iter().any(|&ability| {
            positions.iter().any(|&pos| world.apply(Command::Bend { ability, pos }, defs).is_ok())
        });
        if !placed {
            return;
        }
    }
}

/// Tiles in the box of `radius` tiles around `anchor`, in bounds, whose resource satisfies
/// `want`.
fn nearby(
    anchor: Pos,
    radius: i32,
    world: &World,
    defs: &Defs,
    want: impl Fn(&Tile, &Defs) -> bool,
) -> Vec<Pos> {
    (anchor.y - radius..=anchor.y + radius)
        .flat_map(|y| (anchor.x - radius..=anchor.x + radius).map(move |x| Pos::new(x, y)))
        .filter(|&p| world.map.get(p).is_some_and(|t| want(t, defs)))
        .collect()
}

/// Is the resource on this tile a wild, renewable food (fruit, fish, cabbage): something
/// `Command::DesignateRepeatingHarvest` can stand a standing order on. One-shot harvests (trees,
/// stone, gold) have no `regrow`, and strain never touches anything that isn't food either.
fn is_wild_food(tile: &Tile, defs: &Defs) -> bool {
    let Some(resource) = tile.resource else { return false };
    let Some(harvest) = defs.harvest(resource) else { return false };
    harvest.regrow.is_some() && defs.food_of(harvest.item).is_some()
}

/// Does this tile's resource, once harvested, yield one of `items`?
fn yields_any(tile: &Tile, defs: &Defs, items: &[ItemId]) -> bool {
    let Some(resource) = tile.resource else { return false };
    let Some(harvest) = defs.harvest(resource) else { return false };
    items.contains(&harvest.item)
}

/// Is at least `cost`'s worth of every item already in stockpiles or on the ground: enough that
/// designating the building now wouldn't leave a unit fetching material that isn't there.
fn have_enough(world: &World, cost: &[CostItem]) -> bool {
    cost.iter().all(|c| {
        let ground: u32 =
            world.ground_items().filter(|(_, s)| s.item == c.item).map(|(_, s)| s.count).sum();
        world.stored_total(c.item) + ground >= c.count
    })
}

/// Finished buildings plus unfinished sites of `building`, counted together: a bed being built
/// is as good as claimed, for the opening's purposes.
fn building_count(world: &World, building: BuildingId) -> u32 {
    let built = world.buildings().filter(|&(_, id)| id == building).count();
    let sited = world.build_sites().filter(|&(_, id, _)| id == building).count();
    (built + sited) as u32
}

/// Open, buildable tiles (no resource, no building, not already a site or a stockpile) nearest
/// `anchor`, squared distance first and then by position: the same tie-break
/// `World::found`'s `lay_starting_stores` uses for the same kind of placement. For a stockpile,
/// not a building: unlike `nearest_build_site`, any resource at all rules a tile out, even a
/// clearable one (`docs/design/clearing.md`, issue #425) -- a stockpile tile never clears
/// anything, so `build_blocker`'s build-over allowance doesn't apply here.
fn open_tiles_near(world: &World, defs: &Defs, anchor: Pos, radius: i32) -> Vec<Pos> {
    let mut open: Vec<Pos> = (anchor.y - radius..=anchor.y + radius)
        .flat_map(|y| (anchor.x - radius..=anchor.x + radius).map(move |x| Pos::new(x, y)))
        .filter(|&p| {
            world.build_blocker(p, defs).is_none()
                && world.map.get(p).is_some_and(|t| t.resource.is_none())
        })
        .collect();
    open.sort_by_key(|p| ((p.x - anchor.x).pow(2) + (p.y - anchor.y).pow(2), *p));
    open
}

/// Every tile in the box of `radius` tiles around `anchor`, nearest first (squared distance,
/// then by position, `open_tiles_near`'s own tie-break): candidate bend targets, in the order
/// `Opening::bend` tries them. Unlike `open_tiles_near` this doesn't filter by what's on the
/// tile, since a bend's valid targets (a river bank, a ground stack, open ground) vary by
/// ability and `World::apply`'s own validation is what decides.
fn positions_near(anchor: Pos, radius: i32) -> Vec<Pos> {
    let mut ps: Vec<Pos> = (anchor.y - radius..=anchor.y + radius)
        .flat_map(|y| (anchor.x - radius..=anchor.x + radius).map(move |x| Pos::new(x, y)))
        .collect();
    ps.sort_by_key(|p| ((p.x - anchor.x).pow(2) + (p.y - anchor.y).pow(2), *p));
    ps
}

/// The nearest tile to `anchor` (by `Pos`'s own ordering within each ring, nearest ring first)
/// where `building` could be designated right now, if any within `radius` tiles, and finishing
/// it there wouldn't wall anyone into a dead end (`World::would_strand`, #384: the opening used
/// to pick whatever tile `check_build` allowed, with no regard for what a solid building does to
/// the ground around it, and on one seed that sealed a settler into a mountain pocket with no
/// way back to water) nor sit somewhere nothing can reach to begin with (`World::build_site_viable`,
/// #395: on another seed, the opening designated a bed on a lone Coast tile ringed by Mountain,
/// `check_build` and `would_strand` both happy with it since neither asks whether anyone could
/// ever walk there, and the bed then sat at 0 work forever, with no second one ever tried because
/// `ensure_building` saw a site of that kind already standing).
///
/// Never builds over a resource (`World::to_clear` empty), even a clearable one
/// (`docs/design/clearing.md`, issue #425): `check_build` alone now accepts those, but letting
/// the opening build over trees is a separate tuning question, and this keeps its choices (and
/// the balance harness's numbers) exactly as they were before build-over.
fn nearest_build_site(
    world: &World,
    defs: &Defs,
    anchor: Pos,
    building: BuildingId,
    radius: i32,
) -> Option<Pos> {
    let (w, h) = defs.building(building).footprint;
    let before = world.flood_from_anchor(anchor, defs);
    let ok = |p: Pos| {
        world.check_build(p, building, defs).is_ok()
            && world.to_clear(p, building, defs).is_empty()
            && world.build_site_viable(&before, Footprint::new(p, w, h), defs)
    };
    if ok(anchor) {
        return Some(anchor);
    }
    (1..=radius).find_map(|r| {
        let mut ring: Vec<Pos> = (-r..=r)
            .flat_map(|dy| (-r..=r).map(move |dx| Pos::new(anchor.x + dx, anchor.y + dy)))
            .filter(|&p| p.chebyshev(anchor) == r)
            .filter(|&p| ok(p))
            .collect();
        ring.sort();
        ring.into_iter().next()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::Defs;
    use crate::mapgen::MapGenConfig;
    use crate::nation::Nation;
    use crate::world::World;

    fn world(nation: Nation) -> (World, Defs) {
        let defs = Defs::bundled();
        let size = defs.embark.map_size("small").unwrap();
        let cfg = MapGenConfig {
            seed: 7,
            width: size.width,
            height: size.height,
            ..MapGenConfig::default()
        };
        (World::new(&cfg, nation, 6, &defs), defs)
    }

    #[test]
    fn lays_a_stockpile_and_stands_forage_orders() {
        let (mut world, defs) = world(Nation::Earth);
        let opening = Opening::new(6);
        opening.step(&mut world, &defs);
        assert!(world.stockpile_zones().next().is_some());
        let anchor = world.settlement_anchor();
        let any_forage = (anchor.y - FORAGE_RADIUS..=anchor.y + FORAGE_RADIUS).any(|y| {
            (anchor.x - FORAGE_RADIUS..=anchor.x + FORAGE_RADIUS)
                .any(|x| world.is_repeating_harvest(Pos::new(x, y)))
        });
        assert!(any_forage, "expected at least one standing harvest order near the start");
    }

    #[test]
    fn never_designates_a_bed_it_cant_yet_pay_for() {
        let (mut world, defs) = world(Nation::Water);
        let opening = Opening::new(6);
        let bed = defs.building_id("bed").unwrap();
        for _ in 0..3 {
            opening.step(&mut world, &defs);
            // Any site this opening ever starts must already be fully affordable: it never
            // leaves a unit fetching material that was never there (module doc).
            for (_, id, _) in world.build_sites() {
                if id == bed {
                    assert!(have_enough(&world, &defs.building(bed).cost));
                }
            }
        }
    }

    /// #384: on seed 3, `Economy` once raised a Water Tribe study hall across the one gap out of
    /// a mountain pocket, so whoever sat down to study there had no way back to water once the
    /// gap closed behind them, and died of dehydration. Here, a two-tile-wide dead-end corridor
    /// stands in for that pocket: every 2x2 footprint a study hall could take inside it either
    /// walls off ground further down the corridor, or (at the very bottom) walls off its own
    /// door, since there is nowhere else to stand once it's built. `nearest_build_site` must
    /// refuse every one of them (`World::would_strand`) rather than pick the nearest blindly, so
    /// it comes back with nothing instead of a trap.
    #[test]
    fn nearest_build_site_refuses_every_site_in_a_dead_end_corridor() {
        use crate::defs::{BiomeId, Terrain};
        use crate::geom::Grid;
        use crate::mapgen::Tile;

        let defs = Defs::bundled();
        // Columns 1-2 are the corridor, five rows deep (0-4), flanked by mountain on both sides
        // and sealed at the bottom (row 5): nowhere to go but straight down, and nowhere out
        // once you reach the bottom.
        let rows = ["^..^", "^..^", "^..^", "^..^", "^..^", "^^^^"];
        let map = Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: if rows[p.y as usize].as_bytes()[p.x as usize] == b'^' {
                Terrain::Mountain
            } else {
                Terrain::Land
            },
            biome: BiomeId(0),
            resource: None,
            building: None,
        });
        let anchor = Pos::new(1, 0);
        let world = World::with_map(1, map, Nation::Water, &[anchor], &defs);
        let study_hall = defs.building_id("study_hall").unwrap();

        // A study hall anchored right at the top strands the three rows below it: a real trap,
        // and (unlike the production code) what the old, unchecked pick would have been.
        assert!(world.would_strand(anchor, Footprint::new(anchor, 2, 2), &defs));
        assert_eq!(
            nearest_build_site(&world, &defs, anchor, study_hall, 5),
            None,
            "every site in this corridor strands something; building nowhere is the safe choice"
        );
    }

    /// #395: on seed 3, the economy opening designated a Fire Nation bed on a lone Coast tile
    /// ringed by Mountain, with no way for anyone to ever walk there. `check_build` passed it
    /// (Coast is buildable ground) and so did `would_strand` (nothing reachable today becomes
    /// unreachable by building on a tile nothing could reach in the first place, its own module
    /// doc's point), so the old, unchecked search picked it anyway, and the bed sat at zero work
    /// forever. Here the resource on `anchor` stands for the real settlement's stockpile (also
    /// unbuildable), forcing the search outward; nothing at radius 1 is open ground, and the only
    /// tile that passes `check_build` at radius 2 is the isolated pocket at `(2, 1)`, mountain on
    /// every side. `nearest_build_site` must refuse it and come back with nothing, the same as
    /// the dead-end corridor above, rather than hand back a site nobody can ever reach.
    #[test]
    fn nearest_build_site_refuses_an_unreachable_pocket_even_though_would_strand_passes_it() {
        use crate::defs::{BiomeId, ResourceId, Terrain};
        use crate::geom::Grid;
        use crate::mapgen::Tile;

        let defs = Defs::bundled();
        // `(0, 1)` is the anchor, a resource tile (unbuildable, standing for a stockpile) boxed
        // in by mountain on every side; `(2, 1)` is a second, separate one-tile pocket, also
        // boxed in, two tiles away. Neither can reach the other, or anything else.
        let rows = ["^^^^^", "T^.^^", "^^^^^"];
        let map = Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
            let c = rows[p.y as usize].as_bytes()[p.x as usize];
            Tile {
                elevation: 100,
                temperature: 128,
                moisture: 128,
                terrain: if c == b'^' { Terrain::Mountain } else { Terrain::Land },
                biome: BiomeId(0),
                resource: (c == b'T').then_some(ResourceId(0)),
                building: None,
            }
        });
        let anchor = Pos::new(0, 1);
        let world = World::with_map(1, map, Nation::Fire, &[anchor], &defs);
        let bed = defs.building_id("bed").unwrap();
        let pocket = Pos::new(2, 1);

        // The pocket passes `check_build` (open Land, no resource or building) and `would_strand`
        // too (it was never reachable from `anchor` to begin with, so building there strands
        // nothing): exactly the gap `build_site_viable` has to close.
        assert_eq!(world.check_build(pocket, bed, &defs), Ok(()));
        assert!(!world.would_strand(anchor, Footprint::new(pocket, 1, 1), &defs));
        assert_eq!(
            nearest_build_site(&world, &defs, anchor, bed, 5),
            None,
            "the only candidate within radius is a pocket nothing can walk to; building nowhere \
             is the safe choice"
        );
    }

    #[test]
    fn eventually_builds_beds_once_wood_is_reachable() {
        let (mut world, defs) = world(Nation::Water);
        let opening = Opening::new(6);
        let bed = defs.building_id("bed").unwrap();
        for tick in 0..(30 * 600) {
            if Opening::due(tick) {
                opening.step(&mut world, &defs);
            }
            world.tick(&defs);
        }
        assert!(building_count(&world, bed) > 0, "expected at least one bed after 30 minutes");
    }

    #[test]
    fn queues_fire_making_before_farming_for_a_nation_that_starts_without_it() {
        let (mut world, defs) = world(Nation::Water);
        let opening = Opening::new(6);
        opening.step(&mut world, &defs);
        let fire_making = defs.tech_id("fire_making").unwrap();
        assert!(
            world.research().queue().contains(&fire_making)
                || world.research().is_known(fire_making)
        );
    }

    #[test]
    fn fire_already_knows_fire_making_so_the_opening_queues_farming_next() {
        let (mut world, defs) = world(Nation::Fire);
        let opening = Opening::new(6);
        opening.step(&mut world, &defs);
        let farming = defs.tech_id("farming").unwrap();
        assert!(world.research().queue().contains(&farming));
    }

    #[test]
    fn is_deterministic_given_the_same_world_twice() {
        let (mut a, defs) = world(Nation::Air);
        let (mut b, _) = world(Nation::Air);
        let opening = Opening::new(6);
        for tick in 0..600u64 {
            if Opening::due(tick) {
                opening.step(&mut a, &defs);
                opening.step(&mut b, &defs);
            }
            a.tick(&defs);
            b.tick(&defs);
        }
        assert_eq!(a.command_log(), b.command_log());
    }

    /// Fire's Hearthflame needs no tech, so `with_bending` designates one near the anchor on
    /// the very first step (#343).
    #[test]
    fn with_bending_designates_a_bend_near_the_anchor() {
        let (mut world, defs) = world(Nation::Fire);
        let opening = Opening::with_bending(6, 6);
        opening.step(&mut world, &defs);
        let anchor = world.settlement_anchor();
        assert!(
            world.bend_designations().any(|(_, p)| p.chebyshev(anchor) <= FORAGE_RADIUS),
            "expected a bend designated near the anchor"
        );
    }

    /// Bending is as deterministic as the rest of the opening (module doc, "Bending").
    #[test]
    fn with_bending_is_deterministic_given_the_same_world_twice() {
        let (mut a, defs) = world(Nation::Earth);
        let (mut b, _) = world(Nation::Earth);
        let opening = Opening::with_bending(6, 6);
        for tick in 0..600u64 {
            if Opening::due(tick) {
                opening.step(&mut a, &defs);
                opening.step(&mut b, &defs);
            }
            a.tick(&defs);
            b.tick(&defs);
        }
        assert_eq!(a.command_log(), b.command_log());
    }

    /// `Opening::with_full_research` queues the shared tier and then `earth_shaping`, not just
    /// `RESEARCH_PLAN`'s `fire_making`/`farming` (module doc, "Economy"; #352). `Opening` builds
    /// no study hall on its own, so this leans only on the passive trickle
    /// (`passive_knowledge_per_minute`): slow, but enough within a few game-hours to prove the
    /// plan reaches past the shared tier with no extra code. The limit is 400, not 320, since
    /// #383 added the shared `prospecting` tech (250 points): the whole shared tier is now 1140
    /// points, which takes about 380 minutes at the bare passive trickle alone.
    #[test]
    fn with_full_research_eventually_reaches_the_nations_own_branch() {
        let (mut world, defs) = world(Nation::Earth);
        let opening = Opening::new(6).with_full_research();
        let earth_shaping = defs.tech_id("earth_shaping").unwrap();
        const LIMIT_MINUTES: u64 = 400;
        for tick in 0..(LIMIT_MINUTES * 600) {
            if Opening::due(tick) {
                opening.step(&mut world, &defs);
            }
            world.tick(&defs);
        }
        assert!(
            world.research().is_known(earth_shaping)
                || world.research().queue().contains(&earth_shaping),
            "expected Earth Shaping known or queued after {LIMIT_MINUTES:?} minutes of full research",
        );
    }

    /// `with_branch_first` (#434, `docs/design/pacing.md` "Measuring it"): Earth starts knowing
    /// `stoneworking` (`data/nations.toml`), so its first branch tech (`earth_shaping`) needs no
    /// further shared-tier prereq, and branch-first queues it straight away instead of the
    /// shared tier's own first unknown tech (`fire_making`, which the default order queues
    /// instead: the next test).
    #[test]
    fn with_branch_first_researches_the_nations_branch_before_the_shared_tier() {
        let (mut world, defs) = world(Nation::Earth);
        let economy = Economy::new(6).with_branch_first();
        economy.step(&mut world, &defs);
        let earth_shaping = defs.tech_id("earth_shaping").unwrap();
        let fire_making = defs.tech_id("fire_making").unwrap();
        assert!(world.research().queue().contains(&earth_shaping));
        assert!(!world.research().queue().contains(&fire_making));
    }

    /// Without `with_branch_first`, `Economy`'s default order researches the shared tier first
    /// (module doc, "Economy"): Earth doesn't start knowing `fire_making`, so that's what gets
    /// queued, not `earth_shaping`.
    #[test]
    fn without_branch_first_the_shared_tier_is_researched_before_the_nations_branch() {
        let (mut world, defs) = world(Nation::Earth);
        let economy = Economy::new(6);
        economy.step(&mut world, &defs);
        let earth_shaping = defs.tech_id("earth_shaping").unwrap();
        let fire_making = defs.tech_id("fire_making").unwrap();
        assert!(world.research().queue().contains(&fire_making));
        assert!(!world.research().queue().contains(&earth_shaping));
    }

    /// #376: an unbuilt building's missing cost item is "wanted" once its tech is known, so a
    /// bend that makes it (Cut Ice for the igloo) goes ahead of cheaper bends.
    #[test]
    fn economy_wants_ice_for_the_igloo_only_once_ice_construction_is_known() {
        let (mut world, defs, economy) = economy_opening(Nation::Water);
        let ice = defs.item_id("ice").unwrap();
        assert!(!economy.wanted_items(&world, &defs).contains(&ice));
        world.grant_tech(defs.tech_id("ice_construction").unwrap());
        assert!(economy.wanted_items(&world, &defs).contains(&ice));
    }

    fn economy_opening(nation: Nation) -> (World, Defs, Economy) {
        let (world, defs) = world(nation);
        (world, defs, Economy::new(6))
    }

    /// The same settlement `balance::run_economy` would found on `seed`: `World::found` at
    /// `Survey::default_site`, not the fixed spawn `world` uses (`#378`'s reproduction needed the
    /// same anchor placement the `balance` binary sees).
    fn economy_opening_on_seed(nation: Nation, seed: u64) -> (World, Defs, Economy) {
        use crate::embark::Survey;
        let defs = Defs::bundled();
        let size = defs.embark.map_size("small").unwrap();
        let cfg = MapGenConfig {
            seed,
            width: size.width,
            height: size.height,
            ..MapGenConfig::default()
        };
        let survey = Survey::new(&cfg, &defs);
        let start = survey.default_site(&defs);
        let world = World::found(survey, nation, start, 6, Some("normal".to_owned()), &defs)
            .expect("Survey::default_site always names a site World::found accepts");
        (world, defs, Economy::new(6))
    }

    fn run_economy(world: &mut World, defs: &Defs, economy: &Economy, minutes: u64) {
        for tick in 0..(minutes * 600) {
            if Opening::due(tick) {
                economy.step(world, defs);
            }
            world.tick(defs);
        }
    }

    /// `Economy` is as deterministic as `Opening`, for the same reason (module doc): it reads
    /// only `World` and `Defs`, so two identical worlds given the same steps issue the same
    /// commands.
    #[test]
    fn economy_is_deterministic_given_the_same_world_twice() {
        let (mut a, defs, economy) = economy_opening(Nation::Water);
        let (mut b, _, _) = economy_opening(Nation::Water);
        run_economy(&mut a, &defs, &economy, 60);
        run_economy(&mut b, &defs, &economy, 60);
        assert_eq!(a.command_log(), b.command_log());
    }

    /// Gold needs `prospecting` now (#383, docs/design/materials.md): with a reachable vein
    /// nearby but nothing researched yet, `quarry_and_mine`'s `DesignateHarvest` is refused the
    /// same way it already was for stone before `stoneworking` (seed 7 has gold veins within
    /// `DEPOSIT_RADIUS`, the same map `economy_designates_a_gold_harvest_once_prospecting_is_known`
    /// below uses once the tech is known).
    #[test]
    fn economy_designates_no_gold_harvest_before_prospecting_is_known() {
        let (mut world, defs, economy) = economy_opening(Nation::Fire);
        economy.step(&mut world, &defs);
        let anchor = world.settlement_anchor();
        let ore_items = sign_items(&defs, "ore");
        assert!(
            !world.harvest_designations().any(|p| p.chebyshev(anchor) <= DEPOSIT_RADIUS
                && yields_any(&world.map[p], &defs, &ore_items)),
            "expected no gold harvest designated before prospecting is known"
        );
    }

    /// Once `Economy`'s full-tree research (module doc, "Economy") reaches `prospecting`,
    /// `quarry_and_mine` designates a reachable vein the same way it already does for stone
    /// once `stoneworking` is known (#383; `docs/design/materials.md`'s `balance --economy`
    /// numbers put this around minute 70-80 on this seed, so 150 minutes is ample margin).
    #[test]
    fn economy_designates_a_gold_harvest_once_prospecting_is_known() {
        let (mut world, defs, economy) = economy_opening_on_seed(Nation::Fire, 7);
        run_economy(&mut world, &defs, &economy, 150);
        let prospecting = defs.tech_id("prospecting").unwrap();
        assert!(
            world.research().is_known(prospecting),
            "expected prospecting known within 150 minutes"
        );
        let anchor = world.settlement_anchor();
        let ore_items = sign_items(&defs, "ore");
        assert!(
            world.harvest_designations().any(|p| p.chebyshev(anchor) <= DEPOSIT_RADIUS
                && yields_any(&world.map[p], &defs, &ore_items))
                || ore_items.iter().any(|&i| world.stored_total(i) > 0),
            "expected a gold harvest designated or already mined once prospecting is known"
        );
    }

    /// `Economy` keeps a study hall built once wood is reachable (module doc, "Economy"): with
    /// nothing queued, no scholar would ever claim a seat either (`world/study.rs`), but
    /// `Economy::step`'s own research call keeps something queued throughout.
    #[test]
    fn economy_eventually_builds_a_study_hall() {
        let (mut world, defs, economy) = economy_opening(Nation::Air);
        let study_hall = defs.building_id("study_hall").unwrap();
        run_economy(&mut world, &defs, &economy, 60);
        assert!(building_count(&world, study_hall) > 0, "expected a study hall within an hour");
    }

    /// `Economy` tills and sows a farm plot once farming is known (module doc, "Economy"):
    /// `PlotStage::Empty` only while unsown.
    #[test]
    fn economy_tills_and_sows_a_farm_plot() {
        let (mut world, defs, economy) = economy_opening(Nation::Water);
        let farm_plot = defs.building_id("farm_plot").unwrap();
        run_economy(&mut world, &defs, &economy, 90);
        let sown = world
            .buildings()
            .filter(|&(_, id)| id == farm_plot)
            .any(|(footprint, _)| world.plot_stage(footprint.anchor) != Some(PlotStage::Empty));
        assert!(sown, "expected at least one sown farm plot within 90 minutes");
    }

    /// `#378`: on seed 42, every nation stored 0 stone for 180 minutes under `--economy`, even
    /// though the supply survey finds 30 reachable outcrops (worth 300 stone) within
    /// `DEPOSIT_RADIUS`. The cause was `grow_stockpile`'s flat per-item target: wood, gathered
    /// for every building's cost, filled every tile `Economy` ever laid before stone (gated
    /// behind research, and farther out) got a single one, so it sat on the ground unhauled for
    /// the whole run. `grow_stockpile` now lays more room the moment anything is stranded,
    /// instead of guessing a fixed total from the item list.
    #[test]
    fn economy_stores_stone_on_seed_42() {
        let (mut world, defs, economy) = economy_opening_on_seed(Nation::Earth, 42);
        let stone_items = sign_items(&defs, "stone");
        run_economy(&mut world, &defs, &economy, 150);
        let stored: u32 = stone_items.iter().map(|&i| world.stored_total(i)).sum();
        assert!(stored > 0, "expected some stone stored within 150 minutes on seed 42");
    }

    /// #395: on seed 3, `Economy` never raised a Fire Nation bed, keeping the population stuck
    /// at six: `nearest_build_site` picked a lone Coast tile diagonally across from the anchor,
    /// cut off from it by Mountain on both sides of the corner (no-corner-cutting, `path.rs`'s
    /// module doc). `check_build` passed it (Coast is buildable ground) and so did the old
    /// `would_strand` (nothing reachable today becomes unreachable by building on a tile nothing
    /// could reach to begin with, its own module doc's point), so the unchecked search picked it
    /// anyway, and the bed sat at zero work forever with no second one ever tried
    /// (`ensure_building` saw a site of that kind already standing). `build_site_viable` closes
    /// that gap; this seed is the regression it was found on.
    #[test]
    fn economy_raises_a_bed_on_seed_3() {
        let (mut world, defs, economy) = economy_opening_on_seed(Nation::Fire, 3);
        let bed = defs.building_id("bed").unwrap();
        run_economy(&mut world, &defs, &economy, 30);
        assert!(
            world.buildings().any(|(_, id)| id == bed),
            "expected a finished bed within 30 minutes on seed 3"
        );
    }

    /// `Economy::without_farms` tills nothing (`docs/design/materials.md`'s `--no-farms`, owner
    /// question 2's check), even once farming is known and wood is plentiful.
    #[test]
    fn economy_without_farms_builds_no_farm_plot() {
        let (mut world, defs) = world(Nation::Water);
        let economy = Economy::without_farms(6);
        let farm_plot = defs.building_id("farm_plot").unwrap();
        run_economy(&mut world, &defs, &economy, 90);
        assert_eq!(building_count(&world, farm_plot), 0);
    }

    /// `Economy` keeps a bed ahead of the living population instead of a fixed headcount
    /// (module doc, "Economy"): with no growth in a short run the target is just one more than
    /// today's six, and `ensure_building`'s own "one at a time" rule still applies.
    #[test]
    fn economy_keeps_a_bed_ahead_of_the_population() {
        let (mut world, defs, economy) = economy_opening(Nation::Fire);
        let bed = defs.building_id("bed").unwrap();
        run_economy(&mut world, &defs, &economy, 60);
        assert!(building_count(&world, bed) >= 1, "expected at least one bed within an hour");
        assert!(
            building_count(&world, bed) <= world.units.len() as u32 + 1,
            "never more beds than the population needs"
        );
    }
}

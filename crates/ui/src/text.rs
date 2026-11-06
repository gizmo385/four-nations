//! How things in the world read in English: what a unit is doing, what a stack holds.

use fn_sim::jobs::{JobKind, Stack};
use fn_sim::world::{FoodSource, IdleReason, Task, Unit};
use fn_sim::{Defs, Pos, World};

/// "3 Wood", or "nothing".
pub fn describe_stack(defs: &Defs, stack: Option<Stack>) -> String {
    match stack {
        Some(s) => format!("{} {}", s.count, defs.item(s.item).name),
        None => "nothing".into(),
    }
}

/// "34 tiles north-west" from `from` to `to`: Chebyshev distance (the same measure a Map Room's
/// `range` uses, `docs/design/cartography.md`) plus one of the eight compass points, or just
/// "here" at distance 0. Shared by the Endeavor screen's found-scrolls list (#380) and, later, a
/// finished chart's own notice (#403), so the two phrases never drift apart.
pub fn compass_distance(from: Pos, to: Pos) -> String {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let distance = from.chebyshev(to);
    if dx == 0 && dy == 0 {
        return "here".to_owned();
    }
    let ns = match dy.signum() {
        -1 => "north",
        1 => "south",
        _ => "",
    };
    let ew = match dx.signum() {
        -1 => "west",
        1 => "east",
        _ => "",
    };
    let direction = match (ns, ew) {
        ("", ew) => ew.to_owned(),
        (ns, "") => ns.to_owned(),
        (ns, ew) => format!("{ns}-{ew}"),
    };
    format!("{distance} tiles {direction}")
}

/// A crafter's task: "Crafting at (x, y)", or, once its recipe's `stock_limit` is reached with
/// no batch under way (`World::craft_idle`, issue #350), "<Building> idle: n/limit <item>
/// stored" so the player can see why it stopped.
fn describe_craft(world: &World, defs: &Defs, unit: &Unit, pos: Pos) -> String {
    let fallback = || format!("Crafting at ({}, {})", pos.x, pos.y);
    let Some(building) = world.building_at(pos) else { return fallback() };
    let def = defs.building(building);
    let Some(recipe) = &def.recipe else { return fallback() };
    if !world.craft_idle(pos, recipe) {
        return fallback();
    }
    let stored = world.stored_total(recipe.output.item);
    format!(
        "{} idle: {}/{} {} stored",
        def.display_name(world.nation_of(unit)),
        stored,
        recipe.stock_limit,
        defs.item(recipe.output.item).name
    )
}

/// The selection bar's bends line (`docs/design/endgame.md`, "Mastery pacing", #346): "Bends 7
/// of 10 to master" below the threshold, "Bending master - 12 bends" once it's met, or just
/// "Bends 7" when the data has no masters sign at all (`threshold` is `None`,
/// `fn_sim::endeavor::EndeavorDef::master_threshold`).
pub fn bends_line(bends: u32, threshold: Option<u32>) -> String {
    match threshold {
        Some(t) if bends >= t => format!("Bending master - {bends} bends"),
        Some(t) => format!("Bends {bends} of {t} to master"),
        None => format!("Bends {bends}"),
    }
}

/// The Nation screen's Bends column (`docs/design/endgame.md`, "Mastery pacing", #346): "7/10"
/// below the threshold, "12 *" once it's met, or the bare count with no masters sign.
pub fn bends_cell(bends: u32, threshold: Option<u32>) -> String {
    match threshold {
        Some(t) if bends >= t => format!("{bends} *"),
        Some(t) => format!("{bends}/{t}"),
        None => bends.to_string(),
    }
}

/// "Idle" with nothing designated for anyone at all, else why this particular unit has nothing
/// to do right now (`World::idle_reason`, #337): "Idle: no reachable job", "Idle: out of chi",
/// "Idle: stockpile full", "Idle: too hurt for heavy work" (#388). A unit genuinely idle for a
/// moment too brief to explain (`None`, the odd tick between a job becoming claimable and
/// `claim_job` actually taking it) reads the same bare "Idle".
fn describe_idle(world: &World, defs: &Defs, unit: &Unit) -> String {
    match world.idle_reason(unit.id, defs) {
        None | Some(IdleReason::NoWork) => "Idle".to_owned(),
        Some(IdleReason::Unreachable) => "Idle: no reachable job".to_owned(),
        Some(IdleReason::OutOfChi) => "Idle: out of chi".to_owned(),
        Some(IdleReason::StockpileFull) => "Idle: stockpile full".to_owned(),
        Some(IdleReason::TooHurt) => "Idle: too hurt for heavy work".to_owned(),
    }
}

/// What the unit is doing, in a short phrase for the info panel.
pub fn describe_task(world: &World, defs: &Defs, unit: &Unit) -> String {
    let Some(task) = unit.task() else { return describe_idle(world, defs, unit) };
    match task {
        Task::Harvest(id) => {
            let Some(job) = world.job(id) else { return "Harvesting".into() };
            let JobKind::Harvest { pos } = job.kind else { return "Harvesting".into() };
            let resource = world.map.get(pos).and_then(|t| t.resource);
            match resource.and_then(|r| Some((defs.harvest(r)?, defs.resource(r)))) {
                Some((h, r)) => format!(
                    "{} {} at ({}, {}), {:.0}% done",
                    h.verb,
                    r.name,
                    pos.x,
                    pos.y,
                    (job.progress / h.work * 100.0).clamp(0.0, 100.0)
                ),
                None => format!("Harvesting at ({}, {})", pos.x, pos.y),
            }
        }
        Task::Clear(id) => {
            let Some(job) = world.job(id) else { return "Clearing".into() };
            let JobKind::Clear { pos } = job.kind else { return "Clearing".into() };
            let resource = world.map.get(pos).and_then(|t| t.resource);
            match resource.and_then(|r| Some((defs.harvest(r)?, defs.resource(r)))) {
                Some((h, r)) => {
                    let total = h.clear.map_or(h.work, |c| c.work);
                    format!(
                        "Clearing {} at ({}, {}), {:.0}% done",
                        r.name,
                        pos.x,
                        pos.y,
                        (job.progress / total * 100.0).clamp(0.0, 100.0)
                    )
                }
                None => format!("Clearing at ({}, {})", pos.x, pos.y),
            }
        }
        Task::Haul(id) => {
            let at = world.job(id).and_then(|j| match j.kind {
                JobKind::Haul { pos } => Some(pos),
                JobKind::Harvest { .. }
                | JobKind::Bend { .. }
                | JobKind::Build { .. }
                | JobKind::Study { .. }
                | JobKind::EndeavorTask { .. }
                | JobKind::Attend { .. } => None,
                JobKind::Craft { .. } => None,
                JobKind::Sow { .. } | JobKind::Reap { .. } => None,
                JobKind::Clear { .. } => None,
                JobKind::Chart { .. } => None,
            });
            let stack = at.and_then(|p| world.ground_items().find(|(q, _)| *q == p));
            match (stack, at) {
                (Some((p, s)), _) => {
                    format!("Fetching {} from ({}, {})", describe_stack(defs, Some(s)), p.x, p.y)
                }
                _ => "Fetching items".into(),
            }
        }
        Task::Bend(id) => match world.job(id).map(|j| (j.kind, j.progress)) {
            Some((JobKind::Bend { ability, pos }, progress)) => {
                let def = defs.ability(ability);
                format!(
                    "{} at ({}, {}), {:.0}% done",
                    def.verb,
                    pos.x,
                    pos.y,
                    (progress / def.work * 100.0).clamp(0.0, 100.0)
                )
            }
            _ => "Bending".into(),
        },
        // Minimal text; a build menu and site display are client work for a later issue.
        Task::Build(id) => match world.job(id).map(|j| j.kind) {
            Some(JobKind::Build { pos }) => format!("Building at ({}, {})", pos.x, pos.y),
            _ => "Building".into(),
        },
        Task::Study(id) => match world.job(id).map(|j| j.kind) {
            Some(JobKind::Study { pos }) => format!("Studying at ({}, {})", pos.x, pos.y),
            _ => "Studying".into(),
        },
        // Minimal text; a chart-progress display is client work for a later issue (#403).
        Task::Chart(id) => match world.job(id).map(|j| j.kind) {
            Some(JobKind::Chart { pos }) => format!("Charting at ({}, {})", pos.x, pos.y),
            _ => "Charting".into(),
        },
        // Minimal text; a recipe display is client work for a later issue (#351).
        Task::Craft(id) => match world.job(id).map(|j| j.kind) {
            Some(JobKind::Craft { pos }) => describe_craft(world, defs, unit, pos),
            _ => "Crafting".into(),
        },
        Task::Endeavor(id) => match world.job(id).map(|j| (j.kind, j.progress)) {
            Some((JobKind::EndeavorTask { task }, progress)) => match world.gate_task(task, defs) {
                Some((def, _)) => format!(
                    "Raising the {}, {:.0}% done",
                    def.name,
                    (progress / def.work * 100.0).clamp(0.0, 100.0)
                ),
                None => "Raising the gate".into(),
            },
            _ => "Raising the gate".into(),
        },
        Task::Attend(id) => match world.job(id).map(|j| j.kind) {
            Some(JobKind::Attend { pos }) => {
                format!("Attending the Harmony Rite at ({}, {})", pos.x, pos.y)
            }
            _ => "Attending the Harmony Rite".into(),
        },
        Task::Rite => "Meditating in the Avatar State".into(),
        Task::Sow(id) => match world.job(id).map(|j| j.kind) {
            Some(JobKind::Sow { pos }) => format!("Sowing at ({}, {})", pos.x, pos.y),
            _ => "Sowing".into(),
        },
        Task::Reap(id) => match world.job(id).map(|j| j.kind) {
            Some(JobKind::Reap { pos }) => format!("Reaping at ({}, {})", pos.x, pos.y),
            _ => "Reaping".into(),
        },
        Task::Deliver => {
            format!("Delivering {} to a stockpile", describe_stack(defs, unit.carrying))
        }
        Task::Eat(source) => match source {
            FoodSource::Carried => "Eating what it carries".into(),
            FoodSource::Stockpile(p) => format!("Eating from the stockpile at ({}, {})", p.x, p.y),
            FoodSource::Ground(p) => format!("Eating from the ground at ({}, {})", p.x, p.y),
            FoodSource::Forage(p) => format!("Foraging at ({}, {})", p.x, p.y),
        },
        Task::Drink => "Drinking".into(),
        Task::Sleep => "Sleeping".into(),
        Task::Healing(anchor) => match world.building_at(anchor) {
            Some(id) => {
                format!("Healing in the {}", defs.building(id).display_name(world.nation_of(unit)))
            }
            None => "Healing".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use fn_sim::world::Command;

    use super::*;
    use crate::session::Session;

    /// The closest resource to the first unit whose harvest is used up when worked (a tree, not
    /// a fishing spot) and that anyone can work.
    fn first_tree(s: &Session) -> fn_sim::Pos {
        let home = s.world.units[0].pos;
        s.world
            .map
            .iter()
            .filter(|(_, t)| {
                t.resource
                    .and_then(|r| s.defs.harvest(r))
                    .is_some_and(|h| h.regrow.is_none() && h.unlocked_by.is_none())
            })
            .map(|(p, _)| p)
            .min_by_key(|p| (p.x - home.x).abs().max((p.y - home.y).abs()))
            .expect("a tree on the map")
    }

    #[test]
    fn a_harvester_names_its_work_and_idle_units_say_so() {
        let mut s = Session::new(1, Nation::Earth, 48, 32);
        let pos = first_tree(&s);
        assert!(s.world.units.iter().all(|u| describe_task(&s.world, &s.defs, u) == "Idle"));
        s.apply(Command::DesignateHarvest { pos }).unwrap();
        s.tick();
        let busy = s
            .world
            .units
            .iter()
            .map(|u| describe_task(&s.world, &s.defs, u))
            .find(|d| d != "Idle")
            .expect("a unit takes the job");
        assert!(busy.contains(&format!("({}, {})", pos.x, pos.y)), "{busy}");
    }

    /// `describe_task` names why an idle unit is idle (#337), not just a bare "Idle": a bend
    /// its own nation can cast but can't afford yet reads "Idle: out of chi".
    #[test]
    fn an_idle_unit_names_a_bend_it_cannot_yet_afford() {
        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let hearthflame = s.defs.ability_id("hearthflame").unwrap();
        assert!(s.defs.ability(hearthflame).chi > 10.0, "needs headroom below the cost");
        s.world.units[0].chi = 10.0;
        let pos = s.world.units[0].pos;
        s.apply(Command::Bend { ability: hearthflame, pos }).unwrap();
        assert_eq!(describe_task(&s.world, &s.defs, &s.world.units[0]), "Idle: out of chi");
    }

    /// A haul with nowhere to go because the only stockpile tile already holds a different item
    /// reads "Idle: stockpile full" (#337, the playtest evidence that made this the issue's
    /// highest-value fix): stone quarried first claims the settlement's one stockpile tile, so
    /// wood chopped afterward sits on the ground instead.
    #[test]
    fn an_idle_unit_names_a_full_stockpile_blocking_a_haul() {
        use fn_sim::defs::BiomeId;
        use fn_sim::{Grid, Nation, Pos, Terrain, Tile, World};
        let defs = Defs::bundled();
        let stone = defs.resource_id("stone").unwrap();
        let oak = defs.resource_id("oak_tree").unwrap();
        let wood_item = defs.item_id("wood").unwrap();
        let stone_item = defs.item_id("stone").unwrap();
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
        let unit = s.world.units.iter().find(|u| u.task().is_none()).expect("one stays idle");
        assert_eq!(describe_task(&s.world, &s.defs, unit), "Idle: stockpile full");
    }

    #[test]
    fn stacks_read_naturally() {
        let defs = Defs::bundled();
        let wood = defs.item_id("wood").unwrap();
        assert_eq!(describe_stack(&defs, Some(Stack { item: wood, count: 3 })), "3 Wood");
        assert_eq!(describe_stack(&defs, None), "nothing");
    }

    /// A hurt settler resting in the Healing Hut (#375) reads "Healing in the Healing Hut", the
    /// building's own display name, not a bare "Healing".
    #[test]
    fn a_healing_unit_names_the_hut_it_heals_in() {
        use fn_sim::Terrain;
        use fn_sim::building::Building;
        use fn_sim::geom::Footprint;

        let mut s = Session::new(7, Nation::Water, 48, 32);
        let hut = s.defs.building_id("healing_hut").unwrap();
        let passable = s.defs.building(hut).passable;
        let at = fn_sim::Pos::new(20, 20);
        for dy in 0..2 {
            for dx in 0..2 {
                let t = s.world.map.get_mut(fn_sim::Pos::new(at.x + dx, at.y + dy)).unwrap();
                t.terrain = Terrain::Land;
                t.resource = None;
            }
        }
        let foot = Footprint::new(at, 2, 2);
        let door = foot.entrance();
        s.world.units.retain(|u| !foot.contains(u.pos) && u.pos != door);
        for p in foot.tiles() {
            let part = [(p.x - at.x) as u8, (p.y - at.y) as u8];
            let b = Building { part, size: [2, 2], ..Building::new(hut, passable, false) };
            s.world.map.get_mut(p).unwrap().building = Some(b);
        }
        s.world.units[0].pos = door;
        s.world.units[0].needs.health = 50.0;
        for _ in 0..200 {
            s.tick();
            if s.world.units[0].is_healing() {
                break;
            }
        }
        assert!(s.world.units[0].is_healing(), "never made it inside");
        assert_eq!(
            describe_task(&s.world, &s.defs, &s.world.units[0]),
            "Healing in the Healing Hut"
        );
    }

    /// `compass_distance` reads all eight points plus "here", and measures Chebyshev (a
    /// diagonal step counts the same as a straight one, like a Map Room's own `range`).
    #[test]
    fn compass_distance_names_all_eight_points_and_here() {
        let home = fn_sim::Pos::new(50, 50);
        assert_eq!(compass_distance(home, fn_sim::Pos::new(50, 16)), "34 tiles north");
        assert_eq!(compass_distance(home, fn_sim::Pos::new(84, 50)), "34 tiles east");
        assert_eq!(compass_distance(home, fn_sim::Pos::new(50, 84)), "34 tiles south");
        assert_eq!(compass_distance(home, fn_sim::Pos::new(16, 50)), "34 tiles west");
        assert_eq!(compass_distance(home, fn_sim::Pos::new(84, 16)), "34 tiles north-east");
        assert_eq!(compass_distance(home, fn_sim::Pos::new(84, 84)), "34 tiles south-east");
        assert_eq!(compass_distance(home, fn_sim::Pos::new(16, 84)), "34 tiles south-west");
        assert_eq!(compass_distance(home, fn_sim::Pos::new(16, 16)), "34 tiles north-west");
        assert_eq!(compass_distance(home, home), "here");
        // Chebyshev, not Euclidean: 34 tiles east and 10 tiles south is still 34 away, not ~35.4.
        assert_eq!(compass_distance(home, fn_sim::Pos::new(84, 60)), "34 tiles south-east");
    }

    /// The selection bar's bends line (`docs/design/endgame.md`, "Mastery pacing"): below the
    /// threshold, once it's met, and with no masters sign at all.
    #[test]
    fn the_bends_line_reads_the_three_mockup_forms() {
        assert_eq!(bends_line(7, Some(10)), "Bends 7 of 10 to master");
        assert_eq!(bends_line(12, Some(10)), "Bending master - 12 bends");
        assert_eq!(bends_line(10, Some(10)), "Bending master - 10 bends", "exactly met counts");
        assert_eq!(bends_line(7, None), "Bends 7");
    }

    /// The Nation screen's Bends column, same three cases as the line.
    #[test]
    fn the_bends_cell_reads_the_three_mockup_forms() {
        assert_eq!(bends_cell(7, Some(10)), "7/10");
        assert_eq!(bends_cell(12, Some(10)), "12 *");
        assert_eq!(bends_cell(10, Some(10)), "10 *", "exactly met counts");
        assert_eq!(bends_cell(7, None), "7");
    }
}

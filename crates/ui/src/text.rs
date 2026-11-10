//! How things in the world read in English: what a unit is doing, what a stack holds.

use fn_sim::bending::Rank;
use fn_sim::jobs::{JobKind, Stack};
use fn_sim::world::{FoodSource, IdleReason, MasteryProgress, Task, Unit};
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

/// The selection bar's fourth row (`docs/design/bending.md`, "Seeing it", #361): progress toward
/// the *next* rank ("Firebender: 9 of 10 bends, 2 of 3 forms to master"), reading `progress`'s
/// own `next` (`World::rank_of`'s "what the next rank needs"); once there's no rank left to
/// reach, the master's own line ("Master: 12 bends in 4 forms. Bends cost 80% chi") -- deliberately
/// the bare word "Master", not this nation's own title, since the border already carries that
/// (`title`, used only for the rank the settler holds right now, including a novice's). A novice
/// reads the same shape as an adept, just naming "adept" as the goal instead of "master": the
/// mockup only shows the two upper ranks, but the wording generalises.
pub fn mastery_line(progress: MasteryProgress, title: &str, master_chi: f32) -> String {
    match progress.next {
        Some((next_rank, bends, forms)) => {
            let goal = match next_rank {
                Rank::Adept => "adept",
                Rank::Master => "master",
                Rank::Novice => unreachable!("nobody's next rank is novice"),
            };
            format!(
                "{title}: {} of {bends} bends, {} of {forms} forms to {goal}",
                progress.bends, progress.forms
            )
        }
        None => format!(
            "Master: {} bends in {} forms. Bends cost {:.0}% chi",
            progress.bends,
            progress.forms,
            master_chi * 100.0
        ),
    }
}

/// The Nation screen's Bends and Forms columns (`docs/design/bending.md`, "Seeing it", #346,
/// #361): "9/10" and "2/3" against this nation's own Master thresholds (`master_bends`,
/// `master_forms`, `fn_sim::bending::MasteryDef::master_bends_for`/`master_forms`) below
/// mastery, or "{n} *" in both once `progress.rank` is `Rank::Master` -- unlike the selection
/// bar's own line, every row compares to the *same* pair, since a table's columns need one
/// consistent meaning down the whole list, not each settler's own next rank.
pub fn mastery_cells(
    progress: MasteryProgress,
    master_bends: u32,
    master_forms: u32,
) -> (String, String) {
    if progress.rank == Rank::Master {
        (format!("{} *", progress.bends), format!("{} *", progress.forms))
    } else {
        (format!("{}/{master_bends}", progress.bends), format!("{}/{master_forms}", progress.forms))
    }
}

/// "a" or "an" by `word`'s first letter (A, E, I, O or U reads "an"): good enough for every
/// title in `data/bending.toml`'s `[mastery.titles]` ("a Firebender", "an Earthbender", "an
/// Airbending Master"), which is the only place this is used (#361). Not a real pronunciation
/// check -- content stays within what this covers.
pub fn article(word: &str) -> &'static str {
    match word.chars().next().map(|c| c.to_ascii_lowercase()) {
        Some('a' | 'e' | 'i' | 'o' | 'u') => "an",
        _ => "a",
    }
}

/// The bend picker's own count of who could cast a rank-gated ability right now (#361,
/// `docs/design/bending.md` "Seeing it"): "2 adepts", "no master", or, for an ability with no
/// rank at all, the plain "{n} bender(s)" #248 already gave it. `count` is `World::caster_count`,
/// which already reads `can_cast`'s own rank check, so a rank-gated ability's count here already
/// means "holds the rank", not merely "of the right nation".
pub fn caster_label(rank: Option<Rank>, count: usize) -> String {
    match rank {
        None => format!("{count} {}", if count == 1 { "bender" } else { "benders" }),
        Some(rank) => {
            let word = match rank {
                Rank::Adept => "adept",
                Rank::Master => "master",
                Rank::Novice => unreachable!("no ability is gated on novice"),
            };
            if count == 0 {
                format!("no {word}")
            } else {
                format!("{count} {word}{}", if count == 1 { "" } else { "s" })
            }
        }
    }
}

/// "Idle" with nothing designated for anyone at all, else why this particular unit has nothing
/// to do right now (`World::idle_reason`, #337): "Idle: no reachable job", "Idle: out of chi",
/// "Idle: stockpile full", "Idle: too hurt for heavy work" (#388). A unit genuinely idle for a
/// moment too brief to explain (`None`, the odd tick between a job becoming claimable and
/// `claim_job` actually taking it) reads the same bare "Idle". One that can reach no water, no
/// stockpile and not the settlement's heart reads "Idle: walled in" ahead of all of those
/// (`World::walled_in`, #583), since "no reachable job" would hide that it is cut off from
/// everything.
fn describe_idle(world: &World, defs: &Defs, unit: &Unit) -> String {
    if world.walled_in(unit.id) {
        return "Idle: walled in".to_owned();
    }
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
        Task::Remedy(pos) => match world.stored_at(pos) {
            Some(s) => format!("Taking {}", defs.item(s.item).name.to_lowercase()),
            None => "Taking a remedy".into(),
        },
    }
}

/// A power boost as the player reads it (#452): two decimal places with trailing zeros dropped,
/// so 1.25 is "1.25x", 1.5 is "1.5x" and 2.0 is "2x". One decimal would print the Windmill's
/// 1.25 and a 1.2 source alike as "1.2x".
pub fn boost_label(boost: f32) -> String {
    let s = format!("{boost:.2}");
    format!("{}x", s.trim_end_matches('0').trim_end_matches('.'))
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
    fn boosts_print_two_places_with_trailing_zeros_dropped() {
        assert_eq!(boost_label(1.25), "1.25x");
        assert_eq!(boost_label(1.2), "1.2x");
        assert_eq!(boost_label(1.5), "1.5x");
        assert_eq!(boost_label(2.0), "2x");
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
        use fn_sim::geom::Footprint;

        let mut s = Session::new(7, Nation::Water, 48, 32);
        let hut = s.defs.building_id("healing_hut").unwrap();
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
        s.world.put_building(foot, hut, &s.defs);
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

    /// The selection bar's fourth row (`docs/design/bending.md` "Seeing it"): an adept's own
    /// progress to master, a master's own line, and a novice's progress to adept.
    #[test]
    fn the_mastery_line_reads_the_mockups_two_forms_and_generalises_to_a_novice() {
        let adept = MasteryProgress {
            rank: Rank::Adept,
            bends: 9,
            forms: 2,
            next: Some((Rank::Master, 10, 3)),
        };
        assert_eq!(
            mastery_line(adept, "Firebender", 0.8),
            "Firebender: 9 of 10 bends, 2 of 3 forms to master"
        );
        let master = MasteryProgress { rank: Rank::Master, bends: 12, forms: 4, next: None };
        assert_eq!(
            mastery_line(master, "Firebending Master", 0.8),
            "Master: 12 bends in 4 forms. Bends cost 80% chi"
        );
        let novice = MasteryProgress {
            rank: Rank::Novice,
            bends: 2,
            forms: 1,
            next: Some((Rank::Adept, 5, 2)),
        };
        assert_eq!(
            mastery_line(novice, "Student", 0.8),
            "Student: 2 of 5 bends, 1 of 2 forms to adept"
        );
    }

    /// The Nation screen's Bends and Forms columns, below and at mastery.
    #[test]
    fn the_mastery_cells_read_below_and_at_mastery() {
        let adept = MasteryProgress {
            rank: Rank::Adept,
            bends: 9,
            forms: 2,
            next: Some((Rank::Master, 10, 3)),
        };
        assert_eq!(mastery_cells(adept, 10, 3), ("9/10".to_owned(), "2/3".to_owned()));
        let master = MasteryProgress { rank: Rank::Master, bends: 12, forms: 4, next: None };
        assert_eq!(mastery_cells(master, 10, 3), ("12 *".to_owned(), "4 *".to_owned()));
    }

    /// `article` by first letter, covering every title in `data/bending.toml`.
    #[test]
    fn article_picks_a_or_an_by_first_letter() {
        for title in ["Student", "Firebender", "Firebending Master", "Waterbender"] {
            assert_eq!(article(title), "a", "{title}");
        }
        for title in ["Initiate", "Airbender", "Airbending Master", "Earthbender"] {
            assert_eq!(article(title), "an", "{title}");
        }
    }

    /// The bend picker's own count: plain benders with no rank gate, and "no"/"n adept(s)"/
    /// "n master(s)" for a rank-gated one, singular and plural.
    #[test]
    fn caster_label_reads_rank_and_plain_benders() {
        assert_eq!(caster_label(None, 1), "1 bender");
        assert_eq!(caster_label(None, 2), "2 benders");
        assert_eq!(caster_label(Some(Rank::Adept), 0), "no adept");
        assert_eq!(caster_label(Some(Rank::Adept), 1), "1 adept");
        assert_eq!(caster_label(Some(Rank::Adept), 2), "2 adepts");
        assert_eq!(caster_label(Some(Rank::Master), 0), "no master");
        assert_eq!(caster_label(Some(Rank::Master), 1), "1 master");
        assert_eq!(caster_label(Some(Rank::Master), 3), "3 masters");
    }
}

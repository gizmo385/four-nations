use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

/// A map from ASCII rows: `.` land, `~` water, `,` coast, `^` mountain, `=` ford, `t` tunnel, `T`
/// land with a resource on it.
fn arena(rows: &[&str]) -> TileMap {
    Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
        let c = rows[p.y as usize].as_bytes()[p.x as usize];
        let terrain = match c {
            b'~' => Terrain::Water,
            b',' => Terrain::Coast,
            b'^' => Terrain::Mountain,
            b'=' => Terrain::Ford,
            b't' => Terrain::Tunnel,
            _ => Terrain::Land,
        };
        let resource = (c == b'T').then_some(crate::defs::ResourceId(0));
        Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain,
            biome: BiomeId(0),
            resource,
            building: None,
        }
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn setup(rows: &[&str], units: &[Pos]) -> (World, Defs) {
    setup_as(Nation::Earth, rows, units)
}

fn setup_as(nation: Nation, rows: &[&str], units: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    let map = arena(rows);
    (World::with_map(1, map, nation, units, &defs), defs)
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for n in 0..limit {
        if done(w) {
            return n;
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

const GRID: [&str; 5] = [".........", ".........", ".........", ".........", "........."];

#[test]
fn designating_is_checked() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let bed = defs.building_id("bed").unwrap();
    assert_eq!(
        w.designate_build(at(99, 0), bed, &defs),
        Err(BuildError::CannotBuild { at: at(99, 0), why: Blocker::OffMap })
    );
    assert_eq!(
        w.designate_build(at(2, 2), BuildingId(999), &defs),
        Err(BuildError::UnknownBuilding)
    );
    assert!(w.designate_build(at(2, 2), bed, &defs).is_ok());
    assert_eq!(
        w.designate_build(at(2, 2), bed, &defs),
        Err(BuildError::CannotBuild { at: at(2, 2), why: Blocker::Site })
    );
    assert_eq!(w.cancel_build(at(9, 9), &defs), Err(BuildError::NotDesignated));
}

#[test]
fn a_build_waits_until_a_unit_delivers_its_cost_then_finishes_the_work() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let wood = defs.item_id("wood").unwrap();
    let bed = defs.building_id("bed").unwrap();
    let def = defs.building(bed).clone();
    w.designate_stockpile(at(0, 0)).unwrap();
    w.stored.insert(at(0, 0), Stack { item: wood, count: def.cost[0].count });
    let job = w.designate_build(at(5, 2), bed, &defs).unwrap();
    assert_eq!(w.build_sites().count(), 1);

    run_until(&mut w, &defs, 4000, |w| w.job(job).is_none());
    assert_eq!(w.building_at(at(5, 2)), Some(bed));
    assert_eq!(w.build_sites().count(), 0);
    // The cost was spent, not left lying around.
    assert_eq!(w.stored_total(wood), 0);
    assert_eq!(w.ground_items().count(), 0);
    assert_eq!(w.units[0].task(), None);
}

#[test]
fn a_build_waits_on_missing_materials_instead_of_failing() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let bed = defs.building_id("bed").unwrap();
    let job = w.designate_build(at(5, 2), bed, &defs).unwrap();
    // No materials exist anywhere: the job waits (claim/abandon/retry) rather than erroring out
    // or vanishing.
    for _ in 0..500 {
        w.tick(&defs);
    }
    assert!(w.job(job).is_some(), "the job waits rather than failing outright");
    assert!(w.building_at(at(5, 2)).is_none());
    assert_eq!(w.build_sites().count(), 1);
}

/// #322: a build job that can never be supplied (nothing in the world holds its next missing
/// item) used to pay for a whole-map flood every time `run_build` ran, which is every retry for
/// as long as it stays stuck. Calling `run_build` directly, bypassing the retry delay, makes
/// "every retry" cheap to simulate: if the fix holds, none of these calls floods at all.
#[test]
fn an_unsuppliable_build_job_is_abandoned_without_flooding_the_map() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let bed = defs.building_id("bed").unwrap();
    let job = w.designate_build(at(5, 2), bed, &defs).unwrap();
    // No wood exists anywhere on the map: every retry hits the same shortfall.
    crate::flood::CALLS.with(|c| c.set(0));
    for _ in 0..50 {
        w.run_build(0, job, &defs);
    }
    assert_eq!(
        crate::flood::CALLS.with(|c| c.get()),
        0,
        "no source of the item exists, so the fetch branch should never flood"
    );
    // The job is left waiting, exactly as before the fix.
    assert!(w.job(job).is_some());
    assert_eq!(w.units[0].route, None);
}

#[test]
fn a_finished_building_blocks_pathing_when_its_def_says_so() {
    let (mut w, mut defs) = setup(&GRID, &[at(1, 1)]);
    let wood = defs.item_id("wood").unwrap();
    let bed = defs.building_id("bed").unwrap();
    // Beds and campfires can be walked onto, so make the bed a wall for this test.
    defs.buildings[bed.0 as usize].passable = false;
    let def = defs.building(bed).clone();
    w.designate_stockpile(at(0, 0)).unwrap();
    w.stored.insert(at(0, 0), Stack { item: wood, count: def.cost[0].count });
    let job = w.designate_build(at(5, 2), bed, &defs).unwrap();
    run_until(&mut w, &defs, 4000, |w| w.job(job).is_none());
    assert!(w.map[at(5, 2)].building.is_some());
    assert!(!w.rules.passable(&w.map[at(5, 2)]), "a non-passable building blocks its own tile");
}

#[test]
fn cancelling_a_build_drops_delivered_materials_and_frees_the_unit() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let bed = defs.building_id("bed").unwrap();
    let job = w.designate_build(at(5, 2), bed, &defs).unwrap();
    // Simulate a partial delivery: three of the five wood the bed costs have reached the site.
    w.sites.get_mut(&at(5, 2)).expect("just designated").delivered[0] = 3;

    w.cancel_build(at(5, 2), &defs).unwrap();
    assert!(w.job(job).is_none());
    assert!(w.building_at(at(5, 2)).is_none());
    assert!(w.build_sites().next().is_none());
    // What had been delivered is now on the ground somewhere, to be hauled again.
    assert_eq!(w.ground_items().map(|(_, s)| s.count).sum::<u32>(), 3);
}

#[test]
fn another_nation_s_building_is_refused_because_its_tech_can_never_be_known() {
    let defs = Defs::bundled();
    let mut checked = 0;
    for nation in Nation::ALL {
        let (mut w, _) = setup_as(nation, &GRID, &[at(1, 1)]);
        for (i, def) in defs.buildings.iter().enumerate() {
            let Some(tech) = def.unlocked_by else { continue };
            if defs.tech(tech).nation.is_none_or(|n| n == nation) {
                continue;
            }
            assert_eq!(
                w.designate_build(at(2, 2), BuildingId(i as u16), &defs),
                Err(BuildError::NotResearched(tech)),
                "{nation:?} building {}",
                def.id
            );
            // And the tech can't be researched to get round it.
            assert_eq!(
                w.apply(Command::SetResearch { tech }, &defs),
                Err(CommandError::Research(ResearchError::WrongNation))
            );
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn a_locked_building_needs_its_tech_but_fire_starts_with_it() {
    let defs = Defs::bundled();
    let campfire = defs.building_id("campfire").expect("added by issue #22");
    let fire_making = defs.tech_id("fire_making").unwrap();

    // Water doesn't start with `fire_making`: the campfire is locked.
    let (mut w, _) = setup_as(Nation::Water, &GRID, &[at(1, 1)]);
    assert_eq!(
        w.designate_build(at(2, 2), campfire, &defs),
        Err(BuildError::NotResearched(fire_making))
    );
    assert!(!w.research().is_known(fire_making));

    // Once it finishes researching `fire_making` (25 points at 3/min, so tick 5000), it can.
    w.apply(Command::SetResearch { tech: fire_making }, &defs).unwrap();
    for _ in 0..5000 {
        w.tick(&defs);
    }
    assert!(w.research().is_known(fire_making));
    assert!(w.designate_build(at(2, 2), campfire, &defs).is_ok());

    // Fire Nation starts with `fire_making`, so it can build one from tick 0.
    let (mut fire, _) = setup_as(Nation::Fire, &GRID, &[at(1, 1)]);
    assert!(fire.research().is_known(fire_making));
    assert!(fire.designate_build(at(2, 2), campfire, &defs).is_ok());
}

#[test]
fn a_site_reports_its_building_delivery_and_progress() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let bed = defs.building_id("bed").unwrap();
    assert_eq!(w.build_site(at(2, 2)), None);
    w.designate_build(at(2, 2), bed, &defs).unwrap();
    let (building, delivered, progress) = w.build_site(at(2, 2)).expect("designated");
    assert_eq!(building, bed);
    assert_eq!(delivered, vec![0; defs.building(bed).cost.len()]);
    assert_eq!(progress, 0.0);
    w.cancel_build(at(2, 2), &defs).unwrap();
    assert_eq!(w.build_site(at(2, 2)), None);
}

#[test]
fn rail_is_built_like_any_building_and_marks_its_tile() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let rail = defs.building_id("rail").unwrap();
    let stone = defs.item_id("stone").unwrap();
    // Locked until Earth Rail is known.
    assert!(matches!(w.designate_build(at(5, 2), rail, &defs), Err(BuildError::NotResearched(_))));
    w.research.grant(defs.tech_id("earth_rail").unwrap());
    w.designate_stockpile(at(0, 0)).unwrap();
    w.stored.insert(at(0, 0), Stack { item: stone, count: defs.building(rail).cost[0].count });
    let job = w.designate_build(at(5, 2), rail, &defs).unwrap();
    run_until(&mut w, &defs, 4000, |w| w.job(job).is_none());
    // A plain 1x1 passable building, with the flag baked onto its tile for pathing.
    let built = w.map[at(5, 2)].building.expect("the rail is laid");
    assert!(built.rail && built.passable && built.id == rail);
    let bed = defs.building_id("bed").unwrap();
    assert!(defs.building(bed).rail.is_none());
}

/// The bundled bed turned into a solid 2x2 building with room inside (so it has a door, #226),
/// for footprint tests that shouldn't depend on the bundled study hall's data.
fn hall_2x2(defs: &mut Defs) -> BuildingId {
    let id = defs.building_id("bed").unwrap();
    let def = &mut defs.buildings[id.0 as usize];
    def.footprint = (2, 2);
    def.bed = None;
    def.passable = false;
    def.capacity = 2;
    id
}

/// Designating a 2x2 with its anchor on (2, 1) when (3, 2), the last of its four tiles, is what
/// `rows` makes blocked: refused, naming that tile, and nothing changes.
#[test]
fn one_blocked_tile_of_four_refuses_the_whole_footprint() {
    // A fifth row so the auxiliary site `blocks` designates below has somewhere on the map for
    // its own door (#226); `blocked` below only ever reads the first four.
    let free = [".........", ".........", ".........", ".........", "........."];
    let blocked = |c: &str| {
        let row = format!("...{c}.....");
        [free[0].to_owned(), free[1].to_owned(), row, free[3].to_owned()]
    };
    for (c, why) in [
        ("~", Blocker::Terrain),
        ("^", Blocker::Terrain),
        ("=", Blocker::Terrain),
        ("T", Blocker::Resource),
    ] {
        let rows = blocked(c);
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let (mut w, mut defs) = setup(&rows, &[at(0, 0)]);
        let hall = hall_2x2(&mut defs);
        let jobs = w.jobs().len();
        let refused = w.designate_build(at(2, 1), hall, &defs);
        assert_eq!(refused, Err(BuildError::CannotBuild { at: at(3, 2), why }), "{c}");
        assert_eq!(w.build_sites().count(), 0, "{c}");
        assert_eq!(w.jobs().len(), jobs, "{c}");
        // Moved one tile up, clear of it, the same building fits.
        assert!(w.designate_build(at(2, 0), hall, &defs).is_ok(), "{c}");
    }

    let blocks = |why: Blocker, block: &dyn Fn(&mut World, &Defs)| {
        let (mut w, mut defs) = setup(&free, &[at(0, 0)]);
        let hall = hall_2x2(&mut defs);
        block(&mut w, &defs);
        let (sites, jobs) = (w.build_sites().count(), w.jobs().len());
        assert_eq!(
            w.check_build(at(2, 1), hall, &defs),
            Err(BuildError::CannotBuild { at: at(3, 2), why })
        );
        assert_eq!(
            w.designate_build(at(2, 1), hall, &defs),
            Err(BuildError::CannotBuild { at: at(3, 2), why })
        );
        assert_eq!((w.build_sites().count(), w.jobs().len()), (sites, jobs), "{why:?}");
    };
    blocks(Blocker::Building, &|w, defs| {
        let bed = defs.building_id("campfire").unwrap();
        w.map[at(3, 2)].building = Some(Building::new(bed, true, false));
    });
    // Another 2x2 site anchored outside the footprint, overlapping it on one tile only.
    blocks(Blocker::Site, &|w, defs| {
        let hall = defs.building_id("bed").unwrap();
        w.designate_build(at(3, 2), hall, defs).unwrap();
    });
    blocks(Blocker::Stockpile, &|w, _| w.designate_stockpile(at(3, 2)).unwrap());
}

/// Issue #373: a building with a `beside` rule (Earth's Mine Shaft, `["mountain", "tunnel"]`)
/// needs more than free land -- its footprint's ring must touch one of those terrains, or
/// `check_build` refuses it with `Blocker::Beside`, naming what to touch ("must touch rock").
/// A shaft against a bored tunnel works the same as one against bare rock. The tech gate is
/// covered elsewhere (`a_locked_building_needs_its_tech_but_fire_starts_with_it`), so this frees
/// the shaft from it to focus on placement alone.
#[test]
fn a_mine_shaft_is_refused_on_open_land_and_accepted_beside_a_mountain_or_a_tunnel() {
    let rows = [
        "............",
        "............",
        "..^.......t.",
        "............",
        "............",
        "............",
        "............",
    ];
    let (mut w, mut defs) = setup(&rows, &[]);
    let shaft = defs.building_id("mine_shaft").unwrap();
    defs.buildings[shaft.0 as usize].unlocked_by = None;

    // Anchored at (0, 1): the footprint covers (0,1)-(1,2), and the mountain at (2, 2) touches
    // its ring (corners included).
    assert_eq!(w.check_build(at(0, 1), shaft, &defs), Ok(()));
    // Anchored at (8, 1): the footprint covers (8,1)-(9,2), and the tunnel at (10, 2) touches
    // its ring the same way a mountain would.
    assert_eq!(w.check_build(at(8, 1), shaft, &defs), Ok(()));

    // Anchored well away from either: open land, free of resources and buildings, but still
    // refused for what it doesn't touch.
    assert_eq!(
        w.check_build(at(2, 4), shaft, &defs),
        Err(BuildError::CannotBuild { at: at(2, 4), why: Blocker::Beside("rock") })
    );
    assert_eq!(
        w.designate_build(at(2, 4), shaft, &defs),
        Err(BuildError::CannotBuild { at: at(2, 4), why: Blocker::Beside("rock") })
    );
}

/// Issue #374: Water's Fishing Dock has a `beside` rule too, `["water", "river", "lake"]`, so
/// it's refused on open land exactly as the Mine Shaft is away from rock. A tile Draw Water
/// (#364) turned into a temporary `Ford` doesn't satisfy it: `beside` names the terrain a tile
/// must actually be, and a ford is its own `Terrain` variant, not `water`, `river` or `lake`, so
/// a dock built against one would be standing on nothing once the ford reverts.
#[test]
fn a_fishing_dock_is_refused_on_open_land_and_a_temporary_ford_but_accepted_beside_water() {
    let rows = [
        "............",
        "............",
        "..~.......=.",
        "............",
        "............",
        "............",
        "............",
    ];
    let (w, mut defs) = setup(&rows, &[]);
    let dock = defs.building_id("fishing_dock").unwrap();
    defs.buildings[dock.0 as usize].unlocked_by = None;

    // Anchored at (0, 1): the footprint covers (0,1)-(1,2), and the sea tile at (2, 2) touches
    // its ring (corners included).
    assert_eq!(w.check_build(at(0, 1), dock, &defs), Ok(()));

    // Anchored at (8, 1): the footprint covers (8,1)-(9,2), and the only thing touching its ring
    // is the ford at (10, 2) -- not real water, so this is refused exactly as open land is.
    assert_eq!(
        w.check_build(at(8, 1), dock, &defs),
        Err(BuildError::CannotBuild { at: at(8, 1), why: Blocker::Beside("water") })
    );
    assert_eq!(
        w.check_build(at(2, 4), dock, &defs),
        Err(BuildError::CannotBuild { at: at(2, 4), why: Blocker::Beside("water") })
    );
}

/// Issue #200: the Tide Mill has the same `beside` rule as the Fishing Dock, so it's refused
/// on open land and accepted on the bank.
#[test]
fn a_tide_mill_is_refused_on_open_land_and_accepted_beside_water() {
    let rows = [
        "............",
        "............",
        "..~.........",
        "............",
        "............",
        "............",
        "............",
    ];
    let (w, mut defs) = setup(&rows, &[]);
    let mill = defs.building_id("tide_mill").unwrap();
    defs.buildings[mill.0 as usize].unlocked_by = None;

    // Anchored at (0, 1): the footprint covers (0,1)-(1,2), and the sea tile at (2, 2) touches
    // its ring (corners included).
    assert_eq!(w.check_build(at(0, 1), mill, &defs), Ok(()));

    // Anchored well away from the water: open land, refused for what it doesn't touch.
    assert_eq!(
        w.check_build(at(5, 4), mill, &defs),
        Err(BuildError::CannotBuild { at: at(5, 4), why: Blocker::Beside("water") })
    );
}

#[test]
fn placement_checks_report_the_first_blocked_tile_row_major() {
    let rows = ["..........", "...~T.....", "...T~.....", ".........."];
    let (w, mut defs) = setup(&rows, &[at(0, 0)]);
    let hall = hall_2x2(&mut defs);
    let refused = |anchor| w.check_build(anchor, hall, &defs);
    assert_eq!(
        refused(at(3, 1)),
        Err(BuildError::CannotBuild { at: at(3, 1), why: Blocker::Terrain })
    );
    assert_eq!(
        refused(at(4, 0)),
        Err(BuildError::CannotBuild { at: at(4, 1), why: Blocker::Resource })
    );
    assert_eq!(
        refused(at(9, 0)),
        Err(BuildError::CannotBuild { at: at(10, 0), why: Blocker::OffMap })
    );
    assert_eq!(
        refused(at(-1, 0)),
        Err(BuildError::CannotBuild { at: at(-1, 0), why: Blocker::OffMap })
    );
    assert_eq!(refused(at(6, 1)), Ok(()));
    // Coast is buildable; a unit standing in the footprint doesn't block it.
    let coast = ["......", ".,,...", ".,,...", "......"];
    let (mut w, mut defs) = setup(&coast, &[at(2, 2)]);
    let hall = hall_2x2(&mut defs);
    assert!(w.designate_build(at(1, 1), hall, &defs).is_ok());
    // A 1x1 building on a ford is refused too, though fords are walkable.
    let ford = ["...", ".=.", "..."];
    let (mut w, defs) = setup(&ford, &[at(0, 0)]);
    let bed = defs.building_id("bed").unwrap();
    assert_eq!(
        w.designate_build(at(1, 1), bed, &defs),
        Err(BuildError::CannotBuild { at: at(1, 1), why: Blocker::Terrain })
    );
}

/// #226: a footprint whose own tiles would cover another building's or site's door is refused,
/// and so is one whose own door would land on unwalkable ground, even though neither tile is
/// otherwise in the way.
#[test]
fn a_footprint_may_not_cover_another_doorway() {
    let (mut w, mut defs) = setup(&GRID, &[at(0, 0)]);
    let hall = hall_2x2(&mut defs);
    // Anchored on (2, 0): footprint (2..4, 0..2), door at (2, 2).
    w.designate_build(at(2, 0), hall, &defs).unwrap();
    assert_eq!(w.footprint_at(at(2, 0)).unwrap().entrance(), at(2, 2));
    // A second hall whose footprint would cover that door, even though every ordinary tile is
    // free, is refused.
    assert_eq!(
        w.designate_build(at(2, 2), hall, &defs),
        Err(BuildError::CannotBuild { at: at(2, 2), why: Blocker::Entrance })
    );
    assert_eq!(w.build_sites().count(), 1, "the refused one changed nothing");
    // Moved clear of the door, the same building fits.
    assert!(w.designate_build(at(3, 2), hall, &defs).is_ok());
}

/// Only buildings that can be entered have doors: solid 1x1 buildings with no capacity stack in a
/// column (a north-south wall) and sit with water right below them.
#[test]
fn a_building_nobody_enters_has_no_door_to_keep_clear() {
    let rows = ["......", "......", "......", "~~....", "......"];
    let (mut w, mut defs) = setup(&rows, &[at(5, 4)]);
    let id = defs.building_id("bed").unwrap();
    let wall = &mut defs.buildings[id.0 as usize];
    wall.bed = None;
    wall.passable = false;
    wall.capacity = 0;
    for y in 0..3 {
        assert_eq!(w.designate_build(at(1, y), id, &defs).map(|_| ()), Ok(()), "wall at y={y}");
    }
}

#[test]
fn an_unwalkable_doorway_is_refused_even_though_the_footprint_itself_fits() {
    let rows = ["......", "......", "......", "~~....", "......"];
    let (w, mut defs) = setup(&rows, &[at(0, 0)]);
    let hall = hall_2x2(&mut defs);
    // Anchored on (0, 1): footprint (0..2, 1..3) is all land, but its door at (0, 3) is water.
    assert_eq!(
        w.check_build(at(0, 1), hall, &defs),
        Err(BuildError::CannotBuild { at: at(0, 3), why: Blocker::Entrance })
    );
    // Two tiles over, the door at (2, 3) lands on land instead.
    assert!(w.check_build(at(2, 1), hall, &defs).is_ok());
}

#[test]
fn a_site_covers_and_cancels_from_every_tile_of_its_footprint() {
    let (mut w, mut defs) = setup(&GRID, &[at(0, 0)]);
    let hall = hall_2x2(&mut defs);
    let job = w.designate_build(at(4, 1), hall, &defs).unwrap();
    let foot = Footprint::new(at(4, 1), 2, 2);
    let sites: Vec<_> = w.build_sites().map(|(f, id, _)| (f, id)).collect();
    assert_eq!(sites, [(foot, hall)]);
    for p in foot.tiles() {
        assert_eq!(w.footprint_at(p), Some(foot), "{p:?}");
        assert_eq!(w.build_site(p).map(|(id, ..)| id), Some(hall), "{p:?}");
        assert_eq!(w.build_blocker(p), Some(Blocker::Site), "{p:?}");
    }
    assert_eq!(w.footprint_at(at(6, 1)), None);
    assert_eq!(w.cancel_build(at(6, 2), &defs), Err(BuildError::NotDesignated));
    w.cancel_build(at(5, 2), &defs).unwrap();
    assert!(w.job(job).is_none());
    assert_eq!(w.build_sites().count(), 0);
    assert!(foot.tiles().all(|p| w.build_blocker(p).is_none()));
}

#[test]
fn finishing_a_site_bakes_the_building_onto_every_footprint_tile() {
    let (mut w, mut defs) = setup(&GRID, &[at(1, 1)]);
    let wood = defs.item_id("wood").unwrap();
    let hall = hall_2x2(&mut defs);
    w.designate_stockpile(at(0, 0)).unwrap();
    w.stored.insert(at(0, 0), Stack { item: wood, count: defs.building(hall).cost[0].count });
    let job = w.designate_build(at(5, 2), hall, &defs).unwrap();
    run_until(&mut w, &defs, 4000, |w| w.job(job).is_none());

    let foot = Footprint::new(at(5, 2), 2, 2);
    for p in foot.tiles() {
        let b = w.map[p].building.expect("built on every tile");
        assert_eq!(b.id, hall);
        assert_eq!(b.part, [(p.x - 5) as u8, (p.y - 2) as u8]);
        assert_eq!(b.size, [2, 2]);
        assert!(!b.passable);
        assert_eq!(w.building_at(p), Some(hall));
        assert_eq!(w.footprint_at(p), Some(foot));
        assert_eq!(w.build_blocker(p), Some(Blocker::Building));
    }
    assert_eq!(w.building_at(at(7, 2)), None);
    assert_eq!(w.buildings().collect::<Vec<_>>(), [(foot, hall)], "listed once");
    assert_eq!(w.build_sites().count(), 0);
    // Hauling and building both work from the ring and never enter, even though this building
    // has room inside (#227).
    assert!(w.units[0].inside().is_none());
}

/// Sizes are baked onto the tiles and sites when they are placed, never re-derived from the
/// data: a 1x1 saved before the data made it 2x2 loads as the 1x1 it was built as.
#[test]
fn a_saved_building_keeps_its_size_when_the_data_grows_it() {
    let (mut w, defs) = setup(&GRID, &[at(0, 0)]);
    let bed = defs.building_id("bed").unwrap();
    let campfire = defs.building_id("campfire").unwrap();
    w.map[at(2, 2)].building = Some(Building::new(campfire, true, false));
    w.designate_build(at(5, 2), bed, &defs).unwrap();
    let text = w.save(&defs).unwrap();
    assert!(!text.contains("size") && !text.contains("part"), "1x1 writes no footprint fields");

    let mut grown = defs.clone();
    for id in [bed, campfire] {
        let def = &mut grown.buildings[id.0 as usize];
        def.footprint = (2, 2);
        def.bed = None;
    }
    let loaded = World::load(&text, &grown).unwrap();
    assert_eq!(loaded.footprint_at(at(2, 2)), Some(Footprint::single(at(2, 2))));
    assert_eq!(loaded.footprint_at(at(5, 2)), Some(Footprint::single(at(5, 2))));
    assert_eq!(loaded.footprint_at(at(3, 3)), None);
    assert_eq!(loaded.footprint_at(at(6, 3)), None);
    assert_eq!(loaded.save(&grown).unwrap(), text, "and re-saves unchanged");
}

#[test]
fn a_bigger_building_saves_its_footprint_and_loads_it_back() {
    let (mut w, mut defs) = setup(&GRID, &[at(0, 0)]);
    let hall = hall_2x2(&mut defs);
    w.designate_build(at(5, 2), hall, &defs).unwrap();
    let campfire = defs.building_id("campfire").unwrap();
    let built = Building { size: [2, 1], ..Building::new(campfire, true, false) };
    w.map[at(1, 3)].building = Some(built);
    w.map[at(2, 3)].building = Some(Building { part: [1, 0], ..built });
    let loaded = World::load(&w.save(&defs).unwrap(), &defs).unwrap();
    assert_eq!(loaded.footprint_at(at(6, 3)), Some(Footprint::new(at(5, 2), 2, 2)));
    assert_eq!(loaded.footprint_at(at(2, 3)), Some(Footprint::new(at(1, 3), 2, 1)));
    assert_eq!(
        loaded.buildings().collect::<Vec<_>>(),
        [(Footprint::new(at(1, 3), 2, 1), campfire)]
    );
}

#[test]
fn a_solid_site_is_built_from_beside_it_never_inside() {
    let (mut w, mut defs) = setup(&GRID, &[at(1, 1)]);
    let wood = defs.item_id("wood").unwrap();
    let hall = hall_2x2(&mut defs);
    w.designate_stockpile(at(0, 0)).unwrap();
    w.stored.insert(at(0, 0), Stack { item: wood, count: defs.building(hall).cost[0].count });
    let job = w.designate_build(at(5, 2), hall, &defs).unwrap();
    let foot = Footprint::new(at(5, 2), 2, 2);
    let mut worked_from = None;
    run_until(&mut w, &defs, 4000, |w| {
        let u = &w.units[0];
        assert!(!foot.contains(u.pos), "the builder stood inside the site at {:?}", u.pos);
        if w.build_site(at(5, 2)).is_some_and(|(_, _, work)| work > 0.0) {
            worked_from = Some(u.pos);
        }
        w.job(job).is_none()
    });
    assert_eq!(w.building_at(at(6, 3)), Some(hall));
    let from = worked_from.expect("the site was worked");
    assert_eq!(foot.distance(from), 1, "worked from the ring, at {from:?}");
}

#[test]
fn the_earth_walls_builder_works_from_beside_it() {
    let (mut w, mut defs) = setup(&GRID, &[at(1, 1)]);
    let stone = defs.item_id("stone").unwrap();
    let wall = defs.building_id("earth_wall").unwrap();
    defs.buildings[wall.0 as usize].unlocked_by = None;
    w.designate_stockpile(at(0, 0)).unwrap();
    w.stored.insert(at(0, 0), Stack { item: stone, count: defs.building(wall).cost[0].count });
    let job = w.designate_build(at(5, 2), wall, &defs).unwrap();
    run_until(&mut w, &defs, 4000, |w| {
        assert_ne!(w.units[0].pos, at(5, 2), "the builder stood on its own wall");
        w.job(job).is_none()
    });
    assert_eq!(w.building_at(at(5, 2)), Some(wall));
    assert_eq!(w.units[0].pos.chebyshev(at(5, 2)), 1, "finished from beside it");
}

/// Supplies the site on `anchor` and leaves it a moment's work from done, so a test sees only
/// the finish.
fn nearly_done(w: &mut World, defs: &Defs, anchor: Pos) {
    let site = w.sites.get_mut(&anchor).expect("designated");
    let def = defs.building(site.building);
    site.delivered = def.cost.iter().map(|c| c.count).collect();
    site.progress = def.work - 0.01;
}

#[test]
fn finishing_a_solid_building_moves_units_and_items_off_its_footprint() {
    let (mut w, mut defs) = setup(&GRID, &[at(1, 1), at(6, 3)]);
    let wood = defs.item_id("wood").unwrap();
    let hall = hall_2x2(&mut defs);
    w.designate_build(at(5, 2), hall, &defs).unwrap();
    nearly_done(&mut w, &defs, at(5, 2));
    // The second unit stands still on the footprint and takes no job.
    w.units[1].claim_retry_at = u64::MAX;
    w.units[1].idle_until = u64::MAX;
    w.ground.insert(at(5, 2), Stack { item: wood, count: 2 });

    run_until(&mut w, &defs, 2000, |w| w.building_at(at(5, 2)).is_some());
    // The nearest walkable tile outside, the first of the ring around it in row-major order.
    assert_eq!(w.units[1].pos, at(7, 2));
    assert_eq!(w.units[1].route, None);
    let ground: Vec<_> = w.ground_items().collect();
    assert_eq!(ground, [(at(4, 1), Stack { item: wood, count: 2 })]);
    assert!(Footprint::new(at(5, 2), 2, 2).tiles().all(|p| !w.ground.contains_key(&p)));
}

#[test]
fn finishing_a_walkable_building_leaves_units_and_items_where_they_are() {
    let (mut w, mut defs) = setup(&GRID, &[at(1, 1), at(6, 3)]);
    let wood = defs.item_id("wood").unwrap();
    let hall = hall_2x2(&mut defs);
    defs.buildings[hall.0 as usize].passable = true;
    w.designate_build(at(5, 2), hall, &defs).unwrap();
    nearly_done(&mut w, &defs, at(5, 2));
    w.units[1].claim_retry_at = u64::MAX;
    w.units[1].idle_until = u64::MAX;
    w.ground.insert(at(5, 2), Stack { item: wood, count: 2 });
    run_until(&mut w, &defs, 2000, |w| w.building_at(at(5, 2)).is_some());
    assert_eq!(w.units[1].pos, at(6, 3));
    assert!(w.ground.contains_key(&at(5, 2)));
}

// --- Brick stands in for stone (#371) ---

#[test]
fn a_site_takes_stone_or_its_stand_in_whichever_is_shorter_of_the_two() {
    use crate::building::CostItem;

    let (mut w, mut defs) = setup_as(Nation::Fire, &GRID, &[at(1, 1)]);
    let stone = defs.item_id("stone").unwrap();
    let brick = defs.item_id("brick").unwrap();
    let bed = defs.building_id("bed").unwrap();
    defs.buildings[bed.0 as usize].cost = vec![CostItem { item: stone, count: 10 }];
    w.designate_stockpile(at(0, 0)).unwrap();
    w.designate_stockpile(at(0, 1)).unwrap();
    // Only 4 stone exists; the rest has to come from brick, which stands in for it one for one
    // (issue #371).
    w.stored.insert(at(0, 0), Stack { item: stone, count: 4 });
    w.stored.insert(at(0, 1), Stack { item: brick, count: 6 });
    let job = w.designate_build(at(5, 2), bed, &defs).unwrap();

    run_until(&mut w, &defs, 8000, |w| w.job(job).is_none());
    assert_eq!(w.building_at(at(5, 2)), Some(bed));
    assert_eq!(w.stored_total(stone), 0, "all 4 stone went into the site");
    assert_eq!(w.stored_total(brick), 0, "all 6 brick went into the site too");
}

#[test]
fn cancelling_a_site_paid_partly_in_brick_refunds_brick_not_stone() {
    use crate::building::CostItem;

    let (mut w, mut defs) = setup_as(Nation::Fire, &GRID, &[at(1, 1)]);
    let stone = defs.item_id("stone").unwrap();
    let brick = defs.item_id("brick").unwrap();
    let bed = defs.building_id("bed").unwrap();
    defs.buildings[bed.0 as usize].cost = vec![CostItem { item: stone, count: 10 }];
    let job = w.designate_build(at(5, 2), bed, &defs).unwrap();
    // Simulate a partial delivery: 4 stone and 6 brick (the stand-in) have reached the site.
    {
        let site = w.sites.get_mut(&at(5, 2)).expect("just designated");
        site.delivered[0] = 10;
        site.stand_in_delivered = vec![6];
    }

    w.cancel_build(at(5, 2), &defs).unwrap();
    assert!(w.job(job).is_none());
    assert!(w.building_at(at(5, 2)).is_none());
    let ground: std::collections::HashMap<ItemId, u32> =
        w.ground_items().map(|(_, s)| (s.item, s.count)).collect();
    assert_eq!(ground.get(&stone).copied(), Some(4), "the stone delivered drops as stone");
    assert_eq!(
        ground.get(&brick).copied(),
        Some(6),
        "the brick delivered drops as brick, not stone"
    );
}

#[test]
fn a_stand_in_never_pays_a_recipe_input() {
    // The kiln's own recipe burns stone, not brick, even with nothing but brick in stores
    // (issue #371: a stand-in only ever pays a build or endeavor cost).
    let (mut w, mut defs) = setup_as(Nation::Fire, &GRID, &[at(1, 1)]);
    let stone = defs.item_id("stone").unwrap();
    let brick = defs.item_id("brick").unwrap();
    let wood = defs.item_id("wood").unwrap();
    let kiln = defs.building_id("brick_kiln").unwrap();
    defs.buildings[kiln.0 as usize].unlocked_by = None;
    w.designate_stockpile(at(0, 0)).unwrap();
    w.designate_stockpile(at(0, 1)).unwrap();
    w.stored.insert(at(0, 0), Stack { item: stone, count: defs.building(kiln).cost[0].count });
    w.stored.insert(at(0, 1), Stack { item: wood, count: defs.building(kiln).cost[1].count });
    let job = w.designate_build(at(5, 2), kiln, &defs).unwrap();
    run_until(&mut w, &defs, 6000, |w| w.job(job).is_none());
    assert_eq!(w.building_at(at(5, 2)), Some(kiln));

    // Now give the kiln's crafter nothing but brick where its recipe wants stone: it must not
    // take it.
    w.stored.clear();
    w.designate_stockpile(at(2, 0)).unwrap();
    w.stored.insert(at(2, 0), Stack { item: brick, count: 50 });
    for _ in 0..3000 {
        w.tick(&defs);
    }
    assert_eq!(w.stored_total(brick), 50, "the crafter never touched the brick");
}

/// #384: a building spanning the only way through a narrow place walls off everything past it,
/// the same as a mountain would. `would_strand` catches that before anything is built, reading
/// `study_hall`'s real footprint (2x2) off the data rather than hardcoding one.
#[test]
fn a_building_across_the_only_corridor_strands_the_far_end() {
    let rows = ["^^^^^^^^", "^^^^^^^^", "........", "........", "^^^^^^^^", "^^^^^^^^"];
    let (w, defs) = setup(&rows, &[]);
    let study_hall = defs.building_id("study_hall").unwrap();
    let (fw, fh) = defs.building(study_hall).footprint;
    assert_eq!((fw, fh), (2, 2));
    let anchor = at(0, 2);
    assert!(w.would_strand(anchor, Footprint::new(at(3, 2), fw, fh)), "walls off columns 5-7");
}

/// The same building at the corridor's dead end strands nothing, since there is no ground past
/// it to begin with: `would_strand` isn't just "is this footprint solid", it's "does solid ground
/// here cut anything off".
#[test]
fn a_building_at_the_corridor_s_dead_end_strands_nothing() {
    let rows = ["^^^^^^^^", "^^^^^^^^", "........", "........", "^^^^^^^^", "^^^^^^^^"];
    let (w, defs) = setup(&rows, &[]);
    let study_hall = defs.building_id("study_hall").unwrap();
    let (fw, fh) = defs.building(study_hall).footprint;
    let anchor = at(0, 2);
    assert!(!w.would_strand(anchor, Footprint::new(at(6, 2), fw, fh)));
}

/// A footprint the flood never even reaches (an island with no path from `anchor` at all)
/// strands nothing either: there was nothing there to cut off.
#[test]
fn a_footprint_already_unreachable_from_anchor_strands_nothing() {
    let rows = ["..^.....", "..^.....", "..^.....", "..^....."];
    let (w, defs) = setup(&rows, &[]);
    let study_hall = defs.building_id("study_hall").unwrap();
    let (fw, fh) = defs.building(study_hall).footprint;
    let anchor = at(0, 0);
    assert!(!w.would_strand(anchor, Footprint::new(at(4, 0), fw, fh)));
}

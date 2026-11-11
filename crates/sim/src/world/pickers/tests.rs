use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

fn field(size: i32) -> TileMap {
    Grid::from_fn(size, size, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

const ROOST: Pos = Pos::new(10, 10);

/// A 21x21 field, no settlers, a finished roost at (10, 10), and fruit trees on `trees`.
fn setup(trees: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    let mut map = field(21);
    let tree = defs.resource_id("fruit_tree").unwrap();
    for &p in trees {
        map[p].resource = Some(tree);
    }
    let mut w = World::with_map(1, map, Nation::Air, &[], &defs);
    let roost = defs.building_id("lemur_roost").unwrap();
    w.put_building(Footprint::single(ROOST), roost, &defs);
    (w, defs)
}

fn pick_ticks(defs: &Defs) -> u32 {
    defs.building(defs.building_id("lemur_roost").unwrap()).pickers.as_ref().unwrap().pick_ticks
}

fn run(w: &mut World, defs: &Defs, ticks: u32) {
    for _ in 0..ticks {
        w.tick(defs);
    }
}

fn fruit_everywhere(w: &World, defs: &Defs) -> u32 {
    let fruit = defs.item_id("fruit").unwrap();
    w.stored_total(fruit)
        + w.ground.values().filter(|s| s.item == fruit).map(|s| s.count).sum::<u32>()
}

#[test]
fn pickers_are_validated() {
    let old = "eat_every = 4          # the lemurs eat every 4th fruit they pick";
    for (new, why) in [("eat_every = 1", "eat_every"), ("eat_every = 4\nradius2 = 1", "radius2")] {
        let mut files = crate::defs::BUNDLED_FILES;
        let b = files[7].replacen(old, new, 1);
        files[7] = &b;
        let e = Defs::from_files(files).expect_err(new).0;
        assert!(e.contains(why), "{new}: {e}");
    }
    let mut files = crate::defs::BUNDLED_FILES;
    let b = files[7].replacen("resources = [\"fruit_tree\"]", "resources = [\"nope\"]", 1);
    files[7] = &b;
    assert!(Defs::from_files(files).unwrap_err().0.contains("unknown resource"));
    let defs = Defs::bundled();
    let def = defs.building(defs.building_id("lemur_roost").unwrap()).pickers.clone().unwrap();
    assert_eq!((def.radius, def.pick_ticks, def.eat_every), (5, 300, 4));
    assert_eq!(
        defs.building(defs.building_id("lemur_roost").unwrap()).unlocked_by,
        defs.tech_id("lemur_keeping")
    );
}

#[test]
fn it_picks_the_nearest_ripe_tree_in_reach_every_interval() {
    // Two trees 2 tiles away (tie: the lower tile index wins), one 4 away, one out of reach.
    let (mut w, defs) = setup(&[at(12, 10), at(8, 10), at(10, 14), at(17, 10)]);
    // A preset, so tiring land (strain) is on.
    w.difficulty = Some("normal".into());
    let n = pick_ticks(&defs);
    run(&mut w, &defs, n - 1);
    assert_eq!(w.pickers_at(ROOST).unwrap().picked, 0, "not before a full interval");
    run(&mut w, &defs, 1);
    assert!(w.resting.contains_key(&at(8, 10)), "the tie goes to the lower tile index");
    assert_eq!(w.pickers_at(ROOST).unwrap().picked, 2);
    run(&mut w, &defs, n);
    assert!(w.resting.contains_key(&at(12, 10)));
    run(&mut w, &defs, n);
    assert!(w.resting.contains_key(&at(10, 14)));
    run(&mut w, &defs, 2 * n);
    assert!(!w.resting.contains_key(&at(17, 10)), "out of reach");
    let p = w.pickers_at(ROOST).unwrap();
    assert_eq!((p.picked, p.eaten), (6, 1), "every 4th fruit is eaten");
    assert_eq!(fruit_everywhere(&w, &defs), 5);
    // Each pick counts as a harvest and strains the tree.
    assert_eq!(w.tallies.harvested(defs.item_id("fruit").unwrap()), 6);
    assert!(w.strain.contains_key(&at(8, 10)));
}

#[test]
fn fruit_goes_to_a_stockpile_in_reach_or_beside_the_roost() {
    let (mut w, defs) = setup(&[at(11, 10), at(9, 10)]);
    let fruit = defs.item_id("fruit").unwrap();
    // No stockpile: the fruit drops beside the roost, to be hauled.
    run(&mut w, &defs, pick_ticks(&defs));
    assert_eq!(w.stored_total(fruit), 0);
    assert_eq!(fruit_everywhere(&w, &defs), 2);
    // A stockpile in reach takes the next pick directly.
    w.designate_stockpile(at(13, 13)).unwrap();
    w.designate_stockpile(at(16, 16)).unwrap(); // out of reach
    run(&mut w, &defs, pick_ticks(&defs));
    assert_eq!(w.stored.get(&at(13, 13)).map(|s| s.count), Some(1), "2 picked, 1 eaten");
    assert!(!w.stored.contains_key(&at(16, 16)));
}

#[test]
fn it_skips_a_claimed_tree_and_clears_an_unclaimed_designation() {
    let (mut w, defs) = setup(&[at(11, 10), at(13, 10)]);
    w.research.grant(defs.tech_id("orchards").unwrap());
    let near = w.designate_harvest(at(11, 10), &defs).unwrap();
    let idx = w.job_index(near).unwrap();
    w.jobs[idx].claimed_by = Some(UnitId(99));
    w.designate_harvest(at(13, 10), &defs).unwrap();
    for _ in 0..pick_ticks(&defs) {
        w.tick_pickers(&defs);
    }
    assert!(!w.resting.contains_key(&at(11, 10)), "a settler has it");
    assert!(w.resting.contains_key(&at(13, 10)));
    assert!(
        !w.jobs.iter().any(|j| j.kind == JobKind::Harvest { pos: at(13, 10) }),
        "its unclaimed designation is done with"
    );
}

#[test]
fn picking_is_deterministic_and_survives_a_save() {
    let trees: Vec<Pos> = (5..16).step_by(2).map(|x| at(x, 7)).collect();
    let (mut w, defs) = setup(&trees);
    run(&mut w, &defs, pick_ticks(&defs) * 2 + 7);
    let saved = w.save(&defs).unwrap();
    let mut b = World::load(&saved, &defs).unwrap();
    assert_eq!(b.pickers_at(ROOST), w.pickers_at(ROOST));
    run(&mut w, &defs, pick_ticks(&defs) * 3);
    run(&mut b, &defs, pick_ticks(&defs) * 3);
    assert_eq!(w.save(&defs).unwrap(), b.save(&defs).unwrap());
    assert_eq!(w.pickers_at(ROOST).unwrap().picked, 10);
}

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::endeavor::SignKind;
use crate::geom::Grid;
use crate::jobs::{JobKind, Stack};
use crate::mapgen::Tile;
use crate::world::Command;

fn field(width: i32, height: i32) -> TileMap {
    Grid::from_fn(width, height, |_| Tile {
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

fn item(defs: &Defs, id: &str) -> ItemId {
    defs.item_id(id).unwrap()
}

#[test]
fn item_upgrades_are_validated() {
    let with = |up: &str| {
        let mut files = crate::defs::BUNDLED_FILES;
        let research = format!(
            "{}\n[[tech]]\nid = \"t\"\nname = \"T\"\ndescription = \"\"\ncost = 10\n\
             [tech.unlocks]\n[[tech.unlocks.upgrades]]\n{up}\n",
            files[3]
        );
        files[3] = &research;
        Defs::from_files(files).map(|_| ())
    };
    for (up, why) in [
        ("item = \"nope\"\nstands_for = \"stone\"", "unknown item \"nope\""),
        ("item = \"fish\"\nstands_for = \"nope\"", "unknown item \"nope\""),
        ("item = \"fish\"", "must name one property: stands_for"),
        ("item = \"fish\"\nstands_for = \"fish\"", "can't stand for itself"),
        ("item = \"brick\"\nstands_for = \"wood\"", "already stands for something"),
        ("item = \"fish\"\nstands_for = \"brick\"", "itself stands for something"),
        ("item = \"fish\"\nstands_for = \"stone\"\nseats = 2", "can only take `stands_for`"),
        ("item = \"ice\"\nstands_for = \"wood\"", "already upgraded by \"northern_city\""),
        ("item = \"fish\"\nbuilding = \"bed\"\nstands_for = \"stone\"", "names both"),
        ("building = \"study_hall\"\nstands_for = \"stone\"", "an item upgrade's property"),
    ] {
        let e = with(up).expect_err(up).0;
        assert!(e.contains(why), "{up}: {e}");
    }
    let defs = Defs::bundled();
    let (ice, stone) = (item(&defs, "ice"), item(&defs, "stone"));
    let tech = defs.tech_id("northern_city").unwrap();
    assert_eq!(
        defs.item_upgrades,
        [crate::jobs::ItemUpgrade { tech, item: ice, stands_for: stone }]
    );
}

#[test]
fn ice_stands_for_stone_only_once_the_tech_is_known() {
    let defs = Defs::bundled();
    let (ice, stone, brick) = (item(&defs, "ice"), item(&defs, "stone"), item(&defs, "brick"));
    let mut w = World::with_map(1, field(10, 10), Nation::Water, &[at(0, 0)], &defs);
    assert_eq!(w.stands_for(ice, &defs), None);
    assert_eq!(w.stand_ins(&defs).for_target(stone, &defs), [brick]);
    w.research.grant(defs.tech_id("northern_city").unwrap());
    assert_eq!(w.stands_for(ice, &defs), Some(stone));
    assert_eq!(w.stand_ins(&defs).for_target(stone, &defs), [brick, ice]);
    assert_eq!(w.stands_for(brick, &defs), Some(stone), "brick still does");
}

#[test]
fn ice_pays_a_buildings_stone_and_comes_back_on_cancel() {
    let defs = Defs::bundled();
    let (ice, wood) = (item(&defs, "ice"), item(&defs, "wood"));
    let forge = defs.building_id("forge").unwrap();
    let mut w = World::with_map(1, field(20, 12), Nation::Water, &[at(2, 4)], &defs);
    if let Some(t) = defs.building(forge).unlocked_by {
        w.research.grant(t);
    }
    w.research.grant(defs.tech_id("northern_city").unwrap());
    for (x, it) in [(1, ice), (2, wood)] {
        w.designate_stockpile(at(x, 1)).unwrap();
        w.stored.insert(at(x, 1), Stack { item: it, count: 20 });
    }
    let site = at(10, 6);
    w.apply(Command::DesignateBuild { pos: site, building: forge }, &defs).unwrap();
    for _ in 0..4_000 {
        w.tick(&defs);
        if w.next_shortfall(site, &defs).is_none() {
            break;
        }
    }
    assert!(w.next_shortfall(site, &defs).is_none(), "ice paid the forge's 8 stone");
    assert_eq!(w.stored_total(ice), 12);
    w.jobs.retain(|j| j.kind != JobKind::Build { pos: site });
    w.apply(Command::CancelBuild { pos: site }, &defs).unwrap();
    let back: u32 = w.ground.values().filter(|s| s.item == ice).map(|s| s.count).sum();
    assert_eq!(back, 8, "the ice comes back as ice");
}

#[test]
fn the_stone_sign_still_counts_only_stone_and_brick() {
    let defs = Defs::bundled();
    let sign = defs.endeavor.signs.iter().find(|s| s.id == "stone").expect("a stone sign");
    let SignKind::Stored { items } = &sign.kind else { panic!("{:?}", sign.kind) };
    let names: Vec<&str> = items.iter().map(|&i| defs.item(i).id.as_str()).collect();
    assert_eq!(names, ["stone", "brick"]);
}

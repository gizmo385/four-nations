use super::*;
use crate::autoplay::Economy;
use crate::mapgen::MapGenConfig;
use crate::nation::Nation;

fn world(nation: Nation, seed: u64) -> (World, Defs) {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    let cfg =
        MapGenConfig { seed, width: size.width, height: size.height, ..MapGenConfig::default() };
    (World::new(&cfg, nation, 6, &defs), defs)
}

#[test]
fn the_road_is_short_enough_and_starts_at_the_stockpile() {
    let (mut w, defs) = world(Nation::Earth, 42);
    let economy = Economy::new(2);
    economy.step(&mut w, &defs);
    let anchor = w.settlement_anchor();
    let path = road_path(&w, &defs, anchor).expect("a deposit within reach");
    assert!(path.len() <= ROAD_TILES);
    assert!(w.stockpile_zones().any(|z| z == path[0]));
}

#[test]
fn site_anchor_falls_back_to_the_settlement_anchor() {
    let (w, defs) = world(Nation::Water, 3);
    let anchor = w.settlement_anchor();
    for id in ["oil_lamp", "hothouse", "farm_plot", "chute", "lemur_roost", "bed"] {
        assert_eq!(site_anchor(&w, &defs, id, anchor), anchor, "{id}: nothing to stand beside yet");
    }
}

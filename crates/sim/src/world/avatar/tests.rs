use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

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

/// Where the tests put the gate: a 4x4 footprint, with plenty of room to its west.
const ANCHOR: Pos = Pos::new(12, 4);

/// A valid `[avatar]` section with short timings, so a test doesn't have to wait out the real
/// (many-minute) defaults.
const AVATAR: &str = "\n[avatar]\nid = \"aang\"\nname = \"Aang\"\ncharacter = \"aang\"\n\
                       chi_max = 200.0\npatience_minutes = 1.0\nreturn_minutes = 1.0\n\
                       arrive_after_minutes = 1.0\nfarewell_minutes = 1.0\n\
                       abilities = [\"raise_earth\", \"water_the_land\", \"hearthflame\", \
                       \"wind_carry\"]\nrefuses = [\"fish\"]\n\
                       [avatar.modifiers]\nmove_speed = 1.6\ncarry_capacity = 1.0\n\
                       earthworks = 2.5\nheating = 2.5\nwater_work = 2.0\n\
                       power_generation = 2.0\nconstruction = 1.5\nstudy = 1.0\nchi_regen = 2.0\n";

/// The bundled data with a quick two-stage gate (foundation, then pillars; neither has a task),
/// and no `[avatar]` section: there is nobody to arrive.
fn defs_without_avatar() -> Defs {
    Defs::bundled()
        .with_endeavor(
            "[endeavor]\nname = \"Gate\"\nbuilding = \"harmony_gate\"\n\
             [[stage]]\nid = \"foundation\"\nname = \"F\"\n\
             cost = [{ item = \"stone\", count = 4 }]\nwork = 2.0\n\
             [[stage]]\nid = \"pillars\"\nname = \"P\"\nwork = 2.0\n",
        )
        .expect("test data is valid")
}

/// The bundled data with a quick gate: a foundation, then a final stage whose own construct work
/// is deliberately never finished within these tests (real construct seconds, scaled by
/// `construction`, would take far longer than a few thousand ticks), so the site stays
/// `Building` (and its anchor findable) for as long as these tests need it -- exactly as the real
/// gate's Arch waits on the Avatar's beam lift, just for a plainer reason. It carries a task only
/// the Avatar can do (unreachable here, since the stage's own work never finishes either), so the
/// "needs an [avatar] section" check still has something to check.
fn defs_with_avatar() -> Defs {
    Defs::bundled()
        .with_endeavor(&format!(
            "[endeavor]\nname = \"Gate\"\nbuilding = \"harmony_gate\"\n\
             [[stage]]\nid = \"foundation\"\nname = \"F\"\n\
             cost = [{{ item = \"stone\", count = 4 }}]\nwork = 2.0\n\
             [[stage]]\nid = \"arch\"\nname = \"A\"\nwork = 1000000.0\n\
             [[stage.task]]\nid = \"t\"\nname = \"T\"\nelement = \"avatar\"\n\
             chi = 10.0\nwork = 1.0\n{AVATAR}"
        ))
        .expect("test data is valid")
}

/// A bare field with `units` units of `nation` along the top row, a stockpile of stone (enough
/// for the foundation), and the endeavor Ready.
fn world(nation: Nation, units: i32, defs: &Defs) -> World {
    let spawns: Vec<Pos> = (0..units).map(|x| at(x, 0)).collect();
    let mut w = World::with_map(1, field(24, 10), nation, &spawns, defs);
    let stone = defs.item_id("stone").expect("item exists");
    w.zones.insert(at(1, 8));
    w.stored.insert(at(1, 8), Stack { item: stone, count: 40 });
    w.endeavor = EndeavorState::Ready { tick: 0 };
    w
}

fn begin(w: &mut World, defs: &Defs) {
    w.apply(Command::BeginEndeavor { anchor: ANCHOR }, defs).expect("site is free");
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for n in 0..limit {
        if done(w) {
            return n;
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks: avatar is {:?}", w.avatar);
}

fn avatar_unit(w: &World) -> Option<&Unit> {
    w.units.iter().find(|u| u.is_avatar())
}

#[test]
fn he_never_arrives_without_the_avatar_data_or_a_gate() {
    let defs = defs_without_avatar();
    let mut w = world(Nation::Earth, 2, &defs);
    begin(&mut w, &defs);
    for _ in 0..3_000 {
        w.tick(&defs);
    }
    assert_eq!(w.avatar, AvatarState::NotYetArrived);
    assert!(avatar_unit(&w).is_none());
}

#[test]
fn arrival_tick_and_tile_are_deterministic() {
    let defs = defs_with_avatar();
    let play = || {
        let mut w = world(Nation::Earth, 2, &defs);
        begin(&mut w, &defs);
        run_until(&mut w, &defs, 5_000, |w| avatar_unit(w).is_some());
        (w.tick, avatar_unit(&w).unwrap().pos, avatar_events(&w))
    };
    let (tick, pos, events) = play();
    assert_eq!((tick, pos, events.clone()), play());
    // West of the site, as the design says.
    assert!(pos.x < ANCHOR.x);
    assert_eq!(events, [EndeavorEventKind::AvatarArrived]);
}

/// A save made while he's here loads, and newcomers (#232) still get fresh ids: his fixed id
/// (u32::MAX) mustn't feed the id counter's fix-up on load.
#[test]
fn a_save_with_him_present_loads_and_keeps_counting_settler_ids() {
    let defs = defs_with_avatar();
    let mut w = world(Nation::Earth, 2, &defs);
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 5_000, |w| avatar_unit(w).is_some());
    let text = w.save(&defs).expect("saves");
    let loaded = World::load(&text, &defs).expect("loads");
    assert!(avatar_unit(&loaded).is_some());
    assert_eq!(loaded.next_unit, 2, "the two settlers' ids are 0 and 1");
}

fn avatar_events(w: &World) -> Vec<EndeavorEventKind> {
    w.endeavor_events()
        .map(|e| e.kind.clone())
        .filter(|k| {
            matches!(
                k,
                EndeavorEventKind::AvatarArrived
                    | EndeavorEventKind::AvatarLeft(_)
                    | EndeavorEventKind::AvatarReturned
            )
        })
        .collect()
}

#[test]
fn he_takes_a_designated_raise_earth_in_a_fire_world() {
    let defs = defs_with_avatar();
    let mut w = world(Nation::Fire, 1, &defs);
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 5_000, |w| avatar_unit(w).is_some());
    let river = at(5, 2);
    w.map[river].terrain = Terrain::River;
    w.map_changed();
    let raise_earth = defs.ability_id("raise_earth").unwrap();
    // No Fire unit can cast an Earth ability, but the Avatar, here, can: it is accepted.
    assert_eq!(w.check_ability(raise_earth, &defs), Ok(()));
    w.apply(Command::Bend { ability: raise_earth, pos: river }, &defs).expect("designated");
    run_until(&mut w, &defs, 2_000, |w| w.map[river].terrain == Terrain::Ford);
    assert_eq!(avatar_unit(&w).unwrap().bends(), 1);
}

#[test]
fn he_leaves_when_unfed_and_comes_back() {
    let defs = defs_with_avatar();
    let mut w = world(Nation::Water, 1, &defs);
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 5_000, |w| avatar_unit(w).is_some());
    // No food anywhere: he can never resolve his hunger himself.
    w.stored.clear();
    let i = w.units.iter().position(Unit::is_avatar).unwrap();
    w.units[i].needs.hunger = defs.needs.hunger.low;
    run_until(&mut w, &defs, 2_000, |w| matches!(w.avatar, AvatarState::Away { .. }));
    assert!(avatar_unit(&w).is_none());
    assert_eq!(
        avatar_events(&w),
        [
            EndeavorEventKind::AvatarArrived,
            EndeavorEventKind::AvatarLeft(AvatarLeaveReason::Hungry)
        ]
    );
    assert_eq!(w.care().close_calls(), 1, "his leaving counts as one close call (#533, #520)");
    run_until(&mut w, &defs, 2_000, |w| avatar_unit(w).is_some());
    assert_eq!(w.avatar, AvatarState::Present);
    assert_eq!(avatar_unit(&w).unwrap().needs.hunger, defs.needs.max());
    assert_eq!(avatar_events(&w).last(), Some(&EndeavorEventKind::AvatarReturned));
}

#[test]
fn he_skips_fish() {
    let defs = defs_with_avatar();
    let mut w = world(Nation::Water, 1, &defs);
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 5_000, |w| avatar_unit(w).is_some());
    let fish = defs.item_id("fish").unwrap();
    w.stored.clear();
    let pos = avatar_unit(&w).unwrap().pos;
    w.zones.insert(pos);
    w.stored.insert(pos, Stack { item: fish, count: 10 });
    let i = w.units.iter().position(Unit::is_avatar).unwrap();
    let before = defs.needs.hunger.low - 1.0;
    w.units[i].needs.hunger = before;
    for _ in 0..600 {
        w.tick(&defs);
    }
    // Starving right beside a full stockpile of fish: he never touches it. Either his hunger
    // only ever fell (he refused to eat it), or his patience ran out and he has gone home.
    match avatar_unit(&w) {
        Some(u) => assert!(u.needs.hunger <= before, "{}", u.needs.hunger),
        None => assert!(matches!(w.avatar, AvatarState::Away { .. })),
    }
}

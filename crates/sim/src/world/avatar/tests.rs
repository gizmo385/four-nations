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

/// #1001: he lands where the gate's builders can reach him, not in the map's top-left corner.
/// A strip of land west of a sea channel holds the first free tiles in row order; he must land
/// on the gate's own side of it, west of the site, and be able to walk to it.
#[test]
fn he_lands_on_the_gates_side_of_the_water_not_in_the_corner() {
    let defs = defs_with_avatar();
    let mut map = field(24, 10);
    for y in 0..10 {
        map.get_mut(at(4, y)).expect("on the map").terrain = Terrain::Water;
    }
    let spawns = [at(8, 0), at(9, 0)];
    let mut w = World::with_map(1, map, Nation::Earth, &spawns, &defs);
    let stone = defs.item_id("stone").expect("item exists");
    w.zones.insert(at(8, 8));
    w.stored.insert(at(8, 8), Stack { item: stone, count: 40 });
    w.endeavor = EndeavorState::Ready { tick: 0 };
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 5_000, |w| avatar_unit(w).is_some());
    let pos = avatar_unit(&w).unwrap().pos;
    assert!(pos.x > 4 && pos.x < ANCHOR.x, "{pos:?}: across the channel or not west");
    let flood = Flood::new(&w.map, pos, &w.rules);
    assert!(flood.cost(ANCHOR).is_some(), "{pos:?} can't reach the gate");
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

/// `defs_with_avatar`, whose last stage's task is `element`'s and costs `chi` (#1011).
fn defs_with_task(element: &str, chi: f32) -> Defs {
    Defs::bundled()
        .with_endeavor(&format!(
            "[endeavor]\nname = \"Gate\"\nbuilding = \"harmony_gate\"\n\
             [[stage]]\nid = \"foundation\"\nname = \"F\"\n\
             cost = [{{ item = \"stone\", count = 4 }}]\nwork = 2.0\n\
             [[stage]]\nid = \"pillars\"\nname = \"P\"\nwork = 1000000.0\n\
             [[stage.task]]\nid = \"t\"\nname = \"T\"\nelement = \"{element}\"\n\
             chi = {chi:.1}\nwork = 1.0\n{AVATAR}"
        ))
        .expect("test data is valid")
}

#[test]
fn he_keeps_the_chi_a_task_only_he_can_do_needs() {
    let raise_earth = Defs::bundled().ability_id("raise_earth").unwrap();
    let cost = Defs::bundled().ability(raise_earth).chi;
    // (task's element, the world's nation, whether he must keep the task's chi back)
    for (element, nation, keeps) in [
        ("avatar", Nation::Fire, true),
        ("earth", Nation::Fire, true),
        ("earth", Nation::Earth, false),
    ] {
        let defs = defs_with_task(element, 100.0);
        let mut w = world(nation, 1, &defs);
        begin(&mut w, &defs);
        run_until(&mut w, &defs, 5_000, |w| avatar_unit(w).is_some());
        let i = w.units.iter().position(Unit::is_avatar).unwrap();
        let reserve = if keeps { 100.0 } else { 0.0 };
        assert_eq!(w.avatar_chi_reserve(&defs), reserve, "{element} task, {nation:?} world");
        w.units[i].chi = cost + reserve - 1.0;
        assert!(!w.unit_can_bend(i, raise_earth, &defs), "{element}, {nation:?}: below");
        w.units[i].chi = cost + reserve;
        assert!(w.unit_can_bend(i, raise_earth, &defs), "{element}, {nation:?}: enough");
        // Once the task is done, nothing is held back.
        w.gate_mut().unwrap().tasks[0].done = true;
        assert_eq!(w.avatar_chi_reserve(&defs), 0.0);
    }
}

#[test]
fn he_never_spends_the_pillars_chi_on_bends_only_he_can_cast() {
    // A Fire world with an Earth task: only he can raise it. Its stone isn't in stores yet, so
    // it can't open; a raise_earth is designated meanwhile.
    let defs = Defs::bundled()
        .with_endeavor(&format!(
            "[endeavor]\nname = \"Gate\"\nbuilding = \"harmony_gate\"\n\
             [[stage]]\nid = \"foundation\"\nname = \"F\"\n\
             cost = [{{ item = \"stone\", count = 4 }}]\nwork = 2.0\n\
             [[stage]]\nid = \"pillars\"\nname = \"P\"\nwork = 2.0\n\
             [[stage.task]]\nid = \"earth_pillar\"\nname = \"E\"\nelement = \"earth\"\n\
             cost = [{{ item = \"stone\", count = 60 }}]\nchi = 100.0\nwork = 1.0\n\
             [[stage]]\nid = \"arch\"\nname = \"A\"\nwork = 1000000.0\n{AVATAR}"
        ))
        .expect("test data is valid");
    let mut w = world(Nation::Fire, 1, &defs);
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 5_000, |w| avatar_unit(w).is_some());
    let river = at(5, 2);
    w.map[river].terrain = Terrain::River;
    w.map_changed();
    let raise_earth = defs.ability_id("raise_earth").unwrap();
    w.apply(Command::Bend { ability: raise_earth, pos: river }, &defs).expect("designated");
    let i = w.units.iter().position(Unit::is_avatar).unwrap();
    // Enough for the bend alone, not for the bend and the pillar after it.
    w.units[i].chi = 140.0;
    for _ in 0..3_000 {
        let chi = avatar_unit(&w).map_or(200.0, |u| u.chi);
        assert!(chi >= 100.0, "he spent the pillar's chi: {chi} at tick {}", w.tick);
        w.tick(&defs);
    }
    // His chi came back past 150 and he did cast it, still keeping the pillar's 100.
    assert_eq!(w.map[river].terrain, Terrain::Ford, "he bends once he can spare it");
    // The stone comes in, and he raises the pillar.
    let stone = defs.item_id("stone").unwrap();
    w.stored.insert(at(2, 8), Stack { item: stone, count: 60 });
    w.zones.insert(at(2, 8));
    run_until(&mut w, &defs, 20_000, |w| {
        w.endeavor_events().any(|e| e.kind == EndeavorEventKind::StageDone("pillars".into()))
    });
}

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
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

/// Where the tests put the gate: a 4x4 footprint in the middle of the field.
const ANCHOR: Pos = Pos::new(6, 3);

/// The bundled data with the gate and `stages` as the endeavor, and no signs.
fn defs_with(stages: &str) -> Defs {
    Defs::bundled()
        .with_endeavor(&format!(
            "[endeavor]\nname = \"Gate\"\nbuilding = \"harmony_gate\"\n{stages}"
        ))
        .expect("test stages are valid")
}

/// Short stages: some work, then the given pillars, then some more work.
fn three(pillars: &[Nation]) -> String {
    let mut out = String::from(
        "[[stage]]\nid = \"base\"\nname = \"Base\"\ncost = [{ item = \"stone\", count = 4 }]\n\
         work = 2.0\n\n[[stage]]\nid = \"pillars\"\nname = \"Pillars\"\n",
    );
    for n in pillars {
        let n = n.as_str();
        out.push_str(&format!(
            "[[stage.task]]\nid = \"{n}_pillar\"\nname = \"{n} pillar\"\nelement = \"{n}\"\n\
             cost = [{{ item = \"stone\", count = 2 }}]\nchi = 50.0\nwork = 3.0\n"
        ));
    }
    out.push_str(
        "\n[[stage]]\nid = \"arch\"\nname = \"Arch\"\ncost = [{ item = \"wood\", count = 3 }]\n\
         work = 2.0\n",
    );
    out
}

/// Earth and Fire pillars: an Earth settlement can finish only one of them.
fn earth_and_fire() -> Defs {
    defs_with(&three(&[Nation::Earth, Nation::Fire]))
}

/// Only an Earth pillar: an Earth settlement can finish the whole gate.
fn earth_only() -> Defs {
    defs_with(&three(&[Nation::Earth]))
}

/// A bare 16x10 field with `units` units of `nation` along the top row, a stockpile of stone and
/// one of wood, and the endeavor Ready.
fn world(nation: Nation, units: i32, defs: &Defs) -> World {
    let spawns: Vec<Pos> = (0..units).map(|x| at(x, 0)).collect();
    let mut w = World::with_map(1, field(16, 10), nation, &spawns, defs);
    for (pos, item) in [(at(1, 8), "stone"), (at(2, 8), "wood")] {
        w.zones.insert(pos);
        let item = defs.item_id(item).expect("item exists");
        w.stored.insert(pos, Stack { item, count: 40 });
    }
    w.endeavor = EndeavorState::Ready { tick: 0 };
    w
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for n in 0..limit {
        if done(w) {
            return n;
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks: {:?}", w.endeavor());
}

fn stage_id(w: &World) -> Option<&str> {
    w.gate().map(|g| g.stage.as_str())
}

fn begin(w: &mut World, defs: &Defs) {
    w.apply(Command::BeginEndeavor { anchor: ANCHOR }, defs).expect("site is free");
}

fn stages_done(w: &World) -> Vec<String> {
    w.endeavor_events()
        .iter()
        .filter_map(|e| match &e.kind {
            EndeavorEventKind::StageDone(id) => Some(id.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_site_needs_a_ready_endeavor_and_free_ground() {
    let defs = earth_and_fire();
    let mut w = world(Nation::Earth, 2, &defs);
    w.endeavor = EndeavorState::Dormant;
    let err = w.apply(Command::BeginEndeavor { anchor: ANCHOR }, &defs);
    assert_eq!(err, Err(CommandError::Endeavor(EndeavorError::NotReady)));
    w.endeavor = EndeavorState::Ready { tick: 0 };
    // The footprint runs off the map's right edge.
    let err = w.check_endeavor_site(at(13, 3), &defs);
    assert_eq!(err, Err(EndeavorError::CannotBuild { at: at(16, 3), why: Blocker::OffMap }));
    let err = w.check_endeavor_site(at(1, 5), &defs);
    assert_eq!(err, Err(EndeavorError::CannotBuild { at: at(1, 8), why: Blocker::Stockpile }));
    // Data with no stages has no gate.
    let bare = Defs::bundled().with_endeavor("[endeavor]\nname = \"G\"\n").unwrap();
    assert_eq!(w.check_endeavor_site(ANCHOR, &bare), Err(EndeavorError::NoGate));

    begin(&mut w, &defs);
    let EndeavorState::Building(g) = w.endeavor() else { panic!("{:?}", w.endeavor()) };
    assert_eq!((g.anchor, g.ready_tick, g.stage.as_str()), (ANCHOR, 0, "base"));
    assert_eq!(w.gate_stage(&defs).map(|(n, s)| (n, s.name.as_str())), Some((0, "Base")));
    assert_eq!(w.footprint_at(at(9, 6)), Some(Footprint::new(ANCHOR, 4, 4)));
    assert!(w.jobs().iter().any(|j| j.kind == JobKind::Build { pos: ANCHOR }));
    // Placed once only.
    let again = w.apply(Command::BeginEndeavor { anchor: at(0, 4) }, &defs);
    assert_eq!(again, Err(CommandError::Endeavor(EndeavorError::NotReady)));
}

#[test]
fn the_site_keeps_other_buildings_off_and_is_not_an_ordinary_site() {
    let defs = earth_and_fire();
    let mut w = world(Nation::Earth, 2, &defs);
    begin(&mut w, &defs);
    let bed = defs.building_id("bed").unwrap();
    let err = w.apply(Command::DesignateBuild { pos: at(7, 4), building: bed }, &defs);
    let blocked = BuildError::CannotBuild { at: at(7, 4), why: Blocker::Site };
    assert_eq!(err, Err(CommandError::Build(blocked)));
    assert_eq!(w.build_sites().count(), 0);
    assert!(w.build_site(at(7, 4)).is_none());
    let err = w.apply(Command::CancelBuild { pos: at(7, 4) }, &defs);
    assert_eq!(err, Err(CommandError::Build(BuildError::Endeavor)));
}

#[test]
fn the_gate_is_never_designated_like_a_building() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, 1, &defs);
    let gate = defs.building_id("harmony_gate").unwrap();
    let err = w.apply(Command::DesignateBuild { pos: ANCHOR, building: gate }, &defs);
    assert_eq!(err, Err(CommandError::Build(BuildError::Endeavor)));
}

#[test]
fn an_earth_settlement_raises_its_own_pillar_but_not_the_fire_one() {
    let defs = earth_and_fire();
    let mut w = world(Nation::Earth, 3, &defs);
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 3_000, |w| {
        w.gate().is_some_and(|g| g.tasks.iter().any(|t| t.id == "earth_pillar" && t.done))
    });
    assert_eq!(stage_id(&w), Some("pillars"));
    let raiser = w.units.iter().find(|u| u.bends == 1).expect("someone raised it");
    assert!(raiser.chi < defs.chi.max, "the pillar spent chi");
    // The Fire pillar's materials are in and its job is open, but nobody here can take it.
    let fire = w.gate().unwrap().tasks.iter().position(|t| t.id == "fire_pillar").unwrap() as u8;
    run_until(&mut w, &defs, 100, |w| {
        w.jobs().iter().any(|j| j.kind == JobKind::EndeavorTask { task: fire })
    });
    w.units.iter_mut().for_each(|u| u.chi = defs.chi.max);
    for _ in 0..600 {
        w.tick(&defs);
    }
    let job = w.jobs().iter().find(|j| j.kind == JobKind::EndeavorTask { task: fire }).unwrap();
    assert_eq!(job.claimed_by, None);
    assert_eq!(stage_id(&w), Some("pillars"), "the stage waits for the Fire pillar");
    assert!(stages_done(&w) == ["base"]);
}

#[test]
fn a_pillar_waits_for_a_bender_with_the_chi() {
    let defs = earth_only();
    let mut w = world(Nation::Earth, 2, &defs);
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 3_000, |w| {
        w.jobs().iter().any(|j| matches!(j.kind, JobKind::EndeavorTask { .. }))
    });
    for u in &mut w.units {
        u.chi = 10.0;
    }
    w.tick(&defs);
    let job = w.jobs().iter().find(|j| matches!(j.kind, JobKind::EndeavorTask { .. })).unwrap();
    assert_eq!(job.claimed_by, None, "too little chi to take it");
}

#[test]
fn stages_advance_in_order_and_the_gate_stands_at_the_end() {
    let defs = earth_only();
    let mut w = world(Nation::Earth, 3, &defs);
    begin(&mut w, &defs);
    let mut seen = vec![stage_id(&w).unwrap().to_owned()];
    run_until(&mut w, &defs, 6_000, |w| {
        if let Some(s) = stage_id(w).filter(|s| seen.last().map(String::as_str) != Some(*s)) {
            seen.push(s.to_owned());
        }
        matches!(w.endeavor(), EndeavorState::Complete { .. })
    });
    assert_eq!(seen, ["base", "pillars", "arch"]);
    assert_eq!(stages_done(&w), ["base", "pillars", "arch"]);
    let EndeavorState::Complete { tick } = *w.endeavor() else { unreachable!() };
    assert_eq!(w.endeavor_events().last().unwrap().tick, tick);
    let gate = defs.building_id("harmony_gate").unwrap();
    assert_eq!(w.buildings().collect::<Vec<_>>(), [(Footprint::new(ANCHOR, 4, 4), gate)]);
    assert!(Footprint::new(ANCHOR, 4, 4).tiles().all(|p| w.building_at(p) == Some(gate)));
    assert!(w.sites.is_empty());
    assert!(w.jobs().iter().all(|j| !matches!(j.kind, JobKind::EndeavorTask { .. })));
    assert!(!w.jobs().iter().any(|j| j.kind == JobKind::Build { pos: ANCHOR }));
    // Every material was used: 4 + 2 stone, 3 wood.
    let left = |item: &str| w.stored_total(defs.item_id(item).unwrap());
    assert_eq!((left("stone"), left("wood")), (34, 37));
}

/// The finished gate is a permanent heat source, like any building with `[building.heat]`
/// (docs/design/endgame.md, "The transformation: the United City", #240), so meals cooked and
/// sleep taken near it benefit the moment it stands.
#[test]
fn the_finished_gate_is_a_heat_source_once_complete() {
    let defs = earth_only();
    let mut w = world(Nation::Earth, 3, &defs);
    assert!(!w.is_heated(ANCHOR), "no fire before the gate is even begun");
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 6_000, |w| matches!(w.endeavor(), EndeavorState::Complete { .. }));
    // Every tile of the 4x4 plaza is warmed, and `data/buildings.toml`'s radius of 3 (at Earth's
    // heating modifier of 1.0) reaches outward from it too.
    assert!(Footprint::new(ANCHOR, 4, 4).tiles().all(|p| w.is_heated(p)));
    assert!(w.is_heated(at(ANCHOR.x - 3, ANCHOR.y)), "3 tiles out is still within radius 3");
    assert!(!w.is_heated(at(ANCHOR.x - 4, ANCHOR.y)), "4 tiles out is past it");
}

#[test]
fn cancelling_before_work_starts_refunds_what_was_delivered() {
    let defs = defs_with(
        "[[stage]]\nid = \"base\"\nname = \"Base\"\ncost = [{ item = \"stone\", count = 30 }]\n\
         work = 50.0\n",
    );
    let mut w = world(Nation::Earth, 1, &defs);
    assert_eq!(
        w.apply(Command::CancelEndeavor, &defs),
        Err(CommandError::Endeavor(EndeavorError::NotBuilding))
    );
    begin(&mut w, &defs);
    let stone = defs.item_id("stone").unwrap();
    run_until(&mut w, &defs, 2_000, |w| {
        w.gate().is_some_and(|g| count_of(&g.delivered, stone, &defs) > 0)
    });
    let delivered = count_of(&w.gate().unwrap().delivered, stone, &defs);
    assert!(delivered < 30);
    w.apply(Command::CancelEndeavor, &defs).expect("no work yet");
    assert_eq!(*w.endeavor(), EndeavorState::Ready { tick: 0 });
    assert!(w.sites.is_empty());
    assert!(!w.jobs().iter().any(|j| j.kind == JobKind::Build { pos: ANCHOR }));
    let on_ground: u32 =
        w.ground_items().filter(|(_, s)| s.item == stone).map(|(_, s)| s.count).sum();
    assert_eq!(on_ground, delivered);
    // Placed again, and this time worked on: then it can't be cancelled.
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 6_000, |w| w.gate().is_some_and(|g| g.progress > 0.0));
    assert_eq!(
        w.apply(Command::CancelEndeavor, &defs),
        Err(CommandError::Endeavor(EndeavorError::Started))
    );
}

#[test]
fn the_foundation_accepts_brick_in_place_of_stone() {
    // Brick stands in for stone one for one, the gate included (issue #371).
    let defs = defs_with(
        "[[stage]]\nid = \"base\"\nname = \"Base\"\ncost = [{ item = \"stone\", count = 10 }]\n\
         work = 2.0\n",
    );
    let mut w = world(Nation::Fire, 1, &defs);
    let stone = defs.item_id("stone").unwrap();
    let brick = defs.item_id("brick").unwrap();
    // Swap out `world`'s stone stockpile for one of brick.
    w.stored.remove(&at(1, 8));
    w.stored.insert(at(1, 8), Stack { item: brick, count: 40 });
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 5_000, |w| stage_id(w) != Some("base"));
    assert_eq!(w.stored_total(stone), 0, "no stone existed to spend");
    assert_eq!(w.stored_total(brick), 30, "10 of the 40 brick paid the foundation");
}

#[test]
fn the_gate_goes_up_the_same_way_every_time() {
    let defs = earth_only();
    let play = || {
        let mut w = world(Nation::Earth, 3, &defs);
        begin(&mut w, &defs);
        run_until(&mut w, &defs, 6_000, |w| matches!(w.endeavor(), EndeavorState::Complete { .. }));
        let units: Vec<(Pos, u32)> = w.units.iter().map(|u| (u.pos, u.bends)).collect();
        (w.tick, w.endeavor_events().to_vec(), units)
    };
    assert_eq!(play(), play());
}

#[test]
fn replaying_the_log_rebuilds_the_same_gate() {
    let defs = earth_only();
    let mut w = world(Nation::Earth, 2, &defs);
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 6_000, |w| matches!(w.endeavor(), EndeavorState::Complete { .. }));
    let mut again = world(Nation::Earth, 2, &defs);
    again.replay(w.command_log(), w.tick, &defs);
    assert_eq!(again.endeavor(), w.endeavor());
    assert_eq!(again.endeavor_events(), w.endeavor_events());
}

// The Harmony Rite (issue #238; docs/design/endgame.md, "The Harmony Rite").

/// A short `[avatar]` section, so rite tests don't wait out the real arrival or farewell
/// minutes.
const RITE_AVATAR: &str = "[avatar]\nid = \"aang\"\nname = \"Aang\"\ncharacter = \"aang\"\n\
    chi_max = 200.0\npatience_minutes = 1.0\nreturn_minutes = 1.0\n\
    arrive_after_minutes = 0.01\nfarewell_minutes = 0.05\n\
    abilities = [\"raise_earth\", \"water_the_land\", \"hearthflame\", \"wind_carry\"]\n\
    refuses = [\"fish\"]\n\
    [avatar.modifiers]\nmove_speed = 1.6\ncarry_capacity = 1.0\nearthworks = 2.5\n\
    heating = 2.5\nwater_work = 2.0\npower_generation = 2.0\nconstruction = 1.5\n\
    study = 1.0\nchi_regen = 2.0\n";

/// The bundled data with a quick, costless foundation, then the Harmony Rite with `minutes` and
/// `attendance_share`, and the Avatar.
fn defs_with_rite(minutes: f32, share: f32) -> Defs {
    Defs::bundled()
        .with_endeavor(&format!(
            "[endeavor]\nname = \"Gate\"\nbuilding = \"harmony_gate\"\n\
             [[stage]]\nid = \"base\"\nname = \"Base\"\nwork = 0.01\n\
             [[stage]]\nid = \"rite\"\nname = \"R\"\n\
             [stage.rite]\nminutes = {minutes}\nattendance_share = {share}\n{RITE_AVATAR}"
        ))
        .expect("test stages are valid")
}

fn rite_events(w: &World) -> Vec<&EndeavorEventKind> {
    w.endeavor_events()
        .iter()
        .map(|e| &e.kind)
        .filter(|k| {
            matches!(
                k,
                EndeavorEventKind::RitePaused
                    | EndeavorEventKind::RiteBroken
                    | EndeavorEventKind::Complete
                    | EndeavorEventKind::AvatarFarewell
            )
        })
        .collect()
}

#[test]
fn the_rite_completes_with_enough_attendance_and_the_avatar_eventually_departs() {
    let defs = defs_with_rite(0.05, 0.5); // 3 seconds of attendance, out of 2 settlers.
    let mut w = world(Nation::Earth, 2, &defs);
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 6_000, |w| matches!(w.endeavor(), EndeavorState::Complete { .. }));
    assert_eq!(stages_done(&w), ["base", "rite"]);
    // A brief pause while the Avatar walks over and the first settlers claim an Attend job is
    // normal; what matters is that it completes.
    assert_eq!(rite_events(&w).last(), Some(&&EndeavorEventKind::Complete));
    assert!(matches!(w.avatar(), AvatarState::Farewell { .. }), "{:?}", w.avatar());
    // He stays for the celebration, still a unit, then leaves for good.
    assert!(w.units.iter().any(Unit::is_avatar));
    run_until(&mut w, &defs, 2_000, |w| matches!(w.avatar(), AvatarState::Departed));
    assert!(!w.units.iter().any(Unit::is_avatar));
    assert_eq!(rite_events(&w).last(), Some(&&EndeavorEventKind::AvatarFarewell));
}

/// The rite pauses while too few attend, but a full plaza is always enough: 20 settlers at a 0.9
/// share would need 18, more than the 4x4 plaza's 16 places, and population growth (#232) can
/// outgrow the plaza the same way. Without the cap the rite would pause for good.
#[test]
fn a_full_plaza_is_enough_even_when_the_share_asks_for_more() {
    let defs = defs_with_rite(0.05, 0.9);
    let spawns: Vec<Pos> = (0..20).map(|x| at(x, 0)).collect();
    let mut w = World::with_map(1, field(24, 10), Nation::Earth, &spawns, &defs);
    w.endeavor = EndeavorState::Ready { tick: 0 };
    w.apply(Command::BeginEndeavor { anchor: at(18, 3) }, &defs).expect("site is free");
    run_until(&mut w, &defs, 3_000, |w| stage_id(w) == Some("rite"));
    run_until(&mut w, &defs, 6_000, |w| matches!(w.endeavor(), EndeavorState::Complete { .. }));
    let events = rite_events(&w);
    assert_eq!(events.first(), Some(&&EndeavorEventKind::RitePaused), "paused while gathering");
    assert_eq!(events.last(), Some(&&EndeavorEventKind::Complete));
}

#[test]
fn a_death_during_the_rite_resets_its_progress() {
    let defs = defs_with_rite(0.2, 0.5); // 12 seconds of attendance, out of 3 settlers.
    let mut w = world(Nation::Earth, 3, &defs);
    begin(&mut w, &defs);
    run_until(&mut w, &defs, 3_000, |w| stage_id(w) == Some("rite"));
    run_until(&mut w, &defs, 2_000, |w| w.gate().is_some_and(|g| g.rite_progress > 0.0));
    assert!(w.gate().unwrap().rite_progress > 0.0);
    // Fully depleted, not just low, so health takes damage this very tick instead of regenerating
    // (`Needs::tick_scaled`): it dies for certain, deterministically, on the next tick.
    w.units[0].needs.hunger = 0.0;
    w.units[0].needs.health = 0.0;
    w.tick(&defs);
    assert_eq!(w.deaths.len(), 1);
    assert_eq!(w.gate().unwrap().rite_progress, 0.0);
    assert_eq!(rite_events(&w).last(), Some(&&EndeavorEventKind::RiteBroken));
}

#[test]
fn the_whole_ready_to_complete_run_is_deterministic() {
    let defs = defs_with_rite(0.05, 0.5);
    let play = || {
        let mut w = world(Nation::Earth, 3, &defs);
        begin(&mut w, &defs);
        run_until(&mut w, &defs, 6_000, |w| matches!(w.endeavor(), EndeavorState::Complete { .. }));
        (w.tick, w.endeavor_events().to_vec())
    };
    assert_eq!(play(), play());
}

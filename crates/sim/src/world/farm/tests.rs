use super::*;
use crate::building::{Building, FarmDef, IrrigationDef};
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::jobs::JobKind;
use crate::mapgen::Tile;
use crate::world::command::Command;
use crate::world::work::DesignateError;

fn arena(size: i32) -> TileMap {
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

const SIZE: i32 = 15;
const MID: Pos = Pos::new(7, 7);

fn setup(nation: Nation, seed: u64, units: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    (World::with_map(seed, arena(SIZE), nation, units, &defs), defs)
}

fn farm_def(defs: &Defs) -> FarmDef {
    defs.building(defs.building_id("farm_plot").unwrap()).farm.expect("the farm plot grows cabbage")
}

/// Puts a finished, empty farm plot on `pos`, as if built.
fn raise_farm_plot(w: &mut World, defs: &Defs, pos: Pos) {
    let id = defs.building_id("farm_plot").unwrap();
    let def = defs.building(id);
    w.map[pos].building = Some(Building::new(id, def.passable, false));
    w.open_farm_plot(pos);
}

fn irrigation_def(defs: &Defs) -> IrrigationDef {
    defs.building(defs.building_id("irrigation_channel").unwrap())
        .irrigation
        .expect("the channel speeds up plots")
}

/// Puts a finished irrigation channel on `pos`, as if built.
fn raise_irrigation_channel(w: &mut World, defs: &Defs, pos: Pos) {
    let id = defs.building_id("irrigation_channel").unwrap();
    let def = defs.building(id);
    w.map[pos].building = Some(Building::new(id, def.passable, false));
    w.open_irrigation_channel(pos);
}

fn secs(s: f32) -> u64 {
    (s * TICKS_PER_SECOND as f32).ceil() as u64
}

fn run(w: &mut World, defs: &Defs, ticks: u64) {
    for _ in 0..ticks {
        w.tick(defs);
    }
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

#[test]
fn a_sown_plot_ripens_after_its_growth_time() {
    let (mut w, defs) = setup(Nation::Earth, 1, &[at(0, 0)]);
    let def = farm_def(&defs);
    raise_farm_plot(&mut w, &defs, MID);
    assert_eq!(w.plot_stage(MID), Some(PlotStage::Empty));

    w.apply(Command::Sow { pos: MID }, &defs).unwrap();
    // Sown (walk + work), then growing for the designed time.
    run_until(&mut w, &defs, secs(def.sow_seconds) + 200, |w| {
        w.plot_stage(MID) == Some(PlotStage::Growing)
    });
    let grow = secs(def.grow_seconds);
    run(&mut w, &defs, grow - 20);
    assert_eq!(w.plot_stage(MID), Some(PlotStage::Growing), "not ripe a little early");
    run(&mut w, &defs, 40);
    assert_eq!(w.plot_stage(MID), Some(PlotStage::Ripe), "ripe on schedule");
}

#[test]
fn a_ripe_plot_is_reaped_automatically_and_its_crop_reaches_a_stockpile() {
    let (mut w, defs) = setup(Nation::Earth, 2, &[at(0, 0), at(1, 0)]);
    let def = farm_def(&defs);
    let cabbage = def.crop;
    raise_farm_plot(&mut w, &defs, MID);
    w.designate_stockpile(at(0, 14)).unwrap();
    w.apply(Command::Sow { pos: MID }, &defs).unwrap();

    let ripe = def.sow_seconds + def.grow_seconds;
    run_until(&mut w, &defs, secs(ripe) + 100, |w| w.plot_stage(MID) == Some(PlotStage::Ripe));
    // Nobody designated a Reap: it's queued on its own (#81-style), and a unit works it with no
    // further orders.
    assert!(w.jobs().iter().any(|j| j.kind == JobKind::Reap { pos: MID }));
    run_until(&mut w, &defs, secs(def.reap_seconds) + 200, |w| w.stored_total(cabbage) > 0);
    assert_eq!(w.stored_total(cabbage), def.harvest_count);
    // The plot is empty again, ready to sow once more.
    assert_eq!(w.plot_stage(MID), Some(PlotStage::Empty));
    assert_eq!(w.tallies().crops_harvested(), def.harvest_count);
}

#[test]
fn crop_growth_scales_ripening_time() {
    let ripen_ticks = |crop_growth: f32| {
        let mut defs = Defs::bundled();
        let id = defs.building_id("farm_plot").unwrap();
        defs.buildings[id.0 as usize].farm.as_mut().unwrap().crop_growth = crop_growth;
        let (mut w, _) = (World::with_map(1, arena(SIZE), Nation::Earth, &[at(0, 0)], &defs), ());
        raise_farm_plot(&mut w, &defs, MID);
        w.farm_plots.get_mut(&MID).unwrap().stage = PlotStage::Growing;
        w.farm_plots.get_mut(&MID).unwrap().growing_left = farm_def(&defs).grow_seconds;
        run_until(&mut w, &defs, secs(farm_def(&defs).grow_seconds) + 50, |w| {
            w.plot_stage(MID) == Some(PlotStage::Ripe)
        })
    };
    let plain = ripen_ticks(1.0);
    let boosted = ripen_ticks(2.0);
    assert!(boosted < plain, "{boosted} should be well under {plain}");
    // Close to exactly half: growth is linear in the rate.
    assert!(boosted * 2 <= plain + 2, "boosted {boosted} vs plain {plain}");
}

#[test]
fn sowing_is_refused_off_a_plot_and_on_one_that_isnt_empty() {
    let (mut w, defs) = setup(Nation::Earth, 1, &[at(0, 0)]);
    assert_eq!(
        w.apply(Command::Sow { pos: MID }, &defs),
        Err(CommandError::Designate(DesignateError::NoFarmPlot))
    );
    raise_farm_plot(&mut w, &defs, MID);
    w.apply(Command::Sow { pos: MID }, &defs).unwrap();
    assert_eq!(
        w.apply(Command::Sow { pos: MID }, &defs),
        Err(CommandError::Designate(DesignateError::AlreadyDesignated))
    );
    w.farm_plots.get_mut(&MID).unwrap().stage = PlotStage::Growing;
    assert_eq!(
        w.apply(Command::Sow { pos: MID }, &defs),
        Err(CommandError::Designate(DesignateError::PlotNotEmpty))
    );
}

#[test]
fn cancel_sow_withdraws_the_job_but_not_a_job_that_doesnt_exist() {
    let (mut w, defs) = setup(Nation::Earth, 1, &[at(0, 0)]);
    raise_farm_plot(&mut w, &defs, MID);
    assert_eq!(
        w.apply(Command::CancelSow { pos: MID }, &defs),
        Err(CommandError::Designate(DesignateError::NotDesignated))
    );
    w.apply(Command::Sow { pos: MID }, &defs).unwrap();
    w.apply(Command::CancelSow { pos: MID }, &defs).unwrap();
    assert!(!w.jobs().iter().any(|j| j.kind == JobKind::Sow { pos: MID }));
    assert_eq!(w.plot_stage(MID), Some(PlotStage::Empty));
}

#[test]
fn adopt_farm_plots_registers_a_hand_placed_plot_but_leaves_a_known_one_alone() {
    let (mut w, defs) = setup(Nation::Earth, 1, &[at(0, 0)]);
    let id = defs.building_id("farm_plot").unwrap();
    let def = defs.building(id);
    w.map[MID].building = Some(Building::new(id, def.passable, false));
    assert_eq!(w.plot_stage(MID), None, "not registered until adopted");
    w.adopt_farm_plots(&defs);
    assert_eq!(w.plot_stage(MID), Some(PlotStage::Empty));

    w.farm_plots.get_mut(&MID).unwrap().stage = PlotStage::Growing;
    w.adopt_farm_plots(&defs);
    assert_eq!(w.plot_stage(MID), Some(PlotStage::Growing), "already known, left alone");
}

// ---- crop_growth_rate obeys the season and climate (#311), the same hook World::growth_rate
// uses for wild regrowth ----

/// Ticks in one season (mirrors `World::season_ticks`, private to `world/season.rs`).
fn season_ticks(defs: &Defs) -> u64 {
    (defs.seasons.calendar.season_minutes as f64 * 60.0 * TICKS_PER_SECOND as f64) as u64
}

/// Puts a growing plot on `pos` with `left` seconds to ripen.
fn plant_growing(w: &mut World, pos: Pos, left: f32) {
    let plot = w.farm_plots.get_mut(&pos).unwrap();
    plot.stage = PlotStage::Growing;
    plot.growing_left = left;
}

#[test]
fn a_polar_plot_does_not_ripen_in_winter_while_a_warm_one_does() {
    let ripens = |temperature: u8| {
        let (mut w, defs) = setup(Nation::Earth, 1, &[]);
        w.map[MID].temperature = temperature;
        w.difficulty = Some("normal".into());
        w.tick = 3 * season_ticks(&defs); // winter's first tick
        raise_farm_plot(&mut w, &defs, MID);
        plant_growing(&mut w, MID, farm_def(&defs).grow_seconds);
        // A warm band's winter growth (0.6) is still slower than full speed, so give it twice
        // its plain time, not just a few extra ticks.
        run(&mut w, &defs, secs(farm_def(&defs).grow_seconds) * 2 + 50);
        w.plot_stage(MID) == Some(PlotStage::Ripe)
    };
    assert!(ripens(255), "warm band keeps growing in winter");
    assert!(!ripens(0), "polar winter_growth is 0: it never ripens in winter");
}

#[test]
fn with_no_difficulty_preset_crop_growth_ignores_season_and_climate() {
    let (mut w, defs) = setup(Nation::Earth, 1, &[]);
    assert_eq!(w.difficulty, None);
    w.map[MID].temperature = 0; // polar, which would stall growth with seasons on
    w.tick = 3 * season_ticks(&defs); // what would be winter, if seasons were on
    raise_farm_plot(&mut w, &defs, MID);
    plant_growing(&mut w, MID, farm_def(&defs).grow_seconds);
    run(&mut w, &defs, secs(farm_def(&defs).grow_seconds) + 50);
    assert_eq!(w.plot_stage(MID), Some(PlotStage::Ripe), "identity growth, regardless of band");
}

// ---- the first frost also takes an unharvested ripe crop (#311) ----

#[test]
fn frost_takes_an_unharvested_ripe_crop_on_a_frost_band_tile() {
    let (mut w, defs) = setup(Nation::Earth, 1, &[]);
    w.map[MID].temperature = 0; // polar: frost band
    w.difficulty = Some("normal".into());
    raise_farm_plot(&mut w, &defs, MID);
    w.farm_plots.get_mut(&MID).unwrap().stage = PlotStage::Ripe;
    w.tick = 3 * season_ticks(&defs) - 1; // the tick before winter
    w.tick(&defs); // crosses into winter: the first frost
    assert_eq!(w.plot_stage(MID), Some(PlotStage::Empty), "frost took the unharvested crop");
    assert!(
        w.season_events().iter().any(|e| matches!(e.kind, SeasonEventKind::Frost { count: 1 })),
        "{:?}",
        w.season_events()
    );
}

#[test]
fn with_no_difficulty_preset_frost_never_touches_a_ripe_crop() {
    let (mut w, defs) = setup(Nation::Earth, 1, &[]);
    w.map[MID].temperature = 0; // polar: would be a frost band with seasons on
    raise_farm_plot(&mut w, &defs, MID);
    w.farm_plots.get_mut(&MID).unwrap().stage = PlotStage::Ripe;
    w.tick = 3 * season_ticks(&defs) - 1;
    w.tick(&defs);
    assert_eq!(w.plot_stage(MID), Some(PlotStage::Ripe), "no seasons on, no frost");
}

// ---- irrigation channels speed up farm plots in reach (#126) ----

#[test]
fn a_plot_beside_an_irrigation_channel_ripens_faster_than_one_without() {
    let ripen_ticks = |channel_at: Option<Pos>| {
        let (mut w, defs) = setup(Nation::Water, 1, &[]);
        raise_farm_plot(&mut w, &defs, MID);
        if let Some(pos) = channel_at {
            raise_irrigation_channel(&mut w, &defs, pos);
        }
        plant_growing(&mut w, MID, farm_def(&defs).grow_seconds);
        run_until(&mut w, &defs, secs(farm_def(&defs).grow_seconds) + 50, |w| {
            w.plot_stage(MID) == Some(PlotStage::Ripe)
        })
    };
    let irrigation = irrigation_def(&Defs::bundled());
    let plain = ripen_ticks(None);
    // Within the channel's own radius: at(7, 4) is 3 tiles from MID (7, 7), Chebyshev.
    let irrigated = ripen_ticks(Some(at(MID.x, MID.y - irrigation.radius as i32)));
    assert!(irrigated < plain, "{irrigated} should be well under {plain}");
    // Close to exactly the channel's growth_boost: growth is linear in the rate.
    let want = (plain as f32 / irrigation.growth_boost).round() as u64;
    assert!(irrigated.abs_diff(want) <= 2, "irrigated {irrigated}, want close to {want}");
}

#[test]
fn a_channel_outside_its_radius_does_not_speed_up_a_distant_plot() {
    let (mut w, defs) = setup(Nation::Water, 1, &[]);
    raise_farm_plot(&mut w, &defs, MID);
    let irrigation = irrigation_def(&defs);
    raise_irrigation_channel(&mut w, &defs, at(MID.x, MID.y - irrigation.radius as i32 - 1));
    assert_eq!(w.irrigation_rate(MID, &defs), 1.0, "one tile beyond radius: no boost");
}

#[test]
fn two_channels_in_reach_of_one_plot_dont_stack() {
    let (mut w, defs) = setup(Nation::Water, 1, &[]);
    raise_farm_plot(&mut w, &defs, MID);
    let irrigation = irrigation_def(&defs);
    raise_irrigation_channel(&mut w, &defs, at(MID.x, MID.y - irrigation.radius as i32));
    raise_irrigation_channel(&mut w, &defs, at(MID.x + irrigation.radius as i32, MID.y));
    assert_eq!(w.irrigation_rate(MID, &defs), irrigation.growth_boost, "best boost, not the sum");
}

#[test]
fn irrigation_still_speeds_up_growth_in_a_slow_winter_season() {
    // A warm band's winter_growth (0.6) is still slower than full speed, so this composes the
    // channel's boost with a season factor below 1.0, not just with the plain 1.0 default.
    let ripen_ticks = |channel_at: Option<Pos>| {
        let (mut w, defs) = setup(Nation::Water, 1, &[]);
        w.map[MID].temperature = 255; // warm band
        w.difficulty = Some("normal".into());
        w.tick = 3 * season_ticks(&defs); // winter's first tick
        raise_farm_plot(&mut w, &defs, MID);
        if let Some(pos) = channel_at {
            raise_irrigation_channel(&mut w, &defs, pos);
        }
        plant_growing(&mut w, MID, farm_def(&defs).grow_seconds);
        // Winter growth alone is already slower than plain speed, so give it plenty of room.
        run_until(&mut w, &defs, secs(farm_def(&defs).grow_seconds) * 3 + 50, |w| {
            w.plot_stage(MID) == Some(PlotStage::Ripe)
        })
    };
    let irrigation = irrigation_def(&Defs::bundled());
    let plain = ripen_ticks(None);
    let irrigated = ripen_ticks(Some(at(MID.x, MID.y - irrigation.radius as i32)));
    assert!(irrigated < plain, "irrigation still helps in a slow winter: {irrigated} vs {plain}");
    // Composes multiplicatively with the season factor: still close to growth_boost times
    // faster than the same winter without irrigation.
    let want = (plain as f32 / irrigation.growth_boost).round() as u64;
    assert!(irrigated.abs_diff(want) <= 2, "irrigated {irrigated}, want close to {want}");
}

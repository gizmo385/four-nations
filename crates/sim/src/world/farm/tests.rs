use super::*;
use crate::building::{Building, FarmDef};
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

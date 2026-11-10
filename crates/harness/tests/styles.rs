//! Each bundled play style, run for 30 minutes on seed 7 Earth (#536): deterministic, and its
//! distinguishing behaviour shows.

use fn_harness::Styles;
use fn_sim::autoplay::Opening;
use fn_sim::embark::Survey;
use fn_sim::world::{Command, Recorded, TICKS_PER_SECOND};
use fn_sim::{Defs, MapGenConfig, Nation, World};

const SEED: u64 = 7;
const NATION: Nation = Nation::Earth;
const PEOPLE: u32 = 6;
const MINUTES: u64 = 30;

fn run(style: &str, defs: &Defs) -> World {
    let styles = Styles::bundled();
    let player = styles.get(style).expect("bundled style").player(PEOPLE);
    let size = defs.embark.map_size("small").expect("bundled data always has a small map size");
    let cfg =
        MapGenConfig { seed: SEED, width: size.width, height: size.height, ..Default::default() };
    let survey = Survey::new(&cfg, defs);
    let start = survey.default_site(defs);
    let mut world = World::found(survey, NATION, start, PEOPLE, Some("normal".to_owned()), defs)
        .expect("Survey::default_site always names a site World::found accepts");
    for tick in 0..MINUTES * 60 * u64::from(TICKS_PER_SECOND) {
        if Opening::due(tick) {
            player.step(&mut world, defs);
        }
        world.tick(defs);
    }
    world
}

fn count(world: &World, defs: &Defs, building: &str) -> usize {
    let id = defs.building_id(building).expect("bundled building");
    let built = world.buildings().filter(|b| b.id == id).count();
    let sited = world.build_sites().filter(|&(_, b, _)| b == id).count();
    built + sited
}

/// The data's population target with `target` swapped in, so the bed cap is reachable inside
/// 30 minutes (the bundled 16 plus a margin is not, from 6 people).
fn defs_with_population_target(target: u32) -> Defs {
    let text = include_str!("../../../data/endeavor.toml");
    let needle = "kind = \"population\"\ntarget = 16";
    assert!(text.contains(needle), "data/endeavor.toml's population sign changed shape");
    let text = text.replace(needle, &format!("kind = \"population\"\ntarget = {target}"));
    Defs::bundled().with_endeavor(&text).expect("the edited endeavor.toml loads")
}

#[test]
fn every_style_is_deterministic() {
    let defs = Defs::bundled();
    for style in Styles::bundled().iter() {
        let (a, b) = (run(&style.id, &defs), run(&style.id, &defs));
        assert_eq!(a.command_log(), b.command_log(), "{} diverged between two runs", style.id);
        assert_eq!(a.units.len(), b.units.len(), "{}", style.id);
    }
}

#[test]
fn careless_tills_no_farm_plot() {
    let defs = Defs::bundled();
    assert_eq!(count(&run("careless", &defs), &defs, "farm_plot"), 0);
}

#[test]
fn careless_builds_no_herb_garden_and_never_researches_herbalism() {
    let defs = Defs::bundled();
    let herbalism = defs.tech_id("herbalism").expect("bundled tech");
    let world = run("careless", &defs);
    assert_eq!(count(&world, &defs, "herb_garden"), 0);
    assert!(!world.research().is_known(herbalism));
    assert!(!researched(&world).contains(&herbalism));
}

#[test]
fn steady_beds_stop_at_the_cap_and_careless_beds_do_not() {
    // Population target 4: steady's cap is 4 + its margin of 2.
    let defs = defs_with_population_target(4);
    let steady = run("steady", &defs);
    assert_eq!(count(&steady, &defs, "bed"), 6, "steady builds up to the cap and stops there");
    let careless = run("careless", &defs);
    assert!(
        count(&careless, &defs, "bed") > 6,
        "careless keeps building beds past the cap, got {}",
        count(&careless, &defs, "bed")
    );
}

#[test]
fn branch_first_researches_a_branch_tech_first_and_steady_does_not() {
    let defs = Defs::bundled();
    let first = |style: &str| {
        let world = run(style, &defs);
        let tech = *researched(&world).first().expect("the style set a research target");
        defs.tech(tech).nation
    };
    // The first target is the nation's own branch tech under branch_first, a shared one under
    // steady (`Economy::research_plan`); both pull in whatever shared prereqs they need.
    assert_eq!(first("branch_first"), Some(NATION));
    assert_eq!(first("steady"), None);
}

#[test]
fn idle_issues_no_command_at_all() {
    let defs = Defs::bundled();
    let world = run("idle", &defs);
    assert!(world.command_log().is_empty());
    assert!(world.build_sites().next().is_none());
    assert!(world.research().queue().is_empty());
    assert_eq!(world.units.len(), PEOPLE as usize, "nobody is born without beds being built");
}

/// The techs the run set as research targets, in order.
fn researched(world: &World) -> Vec<fn_sim::research::TechId> {
    world
        .command_log()
        .iter()
        .filter_map(|&Recorded { command, .. }| match command {
            Command::SetResearch { tech } => Some(tech),
            _ => None,
        })
        .collect()
}

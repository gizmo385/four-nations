//! Reference goldens (#455): eleven whole-screen snapshots, one per screen type, kept for a reviewer
//! to look at -- not the regression net. That job belongs to the layout properties
//! (`layout_properties.rs`, #454) and the `contains`-style assertions beside each feature.
//!
//! Every screen here is built from a small hand-built world ([`fixed_map`]), not mapgen: a
//! stretch of plains with two oak trees, one fishing spot in a pond and two finished beds, with
//! the two settlers' names written in ([`SETTLERS`]) instead of drawn from `data/nations.toml`'s
//! pool, and the clock paused, so nothing ticks and nothing computed from a rate can drift. The
//! title screen is a menu; embark surveys the same hand-built map ([`embark_shell`]) through
//! `Survey::from_map`. So changing the settler-name pool, mapgen or a balance rate changes none
//! of them -- [`retuned_names_rates_and_seeds_change_no_reference_frame`] proves it in memory
//! and the PR's scripted check proves it against the real `data/` and `mapgen` sources.
//!
//! What can still change a frame is the TUI itself, and the data a screen prints on purpose: a
//! building's cost in the build list, a tech's price on the research screen, the endeavor's
//! targets, the nation's traits.
//!
//! Rewrite with `FN_BLESS=1 cargo test -p fn_tui --test reference_goldens` (review the diff)
//! when the TUI changes.

use fn_sim::building::BuildingId;
use fn_sim::embark::{MapSizeDef, Survey, SurveyMode};
use fn_sim::geom::Footprint;
use fn_sim::{Defs, Grid, Nation, Pos, Terrain, Tile, TileMap, World};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::embark::EmbarkScreen;
use fn_tui::frame::{render_app, render_shell, to_text};
use fn_tui::shell::{Mode, Shell};
use fn_tui::{keys, layout};
use fn_ui::{Clock, Session};

const SIZE: (u16, u16) = (120, 40);
const SMALL: (u16, u16) = (80, 24);
/// Past [`layout::WIDE_TERMINAL`]: the side panel widens to 36 columns (#423).
const WIDE: (u16, u16) = (160, 48);
/// The world's seed. It only feeds the RNG streams a tick would draw from, and the label in the
/// panel's bottom border.
const SEED: u64 = 1;
/// The two settlers' names, in spawn order.
const SETTLERS: [&str; 2] = ["Juni", "Shen"];

/// Every reference screen, by golden name.
const SCREENS: [&str; 11] = [
    "ref_title",
    "ref_embark",
    "ref_game_80x24",
    "ref_game_wide",
    "ref_build_list",
    "ref_research",
    "ref_nation",
    "ref_endeavor",
    "ref_menu",
    "ref_help",
    "ref_log",
];

fn golden(name: &str, actual: &str) {
    // FN_GOLDEN=skip (cargo xtask ci --fast --no-goldens): every golden comparison is a
    // no-op that passes, so agents can run the gate without caring whether goldens are stale.
    if std::env::var("FN_GOLDEN").ok().as_deref() == Some("skip") {
        return;
    }
    let path = format!("{}/tests/golden/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "{name}: the frame changed; rebless with FN_BLESS=1 and review");
}

// ---------------------------------------------------------------------------------------------
// The fixed world
// ---------------------------------------------------------------------------------------------

/// The map's size: wide and tall enough that the widest reference terminal never shows its edge.
const MAP: (i32, i32) = (128, 56);
/// Where the first settler stands, the middle of the map; everything else is placed from here.
const HOME: (i32, i32) = (64, 28);

/// `HOME` shifted by `(dx, dy)`.
fn home(dx: i32, dy: i32) -> Pos {
    Pos::new(HOME.0 + dx, HOME.1 + dy)
}

/// A [`MAP`]-sized stretch of plains: two oak trees, and a three-by-three pond with a fishing
/// spot in its middle.
fn fixed_map(defs: &Defs) -> TileMap {
    let oak = defs.resource_id("oak_tree").expect("oak_tree is bundled");
    let fish = defs.resource_id("fish_spot").expect("fish_spot is bundled");
    let plains = defs.biome_id("plains").expect("plains is bundled");
    Grid::from_fn(MAP.0, MAP.1, |p| {
        let (x, y) = (p.x - HOME.0, p.y - HOME.1);
        let pond = (16..=18).contains(&x) && (-2..=0).contains(&y);
        let tree = (x == 4 && y == -2) || (x == 6 && y == -1);
        Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: if pond { Terrain::Water } else { Terrain::Land },
            biome: plains,
            resource: if tree {
                Some(oak)
            } else if pond && x == 17 && y == -1 {
                Some(fish)
            } else {
                None
            },
            building: None,
        }
    })
}

/// Stamps a finished `id` at `at`, the way a construction site looks once it finishes, without
/// waiting out the ticks (the same approach `frames.rs`'s and `layout_properties.rs`'s own
/// `place_building` take).
fn place_building(world: &mut World, defs: &Defs, id: BuildingId, at: Pos) {
    let def = defs.building(id);
    let (w, h) = def.footprint;
    let foot = Footprint::new(at, w, h);
    world.units.retain(|u| !foot.contains(u.pos));
    world.put_building(foot, id, defs);
}

/// The hand-built Earth Kingdom world of [`fixed_map`], with two settlers named [`SETTLERS`], two
/// finished beds and a paused clock.
fn fixed_world(defs: &Defs, seed: u64) -> Session {
    let bed = defs.building_id("bed").expect("every nation has a bed");
    let spawns = [home(0, 0), home(1, 0)];
    let mut world = World::with_map(seed, fixed_map(defs), Nation::Earth, &spawns, defs);
    for (unit, name) in world.units.iter_mut().zip(SETTLERS) {
        unit.name = name.into();
    }
    place_building(&mut world, defs, bed, home(2, 3));
    place_building(&mut world, defs, bed, home(2, 5));
    let mut clock = Clock::default();
    clock.paused = true;
    Session { world, defs: defs.clone(), clock, bend_choice: None }
}

/// An embark screen over [`fixed_map`] (not `Survey::new`'s mapgen), on a map size of its own.
fn embark_shell(defs: &Defs) -> Shell {
    let size = MapSizeDef {
        id: "fixed".into(),
        name: "Small".into(),
        width: MAP.0,
        height: MAP.1,
        continent_frequency: None,
    };
    let cfg = Survey::config(SEED, &size);
    let survey = Survey::from_map(&cfg, fixed_map(defs), defs);
    let screen = EmbarkScreen::with_survey(
        survey,
        SEED,
        Nation::Earth,
        size,
        SurveyMode::Exact,
        Some("normal".into()),
        SIZE,
        defs,
    );
    let mut shell = Shell::new_title(SIZE, None, SEED);
    shell.mode = Mode::Embark(Box::new(screen));
    shell
}

fn game_text(defs: &Defs, seed: u64, size: (u16, u16), script: &str) -> String {
    let mut app = App::new(fixed_world(defs, seed), size);
    app.play(&keys::parse(script).expect("a valid script"));
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    to_text(&render_app(&mut app, &p, size))
}

/// The message log over a log written by hand (the run's own lines, `push_agent`: nothing a tick
/// computed), over two days and with the cursor on the second line.
fn log_text(defs: &Defs, seed: u64) -> String {
    let mut app = App::new(fixed_world(defs, seed), SIZE);
    let tps = u64::from(fn_sim::world::TICKS_PER_SECOND);
    let day = fn_ui::time::DAY_SECS * tps;
    for (tick, line) in [
        (30 * tps, "Juni scouted the east shore"),
        (95 * tps, "Shen marked the oak grove for chopping"),
        (210 * tps, "Juni and Shen raised a wall by the beds"),
        (day + 40 * tps, "Shen set the research target"),
        (day + 150 * tps, "Juni began a stockpile by the pond"),
    ] {
        app.ui.alerts.push_agent(tick, line.to_owned());
    }
    app.play(&keys::parse("l <down>").expect("a valid script"));
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    to_text(&render_app(&mut app, &p, SIZE))
}

fn shell_text(shell: &mut Shell) -> String {
    let p = Palette::new(shell.defs(), shell.nation(), ColorDepth::TrueColor, false);
    to_text(&render_shell(shell, &p, SIZE))
}

/// The frame of reference screen `name` over `defs`, with the game world's RNG seeded `seed`.
/// The title screen's own shell reads the bundled data itself, so `defs` only reaches the
/// screens that take it.
fn screen_text(name: &str, defs: &Defs, seed: u64) -> String {
    match name {
        "ref_title" => shell_text(&mut Shell::new_title(SIZE, None, SEED)),
        "ref_embark" => shell_text(&mut embark_shell(defs)),
        "ref_game_80x24" => game_text(defs, seed, SMALL, ""),
        "ref_game_wide" => game_text(defs, seed, WIDE, ""),
        "ref_build_list" => game_text(defs, seed, SIZE, "u"),
        "ref_research" => game_text(defs, seed, SIZE, "t"),
        "ref_nation" => game_text(defs, seed, SIZE, "n"),
        "ref_endeavor" => game_text(defs, seed, SIZE, "g"),
        "ref_menu" => game_text(defs, seed, SIZE, "<esc>"),
        "ref_help" => game_text(defs, seed, SIZE, "?"),
        "ref_log" => log_text(defs, seed),
        other => panic!("no reference screen {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// The eleven reference screens
// ---------------------------------------------------------------------------------------------

fn reference(name: &str) {
    golden(name, &screen_text(name, &Defs::bundled(), SEED));
}

#[test]
fn reference_title_screen() {
    reference("ref_title");
}

#[test]
fn reference_embark() {
    reference("ref_embark");
}

#[test]
fn reference_game_at_80x24() {
    reference("ref_game_80x24");
}

/// At [`WIDE`], past [`layout::WIDE_TERMINAL`]: the side panel widens to 36 columns (#423).
#[test]
fn reference_game_at_a_wide_terminal() {
    assert_eq!(layout::panel_width(WIDE.0), 36, "160 columns is past the 140 threshold");
    reference("ref_game_wide");
}

#[test]
fn reference_build_list() {
    reference("ref_build_list");
}

#[test]
fn reference_research() {
    reference("ref_research");
}

#[test]
fn reference_nation() {
    reference("ref_nation");
}

#[test]
fn reference_endeavor() {
    reference("ref_endeavor");
}

#[test]
fn reference_menu() {
    reference("ref_menu");
}

#[test]
fn reference_help() {
    reference("ref_help");
}

#[test]
fn reference_log() {
    reference("ref_log");
}

// ---------------------------------------------------------------------------------------------
// Decoupled from names, rates and seeds
// ---------------------------------------------------------------------------------------------

/// The bundled data with its settler-name pools replaced and the balance numbers a tick would
/// act on scaled or shifted: every need's decay, health's damage, the research trickle, the
/// arrival check, the season length and the job timings.
fn retuned(defs: &Defs) -> Defs {
    let mut d = defs.clone();
    for nation in &mut d.nations {
        nation.names = (0..nation.names.len().max(8)).map(|i| format!("Renamed{i}")).collect();
    }
    d.needs.hunger.decay_per_minute *= 3.0;
    d.needs.thirst.decay_per_minute *= 3.0;
    d.needs.energy.decay_per_minute *= 3.0;
    d.needs.health.depleted_damage_per_minute *= 2.0;
    d.needs.growth.arrival_minutes *= 0.5;
    d.research.passive_knowledge_per_minute *= 5.0;
    d.research.study_knowledge_per_minute *= 5.0;
    d.seasons.calendar.season_minutes *= 2.0;
    d.jobs.eat_seconds *= 4.0;
    d.jobs.drink_seconds *= 4.0;
    d
}

/// The named-pool, rate and season-length retune of [`retuned`] and a wildly different world
/// seed leave every reference frame byte-identical, except for the one label that prints the
/// seed outright (the panel's bottom border, `seed N`). Mapgen is not run at all by any frame
/// here (the maps are hand-built), so a retune of it has nothing to reach.
#[test]
fn retuned_names_rates_and_seeds_change_no_reference_frame() {
    let base = Defs::bundled();
    let other = retuned(&base);
    assert_ne!(base.nations[0].names, other.nations[0].names, "the retune must retune");
    // The panel's bottom border names the world's seed outright: set that one row aside.
    let without_the_seed = |text: &str| -> String {
        text.lines().filter(|l| !l.contains("seed ")).collect::<Vec<_>>().join("\n")
    };
    for name in SCREENS {
        let before = screen_text(name, &base, SEED);
        let after = screen_text(name, &other, 0xDEAD_BEEF);
        assert_eq!(
            without_the_seed(&before),
            without_the_seed(&after),
            "{name} depends on a name, a rate or the world seed"
        );
    }
}

/// The settlers on screen are the ones written in, whatever the nation's pool says.
#[test]
fn the_settlers_on_screen_are_the_fixed_ones() {
    let frame = screen_text("ref_nation", &retuned(&Defs::bundled()), SEED);
    for name in SETTLERS {
        assert!(frame.contains(name), "{name} in\n{frame}");
    }
    assert!(!frame.contains("Renamed"), "{frame}");
}

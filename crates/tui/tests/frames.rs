//! Frames through the public API: golden text frames (rewrite them with `FN_BLESS=1 cargo test -p
//! fn_tui` and review the diff) and the colour of particular cells.
//!
//! Colours are checked cell by cell here and not in colour goldens, which would churn on every
//! palette tweak.

use std::collections::BTreeSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fn_sim::jobs::JobKind;
use fn_sim::world::Command;
use fn_sim::{Nation, Pos};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette, Tint};
use fn_tui::frame::{render, render_app, to_ansi, to_text};
use fn_tui::state::Modal;
use fn_tui::{keys, layout, new_session};
use fn_ui::tools::tiles_between;
use fn_ui::{Session, Tool};
use ratatui::buffer::{Buffer, Cell};
use ratatui::style::{Color, Modifier};

const SIZE: (u16, u16) = (120, 40);

fn palette(session: &Session, depth: ColorDepth, ascii: bool) -> Palette {
    Palette::new(&session.defs, session.world.nation, depth, ascii)
}

fn frame(app: &mut App, depth: ColorDepth, ascii: bool, size: (u16, u16)) -> Buffer {
    let p = palette(&app.session, depth, ascii);
    render_app(app, &p, size)
}

fn golden(name: &str, actual: &str) {
    let path = format!("{}/tests/golden/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "{name}: the frame changed; see the module docs to rebless");
}

// ---------------------------------------------------------------------------------------------
// Golden frames
// ---------------------------------------------------------------------------------------------

/// One seed per nation, each from the first frame of a new game.
#[test]
fn golden_frame_per_nation() {
    for (seed, nation) in
        [(1, Nation::Air), (2, Nation::Water), (7, Nation::Earth), (3, Nation::Fire)]
    {
        let mut app = App::new(new_session(seed, nation), SIZE);
        let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
        golden(&format!("{}_seed{seed}", nation.as_str()), &text);
    }
}

/// A 160x48 terminal, at least [`layout::WIDE_TERMINAL`] columns wide: the panel widens to 36
/// columns (#423, docs/design/side-panel-v2.md "Width") and every row packs to the extra space,
/// rather than padding.
#[test]
fn golden_frame_at_a_wide_terminal() {
    const WIDE: (u16, u16) = (160, 48);
    assert_eq!(layout::panel_width(WIDE.0), 36, "160 columns is past the 140 threshold");
    let mut app = App::new(new_session(7, Nation::Earth), WIDE);
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, WIDE));
    golden("earth_seed7_wide", &text);
}

#[test]
fn golden_ascii_frame() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, true, SIZE));
    assert!(text.is_ascii(), "the ASCII frame must be ASCII:\n{text}");
    golden("earth_seed7_ascii", &text);
}

/// The active tool banner (#307) at the smallest terminal, for Select (none), Harvest and
/// Build: obvious at a glance, and the difference is the signal (no banner in Select).
#[test]
fn golden_frames_of_the_active_tool_banner_for_select_harvest_and_build() {
    const TINY: (u16, u16) = (80, 24);
    let mut select = App::new(new_session(7, Nation::Earth), TINY);
    assert_eq!(select.ui.tool, Tool::Select);
    golden(
        "earth_seed7_banner_select",
        &to_text(&frame(&mut select, ColorDepth::TrueColor, false, TINY)),
    );

    let mut harvest = App::new(new_session(7, Nation::Earth), TINY);
    harvest.play(&keys::parse("h").unwrap());
    golden(
        "earth_seed7_banner_harvest",
        &to_text(&frame(&mut harvest, ColorDepth::TrueColor, false, TINY)),
    );

    let mut build = App::new(new_session(7, Nation::Earth), TINY);
    build.play(&keys::parse("u <enter>").unwrap());
    assert_eq!(build.ui.tool, Tool::Build);
    golden(
        "earth_seed7_banner_build",
        &to_text(&frame(&mut build, ColorDepth::TrueColor, false, TINY)),
    );
}

/// Paused, with a few designations and the cursor moved: the status badge, the Cursor section on
/// a tree, and the same map at another scroll.
#[test]
fn golden_paused_with_designations_and_a_moved_cursor() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    designate_around_the_settlers(&mut app.session);
    app.act(fn_tui::input::Action::TogglePause);
    app.act(fn_tui::input::Action::Move { dx: 1, dy: 1, tiles: 10 });
    golden("earth_seed7_paused", &to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)));
}

#[test]
fn golden_too_small_frame() {
    for size in [(79, 24), (80, 23), (60, 10)] {
        let mut app = App::new(new_session(7, Nation::Earth), size);
        let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, size));
        assert_eq!(text.lines().next(), Some("Terminal too small (80x24 needed)"));
        assert_eq!(text.lines().filter(|l| !l.is_empty()).count(), 1, "only the notice:\n{text}");
    }
    let mut app = App::new(new_session(7, Nation::Earth), (60, 20));
    golden("too_small", &to_text(&frame(&mut app, ColorDepth::TrueColor, false, (60, 20))));
}

#[test]
fn the_smallest_terminal_draws_the_whole_screen() {
    let mut app = App::new(new_session(7, Nation::Earth), (80, 24));
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, (80, 24)));
    assert!(!text.contains("too small"));
    assert!(text.contains("EARTH KINGDOM"), "{text}");
    assert!(text.contains("HERE"), "{text}");
    assert_eq!(text.lines().count(), 24);
}

// ---------------------------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------------------------

fn press(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    app.key(KeyEvent::new(code, modifiers));
}

fn text_after(keys: &[(KeyCode, KeyModifiers)]) -> String {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    for &(code, modifiers) in keys {
        press(&mut app, code, modifiers);
    }
    to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE))
}

#[test]
fn wasd_and_the_arrows_give_the_same_frame() {
    let none = KeyModifiers::NONE;
    for (letter, arrow) in
        [('w', KeyCode::Up), ('a', KeyCode::Left), ('s', KeyCode::Down), ('d', KeyCode::Right)]
    {
        let by_letter = text_after(&[(KeyCode::Char(letter), none); 3]);
        let by_arrow = text_after(&[(arrow, none); 3]);
        assert_eq!(by_letter, by_arrow, "{letter} and {arrow:?}");
        assert_ne!(by_letter, text_after(&[]), "moving changes the Cursor section");
    }
}

#[test]
fn shift_wasd_and_shift_arrows_give_the_same_frame() {
    let shift = KeyModifiers::SHIFT;
    for (letter, arrow) in
        [('W', KeyCode::Up), ('A', KeyCode::Left), ('S', KeyCode::Down), ('D', KeyCode::Right)]
    {
        let by_letter = text_after(&[(KeyCode::Char(letter), shift)]);
        let by_arrow = text_after(&[(arrow, shift)]);
        assert_eq!(by_letter, by_arrow, "{letter} and Shift+{arrow:?}");
        // Ten tiles is not one: it matches ten plain presses.
        let ten_plain = text_after(&[(arrow, KeyModifiers::NONE); 10]);
        assert_eq!(by_arrow, ten_plain, "Shift+{arrow:?} is ten presses");
    }
}

#[test]
fn wasd_move_the_cursor_and_never_scroll_the_view_alone() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let (view, cursor) = (app.ui.view, app.ui.cursor);
    press(&mut app, KeyCode::Char('d'), KeyModifiers::NONE);
    assert_eq!(app.ui.cursor, Pos::new(cursor.x + 1, cursor.y));
    assert_eq!(app.ui.view, view, "the view only follows at the edges");
}

#[test]
fn space_pauses_and_the_dot_steps_one_tick() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let none = KeyModifiers::NONE;
    press(&mut app, KeyCode::Char('.'), none);
    assert_eq!(app.session.world.tick, 0, "stepping is for pausing");
    press(&mut app, KeyCode::Char(' '), none);
    assert!(app.session.clock.paused);
    app.advance(5.0);
    assert_eq!(app.session.world.tick, 0, "paused: no time passes");
    press(&mut app, KeyCode::Char('.'), none);
    press(&mut app, KeyCode::Char('.'), none);
    assert_eq!(app.session.world.tick, 2);
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
    assert!(text.contains("PAUSED") && text.contains("Paused"), "{text}");
    press(&mut app, KeyCode::Char(' '), none);
    app.advance(1.0);
    assert_eq!(app.session.world.tick, 2 + 10);
}

#[test]
fn plus_and_minus_change_the_speed() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let none = KeyModifiers::NONE;
    press(&mut app, KeyCode::Char('+'), none);
    press(&mut app, KeyCode::Char('+'), none);
    assert_eq!(app.session.clock.speed(), 3);
    assert!(to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)).contains("Running x3"));
    press(&mut app, KeyCode::Char('-'), none);
    assert_eq!(app.session.clock.speed(), 2);
}

#[test]
fn q_asks_first_and_only_y_quits() {
    let none = KeyModifiers::NONE;
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    press(&mut app, KeyCode::Char('q'), none);
    assert!(!app.ui.quit, "q only asks");
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
    assert!(text.contains("QUIT?"), "{text}");
    // Any other key withdraws the question, and is not also acted on.
    let cursor = app.ui.cursor;
    press(&mut app, KeyCode::Char('d'), none);
    assert!(!app.ui.quit && app.ui.prompt.is_none());
    assert_eq!(app.ui.cursor, cursor);
    press(&mut app, KeyCode::Char('Q'), none);
    press(&mut app, KeyCode::Esc, none);
    assert!(!app.ui.quit && app.ui.prompt.is_none());
    press(&mut app, KeyCode::Char('q'), none);
    press(&mut app, KeyCode::Char('y'), none);
    assert!(app.ui.quit);
}

#[test]
fn a_terminal_resize_keeps_the_cursor_in_view() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    app.resize(80, 24);
    assert!(app.ui.cursor_cell().is_some());
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, (80, 24)));
    assert!(text.contains("EARTH KINGDOM"));
}

// ---------------------------------------------------------------------------------------------
// Designation tools and key scripts
// ---------------------------------------------------------------------------------------------

/// Plays `script` on a new Earth game on seed 7, the first settler under the cursor.
fn play(script: &str) -> App {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    app.play(&keys::parse(script).expect("a valid script"));
    app
}

/// A rectangle of trees west of the first settler, as a script and as its corners.
const HARVEST_SCRIPT: &str = "h <left*6> <up*3> <enter> <right*5> <down*4> <enter>";

fn harvest_corners(start: Pos) -> (Pos, Pos) {
    (Pos::new(start.x - 6, start.y - 3), Pos::new(start.x - 1, start.y + 1))
}

#[test]
fn a_script_designates_a_harvest_rectangle_and_the_world_agrees() {
    let start = App::new(new_session(7, Nation::Earth), SIZE).ui.cursor;
    let app = play(HARVEST_SCRIPT);
    let (a, b) = harvest_corners(start);

    // The sim decides what a tile accepts: ask it, tile by tile, on a world of its own.
    let mut fresh = new_session(7, Nation::Earth);
    let want: BTreeSet<Pos> = tiles_between(a, b)
        .filter(|&pos| fresh.apply(Command::DesignateHarvest { pos }).is_ok())
        .collect();
    assert!(!want.is_empty(), "the rectangle should hold something to harvest");
    assert!(want.len() < 6 * 5, "and something that isn't, so the notice has skips");
    let got: BTreeSet<Pos> = app.session.world.harvest_designations().collect();
    assert_eq!(got, want);

    // The tool stays active, the corner is spent, and the Tool section reports.
    assert_eq!((app.ui.tool, app.ui.anchor), (Tool::Harvest, None));
    assert_eq!(app.ui.cursor, Pos::new(start.x - 1, start.y + 1));
    let done = want.len();
    let notice = &app.ui.notice;
    assert!(notice.starts_with("designate harvest"), "{notice}");
    assert!(notice.contains(&format!("{done} done, {} skipped (", 30 - done)), "{notice}");
}

/// A hand-built map (docs/design/clearing.md, #411/#426): a 2x2 block of fruit trees at
/// (2,0)-(3,1), a fish spot on the one water tile, and the settler well away from both so the
/// rectangle below covers only the fruit trees.
fn fruit_tree_session() -> Session {
    use fn_sim::defs::BiomeId;
    use fn_sim::{Defs, Grid, Terrain, Tile, World};

    let defs = Defs::bundled();
    let fruit = defs.resource_id("fruit_tree").unwrap();
    let fish = defs.resource_id("fish_spot").unwrap();
    let map = Grid::from_fn(8, 4, |p| {
        let fruit_block = (2..=3).contains(&p.x) && (0..=1).contains(&p.y);
        let water = p.x == 5 && p.y == 1;
        Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: if water { Terrain::Water } else { Terrain::Land },
            biome: BiomeId(0),
            resource: if fruit_block {
                Some(fruit)
            } else if water {
                Some(fish)
            } else {
                None
            },
            building: None,
        }
    });
    let world = World::with_map(1, map, Nation::Earth, &[Pos::new(0, 3)], &defs);
    Session { world, defs, clock: fn_ui::Clock::default(), bend_choice: None }
}

/// #411/#426: `E` marks a rectangle of fruit trees for clearing (the notice counts them, in
/// "marked" not "done"), and `X` over the same rectangle withdraws the marks, as it would any
/// other order.
#[test]
fn a_script_marks_fruit_trees_for_clearing_and_x_withdraws_them() {
    let mut app = App::new(fruit_tree_session(), SIZE);
    assert_eq!(app.ui.cursor, Pos::new(0, 3));

    let fruit_corners = (Pos::new(2, 0), Pos::new(3, 1));
    app.play(&keys::parse("e <up*3> <right*2> <enter> <right> <down> <enter>").unwrap());
    assert_eq!(app.ui.tool, Tool::Clear);
    assert_eq!(app.ui.notice, "clear: 4 marked, 0 skipped");
    let marked: BTreeSet<Pos> = app.session.world.clear_designations().collect();
    let want: BTreeSet<Pos> = tiles_between(fruit_corners.0, fruit_corners.1).collect();
    assert_eq!(marked, want);

    app.play(&keys::parse("x <left> <up> <enter> <right> <down> <enter>").unwrap());
    assert_eq!(app.ui.tool, Tool::CancelHarvest);
    assert!(app.ui.notice.starts_with("cancel harvest"), "{}", app.ui.notice);
    assert!(app.ui.notice.contains("4 done, 0 skipped"), "{}", app.ui.notice);
    assert_eq!(app.session.world.clear_designations().count(), 0);
    // Withdrawing the mark doesn't touch the resource itself: nothing was cleared yet.
    for pos in tiles_between(fruit_corners.0, fruit_corners.1) {
        assert!(app.session.world.map.get(pos).unwrap().resource.is_some(), "{pos:?}");
    }
}

#[test]
fn the_panel_and_the_status_line_show_the_tool_its_hint_and_the_result() {
    let mut app = play("h");
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
    let status = text.lines().last().unwrap();
    assert!(status.starts_with(" HARVEST "), "{status}");
    assert!(status.contains("Enter set corner") && status.contains("designate harvest"));
    // The Tool section's heading is the tool's own name now, not the literal word "TOOL"
    // (docs/design/side-panel.md "Headings"): `├─ HARVEST ────────── [H] ─┤`.
    assert!(text.contains("HARVEST") && text.contains("[H]"), "{text}");
    assert!(text.contains("Enter: set a corner"), "{text}");

    app.play(&keys::parse("<left*2> <enter> <down> <left>").unwrap());
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
    assert!(text.contains("Enter: designate 2x2"), "the Cursor section has the size:\n{text}");
    assert!(text.lines().last().unwrap().contains("Enter apply 2x2"));

    app.play(&keys::parse("<enter>").unwrap());
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
    assert!(text.contains("done"), "the result is in the Tool section:\n{text}");
    assert!(text.contains("Enter: set a corner"));
}

// ---------------------------------------------------------------------------------------------
// Standing harvest orders (#81, #277)
// ---------------------------------------------------------------------------------------------

/// The nearest `n` tiles (to the first settler) the Harvest tool can designate, found on
/// throwaway clones of `s` so `s` itself stays untouched.
fn nearest_harvestable(s: &Session, n: usize) -> Vec<Pos> {
    let from = s.world.units[0].pos;
    let mut tiles: Vec<Pos> =
        s.world.map.iter().filter(|(_, t)| t.resource.is_some()).map(|(p, _)| p).collect();
    tiles.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
    tiles
        .into_iter()
        .filter(|&pos| s.clone().apply(Command::DesignateHarvest { pos }).is_ok())
        .take(n)
        .collect()
}

/// Like `nearest_harvestable`, but only a renewable resource (`regrow.is_some()`), for
/// `a_standing_order_re_designates_the_resource_once_it_regrows`.
fn nearest_renewable_harvestable(s: &Session, n: usize) -> Vec<Pos> {
    let from = s.world.units[0].pos;
    let mut tiles: Vec<Pos> = s
        .world
        .map
        .iter()
        .filter(|(_, t)| {
            t.resource.and_then(|r| s.defs.harvest(r)).is_some_and(|h| h.regrow.is_some())
        })
        .map(|(p, _)| p)
        .collect();
    tiles.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
    tiles
        .into_iter()
        .filter(|&pos| s.clone().apply(Command::DesignateHarvest { pos }).is_ok())
        .take(n)
        .collect()
}

/// `O` has no effect outside the Harvest tool, toggles with it inside it, and resets when the
/// tool is left; the Tool section's heading and the status line both say "Harvest · repeating"
/// while it's on, and the Cursor section says a tile under it has a standing order.
#[test]
fn o_toggles_a_standing_order_in_the_harvest_tool_only() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let target = nearest_harvestable(&app.session, 1)[0];
    app.ui.cursor = target;

    // `O` outside the Harvest tool does nothing.
    app.play(&keys::parse("o").unwrap());
    assert!(!app.ui.repeating);

    app.play(&keys::parse("h").unwrap());
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
    // The heading is upper case (docs/design/side-panel.md "Headings").
    assert!(text.contains("HARVEST ") && text.contains("[H]"), "{text}");
    assert!(!text.to_lowercase().contains("repeating"), "{text}");

    app.play(&keys::parse("o").unwrap());
    assert!(app.ui.repeating);
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
    assert!(text.contains("HARVEST · REPEATING") && text.contains("[H]"), "{text}");
    let status = text.lines().last().unwrap();
    assert!(status.contains("Harvest · repeating"), "{status}");

    // Designating with it on makes a standing order, and the Cursor section says so.
    app.play(&keys::parse("<enter> <enter>").unwrap());
    assert!(app.session.world.is_repeating_harvest(target));
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
    assert!(text.contains("repeating"), "the Cursor section should name it:\n{text}");

    // Leaving the tool drops the toggle; coming back starts fresh.
    app.play(&keys::parse("<esc> h").unwrap());
    assert!(!app.ui.repeating);
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
    assert!(!text.contains("· repeating"), "{text}");
}

/// A tile with a standing order tints apart from a one-off harvest, in every colour depth,
/// including `--color 16`, and `--color never` marks it bold as well as underlined.
#[test]
fn a_standing_order_tints_apart_from_a_one_off_harvest() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let tiles = nearest_harvestable(&app.session, 2);
    let (one_off, standing) = (tiles[0], tiles[1]);

    app.ui.cursor = one_off;
    app.play(&keys::parse("h <enter> <enter>").unwrap());
    app.ui.cursor = standing;
    // The trailing move steps the cursor off `standing`, so its own colour doesn't hide the
    // tint being checked below.
    app.play(&keys::parse("o <enter> <enter> <left>").unwrap());
    assert!(!app.session.world.is_repeating_harvest(one_off));
    assert!(app.session.world.is_repeating_harvest(standing));
    assert_ne!(app.ui.cursor, one_off);
    assert_ne!(app.ui.cursor, standing);

    for depth in [ColorDepth::TrueColor, ColorDepth::Indexed256, ColorDepth::Ansi16] {
        let p = palette(&app.session, depth, false);
        let buf = frame(&mut app, depth, false, SIZE);
        let got_one_off = cell_at(&app, &buf, one_off).bg;
        let got_standing = cell_at(&app, &buf, standing).bg;
        assert_eq!(got_one_off, p.tinted(ground_bg(&p, &app.session, one_off), Tint::Harvest));
        assert_eq!(
            got_standing,
            p.tinted(ground_bg(&p, &app.session, standing), Tint::RepeatingHarvest)
        );
        assert_ne!(got_one_off, got_standing, "{depth:?}: the two should look different");
    }

    // Without colour, a standing order is bold as well as underlined.
    let buf = frame(&mut app, ColorDepth::Never, false, SIZE);
    let one_off_cell = cell_at(&app, &buf, one_off);
    let standing_cell = cell_at(&app, &buf, standing);
    assert!(one_off_cell.modifier.contains(Modifier::UNDERLINED));
    assert!(!one_off_cell.modifier.contains(Modifier::BOLD), "{one_off_cell:?}");
    assert!(standing_cell.modifier.contains(Modifier::UNDERLINED | Modifier::BOLD));
}

/// `h o <enter> <enter> <tick N>`: once the standing order's resource is worked and ripens
/// again, a fresh job appears without the player doing anything (#81's sim rule, reached through
/// the TUI's own key, #277).
#[test]
fn a_standing_order_re_designates_the_resource_once_it_regrows() {
    let s = new_session(7, Nation::Earth);
    let target = nearest_renewable_harvestable(&s, 1)[0];

    let mut app = App::new(s, SIZE);
    app.ui.cursor = target;
    app.play(&keys::parse("h o <enter> <enter>").unwrap());
    assert!(app.session.world.is_repeating_harvest(target));

    let has_job = |app: &App| {
        app.session.world.jobs().iter().any(|j| j.kind == JobKind::Harvest { pos: target })
    };

    // Worked once, same as a one-shot harvest: the job is gone and the resource rests.
    for _ in 0..200 {
        app.play(&keys::parse("<tick 100>").unwrap());
        if app.session.world.is_resting(target) {
            break;
        }
    }
    assert!(app.session.world.is_resting(target), "the resource should have been worked once");
    assert!(!has_job(&app), "no job sits around while it rests");
    assert!(app.session.world.is_repeating_harvest(target), "the standing order still shows");

    // Once it ripens, a fresh job appears on its own.
    for _ in 0..200 {
        app.play(&keys::parse("<tick 500>").unwrap());
        if !app.session.world.is_resting(target) {
            break;
        }
    }
    assert!(!app.session.world.is_resting(target), "the resource should have ripened again");
    assert!(has_job(&app), "the standing order should have re-designated it");
}

#[test]
fn the_result_notice_fits_on_the_smallest_terminal() {
    let mut app = App::new(new_session(7, Nation::Earth), (80, 24));
    // Harvesting where there is nothing to harvest is refused with a reason.
    let sea = (0..app.session.world.map.width())
        .map(|x| Pos::new(x, 0))
        .find(|&p| app.session.apply(Command::DesignateHarvest { pos: p }).is_err())
        .unwrap();
    app.ui.cursor = sea;
    app.play(&keys::parse("h <enter> <enter>").unwrap());
    // No stale "hold Shift to repeat" (#305: the client that read it is gone, #281); checked on
    // the unwrapped notice since the rendered frame wraps it over a couple of lines.
    assert_eq!(
        app.ui.notice,
        "designate harvest (chop, mine, gather): 0 done, 1 skipped (nothing to harvest)"
    );
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, (80, 24)));
    // What Enter does next is the banner's own line now (#307, trimmed to fit the 48-column
    // map at this size), not the Cursor section's, which still has room for its own rows.
    assert!(text.contains("HARVEST"), "the banner survives:\n{text}");
    assert!(text.contains("Elevation 0"), "the Cursor section survives:\n{text}");
}

#[test]
fn bend_names_the_nations_ability_in_the_status_line_and_the_panel() {
    for nation in Nation::ALL {
        let mut app = App::new(new_session(3, nation), SIZE);
        app.play(&keys::parse("B").unwrap());
        assert_eq!(app.ui.tool, Tool::Bend, "{nation:?}");
        let id = fn_ui::tools::ability_for_nation(&app.session.defs, nation).unwrap();
        let verb = app.session.defs.ability(id).verb.clone();
        let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
        let status = text.lines().last().unwrap();
        assert!(status.starts_with(" BEND "), "{status}");
        assert!(status.contains(&format!("{verb}:")), "{nation:?} status: {status}");
        assert!(text.contains(&verb), "{nation:?} panel:\n{text}");
    }
}

#[test]
fn escape_drops_the_corner_then_leaves_the_tool() {
    let mut app = play("p <enter> <right*2>");
    assert!(app.ui.anchor.is_some());
    app.play(&keys::parse("<esc>").unwrap());
    assert_eq!((app.ui.tool, app.ui.anchor), (Tool::Stockpile, None));
    app.play(&keys::parse("<enter> <esc> <esc>").unwrap());
    assert_eq!((app.ui.tool, app.ui.anchor), (Tool::Select, None));
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
    assert!(text.lines().last().unwrap().starts_with(" SELECT "));
    // The Tool section's heading names the active tool (docs/design/side-panel.md "Headings");
    // Select mode has none, so none of a tool's own name shows.
    assert!(
        !["STOCKPILE", "HARVEST", "BUILD", "BEND"].iter().any(|t| text.contains(t)),
        "Select mode hides the section:\n{text}"
    );
}

#[test]
fn a_tool_key_switches_tools_and_drops_the_corner_and_the_result() {
    let mut app = play("h <enter> <right> <enter> <enter>");
    assert!(!app.ui.notice.is_empty() && app.ui.anchor.is_some());
    app.play(&keys::parse("x").unwrap());
    assert_eq!((app.ui.tool, app.ui.anchor), (Tool::CancelHarvest, None));
    assert!(app.ui.notice.is_empty());
    app.play(&keys::parse("<enter> X").unwrap());
    assert_eq!((app.ui.tool, app.ui.anchor), (Tool::CancelHarvest, None), "the same key again");
}

/// `U` opens the Build modal instead of switching tools (#285); the tool and the choice stay as
/// they were until `Enter` there picks the highlighted building, which closes the modal and
/// leaves the Build tool active with it. `U` again reopens the modal on the current choice
/// (no more cycling), and switching to another tool and back keeps the choice.
#[test]
fn u_opens_the_build_modal_and_enter_picks_the_highlighted_building() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let rows = fn_ui::build::picker_rows(&app.session);
    let open: Vec<_> = rows.iter().filter(|r| !r.locked()).map(|r| r.id).collect();
    assert!(open.len() >= 2, "the scenario needs two placeable buildings");
    let idx = |id| rows.iter().position(|r| r.id == id).unwrap();
    let (i0, i1) = (idx(open[0]), idx(open[1]));

    app.play(&keys::parse("u").unwrap());
    assert_eq!(app.ui.modal, Some(Modal::Build));
    assert_eq!(app.ui.build_cursor, i0, "starts on the first placeable building");
    assert_eq!((app.ui.tool, app.ui.choice), (Tool::Select, None), "nothing changes until Enter");

    app.play(&keys::parse("<enter>").unwrap());
    assert_eq!(app.ui.modal, None);
    assert_eq!((app.ui.tool, app.ui.choice), (Tool::Build, Some(open[0])));

    // U reopens the modal on the choice, rather than cycling to the next building.
    app.play(&keys::parse("u").unwrap());
    assert_eq!(app.ui.modal, Some(Modal::Build));
    assert_eq!(app.ui.build_cursor, i0);
    app.play(&keys::parse(&format!("<down*{}> <enter>", i1 - i0)).unwrap());
    assert_eq!((app.ui.tool, app.ui.choice), (Tool::Build, Some(open[1])));

    // Another tool and back keeps the choice, and reopening the modal starts on it.
    app.play(&keys::parse("h u").unwrap());
    assert_eq!(app.ui.build_cursor, i1, "reopening starts on the current choice");
    app.play(&keys::parse("<esc>").unwrap());
    assert_eq!((app.ui.tool, app.ui.choice), (Tool::Harvest, Some(open[1])), "Esc leaves both be");
}

/// While the modal is open it captures every key: no tool key, movement or clock action reaches
/// the map until `Esc` closes it.
#[test]
fn the_build_modal_captures_input_until_it_closes() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let cursor = app.ui.cursor;
    app.play(&keys::parse("u h <right> <space>").unwrap());
    assert_eq!(app.ui.modal, Some(Modal::Build));
    assert_eq!(app.ui.cursor, cursor, "cursor keys don't reach the map while it's open");
    assert_eq!(app.ui.tool, Tool::Select, "h doesn't switch tools either");
    assert!(!app.session.clock.paused, "space doesn't pause");
    app.play(&keys::parse("<esc>").unwrap());
    assert_eq!(app.ui.modal, None);
    assert_eq!((app.ui.tool, app.ui.choice), (Tool::Select, None));
}

/// A placement the sim refuses (a resource in the way) puts `fn_ui::build::describe_error`'s
/// reason in the Tool notice, as any other tool's refusal does.
#[test]
fn a_refused_build_names_its_blocker_in_the_notice() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let tree = app
        .session
        .world
        .map
        .iter()
        .find(|(_, t)| t.terrain == fn_sim::Terrain::Land && t.resource.is_some())
        .map(|(p, _)| p)
        .expect("a resource on land");
    app.ui.cursor = tree;
    // u, Enter picks the highlighted building, then Enter sets the corner and Enter applies it.
    app.play(&keys::parse("u <enter> <enter> <enter>").unwrap());
    assert!(app.ui.notice.starts_with("build "), "{}", app.ui.notice);
    assert!(app.ui.notice.contains("is in the way"), "{}", app.ui.notice);
    assert!(app.session.world.build_site(tree).is_none(), "the refused build left no site");
}

#[test]
fn designating_changes_nothing_in_select_mode() {
    let app = play("<enter> <right> <enter> <esc>");
    assert_eq!(app.session.world.command_log().len(), 0);
    assert_eq!((app.ui.tool, app.ui.anchor), (Tool::Select, None));
}

#[test]
fn cancel_withdraws_what_the_rectangle_covers() {
    let app =
        play(&format!("{HARVEST_SCRIPT} x <up*4> <left*5> <enter> <down*4> <right*5> <enter>"));
    assert!(app.ui.notice.starts_with("cancel harvest"), "{}", app.ui.notice);
    assert_eq!(app.session.world.harvest_designations().count(), 0, "{}", app.ui.notice);
}

#[test]
fn stockpile_and_bend_designations_show_as_tints() {
    let start = App::new(new_session(7, Nation::Earth), SIZE).ui.cursor;
    // A stockpile 3 wide west of the settler, on tiles that take one.
    let mut app = play("p <left*5> <enter> <left*2> <enter> <up*3>");
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    let zones: Vec<Pos> = app.session.world.stockpile_zones().collect();
    assert!(!zones.is_empty());
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    for pos in zones {
        assert!(pos.y == start.y && (start.x - 7..=start.x - 5).contains(&pos.x), "{pos:?}");
        let want = p.tinted(ground_bg(&p, &app.session, pos), Tint::Stockpile);
        assert_eq!(cell_at(&app, &buf, pos).bg, want, "{pos:?}");
    }
}

#[test]
fn wasd_scripts_move_the_cursor_like_the_arrows() {
    let by_letter = play("d d s S");
    let by_arrow = play("<right*2> <down> <s-down>");
    assert_eq!(by_letter.ui.cursor, by_arrow.ui.cursor);
    let start = App::new(new_session(7, Nation::Earth), SIZE).ui.cursor;
    assert_eq!(by_letter.ui.cursor, Pos::new(start.x + 2, start.y + 11));
    // a, w and W go the other way.
    let back = play("a W w");
    assert_eq!(back.ui.cursor, Pos::new(start.x - 1, start.y - 11));
}

#[test]
fn a_script_never_reads_time_so_the_same_script_is_the_same_game() {
    let script = format!("{HARVEST_SCRIPT} <tick 600>");
    let (mut a, mut b) = (play(&script), play(&script));
    assert_eq!(a.session.world.tick, 600);
    let p = palette(&a.session, ColorDepth::TrueColor, false);
    let (fa, fb) = (render_app(&mut a, &p, SIZE), render_app(&mut b, &p, SIZE));
    assert_eq!(to_ansi(&fa), to_ansi(&fb));
    // The clock never ran: only <tick> moved the world.
    assert!(!a.session.clock.paused);
}

/// The script's command log, replayed into a fresh world, draws the same frame.
#[test]
fn the_command_log_of_a_script_replays_into_the_same_frame() {
    let script = format!(
        "{HARVEST_SCRIPT} <tick 300> p <left*8> <enter> <left*2> <enter> <tick 200> \
         x <left> <enter> <enter> <tick 100> b <enter> <enter> <tick 50> h <up*6>"
    );
    let mut app = play(&script);
    let log = app.session.world.command_log().to_vec();
    assert!(log.len() >= 3, "the script should have issued commands: {log:?}");
    assert!(log.iter().any(|r| r.tick > 0), "and some of them after time passed");

    let mut fresh = new_session(7, Nation::Earth);
    let until = app.session.world.tick;
    let defs = fresh.defs.clone();
    fresh.world.replay(&log, until, &defs);
    assert_eq!(fresh.world.tick, until);
    assert_eq!(fresh.world.command_log(), app.session.world.command_log());

    let p = palette(&app.session, ColorDepth::TrueColor, false);
    let played = render_app(&mut app, &p, SIZE);
    let replayed = render(&fresh, &app.ui, &p, SIZE);
    assert_eq!(to_text(&played), to_text(&replayed));
    assert_eq!(to_ansi(&played), to_ansi(&replayed));
}

/// Mid-drag: the corner is set, the cursor has moved, and the Cursor section shows the size.
#[test]
fn golden_frame_of_a_rectangle_being_dragged() {
    let mut app = play("h <left*6> <up*3> <enter> <right*4> <down*2>");
    golden("earth_seed7_dragging", &to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)));
}

/// After the second `Enter`: the designations stay and the Tool section reports.
#[test]
fn golden_frame_after_the_rectangle_is_applied() {
    let mut app = play(HARVEST_SCRIPT);
    golden(
        "earth_seed7_designated",
        &to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)),
    );
}

/// Harvests up to `n` of the workable resources nearest the first settler, so there is wood
/// coming in for a build to spend.
fn harvest_resources_near_the_settlers(s: &mut Session, n: usize) -> Vec<Pos> {
    let from = s.world.units[0].pos;
    let near = |p: &Pos| (p.x - from.x).pow(2) + (p.y - from.y).pow(2);
    let mut tiles: Vec<Pos> =
        s.world.map.iter().filter(|(_, t)| t.resource.is_some()).map(|(p, _)| p).collect();
    tiles.sort_by_key(near);
    let mut done = Vec::new();
    for pos in tiles {
        if s.apply(Command::DesignateHarvest { pos }).is_ok() {
            done.push(pos);
            if done.len() >= n {
                break;
            }
        }
    }
    done
}

/// A walkable, resource-free tile near `near` that isn't in `avoid` and has no unit on it, for a
/// build designation.
fn free_tile_near(s: &Session, near: Pos, avoid: &[Pos]) -> Pos {
    s.world
        .map
        .iter()
        .filter(|(p, t)| {
            t.terrain.is_walkable()
                && t.resource.is_none()
                && (p.x - near.x).abs().max((p.y - near.y).abs()) <= 10
                && !s.world.units.iter().any(|u| u.pos == *p)
        })
        .map(|(p, _)| p)
        .find(|p| !avoid.contains(p))
        .expect("a free tile near home")
}

/// `U` opens the Build modal; `Enter` there picks the first building the nation can place (Bed,
/// for every nation; `fn_ui::build`'s `valid_choice` with no choice yet). Two more `Enter`s
/// designate it under the cursor. With wood coming in from a harvest designated the ordinary
/// way, ticking finishes it.
#[test]
fn golden_frame_of_a_build_site_then_the_finished_building() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let trees = harvest_resources_near_the_settlers(&mut app.session, 3);
    let bed = app.session.defs.building_id("bed").unwrap();
    let home = app.session.world.units[0].pos;
    let site = free_tile_near(&app.session, home, &trees);
    app.ui.cursor = site;

    app.play(&keys::parse("u <enter> <enter> <enter>").unwrap());
    assert_eq!((app.ui.tool, app.ui.choice), (Tool::Build, Some(bed)));
    assert!(app.session.world.build_site(site).is_some(), "a site should be designated");
    golden(
        "earth_seed7_build_site",
        &to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)),
    );

    app.play(&keys::parse("<tick 9000>").unwrap());
    assert!(app.session.world.building_at(site).is_some(), "the bed should finish building");
    golden("earth_seed7_built", &to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)));
}

/// Golden frames of the Build modal itself (#285): a placeable building highlighted, then a
/// locked one, so the reviewer can see both the list and the detail pane (the tech it still
/// needs) at a glance.
#[test]
fn golden_frame_of_the_build_modal() {
    let mut app = play("u");
    golden(
        "earth_seed7_build_modal",
        &to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)),
    );

    let rows = fn_ui::build::picker_rows(&app.session);
    let locked = rows.iter().position(|r| r.locked()).expect("Earth has a locked building");
    app.play(&keys::parse(&format!("<down*{locked}>")).unwrap());
    golden(
        "earth_seed7_build_modal_locked",
        &to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)),
    );
}

/// A Fire picker row whose stone cost names brick as a stand-in (issue #371): "N Stone (or
/// brick)". Only Fire can ever raise a brick kiln, so this is Fire's own golden, not Earth's.
#[test]
fn golden_frame_of_a_fire_picker_row_with_a_stone_stand_in() {
    let mut app = App::new(new_session(7, Nation::Fire), SIZE);
    app.play(&keys::parse("u").unwrap());
    let rows = fn_ui::build::picker_rows(&app.session);
    let workshop = rows.iter().position(|r| r.name == "Workshop").expect("Fire has a workshop");
    app.play(&keys::parse(&format!("<down*{workshop}>")).unwrap());
    assert!(
        rows[workshop].cost.contains("(or brick)"),
        "the workshop's stone cost should name brick: {}",
        rows[workshop].cost
    );
    golden(
        "fire_seed7_build_modal_stand_in",
        &to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)),
    );
}

// ---------------------------------------------------------------------------------------------
// Power (#428, #429): readouts in the Build modal and the Here section
// ---------------------------------------------------------------------------------------------

/// Stamps a finished `id` with its anchor at `at`, baking every tile's own `part` and `size`
/// (`fn_sim::building::Building`), the way a construction site would once it finishes: for a
/// test that needs a building already standing, not under construction.
fn place_building(s: &mut Session, id: fn_sim::building::BuildingId, at: Pos) {
    use fn_sim::building::Building;
    use fn_sim::geom::Footprint;
    let def = s.defs.building(id);
    let (w, h) = def.footprint;
    let passable = def.passable;
    let foot = Footprint::new(at, w, h);
    s.world.units.retain(|u| !foot.contains(u.pos));
    for p in foot.tiles() {
        let part = [(p.x - at.x) as u8, (p.y - at.y) as u8];
        let b = Building { part, size: [w, h], ..Building::new(id, passable, false) };
        s.world.map.get_mut(p).unwrap().building = Some(b);
    }
}

/// No bundled building is a power source yet (#255, #204, #200), so every test here mutates
/// Bed's def into one, the same approach `fn_sim::world::power::tests`'s `test_source` takes for
/// the sim and `fn_ui::build`'s own detail test takes for the UI: Bed needs no tech and no
/// nation (`BuildingDef::unlocked_by` is `None`), so the Build modal's picker never substitutes
/// another choice. Water's `power_generation` is 1.0 (data/nations.toml), so the reach shown is
/// always the bare radius.
fn session_with_a_test_source(
    seed: u64,
    nation: Nation,
    radius: u32,
    boost: f32,
) -> (Session, fn_sim::building::BuildingId) {
    let mut s = Session::new(seed, nation, 48, 32);
    let source = s.defs.building_id("bed").unwrap();
    s.defs.buildings[source.0 as usize].power = Some(fn_sim::building::PowerDef {
        radius,
        boost,
        boost_millis: (boost * 1000.0).round() as u32,
    });
    (s, source)
}

/// The Build modal's detail (#429, docs/design/late-tree.md "UI"): a source names its reach,
/// already scaled for the nation's current `power_generation`, and what being powered buys
/// (`fn_ui::build`'s own test covers a building that otherwise says "Nothing beyond shelter"
/// losing that line); a consumer (the real Workshop, `uses_power = true` since #428) just says
/// it uses power.
#[test]
fn the_build_modal_names_a_sources_reach_and_boost_and_a_consumers_uses_power() {
    let (s, source) = session_with_a_test_source(7, Nation::Water, 3, 1.5);
    let mut app = App::new(s, SIZE);
    let rows = fn_ui::build::picker_rows(&app.session);
    app.play(&keys::parse("u").unwrap());
    app.ui.build_cursor = rows.iter().position(|r| r.id == source).unwrap();
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, true, SIZE));
    assert!(text.contains("Powers buildings within 3 tiles"), "{text}");
    assert!(text.contains("Powered work: 1.5x faster"), "{text}");

    let workshop = app.session.defs.building_id("workshop").unwrap();
    app.ui.build_cursor = rows.iter().position(|r| r.id == workshop).unwrap();
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, true, SIZE));
    assert!(text.contains("Uses power"), "{text}");
}

/// The Here section (#429): a consumer names the source that actually powers it and the boost,
/// or "Unpowered" out of reach; a source names its reach and how many buildings it's powering
/// right now.
#[test]
fn the_here_section_names_a_powered_and_an_unpowered_workshop_and_the_source_itself() {
    let (mut s, source) = session_with_a_test_source(7, Nation::Water, 3, 1.5);
    let workshop = s.defs.building_id("workshop").unwrap();
    let source_at = Pos::new(10, 10);
    let powered_at = Pos::new(13, 10); // 3 tiles from the source's own tile: just in reach
    let unpowered_at = Pos::new(25, 10); // far outside it
    place_building(&mut s, source, source_at);
    s.world.adopt_power_sources(&s.defs);
    place_building(&mut s, workshop, powered_at);
    place_building(&mut s, workshop, unpowered_at);

    let mut app = App::new(s, SIZE);
    app.ui.cursor = powered_at;
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, true, SIZE));
    assert!(text.contains("Powered: Bed, 1.5x"), "{text}");

    app.ui.cursor = unpowered_at;
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, true, SIZE));
    assert!(text.contains("Unpowered"), "{text}");

    app.ui.cursor = source_at;
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, true, SIZE));
    assert!(text.contains("Reach: 3 tiles"), "{text}");
    assert!(text.contains("Powering: 1 building"), "{text}");
}

/// The rectangle's tiles take the tool's hue (stronger than a designation's), the cursor keeps
/// its own colour, and tiles outside the rectangle are untouched.
#[test]
fn the_dragged_rectangle_is_tinted_between_the_cursor_and_the_designations() {
    for (keys, tint) in
        [("h", Tint::Harvest), ("x", Tint::Harvest), ("p", Tint::Stockpile), ("b", Tint::Bend)]
    {
        for depth in [ColorDepth::TrueColor, ColorDepth::Indexed256, ColorDepth::Ansi16] {
            let mut app = play(&format!("{keys} <left*4> <up*2> <enter> <right*2> <down>"));
            let p = palette(&app.session, depth, false);
            let buf = frame(&mut app, depth, false, SIZE);
            let (a, b) = app.ui.rect().unwrap();
            let unit_free = |pos: &Pos| !app.session.world.units.iter().any(|u| u.pos == *pos);
            let mut checked = 0;
            for pos in tiles_between(a, b).filter(unit_free) {
                let cell = cell_at(&app, &buf, pos);
                if pos == app.ui.cursor {
                    assert_eq!(cell.bg, p.cursor_bg(), "{keys} {depth:?}: the cursor wins");
                } else {
                    let want = p.dragged(ground_bg(&p, &app.session, pos), tint);
                    assert_eq!(cell.bg, want, "{keys} {depth:?} {pos:?}");
                    assert_ne!(cell.bg, p.color(ground_bg(&p, &app.session, pos)));
                    assert_ne!(cell.bg, p.cursor_bg());
                    checked += 1;
                }
            }
            assert!(checked >= 5, "{keys} {depth:?}: {checked}");
            // One tile outside is the plain ground.
            let outside = Pos::new(a.x.min(b.x) - 1, a.y.min(b.y));
            let cell = cell_at(&app, &buf, outside);
            assert_eq!(cell.bg, p.color(ground_bg(&p, &app.session, outside)));
        }
    }
}

#[test]
fn without_colour_the_dragged_rectangle_is_reversed_and_dim() {
    let mut app = play("h <left*4> <enter> <right*2>");
    let buf = frame(&mut app, ColorDepth::Never, false, SIZE);
    let (a, b) = app.ui.rect().unwrap();
    let (cursor, other) = (cell_at(&app, &buf, b), cell_at(&app, &buf, a));
    assert!(other.modifier.contains(Modifier::REVERSED | Modifier::DIM), "{other:?}");
    assert!(cursor.modifier.contains(Modifier::REVERSED));
    assert!(!cursor.modifier.contains(Modifier::DIM), "{cursor:?}");
    let outside = Pos::new(a.x - 1, a.y);
    assert!(!cell_at(&app, &buf, outside).modifier.contains(Modifier::REVERSED));
}

// ---------------------------------------------------------------------------------------------
// Colours of particular cells
// ---------------------------------------------------------------------------------------------

/// The cell that shows tile `pos` in the frame `buf` drawn for `app`.
fn cell_at<'b>(app: &App, buf: &'b Buffer, pos: Pos) -> &'b Cell {
    let map = layout::regions(buf.area).unwrap().map;
    let (x, y) = (pos.x - app.ui.view.x, pos.y - app.ui.view.y);
    assert!(
        x >= 0 && y >= 0 && x < i32::from(map.width) && y < i32::from(map.height),
        "{pos:?} not in view"
    );
    &buf[(map.x + x as u16, map.y + y as u16)]
}

/// Designates a harvest on the nearest tree, a stockpile beside the first settler, and a build
/// site, so the map has each kind of tint.
fn designate_around_the_settlers(s: &mut Session) -> (Pos, Pos, Pos) {
    let from = s.world.units[0].pos;
    let near = |p: &Pos| (p.x - from.x).pow(2) + (p.y - from.y).pow(2);
    let mut tiles: Vec<_> = s.world.map.iter().map(|(p, t)| (p, *t)).collect();
    tiles.sort_by_key(|(p, _)| near(p));
    let occupied = |s: &Session, p: Pos| s.world.units.iter().any(|u| u.pos == p);

    // The nearest resource the nation can work already (some need research first).
    let candidates: Vec<Pos> = tiles
        .iter()
        .filter(|(p, t)| t.resource.is_some() && !occupied(s, *p))
        .map(|(p, _)| *p)
        .collect();
    let tree = candidates
        .into_iter()
        .find(|&pos| s.apply(Command::DesignateHarvest { pos }).is_ok())
        .expect("a harvestable resource");

    let free = |s: &Session, p: &Pos, t: &fn_sim::Tile| {
        t.terrain == fn_sim::Terrain::Land && t.resource.is_none() && !occupied(s, *p) && *p != tree
    };
    let land: Vec<Pos> =
        tiles.iter().filter(|(p, t)| free(s, p, t)).map(|(p, _)| *p).take(2).collect();
    let zone = land[0];
    s.apply(Command::DesignateStockpile { pos: zone }).unwrap();
    let site = land[1];
    let bed = s.defs.building_id("bed").unwrap();
    s.apply(Command::DesignateBuild { pos: site, building: bed }).unwrap();
    (tree, zone, site)
}

fn ground_bg(p: &Palette, session: &Session, pos: Pos) -> [u8; 3] {
    let tile = session.world.map.get(pos).unwrap();
    let look = match tile.terrain {
        fn_sim::Terrain::Land => &p.glyphs.biome[&session.defs.biome(tile.biome).id],
        other => p.glyphs.terrain(other).unwrap(),
    };
    look.bg.unwrap()
}

#[test]
fn designations_tint_the_background_and_the_cursor_wins() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let (tree, zone, site) = designate_around_the_settlers(&mut app.session);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    // Put the cursor on the tree so the view covers all three, then off it.
    app.ui.cursor = site;
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    for (pos, tint) in [(tree, Tint::Harvest), (zone, Tint::Stockpile), (site, Tint::Build)] {
        if pos == site {
            continue; // under the cursor, below
        }
        let want = p.tinted(ground_bg(&p, &app.session, pos), tint);
        assert_eq!(cell_at(&app, &buf, pos).bg, want, "{pos:?} should be {tint:?}");
    }
    // The cursor beats the build tint: black on the cursor colour.
    let cursor = cell_at(&app, &buf, site);
    assert_eq!(cursor.bg, p.cursor_bg());
    assert_eq!(cursor.fg, Color::Rgb(0, 0, 0));
    // The site draws as its building, dimmed.
    assert_eq!(cursor.symbol(), "Θ");
}

#[test]
fn a_unit_draws_in_its_nation_colour_in_bold() {
    for nation in Nation::ALL {
        let mut app = App::new(new_session(7, nation), SIZE);
        let p = palette(&app.session, ColorDepth::TrueColor, false);
        // Look at a unit that isn't under the cursor.
        let unit = app.session.world.units[1].pos;
        let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
        let cell = cell_at(&app, &buf, unit);
        assert_eq!(cell.symbol(), "☺", "{nation:?}");
        assert_eq!(cell.fg, p.unit(), "{nation:?}");
        assert!(cell.modifier.contains(Modifier::BOLD));
        let [r, g, b] = p.unit_rgb();
        assert_eq!(cell.fg, Color::Rgb(r, g, b));
    }
}

#[test]
fn ascii_units_are_ats() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let unit = app.session.world.units[1].pos;
    let buf = frame(&mut app, ColorDepth::TrueColor, true, SIZE);
    assert_eq!(cell_at(&app, &buf, unit).symbol(), "@");
}

#[test]
fn the_cursor_cell_is_black_on_yellow_and_the_terminal_cursor_sits_there() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let cell = cell_at(&app, &buf, app.ui.cursor);
    assert_eq!((cell.fg, cell.bg), (Color::Rgb(0, 0, 0), Color::Rgb(250, 220, 60)));
    assert_eq!(cell.symbol(), "☺", "the cursor keeps the glyph it covers");
    let at = fn_tui::draw::terminal_cursor(&app.ui, buf.area).unwrap();
    assert_eq!(&buf[(at.x, at.y)], cell);
    assert_eq!(p.cursor_bg(), cell.bg);
}

#[test]
fn without_colour_the_cursor_is_reversed_and_designations_are_underlined() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let (tree, _, _) = designate_around_the_settlers(&mut app.session);
    let buf = frame(&mut app, ColorDepth::Never, false, SIZE);
    let cursor = cell_at(&app, &buf, app.ui.cursor);
    assert!(cursor.modifier.contains(Modifier::REVERSED));
    let cell = cell_at(&app, &buf, tree);
    assert!(cell.modifier.contains(Modifier::UNDERLINED), "{cell:?}");
    for cell in buf.content() {
        assert!(matches!(cell.fg, Color::Reset) && matches!(cell.bg, Color::Reset), "{cell:?}");
    }
}

#[test]
fn quantised_frames_only_use_the_depth_they_were_asked_for() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let buf = frame(&mut app, ColorDepth::Indexed256, false, SIZE);
    for c in buf.content() {
        assert!(!matches!(c.fg, Color::Rgb(..)) && !matches!(c.bg, Color::Rgb(..)), "{c:?}");
    }
    let buf = frame(&mut app, ColorDepth::Ansi16, false, SIZE);
    for c in buf.content() {
        assert!(
            !matches!(c.fg, Color::Rgb(..) | Color::Indexed(_))
                && !matches!(c.bg, Color::Rgb(..) | Color::Indexed(_)),
            "{c:?}"
        );
    }
    assert!(to_ansi(&buf).contains("\x1b["));
}

#[test]
fn items_on_the_ground_and_in_the_stockpile_show_as_item_glyphs() {
    let mut s = new_session(7, Nation::Earth);
    // Harvest every tree and rock near the settlers, and give them a stockpile to fill.
    let from = s.world.units[0].pos;
    let near: Vec<Pos> = s
        .world
        .map
        .iter()
        .filter(|(p, t)| {
            (p.x - from.x).abs() < 12
                && (p.y - from.y).abs() < 8
                && t.resource.is_some_and(|r| s.defs.harvest(r).is_some())
        })
        .map(|(p, _)| p)
        .collect();
    for p in near {
        let _ = s.apply(Command::DesignateHarvest { pos: p });
    }
    let zone = Pos::new(from.x, from.y + 2);
    for dx in 0..3 {
        let _ = s.apply(Command::DesignateStockpile { pos: Pos::new(zone.x + dx, zone.y) });
    }
    let mut seen = (false, false);
    for _ in 0..40 {
        s.tick_n(100);
        seen.0 |= s.world.ground_items().next().is_some();
        seen.1 |= s.world.stored_at(zone).is_some();
        if seen == (true, true) {
            break;
        }
    }
    assert!(seen.0, "something should have been dropped on the ground");

    let mut app = App::new(s, SIZE);
    app.ui.cursor = from;
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let units: Vec<Pos> = app.session.world.units.iter().map(|u| u.pos).collect();
    let mut checked = 0;
    for (pos, stack) in app.session.world.ground_items() {
        let (x, y) = (pos.x - app.ui.view.x, pos.y - app.ui.view.y);
        if units.contains(&pos) || pos == app.ui.cursor || x < 0 || y < 0 || x >= 88 || y >= 39 {
            continue;
        }
        let item = &app.session.defs.item(stack.item).id;
        let want = p.glyphs.item[item];
        let cell = cell_at(&app, &buf, pos);
        assert_eq!(cell.symbol(), want.glyph.to_string(), "{pos:?} holds {item}");
        assert_eq!(cell.fg, Color::Rgb(want.fg[0], want.fg[1], want.fg[2]));
        checked += 1;
    }
    assert!(checked > 0, "the scenario should put an item on view");
}

#[test]
fn a_lit_flame_draws_over_the_resource_and_warms_its_reach_under_the_cursor() {
    let mut s = Session::new(1, Nation::Fire, 48, 32);
    let pos = s.world.units[0].pos;
    let ability = fn_ui::tools::ability_for_nation(&s.defs, Nation::Fire).unwrap();
    s.apply(Command::Bend { ability, pos }).unwrap();
    for _ in 0..20_000 {
        s.tick();
        if s.world.is_lit(pos) && !s.world.units.iter().any(|u| u.pos == pos) {
            break;
        }
    }
    assert!(s.world.is_lit(pos), "the flame should finish lighting");
    assert!(!s.world.units.iter().any(|u| u.pos == pos), "no unit stands on the flame");
    let radius = s.world.flames()[0].radius as i32;

    let mut app = App::new(s, SIZE);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    app.ui.cursor = Pos::new(pos.x + 5, pos.y); // elsewhere
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let flame = cell_at(&app, &buf, pos);
    assert_eq!(flame.symbol(), "☼");
    let look = p.glyphs.overlay.flame;
    assert_eq!(flame.fg, Color::Rgb(look.fg[0], look.fg[1], look.fg[2]));

    // With the cursor on the flame, the tiles it warms get the warm background; others don't.
    app.ui.cursor = pos;
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let warm = Pos::new(pos.x + radius, pos.y);
    let cold = Pos::new(pos.x + radius + 1, pos.y);
    let warm_cell = cell_at(&app, &buf, warm);
    let cold_cell = cell_at(&app, &buf, cold);
    assert_eq!(warm_cell.bg, p.tinted(ground_bg(&p, &app.session, warm), Tint::Warm));
    assert_eq!(cold_cell.bg, p.color(ground_bg(&p, &app.session, cold)));
}

/// A finished power source's reach tints `Tint::Powered`, the way a flame's does, while the
/// cursor is on it; elsewhere it doesn't (#428, #429).
#[test]
fn a_finished_power_sources_reach_is_tinted_under_the_cursor() {
    let (mut s, source) = session_with_a_test_source(7, Nation::Water, 3, 1.5);
    let at = Pos::new(10, 10);
    place_building(&mut s, source, at);
    s.world.adopt_power_sources(&s.defs);

    let mut app = App::new(s, SIZE);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    app.ui.cursor = Pos::new(at.x + 5, at.y); // elsewhere: no tint yet
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let edge = Pos::new(at.x + 3, at.y);
    let cell = cell_at(&app, &buf, edge);
    assert_eq!(cell.bg, p.color(ground_bg(&p, &app.session, edge)), "no tint off the source");

    app.ui.cursor = at;
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let reached = Pos::new(at.x + 3, at.y);
    let outside = Pos::new(at.x + 4, at.y);
    let reached_cell = cell_at(&app, &buf, reached);
    let outside_cell = cell_at(&app, &buf, outside);
    assert_eq!(reached_cell.bg, p.tinted(ground_bg(&p, &app.session, reached), Tint::Powered));
    assert_eq!(outside_cell.bg, p.color(ground_bg(&p, &app.session, outside)));
}

/// While the Build tool is about to place a power source, its hypothetical reach previews the
/// same tint, whether or not the placement would be refused (#428, #429).
#[test]
fn placing_a_power_source_previews_its_reach() {
    let (s, source) = session_with_a_test_source(7, Nation::Water, 3, 1.5);
    let mut app = App::new(s, SIZE);
    app.ui.cursor = Pos::new(20, 16);
    app.ui.tool = Tool::Build;
    app.ui.choice = Some(source);
    let cursor = app.ui.cursor;

    let p = palette(&app.session, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let reached = Pos::new(cursor.x + 3, cursor.y);
    let outside = Pos::new(cursor.x + 4, cursor.y);
    let reached_cell = cell_at(&app, &buf, reached);
    let outside_cell = cell_at(&app, &buf, outside);
    assert_eq!(reached_cell.bg, p.tinted(ground_bg(&p, &app.session, reached), Tint::Powered));
    assert_eq!(outside_cell.bg, p.color(ground_bg(&p, &app.session, outside)));
}

/// A renewable resource that has been worked and is growing back draws dimmed.
#[test]
fn a_resource_growing_back_is_dim_grey() {
    let mut s = new_session(7, Nation::Earth);
    let from = s.world.units[0].pos;
    let mut renewable: Vec<Pos> = s
        .world
        .map
        .iter()
        .filter(|(_, t)| {
            t.resource.and_then(|r| s.defs.harvest(r)).is_some_and(|h| h.regrow.is_some())
        })
        .map(|(p, _)| p)
        .collect();
    renewable.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
    let target = renewable
        .into_iter()
        .find(|&pos| s.apply(Command::DesignateHarvest { pos }).is_ok())
        .expect("a renewable resource the nation can work");
    for _ in 0..100 {
        s.tick_n(100);
        if s.world.is_resting(target) {
            break;
        }
    }
    assert!(s.world.is_resting(target), "the resource should have been worked");

    let mut app = App::new(s, SIZE);
    app.ui.cursor = target;
    app.ui.cursor.x = (target.x + 4).min(app.session.world.map.width() - 1);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let cell = cell_at(&app, &buf, target);
    let dim = fn_tui::color::to_u8(p.theme.dim);
    let res = app.session.world.map.get(target).unwrap().resource.unwrap();
    let id = &app.session.defs.resource(res).id;
    if !app.session.world.units.iter().any(|u| u.pos == target) {
        assert_eq!(cell.symbol(), p.glyphs.resource[id].glyph.to_string());
        assert_eq!(cell.fg, Color::Rgb(dim[0], dim[1], dim[2]));
    }
}

/// An orchard's young fruit tree (#142) is drawn as a sapling, not as the resting tree it also is.
#[test]
fn an_orchards_sapling_has_its_own_glyph() {
    let mut s = Session::new(3, Nation::Air, 48, 32);
    let home = s.world.units[0].pos;
    let id = s.defs.building_id("orchard").unwrap();
    let passable = s.defs.building(id).passable;
    // The orchard on the open land nearest the settlers, which then plants beside itself.
    let land = |p: Pos| {
        (-4..=4)
            .flat_map(move |dy| (-4..=4).map(move |dx| Pos::new(p.x + dx, p.y + dy)))
            .filter(|q| {
                s.world.map.get(*q).is_some_and(|t| {
                    t.terrain == fn_sim::Terrain::Land
                        && t.resource.is_none()
                        && t.building.is_none()
                })
            })
            .count()
    };
    let mut spots: Vec<Pos> = s.world.map.iter().map(|(p, _)| p).collect();
    spots.sort_by_key(|p| ((p.x - home.x).pow(2) + (p.y - home.y).pow(2), p.x, p.y));
    spots.truncate(400);
    let orchard = spots.into_iter().max_by_key(|&p| land(p)).expect("an open meadow");
    assert!(land(orchard) >= 40, "the open meadow has room to plant");
    s.world.map.get_mut(orchard).unwrap().building =
        Some(fn_sim::building::Building::new(id, passable, false));
    s.world.adopt_orchards(&s.defs);
    s.tick_n(1200);
    let sapling = s
        .world
        .map
        .iter()
        .map(|(p, _)| p)
        .find(|&p| s.world.is_sapling(p) && !s.world.units.iter().any(|u| u.pos == p))
        .expect("the orchard planted a sapling");

    let mut app = App::new(s, SIZE);
    app.ui.cursor = Pos::new(sapling.x, if sapling.y > 0 { sapling.y - 1 } else { 1 });
    for ascii in [false, true] {
        let p = palette(&app.session, ColorDepth::TrueColor, ascii);
        let buf = frame(&mut app, ColorDepth::TrueColor, ascii, SIZE);
        let look = &p.glyphs.overlay.sapling;
        let cell = cell_at(&app, &buf, sapling);
        assert_eq!(cell.symbol(), look.glyph(ascii).to_string());
        assert_eq!(cell.fg, Color::Rgb(look.fg[0], look.fg[1], look.fg[2]));
    }
}

/// A farm plot draws its own glyph while empty, the sim's `overlay.plot_growing` once sown and
/// `overlay.plot_ripe` once it ripens (#198).
#[test]
fn a_farm_plot_shows_its_growing_and_ripe_glyphs() {
    let mut s = Session::new(1, Nation::Earth, 48, 32);
    let plot = Pos::new(s.world.units[0].pos.x + 2, s.world.units[0].pos.y);
    let id = s.defs.building_id("farm_plot").unwrap();
    let passable = s.defs.building(id).passable;
    s.world.map.get_mut(plot).unwrap().building =
        Some(fn_sim::building::Building::new(id, passable, false));
    s.world.adopt_farm_plots(&s.defs);
    s.apply(Command::Sow { pos: plot }).unwrap();

    let mut app = App::new(s, SIZE);
    app.ui.cursor = Pos::new(plot.x, if plot.y > 0 { plot.y - 1 } else { 1 });
    let empty = |app: &App| !app.session.world.units.iter().any(|u| u.pos == plot);
    for _ in 0..3000 {
        if app.session.world.plot_stage(plot) == Some(fn_sim::world::PlotStage::Growing)
            && empty(&app)
        {
            break;
        }
        app.session.tick_n(10);
    }
    assert_eq!(app.session.world.plot_stage(plot), Some(fn_sim::world::PlotStage::Growing));
    assert!(empty(&app), "no unit should be standing on the plot between jobs");
    for ascii in [false, true] {
        let p = palette(&app.session, ColorDepth::TrueColor, ascii);
        let buf = frame(&mut app, ColorDepth::TrueColor, ascii, SIZE);
        let look = &p.glyphs.overlay.plot_growing;
        let cell = cell_at(&app, &buf, plot);
        assert_eq!(cell.symbol(), look.glyph(ascii).to_string());
        assert_eq!(cell.fg, Color::Rgb(look.fg[0], look.fg[1], look.fg[2]));
    }

    for _ in 0..6000 {
        if app.session.world.plot_stage(plot) == Some(fn_sim::world::PlotStage::Ripe) && empty(&app)
        {
            break;
        }
        app.session.tick_n(1);
    }
    assert_eq!(app.session.world.plot_stage(plot), Some(fn_sim::world::PlotStage::Ripe));
    assert!(empty(&app), "no unit should be standing on the plot while it's ripe");
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let look = &p.glyphs.overlay.plot_ripe;
    let cell = cell_at(&app, &buf, plot);
    assert_eq!(cell.symbol(), look.glyph(false).to_string());
    assert_eq!(cell.fg, Color::Rgb(look.fg[0], look.fg[1], look.fg[2]));
}

/// A wet patch (Water the Land) is a dark mud background under whatever else is on the tile.
#[test]
fn a_wet_patch_is_a_mud_background() {
    let mut s = Session::new(2, Nation::Water, 48, 32);
    let ability = fn_ui::tools::ability_for_nation(&s.defs, Nation::Water).unwrap();
    let from = s.world.units[0].pos;
    // Water the nearest tile the sim accepts.
    let mut tiles: Vec<Pos> = s.world.map.iter().map(|(p, _)| p).collect();
    tiles.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
    let target = tiles
        .into_iter()
        .find(|&pos| s.apply(Command::Bend { ability, pos }).is_ok())
        .expect("a tile to water");
    for _ in 0..200 {
        s.tick_n(100);
        if !s.world.wet_tiles().is_empty() {
            break;
        }
    }
    let wet = s.world.wet_tiles();
    assert!(!wet.is_empty(), "the bend should finish and wet some tiles ({target:?})");

    let mut app = App::new(s, SIZE);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    let (pos, _) = wet[0];
    // The cursor beside it, so the cursor's own colour doesn't hide the patch.
    app.ui.cursor = Pos::new(pos.x, if pos.y > 0 { pos.y - 1 } else { 1 });
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let cell = cell_at(&app, &buf, pos);
    assert_eq!(cell.bg, p.tinted(ground_bg(&p, &app.session, pos), Tint::Wet));
    assert_ne!(cell.bg, p.color(ground_bg(&p, &app.session, pos)));
}

// ---------------------------------------------------------------------------------------------
// The moment of a bend (#278): channels, the cast highlight and the Inner Fire aura
// ---------------------------------------------------------------------------------------------

/// Right after a bend lands, the tile it changed flashes the nation's accent, the same hue a
/// designation of it wears; once the short window passes, the tile goes back to plain ground.
#[test]
fn a_bend_flashes_the_nations_accent_right_after_it_lands_then_fades() {
    use fn_sim::world::BendOutcome;

    let mut s = Session::new(1, Nation::Fire, 48, 32);
    let pos = s.world.units[0].pos;
    let ability = fn_ui::tools::ability_for_nation(&s.defs, Nation::Fire).unwrap();
    s.apply(Command::Bend { ability, pos }).unwrap();
    for _ in 0..20_000 {
        s.tick();
        if !s.world.bend_events().is_empty() {
            break;
        }
    }
    assert_eq!(s.world.bend_events().last().map(|e| e.outcome), Some(BendOutcome::Done));

    let mut app = App::new(s, SIZE);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    // Elsewhere, so the cursor's own colour doesn't hide the highlight.
    app.ui.cursor = Pos::new(pos.x, if pos.y > 0 { pos.y - 1 } else { 1 });
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let cell = cell_at(&app, &buf, pos);
    assert_eq!(cell.bg, p.tinted(ground_bg(&p, &app.session, pos), Tint::Bend), "{cell:?}");

    // Well past the short window: it has faded back to plain ground.
    app.session.tick_n(50);
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let cell = cell_at(&app, &buf, pos);
    assert_ne!(cell.bg, p.tinted(ground_bg(&p, &app.session, pos), Tint::Bend));
}

/// A channel in progress, then the highlight right after completion, then an aura carrier: three
/// golden frames of the moment of a bend (#278).
#[test]
fn golden_frame_of_a_cast_highlight() {
    let mut s = Session::new(1, Nation::Fire, 48, 32);
    let pos = s.world.units[0].pos;
    let ability = fn_ui::tools::ability_for_nation(&s.defs, Nation::Fire).unwrap();
    s.apply(Command::Bend { ability, pos }).unwrap();
    for _ in 0..20_000 {
        s.tick();
        if !s.world.bend_events().is_empty() {
            break;
        }
    }
    let mut app = App::new(s, SIZE);
    app.ui.cursor = Pos::new(pos.x, if pos.y > 0 { pos.y - 1 } else { 1 });
    golden(
        "fire_seed1_cast_highlight",
        &to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)),
    );
}

/// A unit carrying Inner Fire's aura wears a warm background wherever it stands, cursor or not;
/// with the cursor on it, its reach is tinted too, the way a lit flame's is.
#[test]
fn an_aura_carrier_wears_a_warm_background_and_warms_its_reach_under_the_cursor() {
    let mut s = Session::new(3, Nation::Fire, 48, 32);
    let dragon = s.defs.tech_id("dancing_dragon").unwrap();
    s.apply(Command::SetResearch { tech: dragon }).unwrap();
    for _ in 0..200_000 {
        if s.world.research().is_known(dragon) {
            break;
        }
        s.tick();
    }
    assert!(s.world.research().is_known(dragon), "Dancing Dragon should be researched by now");

    let inner = s.defs.ability_id("inner_fire").unwrap();
    // Whoever claims the job carries the aura; it need not be `units[0]` (#278, World::glows).
    let start = s.world.units[0].pos;
    s.apply(Command::Bend { ability: inner, pos: start }).unwrap();
    for _ in 0..20_000 {
        s.tick();
        if !s.world.glows().is_empty() {
            break;
        }
    }
    // Past the cast highlight's own short window (its landing is also a `BendEvent`), so only
    // the aura's warmth is left to check.
    s.tick_n(20);
    let glow = s.world.glows().into_iter().next().expect("the aura should be lit");
    let (carrier, radius) = (glow.pos, glow.radius as i32);

    let mut app = App::new(s, SIZE);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    // Elsewhere: the carrier's own tile is still warm without the cursor's help.
    app.ui.cursor = Pos::new(carrier.x, if carrier.y > 0 { carrier.y - 1 } else { 1 });
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let cell = cell_at(&app, &buf, carrier);
    assert_eq!(cell.bg, p.tinted(ground_bg(&p, &app.session, carrier), Tint::Warm), "{cell:?}");

    // The cursor on the carrier: the reach is tinted too, the way a flame's is.
    app.ui.cursor = carrier;
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let warm = Pos::new(carrier.x + radius, carrier.y);
    let cold = Pos::new(carrier.x + radius + 1, carrier.y);
    let warm_cell = cell_at(&app, &buf, warm);
    let cold_cell = cell_at(&app, &buf, cold);
    assert_eq!(warm_cell.bg, p.tinted(ground_bg(&p, &app.session, warm), Tint::Warm));
    assert_eq!(cold_cell.bg, p.color(ground_bg(&p, &app.session, cold)));
}

#[test]
fn golden_frame_of_an_aura_carrier() {
    let mut s = Session::new(3, Nation::Fire, 48, 32);
    let dragon = s.defs.tech_id("dancing_dragon").unwrap();
    s.apply(Command::SetResearch { tech: dragon }).unwrap();
    for _ in 0..200_000 {
        if s.world.research().is_known(dragon) {
            break;
        }
        s.tick();
    }
    let inner = s.defs.ability_id("inner_fire").unwrap();
    let start = s.world.units[0].pos;
    s.apply(Command::Bend { ability: inner, pos: start }).unwrap();
    for _ in 0..20_000 {
        s.tick();
        if !s.world.glows().is_empty() {
            break;
        }
    }
    s.tick_n(20);
    let carrier = s.world.glows()[0].pos;
    let mut app = App::new(s, SIZE);
    app.ui.cursor = carrier;
    golden("fire_seed3_aura", &to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)));
}

//! The panels (#99): selecting units and the selection bar, the side panel's sections, alerts, the
//! message log, key help, and the new-map and nation prompts. Whole-frame goldens live in
//! `reference_goldens.rs` now (#455); everything here is `contains`-style assertions.

use fn_sim::world::Command;
use fn_sim::{Nation, Pos, Terrain};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::layout::panel_width;
use fn_tui::state::{PINNED, Screen};
use fn_tui::widgets::Tone;
use fn_tui::{keys, new_session};
use fn_ui::tools::ability_for_nation;
use fn_ui::{Session, Tool};
use ratatui::style::Modifier;

const SIZE: (u16, u16) = (120, 40);
const SMALL: (u16, u16) = (80, 24);

fn app(seed: u64, nation: Nation, size: (u16, u16)) -> App {
    App::new(new_session(seed, nation), size)
}

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn text(app: &mut App, size: (u16, u16)) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    to_text(&render_app(app, &p, size))
}

/// The unit ids in the order of the world's list.
fn ids(app: &App) -> Vec<fn_sim::world::UnitId> {
    app.session.world.units.iter().map(|u| u.id).collect()
}

/// A Fire game where the first settler has bent a few times, so the log has lines, and one of
/// those bends has failed, so there is a live toast too (#417: a finished bend is routine and
/// log-only now, so the tests that need a live toast have to earn one with a failure).
///
/// Seed 3 because it has enough fuel around the start for a log long enough to scroll (seed 1
/// did until #225 reshuffled the map).
fn bending_game() -> App {
    let mut s = Session::new(3, Nation::Fire, 48, 32);
    let ability = ability_for_nation(&s.defs, Nation::Fire).unwrap();
    let from = s.world.units[0].pos;
    let mut designated = 0;
    // -4..=4 (a 9x9 box) was enough fuel before #349's foothill boulders started blocking a few
    // of its tiles near the mountain this settler starts beside, which otherwise trimmed the
    // message log below the 40-line floor `the_log_scrolls_...` needs; -5..=5 has slack again.
    for dy in -5..=5 {
        for dx in -5..=5 {
            let pos = Pos::new(from.x + dx, from.y + dy);
            if s.apply(Command::Bend { ability, pos }).is_ok() {
                designated += 1;
            }
        }
    }
    assert!(designated > 3, "the settler should have fire to light");
    // The settler's own tile turns to rock before its channel finishes, so that one designation
    // fails instead of finishing (the same technique `fn_ui::alerts`'s own tests use): no
    // walking is involved, since it's already standing there, so this fails at the channel's
    // own validation rather than being quietly abandoned for an unreachable target.
    s.world.map[from].terrain = Terrain::Mountain;
    s.world.map_changed();
    let mut app = App::new(s, SIZE);
    for _ in 0..200 {
        app.play(&[keys::Step::Tick(100)]);
        if app.ui.alerts.history().len() >= 3 && !app.ui.alerts.toasts().is_empty() {
            break;
        }
    }
    assert!(app.ui.alerts.history().len() >= 3, "bending should have been logged");
    assert!(!app.ui.alerts.toasts().is_empty(), "the failed bend should toast");
    app
}

// ---------------------------------------------------------------------------------------------
// Selecting
// ---------------------------------------------------------------------------------------------

#[test]
fn enter_selects_the_unit_under_the_cursor_and_again_the_next_one_there() {
    let mut a = app(7, Nation::Earth, SIZE);
    let ids = ids(&a);
    let spot = a.session.world.units[0].pos;
    a.session.world.units[1].pos = spot;
    a.session.world.units[2].pos = spot;
    a.ui.cursor = spot;
    assert_eq!(a.ui.selected, None);

    run(&mut a, "<enter>");
    assert_eq!(a.ui.selected, Some(ids[0]));
    run(&mut a, "<enter>");
    assert_eq!(a.ui.selected, Some(ids[1]), "again: the next one on that tile");
    run(&mut a, "<enter>");
    assert_eq!(a.ui.selected, Some(ids[2]));
    run(&mut a, "<enter>");
    assert_eq!(a.ui.selected, Some(ids[0]), "and round again");

    // An empty tile changes nothing.
    a.ui.cursor = Pos::new(0, 0);
    run(&mut a, "<enter>");
    assert_eq!(a.ui.selected, Some(ids[0]));
}

#[test]
fn tab_and_shift_tab_cycle_through_the_units_and_the_cursor_follows() {
    let mut a = app(7, Nation::Earth, SIZE);
    let ids = ids(&a);
    run(&mut a, "<tab>");
    assert_eq!(a.ui.selected, Some(ids[0]));
    assert_eq!(a.ui.cursor, a.session.world.units[0].pos);
    run(&mut a, "<tab>");
    assert_eq!(a.ui.selected, Some(ids[1]));
    assert_eq!(a.ui.cursor, a.session.world.units[1].pos);
    run(&mut a, "<s-tab> <s-tab>");
    assert_eq!(a.ui.selected, Some(ids[ids.len() - 1]), "backwards wraps round to the last");
    assert_eq!(a.ui.cursor, a.session.world.units[ids.len() - 1].pos);
    run(&mut a, "<tab>");
    assert_eq!(a.ui.selected, Some(ids[0]), "forwards wraps to the first");

    // Shift+Tab with nothing selected starts at the last unit.
    let mut b = app(7, Nation::Earth, SIZE);
    run(&mut b, "<s-tab>");
    assert_eq!(b.ui.selected, Some(ids[ids.len() - 1]));
}

#[test]
fn escape_closes_the_bar_but_in_a_tool_it_leaves_the_tool_first() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "<tab>");
    assert!(a.ui.selected.is_some());
    run(&mut a, "h");
    // Tab and F are for Select mode.
    let before = a.ui.selected;
    run(&mut a, "<tab> f");
    assert_eq!((a.ui.selected, a.ui.following), (before, false));
    run(&mut a, "<esc>");
    assert_eq!(a.ui.tool, Tool::Select);
    assert!(a.ui.selected.is_some(), "the first Esc left the tool");
    run(&mut a, "<esc>");
    assert_eq!(a.ui.selected, None, "the second closed the bar");
}

#[test]
fn f_follows_the_selected_unit_until_the_cursor_is_moved_by_hand() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "f");
    assert!(!a.ui.following, "nothing selected, nothing to follow");
    run(&mut a, "<tab> f");
    assert!(a.ui.following);
    // Walk the unit away; the cursor and the view go with it.
    let id = a.ui.selected.unwrap();
    let to = Pos::new(a.session.world.units[0].pos.x + 7, a.session.world.units[0].pos.y);
    a.session.world.units.iter_mut().find(|u| u.id == id).unwrap().pos = to;
    run(&mut a, "<tick 1>");
    assert_eq!(a.ui.cursor, to);
    assert!(a.ui.cursor_cell().is_some());
    run(&mut a, "d");
    assert!(!a.ui.following, "moving the cursor by hand stops following");
    run(&mut a, "f");
    assert!(a.ui.following, "F again follows again");
    run(&mut a, "f");
    assert!(!a.ui.following, "and a third time stops");
}

#[test]
fn a_unit_that_dies_closes_the_bar() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "<tab> f");
    let id = a.ui.selected.unwrap();
    a.session.world.units.retain(|u| u.id != id);
    run(&mut a, "<tick 1>");
    assert_eq!((a.ui.selected, a.ui.following), (None, false));
}

#[test]
fn the_view_keeps_the_selected_unit_above_the_bar() {
    let mut a = app(7, Nation::Earth, SIZE);
    let (_, h) = a.ui.viewport();
    // Stand a settler on the bottom edge of the map and select it.
    let bottom = Pos::new(a.session.world.units[0].pos.x, a.session.world.map.height() - 1);
    a.session.world.units[0].pos = bottom;
    a.ui.cursor = bottom;
    run(&mut a, "<enter>");
    let (_, row) = a.ui.cursor_cell().expect("the unit is in view");
    assert!(row < h - 7, "row {row} must clear the 7-row bar on a {h}-row map");
    let frame = text(&mut a, SIZE);
    let lines: Vec<&str> = frame.lines().collect();
    let unit_row = lines[row as usize];
    assert!(unit_row.contains('☺'), "the unit is drawn above the bar:\n{frame}");

    // Closing the bar leaves the view be.
    run(&mut a, "<esc>");
    assert_eq!(a.ui.selected, None);
    assert!(a.ui.cursor_cell().is_some());
}

// ---------------------------------------------------------------------------------------------
// The selection bar
// ---------------------------------------------------------------------------------------------

#[test]
fn the_bar_names_the_unit_and_shows_its_task_cargo_tile_and_meters() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "<tab>");
    let unit = &a.session.world.units[0];
    let (name, pos) = (unit.name.clone(), unit.pos);
    let frame = text(&mut a, SIZE);
    let lines: Vec<&str> = frame.lines().collect();
    // The bar is the 7 rows above the status line, on the map side.
    let top = lines[32];
    assert!(top.starts_with("┏─ ") && top.contains(&format!("{name} · Earth Kingdom")), "{top}");
    let bar = lines[32..39].join("\n");
    for want in ["Idle", "Carrying: nothing", &format!("At ({}, {})", pos.x, pos.y)] {
        assert!(bar.contains(want), "{want:?} in\n{bar}");
    }
    for want in ["Health", "Hunger", "Thirst", "Energy", "Chi"] {
        assert!(bar.contains(want), "{want:?} in\n{bar}");
    }
    assert!(bar.contains("██████████ 100%"), "a full meter is ten cells:\n{bar}");
    // The bar names no keys (#651): the hint bar below is the only place they appear.
    assert!(!bar.contains("[Esc]") && !bar.contains("Esc close"), "{bar}");
    assert!(
        lines[39].contains("Esc close") && lines[39].contains("F follow"),
        "the hint bar has the settler's keys:\n{}",
        lines[39]
    );
    // The side panel runs the full height beside it.
    assert!(lines[36].contains('│') && lines[39].starts_with(" SELECT"), "{frame}");
}

/// A channelling bender's People line and bar say so and how long is left (#278), instead of the
/// plain task every other frontend shows.
#[test]
fn a_channelling_unit_names_the_ability_and_the_time_left_in_the_bar_and_the_people_list() {
    let mut s = Session::new(1, Nation::Fire, 48, 32);
    let pos = s.world.units[0].pos;
    let id = s.world.units[0].id;
    let ability = ability_for_nation(&s.defs, Nation::Fire).unwrap();
    s.apply(Command::Bend { ability, pos }).unwrap();
    for _ in 0..2000 {
        s.tick();
        if s.world.bend_channels(&s.defs).iter().any(|c| c.unit == id) {
            break;
        }
    }
    assert!(s.world.bend_channels(&s.defs).iter().any(|c| c.unit == id), "should be channelling");

    let mut a = App::new(s, SIZE);
    a.ui.select(Some(id));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Channelling Hearthflame"), "{frame}");
    // The bar's own header and rows, as the deleted channelling golden showed them.
    assert!(frame.contains("Fire Nation · Student"), "{frame}");
    assert!(frame.contains("Chi    ██████████"), "{frame}");
    assert!(frame.lines().last().unwrap().contains("Tab next unit · F follow"), "{frame}");
}

#[test]
fn a_meter_that_is_not_ok_has_a_bang_and_the_others_do_not() {
    let mut a = app(7, Nation::Earth, SIZE);
    a.session.world.units[0].needs.energy = 0.0;
    a.session.world.units[0].needs.hunger = 0.0;
    run(&mut a, "<tab>");
    let frame = text(&mut a, SIZE);
    let row = |label: &str| {
        frame
            .lines()
            .skip(32)
            .find(|l| l.contains(label))
            .unwrap_or_else(|| panic!("{label} row in\n{frame}"))
            .to_owned()
    };
    for label in ["Energy", "Hunger"] {
        assert!(row(label).contains("0%!"), "{}", row(label));
    }
    for label in ["Health", "Thirst"] {
        assert!(!row(label).contains('!'), "{}", row(label));
    }
    assert!(!row("Chi").contains('!'));
}

#[test]
fn meters_wear_the_ok_warn_and_bad_tokens() {
    let mut a = app(7, Nation::Earth, SIZE);
    {
        let needs = &mut a.session.world.units[0].needs;
        let max = a.session.defs.needs.max();
        needs.hunger = max; // ok
        needs.thirst = max * a.session.defs.needs.thirst.low / max * 0.8; // warn
        needs.energy = 0.0; // bad
    }
    run(&mut a, "<tab>");
    let p = Palette::new(&a.session.defs, Nation::Earth, ColorDepth::TrueColor, false);
    let buf = render_app(&mut a, &p, SIZE);
    let cell_of = |label: &str| {
        let y = (32..39)
            .find(|&y| (0..120).map(|x| buf[(x, y)].symbol()).collect::<String>().contains(label))
            .unwrap();
        let row: String = (0..120).map(|x| buf[(x, y)].symbol()).collect();
        let x = row.chars().position(|c| c == '█').unwrap() as u16;
        buf[(x, y)].fg
    };
    let want = |rgb| p.token(rgb);
    assert_eq!(cell_of("Hunger"), want(p.theme.ok));
    assert_eq!(cell_of("Thirst"), want(p.theme.warn));
    let energy_y = (32..39)
        .find(|&y| (0..120).map(|x| buf[(x, y)].symbol()).collect::<String>().contains("Energy"))
        .unwrap();
    // A bad meter has no filled cells, so its mark carries the colour.
    let bang = (0..120).find(|&x| buf[(x, energy_y)].symbol() == "!").unwrap();
    assert_eq!(buf[(bang, energy_y)].fg, want(p.theme.bad));
}

#[test]
fn the_selected_unit_is_drawn_in_reverse_video() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "<tab> <right*3>");
    let p = Palette::new(&a.session.defs, Nation::Earth, ColorDepth::TrueColor, false);
    let buf = render_app(&mut a, &p, SIZE);
    let unit = a.session.world.units[0].pos;
    let map = fn_tui::layout::regions(buf.area).unwrap().map;
    let at = |pos: Pos| {
        &buf[(map.x + (pos.x - a.ui.view.x) as u16, map.y + (pos.y - a.ui.view.y) as u16)]
    };
    assert!(at(unit).modifier.contains(Modifier::REVERSED), "the selected unit");
    for other in &a.session.world.units[1..] {
        if other.pos != unit && other.pos != a.ui.cursor {
            let cell = at(other.pos);
            if cell.symbol() == "☺" {
                assert!(!cell.modifier.contains(Modifier::REVERSED), "{:?}", other.name);
            }
        }
    }
}

#[test]
fn the_cursor_section_offers_enter_over_a_unit() {
    let mut a = app(7, Nation::Earth, SIZE);
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Press Enter to select"), "{frame}");
    run(&mut a, "h");
    assert!(!text(&mut a, SIZE).contains("Press Enter to select"), "not in a tool");
}

/// A selected unit's bar: the title, the task, where it stands, the five meters and the key
/// hints under them. A tired unit (energy low) shows its Energy meter short of full.
#[test]
fn a_selected_unit_opens_the_bar_with_its_title_meters_and_hints() {
    let mut a = app(7, Nation::Earth, SIZE);
    a.session.world.units[0].needs.energy = a.session.defs.needs.max() * 0.1;
    run(&mut a, "<tick 200> <tab>");
    let frame = text(&mut a, SIZE);
    let u = &a.session.world.units[0];
    assert!(frame.contains(&format!("{} · Earth Kingdom · Student", u.name)), "{frame}");
    assert!(frame.contains("Carrying: nothing"), "{frame}");
    assert!(frame.contains(&format!("At ({}, {})", u.pos.x, u.pos.y)), "{frame}");
    for meter in ["Health", "Hunger", "Thirst", "Energy", "Chi"] {
        assert!(frame.contains(meter), "{meter} in\n{frame}");
    }
    assert!(
        !frame.contains("Energy ██████████ 100%"),
        "the tired unit's Energy is short:\n{frame}"
    );
    assert!(frame.contains("To adept: 0 of "), "{frame}");
}

/// The same bar in ASCII: pure ASCII, with `#` for the meter cells and `-` for the frame.
#[test]
fn an_ascii_selected_unit_bar_is_pure_ascii() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "<tab>");
    let p = Palette::new(&a.session.defs, Nation::Earth, ColorDepth::Never, true);
    let frame = to_text(&render_app(&mut a, &p, SIZE));
    assert!(frame.is_ascii(), "{frame}");
    assert!(frame.contains("Health ########## 100%"), "{frame}");
    assert!(frame.lines().last().unwrap().contains("Tab next unit - F follow"), "{frame}");
    assert!(frame.contains("+- "), "{frame}");
}

// ---------------------------------------------------------------------------------------------
// The side panel
// ---------------------------------------------------------------------------------------------

/// Pushes `a`'s settlement to 20 units (#310's own test scenario), cloning the first one with a
/// fresh id so every other id stays unique.
fn push_to_twenty(a: &mut App) {
    while a.session.world.units.len() < 20 {
        let mut u = a.session.world.units[0].clone();
        u.id = fn_sim::world::UnitId(1000 + a.session.world.units.len() as u32);
        a.session.world.units.push(u);
    }
}

/// #416: the People row is always one line ("People N" left, "Idle N" right), whatever the
/// settlement's size -- the activity breakdown (Haul, Craft, ...) moved to the Nation screen
/// (`N`, #308). A starving unit still flags on its own row ("Starving 1"), in the bad tone.
#[test]
fn the_people_row_is_one_line_and_a_starving_unit_flags_in_the_bad_tone() {
    let mut a = app(7, Nation::Earth, SIZE);
    push_to_twenty(&mut a);
    a.session.world.units[0].needs.hunger = 0.0;
    assert_eq!(a.session.world.units.len(), 20);

    let p = Palette::new(&a.session.defs, a.session.world.nation, ColorDepth::TrueColor, false);
    let buf = render_app(&mut a, &p, SIZE);
    let frame = to_text(&buf);
    assert!(frame.contains("People 20") && frame.contains("Idle"), "{frame}");
    assert!(frame.contains("Starving 1"), "{frame}");
    assert!(frame.contains("[N]"), "{frame}");
    let row = frame.lines().position(|l| l.contains("Starving 1")).unwrap();
    let col = frame.lines().nth(row).unwrap().chars().position(|c| c == 'S').unwrap();
    let want = Tone::Bad.style(&p).fg;
    assert_eq!(buf[(col as u16, row as u16)].fg, want.unwrap());

    let mut b = app(7, Nation::Earth, SMALL);
    push_to_twenty(&mut b);
    let frame = text(&mut b, SMALL);
    assert!(frame.contains("People 20") && frame.contains("HERE"), "{frame}");
}

#[test]
fn research_shows_the_target_bar_and_rate() {
    let mut a = app(7, Nation::Earth, SIZE);
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Research: none set"), "{frame}");

    let tech = fn_ui::research::screen_view(&a.session)
        .columns
        .iter()
        .flatten()
        .find(|c| c.state == fn_ui::research::TechState::Available)
        .expect("an available tech")
        .clone();
    a.session.apply(Command::SetResearch { tech: tech.id }).unwrap();
    run(&mut a, "<tick 600>");
    let frame = text(&mut a, SIZE);
    assert!(frame.contains(&tech.name), "the target is named:\n{frame}");
    assert!(frame.contains('%'), "{frame}");
    assert!(frame.contains('░') || frame.contains('█'), "with a bar:\n{frame}");
}

#[test]
fn materials_appear_once_something_is_stockpiled() {
    let mut a = app(7, Nation::Earth, SIZE);
    assert!(fn_ui::view::materials(&a.session).is_empty(), "a new game has stored nothing");
    let from = a.session.world.units[0].pos;
    let near: Vec<Pos> = a
        .session
        .world
        .map
        .iter()
        .filter(|(p, t)| {
            (p.x - from.x).abs() < 12
                && (p.y - from.y).abs() < 8
                && t.resource.is_some_and(|r| a.session.defs.harvest(r).is_some())
        })
        .map(|(p, _)| p)
        .collect();
    for p in near {
        let _ = a.session.apply(Command::DesignateHarvest { pos: p });
    }
    let mut zone: Vec<Pos> = a
        .session
        .world
        .map
        .iter()
        .filter(|(_, t)| t.terrain == fn_sim::Terrain::Land && t.resource.is_none())
        .map(|(p, _)| p)
        .collect();
    zone.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
    for pos in zone.into_iter().take(4) {
        a.session.apply(Command::DesignateStockpile { pos }).unwrap();
    }
    for _ in 0..40 {
        run(&mut a, "<tick 100>");
        if !fn_ui::view::materials(&a.session).is_empty() {
            break;
        }
    }
    let (item, count) = fn_ui::view::materials(&a.session).remove(0);
    let frame = text(&mut a, SIZE);
    assert!(frame.contains(&format!("{item} {count}")), "{frame}");
}

#[test]
fn the_sections_come_in_ui_md_order() {
    let mut a = bending_game();
    run(&mut a, "b");
    let frame = text(&mut a, SIZE);
    let at = |s: &str| frame.find(s).unwrap_or_else(|| panic!("{s} in\n{frame}"));
    // Zone 1 (SETTLEMENT, #416) is never trimmed, so it always shows; ALERTS may give way to
    // nothing but its heading on a tight panel. The Tool section's heading is the tool's own
    // name, not the literal word "TOOL" (docs/design/side-panel.md "Headings"), but "BEND" also
    // names itself in the active tool banner's own row (#307) above the panel, so this looks for
    // its heading's own key bracket instead; the seed moved to the panel's bottom edge, after
    // HERE, as "seed" (lower case).
    let order: Vec<&str> = ["SETTLEMENT", "ALERTS", "[B]", "HERE", "seed"]
        .into_iter()
        .filter(|s| frame.contains(s))
        .collect();
    assert!(order.contains(&"SETTLEMENT") && order.contains(&"HERE"), "{frame}");
    for pair in order.windows(2) {
        // Compare by row: the map shares each row with the panel.
        let row = |s: &str| frame[..at(s)].matches('\n').count();
        assert!(row(pair[0]) < row(pair[1]), "{} before {}\n{frame}", pair[0], pair[1]);
    }
}

#[test]
fn a_small_terminal_keeps_the_cursor_and_settlement_glance_with_every_section_on() {
    for nation in Nation::ALL {
        let mut a = bending_game_for(nation);
        run(&mut a, "b <enter>");
        let frame = text(&mut a, SMALL);
        assert!(frame.contains("HERE"), "{nation:?}\n{frame}");
        assert!(frame.contains("Enter: bend"), "{nation:?}\n{frame}");
        assert!(frame.contains("SETTLEMENT"), "{nation:?}\n{frame}");
    }
}

fn bending_game_for(nation: Nation) -> App {
    if nation == Nation::Fire {
        return bending_game();
    }
    app(3, nation, SMALL)
}

// ---------------------------------------------------------------------------------------------
// Divider headings (#309, docs/design/side-panel.md "Headings")
// ---------------------------------------------------------------------------------------------

/// At 80x24, with the Harvest tool active and a notice showing, every section's heading still
/// fits on screen (docs/design/side-panel.md's own shapes, #309's acceptance criteria).
#[test]
fn every_heading_is_on_screen_at_80x24_with_a_tool_and_a_notice() {
    let mut a = app(7, Nation::Earth, SMALL);
    run(&mut a, "h <enter> <right*2> <enter> <enter>");
    assert!(!a.ui.notice.is_empty(), "there should be a result to report");
    let frame = text(&mut a, SMALL);
    // Stores hides itself with nothing stockpiled yet (own test: `stores_appear_once_...`); it
    // isn't exercised here.
    for heading in ["SETTLEMENT", "HARVEST", "HERE"] {
        assert!(frame.contains(heading), "{heading} in\n{frame}");
    }
}

/// `--ascii --color never`: every heading row starts with `+-` and names its section, shape
/// alone carrying the divider (no colour to rely on).
#[test]
fn ascii_headings_start_with_a_plus_dash_and_name_their_section() {
    let mut a = app(7, Nation::Earth, SMALL);
    run(&mut a, "h <enter> <right*2> <enter> <enter>");
    let p = Palette::new(&a.session.defs, a.session.world.nation, ColorDepth::Never, true);
    let frame = to_text(&render_app(&mut a, &p, SMALL));
    let headings: Vec<&str> = frame.lines().filter(|l| l.contains("+-")).collect();
    assert!(!headings.is_empty(), "{frame}");
    for section in ["SETTLEMENT", "HARVEST", "HERE"] {
        assert!(
            headings.iter().any(|l| l.contains("+-") && l.contains(section)),
            "{section} in\n{}",
            headings.join("\n")
        );
    }
}

/// The Tool section's heading wears the active tool's own tint (docs/design/side-panel.md
/// "Headings": "the title is the tool's name in the tool's tint"), the same colour the active
/// tool banner and the status badge wear (#307).
#[test]
fn the_tool_heading_wears_the_active_tools_tint() {
    let mut a = App::new(new_session(7, Nation::Earth), SIZE);
    run(&mut a, "h");
    let p = Palette::new(&a.session.defs, a.session.world.nation, ColorDepth::TrueColor, false);
    let buf = render_app(&mut a, &p, SIZE);
    let frame = to_text(&buf);
    let row = frame.lines().position(|l| l.contains("HARVEST") && l.contains("[H]")).unwrap();
    let line = frame.lines().nth(row).unwrap();
    // The character column, not the byte offset: the map side of the same row is full of
    // multi-byte glyphs.
    let col = line.chars().position(|c| c == 'H').unwrap();
    let want = p.color(p.tint_rgb(fn_tui::color::Tint::Harvest));
    assert_eq!(buf[(col as u16, row as u16)].fg, want);
}

// ---------------------------------------------------------------------------------------------
// Alerts and the message log
// ---------------------------------------------------------------------------------------------

/// A failed bend becomes an alert in the side panel (and the log); a finished one is routine
/// (#417), so it's only ever a line in the log, never a toast.
#[test]
fn a_failed_bend_becomes_an_alert_but_a_finished_one_is_log_only() {
    let mut a = bending_game();
    let frame = text(&mut a, SIZE);
    let failed =
        a.ui.alerts
            .history()
            .iter()
            .find(|e| e.text.contains("failed"))
            .expect("bending_game designated a failure")
            .text
            .clone();
    let done =
        a.ui.alerts
            .history()
            .iter()
            .find(|e| !e.text.contains("failed"))
            .expect("bending_game designated plenty that finished")
            .text
            .clone();
    assert!(frame.contains("ALERTS"), "{frame}");
    assert!(frame.contains(&failed[..failed.len().min(26)]), "{failed}\n{frame}");
    assert!(
        !frame.contains(&done[..done.len().min(26)]),
        "a finished bend is log-only: {done}\n{frame}"
    );

    run(&mut a, "l");
    assert_eq!(a.ui.screen, Some(Screen::Log));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("MESSAGE LOG"), "{frame}");
    let history: Vec<String> = a.ui.alerts.history().iter().map(|e| e.text.clone()).collect();
    // Newest last: every line is there, in order.
    let mut from = 0;
    for line in &history {
        let at = frame[from..]
            .find(&line[..line.len().min(20)])
            .unwrap_or_else(|| panic!("{line} in\n{frame}"));
        from += at;
    }
    assert!(frame.contains("00:"), "each line is stamped with the game time:\n{frame}");
}

/// At 80x24, with Season on, Endeavor in the data and the Bend tool active, fitting is tight
/// enough to want to drop the ALERTS section entirely, but a live toast must keep the heading
/// and its own row on screen (#415): giving up alerts "beyond the newest" should never mean
/// giving up the newest too.
#[test]
fn alerts_keep_the_heading_and_the_newest_row_at_80x24() {
    let mut a = bending_game();
    a.session.world.set_difficulty(Some("normal".to_owned()));
    a.resize(SMALL.0, SMALL.1);
    run(&mut a, "b <enter> <enter>");
    assert!(!a.ui.alerts.toasts().is_empty(), "there should be a live toast");
    assert_eq!(a.ui.tool, Tool::Bend, "the Bend tool should be active");
    let newest = a.ui.alerts.toasts().last().unwrap().text.clone();
    let frame = text(&mut a, SMALL);
    assert!(frame.contains("ALERTS"), "the heading should stay:\n{frame}");
    assert!(frame.contains(&newest[..newest.len().min(20)]), "{newest}\n{frame}");
}

#[test]
fn toasts_age_out_by_real_seconds_but_the_log_keeps_them() {
    let mut a = bending_game();
    assert!(!a.ui.alerts.toasts().is_empty());
    let kept = a.ui.alerts.history().len();
    a.session.clock.paused = true;
    // Paused: no ticks, but real time passes and the toasts go.
    for _ in 0..20 {
        a.age(1.0);
    }
    assert!(a.ui.alerts.toasts().is_empty());
    assert!(!text(&mut a, SIZE).contains("ALERTS"));
    assert_eq!(a.ui.alerts.history().len(), kept);
    run(&mut a, "l");
    assert!(text(&mut a, SIZE).contains("MESSAGE LOG"));
}

#[test]
fn deaths_are_logged() {
    let mut a = app(7, Nation::Earth, SIZE);
    // Starve one settler: with no hunger or thirst left they die in a few seconds.
    a.session.world.units[0].needs.hunger = 0.0;
    a.session.world.units[0].needs.thirst = 0.0;
    a.session.world.units[0].needs.energy = 0.0;
    a.session.world.units[0].needs.health = 0.01;
    for _ in 0..50 {
        run(&mut a, "<tick 20>");
        if a.session.world.deaths().next().is_some() {
            break;
        }
    }
    assert!(a.session.world.deaths().next().is_some(), "the settler should have died");
    run(&mut a, "l");
    let frame = text(&mut a, SIZE);
    assert!(frame.contains(" died of "), "{frame}");
}

/// Strips box-drawing borders and folds runs of whitespace to one space, including a line
/// break: a long alert now carries its mastery count too (#347), long enough for the smallest
/// terminal to wrap it across two rows, and folding reads it back as the one contiguous phrase
/// `widgets::wrap` broke apart only to fit the column.
fn fold(s: &str) -> String {
    s.chars()
        .map(|c| if "│┃┏┓┗┛┣┫─◆▶".contains(c) { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `frame`'s left side only (the map or an overlay like the Log screen), folded
/// (`layout::panel_width` gives the side panel's own width): the side panel's unrelated text
/// on the same physical row must never complete a phrase a wrapped alert line split in two.
fn left_text(frame: &str) -> String {
    let width = frame.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let split_at = width.saturating_sub(usize::from(panel_width(width as u16)));
    let left: String = frame
        .lines()
        .map(|line| {
            let chars: Vec<char> = line.chars().collect();
            let at = split_at.min(chars.len());
            chars[..at].iter().collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ");
    fold(&left)
}

#[test]
fn the_log_scrolls_with_the_arrows_ws_and_the_page_keys_and_sticks_to_the_end() {
    let mut a = bending_game();
    // Enough lines to need more than two pages at the smallest size (the paging steps below need
    // that). Runs on until the settlers have bent and starved, so the count is the whole story.
    let mut guard = 0;
    while a.ui.alerts.history().len() < 60 && guard < 400 {
        run(&mut a, "<tick 100>");
        guard += 1;
    }
    a.resize(SMALL.0, SMALL.1);
    let n = a.ui.alerts.history().len();
    assert!(n >= 40, "only {n} log lines: pick a seed with more fuel near the start");
    run(&mut a, "l");
    assert_eq!(a.ui.scroll, PINNED, "the log opens on its newest line");
    let newest = fold(&a.ui.alerts.history()[n - 1].text);
    let frame = text(&mut a, SMALL);
    if n > 22 {
        assert!(left_text(&frame).contains(&newest), "{frame}");
        let oldest = fold(&a.ui.alerts.history()[0].text);
        assert!(!left_text(&frame).contains(&oldest), "scrolled to the end:\n{frame}");

        run(&mut a, "<up>");
        let one_up = a.ui.scroll;
        assert_ne!(one_up, PINNED);
        run(&mut a, "w");
        assert_eq!(a.ui.scroll, one_up - 1, "W scrolls like the arrow");
        run(&mut a, "s <down>");
        assert_eq!(a.ui.scroll, PINNED, "back at the end it sticks again");
        run(&mut a, "<pgup>");
        let page = a.ui.scroll;
        run(&mut a, "<pgup>");
        assert!(a.ui.scroll < page);
        run(&mut a, "<pgup*40>");
        assert_eq!(a.ui.scroll, 0, "the top");
        let frame = text(&mut a, SMALL);
        assert!(left_text(&frame).contains(&oldest), "{frame}");
        run(&mut a, "<pgdn*40>");
        assert_eq!(a.ui.scroll, PINNED);
    }
    // Esc closes it, and L opens and closes it too.
    run(&mut a, "<esc>");
    assert_eq!(a.ui.screen, None);
    run(&mut a, "l l");
    assert_eq!(a.ui.screen, None);
}

/// The message log screen: its heading and scroll hints, the newest line (the log opens
/// pinned to the end), and the Alerts section of the side panel beside it.
#[test]
fn the_message_log_screen_shows_its_heading_hints_and_newest_line() {
    let mut a = bending_game();
    run(&mut a, "l");
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("◆ MESSAGE LOG ◆"), "{frame}");
    let status = frame.lines().last().unwrap();
    assert!(status.starts_with(" LOG "), "{status}");
    assert!(status.contains("Up/Down scroll · Esc or L close · PgUp/PgDn page"), "{status}");
    assert!(status.trim_end().ends_with("? keys"), "{status}");
    let newest = a.ui.alerts.history().last().expect("the bending game has a log").text.clone();
    let newest: String = newest.chars().take(30).collect();
    assert!(frame.contains(&newest), "the newest line ({newest:?}) in\n{frame}");
    assert!(frame.contains("ALERTS"), "{frame}");
}

#[test]
fn a_screen_hides_the_bar_and_takes_the_keys_but_not_the_clock() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "<tab> l");
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("MESSAGE LOG") && !frame.contains("Carrying:"), "{frame}");
    assert!(frame.contains("EARTH KINGDOM"), "the side panel stays:\n{frame}");
    assert!(frame.lines().last().unwrap().starts_with(" LOG "), "{frame}");
    let (cursor, tool) = (a.ui.cursor, a.ui.tool);
    run(&mut a, "h d <enter> <tab>");
    assert_eq!((a.ui.cursor, a.ui.tool), (cursor, tool), "the map keys wait");
    run(&mut a, "<space>");
    assert!(a.session.clock.paused, "but the clock keys work");
    run(&mut a, "<esc>");
    assert_eq!(a.ui.screen, None);
    assert!(a.ui.selected.is_some(), "the bar comes back");
    assert!(text(&mut a, SIZE).contains("Carrying:"));
}

// ---------------------------------------------------------------------------------------------
// Key help
// ---------------------------------------------------------------------------------------------

#[test]
fn the_help_lists_the_keys_from_the_tables() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "?");
    assert_eq!(a.ui.screen, Some(Screen::Help));
    // The table has grown past one 120x40 screen, so this pages to the end collecting every
    // frame's text instead of assuming it all shows at once.
    let mut seen = text(&mut a, SIZE);
    loop {
        let before = a.ui.scroll;
        run(&mut a, "<pgdn>");
        if a.ui.scroll == before {
            break;
        }
        seen.push_str(&text(&mut a, SIZE));
    }
    assert!(seen.contains("KEY HELP"), "{seen}");
    for row in fn_tui::input::help_rows() {
        let what = &row.what[..row.what.len().min(18)];
        assert!(seen.contains(what), "{what:?} in\n{seen}");
    }
    for tool in [Tool::Harvest, Tool::CancelHarvest, Tool::Stockpile, Tool::Bend] {
        assert!(seen.contains(&format!("{} tool", tool.name())), "{seen}");
    }
    // `?` closes it again, and L switches to the log.
    run(&mut a, "l");
    assert_eq!(a.ui.screen, Some(Screen::Log));
    run(&mut a, "? ?");
    assert_eq!(a.ui.screen, None);
}

#[test]
fn the_help_scrolls_on_a_small_terminal_and_stops_at_its_ends() {
    let mut a = app(7, Nation::Earth, SMALL);
    run(&mut a, "?");
    assert_eq!(a.ui.scroll, 0);
    let top = text(&mut a, SMALL);
    assert!(top.contains("CURSOR AND VIEW"), "{top}");
    run(&mut a, "<up>");
    assert_eq!(a.ui.scroll, 0, "no scrolling above the top");
    run(&mut a, "<pgdn*20>");
    let end = a.ui.scroll;
    assert!(end > 0, "80x24 cannot show the whole help");
    run(&mut a, "<down*5>");
    assert_eq!(a.ui.scroll, end, "or below the end");
    let bottom = text(&mut a, SMALL);
    assert!(bottom.contains("Quit at once"), "the last row is reachable:\n{bottom}");
    assert!(!bottom.contains("CURSOR AND VIEW"));
}

/// The key help screen: its heading and footer, and its first sections (the last row is
/// covered by `the_help_scrolls_on_a_small_terminal_and_stops_at_its_ends`).
#[test]
fn the_key_help_screen_lists_its_sections_and_keys() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "?");
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("◆ KEY HELP ◆"), "{frame}");
    for want in [
        "CURSOR AND VIEW",
        "UNITS",
        "DESIGNATING",
        "Harvest tool: designate harvest (chop, mine, gather)",
    ] {
        assert!(frame.contains(want), "{want:?} in\n{frame}");
    }
    let status = frame.lines().last().unwrap();
    assert!(status.starts_with(" KEYS "), "{status}");
    assert!(status.contains("Up/Down scroll · Esc or ? close · PgUp/PgDn page"), "{status}");
}

/// `G` opens the Endeavor screen (#242): the same screen for every nation, since the signs and
/// stages are shared; the gate's own per-nation roof is on the map, not this screen.
#[test]
fn the_endeavor_screen_lists_the_gates_groups_and_signs() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "g");
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("◆ ENDEAVOR ◆"), "{frame}");
    for want in [
        "The Harmony Gate",
        "0 of 9 signs",
        "Stage: not begun",
        "PEOPLE",
        "Population",
        "BUILDINGS",
        "KNOWLEDGE",
        "STORES AND HARVESTS",
        "BENDING",
    ] {
        assert!(frame.contains(want), "{want:?} in\n{frame}");
    }
}

/// Before the first chart the scrolls sign is closed (#403, owner decisions 2026-11-09): a
/// marked surface scroll and an unmarked one are both absent, and a lost ruin a Glide uncovered
/// (one no chart recorded) is the only row. The open gate is covered through the agent surface
/// (`surface.rs`, which can write a finished chart into the save).
#[test]
fn before_the_first_chart_the_scrolls_sign_lists_only_a_glide_ruin() {
    let mut a = app(7, Nation::Earth, SIZE);
    let scroll = a.session.defs.resource_id("ancient_scroll").unwrap();
    let ruins = a.session.defs.resource_id("lost_ruins").unwrap();
    let anchor = a.session.world.settlement_anchor();
    let mut spots: Vec<Pos> = a
        .session
        .world
        .map
        .iter()
        .filter(|(p, t)| {
            let d = anchor.chebyshev(*p);
            t.terrain == Terrain::Land
                && t.resource.is_none()
                && t.building.is_none()
                && (3..=9).contains(&d)
        })
        .map(|(p, _)| p)
        .collect();
    spots.sort_by_key(|p| (anchor.chebyshev(*p), p.y, p.x));
    let (unmarked, marked, ruin) = (spots[0], spots[1], spots[2]);
    a.session.world.map[unmarked].resource = Some(scroll);
    a.session.world.map[marked].resource = Some(scroll);
    a.session.world.map[ruin].resource = Some(ruins);
    a.session.apply(Command::DesignateHarvest { pos: marked }).unwrap();

    run(&mut a, "g");
    let frame = text(&mut a, SIZE);
    assert!(!frame.contains("Ancient Scroll - "), "no surface scroll yet\n{frame}");
    let ruin_row = frame.lines().find(|l| l.contains("Lost Ruins (2) - ")).expect(&frame);
    assert!(ruin_row.contains("tiles") && ruin_row.contains("not marked"), "{ruin_row}");
    // The hint still closes the sign, and points at cartography while it is unknown.
    assert!(frame.contains("Bring scrolls to a study hall. Cartography charts ruins."), "{frame}");
}

/// With nothing marked or charted the scrolls sign is its name, its count and its hint: no list,
/// no number of scrolls left to find, no direction (#403).
#[test]
fn with_nothing_marked_or_charted_the_scrolls_sign_is_just_its_hint() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "g");
    let frame = text(&mut a, SIZE);
    let lines: Vec<&str> = frame.lines().collect();
    let at = lines.iter().position(|l| l.contains("Ancient scrolls read")).expect("the sign");
    assert!(
        lines[at + 1].contains("Bring scrolls to a study hall. Cartography charts ruins."),
        "{frame}"
    );
    assert!(!frame.contains("Ancient Scroll -"), "{frame}");
    assert!(!frame.contains("Lost Ruins"), "{frame}");
}

/// `G` toggles the screen like every other screen key, closes it the same as `Esc`, and gives
/// way to `T` and `L` opening their own screens (only one screen covers the map at a time).
#[test]
fn g_opens_and_closes_the_endeavor_screen_and_gives_way_to_other_screens() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "g");
    assert_eq!(a.ui.screen, Some(Screen::Endeavor));
    run(&mut a, "g");
    assert_eq!(a.ui.screen, None, "G toggles it");
    run(&mut a, "g t");
    assert_eq!(a.ui.screen, Some(Screen::Research), "T replaces it");
    run(&mut a, "g l");
    assert_eq!(a.ui.screen, Some(Screen::Log), "L replaces it too");
    run(&mut a, "g <esc>");
    assert_eq!(a.ui.screen, None, "Esc closes it like any other screen");
}

// ---------------------------------------------------------------------------------------------
// New map and nation
// ---------------------------------------------------------------------------------------------

/// The menu's route to the New map… box (#655): `Esc` opens it with nothing selected.
const NEW_MAP: &str = "<esc> <down*4> <enter>";

#[test]
fn new_map_starts_only_after_its_box_is_answered() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "<tick 50> r");
    assert_eq!(a.ui.prompt, None, "R is no key any more");
    run(&mut a, NEW_MAP);
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("NEW MAP") && frame.contains("seed 8"), "{frame}");
    assert_eq!(a.session.world.seed, 7, "nothing happens until the answer");

    run(&mut a, "y n");
    assert_eq!((a.session.world.seed, a.session.world.tick), (7, 50));
    assert!(a.ui.prompt.is_some(), "only a row answers");

    run(&mut a, "<esc>");
    assert_eq!(a.session.world.seed, 7, "Esc stays");
    // Confirming no longer restarts in place (#290): it flags that `Shell` should reopen
    // embark on the next seed instead, and leaves this game exactly as it was until that
    // happens (`crate::shell::Shell::key`, tested at that level in `tests/title.rs`).
    run(&mut a, &format!("{NEW_MAP} <down> <enter>"));
    assert!(a.reopen_embark);
    assert_eq!(a.session.world.seed, 7, "App itself doesn't restart; Shell does");
    assert!(a.ui.prompt.is_none());
}

/// The menu's route to the nation at `index` on Switch nation…'s list, as far as its box.
fn switch_box(index: usize) -> String {
    let down = if index == 0 { String::new() } else { format!("<down*{index}> ") };
    format!("<esc> <down*5> <enter> {down}<enter>")
}

/// The whole route, answered "Start over without saving" (no save directory: Keep, Go).
fn switch_to(index: usize) -> String {
    format!("{} <down> <enter>", switch_box(index))
}

#[test]
fn switch_nation_restarts_on_the_same_seed_after_its_box_is_answered() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "2");
    assert_eq!(a.ui.prompt, None, "the number row is no key any more");
    run(&mut a, &switch_box(1));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("SWITCH NATION") && frame.contains("Water Tribes"), "{frame}");
    run(&mut a, "x <esc>");
    assert_eq!(a.session.world.nation, Nation::Earth);

    for (index, nation) in
        [(0, Nation::Air), (1, Nation::Water), (2, Nation::Earth), (3, Nation::Fire)]
    {
        run(&mut a, &switch_to(index));
        assert_eq!((a.session.world.nation, a.session.world.seed), (nation, 7));
        let frame = text(&mut a, SIZE);
        let name = a.session.defs.nation(nation).name.to_uppercase();
        assert!(frame.contains(&name), "{name}\n{frame}");
    }
}

/// `App::restart` (here, through the menu's Switch nation…, the one question it still answers
/// in place since New map… leaves restarting to `Shell`, #290) keeps the screen size and shows the
/// cursor on the new world's first settler.
#[test]
fn restarting_in_place_keeps_the_screen_size_and_shows_the_cursor_on_its_first_settler() {
    let mut a = app(7, Nation::Earth, SMALL);
    run(&mut a, &switch_to(1));
    assert_eq!(a.session.world.nation, Nation::Water);
    assert_eq!(a.ui.viewport(), (48, 23));
    assert!(a.ui.cursor_cell().is_some());
    assert_eq!(a.ui.cursor, a.session.world.units[0].pos);
}

/// The nation switch (#290): with no `start` (a world made by `App::new`'s plain
/// `Session::new`), it keeps the centre-spiral spawn; founded at a chosen site, it rebuilds
/// there instead of snapping back to the centre.
#[test]
fn the_nation_switch_rebuilds_at_the_world_s_start_when_it_has_one() {
    let mut a = app(7, Nation::Earth, SIZE);
    assert_eq!(a.session.world.start(), None);
    run(&mut a, &switch_to(1));
    assert_eq!(a.session.world.start(), None, "no start to keep");

    let defs = fn_sim::Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    let cfg = fn_sim::embark::Survey::config(7, size);
    let survey = fn_sim::embark::Survey::new(&cfg, &defs);
    let start = survey.default_site(&defs);
    let session = Session::found(survey, Nation::Earth, start, None).unwrap();
    let mut a = App::new(session, SIZE);
    assert_eq!(a.session.world.start(), Some(start));

    run(&mut a, &switch_to(1));
    assert_eq!(a.session.world.nation, Nation::Water);
    assert_eq!(a.session.world.start(), Some(start), "rebuilt at the same start");
    let radius = defs.embark.spawn_radius as u32;
    for u in &a.session.world.units {
        assert!(
            u.pos.x.abs_diff(start.x) <= radius && u.pos.y.abs_diff(start.y) <= radius,
            "{:?} is far from the kept start {start:?}",
            u.pos
        );
    }
}

/// `App::restart` (here, through the nation switch) carries the current world's
/// difficulty preset over (#298): it is a new game, not a change of preset mid-game, on both
/// the plain and the founded path.
#[test]
fn restart_carries_the_current_difficulty_preset_over() {
    let mut a = app(7, Nation::Earth, SIZE);
    a.session.world.set_difficulty(Some("hard".into()));
    run(&mut a, &switch_to(1));
    assert_eq!(a.session.world.difficulty(), Some("hard"), "the plain path keeps it too");

    let defs = fn_sim::Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    let cfg = fn_sim::embark::Survey::config(7, size);
    let survey = fn_sim::embark::Survey::new(&cfg, &defs);
    let start = survey.default_site(&defs);
    let session = Session::found(survey, Nation::Earth, start, Some("hard".into())).unwrap();
    let mut a = App::new(session, SIZE);
    run(&mut a, &switch_to(1));
    assert_eq!(a.session.world.nation, Nation::Water);
    assert_eq!(a.session.world.difficulty(), Some("hard"), "found keeps it across the switch");
}

#[test]
fn no_key_mix_breaks_a_frame_on_any_size_or_nation() {
    let script = "<tab> f <tab> <s-tab> l <pgdn> <esc> ? <down*3> <esc> h <enter> <right*4> <enter> \
                  <esc> <esc> <esc> <tick 300> D <enter> <tab> <esc> r n 3 y <tab> <tick 50>";
    for nation in Nation::ALL {
        for size in [SMALL, (100, 30), SIZE, (200, 60)] {
            let mut a = app(3, nation, size);
            run(&mut a, script);
            let frame = text(&mut a, size);
            assert_eq!(frame.lines().count(), usize::from(size.1));
            assert!(frame.lines().all(|l| l.chars().count() <= usize::from(size.0)));
        }
    }
}

#[test]
fn selecting_issues_no_commands_so_a_replay_needs_none() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "<tab> f <tab> l <esc> ? <esc> <tick 100>");
    assert!(a.session.world.command_log().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Seasons and strain, made readable (#299, docs/design/difficulty.md "TUI")
// ---------------------------------------------------------------------------------------------

/// Ticks in one season, at the bundled `data/seasons.toml` numbers.
fn season_ticks(a: &App) -> u64 {
    let minutes = f64::from(a.session.defs.seasons.calendar.season_minutes);
    (minutes * 60.0 * f64::from(fn_sim::world::TICKS_PER_SECOND)) as u64
}

#[test]
fn the_season_section_in_autumn_warns_about_low_stores() {
    let mut a = app(7, Nation::Earth, SMALL);
    a.session.world.set_difficulty(Some("normal".to_owned()));
    let per_season = season_ticks(&a);
    // Spring(0), summer(1), autumn(2): nothing has been stockpiled, so food per head is 0 and
    // the Season section's food line warns.
    run(&mut a, &format!("<tick {}>", per_season * 2 + 1));
    assert_eq!(a.session.world.season(&a.session.defs).id, "autumn");
    let frame = text(&mut a, SMALL);
    assert!(frame.contains("Autumn"), "{frame}");
    assert!(frame.contains("Food 0 · 0 each"), "{frame}");
}

/// #416's own acceptance criteria: at 80x24, with Season, the Gate (an endeavor is always in
/// the bundled data), a need flag and a tool active, every row of the Settlement glance still
/// shows.
#[test]
fn every_settlement_glance_row_is_on_screen_at_80x24_with_everything_on() {
    let mut a = app(7, Nation::Earth, SMALL);
    a.session.world.set_difficulty(Some("normal".to_owned()));
    // The same warn-only formula `meters_wear_the_ok_warn_and_bad_tokens` uses: 80% of the low
    // threshold flags the need (Hungry) without killing the settler (Starving).
    a.session.world.units[0].needs.hunger = a.session.defs.needs.hunger.low * 0.8;
    run(&mut a, "h");
    let frame = text(&mut a, SMALL);
    for want in [
        "Spring Y1",  // the header's season, seasons on
        "SETTLEMENT", // the one heading
        "People 6",   // People, with Idle right-aligned
        "Idle 6",
        "Hungry 1",             // the need-flags row
        "Food 0 · 0 each",      // the Food row
        "Winter: mild",         // the winter row
        "Research: none set",   // the Research row, no target yet
        "Gate 0 of 9 signs",    // the Gate row
        "Needs 10 more people", // what it still needs
        "HARVEST",              // the active tool
        "HERE",
    ] {
        assert!(frame.contains(want), "{want:?} in\n{frame}");
    }
}

#[test]
fn a_tired_plants_hover_names_its_strain() {
    let mut a = app(7, Nation::Earth, SIZE);
    a.session.world.set_difficulty(Some("normal".to_owned()));
    let from = a.session.world.units[0].pos;
    let mut renewable: Vec<Pos> = a
        .session
        .world
        .map
        .iter()
        .filter(|(_, t)| {
            t.resource.and_then(|r| a.session.defs.harvest(r)).is_some_and(|h| h.regrow.is_some())
        })
        .map(|(p, _)| p)
        .collect();
    renewable.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
    let target = renewable
        .into_iter()
        .find(|&pos| a.session.apply(Command::DesignateHarvest { pos }).is_ok())
        .expect("a renewable resource the nation can work");
    for _ in 0..200 {
        a.session.tick_n(100);
        if a.session.world.is_resting(target) {
            break;
        }
    }
    assert!(a.session.world.is_resting(target), "the resource should have been worked");
    assert!(a.session.world.strain_at(target).unwrap() > 0.0, "and tired, on Normal");
    a.ui.cursor = target;
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("ripe in"), "{frame}");
    assert!(frame.contains("Tired (strain"), "{frame}");
}

#[test]
fn the_alert_log_carries_the_winter_notices() {
    let mut a = app(7, Nation::Earth, SIZE);
    a.session.world.set_difficulty(Some("normal".to_owned()));
    let per_season = season_ticks(&a);
    run(&mut a, &format!("<tick {}> l", per_season * 3 + 1));
    assert!(a.session.world.season(&a.session.defs).winter);
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Winter in 3 minutes"), "{frame}");
    assert!(frame.contains("Winter. Wild plants rest"), "{frame}");
    assert!(frame.contains("First frost"), "{frame}");
}

/// #643's frame check: with the endeavor Ready, the key script `g <enter>`, a cursor walk and
/// `<enter>` places the gate, and the next frame draws the Foundation's own glyph block on the
/// map (`data/tui.toml`'s `[gate_stage.foundation]`), which no frame before it has. (Before
/// #643 the map left the gate's site out altogether: `World::build_sites` skips it.)
#[test]
fn a_key_script_places_the_gate_and_the_frame_shows_its_site() {
    let mut a = app(7, Nation::Earth, SIZE);
    // Ready, as if every sign had been met: a save round trip with the state spliced in.
    let text_save = a.session.world.save(&a.session.defs).unwrap();
    let text_save = text_save.replacen("Dormant", "Ready(tick:0)", 1);
    let world = fn_sim::World::load(&text_save, &a.session.defs).unwrap();
    a.load(world);
    let before = text(&mut a, SIZE);
    assert!(!before.contains('▦'), "no gate site yet\n{before}");
    assert!(before.contains("Gate 0 of 9 signs"), "the side panel says so\n{before}");
    assert!(before.contains("Ready"), "the side panel says so\n{before}");

    run(&mut a, "g");
    let screen = text(&mut a, SIZE);
    assert!(screen.contains("Enter: choose a site for the gate"), "{screen}");

    // A site a few tiles off the cursor, where the ghost says the gate fits.
    let start = a.ui.cursor;
    let cursor = (1..12)
        .flat_map(|r| (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy))))
        .map(|(dx, dy)| Pos::new(start.x + dx, start.y + dy))
        .find(|&c| fn_ui::gate::placement(&a.session, c).is_some_and(|p| p.ok()))
        .expect("a free site near the cursor");
    let walk = |d: i32, pos: &str, neg: &str| {
        let key = if d >= 0 { pos } else { neg };
        if d == 0 { String::new() } else { format!("<{key}*{}>", d.abs()) }
    };
    let script = format!(
        "<enter> {} {} <enter>",
        walk(cursor.x - start.x, "right", "left"),
        walk(cursor.y - start.y, "down", "up")
    );
    run(&mut a, &script);
    assert!(
        matches!(a.session.world.endeavor(), fn_sim::world::EndeavorState::Building(_)),
        "{script}: {:?}",
        a.session.world.endeavor()
    );
    let after = text(&mut a, SIZE);
    // Sixteen tiles, less any a settler stands on.
    let block = after.chars().filter(|&c| c == '▦').count();
    assert!(block >= 8, "the Foundation's block on the map ({block} cells)\n{after}");
}

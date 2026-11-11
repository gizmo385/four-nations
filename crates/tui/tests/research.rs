//! The research screen (#100): `T` opens the tech tree over the map, the arrows and WASD move
//! between cards, and `Enter` issues `Command::SetResearch`. The one reference golden lives in
//! `reference_goldens.rs` now (#455).

use fn_sim::Nation;
use fn_sim::research::TechId;
use fn_sim::world::Command;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::state::Screen;
use fn_tui::{keys, new_session};
use fn_ui::Tool;
use fn_ui::research::{TechState, screen_view};

const SIZE: (u16, u16) = (120, 40);
const SMALL: (u16, u16) = (80, 24);
const WIDE: (u16, u16) = (220, 50);

fn app(seed: u64, nation: Nation, size: (u16, u16)) -> App {
    App::new(new_session(seed, nation), size)
}

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn palette(app: &App, ascii: bool) -> Palette {
    Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, ascii)
}

fn text(app: &mut App, size: (u16, u16)) -> String {
    let p = palette(app, false);
    to_text(&render_app(app, &p, size))
}

fn tech(app: &App, id: &str) -> TechId {
    app.session.defs.tech_id(id).unwrap_or_else(|| panic!("no tech {id}"))
}

fn queue(app: &App) -> Vec<TechId> {
    app.session.world.research().queue().to_vec()
}

/// Moves the cursor onto the card for `id` (the arrows from wherever it is) and returns.
fn go_to(app: &mut App, id: &str) {
    let want = tech(app, id);
    let view = screen_view(&app.session);
    let (c, r) = view
        .columns
        .iter()
        .enumerate()
        .find_map(|(c, col)| col.iter().position(|t| t.id == want).map(|r| (c, r)))
        .unwrap_or_else(|| panic!("{id} is not in this nation's tree"));
    let (c0, r0) = app.ui.tech;
    let step = |n: usize, back: &str, fwd: &str, from: usize| match n.cmp(&from) {
        std::cmp::Ordering::Equal => String::new(),
        std::cmp::Ordering::Greater => format!("<{fwd}*{}> ", n - from),
        std::cmp::Ordering::Less => format!("<{back}*{}> ", from - n),
    };
    // Up first so the row is in range of the column it leaves; then across, then down.
    let script = format!(
        "{}{}{}",
        step(0, "up", "down", r0),
        step(c, "left", "right", c0),
        step(r, "up", "down", 0)
    );
    run(app, &script);
    assert_eq!(app.ui.tech, (c, r), "{id}");
}

// ---------------------------------------------------------------------------------------------
// Opening, closing and the keys that stop
// ---------------------------------------------------------------------------------------------

#[test]
fn t_opens_the_screen_and_t_or_esc_close_it_over_the_side_panel() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "t");
    assert_eq!(a.ui.screen, Some(Screen::Research));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("RESEARCH"), "{frame}");
    assert!(frame.contains("TIER 1"), "{frame}");
    // A full screen covers the side panel (#652); the status line stays and says so.
    assert!(!frame.contains("EARTH KINGDOM"), "{frame}");
    assert!(!frame.contains("SETTLEMENT"), "{frame}");
    assert!(frame.lines().last().unwrap().contains("RESEARCH"), "the status line says so");

    run(&mut a, "t");
    assert_eq!(a.ui.screen, None);
    run(&mut a, "t <esc>");
    assert_eq!(a.ui.screen, None);
    run(&mut a, "T");
    assert_eq!(a.ui.screen, Some(Screen::Research));
}

#[test]
fn the_map_keys_wait_while_it_is_open() {
    let mut a = app(7, Nation::Earth, SIZE);
    let (cursor, view) = (a.ui.cursor, a.ui.view);
    run(&mut a, "t h x p b <right*3> <down*2> W <pgdn> <tab> f <enter>");
    assert_eq!(a.ui.cursor, cursor, "the map cursor did not move");
    assert_eq!(a.ui.view, view);
    assert_eq!(a.ui.tool, Tool::Select, "no tool was picked");
    assert_eq!(a.ui.selected, None, "Enter chose a tech, not a unit");
    assert_ne!(a.ui.tech, (0, 0), "the arrows moved the tech cursor");
    // Esc closed nothing else: the screen is still open and the map answers once it is shut.
    assert_eq!(a.ui.screen, Some(Screen::Research));
    run(&mut a, "<esc> <right>");
    assert_ne!(a.ui.cursor, cursor);
}

#[test]
fn the_clock_prompts_and_other_screens_still_work() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "t <space>");
    assert!(a.session.clock.paused);
    run(&mut a, "<f9>");
    assert!(a.ui.prompt.is_some(), "F9 asks first, over the screen");
    run(&mut a, "<esc>");
    assert!(a.ui.prompt.is_none() && a.ui.screen == Some(Screen::Research));
    run(&mut a, "l");
    assert_eq!(a.ui.screen, Some(Screen::Log), "L switches to the log");
    run(&mut a, "t");
    assert_eq!(a.ui.screen, Some(Screen::Research), "and T back");
    run(&mut a, "?");
    assert_eq!(a.ui.screen, Some(Screen::Help));
}

#[test]
fn the_key_help_lists_the_research_key() {
    let row = fn_tui::input::help_rows()
        .into_iter()
        .find(|r| r.what.starts_with("Research"))
        .expect("a Research row");
    assert_eq!(row.keys, "T");
}

// ---------------------------------------------------------------------------------------------
// What shows
// ---------------------------------------------------------------------------------------------

#[test]
fn only_the_shared_tier_and_the_players_own_branch_show() {
    for nation in Nation::ALL {
        let mut a = app(7, nation, WIDE);
        run(&mut a, "t");
        let frame = text(&mut a, WIDE);
        let defs = &a.session.defs;
        let own: Vec<_> = defs.techs_for(nation).map(|(_, d)| d.name.clone()).collect();
        for def in &defs.techs {
            // Names are cut to the card; their first twelve letters are enough to tell them apart.
            let head: String = def.name.chars().take(12).collect();
            let shown = frame.contains(&head);
            let visible = own.contains(&def.name);
            assert_eq!(shown, visible, "{nation:?}: {} (visible: {visible})\n{frame}", def.name);
        }
    }
}

#[test]
fn every_card_names_its_state_cost_and_unlocks_or_what_it_still_needs() {
    let mut a = app(7, Nation::Earth, WIDE);
    run(&mut a, "t");
    let frame = text(&mut a, WIDE);
    for word in ["Known", "Available", "Locked"] {
        assert!(frame.contains(word), "{word}\n{frame}");
    }
    // Earth Shaping: 60 points, available from the start, a modifier of its own.
    assert!(frame.contains("60 pts"), "{frame}");
    assert!(frame.contains("Earthworks x1.2"), "{frame}");
    // Masonry is Locked (Carpentry still unknown): the card names what it still needs (#338),
    // not the construction bonus it would eventually grant.
    assert!(frame.contains("Needs Carpentry"), "{frame}");
    // Stoneworking is the Earth Kingdom's starting tech.
    let known = a.session.world.research().is_known(tech(&a, "stoneworking"));
    assert!(known);
    let row = frame.lines().find(|l| l.contains("Stoneworking")).unwrap();
    assert!(row.contains('✓'), "{row}");
}

/// A substring's position in `char`s, not bytes: the frame carries multi-byte marks (bullets,
/// checks, arrows) ahead of a card's name, so a plain `str::find` (byte offset) drifts away from
/// the column it actually landed in as soon as the grid's own column width (it follows the
/// terminal's, #423) shifts a column just past a multiple of the divisor below.
fn char_index_of(haystack: &str, needle: &str) -> Option<usize> {
    let byte_idx = haystack.find(needle)?;
    Some(haystack[..byte_idx].chars().count())
}

#[test]
fn columns_are_by_depth_and_start_with_the_tier_numbers() {
    let mut a = app(7, Nation::Earth, WIDE);
    run(&mut a, "t");
    let frame = text(&mut a, WIDE);
    let heads = frame.lines().find(|l| l.contains("TIER 1")).unwrap();
    let at =
        |n: usize| char_index_of(heads, &format!("TIER {n}")).unwrap_or_else(|| panic!("TIER {n}"));
    for n in 1..5 {
        assert!(at(n) < at(n + 1), "{heads}");
    }
    // Earth Rail, five prereqs deep, is in the fifth column, under TIER 5. Omashu Chutes, after
    // it, makes a sixth.
    let deepest = screen_view(&a.session).columns.len();
    assert_eq!(deepest, 6);
    let row = frame.lines().find(|l| l.contains("Earth Rail")).unwrap();
    assert_eq!(char_index_of(row, "Earth Rail").map(|x| x / 19), Some(at(5) / 19));
}

#[test]
fn a_narrow_terminal_scrolls_across_the_tree_with_the_cursor() {
    let mut a = app(7, Nation::Earth, SMALL);
    run(&mut a, "t");
    let first = text(&mut a, SMALL);
    assert!(first.contains("TIER 1") && !first.contains("TIER 5"), "{first}");
    assert!(!first.contains('◂'), "nothing hidden on the left yet");
    assert!(first.contains('▸'), "more columns to the right");
    run(&mut a, "<right*4>");
    let last = text(&mut a, SMALL);
    assert!(last.contains("TIER 5") && !last.contains("TIER 1"), "{last}");
    assert!(last.contains('◂'), "{last}");
    // The card under the cursor is always on screen, with its details underneath.
    let (c, r) = a.ui.tech;
    let name = &screen_view(&a.session).columns[c][r].name;
    assert!(last.contains(name.as_str()), "{name}\n{last}");
}

#[test]
fn the_cursor_stays_in_sight_on_every_card_and_every_size() {
    for nation in Nation::ALL {
        for size in [SMALL, SIZE, (100, 30), WIDE] {
            let mut a = app(7, nation, size);
            run(&mut a, "t");
            let view = screen_view(&a.session);
            for (c, col) in view.columns.iter().enumerate() {
                for (r, card) in col.iter().enumerate() {
                    a.ui.tech = (c, r);
                    let frame = text(&mut a, size);
                    // The details line shows the full name even when the card cuts it.
                    let detail = frame.lines().rev().nth(4);
                    assert!(
                        detail.is_some_and(|l| l.contains(card.name.as_str())),
                        "{nation:?} {size:?} {}\n{frame}",
                        card.name
                    );
                    assert_eq!(frame.lines().count(), usize::from(size.1));
                    assert!(frame.lines().all(|l| l.chars().count() <= usize::from(size.0)));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Moving and choosing
// ---------------------------------------------------------------------------------------------

#[test]
fn the_arrows_and_wasd_move_the_same_way_and_stop_at_the_edges() {
    for (letter, arrow) in [("w", "<up>"), ("a", "<left>"), ("s", "<down>"), ("d", "<right>")] {
        let mut by_letter = app(7, Nation::Earth, SIZE);
        let mut by_arrow = app(7, Nation::Earth, SIZE);
        run(&mut by_letter, &format!("t <right> <down> {letter}"));
        run(&mut by_arrow, &format!("t <right> <down> {arrow}"));
        assert_eq!(by_letter.ui.tech, by_arrow.ui.tech, "{letter}");
        assert_eq!(text(&mut by_letter, SIZE), text(&mut by_arrow, SIZE), "{letter}");
    }
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "t <left*9> <up*99>");
    assert_eq!(a.ui.tech, (0, 0));
    run(&mut a, "<right*99> <down*99>");
    let view = screen_view(&a.session);
    let last = view.columns.len() - 1;
    assert_eq!(a.ui.tech, (last, view.columns[last].len() - 1));
}

#[test]
fn shift_does_not_make_a_step_longer() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "t <left*9> <up*99>");
    run(&mut a, "S <s-down> D");
    assert_eq!(a.ui.tech, (1, 2));
}

#[test]
fn page_keys_move_a_page_of_cards() {
    let mut a = app(7, Nation::Air, SMALL);
    run(&mut a, "t <left*9> <up*99>");
    run(&mut a, "<pgdn>");
    assert_eq!(a.ui.tech, (0, 4), "four cards fit at 80x24");
    run(&mut a, "<pgup> <pgup>");
    assert_eq!(a.ui.tech, (0, 0));
}

#[test]
fn enter_sets_the_target_and_issues_set_research() {
    let mut a = app(7, Nation::Earth, SIZE);
    assert!(queue(&a).is_empty());
    run(&mut a, "t");
    go_to(&mut a, "shelter");
    run(&mut a, "<enter>");
    assert_eq!(queue(&a), [tech(&a, "shelter")]);
    // The same command, applied directly, gives the same world.
    let mut direct = new_session(7, Nation::Earth);
    direct.apply(Command::SetResearch { tech: tech(&a, "shelter") }).unwrap();
    assert_eq!(a.session.world.research().queue(), direct.world.research().queue());
    // The side panel's Research section follows.
    assert!(text(&mut a, SIZE).contains("Shelter"));
    // Choosing another tech replaces the target.
    go_to(&mut a, "fishing");
    run(&mut a, "<enter>");
    assert_eq!(queue(&a), [tech(&a, "fishing")]);
}

#[test]
fn enter_on_a_locked_tech_queues_its_prereqs() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "t");
    go_to(&mut a, "masonry");
    let masonry = tech(&a, "masonry");
    let state = |a: &App, id: TechId| {
        let card =
            screen_view(&a.session).columns.into_iter().flatten().find(|c| c.id == id).unwrap();
        card.state
    };
    assert_eq!(state(&a, masonry), TechState::Locked);

    run(&mut a, "<enter>");
    let queued = queue(&a);
    assert!(queued.len() > 1, "the chain, not just the tech: {queued:?}");
    assert_eq!(queued.last(), Some(&masonry), "the target comes last");
    let mut direct = new_session(7, Nation::Earth);
    direct.apply(Command::SetResearch { tech: masonry }).unwrap();
    assert_eq!(queued, direct.world.research().queue(), "what the sim queues for it");
    for &id in &queued {
        assert_eq!(state(&a, id), TechState::Queued);
    }

    // A queued prereq is drawn the same as the target, bar its word: "Queued" for every tech in
    // the chain ahead of it, "Target" for the one chosen (#659).
    let frame = text(&mut a, WIDE);
    // (The details line under the grid says it once more, for the card under the cursor.)
    assert_eq!(frame.matches("Queued").count(), queued.len() - 1, "{frame}");
    assert_eq!(frame.matches("Target").count(), 2, "{frame}");
    // The map region only: the side panel's own Research row names the current target too (a
    // queued prereq, here), which can otherwise collide with the grid's own card for it.
    let map_width = usize::from(WIDE.0 - fn_tui::layout::panel_width(WIDE.0));
    let mark = |name: &str| {
        frame
            .lines()
            .map(|l| l.chars().take(map_width).collect::<String>())
            .find(|l| l.contains(name))
            .unwrap()
    };
    let chain: Vec<char> = queued
        .iter()
        .map(|&id| {
            let name = a.session.defs.tech(id).name.clone();
            let row = mark(&name);
            let at = row.find(name.as_str()).unwrap();
            row[..at].trim_end().chars().last().unwrap()
        })
        .collect();
    assert!(chain.iter().all(|c| *c == chain[0]), "one mark for the whole chain: {chain:?}");
}

#[test]
fn enter_on_a_known_tech_changes_nothing() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "t");
    go_to(&mut a, "stoneworking");
    run(&mut a, "<enter>");
    assert!(queue(&a).is_empty());
    assert_eq!(a.ui.screen, Some(Screen::Research));
}

#[test]
fn the_screen_opens_on_the_target() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "t");
    go_to(&mut a, "masonry");
    let at = a.ui.tech;
    run(&mut a, "<enter> <esc> <right*3>");
    assert_eq!(a.ui.screen, None);
    run(&mut a, "t");
    assert_eq!(a.ui.tech, at, "back on Masonry");
}

#[test]
fn a_finished_tech_becomes_known_on_the_screen() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "t");
    go_to(&mut a, "shelter");
    run(&mut a, "<enter> <tick 10000>");
    let id = tech(&a, "shelter");
    assert!(a.session.world.research().is_known(id), "ten thousand ticks pay for 25 points");
    let frame = text(&mut a, SIZE);
    let row = frame.lines().find(|l| l.contains("Shelter")).unwrap();
    assert!(row.contains('✓'), "{row}");
}

// ---------------------------------------------------------------------------------------------
// ASCII
// ---------------------------------------------------------------------------------------------

/// Another nation's tree, in ASCII: `a_narrow_terminal_scrolls_across_the_tree_with_the_cursor`
/// already covers the smallest terminal's own scrolling.
#[test]
fn another_nations_tree_renders_in_ascii() {
    let mut a = app(3, Nation::Fire, SIZE);
    run(&mut a, "t");
    go_to(&mut a, "smelting");
    run(&mut a, "<enter>");
    let p = palette(&a, true);
    let frame = to_text(&render_app(&mut a, &p, SIZE));
    assert!(frame.is_ascii(), "the ASCII frame must be ASCII:\n{frame}");
    assert!(frame.contains("Smelting"), "{frame}");
}

/// The chosen card wears the selection (#652): a quiet background over its three rows in every
/// colour depth that has one, bold text, and never reverse video, so its state colours stay.
#[test]
fn the_chosen_card_wears_a_quiet_background_never_reverse_video() {
    use ratatui::style::Modifier;
    for depth in
        [ColorDepth::TrueColor, ColorDepth::Indexed256, ColorDepth::Ansi16, ColorDepth::Never]
    {
        let mut a = app(7, Nation::Earth, SIZE);
        run(&mut a, "t <down>");
        let p = Palette::new(&a.session.defs, Nation::Earth, depth, false);
        let buf = render_app(&mut a, &p, SIZE);
        // The page: every row but the status line, which has its own badges.
        let page = 0..SIZE.1 - 1;
        let reversed = page
            .clone()
            .filter(|&y| (0..SIZE.0).any(|x| buf[(x, y)].modifier.contains(Modifier::REVERSED)))
            .count();
        assert_eq!(reversed, 0, "{depth:?}: no reverse video on the page");
        let Some(select) = fn_tui::page::select_bg(&p) else {
            assert_eq!(depth, ColorDepth::Never);
            continue;
        };
        let selected: Vec<u16> =
            page.filter(|&y| (0..SIZE.0).any(|x| buf[(x, y)].bg == select)).collect();
        assert_eq!(selected.len(), 3, "{depth:?}: the card's three rows {selected:?}");
        assert_eq!(selected[2] - selected[0], 2, "{depth:?}: three rows together");
        let name_row = selected[0];
        let mut bold = (0..SIZE.0).filter(|&x| buf[(x, name_row)].bg == select);
        assert!(
            bold.all(|x| buf[(x, name_row)].modifier.contains(Modifier::BOLD)),
            "{depth:?}: brighter text"
        );
        assert!((0..SIZE.0).any(|x| buf[(x, name_row)].symbol() == "▸"), "{depth:?}: the pointer");
    }
}

// ---------------------------------------------------------------------------------------------
// Whole cards (#659)
// ---------------------------------------------------------------------------------------------

/// The grid's rows of a frame: under the top edge, down to the rule over the detail strip.
fn grid_rows(frame: &str) -> Vec<&str> {
    let lines: Vec<&str> = frame.lines().collect();
    let rule = lines.iter().position(|l| l.starts_with('├')).expect("a rule over the details");
    lines[1..rule].to_vec()
}

/// No card is ever cut: with the cursor on every card of every nation's tree, at the two sizes
/// the owner reviews at, no row of the grid ends in an ellipsis, and the cursor's card shows
/// every word of its name and of what it gives.
#[test]
fn no_card_is_cut_for_any_nation_at_80x24_and_120x40() {
    for nation in Nation::ALL {
        for size in [SMALL, SIZE] {
            let mut a = app(7, nation, size);
            run(&mut a, "t");
            let view = screen_view(&a.session);
            for (c, col) in view.columns.iter().enumerate() {
                for (r, card) in col.iter().enumerate() {
                    a.ui.tech = (c, r);
                    let frame = text(&mut a, size);
                    let rows = grid_rows(&frame);
                    for row in &rows {
                        assert!(
                            !row.contains('…'),
                            "{nation:?} {size:?} cursor on {}: {row}\n{frame}",
                            card.name
                        );
                    }
                    let grid = rows.join("\n");
                    let gives = if card.state == TechState::Locked {
                        card.needs.clone()
                    } else {
                        card.unlock.clone()
                    };
                    for word in card.name.split_whitespace().chain(gives.split_whitespace()) {
                        assert!(
                            grid.contains(word),
                            "{nation:?} {size:?} {}: {word}\n{frame}",
                            card.name
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn four_tiers_show_at_120_columns_and_three_at_80_and_the_bottom_edge_says_which() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "t");
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("TIER 4") && !frame.contains("TIER 5"), "{frame}");
    assert!(frame.lines().rev().nth(1).unwrap().contains(" tiers 1-4 of 6 ─┛"), "{frame}");

    let mut a = app(7, Nation::Earth, SMALL);
    run(&mut a, "t");
    let frame = text(&mut a, SMALL);
    assert!(frame.contains("TIER 3") && !frame.contains("TIER 4"), "{frame}");
    assert!(frame.lines().rev().nth(1).unwrap().contains(" tiers 1-3 of 6 ─┛"), "{frame}");
}

#[test]
fn a_locked_card_names_what_it_needs_and_a_card_that_only_opens_others_says_what_it_leads_to() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "t");
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Needs Fire Making"), "{frame}");
    assert!(frame.contains("Leads to Carpentry"), "{frame}");
    assert!(!frame.contains("Prerequisite for"), "{frame}");
    assert!(frame.contains("Available · 25 pts"), "state and cost share a line:\n{frame}");
}

#[test]
fn the_target_is_marked_on_its_card_and_named_in_the_detail_strip() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "t");
    go_to(&mut a, "farming");
    run(&mut a, "<enter>");
    // The cursor on the target: its state reads Target, in its card and in the strip.
    let frame = text(&mut a, SIZE);
    assert!(grid_rows(&frame).iter().any(|r| r.contains("Target · 60 pts")), "{frame}");
    assert!(frame.lines().rev().nth(4).unwrap().contains("Target · 60 pts"), "{frame}");
    // The cursor elsewhere: the strip still names the target.
    go_to(&mut a, "shelter");
    let frame = text(&mut a, SIZE);
    assert!(grid_rows(&frame).iter().any(|r| r.contains("Target · 60 pts")), "{frame}");
    assert!(frame.lines().rev().nth(4).unwrap().contains("Target: Farming"), "{frame}");
}

// ---------------------------------------------------------------------------------------------
// A whole detail strip (#768)
// ---------------------------------------------------------------------------------------------

/// The detail strip's rows of a frame: under the rule, down to the bottom edge.
fn strip_rows(frame: &str) -> Vec<&str> {
    let lines: Vec<&str> = frame.lines().collect();
    let rule = lines.iter().position(|l| l.starts_with('├')).expect("a rule over the details");
    let bottom = lines.iter().rposition(|l| l.starts_with('┗')).expect("a bottom edge");
    lines[rule + 1..bottom].to_vec()
}

/// The strip is never cut: with the cursor on every card of every nation's tree, at 80x24 and
/// 120x40, no row of it ends in an ellipsis and every word of what the tech unlocks and needs is
/// there, wrapped onto a further row when it doesn't fit one.
#[test]
fn the_detail_strip_wraps_what_a_tech_unlocks_and_needs_for_any_nation_at_80x24_and_120x40() {
    let mut wrapped = 0;
    for (nation, size, deep) in
        Nation::ALL.into_iter().flat_map(|n| [SMALL, SIZE].map(|s| (n, s))).flat_map(|(n, s)| {
            // Each card of the last column in turn: which one is deepest depends on the nation's
            // tree, and a shared tech may sit beside the nation's own.
            let last = screen_view(&app(7, n, s).session).columns.last().expect("a tree").len();
            std::iter::once((n, s, None)).chain((0..last).map(move |r| (n, s, Some(r))))
        })
    {
        {
            let mut a = app(7, nation, size);
            if let Some(row) = deep {
                // Targeting a deep tech queues every prereq: a Queued card shows what it
                // unlocks and what it still needs, side by side.
                let view = screen_view(&a.session);
                let deepest = view.columns.last().expect("a tree")[row].id;
                a.session.apply(Command::SetResearch { tech: deepest }).unwrap();
            }
            run(&mut a, "t");
            let view = screen_view(&a.session);
            for (c, col) in view.columns.iter().enumerate() {
                for (r, card) in col.iter().enumerate() {
                    a.ui.tech = (c, r);
                    let frame = text(&mut a, size);
                    let rows = strip_rows(&frame);
                    for row in &rows {
                        assert!(
                            !row.contains('…'),
                            "{nation:?} {size:?} cursor on {}: {row}\n{frame}",
                            card.name
                        );
                    }
                    if rows.len() > 3 {
                        wrapped += 1;
                    }
                    let strip = rows.join("\n");
                    for word in card.unlock.split_whitespace().chain(card.needs.split_whitespace())
                    {
                        assert!(
                            strip.contains(word),
                            "{nation:?} {size:?} {}: {word}\n{frame}",
                            card.name
                        );
                    }
                }
            }
        }
    }
    assert!(wrapped > 0, "some tech at 80x24 has a line too long for one row");
}

/// The strip words things as the cards do ("Needs X" and "Leads to X", no colon), and a short
/// line stays on one row beside the other.
#[test]
fn the_strip_and_the_cards_use_the_same_words() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "t");
    let view = screen_view(&a.session);
    for (c, col) in view.columns.iter().enumerate() {
        for (r, card) in col.iter().enumerate() {
            a.ui.tech = (c, r);
            let frame = text(&mut a, SIZE);
            let strip = strip_rows(&frame).join("\n");
            assert!(!strip.contains("Needs:") && !strip.contains("Unlocks:"), "{strip}");
            if card.state == TechState::Locked {
                assert!(
                    strip.contains(&format!("Needs {}", card.needs.split(", ").next().unwrap()))
                );
            }
        }
    }
}

//! The side panel's Alerts section (#656): alerts of one cause are one entry, each is wrapped in
//! full, the heading counts them, a research target that is not set is one, and when they would
//! take more than half the rows below the glance the rest fold into "and N more, press L".
//! `contains`-style checks; the whole-frame goldens are in `reference_goldens.rs`.

use fn_sim::Nation;
use fn_sim::world::Command;
use fn_tui::app::App;
use fn_tui::cli;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{self, render_app, to_text};
use fn_tui::layout;
use fn_tui::shell::Shell;
use fn_tui::{keys, new_session};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// The sizes the acceptance criteria name.
const SIZES: [(u16, u16); 3] = [(80, 24), (120, 40), (200, 50)];

/// The side panel's rows in `buf`, without its frame cells.
fn panel_rows(buf: &Buffer, panel: Rect) -> Vec<String> {
    (panel.top()..panel.bottom())
        .map(|y| (panel.left() + 1..panel.right() - 1).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

/// The rows of the Alerts section: from its heading to the next one, heading included.
fn alerts_section(rows: &[String]) -> Vec<String> {
    let start = rows.iter().position(|r| r.contains("ALERTS")).expect("an ALERTS heading");
    let end = rows[start + 1..]
        .iter()
        .position(|r| r.trim_start().starts_with('─'))
        .map_or(rows.len(), |n| start + 1 + n);
    rows[start..end].to_vec()
}

/// Earth, year two: a stockpile full of wood, stone, fish and fruit and no research target, the
/// owner's own example. Built once; resized for each case.
fn year_two() -> Shell {
    let args =
        cli::parse(["--scenario", "earth-year-two", "--frame"].into_iter().map(str::to_owned))
            .unwrap();
    let mut shell = Shell::for_args(&args, SIZES[0], None, 0).unwrap();
    // The stockpile fills up over the next few minutes (the owner's view was 6000 ticks in).
    shell.play(&[keys::Step::Tick(6000)]);
    shell
}

#[test]
fn every_alert_is_wrapped_in_full_at_every_size_and_the_rest_fold() {
    let mut shell = year_two();
    for size in SIZES {
        shell.resize(size.0, size.1);
        let palette = Palette::new(shell.defs(), shell.nation(), ColorDepth::TrueColor, false);
        let buf = frame::render_shell(&mut shell, &palette, size);
        let regions = layout::regions(buf.area).expect("a usable size");
        let rows = panel_rows(&buf, regions.panel);
        let section = alerts_section(&rows);
        let body = section[1..].join("\n");
        let context = format!("at {size:?}:\n{}", to_text(&buf));

        assert!(section[0].contains("2 · [L]"), "the heading counts the alerts {context}");
        assert!(!body.contains('…'), "no alert is cut {context}");
        // Whitespace-joined, so a wrap point doesn't matter.
        let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(flat.starts_with("Stockpiles are full. "), "stores come first {context}");
        assert!(flat.contains(" stone, "), "{context}");
        assert!(flat.contains(" have nowhere to go."), "the first alert is whole {context}");
        assert_eq!(flat.matches("Stockpiles are full").count(), 1, "one entry per cause {context}");
        // The second either shows whole or is folded into the last line, never cut.
        let research = flat.contains("No research target is set.") && flat.contains(" waiting.");
        let folded = flat.ends_with("and 1 more, press L");
        assert!(research != folded, "whole or folded, not both or neither {context}");
        // Blank rows separate alerts, so the next alert starts a paragraph.
        if research {
            let at = body.lines().position(|r| r.contains("No research")).unwrap();
            assert!(body.lines().nth(at - 1).unwrap().trim().is_empty(), "{context}");
        }
        // The glance still has its own row for it.
        assert!(rows.iter().any(|r| r.contains("Research: none set")), "{context}");
    }
}

#[test]
fn alerts_take_at_most_half_the_rows_below_the_glance() {
    let mut shell = year_two();
    shell.resize(80, 24);
    let palette = Palette::new(shell.defs(), shell.nation(), ColorDepth::TrueColor, false);
    let buf = frame::render_shell(&mut shell, &palette, (80, 24));
    let regions = layout::regions(buf.area).unwrap();
    let rows = panel_rows(&buf, regions.panel);
    let section = alerts_section(&rows);
    let alerts = rows.iter().position(|r| r.contains("ALERTS")).unwrap();
    // Every row under the Settlement glance: from the ALERTS heading to the last row before the
    // panel's bottom edge.
    let below = rows.len() - 1 - alerts;
    assert!(
        section.len() - 1 <= below / 2,
        "{} alert rows of {below} below the glance:\n{}",
        section.len() - 1,
        rows.join("\n")
    );
    assert!(section.last().unwrap().trim().ends_with("press L"), "{section:#?}");
}

/// A game with a stockpile, a fresh settlement and a pool of research points: the standing
/// condition the panel shows, and clears when a target is set.
fn banked_game() -> App {
    let session = new_session(7, Nation::Earth);
    let mut app = App::new(session, (120, 40));
    for _ in 0..40 {
        app.play(&[keys::Step::Tick(100)]);
        if app.session.world.research().pool_millis() >= fn_sim::research::MILLIS_PER_POINT {
            break;
        }
    }
    app
}

fn frame_text(app: &mut App) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    to_text(&render_app(app, &p, (120, 40)))
}

#[test]
fn banked_research_with_no_target_is_an_alert_until_a_target_is_set() {
    let mut app = banked_game();
    let frame = frame_text(&mut app);
    assert!(frame.contains("No research target is set."), "{frame}");
    assert!(frame.contains("Research: none set"), "the glance's own row stays:\n{frame}");
    assert!(frame.contains("1 · [L]"), "{frame}");
    // And the log has the line it was raised with, under its Alerts filter.
    let raised = app.ui.alerts.history().iter().find(|e| e.text.starts_with("No research target"));
    assert!(raised.is_some_and(|e| e.category == fn_ui::alerts::Category::Alert));

    let shelter = app.session.defs.tech_id("shelter").unwrap();
    app.session.apply(Command::SetResearch { tech: shelter }).unwrap();
    app.play(&[keys::Step::Tick(1)]);
    let frame = frame_text(&mut app);
    assert!(!frame.contains("No research target is set."), "a target is set:\n{frame}");
    assert!(!frame.contains("Research: none set"), "{frame}");
}

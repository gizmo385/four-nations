//! The message log (#661): newest first, a run of the same event folded into one line, one kind
//! of line at a time, a cursor, and `Enter` goes to the place. `contains`-style assertions; the
//! whole frame is golden in `reference_goldens.rs`.

use fn_sim::world::Command;
use fn_sim::{Nation, Pos, Terrain};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::state::Screen;
use fn_tui::widgets::Tone;
use fn_tui::{keys, msglog};
use fn_ui::Session;
use fn_ui::alerts::Category;
use fn_ui::tools::ability_for_nation;

const SIZE: (u16, u16) = (120, 40);

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn text(app: &mut App) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    to_text(&render_app(app, &p, SIZE))
}

/// A Fire game where the settlers have bent a good many times and one bend has failed: a log
/// with a run of finished bends, an alert, and (pushed in by the test) a line of work.
fn bending_game() -> App {
    let mut s = Session::new(3, Nation::Fire, 48, 32);
    let ability = ability_for_nation(&s.defs, Nation::Fire).unwrap();
    let from = s.world.units[0].pos;
    for dy in -5..=5 {
        for dx in -5..=5 {
            let pos = Pos::new(from.x + dx, from.y + dy);
            let _ = s.apply(Command::Bend { ability, pos });
        }
    }
    s.world.map[from].terrain = Terrain::Mountain;
    s.world.map_changed();
    let mut app = App::new(s, SIZE);
    for _ in 0..200 {
        app.play(&[keys::Step::Tick(100)]);
        if app.ui.alerts.history().len() >= 12 && !app.ui.alerts.toasts().is_empty() {
            break;
        }
    }
    assert!(app.ui.alerts.history().len() >= 12, "bending should have been logged");
    let tick = app.session.world.tick;
    app.ui.alerts.push_agent(tick, "Agent: scouting the east".to_owned());
    app
}

/// Folds runs of whitespace and box-drawing to one space.
fn fold(s: &str) -> String {
    s.chars()
        .map(|c| if "│┃┏┓┗┛┣┫─◆▶".contains(c) { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn the_log_opens_with_the_newest_line_at_the_top_and_the_cursor_on_it() {
    let mut a = bending_game();
    run(&mut a, "l");
    assert_eq!(a.ui.screen, Some(Screen::Log));
    let all = msglog::lines(&a.ui);
    // The agent line was pushed last.
    assert_eq!(all[0].text, "Agent: scouting the east");
    let frame = text(&mut a);
    let rows: Vec<&str> = frame.lines().collect();
    let at = |needle: &str| rows.iter().position(|r| r.contains(needle)).unwrap();
    assert!(at("Agent: scouting") < at(&all[1].text[..20]), "newest above older:\n{frame}");
    // The cursor row wears the pointer.
    assert!(rows[at("Agent: scouting")].contains("▸"), "{frame}");
}

/// Keys `1` to `5` show one kind; `0` or the same key again shows all; the filter row says
/// which is on. The log is mixed: an alert (the failed bend), bending, and work.
#[test]
fn a_filter_shows_one_kind_and_the_same_key_or_zero_shows_all() {
    let mut a = bending_game();
    run(&mut a, "l");
    let all = msglog::lines(&a.ui);
    let kinds =
        |a: &App| -> Vec<Category> { msglog::lines(&a.ui).iter().map(|l| l.category).collect() };
    for c in [Category::Alert, Category::Bending, Category::Work] {
        assert!(kinds(&a).contains(&c), "the log is mixed: {c:?} is missing");
    }

    run(&mut a, "1");
    assert_eq!(a.ui.log_filter, Some(Category::Alert));
    assert!(kinds(&a).iter().all(|&c| c == Category::Alert) && !kinds(&a).is_empty());
    let frame = text(&mut a);
    assert!(frame.contains("[1 Alerts]"), "the filter row shows which is on:\n{frame}");
    assert!(frame.contains("failed"), "{frame}");
    assert!(!frame.contains("Agent: scouting"), "{frame}");
    assert!(!frame.contains("used "), "no finished bend under Alerts:\n{frame}");

    run(&mut a, "1");
    assert_eq!(a.ui.log_filter, None, "the same key again shows all");
    assert_eq!(msglog::lines(&a.ui).len(), all.len());
    assert!(text(&mut a).contains("[All]"));

    run(&mut a, "4");
    assert_eq!(a.ui.log_filter, Some(Category::Bending));
    assert!(kinds(&a).iter().all(|&c| c == Category::Bending));
    let frame = text(&mut a);
    assert!(frame.contains("[4 Bending]") && frame.contains(" used "), "{frame}");
    assert!(!frame.contains("failed"), "{frame}");

    run(&mut a, "5");
    assert_eq!(kinds(&a), [Category::Work]);
    // A kind with nothing says so.
    run(&mut a, "2");
    let frame = text(&mut a);
    assert!(frame.contains("Nothing under People yet"), "{frame}");

    run(&mut a, "0");
    assert_eq!(a.ui.log_filter, None);
    assert_eq!(msglog::lines(&a.ui).len(), all.len());
}

/// A run of bends close together is one line saying how many and by how many; `Right` opens it
/// into its lines and `Left` folds it again.
#[test]
fn a_run_of_bends_is_one_line_that_right_opens_and_left_folds() {
    let mut a = bending_game();
    let bends = a.ui.alerts.history().iter().filter(|e| e.category == Category::Bending).count();
    assert!(bends >= 5, "{bends} bends");
    run(&mut a, "l 4");
    let folded = msglog::lines(&a.ui);
    assert!(folded.len() < bends, "{bends} bends folded into {} lines", folded.len());
    let fold_at = folded.iter().position(|l| l.is_fold()).expect("a fold");
    let line = folded[fold_at].clone();
    assert!(line.text.contains(&format!("used {} times by", line.count)), "{}", line.text);
    assert!(line.text.contains(", near ("), "{}", line.text);
    assert!(text(&mut a).contains(&fold(&line.text)));

    if fold_at > 0 {
        run(&mut a, &format!("<down*{fold_at}>"));
    }
    run(&mut a, "<right>");
    let open = msglog::lines(&a.ui);
    assert_eq!(open.len(), folded.len() + line.count, "the fold opened into its lines");
    assert!(open[fold_at].open);
    assert!(text(&mut a).contains("- "), "an open fold wears a minus");

    // Left on one of its lines goes back to the fold and folds it.
    run(&mut a, "<down> <left>");
    assert_eq!(msglog::lines(&a.ui).len(), folded.len());
    assert_eq!(a.ui.log_cursor, Some(line.key), "the cursor is on the fold");
    // Left on a closed fold does nothing.
    run(&mut a, "<left>");
    assert_eq!(msglog::lines(&a.ui).len(), folded.len());
}

/// Folding and filtering are the view's: the history, and what an agent reads of it, is one line
/// per event whatever the log shows.
#[test]
fn folding_and_filtering_never_change_the_history_an_agent_reads() {
    let mut a = bending_game();
    let before: Vec<String> = a.ui.alerts.history().iter().map(|e| e.text.clone()).collect();
    run(&mut a, "l 4 <down*2> <right> 1 0 <pgdn> <esc>");
    let after: Vec<String> = a.ui.alerts.history().iter().map(|e| e.text.clone()).collect();
    assert_eq!(before, after);
    a.ui.log_open.clear();
    assert!(
        before.iter().filter(|t| t.contains(" used ")).count()
            > msglog::lines(&a.ui).iter().filter(|l| l.text.contains(" used ")).count(),
        "the view folds what the history does not"
    );
}

/// `Enter` on a line with a place closes the log with the map cursor there.
#[test]
fn enter_goes_to_the_place_and_closes_the_log() {
    let mut a = bending_game();
    run(&mut a, "l 4");
    let place = msglog::lines(&a.ui)[0].place.expect("a bend line has a place");
    a.ui.cursor = Pos::new(0, 0);
    run(&mut a, "<enter>");
    assert_eq!(a.ui.screen, None);
    assert_eq!(a.ui.cursor, place);
    assert!(a.ui.cursor_cell().is_some(), "the view shows it");

    // A line with no place and no settler stays open and says so.
    run(&mut a, "l");
    assert_eq!(msglog::lines(&a.ui)[0].place, None);
    run(&mut a, "<enter>");
    assert_eq!(a.ui.screen, Some(Screen::Log));
    assert!(a.ui.status.contains("no place"), "{}", a.ui.status);
}

/// Alert lines wear the warning colour; a finished bend does not.
#[test]
fn alert_lines_are_drawn_in_the_warning_colour() {
    let mut a = bending_game();
    run(&mut a, "l");
    let p = Palette::new(&a.session.defs, a.session.world.nation, ColorDepth::TrueColor, false);
    let buf = render_app(&mut a, &p, SIZE);
    let frame = to_text(&buf);
    let find = |needle: &str| {
        let row = frame.lines().position(|l| l.contains(needle)).unwrap();
        let col = frame.lines().nth(row).unwrap().find(needle).unwrap();
        let col = frame.lines().nth(row).unwrap()[..col].chars().count();
        buf[(col as u16, row as u16)].fg
    };
    assert_eq!(find("failed"), Tone::Warn.style(&p).fg.unwrap());
    assert_ne!(find(" used "), Tone::Warn.style(&p).fg.unwrap());
}

/// The panel's open alerts are lines the log's Alerts filter shows.
#[test]
fn the_panels_open_alerts_are_in_the_logs_alerts_filter() {
    let mut a = bending_game();
    let open = a.ui.alerts.open_alerts();
    assert!(!open.is_empty(), "the failed bend is an open alert");
    let panel = text(&mut a);
    run(&mut a, "l 1");
    let log = msglog::lines(&a.ui);
    for alert in &open {
        let subject = alert.text.split(" ×").next().unwrap();
        assert!(
            log.iter().any(|l| l.text.contains(subject)),
            "{subject:?} is an open alert but not in the Alerts filter: {log:#?}"
        );
        assert!(panel.contains(&alert.text[..alert.text.len().min(26)]), "{panel}");
    }
    // And what the filter shows is only alerts.
    assert!(log.iter().all(|l| l.category == Category::Alert));
}

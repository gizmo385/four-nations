//! One meter and one clock on every screen (#654). Every bar in the game is drawn from the same
//! two glyphs (`█` and `░`, `#` and `.` with `--ascii`); a low need is the meter itself in the
//! warning colour with a `!` directly against it. The clock is the season, the year and the time
//! left in it ("Spring Y2 · 15:00 left"), on the panel, the Nation header, the game menu, the
//! saves list and the message log alike, never minutes since the start.

use fn_sim::Nation;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::{keys, new_session};
use ratatui::buffer::Buffer;

const SIZE: (u16, u16) = (120, 40);
const SMALL: (u16, u16) = (80, 24);

fn app(seasons: bool) -> App {
    let mut session = new_session(7, Nation::Earth);
    if seasons {
        session.world.set_difficulty(Some("normal".to_owned()));
    }
    App::new(session, SIZE)
}

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn palette(app: &App, ascii: bool) -> Palette {
    Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, ascii)
}

fn buffer(app: &mut App, size: (u16, u16)) -> Buffer {
    let p = palette(app, false);
    render_app(app, &p, size)
}

fn text(app: &mut App, size: (u16, u16)) -> String {
    to_text(&buffer(app, size))
}

fn ascii_text(app: &mut App, size: (u16, u16)) -> String {
    let p = palette(app, true);
    to_text(&render_app(app, &p, size))
}

/// The frame of every screen that draws a meter, with one settler's hunger and energy at zero
/// and the settler selected: the settler view (the selection bar), Nation, Endeavor and the
/// side panel's own research row.
fn meter_frames(ascii: bool) -> Vec<(&'static str, String)> {
    let mut a = app(false);
    a.session.world.units[0].needs.energy = 0.0;
    a.session.world.units[0].needs.hunger = 0.0;
    run(&mut a, "<tab>");
    let frame = |a: &mut App| if ascii { ascii_text(a, SIZE) } else { text(a, SIZE) };
    let mut out = vec![("settler", frame(&mut a))];
    for (key, name) in [("n", "nation"), ("g", "endeavor")] {
        run(&mut a, key);
        out.push((name, frame(&mut a)));
        run(&mut a, "<esc>");
    }
    out
}

/// No `▰▱` pips remain anywhere, and the bars on the settler view, Nation and Endeavor are all
/// the same two glyphs.
#[test]
fn every_bar_is_drawn_from_the_same_two_glyphs() {
    for (name, frame) in meter_frames(false) {
        assert!(!frame.contains('▰') && !frame.contains('▱'), "{name} has pips:\n{frame}");
        assert!(frame.contains('█') || frame.contains('░'), "{name} has no bar:\n{frame}");
    }
    for (name, frame) in meter_frames(true) {
        assert!(frame.is_ascii(), "{name} is not ASCII:\n{frame}");
        assert!(frame.contains("#####") || frame.contains("....."), "{name}:\n{frame}");
    }
}

/// Ten cells with the value after them on the settler view and on Endeavor, five cells in the
/// Nation roster.
#[test]
fn a_meter_is_ten_cells_with_its_value_or_five_in_the_roster() {
    let frames = meter_frames(false);
    let frame = |name: &str| &frames.iter().find(|(n, _)| *n == name).unwrap().1;
    // Settler view: label, ten cells, flag column, percent.
    let health = frame("settler").lines().find(|l| l.contains("Health ")).unwrap().to_owned();
    assert!(health.contains("Health ██████████ 100%"), "{health}");
    // Endeavor: ten cells, then the count.
    let cells = |line: &str| line.chars().filter(|c| matches!(c, '█' | '░')).count();
    let sign = frame("endeavor").lines().find(|l| l.contains("Population")).unwrap();
    assert_eq!(cells(sign), 10, "{sign}");
    let after = sign.trim_start_matches(|c| !matches!(c, '█' | '░'));
    let after = after.trim_start_matches(['█', '░']).trim_start();
    assert!(after.starts_with(|c: char| c.is_ascii_digit()), "the value follows: {sign}");
    // Nation roster: five cells for each of the four needs (chi is in the strip, not the row).
    let row = frame("nation").lines().find(|l| l.contains("▸ ")).unwrap().to_owned();
    assert_eq!(cells(&row), 5 * 4, "{row}");
}

/// A low need is the whole meter in the warning colour. In the Nation roster a `!` sits directly
/// against it and nothing else (no word) is drawn; in the settler view (the side panel's
/// inspector, #653) the word "low" follows the value instead of the `!`.
#[test]
fn a_low_need_is_the_meter_in_the_warning_colour_with_its_flag_against_it() {
    let mut a = app(false);
    a.session.world.units[0].needs.energy = 0.0;
    run(&mut a, "<tab>");
    let p = palette(&a, false);
    let warn = [p.token(p.theme.warn), p.token(p.theme.bad)];
    for screen in ["", "n"] {
        if !screen.is_empty() {
            run(&mut a, screen);
        }
        let buf = buffer(&mut a, SIZE);
        let mut found = 0;
        for y in 0..SIZE.1 {
            for x in 1..SIZE.0 {
                // The roster flags with `!` against the cells; the inspector says "low" after
                // the value, so its meter is the run of empty cells before the row's value.
                let flagged = if screen.is_empty() {
                    buf[(x, y)].symbol() == "l"
                        && buf[(x + 1, y)].symbol() == "o"
                        && buf[(x + 2, y)].symbol() == "w"
                        && buf[(x - 1, y)].symbol() == " "
                        && x >= 2
                        && buf[(x - 2, y)].symbol() == "%"
                } else {
                    buf[(x, y)].symbol() == "!" && buf[(x - 1, y)].symbol() == "░"
                };
                if !flagged {
                    continue;
                }
                found += 1;
                // The empty meter's cells, up to its first cell, all wear the warning.
                let mut at = if screen.is_empty() {
                    // Back over the value (e.g. "  0%") and the flag column to the cells.
                    let mut c = x;
                    while c > 0 && buf[(c - 1, y)].symbol() != "░" {
                        c -= 1;
                    }
                    c
                } else {
                    x
                };
                let end = at;
                while at > 0 && buf[(at - 1, y)].symbol() == "░" {
                    at -= 1;
                    assert!(warn.contains(&buf[(at, y)].fg), "({at},{y}) {:?}", buf[(at, y)].fg);
                }
                assert!(warn.contains(&buf[(x, y)].fg), "the flag wears the warning too");
                let cells = usize::from(end - at);
                assert!(cells == 10 || cells == 5, "{cells} cells at ({at},{y})");
            }
        }
        assert_eq!(found, 1, "exactly one flagged meter on screen {screen:?}");
        if !screen.is_empty() {
            // The roster's row carries the meter and no word for the need.
            let frame = text(&mut a, SIZE);
            let row = frame.lines().find(|l| l.contains("▸ ")).unwrap();
            assert!(!row.contains("Exhausted") && !row.contains("Tired"), "no loose word: {row}");
            run(&mut a, "<esc>");
        }
    }
}

/// `text` with every `mm:ss`-looking run (two digits, a colon, two digits) not directly followed
/// by " left": that is minutes since the start, which no screen shows any more.
fn elapsed_stamps(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    for i in 0..chars.len().saturating_sub(4) {
        let w = &chars[i..i + 5];
        let shape = w[0].is_ascii_digit()
            && w[1].is_ascii_digit()
            && w[2] == ':'
            && w[3].is_ascii_digit()
            && w[4].is_ascii_digit();
        // The tail of a longer number ("115:00") isn't a clock either: look one back.
        let rest: String = chars[i + 5..].iter().take(5).collect();
        if shape && !rest.starts_with(" left") {
            out.push(chars[i..(i + 10).min(chars.len())].iter().collect());
        }
    }
    out
}

/// The panel, the Nation header and the game menu all say the same clock, the season, the year
/// and the time left, with seasons on; nothing shows minutes since the start.
#[test]
fn the_panel_the_nation_header_and_the_menu_say_the_same_clock() {
    for (seasons, at_start, a_minute_in) in [
        (true, "Spring Y1 · 15:00 left", "Spring Y1 · 14:00 left"),
        (false, "Day 1 · 5:00 left", "Day 1 · 4:00 left"),
    ] {
        let mut a = app(seasons);
        for (clock, ticks) in [(at_start, ""), (a_minute_in, "<tick 600> ")] {
            run(&mut a, ticks);
            let panel = text(&mut a, SIZE);
            assert!(panel.contains(clock), "the panel says {clock}:\n{panel}");
            for (key, what) in [("n", "Nation header"), ("<esc>", "game menu"), ("l", "log")] {
                if what == "log" {
                    run(&mut a, "<esc>");
                }
                run(&mut a, key);
                let frame = text(&mut a, SIZE);
                if what != "log" {
                    assert!(frame.contains(clock), "the {what} says {clock}:\n{frame}");
                }
                assert!(elapsed_stamps(&frame).is_empty(), "{what}: {:?}", elapsed_stamps(&frame));
                run(&mut a, "<esc>");
            }
        }
    }
}

/// The panel's clock row has the same shape from tick 0 onward, running or paused: the run state
/// and the speed, then the season (or day) and the time left.
#[test]
fn the_panel_clock_has_one_shape_from_tick_zero_onward() {
    let shape = |frame: &str| -> (String, String) {
        let lines: Vec<&str> = frame.lines().collect();
        let top = lines.iter().position(|l| l.contains("EARTH KINGDOM")).unwrap();
        let cut = |l: &str| {
            let at = l.find('│').unwrap();
            l[at + '│'.len_utf8()..].trim_end_matches('│').trim().to_owned()
        };
        let digits =
            |s: String| s.chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect();
        (digits(cut(lines[top + 1])), digits(cut(lines[top + 2])))
    };
    let mut a = app(true);
    for size in [SIZE, SMALL] {
        let first = shape(&text(&mut a, size));
        assert_eq!(first.0, "▶ Running x#", "{first:?}");
        assert_eq!(first.1, "Spring Y# · ##:## left", "{first:?}");
    }
    // Paused, the same clock row.
    run(&mut a, "<space>");
    let paused = shape(&text(&mut a, SIZE));
    assert_eq!(paused, ("‖ Paused x#".to_owned(), "Spring Y# · ##:## left".to_owned()));
    // Running, a minute in and then a season in: the same two rows.
    run(&mut a, "<space>");
    let running = shape(&text(&mut a, SIZE));
    assert_eq!(running.0, "▶ Running x#", "{running:?}");
    run(&mut a, "<tick 600>");
    assert_eq!(shape(&text(&mut a, SIZE)), running);
    let per = a.session.defs.seasons.season_ticks();
    a.session.world.tick = per + 5;
    let later = shape(&text(&mut a, SIZE));
    assert_eq!(later.1, "Summer Y# · ##:## left", "{later:?}");
    assert_eq!(later.0, running.0);
}

/// The message log stamps a line with the time left in its season, under a heading row for each
/// season, so a line can be placed against the panel's clock.
#[test]
fn the_log_is_stamped_with_the_time_left_under_a_heading_per_season() {
    let mut a = app(true);
    let per = a.session.defs.seasons.season_ticks();
    a.ui.alerts.push_agent(600, "Scouted the east".to_owned());
    a.ui.alerts.push_agent(per + 100, "Raised a wall".to_owned());
    run(&mut a, "l");
    let frame = text(&mut a, SIZE);
    let lines: Vec<&str> = frame.lines().collect();
    let at = |needle: &str| lines.iter().position(|l| l.contains(needle)).unwrap();
    // Newest first: the later season is on top.
    assert!(at("── SUMMER, YEAR 1 ──") < at("14:50 left"), "{frame}");
    assert!(lines[at("14:50 left")].contains("Raised a wall"), "{frame}");
    assert!(at("14:50 left") < at("── SPRING, YEAR 1 ──"), "{frame}");
    assert!(at("── SPRING, YEAR 1 ──") < at("14:00 left"), "{frame}");
    assert!(lines[at("14:00 left")].contains("Scouted the east"), "{frame}");
    assert!(elapsed_stamps(&frame).is_empty(), "{:?}", elapsed_stamps(&frame));
}

/// Wording like "12/16" with no words around it is #663's rule: nothing the new text adds reads
/// as an `n/m` code run into a word.
#[test]
fn the_new_text_has_no_run_together_codes() {
    let mut a = app(true);
    for key in ["", "n", "<esc> <esc>", "l", "<esc>", "g", "<esc>"] {
        run(&mut a, key);
        let frame = text(&mut a, SIZE);
        let chars: Vec<char> = frame.chars().collect();
        for i in 1..chars.len().saturating_sub(2) {
            if chars[i] == '/' && chars[i - 1].is_ascii_digit() {
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                let bad = j > i + 1 && j < chars.len() && chars[j].is_ascii_alphabetic();
                assert!(
                    !bad,
                    "a code run into a word near {:?}",
                    &chars[i - 3..(j + 2).min(chars.len())]
                );
            }
        }
    }
}

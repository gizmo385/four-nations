//! The run browser (#556) against a folder of small fixture runs: metadata parsing with no TUI
//! at all, then the headless frames (`Browser::render`, which `fn_tui --runs DIR --frame` prints)
//! driven by key scripts. Nothing here opens a terminal. The assertions are `contains`-style and
//! layout properties, not golden files.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

use fn_sim::Nation;
use fn_tui::browser::{self, Browser, Outcome, Pool, Scoring, Sort};
use fn_tui::color::ColorDepth;
use fn_tui::frame::to_text;
use fn_tui::keys;
use fn_tui::runs::{self, Mode, RunInfo, State};
use fn_tui::surface::{Host, LogLine, Prelude, PreludeOpening};

const FINISHED: &str = "20261108-142213-water-7633-normal";
const SECOND: &str = "20261107-232721-fire-9-normal";
const SCENARIO: &str = "20261107-004834-scenario-earth-year-two";
const LIVE: &str = "20261109-010101-earth-7-normal";
const CUT: &str = "20261106-012741-earth-7-normal";
const BAD_HEADER: &str = "20261105-020756-earth-7-normal";
const EMPTY: &str = "20261104-235100-earth-7-normal";
const NO_LOG: &str = "20261103-000000-air-1-normal";
const SCORED: &str = "20261102-000000-air-2-normal";

/// The fixture's runs, newest first.
const ORDER: [&str; 9] = [LIVE, FINISHED, SECOND, SCENARIO, CUT, BAD_HEADER, EMPTY, NO_LOG, SCORED];

fn fresh_dir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fn_tui_browser_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A log of `turns` turns of a real (tiny) game, as `fn_agent serve` would write it.
fn game_log(seed: u64, nation: Nation, turns: u32) -> String {
    let mut h = Host::new(seed, nation, None, None, None, false).unwrap().with_turn_seconds(20);
    if turns > 0 {
        h.press_keys("h <right*2> <enter> <down> <enter>", None, None).unwrap();
    }
    for _ in 0..turns {
        h.end_turn(1);
    }
    h.log().to_jsonl(&h.header())
}

/// The scenario fixture's log, made once: a (very short) prelude is the slow part.
fn scenario_log() -> &'static str {
    static LOG: OnceLock<String> = OnceLock::new();
    LOG.get_or_init(|| {
        let mut h = Host::new(7, Nation::Earth, None, None, None, false)
            .unwrap()
            .with_prelude(Prelude { opening: PreludeOpening::Opening, minutes: 1.0 });
        h.end_turn(1);
        h.log().to_jsonl(&h.header())
    })
}

fn end_line(reason: &str) -> String {
    let line = LogLine::End { tick: 400, reason: reason.to_owned() };
    format!("{}\n", serde_json::to_string(&line).unwrap())
}

fn put(dir: &Path, name: &str, log: Option<&str>, age_secs: u64) -> PathBuf {
    let run = dir.join(name);
    fs::create_dir_all(&run).unwrap();
    if let Some(log) = log {
        fs::write(run.join("run.jsonl"), log).unwrap();
    }
    age(&run, age_secs);
    run
}

/// Sets the age of the log (and of the folder, which `list` sorts by).
fn age(run: &Path, secs: u64) {
    let when = SystemTime::now() - Duration::from_secs(secs);
    if let Ok(f) = fs::File::options().write(true).open(run.join("run.jsonl")) {
        f.set_modified(when).unwrap();
    }
    fs::File::open(run).unwrap().set_modified(when).unwrap();
}

/// A folder with one of each kind of run.
fn fixture() -> PathBuf {
    let dir = fresh_dir();
    let day = 86_400;

    let finished = put(
        &dir,
        FINISHED,
        Some(&(game_log(7, Nation::Water, 3) + &end_line("agent finished"))),
        5 * day,
    );
    fs::write(
        finished.join("mcp.json"),
        r#"{"mcpServers":{"four-nations":{"command":"fn_agent","args":["serve","--resume","x","--scripts"]}}}"#,
    )
    .unwrap();
    fs::write(
        finished.join("chunk-1.json"),
        r#"{"terminal_reason":"completed","modelUsage":{"claude-sonnet-5-5":{"costUSD":0.5}}}"#,
    )
    .unwrap();
    fs::write(finished.join("cost.json"), r#"{"round_trips":3,"usd":1.1149784}"#).unwrap();

    let second = put(
        &dir,
        SECOND,
        Some(&(game_log(9, Nation::Fire, 2) + &end_line("stopped by SIGINT"))),
        6 * day,
    );
    fs::write(second.join("mcp.json"), r#"{"mcpServers":{"x":{"args":["serve"]}}}"#).unwrap();
    // Killed during a session that never completed: the end line alone doesn't say finished.
    fs::write(second.join("chunk-1.json"), r#"{"terminal_reason":"aborted","modelUsage":{}}"#)
        .unwrap();

    put(&dir, SCENARIO, Some(&(scenario_log().to_owned() + &end_line("agent finished"))), 7 * day);

    // Written to a moment ago, no end line: live.
    put(&dir, LIVE, Some(&game_log(7, Nation::Earth, 2)), 10);

    // Cut off in the middle of a line, long ago.
    let whole = game_log(7, Nation::Earth, 2);
    put(&dir, CUT, Some(&whole[..whole.len() - 7]), 30 * day);

    put(&dir, BAD_HEADER, Some("this is not a run log\n"), 31 * day);
    put(&dir, EMPTY, Some(""), 32 * day);
    put(&dir, NO_LOG, None, 33 * day);

    // A log that has its own `score` line, made at another horizon.
    let score = LogLine::Score {
        score: 41.25,
        progress: 0.3,
        care: 0.9,
        horizon: 2,
        attempt: None,
        champion: None,
        build: Some(fn_tui::buildid::CURRENT.to_owned()),
    };
    put(
        &dir,
        SCORED,
        Some(&format!(
            "{}{}\n",
            game_log(2, Nation::Air, 2) + &end_line("agent finished"),
            serde_json::to_string(&score).unwrap()
        )),
        8 * day,
    );
    dir
}

fn open(dir: &Path) -> Browser {
    Browser::open(dir, SystemTime::now(), Scoring::Inline, ColorDepth::Never, false)
}

fn frame(b: &mut Browser, w: u16, h: u16) -> String {
    to_text(&b.render((w, h)))
}

fn press(b: &mut Browser, script: &str) {
    b.play(&keys::parse(script).unwrap());
}

fn line_with<'a>(frame: &'a str, name: &str) -> &'a str {
    frame
        .lines()
        .find(|l| l.contains(name))
        .unwrap_or_else(|| panic!("no row for {name}:\n{frame}"))
}

// ---- metadata, with no TUI ----

#[test]
fn metadata_reads_what_each_run_has_and_leaves_gaps_for_what_it_hasnt() {
    let dir = fixture();
    let now = SystemTime::now();
    let info = |name: &str| RunInfo::read(&dir.join(name), now);

    let f = info(FINISHED);
    assert_eq!(f.stamp.as_deref(), Some("20261108-142213"));
    assert_eq!(f.started_text().as_deref(), Some("2026-11-08 14:22"));
    assert_eq!((f.nation.as_deref(), f.seed), (Some("water"), Some(7)));
    assert_eq!(f.difficulty, None, "the fixture's header names none");
    assert_eq!(f.start_text().as_deref(), Some("seed 7"));
    assert_eq!(f.mode, Some(Mode::Scripts));
    assert_eq!(f.model.as_deref(), Some("sonnet-5-5"));
    assert_eq!(f.turns, 3);
    assert_eq!(f.game_time().as_deref(), Some("0:01"), "3 turns of 20 s");
    assert_eq!(f.cost_usd, Some(1.1149784));
    assert_eq!(f.state(now), Some(State::Finished));
    assert_eq!(f.problem, None);

    // A run with no mcp.json, chunk or cost files: those are gaps, and the row is still whole.
    let l = info(LIVE);
    assert_eq!((l.mode, l.model.clone(), l.cost_usd), (Some(Mode::Turns), None, None));
    assert_eq!(l.state(now), Some(State::Live), "written to 10 s ago with no end line");
    assert_eq!(l.state(now + Duration::from_secs(600)), Some(State::Stopped), "and ten minutes on");

    // `stopped by SIGINT` is a normal end only when the last session completed.
    let s = info(SECOND);
    assert_eq!(s.mode, Some(Mode::Turns));
    assert_eq!(s.state(now), Some(State::Stopped));

    // A scenario: from the header's prelude and the folder name.
    let sc = info(SCENARIO);
    assert_eq!(sc.start_text().as_deref(), Some("scenario earth-year-two"));

    // A log's own score line, and the horizon it was made at.
    let sd = info(SCORED);
    assert!(matches!(sd.logged_score_text().as_deref(), Some("41.2@2" | "41.3@2")));
    assert_eq!(sd.logged_score.as_ref().map(|s| (s.score, s.horizon)), Some((41.25, 2)));
    // #566: a score some other build computed (or a line from before builds were recorded) wears
    // a mark; one this build computed doesn't.
    let mut other = sd.clone();
    other.logged_score.as_mut().unwrap().build = Some("0badc0d".to_owned());
    assert!(other.logged_score_text().unwrap().ends_with('~'));
    other.logged_score.as_mut().unwrap().build = None;
    assert!(other.logged_score_text().unwrap().ends_with('~'));
    assert!(!sd.logged_score_text().unwrap().contains('~'));
    assert_eq!(
        sd.state(now),
        Some(State::Finished),
        "a score line after the end doesn't un-end it"
    );
}

#[test]
fn unreadable_and_partial_logs_say_why_and_never_hide_the_row() {
    let dir = fixture();
    let now = SystemTime::now();
    let why = |name: &str| RunInfo::read(&dir.join(name), now).problem.unwrap();

    let cut = why(CUT);
    assert_eq!(cut.short, "cut off");
    assert!(!cut.unopenable, "what is complete can still be watched");
    assert!(
        cut.why.contains("middle of a line") || cut.why.contains("doesn't parse"),
        "{}",
        cut.why
    );
    for (name, short) in [(BAD_HEADER, "unreadable"), (EMPTY, "empty log"), (NO_LOG, "no log")] {
        let p = why(name);
        assert_eq!(p.short, short, "{name}");
        assert!(p.unopenable && !p.why.is_empty(), "{name}: {p:?}");
        assert_eq!(RunInfo::read(&dir.join(name), now).state(now), None);
    }
    // A campaign folder has no log of its own, and says so.
    let campaign = dir.join("campaign-1");
    fs::create_dir_all(&campaign).unwrap();
    fs::write(campaign.join("ledger.json"), "{}").unwrap();
    assert!(RunInfo::read(&campaign, now).problem.unwrap().why.contains("campaign"));
}

#[test]
fn a_log_still_being_written_is_not_called_cut_off() {
    let dir = fresh_dir();
    let whole = game_log(7, Nation::Earth, 2);
    put(&dir, "20261109-000000-earth-7-normal", Some(&whole[..whole.len() - 7]), 2);
    let info = RunInfo::read(&dir.join("20261109-000000-earth-7-normal"), SystemTime::now());
    assert_eq!(info.problem, None);
    assert_eq!(info.state(SystemTime::now()), Some(State::Live));
}

// ---- scores ----

#[test]
fn scores_come_from_the_replay_at_540_and_are_cached_without_touching_the_log() {
    let dir = fixture();
    let run = dir.join(FINISHED);
    let before = fs::read(run.join("run.jsonl")).unwrap();

    let (bytes, scored) = runs::score_run(&run);
    let scored = scored.unwrap();
    assert_eq!(bytes, before.len() as u64);
    assert!(scored.score > 0.0, "{scored:?}");
    assert_eq!(scored.signs_total, 9, "the Harmony Gate's nine signs");
    assert!(scored.signs_met <= scored.signs_total);

    // The same call `fn_agent score` makes: referee::score at the horizon with the bundled weights.
    let (header, lines) = fn_tui::watch::read_file(&run.join("run.jsonl")).unwrap();
    let replay = fn_tui::surface::referee::score(
        &header,
        &lines,
        Some(runs::SCORE_HORIZON),
        &fn_tui::surface::referee::score::ScoreDef::bundled(),
    )
    .unwrap();
    assert_eq!(scored.score, replay.card.score);
    assert_eq!(scored.deaths, replay.card.care.deaths);
    assert_eq!(replay.horizon_turns, 540, "scored at the horizon, though the log stops at 3 turns");

    assert_eq!(fs::read(run.join("run.jsonl")).unwrap(), before, "the log is untouched");
    assert!(run.join(runs::CACHE_FILE).is_file());
    assert_eq!(runs::cached_score(&run, bytes), Some(Ok(scored.clone())));
    assert_eq!(runs::cached_score(&run, bytes + 1), None, "a log of another length is a miss");

    // A hit is read from the file, not replayed: a doctored cache is what comes back.
    let text = fs::read_to_string(run.join(runs::CACHE_FILE)).unwrap();
    let doctored = text.replacen(&format!("\"score\":{}", scored.score), "\"score\":999.5", 1);
    assert_ne!(text, doctored);
    fs::write(run.join(runs::CACHE_FILE), doctored).unwrap();
    assert_eq!(runs::score_run(&run).1.unwrap().score, 999.5);

    // A log that grew is scored again.
    let mut grown = fs::read_to_string(run.join("run.jsonl")).unwrap();
    grown.push_str("{\"type\":\"note\",\"text\":\"later\"}\n");
    fs::write(run.join("run.jsonl"), grown).unwrap();
    assert_eq!(runs::score_run(&run).1.unwrap().score, scored.score);
}

#[test]
fn a_log_made_against_other_data_has_a_score_error_not_a_crash() {
    let dir = fresh_dir();
    let log = game_log(7, Nation::Earth, 1).replacen("\"data_digest\":", "\"data_digest\":1", 1);
    let run = put(
        &dir,
        "20261109-000000-earth-7-normal",
        Some(&(log + &end_line("agent finished"))),
        3600,
    );
    let (_, result) = runs::score_run(&run);
    assert!(result.unwrap_err().contains("different data"));
    // The failure is cached too: replaying it again would fail the same way.
    assert!(
        runs::cached_score(&run, fs::metadata(run.join("run.jsonl")).unwrap().len())
            .unwrap()
            .is_err()
    );
    let mut b = open(&dir);
    let f = frame(&mut b, 120, 20);
    assert!(line_with(&f, "20261109").contains("error"), "{f}");
    assert!(f.contains("different data"), "the status line says why:\n{f}");
}

// ---- the list ----

#[test]
fn the_list_shows_every_run_newest_first_with_the_newest_selected() {
    let dir = fixture();
    let mut b = open(&dir);
    let f = frame(&mut b, 200, 30);

    for name in [FINISHED, SECOND, SCENARIO, LIVE, CUT, BAD_HEADER, EMPTY, NO_LOG, SCORED] {
        assert!(f.contains(name), "{name} should have a row:\n{f}");
    }
    let order: Vec<usize> = ORDER.iter().map(|n| f.find(n).unwrap()).collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "newest first:\n{f}");
    assert!(line_with(&f, LIVE).starts_with("> "), "the newest is selected:\n{f}");
    assert_eq!(b.selected_name(), Some(LIVE));

    // Every column the issue names has a heading at this width.
    let head = f.lines().nth(1).unwrap();
    for col in [
        "STATE",
        "SCORE@540",
        "TURNS",
        "GAME",
        "SIGNS",
        "DEAD",
        "NATION",
        "START",
        "MODE",
        "COST",
        "MODEL",
        "DIFF",
        "STARTED",
    ] {
        assert!(head.contains(col), "{col} in {head:?}");
    }

    let fin = line_with(&f, FINISHED);
    for want in [
        "finished",
        "water",
        "seed 7",
        "scripts",
        "$1.11",
        "sonnet-5-5",
        "2026-11-08 14:22",
        "0:01",
    ] {
        assert!(fin.contains(want), "{want} in {fin:?}");
    }
    assert!(line_with(&f, LIVE).contains("live"));
    assert!(line_with(&f, SECOND).contains("stopped early"));
    assert!(line_with(&f, SCENARIO).contains("scenario earth-year-two"));
    // Where a run lacks a value, a dash, and the row is still there.
    assert!(line_with(&f, LIVE).contains(" - "), "{f}");
    // A log's own score line wins, with its horizon shown when it isn't 540.
    assert!(
        line_with(&f, SCORED).contains("41.2@2") || line_with(&f, SCORED).contains("41.3@2"),
        "{f}"
    );
    // And the others say why they have no row data.
    for (name, why) in
        [(CUT, "cut off"), (BAD_HEADER, "unreadable"), (EMPTY, "empty log"), (NO_LOG, "no log")]
    {
        assert!(line_with(&f, name).contains(why), "{name} should say {why}:\n{f}");
    }
}

#[test]
fn the_score_column_says_it_is_at_540_and_how_it_got_there() {
    let dir = fixture();
    let mut b = open(&dir);
    let f = frame(&mut b, 200, 30);
    assert!(f.contains("SCORE@540"));
    // The newest (live) run's score was replayed; the status line says by what.
    assert!(f.contains("at 540 turns, as fn_agent score computes it"), "{f}");
    press(&mut b, "<down>");
    // (Selecting down moves to the finished run, replayed the same way.)
    let f = frame(&mut b, 200, 30);
    assert!(f.contains(&format!("{FINISHED}: score")), "{f}");
    press(&mut b, "G k");
    assert_eq!(b.selected_name(), Some(NO_LOG));
    let f = frame(&mut b, 200, 30);
    assert!(f.contains("no run.jsonl"), "{f}");
}

#[test]
fn narrow_terminals_drop_columns_from_the_right_and_keep_names_and_the_selection_whole() {
    let dir = fixture();
    let mut b = open(&dir);
    let all = ORDER;
    let columns = [
        "STATE",
        "SCORE@540",
        "TURNS",
        "GAME",
        "SIGNS",
        "DEAD",
        "NATION",
        "START",
        "MODE",
        "COST",
        "MODEL",
        "DIFF",
        "STARTED",
    ];
    let mut last_kept = columns.len();
    for width in [200u16, 160, 130, 110, 100, 80, 60] {
        let f = frame(&mut b, width, 24);
        for line in f.lines() {
            assert!(line.chars().count() <= width as usize, "{width}: {line:?}");
        }
        // Run names are whole, at any of these widths; none is wrapped onto a second line.
        for name in all {
            let rows = f.lines().filter(|l| l.get(2..).is_some_and(|r| r.starts_with(name)));
            assert_eq!(rows.count(), 1, "{width}: {name}\n{f}");
        }
        assert!(line_with(&f, LIVE).starts_with("> "), "{width}: the selected row\n{f}");
        // The heading keeps a prefix of the columns: dropping goes from the least valuable.
        let head = f.lines().nth(1).unwrap();
        let kept = columns.iter().take_while(|c| head.contains(*c)).count();
        assert!(columns[kept..].iter().all(|c| !head.contains(c)), "{width}: {head:?}");
        assert!(kept <= last_kept, "{width}: narrower never shows more");
        last_kept = kept;
        if width >= 100 {
            assert!(kept >= 6, "{width}: the valuable columns survive in {head:?}");
            assert!(head.contains("SCORE@540"));
        }
        // Each row is the same shape as the heading: a value under every kept column.
        assert!(line_with(&f, FINISHED).contains("finished"), "{width}\n{f}");
    }
    assert!(last_kept < columns.len(), "60 columns can't hold everything");
}

#[test]
fn the_list_scrolls_to_keep_the_selection_on_screen() {
    let dir = fixture();
    let mut b = open(&dir);
    press(&mut b, "G");
    let f = frame(&mut b, 120, 8);
    assert!(f.contains(SCORED), "{f}");
    assert!(line_with(&f, SCORED).starts_with("> "), "{f}");
    assert!(!f.contains(LIVE), "the top has scrolled off:\n{f}");
    press(&mut b, "g");
    assert!(frame(&mut b, 120, 8).contains(LIVE));
    // A terminal too small for anything still draws without panicking.
    for (w, h) in [(1, 1), (10, 3), (20, 2), (40, 5), (0, 0)] {
        let _ = frame(&mut b, w, h);
    }
}

#[test]
fn keys_move_sort_and_filter() {
    let dir = fixture();
    let mut b = open(&dir);

    press(&mut b, "j j");
    assert_eq!(b.selected_name(), Some(SECOND));
    press(&mut b, "k");
    assert_eq!(b.selected_name(), Some(FINISHED));
    press(&mut b, "<down>");
    assert_eq!(b.selected_name(), Some(SECOND));
    press(&mut b, "<up> <up> <up>");
    assert_eq!(b.selected_name(), Some(LIVE), "stops at the top");

    // Score order: highest first (the log's own 41.25, then the scenario's replay), the rest
    // newest first among equals, and runs with no score last.
    press(&mut b, "s");
    assert_eq!(b.sort(), Sort::Score);
    let f = frame(&mut b, 200, 30);
    assert!(f.contains("sort: score"));
    let by_score = [SCORED, SCENARIO, LIVE, FINISHED, SECOND, CUT, BAD_HEADER, EMPTY, NO_LOG];
    let at: Vec<usize> = by_score.iter().map(|n| f.find(n).unwrap()).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{f}");
    assert_eq!(b.selected_name(), Some(LIVE), "the selection follows its run through a re-sort");
    press(&mut b, "s");
    assert_eq!(b.sort(), Sort::Date);

    // Nation filter: air, earth, fire, water, then back to all.
    press(&mut b, "n");
    assert_eq!(b.nation_filter(), Some("air"));
    let f = frame(&mut b, 200, 30);
    assert!(f.contains(SCORED) && !f.contains(FINISHED) && !f.contains(LIVE), "{f}");
    assert!(f.contains("nation: air") && f.contains("(1 of 9)"), "{f}");
    assert_eq!(b.selected_name(), Some(SCORED), "the selection moves into the filtered list");
    press(&mut b, "n n n");
    assert_eq!(b.nation_filter(), Some("water"));
    press(&mut b, "n");
    assert_eq!(b.nation_filter(), None);
    assert!(frame(&mut b, 200, 30).contains("nation: all"));
}

#[test]
fn enter_opens_the_run_and_q_in_the_watcher_returns_to_the_same_row() {
    let dir = fixture();
    let mut b = open(&dir);
    press(&mut b, "G");
    assert_eq!(b.selected_name(), Some(SCORED));

    press(&mut b, "<enter>");
    assert!(b.is_watching());
    let f = frame(&mut b, 120, 40);
    assert!(f.contains("AGENT air") && f.contains("turn 2"), "the watcher's own frame:\n{f}");
    assert!(!f.contains("SCORE@540"));

    // Its keys are the watcher's: j scrolls the code pane, it does not move the list.
    press(&mut b, "j");
    assert!(b.is_watching());
    press(&mut b, "q");
    assert!(!b.is_watching() && !b.quit());
    assert_eq!(b.selected_name(), Some(SCORED), "back on the run it was opened from");
    assert!(line_with(&frame(&mut b, 120, 24), SCORED).starts_with("> "));

    // q in the list exits.
    press(&mut b, "q");
    assert!(b.quit());
}

#[test]
fn opening_a_run_with_no_usable_log_says_so_instead_of_crashing() {
    let dir = fixture();
    let mut b = open(&dir);
    for (name, want) in [
        (NO_LOG, "no run.jsonl"),
        (EMPTY, "run.jsonl is empty"),
        (BAD_HEADER, "isn't a run header"),
    ] {
        b.set_notice("");
        let at = ORDER.iter().position(|n| *n == name).unwrap();
        press(&mut b, "g");
        press(&mut b, &"j ".repeat(at));
        assert_eq!(b.selected_name(), Some(name));
        press(&mut b, "<enter>");
        assert!(!b.is_watching(), "{name}");
        let f = frame(&mut b, 120, 24);
        assert!(f.contains("can't open") && f.contains(want), "{name}:\n{f}");
        assert!(line_with(&f, name).starts_with("> "), "still on that row");
    }
    // A cut-off log still opens, with what is complete.
    press(&mut b, "g");
    press(&mut b, &"j ".repeat(4));
    assert_eq!(b.selected_name(), Some(CUT));
    press(&mut b, "<enter>");
    assert!(b.is_watching());
}

#[test]
fn a_live_run_refreshes_its_turn_count_while_the_browser_is_open() {
    let dir = fresh_dir();
    let name = "20261109-010101-earth-7-normal";
    let two = game_log(7, Nation::Earth, 2);
    let four = game_log(7, Nation::Earth, 4);
    assert!(four.starts_with(&two), "a longer game's log extends a shorter one's");
    let run = put(&dir, name, Some(&two), 1);
    let now = SystemTime::now();
    let mut b = Browser::open(&dir, now, Scoring::Inline, ColorDepth::Never, false);
    let row = |b: &mut Browser| line_with(&frame(b, 200, 12), name).to_owned();
    let turns = |row: &str| -> String {
        row.split_whitespace().skip_while(|w| *w != "live").nth(2).unwrap().to_owned()
    };
    assert!(row(&mut b).contains("live"));
    assert_eq!(turns(&row(&mut b)), "2", "{}", row(&mut b));

    fs::write(run.join("run.jsonl"), &four).unwrap();
    b.refresh(now + Duration::from_secs(2));
    assert_eq!(turns(&row(&mut b)), "4", "{}", row(&mut b));

    // Once it has been quiet for a few minutes with no end line it reads as stopped.
    b.refresh(now + Duration::from_secs(900));
    assert!(row(&mut b).contains("stopped early"), "{}", row(&mut b));

    // New runs appear and gone ones go.
    put(&dir, "20261109-020202-fire-3-normal", Some(&game_log(3, Nation::Fire, 1)), 1);
    b.refresh(now + Duration::from_secs(901));
    assert!(frame(&mut b, 200, 12).contains("20261109-020202"));
    fs::remove_dir_all(&run).unwrap();
    b.refresh(now + Duration::from_secs(902));
    assert!(!frame(&mut b, 200, 12).contains(name));
}

#[test]
fn a_growing_log_is_not_replayed_over_and_over() {
    let dir = fresh_dir();
    let name = "20261109-010101-earth-7-normal";
    let run = put(&dir, name, Some(&game_log(7, Nation::Earth, 1)), 1);
    let now = SystemTime::now();
    let mut b = Browser::open(&dir, now, Scoring::Inline, ColorDepth::Never, false);
    // Doctor the cache, then grow the log: within RESCORE_LIVE the old score stands (the cache
    // no longer matches the log, but a replay is not worth it yet)...
    let two = game_log(7, Nation::Earth, 2);
    fs::write(run.join("run.jsonl"), &two).unwrap();
    b.refresh(now + Duration::from_secs(5));
    assert!(!b.scoring_pending());
    // ...and after it, the score is asked for again and now matches the longer log.
    b.refresh(now + browser::RESCORE_LIVE + Duration::from_secs(1));
    let bytes = fs::metadata(run.join("run.jsonl")).unwrap().len();
    assert!(runs::cached_score(&run, bytes).is_some());
}

// ---- scoring in the background ----

#[test]
fn background_scoring_never_blocks_the_list() {
    let dir = fixture();
    let mut b = Browser::open(
        &dir,
        SystemTime::now(),
        Scoring::Background(Pool::new(2)),
        ColorDepth::Never,
        false,
    );
    // The first frame is there before any replay is done: scores are `...`.
    let f = frame(&mut b, 200, 30);
    assert!(f.contains(FINISHED) && f.contains(NO_LOG), "{f}");
    assert!(line_with(&f, FINISHED).contains("..."), "{f}");
    assert!(b.scoring_pending());
    // (A run that can't be scored at all is a dash straight away, not a wait.)
    assert!(!line_with(&f, NO_LOG).contains("..."), "{f}");

    b.wait_for_scores(Duration::from_secs(120));
    assert!(!b.scoring_pending(), "every score should settle");
    let f = frame(&mut b, 200, 30);
    assert!(!line_with(&f, FINISHED).contains("..."), "{f}");
    assert!(!line_with(&f, LIVE).contains("..."), "{f}");
    assert!(
        fs::read_dir(dir.join(FINISHED))
            .unwrap()
            .any(|e| e.unwrap().file_name() == runs::CACHE_FILE)
    );
}

// ---- the binary ----

fn fn_tui() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fn_tui"));
    cmd.stdin(Stdio::null()).env_remove("NO_COLOR").env_remove("COLORTERM");
    cmd
}

#[test]
fn runs_frame_prints_the_list_and_takes_keys() {
    let dir = fixture();
    let out = fn_tui()
        .args(["--runs"])
        .arg(&dir)
        .args(["--frame", "--size", "130x24", "--keys", "j s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(text.lines().count(), 24);
    assert!(text.contains("sort: score") && text.contains(FINISHED), "{text}");

    // Enter, then the watcher's frame.
    let out = fn_tui()
        .args(["--runs"])
        .arg(&dir)
        .args(["--frame", "--size", "100x30", "--keys", "<enter>"])
        .output()
        .unwrap();
    assert!(String::from_utf8(out.stdout).unwrap().contains("AGENT earth"));
}

#[test]
fn runs_without_a_terminal_or_with_the_wrong_flags_says_what_is_wrong() {
    let dir = fixture();
    let out = fn_tui().arg("--runs").arg(&dir).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("needs a terminal"));

    for (args, want) in [
        (vec!["--runs"], "--runs needs a value"),
        (vec!["--runs", "x", "7", "earth"], "takes no SEED"),
        (vec!["--runs", "x", "--watch", "l.jsonl"], "can't be combined"),
        (vec!["--runs", "x", "--keys", "j"], "only means something with --frame"),
        (vec!["--runs", "x", "--pace", "3"], "--pace wants one of"),
    ] {
        let out = fn_tui().args(&args).output().unwrap();
        assert!(!out.status.success(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(want),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    // `--help` still prints the usage, which mentions the browser.
    let out = fn_tui().arg("--help").output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("--runs DIR"));
}

#[test]
fn outcome_open_names_the_folder() {
    let dir = fixture();
    let mut b = open(&dir);
    let key = keys::parse("<enter>").unwrap();
    let fn_tui::keys::Step::Key(enter) = key[0] else { panic!() };
    assert_eq!(b.key(enter), Outcome::Open(dir.join(LIVE)));
}

// ---- playtests (#576) ----

/// A log of `turns` turns that `agent-run --playtest` would have started: the header says so, and
/// a remark sits in the middle.
fn playtest_log(seed: u64, nation: Nation, turns: u32) -> String {
    let mut h = Host::new(seed, nation, None, None, None, false)
        .unwrap()
        .with_turn_seconds(20)
        .with_playtest();
    for _ in 0..turns {
        h.end_turn(1);
    }
    let mut text = h.log().to_jsonl(&h.header());
    let say = LogLine::Say { text: "game problem: the well gives no water".to_owned() };
    text.push_str(&serde_json::to_string(&say).unwrap());
    text.push('\n');
    text
}

#[test]
fn a_playtest_is_marked_in_its_row_and_kept_out_of_the_score_ranking() {
    let dir = fresh_dir();
    let day = 86_400;
    // (SECOND is the scored run, FINISHED the playtest: runs list newest first by their names.)
    put(
        &dir,
        SECOND,
        Some(&(game_log(7, Nation::Water, 3) + &end_line("agent finished"))),
        2 * day,
    );
    put(
        &dir,
        FINISHED,
        Some(&(playtest_log(9, Nation::Fire, 2) + &end_line("agent finished"))),
        100,
    );

    let info = RunInfo::read(&dir.join(FINISHED), SystemTime::now());
    assert!(info.playtest);
    assert!(!RunInfo::read(&dir.join(SECOND), SystemTime::now()).playtest);
    // A remark is not a turn, a key press or an end: the run reads as it would without it.
    assert_eq!(info.turns, 2);
    assert_eq!(info.state(SystemTime::now()), Some(State::Finished));

    let mut b = open(&dir);
    b.wait_for_scores(Duration::from_secs(60));
    let f = frame(&mut b, 200, 20);
    assert!(line_with(&f, FINISHED).contains("playtest"), "{f}");
    assert!(!line_with(&f, SECOND).contains("playtest"), "{f}");
    // Its score is not shown beside the scored run's.
    assert!(line_with(&f, SECOND).contains('.'), "the scored run has a number:\n{f}");

    // The playtest is the newer run, so it leads by date; by score it is last.
    press(&mut b, "s");
    assert_eq!(b.sort(), Sort::Score);
    let f = frame(&mut b, 200, 20);
    let at = |name: &str| f.find(name).unwrap();
    assert!(at(SECOND) < at(FINISHED), "{f}");
    let f = frame(&mut b, 200, 20);
    assert!(f.contains("a playtest"), "the status line says so:\n{f}");
}

#[test]
fn a_playtest_is_still_marked_when_the_terminal_is_narrow() {
    let dir = fresh_dir();
    put(&dir, SECOND, Some(&(playtest_log(9, Nation::Fire, 2) + &end_line("agent finished"))), 100);
    let mut b = open(&dir);
    b.wait_for_scores(Duration::from_secs(60));
    // The score column comes first after the state, so it survives a narrow terminal.
    let f = frame(&mut b, 60, 12);
    assert!(f.contains("playtest"), "{f}");
}

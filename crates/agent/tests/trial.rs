//! `test_governor` (#561, `fn_agent::trial`): a clean test of the saved governor and the plan from
//! turn 0, on a world of its own, from inside a live session.
//!
//! What these pin: the result is the clean evaluation's own (same stop reason, turn and score as
//! `fn_agent play --horizon`); the live game is untouched, so a log with `test_governor` calls in
//! it replays and scores exactly like the same session without them; only the train seeds the
//! session was given can be tested on; and the number of tests per attempt is bounded, across a
//! `--resume` too.

use fn_agent::clean;
use fn_agent::runlog::Writer;
use fn_agent::scorecard;
use fn_agent::script::Limits;
use fn_agent::tools::Tools;
use fn_agent::trial::{self, Trial};
use fn_tui::surface::{DEFAULT_TURN_SECONDS, Host, LogLine};
use fn_tui::watch;
use serde_json::{Value, json};

const LIVE_SEED: u64 = 7;
const OTHER_SEED: u64 = 42;
const HELD_OUT: u64 = 1;
/// Short enough for an unoptimised build; the real tracks are 540.
const HORIZON: u32 = 24;

/// The stated sizes of a result (#570): the summary form, and the detail form.
const SUMMARY_BYTES: usize = 1600;
const DETAIL_BYTES: usize = 3200;

const WORKS: &str = "loop { wait(1); }";
const RUNS_AWAY: &str = "wait(1); wait(1); loop { }";
const ERRORS: &str = "let x = 1 / 0;";

fn host(seed: u64) -> Host {
    let nation = fn_tui::cli::parse_nation("water").unwrap();
    Host::new(seed, nation, None, None, None, false)
        .unwrap()
        .with_turn_seconds(DEFAULT_TURN_SECONDS)
}

fn trial_config() -> Trial {
    Trial::new(vec![LIVE_SEED, OTHER_SEED], HORIZON).unwrap()
}

/// Few enough tests to run them all in an unoptimised build.
const FEW: u32 = 4;

fn tools_in_memory() -> Tools<Vec<u8>> {
    Tools::new(host(LIVE_SEED), Writer::new(Vec::new()))
        .unwrap()
        .with_scripts(Limits::bundled())
        .with_trial(trial_config())
}

fn tools_with_few_tests() -> Tools<Vec<u8>> {
    Tools::new(host(LIVE_SEED), Writer::new(Vec::new()))
        .unwrap()
        .with_scripts(Limits::bundled())
        .with_trial(trial_config().with_max_tests(FEW))
}

fn save(tools: &mut Tools<Vec<u8>>, code: &str) {
    tools.call("save_script", json!({"name": "governor", "code": code})).unwrap();
}

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("fn_agent_trial_test_{}_{name}", std::process::id()))
}

#[test]
fn the_tool_is_offered_only_with_scripts_and_a_trial() {
    let names = |t: &Tools<Vec<u8>>| -> Vec<String> {
        t.list().iter().map(|t| t["name"].as_str().unwrap().to_owned()).collect()
    };
    let plain = Tools::new(host(LIVE_SEED), Writer::new(Vec::new())).unwrap();
    assert!(!names(&plain).contains(&trial::TOOL.to_owned()));
    // A trial without scripts offers nothing (there is no library to test).
    let no_scripts =
        Tools::new(host(LIVE_SEED), Writer::new(Vec::new())).unwrap().with_trial(trial_config());
    assert!(!names(&no_scripts).contains(&trial::TOOL.to_owned()));
    assert!(names(&tools_in_memory()).contains(&trial::TOOL.to_owned()));
}

/// The test's verdict is what the clean evaluation computes: `fn_agent play --horizon`'s own steps
/// (`clean::run` into a log file, then `scorecard::build` over the file) on the same seed.
#[test]
fn the_verdict_is_the_clean_evaluations_own() {
    for (code, stops) in [(WORKS, false), (RUNS_AWAY, true), (ERRORS, true)] {
        // The reference, as `play_to_horizon` does it.
        let log = temp_path("reference.jsonl");
        let mut reference = host(OTHER_SEED);
        let mut writer = Writer::create(&log).unwrap();
        writer.write_header(&reference.header()).unwrap();
        let run = clean::run(
            &mut reference,
            code,
            "play",
            &json!({}),
            HORIZON,
            &Limits::bundled(),
            |line| writer.write_line(line).map_err(|e| e.to_string()),
        )
        .unwrap();
        writer.write_end(reference.tick(), "horizon reached").unwrap();
        let (header, lines) = watch::read_file(&log).unwrap();
        std::fs::remove_file(&log).ok();
        let want = scorecard::build(&header, &lines, Some(HORIZON)).unwrap().replay.card.score;

        // The tool, inside a session playing a different map.
        let mut tools = tools_in_memory();
        save(&mut tools, code);
        let got = tools.call("test_governor", json!({"seed": OTHER_SEED})).unwrap();
        assert_eq!(got["seed"], OTHER_SEED);
        assert_eq!(got["turns"], HORIZON);
        assert_eq!(got["score"].as_f64().unwrap(), want, "{code}: {got}");
        assert_eq!(got["stopped"], run.stopped_by.as_ref().map_or(Value::Null, |s| s.json()));
        assert_eq!(got["stopped"].is_object(), stops, "{code}: {got}");
        assert_eq!(got["tests_left"], trial_config().max_tests - 1);
    }
}

#[test]
fn a_stop_names_its_reason_turn_and_why_in_a_short_result() {
    let mut tools = tools_in_memory();
    save(&mut tools, ERRORS);
    let r = tools.call("test_governor", json!({})).unwrap();
    assert_eq!(r["seed"], LIVE_SEED, "defaults to the map being played");
    assert_eq!(r["stopped"]["reason"], "error");
    assert_eq!(r["stopped"]["turn"], 0);
    assert!(r["stopped"]["why"].as_str().unwrap().contains("zero"), "{r}");
    assert!(r["summary"].as_str().unwrap().contains("coasted with no orders"), "{r}");
    assert!(r.to_string().len() < SUMMARY_BYTES, "a short result, not a log: {r}");

    save(&mut tools, RUNS_AWAY);
    let r = tools.call("test_governor", json!({})).unwrap();
    assert_eq!(r["stopped"]["reason"], "runaway");
    assert_eq!(r["stopped"]["turn"], 2);

    save(&mut tools, WORKS);
    let r = tools.call("test_governor", json!({})).unwrap();
    assert_eq!(r["stopped"], Value::Null);
    assert!(r["summary"].as_str().unwrap().contains("without stopping"), "{r}");
}

#[test]
fn turns_ask_for_a_shorter_test_and_never_a_longer_one() {
    let mut tools = tools_in_memory();
    save(&mut tools, WORKS);
    assert_eq!(tools.call("test_governor", json!({"turns": 5})).unwrap()["turns"], 5);
    assert_eq!(tools.call("test_governor", json!({"turns": 100_000})).unwrap()["turns"], HORIZON);
}

#[test]
fn only_the_trains_seeds_and_a_saved_governor_will_do() {
    let mut tools = tools_in_memory();
    let err = tools.call("test_governor", json!({})).unwrap_err();
    assert!(err.contains("governor") && err.contains("save_script"), "{err}");
    save(&mut tools, WORKS);
    let err = tools.call("test_governor", json!({"seed": HELD_OUT})).unwrap_err();
    assert!(err.contains("isn't an evaluation map"), "{err}");
    // The refusal names the allowed seeds and nothing else.
    assert!(err.contains(&format!("{LIVE_SEED}, {OTHER_SEED}")), "{err}");
}

#[test]
fn the_plan_the_session_holds_is_the_plan_that_is_tested() {
    let mut tools = tools_in_memory();
    // Errors unless the plan asks it not to.
    save(&mut tools, "if plan().stop == () { loop { wait(1); } } else { let x = 1 / 0; }");
    let r = tools.call("test_governor", json!({})).unwrap();
    assert_eq!(r["stopped"], Value::Null, "{r}");
    tools.call("plan", json!({"set": {"stop": true}})).unwrap();
    let r = tools.call("test_governor", json!({})).unwrap();
    assert_eq!(r["stopped"]["reason"], "error", "{r}");
}

#[test]
fn tests_are_limited_per_attempt() {
    let mut tools = tools_with_few_tests();
    save(&mut tools, WORKS);
    for n in 1..=FEW {
        let r = tools.call("test_governor", json!({"turns": 1})).unwrap();
        assert_eq!(r["tests_left"], FEW - n);
    }
    let err = tools.call("test_governor", json!({"turns": 1})).unwrap_err();
    assert!(err.contains("used"), "{err}");
}

/// A chunked attempt (`serve --resume`) shares one count: it is read back from the log's own
/// `call` lines.
#[test]
fn a_resumed_session_remembers_how_many_tests_were_used() {
    let path = temp_path("resume.jsonl");
    {
        let mut tools = Tools::new(host(LIVE_SEED), Writer::create(&path).unwrap())
            .unwrap()
            .with_scripts(Limits::bundled())
            .with_trial(trial_config().with_max_tests(FEW));
        tools.call("save_script", json!({"name": "governor", "code": WORKS})).unwrap();
        for _ in 0..FEW - 1 {
            tools.call("test_governor", json!({"turns": 1})).unwrap();
        }
        tools.end("chunk over").unwrap();
    }
    let (header, lines) = watch::read_file(&path).unwrap();
    let live = Host::resume(&header, &lines).unwrap();
    let mut tools = Tools::resume(live, Writer::append(&path).unwrap(), &lines)
        .with_scripts(Limits::bundled())
        .with_trial(trial_config().with_max_tests(FEW));
    let r = tools.call("test_governor", json!({"turns": 1})).unwrap();
    assert_eq!(r["tests_left"], 0);
    assert!(tools.call("test_governor", json!({"turns": 1})).is_err());
    std::fs::remove_file(&path).ok();
}

/// One session against a log file: the same orders either way, with `with_tests` adding
/// `test_governor` calls (on both train maps, a stopping governor and a working one) in between.
/// Returns the finished log's path.
fn play_session(name: &str, with_tests: bool) -> std::path::PathBuf {
    let path = temp_path(name);
    let mut tools = Tools::new(host(LIVE_SEED), Writer::create(&path).unwrap())
        .unwrap()
        .with_scripts(Limits::bundled())
        .with_trial(trial_config());
    let cursor = json!({"steps": [{"step": "cursor", "x": 5, "y": 5}], "note": "scout"});
    tools.call("save_script", json!({"name": "governor", "code": WORKS})).unwrap();
    tools.call("act", cursor).unwrap();
    tools.call("end_turn", json!({"turns": 2})).unwrap();
    if with_tests {
        tools.call("test_governor", json!({"turns": 8})).unwrap();
        tools.call("save_script", json!({"name": "other", "code": ERRORS})).unwrap();
    } else {
        tools.call("save_script", json!({"name": "other", "code": ERRORS})).unwrap();
    }
    tools.call("run_script", json!({"name": "governor", "turns": 3})).unwrap();
    if with_tests {
        tools.call("test_governor", json!({"seed": OTHER_SEED, "turns": 8})).unwrap();
    }
    tools.call("end_turn", json!({"turns": 1})).unwrap();
    tools.end("done").unwrap();
    path
}

/// The live game is untouched: with and without `test_governor` calls, the log holds the same
/// lines (bar the `call` lines for the tool itself), replays to the same world, and scores the
/// same.
#[test]
fn a_log_with_tests_in_it_replays_and_scores_like_one_without() {
    let with = play_session("with.jsonl", true);
    let without = play_session("without.jsonl", false);
    let (header_a, lines_a) = watch::read_file(&with).unwrap();
    let (header_b, lines_b) = watch::read_file(&without).unwrap();
    std::fs::remove_file(&with).ok();
    std::fs::remove_file(&without).ok();

    let is_test_call = |l: &LogLine| matches!(l, LogLine::Call { tool, .. } if tool == trial::TOOL);
    assert_eq!(lines_a.iter().filter(|l| is_test_call(l)).count(), 2);
    let rest_a: Vec<&LogLine> = lines_a.iter().filter(|l| !is_test_call(l)).collect();
    let rest_b: Vec<&LogLine> = lines_b.iter().collect();
    assert_eq!(rest_a, rest_b, "the tests left nothing in the log but their own call lines");

    // Same world after a replay.
    let observe = |h: &RunLogHeader, l: &[LogLine]| {
        serde_json::to_string(&Host::resume(h, l).unwrap().observe()).unwrap()
    };
    assert_eq!(observe(&header_a, &lines_a), observe(&header_b, &lines_b));

    // Same score.
    let score =
        |h: &RunLogHeader, l: &[LogLine]| scorecard::build(h, l, None).unwrap().replay.card.score;
    assert_eq!(score(&header_a, &lines_a), score(&header_b, &lines_b));
    assert!(lines_a.iter().any(|l| matches!(l, LogLine::Turn { .. })));
}

use fn_tui::surface::RunLogHeader;

/// A governor that logs every turn and is refused every turn (`<space>` is a key the host
/// won't press).
const REFUSED: &str = r#"loop { log("tick"); keys("<space>"); wait(1); }"#;

fn text(r: &Value, key: &str) -> String {
    r[key].as_str().unwrap_or_else(|| panic!("no {key} in {r}")).to_owned()
}

/// #570: the result says what the clean evaluation would say and enough about the run to see why
/// it went the way it did: signs with turns, deaths, and a trace by slice. The summary is short;
/// `detail` adds rows, refusals and the governor's own log lines, and stays under its size.
#[test]
fn the_result_has_the_scorecard_and_a_short_trace_and_detail_adds_to_it() {
    let mut tools = tools_in_memory();
    save(&mut tools, REFUSED);
    let short = tools.call("test_governor", json!({"seed": OTHER_SEED})).unwrap();
    assert_eq!(short["deaths"], "none");
    assert!(short["care"].as_f64().unwrap() <= 1.0 && short["progress"].is_number(), "{short}");
    let signs = text(&short, "signs");
    assert!(signs.contains("Food in stores") && signs.contains("Population"), "{signs}");
    assert!(signs.contains("never started") || signs.contains('/'), "{signs}");
    let trace = text(&short, "trace");
    // 24 turns: 12 slices of 2 turns, one column each.
    let row = |name: &str| -> Vec<String> {
        let line = trace.lines().find(|l| l.starts_with(name)).unwrap_or_else(|| panic!("{trace}"));
        line.split_whitespace().skip(1).map(str::to_owned).collect()
    };
    assert_eq!(row("turn").len(), 12, "{trace}");
    assert_eq!(row("turn").last().unwrap(), "24", "{trace}");
    assert_eq!(row("people").len(), 12, "{trace}");
    assert_eq!(row("idle%").len(), 12);
    assert_eq!(row("hungry").len(), 12);
    assert_eq!(row("thirsty").len(), 12);
    let refused: u32 = row("refused").iter().map(|c| c.parse::<u32>().unwrap()).sum();
    assert_eq!(refused, 25, "one refused key at each of turns 0 to 24: {trace}");
    assert!(trace.contains("refused 25x") && trace.contains("<space>"), "{trace}");
    assert!(!trace.contains("tick"), "log lines are the detail's: {trace}");
    assert!(short.to_string().len() < SUMMARY_BYTES, "{} bytes", short.to_string().len());

    let long = tools.call("test_governor", json!({"seed": OTHER_SEED, "detail": true})).unwrap();
    // The scorecard half is the same; the trace has more.
    for key in ["score", "signs", "deaths", "stopped", "progress", "care"] {
        assert_eq!(long[key], short[key], "{key}");
    }
    let trace = text(&long, "trace");
    for row in ["food", "tired", "hurt", "deaths"] {
        assert!(trace.lines().any(|l| l.starts_with(row)), "{row} missing: {trace}");
    }
    assert!(trace.contains("25x tick"), "the governor's own log lines, counted: {trace}");
    assert!(long.to_string().len() < DETAIL_BYTES, "{} bytes", long.to_string().len());
    assert!(long.to_string().len() > short.to_string().len());
}

/// The bound holds for a run that says a lot: many distinct long log lines and refusals are
/// cut to a few short ones.
#[test]
fn a_noisy_governor_cannot_make_the_result_long() {
    let mut tools = tools_in_memory();
    let noisy = r#"
        let n = 0;
        loop {
            n += 1;
            log("an extremely long line that goes on and on to fill up a result " + n
                + " and on and on and on and on and on and on and on and on and on");
            keys(if n % 4 == 0 { "<space>" } else if n % 4 == 1 { "<f5>" } else { "<f6>" });
            wait(1);
        }
    "#;
    save(&mut tools, noisy);
    let r = tools.call("test_governor", json!({"detail": true})).unwrap();
    assert!(r.to_string().len() < DETAIL_BYTES, "{} bytes: {r}", r.to_string().len());
    let trace = text(&r, "trace");
    assert!(trace.lines().all(|l| l.chars().count() < 220), "{trace}");
}

/// A stopped governor still gets the trace of the run it coasted through.
#[test]
fn a_stopped_run_has_a_trace_too() {
    let mut tools = tools_in_memory();
    save(&mut tools, ERRORS);
    let r = tools.call("test_governor", json!({"detail": true})).unwrap();
    assert_eq!(r["stopped"]["reason"], "error");
    assert!(text(&r, "trace").contains("people"), "{r}");
}

/// A held-out seed is refused, and the refusal says nothing about held-out seeds: not that there
/// are any, and not which. The result names only the seed it was asked about.
#[test]
fn the_tool_knows_nothing_of_held_out_seeds() {
    let mut tools = tools_in_memory();
    save(&mut tools, WORKS);
    let err = tools.call("test_governor", json!({"seed": HELD_OUT})).unwrap_err();
    assert!(!err.contains("held") && !err.contains("hold"), "{err}");
    let r = tools.call("test_governor", json!({"seed": OTHER_SEED, "detail": true})).unwrap();
    assert_eq!(r["seed"], OTHER_SEED);
    assert!(!r.to_string().contains("held"), "{r}");
}

/// Seed 2 punishes a settlement with no orders: it comes through two winters on the wild food
/// around it and loses people in the third (turns 495-540), with no stores laid by. (Seed 42 did
/// it before turn 100 until #582: two settlers spawned in sealed pockets of trees and starved
/// beside the stores. That was a spawn bug, now fixed.) The result names them as the alert did
/// (who, of what, on which turn), and the detail's deaths row agrees.
#[test]
fn deaths_come_with_their_cause_and_turn() {
    const DEATH_SEED: u64 = 2;
    // The small map on Normal, the track's own start: seed 2 only dies there.
    let nation = fn_tui::cli::parse_nation("water").unwrap();
    let live =
        Host::new(LIVE_SEED, nation, Some("small".into()), Some("normal".into()), None, false)
            .unwrap()
            .with_turn_seconds(DEFAULT_TURN_SECONDS);
    let mut tools = Tools::new(live, Writer::new(Vec::new()))
        .unwrap()
        .with_scripts(Limits::bundled())
        .with_trial(Trial::new(vec![LIVE_SEED, DEATH_SEED], 540).unwrap());
    save(&mut tools, WORKS);
    let r = tools.call("test_governor", json!({"seed": DEATH_SEED, "detail": true})).unwrap();
    let deaths = text(&r, "deaths");
    let (count, named) = deaths.split_once(": ").unwrap_or_else(|| panic!("no deaths: {deaths}"));
    let count: u32 = count.parse().unwrap();
    assert!(count >= 1, "{deaths}");
    let first = named.split("; ").next().unwrap();
    let (cause, turn) =
        first.rsplit_once(" t").map(|(a, b)| (a, b.parse::<u32>().unwrap())).unwrap();
    assert!(cause.ends_with("starvation") || cause.ends_with("dehydration"), "{deaths}");
    assert!((1..=540).contains(&turn), "{deaths}");
    let trace = text(&r, "trace");
    let died: u32 = trace
        .lines()
        .find(|l| l.starts_with("deaths"))
        .unwrap()
        .split_whitespace()
        .skip(1)
        .map(|c| c.parse::<u32>().unwrap())
        .sum();
    assert_eq!(died, count, "{trace}");
    assert!(r["care"].as_f64().unwrap() < 1.0, "{r}");
}

/// Seed 42 used to lose someone to a spawn pocket before turn 100 (#582); now nobody dies there
/// in that time, and the result says so.
#[test]
fn no_deaths_reads_none() {
    let mut tools = Tools::new(host(LIVE_SEED), Writer::new(Vec::new()))
        .unwrap()
        .with_scripts(Limits::bundled())
        .with_trial(Trial::new(vec![LIVE_SEED, OTHER_SEED], 100).unwrap());
    save(&mut tools, WORKS);
    let r = tools.call("test_governor", json!({"seed": OTHER_SEED, "detail": true})).unwrap();
    assert_eq!(text(&r, "deaths"), "none", "{r}");
}

/// Same governor, plan and seed: the same bytes, the detail form too (no clock, no thread RNG).
#[test]
fn the_result_is_deterministic() {
    let run = || {
        let mut tools = tools_in_memory();
        save(&mut tools, REFUSED);
        let r = tools.call("test_governor", json!({"seed": OTHER_SEED, "detail": true})).unwrap();
        r.to_string()
    };
    assert_eq!(run(), run());
}

//! `fn_agent serve --horizon N` (#569, `Tools::with_horizon`): live play stops at the horizon.
//!
//! What these pin: no tool plays a turn past it; a `run_script` that runs into it ends with
//! reason `horizon` (never an error, a runaway or a budget) and says so plainly, and one started
//! at it isn't run at all; `end_turn` at it answers with the observation and the same note;
//! `test_governor`, `save_script`, `plan` and `notes` keep working; the session gets a bounded
//! number of calls to finish up, across a `--resume` too; and a log made with the stop replays
//! and scores exactly as the same game stopped by hand.

use fn_agent::runlog::Writer;
use fn_agent::scorecard;
use fn_agent::script::Limits;
use fn_agent::tools::{HORIZON_GRACE_CALLS, Tools};
use fn_agent::trial::Trial;
use fn_tui::surface::{DEFAULT_TURN_SECONDS, Host, LogLine};
use fn_tui::watch;
use serde_json::{Value, json};

const SEED: u64 = 7;
/// Short enough for an unoptimised build; a campaign's is 540.
const HORIZON: u32 = 6;

const LOOPS: &str = "loop { wait(1); }";

fn host() -> Host {
    let nation = fn_tui::cli::parse_nation("water").unwrap();
    Host::new(SEED, nation, None, None, None, false)
        .unwrap()
        .with_turn_seconds(DEFAULT_TURN_SECONDS)
}

fn tools() -> Tools<Vec<u8>> {
    Tools::new(host(), Writer::new(Vec::new()))
        .unwrap()
        .with_horizon(HORIZON)
        .with_scripts(Limits::bundled())
        .with_trial(Trial::new(vec![SEED], 4).unwrap())
}

fn save(tools: &mut Tools<Vec<u8>>, code: &str) {
    tools.call("save_script", json!({"name": "governor", "code": code})).unwrap();
}

fn turn_lines(tools: &Tools<Vec<u8>>) -> usize {
    String::from_utf8_lossy(tools.log().get_ref())
        .lines()
        .filter(|l| l.contains("\"type\":\"turn\""))
        .count()
}

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("fn_agent_horizon_test_{}_{name}", std::process::id()))
}

#[test]
fn end_turn_stops_at_the_horizon_and_says_so() {
    let mut t = tools();
    let r = t.call("end_turn", json!({"turns": 2})).unwrap();
    assert_eq!(r["turn"], 2);
    assert!(r.get("horizon").is_none(), "nothing to say before it: {r}");

    let r = t.call("end_turn", json!({"turns": 100})).unwrap();
    assert_eq!(r["turn"], HORIZON, "only the turns left were played: {r}");
    let note = r["horizon"].as_str().unwrap();
    assert!(note.contains("reached its horizon") && note.contains("turn 6"), "{note}");
    assert!(note.contains("test_governor") && note.contains("save_script"), "{note}");

    // At the horizon a further call is an answer, not an error, and plays nothing.
    let r = t.call("end_turn", json!({"turns": 5})).unwrap();
    assert_eq!(r["turn"], HORIZON);
    assert!(r["horizon"].as_str().unwrap().contains("no more turns will be played"), "{r}");
    assert_eq!(turn_lines(&t), HORIZON as usize);
}

#[test]
fn a_governor_running_into_the_horizon_ends_with_reason_horizon() {
    let mut t = tools();
    save(&mut t, LOOPS);
    let r = t.call("run_script", json!({"name": "governor", "turns": 4})).unwrap();
    assert_eq!(r["reason"], "budget", "an ordinary budget stop far from it: {r}");
    assert!(r.get("horizon").is_none());

    // 15 is the most a run may ask for; only 2 turns are left.
    let r = t.call("run_script", json!({"name": "governor", "turns": 15})).unwrap();
    assert_eq!(r["reason"], "horizon", "{r}");
    assert_eq!(r["turns_run"], 2);
    assert_eq!(r["observe"]["turn"], HORIZON);
    let why = r["why"].as_str().unwrap();
    assert!(why.contains("reached its horizon") && why.contains("calls left"), "{why}");
    assert_ne!(r["reason"], "error");
    assert_ne!(r["reason"], "runaway");
}

#[test]
fn a_run_started_at_the_horizon_is_answered_without_running() {
    let mut t = tools();
    t.call("end_turn", json!({"turns": 100})).unwrap();
    save(&mut t, "act([#{step: \"cursor\", x: 5, y: 5}], \"late\"); loop { wait(1); }");
    let before = String::from_utf8_lossy(t.log().get_ref()).lines().count();
    let r = t.call("run_script", json!({"name": "governor", "turns": 5})).unwrap();
    assert_eq!(r["reason"], "horizon", "{r}");
    assert_eq!(r["turns_run"], 0);
    assert!(r["why"].as_str().unwrap().contains("reached its horizon"), "{r}");
    let after: Vec<String> = String::from_utf8_lossy(t.log().get_ref())
        .lines()
        .skip(before)
        .map(str::to_owned)
        .collect();
    assert_eq!(after.len(), 1, "only the call line: {after:?}");
    assert!(after[0].contains("\"type\":\"call\""), "{after:?}");

    // A read-only probe is still fine: looking at the finished settlement costs no turn.
    let r = t.call("run_script", json!({"code": "observe().turn"})).unwrap();
    assert_eq!(r["reason"], "done", "{r}");
    assert_eq!(r["value"], HORIZON);
}

#[test]
fn a_run_that_ends_for_another_reason_at_the_horizon_keeps_it_and_adds_the_note() {
    let mut t = tools();
    t.call("end_turn", json!({"turns": 5})).unwrap();
    save(&mut t, "wait(1); let x = 1 / 0;");
    let r = t.call("run_script", json!({"name": "governor"})).unwrap();
    assert_eq!(r["reason"], "error", "its own error is still an error: {r}");
    assert!(r["horizon"].as_str().unwrap().contains("reached its horizon"), "{r}");
}

#[test]
fn test_governor_save_script_plan_and_notes_work_after_the_horizon() {
    let mut t = tools();
    t.call("end_turn", json!({"turns": 100})).unwrap();
    save(&mut t, LOOPS);
    let r = t.call("test_governor", json!({"turns": 3})).unwrap();
    assert_eq!(r["turns"], 3, "{r}");
    assert_eq!(r["stopped"], Value::Null, "{r}");
    t.call("plan", json!({"set": {"goal": "late"}})).unwrap();
    t.call("notes", json!({"append": "learned late"})).unwrap();
    let r = t.call("scripts", json!({"name": "governor"})).unwrap();
    assert!(r.to_string().contains("loop"), "{r}");
}

#[test]
fn the_session_gets_a_bounded_number_of_calls_after_the_horizon() {
    let mut t = tools();
    t.call("end_turn", json!({"turns": 100})).unwrap();
    for n in 1..=HORIZON_GRACE_CALLS {
        t.call("observe", json!({})).unwrap_or_else(|e| panic!("call {n} was refused: {e}"));
    }
    let err = t.call("save_script", json!({"name": "governor", "code": LOOPS})).unwrap_err();
    assert!(err.contains("stop now") && err.contains("short summary"), "{err}");
    assert!(t.call("observe", json!({})).is_err());
}

/// A chunk that ends at the horizon and is resumed (a crashed session, say) doesn't get the
/// grace over again: the log's own `call` lines since the last turn are counted.
#[test]
fn a_resumed_session_does_not_get_the_grace_again() {
    let path = temp_path("resume.jsonl");
    {
        let mut t = Tools::new(host(), Writer::create(&path).unwrap())
            .unwrap()
            .with_horizon(HORIZON)
            .with_scripts(Limits::bundled());
        t.call("end_turn", json!({"turns": 100})).unwrap();
        for _ in 0..HORIZON_GRACE_CALLS - 1 {
            t.call("observe", json!({})).unwrap();
        }
        t.end("chunk over").unwrap();
    }
    let (header, lines) = watch::read_file(&path).unwrap();
    let live = Host::resume(&header, &lines).unwrap();
    let mut t = Tools::resume(live, Writer::append(&path).unwrap(), &lines)
        .with_horizon(HORIZON)
        .with_scripts(Limits::bundled());
    t.call("observe", json!({})).unwrap();
    assert!(t.call("observe", json!({})).is_err());
    std::fs::remove_file(&path).ok();
}

/// Not at the horizon yet, a resume starts the count at zero.
#[test]
fn a_resume_before_the_horizon_counts_nothing() {
    let path = temp_path("early.jsonl");
    {
        let mut t = Tools::new(host(), Writer::create(&path).unwrap()).unwrap();
        for _ in 0..HORIZON_GRACE_CALLS + 5 {
            t.call("observe", json!({})).unwrap();
        }
        t.end("chunk over").unwrap();
    }
    let (header, lines) = watch::read_file(&path).unwrap();
    let live = Host::resume(&header, &lines).unwrap();
    let mut t = Tools::resume(live, Writer::append(&path).unwrap(), &lines).with_horizon(HORIZON);
    t.call("end_turn", json!({"turns": 100})).unwrap();
    for _ in 0..HORIZON_GRACE_CALLS {
        t.call("observe", json!({})).unwrap();
    }
    std::fs::remove_file(&path).ok();
}

/// One session against a log: the horizon stop reached by a governor and an over-long `end_turn`
/// (`stop`), or the same game stopped by hand at the horizon with no stop in the tools.
fn play(name: &str, stop: bool) -> std::path::PathBuf {
    let path = temp_path(name);
    let mut t =
        Tools::new(host(), Writer::create(&path).unwrap()).unwrap().with_scripts(Limits::bundled());
    if stop {
        t = t.with_horizon(HORIZON);
    }
    let turns = |n: u32| if stop { n + 50 } else { n };
    t.call("save_script", json!({"name": "governor", "code": LOOPS})).unwrap();
    t.call("run_script", json!({"name": "governor", "turns": 3})).unwrap();
    if stop {
        // Past the horizon on purpose: the stop clamps this, the by-hand game asks for 3.
        t.call("run_script", json!({"name": "governor", "turns": 15})).unwrap();
        t.call("end_turn", json!({"turns": turns(1)})).unwrap();
        t.call("run_script", json!({"name": "governor", "turns": 15})).unwrap();
    } else {
        t.call("run_script", json!({"name": "governor", "turns": HORIZON - 3})).unwrap();
    }
    t.end("done").unwrap();
    path
}

#[test]
fn a_log_made_with_the_stop_scores_like_the_same_game_stopped_by_hand() {
    let with = play("with.jsonl", true);
    let without = play("without.jsonl", false);
    let (header_a, lines_a) = watch::read_file(&with).unwrap();
    let (header_b, lines_b) = watch::read_file(&without).unwrap();
    std::fs::remove_file(&with).ok();
    std::fs::remove_file(&without).ok();

    let turns = |l: &[LogLine]| l.iter().filter(|x| matches!(x, LogLine::Turn { .. })).count();
    assert_eq!((turns(&lines_a), turns(&lines_b)), (HORIZON as usize, HORIZON as usize));
    let score = |h: &fn_tui::surface::RunLogHeader, l: &[LogLine]| {
        scorecard::build(h, l, Some(HORIZON)).unwrap().replay.card.score
    };
    assert_eq!(score(&header_a, &lines_a), score(&header_b, &lines_b));
    // The world the log replays to is the same one.
    let observe = |h: &fn_tui::surface::RunLogHeader, l: &[LogLine]| {
        serde_json::to_string(&Host::resume(h, l).unwrap().observe()).unwrap()
    };
    assert_eq!(observe(&header_a, &lines_a), observe(&header_b, &lines_b));
}

/// Without `--horizon` nothing changes: a run goes on as far as it is asked.
#[test]
fn no_horizon_no_stop() {
    let mut t =
        Tools::new(host(), Writer::new(Vec::new())).unwrap().with_scripts(Limits::bundled());
    let r = t.call("end_turn", json!({"turns": HORIZON + 3})).unwrap();
    assert_eq!(r["turn"], HORIZON + 3);
    assert!(r.get("horizon").is_none());
    let briefing = t.call("briefing", json!({})).unwrap();
    assert!(briefing.get("horizon").is_none());
}

#[test]
fn the_briefing_names_the_horizon() {
    let mut t = tools();
    let briefing = t.call("briefing", json!({})).unwrap();
    assert!(briefing["horizon"].as_str().unwrap().contains("turn 6"), "{briefing}");
}

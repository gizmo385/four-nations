//! `fn_agent score LOG` end-to-end (#525, docs/design/agent-hillclimb.md, "The scorecard"):
//! the scorecard a replay finds is the one `Scorecard::compute` gives the live world, the
//! horizon cuts a replay off where it says, `--append` adds a line every reader ignores, and a
//! log made against different data is refused.

use std::process::{Command, Stdio};

use fn_agent::scorecard::{self, Churn};
use fn_agent::script::{Limits, Request, run};
use fn_tui::surface::referee::{self, score::ScoreDef};
use fn_tui::surface::{EventKind, Host, LogLine, RunLogHeader};
use fn_tui::watch;
use serde_json::{Value, json};

const SETTLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/settle.rhai");
const SEED: u64 = 7;

fn fn_agent() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fn_agent"));
    cmd.stdin(Stdio::null());
    cmd
}

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("fn_agent_scorecard_test_{}_{name}", std::process::id()))
}

fn limits() -> Limits {
    Limits {
        default_turns: 15,
        max_turns: 180,
        max_operations: 2_000_000,
        log_tail: 30,
        default_wake: vec![EventKind::Death],
        memory_bytes: 8192,
        plan_bytes: 8192,
    }
}

/// A live host after `turns` turns of `settle.rhai`, run the way `fn_agent play` runs a script,
/// with the log lines a `play` run would have written (a `Script` line, the keys and turns, a
/// `Wake` and an `End`).
fn played(turns: u32) -> (Host, RunLogHeader, Vec<LogLine>) {
    let nation = fn_tui::cli::parse_nation("earth").unwrap();
    let mut host = Host::new(SEED, nation, None, None, None, false).unwrap();
    let code = std::fs::read_to_string(SETTLE).unwrap();
    let outcome = run(
        &mut host,
        Request {
            code: &code,
            turns: Some(turns),
            note: Some("play"),
            version: 0,
            wake_on: &[],
            memory: &json!({}),
            plan: &json!({}),
            probe: false,
        },
        &limits(),
    );
    let mut lines =
        vec![LogLine::Script { name: "play".into(), version: 0, code, turns, wake_on: Vec::new() }];
    lines.extend(host.log().lines());
    lines.push(LogLine::Wake {
        reason: outcome.reason.to_owned(),
        why: outcome.why.clone(),
        turn: outcome.observe.turn,
        memory: outcome.memory.clone(),
    });
    lines.push(LogLine::End { tick: host.tick(), reason: "script finished".into() });
    let header = host.header();
    (host, header, lines)
}

fn turn_lines(lines: &[LogLine]) -> usize {
    lines.iter().filter(|l| matches!(l, LogLine::Turn { .. })).count()
}

/// The core promise: replaying a scripted `play` log and scoring it gives exactly the numbers
/// `Scorecard::compute` gives the live world the script played.
#[test]
fn a_replayed_play_log_scores_the_same_as_the_live_world() {
    let (host, header, lines) = played(20);
    assert!(turn_lines(&lines) >= 10, "the fixture should play some turns");

    let live = referee::live(&host, &ScoreDef::bundled());
    let report = scorecard::build(&header, &lines, None).unwrap();
    assert_eq!(report.replay.card, live);
    assert!(!report.replay.card.past_horizon);
    assert_eq!(report.replay.horizon_turns as usize, turn_lines(&lines));
    assert_eq!(report.replay.played_turns, report.replay.horizon_turns);
}

/// A horizon short of the log's end replays only that far: the same card as the log cut off at
/// that turn, and never `past_horizon`. A horizon past the end scores what the run reached.
#[test]
fn the_horizon_cuts_the_replay_where_it_says() {
    let (_, header, lines) = played(20);
    let total = turn_lines(&lines) as u32;
    let cut = total / 2;

    let at_cut = scorecard::build(&header, &lines, Some(cut)).unwrap();
    assert_eq!(at_cut.replay.played_turns, cut);
    assert_eq!(at_cut.replay.horizon_turns, cut);
    assert!(!at_cut.replay.card.past_horizon);
    assert!(at_cut.replay.lines_used < lines.len());

    // The same run, truncated by hand at that turn, scores identically.
    let mut seen = 0;
    let truncated: Vec<LogLine> = lines
        .iter()
        .take_while(|l| {
            if matches!(l, LogLine::Turn { .. }) {
                seen += 1;
            }
            seen <= cut
        })
        .cloned()
        .collect();
    let by_hand = scorecard::build(&header, &truncated, None).unwrap();
    assert_eq!(by_hand.replay.card, at_cut.replay.card);
    assert_eq!(by_hand.churn, Churn::of(&truncated));

    // Past the end: stopped early, scored against the full horizon, not past it.
    let past = scorecard::build(&header, &lines, Some(total + 50)).unwrap();
    assert_eq!(past.replay.played_turns, total);
    assert_eq!(past.replay.horizon_turns, total + 50);
    assert!(!past.replay.card.past_horizon);
    assert!(past.text().contains("stopped early"));
}

#[test]
fn a_log_made_against_different_data_is_refused_with_the_digest_message() {
    let (_, mut header, lines) = played(5);
    header.data_digest ^= 1;
    let err = scorecard::build(&header, &lines, None).unwrap_err();
    assert!(err.contains("made against different data"), "{err}");
}

#[test]
fn the_text_has_the_docs_sections_and_the_json_has_the_same_fields() {
    let (_, header, lines) = played(20);
    let report = scorecard::build(&header, &lines, None).unwrap();
    let text = report.text();
    for section in ["horizon", "Played", "SCORE", "PROGRESS", "CARE", "DIAGNOSTICS"] {
        assert!(text.contains(section), "text lacks {section}:\n{text}");
    }
    for part in ["reported only:", "food per head entering winter", "idle share by 15 min"] {
        assert!(text.contains(part), "text lacks {part}:\n{text}");
    }
    assert!(text.contains("research idle from"));
    assert!(text.contains("one-shot"));
    assert!(text.contains("plan edits"));

    let json = report.json();
    let card = &report.replay.card;
    assert_eq!(json["score"], json!(card.score));
    assert_eq!(json["progress"], json!(card.progress));
    assert_eq!(json["care"], json!(card.care.value));
    assert_eq!(json["par"], Value::Null, "par stays empty until the balance harness fills it");
    assert_eq!(json["horizon"], json!(report.replay.horizon_turns));
    assert_eq!(
        json["progress_detail"]["milestones"].as_array().unwrap().len(),
        card.milestones.len()
    );
    for key in ["deaths", "close_calls", "suffering", "reported"] {
        assert!(json["care_detail"].get(key).is_some(), "care_detail lacks {key}");
    }
    for key in ["idle_share", "research_idle", "scripts"] {
        assert!(json["diagnostics"].get(key).is_some(), "diagnostics lacks {key}");
    }
    for key in ["names", "one_shot_names", "governor_share", "plan_edits", "scripts"] {
        assert!(json["diagnostics"]["scripts"].get(key).is_some(), "scripts lacks {key}");
    }
}

/// None of the scorecard's values are in `observe`: no field of an observation names a score,
/// care tally or diagnostic, and no tool the surface offers does either.
#[test]
fn nothing_the_scorecard_reports_appears_in_observe() {
    let (host, _, _) = played(10);
    let observed = serde_json::to_value(host.observe()).unwrap();
    let mut keys = Vec::new();
    collect_keys(&observed, &mut keys);
    let banned = [
        "score",
        "scorecard",
        "care",
        "par",
        "past_horizon",
        "close_calls",
        "suffering",
        "idle_share",
        "winter_food",
        "harm_events",
        "credit",
        "churn",
        "horizon",
    ];
    for key in &keys {
        assert!(!banned.contains(&key.as_str()), "observe carries {key:?}");
    }
    let text = observed.to_string();
    assert!(!text.contains("SCORE") && !text.contains("food per head entering winter"));
    assert!(
        fn_agent::tools::list().iter().all(|t| !t["name"].as_str().unwrap().contains("score")),
        "no tool scores a run"
    );
}

fn collect_keys(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                out.push(k.clone());
                collect_keys(v, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_keys(v, out)),
        _ => {}
    }
}

/// `fn_agent score` the real binary, on a log `fn_agent play` made: `--json` matches the library,
/// and `--append` adds a `score` line that replay, `keys`, a resumed session, the watcher and a
/// second scoring all ignore -- the final state is unchanged.
#[test]
fn the_binary_scores_a_play_log_and_append_leaves_the_replay_untouched() {
    let script = temp_path("script.rhai");
    std::fs::write(&script, std::fs::read_to_string(SETTLE).unwrap()).unwrap();
    let log = temp_path("play.jsonl");
    let out = fn_agent()
        .args(["play", "--script"])
        .arg(&script)
        .args(["--seed", "7", "--nation", "earth", "--turns", "12", "--wake-on", "death"])
        .arg("--log")
        .arg(&log)
        .output()
        .unwrap();
    std::fs::remove_file(&script).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let (header, lines) = watch::read_file(&log).unwrap();
    let want = scorecard::build(&header, &lines, None).unwrap();

    let json_out = fn_agent().arg("score").arg(&log).arg("--json").output().unwrap();
    assert!(json_out.status.success(), "{}", String::from_utf8_lossy(&json_out.stderr));
    let json: Value = serde_json::from_slice(&json_out.stdout).unwrap();
    assert_eq!(json, want.json());

    let text_out = fn_agent().arg("score").arg(&log).args(["--horizon", "6"]).output().unwrap();
    assert!(text_out.status.success());
    let text = String::from_utf8(text_out.stdout).unwrap();
    assert!(text.contains("horizon 6 turns (2:00)"), "{text}");
    assert!(text.contains("Played 6 turns"), "{text}");

    let keys_before = fn_agent().arg("keys").arg(&log).output().unwrap().stdout;
    let resumed_before = Host::resume(&header, &lines).unwrap();
    let digest_before = (resumed_before.tick(), resumed_before.log().clone());

    // --append: one more line, a `score`, after everything else.
    let appended =
        fn_agent().arg("score").arg(&log).arg("--append").stdout(Stdio::null()).status().unwrap();
    assert!(appended.success());
    let (header_after, lines_after) = watch::read_file(&log).unwrap();
    assert_eq!(header_after, header);
    assert_eq!(lines_after.len(), lines.len() + 1);
    assert_eq!(&lines_after[..lines.len()], &lines[..]);
    let Some(LogLine::Score { score, progress, care, horizon, attempt, champion, build }) =
        lines_after.last().cloned()
    else {
        panic!("the last line should be the score: {:?}", lines_after.last());
    };
    assert_eq!(score, want.replay.card.score);
    assert_eq!(progress, want.replay.card.progress);
    assert_eq!(care, want.replay.card.care.value);
    assert_eq!(horizon, want.replay.horizon_turns);
    assert_eq!((attempt, champion), (None, None));
    assert_eq!(build.as_deref(), Some(fn_tui::buildid::CURRENT), "the score line says who scored");

    // Everything that reads the log ignores the new line.
    let keys_after = fn_agent().arg("keys").arg(&log).output().unwrap().stdout;
    assert_eq!(keys_after, keys_before);
    assert_eq!(
        watch::to_keys_script(&header_after, &lines_after),
        watch::to_keys_script(&header, &lines)
    );
    let resumed_after = Host::resume(&header_after, &lines_after).unwrap();
    assert_eq!((resumed_after.tick(), resumed_after.log().clone()), digest_before);
    let rescored = scorecard::build(&header_after, &lines_after, None).unwrap();
    assert_eq!(rescored.replay.card, want.replay.card);
    assert_eq!(rescored.churn, want.churn);

    let mut before = watch::Watcher::new(&header).unwrap();
    before.feed(lines.iter().cloned());
    before.fast_forward_to_turn(u32::MAX);
    let mut after = watch::Watcher::new(&header_after).unwrap();
    after.feed(lines_after.iter().cloned());
    after.fast_forward_to_turn(u32::MAX);
    assert_eq!(after.turn(), before.turn());
    assert_eq!(after.finished(), before.finished());

    std::fs::remove_file(&log).ok();
}

#[test]
fn the_binary_refuses_a_log_made_against_different_data() {
    let (_, mut header, lines) = played(3);
    header.data_digest ^= 1;
    let log = temp_path("stale.jsonl");
    let mut text = serde_json::to_string(&LogLine::Header(header)).unwrap();
    for line in &lines {
        text.push('\n');
        text.push_str(&serde_json::to_string(line).unwrap());
    }
    text.push('\n');
    std::fs::write(&log, text).unwrap();

    let out = fn_agent().arg("score").arg(&log).output().unwrap();
    std::fs::remove_file(&log).ok();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("made against different data"), "{err}");
}

#[test]
fn bad_arguments_are_usage_errors() {
    for args in
        [vec!["score"], vec!["score", "x.jsonl", "--horizon"], vec!["score", "x", "--bogus"]]
    {
        let out = fn_agent().args(&args).output().unwrap();
        assert!(!out.status.success(), "{args:?} should fail");
    }
}

/// A scenario start (#481) puts turn 0 at the end of its prelude (#548): the replayed card and
/// the live host's card agree on where that is, and a sign the prelude already met earns full
/// credit.
#[test]
fn a_scenario_start_scores_from_the_end_of_its_prelude() {
    let host = Host::from_scenario("earth-year-two").unwrap();
    let header = host.header();
    assert!(header.prelude.is_some(), "the scenario has a prelude");
    let start = host.tick();
    assert!(start > 0);

    let live = referee::live(&host, &ScoreDef::bundled());
    assert_eq!(live.start_tick, start);
    let report = scorecard::build(&header, &[], Some(0)).unwrap();
    assert_eq!(report.replay.card, live);
    assert_eq!(report.replay.start_tick, start);
    assert_eq!(report.replay.card.start_tick, start);
    let met = |r: &&referee::score::MilestoneRow| r.first_met.is_some_and(|t| t <= start);
    let early: Vec<_> = report.replay.card.milestones.iter().filter(met).collect();
    assert!(!early.is_empty(), "a year of prelude meets some sign");
    for row in early {
        assert!((row.credit - 1.0).abs() < 1e-9, "{} was met in the prelude: {row:?}", row.id);
    }
}

/// A clean evaluation (`play --horizon`, #528) is the governor played alone, and its scorecard
/// reaches the next attempt's prompt (#565): it says the governor ran every turn, not "no
/// governor", and doesn't call the one whole-run script a one-shot or an inline run.
#[test]
fn a_clean_runs_scorecard_describes_the_governor_that_ran() {
    let script = temp_path("clean-script.rhai");
    std::fs::write(&script, std::fs::read_to_string(SETTLE).unwrap()).unwrap();
    let log = temp_path("clean.jsonl");
    let out = fn_agent()
        .args(["play", "--script"])
        .arg(&script)
        .args(["--seed", "7", "--nation", "earth", "--horizon", "12"])
        .arg("--log")
        .arg(&log)
        .output()
        .unwrap();
    std::fs::remove_file(&script).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let text_out = fn_agent().arg("score").arg(&log).args(["--horizon", "12"]).output().unwrap();
    assert!(text_out.status.success());
    let text = String::from_utf8(text_out.stdout).unwrap();
    assert!(text.contains("governor ran 12 of 12 turns (100%)"), "{text}");
    assert!(text.contains("inline runs 0"), "{text}");
    assert!(text.contains("0 one-shot"), "{text}");
    assert!(!text.contains("no governor"), "{text}");
    std::fs::remove_file(&log).ok();
}

/// The run browser's score column (#556) is `fn_agent score LOG --horizon 540`: the same number,
/// signs and deaths, read off a run folder whose `run.jsonl` the browser never writes to.
#[test]
fn the_run_browsers_score_is_the_one_fn_agent_score_prints() {
    let script = temp_path("browser-script.rhai");
    std::fs::write(&script, std::fs::read_to_string(SETTLE).unwrap()).unwrap();
    let dir = temp_path("browser-run");
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("run.jsonl");
    let out = fn_agent()
        .args(["play", "--script"])
        .arg(&script)
        .args(["--seed", "7", "--nation", "earth", "--turns", "12", "--wake-on", "death"])
        .arg("--log")
        .arg(&log)
        .output()
        .unwrap();
    std::fs::remove_file(&script).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let before = std::fs::read(&log).unwrap();

    let json_out =
        fn_agent().arg("score").arg(&log).args(["--horizon", "540", "--json"]).output().unwrap();
    assert!(json_out.status.success(), "{}", String::from_utf8_lossy(&json_out.stderr));
    let want: Value = serde_json::from_slice(&json_out.stdout).unwrap();

    let (_, got) = fn_tui::runs::score_run(&dir);
    let got = got.unwrap();
    assert_eq!(got.score, want["score"].as_f64().unwrap());
    assert_eq!(u64::from(got.deaths), want["care_detail"]["deaths"].as_u64().unwrap());
    let signs: Vec<&Value> = want["progress_detail"]["milestones"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["kind"] == "sign")
        .collect();
    assert_eq!(got.signs_total as usize, signs.len());
    let met = signs.iter().filter(|m| m["met_now"] == true && !m["met_turn"].is_null()).count();
    assert_eq!(got.signs_met as usize, met);

    assert_eq!(
        std::fs::read(&log).unwrap(),
        before,
        "scoring for the browser never writes the log"
    );
    std::fs::remove_dir_all(&dir).ok();
}

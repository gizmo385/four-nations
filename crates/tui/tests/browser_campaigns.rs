//! Campaigns in the run browser (#585): a campaign folder is a row, its attempts sit beneath it,
//! and an attempt's clean evaluation logs can be opened. Fixture folders only (tiny real games,
//! ledger files written by hand in both shapes), `contains` assertions and layout properties, and
//! no golden files. Nothing here opens a terminal.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

use fn_sim::Nation;
use fn_tui::browser::{Browser, Outcome, Pool, Scoring};
use fn_tui::color::ColorDepth;
use fn_tui::frame::to_text;
use fn_tui::keys;
use fn_tui::runs::campaign::{self, Activity, Campaign, State, Tier};
use fn_tui::surface::{Host, LogLine};

const DAY: Duration = Duration::from_secs(86_400);

fn fresh_dir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fn_tui_campaign_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A log of `turns` turns of a real (tiny) game on `seed`.
fn game_log(seed: u64, turns: u32) -> String {
    let mut h =
        Host::new(seed, Nation::Water, None, None, None, false).unwrap().with_turn_seconds(20);
    if turns > 0 {
        h.press_keys("h <right*2> <enter> <down> <enter>", None, None).unwrap();
    }
    for _ in 0..turns {
        h.end_turn(1);
    }
    h.log().to_jsonl(&h.header())
}

fn end_line() -> String {
    let line = LogLine::End { tick: 400, reason: "agent finished".to_owned() };
    format!("{}\n", serde_json::to_string(&line).unwrap())
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A campaign in the seed-tier shape (#571), as `xtask agent-run --track` writes it:
///   attempt 1: 2 live turns, kept; the chosen pair is the second (`v3-2`), with gate and
///              owner-only logs, beside an unchosen first pair;
///   attempt 2: no ledger row yet, no live turns (a header and a `test_governor` call), its
///              session over (`stop.json`) and one pair being evaluated.
fn tiered_campaign(runs: &Path, name: &str) -> PathBuf {
    let dir = runs.join(name);
    write(
        &dir.join("ledger.json"),
        r#"{
          "track": {"id":"water-3h","name":"Water, three hours","nation":"water","difficulty":"normal",
                    "map_size":"small","site":"default","prelude":null,"horizon_turns":540,
                    "play_seed":7633,"screen_seeds":[7633,7],"gate_seeds":[101,102],
                    "owner_seeds":[201],"collapse_below":120.0},
          "rows": [
            {"attempt":1,"governor_version":3,"plan_revision":2,"played":210.5,
             "screen":[{"seed":7633,"score":300.0},{"seed":7,"score":250.0}],"screen_mean":275.0,
             "gate":[{"seed":101,"score":240.0},{"seed":102,"score":260.0,"stopped":"runaway","stopped_turn":112}],
             "gate_mean":250.0,
             "owner":[{"seed":201,"score":222.0}],
             "kept":true,"cost_usd":0.5,"build":"c1ddaff","saved":3,"tried":2}
          ]
        }"#,
    );
    let a1 = dir.join("attempt-1");
    write(&a1.join("run.jsonl"), &(game_log(7633, 2) + &end_line()));
    write(&a1.join("cost.json"), r#"{"usd":0.5}"#);
    write(&a1.join("stop.json"), r#"{"chunk":2,"reason":"done","turns":2}"#);
    write(
        &a1.join("eval/versions.json"),
        r#"{"chosen":3,"pairs":[
              {"version":2,"chosen":false,"screen_mean":100.0},
              {"version":3,"chosen":true,"screen_mean":275.0}]}"#,
    );
    let small = game_log(7, 1);
    for (pair, seeds) in [("v2-1", vec![7633, 7]), ("v3-2", vec![7633, 7, 101, 102, 201])] {
        for seed in seeds {
            write(&a1.join(format!("eval/{pair}/seed-{seed}.jsonl")), &small);
        }
    }

    let a2 = dir.join("attempt-2");
    write(
        &a2.join("run.jsonl"),
        &format!(
            "{}{{\"type\":\"call\",\"tool\":\"test_governor\",\"summary\":\"seed 7\"}}\n",
            game_log(7633, 0)
        ),
    );
    write(&a2.join("cost.json"), r#"{"usd":0.25}"#);
    write(&a2.join("stop.json"), r#"{"chunk":4,"reason":"no_progress","turns":0}"#);
    write(&a2.join("eval/v4-1/seed-7633.jsonl"), &small);
    dir
}

/// A campaign in the first shape (train and held-out seeds), as the older folders in
/// `~/fn-agent-runs` are: loose `eval/seed-N.jsonl`, no `versions.json`, no governor version.
fn old_campaign(runs: &Path, name: &str) -> PathBuf {
    let dir = runs.join(name);
    write(
        &dir.join("ledger.json"),
        r#"{
          "track": {"id":"water-3h","name":"Water","nation":"water","difficulty":"normal",
                    "horizon_turns":540,"play_seed":7633,"train_seeds":[7633,7],"holdout_seeds":[1]},
          "rows": [
            {"attempt":1,"governor_version":16,"plan_revision":8,"played":355.3,
             "clean":[{"seed":7633,"score":101.4,"stopped":"runaway"},{"seed":7,"score":103.3}],
             "train_mean":102.3,
             "held_out":[{"seed":1,"score":96.7,"stopped":"runaway","stopped_turn":229}],
             "kept":true,"cost_usd":1.3}
          ]
        }"#,
    );
    let a1 = dir.join("attempt-1");
    write(&a1.join("run.jsonl"), &(game_log(7633, 2) + &end_line()));
    let small = game_log(7, 1);
    for seed in [7633, 7, 1] {
        write(&a1.join(format!("eval/seed-{seed}.jsonl")), &small);
    }
    dir
}

fn open_at(dir: &Path, now: SystemTime) -> Browser {
    Browser::open(dir, now, Scoring::Inline, ColorDepth::Never, false)
}

fn frame(b: &mut Browser, w: u16, h: u16) -> String {
    to_text(&b.render((w, h)))
}

fn press(b: &mut Browser, script: &str) {
    b.play(&keys::parse(script).unwrap());
}

fn line_with<'a>(frame: &'a str, text: &str) -> &'a str {
    frame
        .lines()
        .find(|l| l.contains(text))
        .unwrap_or_else(|| panic!("no row with {text}:\n{frame}"))
}

const NEW: &str = "20261108-225225-campaign-water-3h";
const OLD: &str = "20261108-154451-campaign-water-3h";

// ---- the data ----

#[test]
fn a_campaign_reads_in_the_seed_tier_shape_with_tiers_and_the_chosen_pair() {
    let runs = fresh_dir();
    let dir = tiered_campaign(&runs, NEW);
    let now = SystemTime::now();
    let c = Campaign::read(&dir, now, None);

    assert_eq!(c.track.id.as_deref(), Some("water-3h"));
    assert_eq!(c.attempts.len(), 2);
    assert_eq!(c.kept_of(), (1, 2));
    assert_eq!(c.best(), Some(275.0), "the champion's clean mean");
    assert_eq!(c.cost_usd(), Some(0.75), "the ledger's cost, then cost.json for the open attempt");

    let a1 = &c.attempts[0];
    assert_eq!(a1.chosen.as_deref(), Some("v3-2"));
    let labels: Vec<&str> = a1.games.iter().map(|g| g.label.as_str()).collect();
    assert_eq!(
        labels,
        ["v3-2/seed-7633", "v3-2/seed-7", "v3-2/seed-101", "v3-2/seed-102", "v3-2/seed-201"],
        "the chosen pair only, screen then gate then owner-only"
    );
    let tiers: Vec<Option<Tier>> = a1.games.iter().map(|g| g.tier).collect();
    assert_eq!(
        tiers,
        [
            Some(Tier::Screen),
            Some(Tier::Screen),
            Some(Tier::Gate),
            Some(Tier::Gate),
            Some(Tier::Owner)
        ]
    );
    assert_eq!(a1.games[4].score, Some(222.0), "owner-only scores show: this is the owner's view");
    assert_eq!(a1.games[3].stopped.as_deref(), Some("runaway"));

    // The attempt with no row yet lists every pair it has, and has no scores for them.
    let a2 = &c.attempts[1];
    assert!(a2.row.is_none() && a2.chosen.is_none());
    assert_eq!(a2.games.len(), 1);
    assert_eq!(a2.games[0].score, None);
    assert_eq!(a2.live_turns(), 0);
}

#[test]
fn a_campaign_in_the_first_shape_reads_with_train_and_held_out_tiers() {
    let runs = fresh_dir();
    let dir = old_campaign(&runs, OLD);
    let c = Campaign::read(&dir, SystemTime::now(), None);
    assert_eq!(c.best(), Some(102.3), "train_mean is the clean mean");
    let a1 = &c.attempts[0];
    let got: Vec<(&str, Option<Tier>, Option<f64>)> =
        a1.games.iter().map(|g| (g.label.as_str(), g.tier, g.score)).collect();
    assert_eq!(
        got,
        [
            ("seed-7633", Some(Tier::Train), Some(101.4)),
            ("seed-7", Some(Tier::Train), Some(103.3)),
            ("seed-1", Some(Tier::HeldOut), Some(96.7)),
        ]
    );
    assert_eq!(a1.games[0].stopped.as_deref(), Some("runaway"));
    assert_eq!(a1.games[2].stopped_turn, Some(229));
}

#[test]
fn what_the_folder_shows_of_a_running_campaign_is_its_attempt_and_activity() {
    let runs = fresh_dir();
    let dir = tiered_campaign(&runs, NEW);
    let now = SystemTime::now();
    let c = Campaign::read(&dir, now, None);

    // Attempt 2 has no row, its session is over and a clean log exists: evaluating.
    assert_eq!(c.state(now), State::Live(2, Some(Activity::Evaluating)));
    // Without the stop file or any clean log, its last call being `test_governor` is testing.
    fs::remove_file(dir.join("attempt-2/stop.json")).unwrap();
    fs::remove_dir_all(dir.join("attempt-2/eval")).unwrap();
    let c = Campaign::read(&dir, now, None);
    assert_eq!(c.state(now), State::Live(2, Some(Activity::Testing)));
    // A turn after that is playing.
    let mut log = fs::read_to_string(dir.join("attempt-2/run.jsonl")).unwrap();
    log.push_str("{\"type\":\"turn\",\"index\":1,\"tick\":200}\n");
    fs::write(dir.join("attempt-2/run.jsonl"), log).unwrap();
    let c = Campaign::read(&dir, now, None);
    assert_eq!(c.state(now), State::Live(2, Some(Activity::Playing)));

    // Left alone for a day, an unfinished campaign has stopped; one whose attempts all have rows
    // has finished.
    assert_eq!(c.state(now + DAY), State::Stopped);
    let old = Campaign::read(&old_campaign(&runs, OLD), now, None);
    assert_eq!(old.state(now + DAY), State::Finished);
}

#[test]
fn missing_and_damaged_files_are_gaps_and_the_attempts_still_list() {
    let runs = fresh_dir();
    let dir = runs.join("20261108-100000-campaign-water-3h");
    write(&dir.join("ledger.json"), "{ not json");
    fs::create_dir_all(dir.join("attempt-1")).unwrap();
    write(&dir.join("attempt-2/run.jsonl"), "");
    write(&dir.join("attempt-3/eval/versions.json"), "[]");
    write(&dir.join("attempt-3/eval/seed-9.jsonl"), "");
    let c = Campaign::read(&dir, SystemTime::now(), None);
    assert!(c.problem.as_deref().unwrap().contains("ledger.json"));
    assert_eq!(c.attempts.len(), 3);
    assert_eq!(c.best(), None);
    assert_eq!(c.cost_usd(), None);

    // Every row still draws, with dashes, and none of it panics.
    let mut b = open_at(&runs, SystemTime::now() + DAY);
    press(&mut b, "l j l j j l");
    let f = frame(&mut b, 160, 24);
    assert!(line_with(&f, "campaign-water-3h").contains("bad ledger"), "{f}");
    for k in 1..=3 {
        assert!(f.contains(&format!("attempt-{k}")), "attempt {k} should have a row:\n{f}");
    }
    assert!(f.contains("seed-9"), "an empty clean log is still listed:\n{f}");
    // And it says why it will not open.
    press(&mut b, "j <enter>");
    assert!(!b.is_watching());
    assert!(frame(&mut b, 160, 24).contains("the log is empty"));
}

// ---- the browser ----

#[test]
fn the_list_shows_a_campaign_with_its_attributes_and_opens_it_to_the_attempts() {
    let runs = fresh_dir();
    tiered_campaign(&runs, NEW);
    // Nothing is running a day later, so the campaign starts folded.
    let mut b = open_at(&runs, SystemTime::now() + DAY);
    let f = frame(&mut b, 200, 24);
    let row = line_with(&f, NEW);
    for want in ["water-3h", "campaign", "1 of 2", "275.0", "$0.75", "stopped early", "water"] {
        assert!(row.contains(want), "{want} in {row:?}");
    }
    assert!(row.starts_with("> + "), "folded, and selected: {row:?}");
    assert!(!f.contains("attempt-1"), "folded:\n{f}");
    for col in ["CLEAN", "KEPT", "SCORE@540"] {
        assert!(f.lines().nth(1).unwrap().contains(col), "{col}\n{f}");
    }

    press(&mut b, "l");
    let f = frame(&mut b, 200, 24);
    assert!(line_with(&f, NEW).starts_with("> - "), "open:\n{f}");
    let a1 = line_with(&f, "attempt-1");
    for want in ["kept", "275.0", "210.5", "$0.50", "v3", "by turn"] {
        assert!(a1.contains(want), "{want} in {a1:?}");
    }
    let a2 = line_with(&f, "attempt-2");
    assert!(a2.contains("no live turns") && a2.contains("$0.25") && a2.contains(" - "), "{a2:?}");
    assert!(f.find("attempt-1").unwrap() < f.find("attempt-2").unwrap(), "oldest first:\n{f}");

    // The status line tells the story of the selected row.
    assert!(
        f.contains("campaign on water-3h, 2 attempt(s) so far, 1 kept, best clean mean 275.0"),
        "{f}"
    );
    press(&mut b, "j");
    let f = frame(&mut b, 200, 24);
    assert!(
        f.contains("attempt 1 of") && f.contains("clean mean 275.0") && f.contains("gate 250.0"),
        "{f}"
    );

    // h folds the campaign again from an attempt: first up to the campaign, then closed.
    press(&mut b, "h");
    assert_eq!(b.selected_name(), Some(NEW));
    press(&mut b, "h");
    assert!(!frame(&mut b, 200, 24).contains("attempt-1"));
    // Enter on a campaign opens and folds it too.
    press(&mut b, "<enter>");
    assert!(frame(&mut b, 200, 24).contains("attempt-1"));
    press(&mut b, "<space>");
    assert!(!frame(&mut b, 200, 24).contains("attempt-1"));
}

#[test]
fn a_running_campaign_opens_expanded_and_says_what_it_is_doing() {
    let runs = fresh_dir();
    tiered_campaign(&runs, NEW);
    let mut b = open_at(&runs, SystemTime::now());
    let f = frame(&mut b, 200, 24);
    assert!(line_with(&f, NEW).contains("live: a2 evaluating"), "{f}");
    assert!(f.contains("attempt-1") && f.contains("attempt-2"), "opened for the owner:\n{f}");
    assert!(
        line_with(&f, "attempt-2").contains("evaluating"),
        "the open attempt says so too:\n{f}"
    );
    // Folding it stays folded through a rescan.
    press(&mut b, "h");
    b.refresh(SystemTime::now());
    assert!(!frame(&mut b, 200, 24).contains("attempt-1"));
}

#[test]
fn enter_on_an_attempt_with_live_turns_watches_it_and_q_comes_back_to_it() {
    let runs = fresh_dir();
    tiered_campaign(&runs, NEW);
    let mut b = open_at(&runs, SystemTime::now() + DAY);
    press(&mut b, "l j");
    assert_eq!(b.selected_name(), Some("attempt-1"));
    press(&mut b, "<enter>");
    assert!(b.is_watching());
    let f = frame(&mut b, 120, 40);
    assert!(f.contains("AGENT water") && f.contains("turn 2"), "{f}");
    press(&mut b, "q");
    assert!(!b.is_watching());
    assert_eq!(b.selected_name(), Some("attempt-1"), "back on the attempt");
}

#[test]
fn an_attempt_with_no_live_turns_says_so_and_offers_its_clean_runs() {
    let runs = fresh_dir();
    tiered_campaign(&runs, NEW);
    let mut b = open_at(&runs, SystemTime::now() + DAY);
    press(&mut b, "l j j");
    assert_eq!(b.selected_name(), Some("attempt-2"));
    press(&mut b, "<enter>");
    assert!(!b.is_watching(), "there is no game to watch");
    let f = frame(&mut b, 200, 24);
    assert!(f.contains("played 0 live turns") && f.contains("test_governor"), "{f}");
    assert!(f.contains("its clean runs are listed below"), "{f}");
    assert!(f.contains("v4-1/seed-7633"), "the clean run is under it:\n{f}");
}

#[test]
fn the_chosen_versions_clean_runs_open_and_wear_their_tier() {
    let runs = fresh_dir();
    tiered_campaign(&runs, NEW);
    let mut b = open_at(&runs, SystemTime::now() + DAY);
    press(&mut b, "l j l");
    let f = frame(&mut b, 200, 30);
    for (label, tier) in
        [("v3-2/seed-7633", "screen"), ("v3-2/seed-101", "gate"), ("v3-2/seed-201", "owner")]
    {
        let row = line_with(&f, label);
        assert!(row.contains(tier), "{label} is {tier}: {row:?}");
    }
    assert!(!f.contains("v2-1/"), "the unchosen pair is not listed:\n{f}");
    assert!(
        line_with(&f, "v3-2/seed-201").contains("222.0"),
        "the ledger's score, no replay:\n{f}"
    );
    assert!(line_with(&f, "v3-2/seed-102").contains("stopped: runaway t112"), "{f}");

    // The nested rows move and fold like the rest, and Enter opens the log itself.
    press(&mut b, "j");
    assert_eq!(b.selected_name(), Some("seed-7633.jsonl"));
    assert!(frame(&mut b, 200, 30).contains("clean run v3-2/seed-7633 (screen, seed 7633)"));
    press(&mut b, "<enter>");
    assert!(b.is_watching());
    assert!(frame(&mut b, 120, 40).contains("AGENT water"));
    press(&mut b, "q");
    assert_eq!(b.selected_name(), Some("seed-7633.jsonl"));
    // h goes up to the attempt, and folds its logs away with another.
    press(&mut b, "h");
    assert_eq!(b.selected_name(), Some("attempt-1"));
    press(&mut b, "h");
    assert!(!frame(&mut b, 200, 30).contains("v3-2/seed-7633"));
}

#[test]
fn older_campaigns_show_loose_clean_runs_with_train_and_held_out_tiers() {
    let runs = fresh_dir();
    old_campaign(&runs, OLD);
    let mut b = open_at(&runs, SystemTime::now() + DAY);
    press(&mut b, "l j l");
    let f = frame(&mut b, 200, 24);
    assert!(line_with(&f, OLD).contains("finished"), "{f}");
    assert!(
        line_with(&f, "attempt-1").contains("102.3") && line_with(&f, "attempt-1").contains("v16"),
        "{f}"
    );
    assert!(line_with(&f, "  seed-7633").contains("train"), "{f}");
    assert!(line_with(&f, "  seed-1 ").contains("held-out"), "{f}");
    assert!(line_with(&f, "  seed-1 ").contains("stopped: runaway t229"), "{f}");
}

#[test]
fn clean_logs_the_ledger_has_no_score_for_are_replayed_in_the_background_and_cached_beside_them() {
    let runs = fresh_dir();
    let dir = tiered_campaign(&runs, NEW);
    let mut b = Browser::open(
        &runs,
        SystemTime::now(),
        Scoring::Background(Pool::new(2)),
        ColorDepth::Never,
        false,
    );
    // The first frame is there at once, and the unscored log says `...`; the ledger's scores
    // need no replay.
    press(&mut b, "j j");
    assert_eq!(b.selected_name(), Some("attempt-2"));
    press(&mut b, "l");
    let f = frame(&mut b, 200, 24);
    assert!(line_with(&f, "v4-1/seed-7633").contains("..."), "{f}");
    assert!(b.scoring_pending());
    b.wait_for_scores(Duration::from_secs(120));
    assert!(!b.scoring_pending());
    let f = frame(&mut b, 200, 24);
    assert!(!line_with(&f, "v4-1/seed-7633").contains("..."), "{f}");
    let eval = dir.join("attempt-2/eval/v4-1");
    assert!(eval.join("seed-7633.score-cache.json").is_file());
    // The ledger-scored logs of attempt 1 were never replayed (it was never expanded).
    let any_cache = fs::read_dir(dir.join("attempt-1/eval/v3-2"))
        .unwrap()
        .any(|e| e.unwrap().file_name().to_string_lossy().contains("score-cache"));
    assert!(!any_cache);
}

#[test]
fn focus_starts_on_a_campaign_an_attempt_or_a_log_with_the_rows_above_it_open() {
    let runs = fresh_dir();
    let dir = tiered_campaign(&runs, NEW);
    tiered_campaign(&runs, "20261107-225225-campaign-water-3h");
    let now = SystemTime::now() + DAY;

    let mut b = open_at(&runs, now);
    b.focus(&dir);
    assert_eq!(b.selected_name(), Some(NEW));
    assert!(frame(&mut b, 200, 24).contains("attempt-2"), "the campaign is open");

    let mut b = open_at(&runs, now);
    b.focus(&dir.join("attempt-1"));
    assert_eq!(b.selected_name(), Some("attempt-1"));

    let mut b = open_at(&runs, now);
    b.focus(&dir.join("attempt-1/eval/v3-2/seed-101.jsonl"));
    assert_eq!(b.selected_name(), Some("seed-101.jsonl"));
    assert!(frame(&mut b, 200, 30).contains("v3-2/seed-101"));

    // Somewhere else entirely: a notice, and the list as it was.
    let mut b = open_at(&runs, now);
    b.focus(Path::new("/nonexistent/elsewhere"));
    assert!(frame(&mut b, 200, 24).contains("isn't in"));
    assert_eq!(b.selected_name(), Some(NEW));
}

#[test]
fn outcome_open_names_a_log_file_for_a_clean_run() {
    let runs = fresh_dir();
    let dir = tiered_campaign(&runs, NEW);
    let mut b = open_at(&runs, SystemTime::now() + DAY);
    b.focus(&dir.join("attempt-1/eval/v3-2/seed-7.jsonl"));
    let keys::Step::Key(enter) = keys::parse("<enter>").unwrap()[0] else { panic!() };
    assert_eq!(b.key(enter), Outcome::Open(dir.join("attempt-1/eval/v3-2/seed-7.jsonl")));
    assert_eq!(
        fn_tui::browser::log_of(&dir.join("attempt-1")),
        dir.join("attempt-1/run.jsonl"),
        "a folder opens its run.jsonl, a file opens as it is"
    );
}

// ---- ordering ----

/// The folder's own modification time moves when a cache file is written into it; the order may
/// not follow it. Here the oldest runs have the newest folders, as `score-cache.json` once made
/// them.
#[test]
fn runs_and_campaigns_sort_on_the_date_in_their_name_whatever_the_folders_mtime() {
    let runs = fresh_dir();
    let names = [NEW, OLD, "20261107-002038-earth-7-normal", "20261104-224847-earth-7-normal"];
    tiered_campaign(&runs, NEW);
    old_campaign(&runs, OLD);
    for name in &names[2..] {
        write(&runs.join(name).join("run.jsonl"), &(game_log(7, 1) + &end_line()));
    }
    // The oldest name gets the newest folder time.
    let set_mtimes = |newest_first: bool| {
        let base = SystemTime::now() - 10 * DAY;
        for (n, name) in names.iter().enumerate() {
            let hours = if newest_first { n } else { names.len() - n } as u64;
            let when = base + Duration::from_secs(3600 * hours);
            fs::File::open(runs.join(name)).unwrap().set_modified(when).unwrap();
        }
    };
    let positions = |b: &mut Browser| -> Vec<usize> {
        let f = frame(b, 200, 30);
        names.iter().map(|n| f.find(n).unwrap_or_else(|| panic!("{n}:\n{f}"))).collect()
    };

    set_mtimes(false);
    let mut b = open_at(&runs, SystemTime::now() + DAY);
    let at = positions(&mut b);
    assert!(at.windows(2).all(|w| w[0] < w[1]), "newest name first:\n{}", frame(&mut b, 200, 30));

    // Opening the browser replayed the plain runs and wrote score-cache.json into their
    // folders, which moved their modification times. Reverse every folder's time again and
    // open it afresh: the order is the same.
    assert!(runs.join(names[3]).join("score-cache.json").is_file());
    set_mtimes(true);
    let mut again = open_at(&runs, SystemTime::now() + DAY);
    let at = positions(&mut again);
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{}", frame(&mut again, 200, 30));
    assert_eq!(b.selected_name(), Some(NEW));
    assert_eq!(again.selected_name(), Some(NEW));
}

// ---- layout ----

#[test]
fn every_width_draws_whole_lines_and_keeps_the_selection_on_screen() {
    let runs = fresh_dir();
    tiered_campaign(&runs, NEW);
    old_campaign(&runs, OLD);
    let now = SystemTime::now() + DAY;
    for (w, h) in [(24, 8), (40, 10), (80, 14), (120, 20), (200, 30), (60, 4)] {
        let mut b = open_at(&runs, now);
        press(&mut b, "l j l j");
        let f = frame(&mut b, w, h);
        assert_eq!(f.lines().count(), usize::from(h), "{w}x{h}");
        for line in f.lines() {
            assert!(line.chars().count() <= usize::from(w), "{w}x{h}: {line:?}");
        }
        if h >= 5 {
            assert!(f.lines().any(|l| l.starts_with("> ")), "{w}x{h}: the selection shows:\n{f}");
        }
    }
}

// ---- the binary ----

fn fn_tui() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fn_tui"));
    cmd.stdin(Stdio::null()).env_remove("NO_COLOR").env_remove("COLORTERM");
    cmd
}

#[test]
fn frame_takes_a_focus_and_opens_the_campaign_on_it() {
    let runs = fresh_dir();
    let dir = tiered_campaign(&runs, NEW);
    let out = fn_tui()
        .arg("--runs")
        .arg(&runs)
        .arg("--focus")
        .arg(&dir)
        .args(["--frame", "--size", "180x20", "--keys", "j"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("attempt-1") && text.contains("attempt-2"), "{text}");
    assert!(line_with(&text, "attempt-1").starts_with("> "), "j moved off the campaign:\n{text}");

    let out = fn_tui().arg("--runs").arg(&runs).args(["--frame", "--focus"]).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("--focus needs a value"));
    assert!(campaign::is_campaign(&dir));
}

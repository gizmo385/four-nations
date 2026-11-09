//! Campaign folders as data, for the run browser (#585, `crate::browser`): what
//! `cargo xtask agent-run --track ID --attempts N` leaves under `$FN_AGENT_RUNS`, read into one
//! [`Campaign`] with an [`Attempt`] per `attempt-K/` folder and a [`Game`] per clean evaluation log
//! it can open.
//!
//! ```text
//! CAMPAIGN/
//!   ledger.json         the track, and a row per finished attempt
//!   champion/           the best strategy so far
//!   attempt-K/          a normal run folder (run.jsonl, cost.json, stop.json...), the live game
//!     eval/versions.json     which governor/plan pair was chosen (absent while evaluating)
//!     eval/vV-N/seed-S.jsonl the clean evaluation logs of pair N (governor version V)
//!     eval/seed-S.jsonl      the same, in the layout from before pairs
//! ```
//!
//! Two ledger shapes exist and both are read: the first one's rows carry `clean`, `train_mean` and
//! `held_out`; the seed tiers (#571) replaced them with `screen`, `gate`, `owner` (each with a
//! mean) and a `verdict`. Every field is optional here, a row that does not parse is skipped and
//! a missing file is a gap, never an error: a half-written campaign still lists.
//!
//! Nothing here writes, and nothing here opens a log: the attempts' `run.jsonl` are read by
//! [`RunInfo::read`] only when they have changed since the last read ([`Campaign::read`]'s
//! `prev`), and the clean logs are only listed.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Deserialize;

use super::{RunInfo, stamp_of};

/// `ledger.json`, the file that makes a folder a campaign.
pub const LEDGER: &str = "ledger.json";

/// A campaign whose newest attempt has written something this recently is running.
pub const LIVE_WITHIN: Duration = Duration::from_secs(300);

/// ... or this recently, when every attempt it has already has its row: the next one starts at
/// once, so a quiet gap this long means it is over.
pub const BETWEEN_ATTEMPTS: Duration = Duration::from_secs(60);

/// Whether `dir` is a campaign folder: no log of its own, and a ledger or attempt folders.
pub fn is_campaign(dir: &Path) -> bool {
    !dir.join("run.jsonl").is_file()
        && (dir.join(LEDGER).is_file() || !attempt_numbers(dir).is_empty())
}

/// The `K` of every `attempt-K` folder in `dir`, ascending.
fn attempt_numbers(dir: &Path) -> Vec<u32> {
    let mut v: Vec<u32> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_string_lossy().strip_prefix("attempt-")?.parse().ok())
        .collect();
    v.sort_unstable();
    v
}

// -------------------------------------------------------------------------------------------
// The ledger
// -------------------------------------------------------------------------------------------

/// One seed's clean score.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct SeedScore {
    pub seed: u64,
    pub score: f64,
    /// Why the governor stopped giving orders, if it did (`runaway`).
    pub stopped: Option<String>,
    pub stopped_turn: Option<u32>,
}

/// How a candidate compared with the champion on the gate seeds (#571).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Verdict {
    pub champion_attempt: Option<u32>,
    pub diff: f64,
    pub lo: f64,
    pub hi: f64,
    pub promoted: bool,
}

impl Verdict {
    /// `+31 [+8, +55]`: the gain and its interval, in whole points.
    pub fn text(&self) -> String {
        format!("{:+.0} [{:+.0}, {:+.0}]", self.diff, self.lo, self.hi)
    }
}

/// An attempt's ledger row, in either shape.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LedgerRow {
    pub attempt: u32,
    pub governor_version: Option<u32>,
    /// The attempt's own live game, scored at the horizon.
    pub played: Option<f64>,
    // The first shape.
    pub clean: Vec<SeedScore>,
    pub train_mean: Option<f64>,
    pub held_out: Vec<SeedScore>,
    // The seed tiers.
    pub screen: Vec<SeedScore>,
    pub screen_mean: Option<f64>,
    pub gate: Vec<SeedScore>,
    pub gate_mean: Option<f64>,
    pub verdict: Option<Verdict>,
    pub owner: Vec<SeedScore>,
    pub kept: bool,
    pub cost_usd: Option<f64>,
}

impl LedgerRow {
    /// The mean clean score the attempt was judged on first: the screen mean, or the train mean
    /// of the first shape.
    pub fn clean_mean(&self) -> Option<f64> {
        self.screen_mean.or(self.train_mean)
    }

    /// The clean score of `seed` anywhere in the row.
    pub fn score_of(&self, seed: u64) -> Option<&SeedScore> {
        [&self.clean, &self.screen, &self.gate, &self.held_out, &self.owner]
            .into_iter()
            .flatten()
            .find(|s| s.seed == seed)
    }
}

/// The seeds of a track, by tier. The first shape has `train` and `held_out`; the seed tiers
/// have `screen`, `gate` and `owner`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Track {
    pub id: Option<String>,
    pub name: Option<String>,
    pub nation: Option<String>,
    pub difficulty: Option<String>,
    pub horizon_turns: Option<u32>,
    pub play_seed: Option<u64>,
    pub screen_seeds: Vec<u64>,
    pub gate_seeds: Vec<u64>,
    pub owner_seeds: Vec<u64>,
    pub train_seeds: Vec<u64>,
    #[serde(rename = "holdout_seeds")]
    pub held_out_seeds: Vec<u64>,
}

impl Track {
    /// Which tier `seed` is in, and its place in that tier's list.
    pub fn tier_of(&self, seed: u64) -> Option<(Tier, usize)> {
        [
            (Tier::Screen, &self.screen_seeds),
            (Tier::Train, &self.train_seeds),
            (Tier::Gate, &self.gate_seeds),
            (Tier::HeldOut, &self.held_out_seeds),
            (Tier::Owner, &self.owner_seeds),
        ]
        .into_iter()
        .find_map(|(tier, seeds)| Some((tier, seeds.iter().position(|s| *s == seed)?)))
    }
}

/// A group of seeds a clean evaluation is run on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Screen,
    Train,
    Gate,
    HeldOut,
    Owner,
}

impl Tier {
    pub fn word(self) -> &'static str {
        match self {
            Tier::Screen => "screen",
            Tier::Train => "train",
            Tier::Gate => "gate",
            Tier::HeldOut => "held-out",
            Tier::Owner => "owner",
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawLedger {
    track: Track,
    rows: Vec<serde_json::Value>,
}

// -------------------------------------------------------------------------------------------
// Attempts and their games
// -------------------------------------------------------------------------------------------

/// What an unfinished attempt is doing, as far as its folder shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    /// The model is playing the live game, or writing its governor.
    Playing,
    /// Its last call was `test_governor`.
    Testing,
    /// The session is over (`stop.json`) and the governors are being cleanly evaluated.
    Evaluating,
}

impl Activity {
    pub fn word(self) -> &'static str {
        match self {
            Activity::Playing => "playing",
            Activity::Testing => "testing",
            Activity::Evaluating => "evaluating",
        }
    }
}

/// One clean evaluation log.
#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    /// `seed-7633`, or `v5-6/seed-7633` when the attempt tried several pairs.
    pub label: String,
    pub log: PathBuf,
    pub seed: Option<u64>,
    pub tier: Option<Tier>,
    /// The score the ledger recorded for this seed, if the attempt has a row.
    pub score: Option<f64>,
    pub stopped: Option<String>,
    pub stopped_turn: Option<u32>,
    pub bytes: u64,
}

/// What `stop.json` says: why the attempt's session ended.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Stop {
    pub reason: Option<String>,
    pub detail: Option<String>,
    pub turns: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct Attempt {
    pub number: u32,
    pub dir: PathBuf,
    /// The attempt's own live game, read like any run folder's.
    pub run: RunInfo,
    pub row: Option<LedgerRow>,
    pub stop: Option<Stop>,
    pub cost_usd: Option<f64>,
    /// Which pair was chosen (`v5-6`), from `eval/versions.json`.
    pub chosen: Option<String>,
    pub games: Vec<Game>,
    /// What its `run.jsonl` looked like when `run` was read.
    seen: (u64, Option<SystemTime>),
}

impl Attempt {
    /// Live turns played.
    pub fn live_turns(&self) -> u32 {
        self.run.turns
    }

    /// The label for the chosen governor, `v5`.
    pub fn governor_text(&self) -> Option<String> {
        let v = self.row.as_ref()?.governor_version?;
        Some(format!("v{v}"))
    }

    /// Why the attempt has no live game to watch, when it has none.
    pub fn no_live_turns_why(&self) -> &'static str {
        "played 0 live turns: its calls went to test_governor, which leaves no replayable log"
    }
}

// -------------------------------------------------------------------------------------------
// The campaign
// -------------------------------------------------------------------------------------------

/// How a campaign stands at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Running: attempt `.0` is under way, doing `.1` if the folder says.
    Live(u32, Option<Activity>),
    /// Every attempt it has a folder for has its row.
    Finished,
    /// The newest attempt never got its row and nothing is being written.
    Stopped,
}

#[derive(Clone, Debug)]
pub struct Campaign {
    pub name: String,
    pub dir: PathBuf,
    pub stamp: Option<String>,
    pub track: Track,
    pub attempts: Vec<Attempt>,
    /// Why the ledger could not be read, when it could not.
    pub problem: Option<String>,
    /// The newest write the folder shows, in the newest attempt or the ledger.
    pub last_write: Option<SystemTime>,
}

impl Campaign {
    /// Reads `dir`. `prev` is the same campaign read before: an attempt whose `run.jsonl` has
    /// not changed keeps its [`RunInfo`] instead of reading a log of megabytes again.
    pub fn read(dir: &Path, now: SystemTime, prev: Option<&Campaign>) -> Campaign {
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut problem = None;
        let raw = match fs::read_to_string(dir.join(LEDGER)) {
            Ok(text) => serde_json::from_str::<RawLedger>(&text).unwrap_or_else(|e| {
                problem = Some(format!("ledger.json can't be read: {e}"));
                RawLedger::default()
            }),
            Err(_) => RawLedger::default(),
        };
        let rows: Vec<LedgerRow> =
            raw.rows.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect();

        let mut numbers = attempt_numbers(dir);
        numbers.extend(rows.iter().map(|r| r.attempt));
        numbers.sort_unstable();
        numbers.dedup();

        let attempts = numbers
            .into_iter()
            .map(|k| {
                let adir = dir.join(format!("attempt-{k}"));
                let before =
                    prev.and_then(|p| p.attempts.iter().find(|a| a.number == k && a.dir == adir));
                let row = rows.iter().find(|r| r.attempt == k).cloned();
                read_attempt(k, &adir, row, &raw.track, now, before)
            })
            .collect::<Vec<_>>();
        let mut c = Campaign {
            stamp: stamp_of(&name),
            name,
            dir: dir.to_path_buf(),
            track: raw.track,
            attempts,
            problem,
            last_write: None,
        };
        c.last_write = c.newest_write();
        c
    }

    /// The newest modification time among the ledger and what the newest attempt has written:
    /// its own files, and the clean logs under `eval/`. A shallow look, a few `stat`s.
    fn newest_write(&self) -> Option<SystemTime> {
        let mut times = vec![mtime(&self.dir.join(LEDGER))];
        if let Some(a) = self.attempts.last() {
            let eval = a.dir.join("eval");
            let mut folders = vec![a.dir.clone(), eval.clone()];
            folders.extend(sub_dirs(&eval));
            for f in folders {
                for e in fs::read_dir(&f).into_iter().flatten().filter_map(|e| e.ok()) {
                    if e.path().is_file() {
                        times.push(e.metadata().ok().and_then(|m| m.modified().ok()));
                    }
                }
            }
        }
        times.into_iter().flatten().max()
    }

    /// The attempt under way: the newest one, when it has no ledger row yet.
    pub fn open_attempt(&self) -> Option<&Attempt> {
        self.attempts.last().filter(|a| a.row.is_none())
    }

    /// What the activity of the open attempt is.
    pub fn activity(&self) -> Option<Activity> {
        let a = self.open_attempt()?;
        Some(if a.stop.is_some() || has_eval_logs(&a.dir) {
            Activity::Evaluating
        } else {
            last_tool_activity(&a.dir.join("run.jsonl"))
        })
    }

    pub fn state(&self, now: SystemTime) -> State {
        // A write dated after `now` (a clock that moved) is as fresh as it gets.
        let quiet = self.last_write.map(|t| now.duration_since(t).unwrap_or_default());
        let window = if self.open_attempt().is_some() { LIVE_WITHIN } else { BETWEEN_ATTEMPTS };
        if let (Some(last), true) = (self.attempts.last(), quiet.is_some_and(|q| q < window)) {
            let next = self.open_attempt().map_or(last.number + 1, |a| a.number);
            return State::Live(next, self.activity());
        }
        if self.open_attempt().is_some() { State::Stopped } else { State::Finished }
    }

    /// The attempt that is the champion: the newest one the ledger kept.
    pub fn champion(&self) -> Option<&Attempt> {
        self.attempts.iter().rev().find(|a| a.row.as_ref().is_some_and(|r| r.kept))
    }

    /// The champion's clean mean: the best score the campaign has.
    pub fn best(&self) -> Option<f64> {
        self.champion()?.row.as_ref()?.clean_mean()
    }

    /// How many attempts are kept, of how many there are.
    pub fn kept_of(&self) -> (usize, usize) {
        let kept = self.attempts.iter().filter(|a| a.row.as_ref().is_some_and(|r| r.kept)).count();
        (kept, self.attempts.len())
    }

    /// The attempts' cost added up, if any of them has one.
    pub fn cost_usd(&self) -> Option<f64> {
        let costs: Vec<f64> = self.attempts.iter().filter_map(|a| a.cost_usd).collect();
        (!costs.is_empty()).then(|| costs.iter().sum())
    }

    /// `2026-11-08 22:52`, from the folder name.
    pub fn started_text(&self) -> Option<String> {
        let s = self.stamp.as_deref()?;
        Some(format!("{}-{}-{} {}:{}", &s[0..4], &s[4..6], &s[6..8], &s[9..11], &s[11..13]))
    }

    /// The attempt a path belongs to, and the game it is, if it names one.
    pub fn locate(&self, path: &Path) -> Option<(usize, Option<usize>)> {
        let a = self.attempts.iter().position(|a| path.starts_with(&a.dir))?;
        let g = self.attempts[a].games.iter().position(|g| g.log == path);
        Some((a, g))
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

fn sub_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

fn json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// Whether any clean evaluation log exists yet under `attempt/eval`.
fn has_eval_logs(attempt: &Path) -> bool {
    let eval = attempt.join("eval");
    !seed_logs(&eval).is_empty() || sub_dirs(&eval).iter().any(|d| !seed_logs(d).is_empty())
}

/// What the live session is doing, from the tail of its log: a `test_governor` call is the last
/// thing it did, or it isn't.
fn last_tool_activity(log: &Path) -> Activity {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = fs::File::open(log) else { return Activity::Playing };
    let len = f.metadata().map_or(0, |m| m.len());
    let mut tail = Vec::new();
    if f.seek(SeekFrom::Start(len.saturating_sub(16 * 1024))).is_err()
        || f.read_to_end(&mut tail).is_err()
    {
        return Activity::Playing;
    }
    for line in String::from_utf8_lossy(&tail).lines().rev() {
        if line.starts_with(r#"{"type":"turn""#) {
            return Activity::Playing;
        }
        if line.starts_with(r#"{"type":"call""#) {
            return if line.contains(r#""tool":"test_governor""#) {
                Activity::Testing
            } else {
                Activity::Playing
            };
        }
    }
    Activity::Playing
}

/// `seed-N.jsonl` files in `dir`, as `(N, path)`.
fn seed_logs(dir: &Path) -> Vec<(u64, PathBuf)> {
    let mut v: Vec<(u64, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let seed = name.strip_prefix("seed-")?.strip_suffix(".jsonl")?.parse().ok()?;
            Some((seed, e.path()))
        })
        .filter(|(_, p)| p.is_file())
        .collect();
    v.sort();
    v
}

fn read_attempt(
    number: u32,
    dir: &Path,
    row: Option<LedgerRow>,
    track: &Track,
    now: SystemTime,
    before: Option<&Attempt>,
) -> Attempt {
    let meta = fs::metadata(dir.join("run.jsonl")).ok();
    let seen = (meta.as_ref().map_or(0, |m| m.len()), meta.and_then(|m| m.modified().ok()));
    let run = match before {
        // A log that is still being written is read again: whether it looks live moves with `now`.
        Some(b) if b.seen == seen && b.run.state(now) != Some(super::State::Live) => b.run.clone(),
        _ => RunInfo::read(dir, now),
    };
    let stop: Option<Stop> = json(&dir.join("stop.json"));
    let cost_usd = row.as_ref().and_then(|r| r.cost_usd).or(run.cost_usd);

    let eval = dir.join("eval");
    let versions: Option<serde_json::Value> = json(&eval.join("versions.json"));
    // The chosen pair is the one `versions.json` marks, named `vV-N` for its version and place.
    let chosen = versions.as_ref().and_then(|v| {
        let pairs = v["pairs"].as_array()?;
        let (n, p) = pairs.iter().enumerate().find(|(_, p)| p["chosen"] == true)?;
        Some(format!("v{}-{}", p["version"].as_u64()?, n + 1))
    });
    let games = list_games(&eval, chosen.as_deref(), row.as_ref(), track);
    Attempt { number, dir: dir.to_path_buf(), run, row, stop, cost_usd, chosen, games, seen }
}

/// The clean logs of an attempt: the chosen pair's, or every pair's while none is chosen yet,
/// or the loose `seed-N.jsonl` of the layout from before pairs.
fn list_games(
    eval: &Path,
    chosen: Option<&str>,
    row: Option<&LedgerRow>,
    track: &Track,
) -> Vec<Game> {
    let mut found: Vec<(String, u64, PathBuf)> =
        seed_logs(eval).into_iter().map(|(seed, p)| (format!("seed-{seed}"), seed, p)).collect();
    for d in sub_dirs(eval) {
        let name = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if chosen.is_some_and(|c| c != name) {
            continue;
        }
        found.extend(
            seed_logs(&d).into_iter().map(|(seed, p)| (format!("{name}/seed-{seed}"), seed, p)),
        );
    }
    let mut games: Vec<Game> = found
        .into_iter()
        .map(|(label, seed, log)| {
            let s = row.and_then(|r| r.score_of(seed));
            Game {
                label,
                seed: Some(seed),
                tier: track.tier_of(seed).map(|(t, _)| t),
                score: s.map(|s| s.score),
                stopped: s.and_then(|s| s.stopped.clone()),
                stopped_turn: s.and_then(|s| s.stopped_turn),
                bytes: fs::metadata(&log).map_or(0, |m| m.len()),
                log,
            }
        })
        .collect();
    // By tier (screen or train first, the owner's seeds last), then where the track lists the
    // seed, then the pair, so that a view with several pairs keeps each pair together.
    games.sort_by_key(|g| {
        let tier = g.seed.and_then(|s| track.tier_of(s));
        let pair = g.label.split_once('/').map(|(p, _)| p.to_owned());
        (pair, tier.is_none(), tier, g.seed)
    });
    games
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_of_either_ledger_shape_read_with_gaps_where_a_field_is_missing() {
        let old: LedgerRow = serde_json::from_str(
            r#"{"attempt":1,"played":361.0,"clean":[{"seed":7,"score":197.5,"stopped":"runaway","stopped_turn":112}],
                "train_mean":273.3,"held_out":[{"seed":1,"score":62.7}],"kept":true,"cost_usd":0.5}"#,
        )
        .unwrap();
        assert_eq!(old.clean_mean(), Some(273.3));
        assert_eq!(old.score_of(1).map(|s| s.score), Some(62.7));
        assert_eq!(old.score_of(7).and_then(|s| s.stopped_turn), Some(112));
        assert_eq!(old.governor_version, None);

        let new: LedgerRow = serde_json::from_str(
            r#"{"attempt":2,"governor_version":6,"screen":[{"seed":7,"score":265.0}],"screen_mean":295.0,
                "gate":[{"seed":2,"score":200.0}],"gate_mean":210.0,
                "verdict":{"champion_attempt":1,"diff":31.4,"lo":8.0,"hi":55.2,"level":0.9,"promoted":true,"reason":"x"},
                "owner":[{"seed":40,"score":300.0}],"kept":true,"cost_usd":0.3,"extra":1}"#,
        )
        .unwrap();
        assert_eq!(new.clean_mean(), Some(295.0));
        assert_eq!(new.verdict.as_ref().map(Verdict::text).as_deref(), Some("+31 [+8, +55]"));
        assert_eq!(new.score_of(40).map(|s| s.score), Some(300.0));

        let bare: LedgerRow = serde_json::from_str(r#"{"attempt":3}"#).unwrap();
        assert_eq!((bare.clean_mean(), bare.kept, bare.cost_usd), (None, false, None));
    }

    #[test]
    fn a_seed_belongs_to_the_tier_the_track_lists_it_in() {
        let t: Track = serde_json::from_str(
            r#"{"id":"x","screen_seeds":[7633,7],"gate_seeds":[101],"owner_seeds":[200],"train_seeds":[],"holdout_seeds":[1]}"#,
        )
        .unwrap();
        assert_eq!(t.tier_of(7), Some((Tier::Screen, 1)));
        assert_eq!(t.tier_of(101).map(|(t, _)| t), Some(Tier::Gate));
        assert_eq!(t.tier_of(1).map(|(t, _)| t), Some(Tier::HeldOut));
        assert_eq!(t.tier_of(200).map(|(t, _)| t), Some(Tier::Owner));
        assert_eq!(t.tier_of(99), None);
    }
}

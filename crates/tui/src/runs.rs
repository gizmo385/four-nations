//! Run folders as data, for the run browser (#556, `crate::browser`): what `agent-run` leaves
//! under `$FN_AGENT_RUNS`, read into one [`RunInfo`] per folder. No terminal in here, so the
//! metadata parsing is tested on its own.
//!
//! A run folder holds `run.jsonl` (the log: header, then one line a turn, key press, script...),
//! and, depending on how it was started, `mcp.json` (whether it played by scripts), `chunk-N.json`
//! (each `claude -p` session's result: the model, and how it ended) and `cost.json` (the USD
//! total). Everything is optional except the log, and a missing value is `None`, never an error:
//! a row with gaps is still a row.
//!
//! **Scores.** [`RunInfo`] only carries what reading the log's text gives away (a `score` line
//! `fn_agent score --append` wrote, if any). The rest of the scorecard (the score at the 540-turn
//! default horizon, signs met, deaths) takes a replay of the whole log: [`score_run`] does that
//! with `surface::referee::score`, the call `fn_agent score` itself makes, and caches the result
//! in [`CACHE_FILE`] in the run folder, keyed by the log's byte length, so a log that has grown
//! is scored again. Nothing here ever writes to `run.jsonl`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::surface::RunLogHeader;
use crate::surface::referee::{self, score::MilestoneKind, score::ScoreDef};
use crate::watch;

pub mod campaign;

/// The horizon the browser scores at: `fn_agent score`'s fixed horizon for a track
/// (`horizon_turns` in data/tracks.toml; docs/design/agent-hillclimb.md, "Scoring").
pub const SCORE_HORIZON: u32 = 540;

/// The score cache, in the run folder next to `run.jsonl`. Safe to delete.
pub const CACHE_FILE: &str = "score-cache.json";

/// A log written to within this long ago, with no end marker, is a run still being played.
pub const LIVE_WITHIN: Duration = Duration::from_secs(180);

/// How a run ended, or hasn't.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Still being written to.
    Live,
    /// The agent's session ended on its own.
    Finished,
    /// Ended without an agent finishing: an error, a kill, or a log that just stopped.
    Stopped,
}

impl State {
    pub fn word(self) -> &'static str {
        match self {
            State::Live => "live",
            State::Finished => "finished",
            State::Stopped => "stopped early",
        }
    }
}

/// How the agent played.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// By writing and running scripts (`agent-run --scripts`).
    Scripts,
    /// One action at a time.
    Turns,
}

impl Mode {
    pub fn word(self) -> &'static str {
        match self {
            Mode::Scripts => "scripts",
            Mode::Turns => "by turn",
        }
    }
}

/// Why a folder's row has nothing (or little) to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// Two words for the row's state cell.
    pub short: &'static str,
    /// The whole reason, for the status line and for opening the run.
    pub why: String,
    /// The log can't be opened in the watcher at all (no log, empty, or no header).
    pub unopenable: bool,
}

/// A `score` line `fn_agent score --append` left in the log.
#[derive(Clone, Debug, PartialEq)]
pub struct LoggedScore {
    pub score: f64,
    pub horizon: u32,
    /// The build that computed it (#566); `None` for a line from before builds were recorded.
    pub build: Option<String>,
}

impl LoggedScore {
    /// Whether another build than this one computed the score: the same log can score
    /// differently here, so the number may not be what a rescore gives. An unrecorded build
    /// counts: nothing says it was this one.
    pub fn from_another_build(&self) -> bool {
        crate::buildid::is_other(self.build.as_deref())
    }
}

/// Appended to a logged score that [`LoggedScore::from_another_build`].
pub const OTHER_BUILD_MARK: &str = "~";

/// What one run folder says about its run.
#[derive(Clone, Debug, PartialEq)]
pub struct RunInfo {
    /// The folder's name.
    pub name: String,
    pub dir: PathBuf,
    /// `YYYYMMDD-HHMMSS` from the folder name, if it has one. Sorts as time.
    pub stamp: Option<String>,
    pub nation: Option<String>,
    pub seed: Option<u64>,
    pub scenario: Option<String>,
    pub difficulty: Option<String>,
    /// The header says this was a playtest (#576): the model was asked to say what it thinks of
    /// the game, not to play for a score, so its score is not set beside the scored runs'.
    pub playtest: bool,
    pub mode: Option<Mode>,
    pub model: Option<String>,
    /// The highest turn the log has reached.
    pub turns: u32,
    pub turn_seconds: Option<u32>,
    pub logged_score: Option<LoggedScore>,
    pub cost_usd: Option<f64>,
    /// Why the run ended, if its log says so (the last line that isn't a `score` is an `end`).
    pub ended: Option<String>,
    /// The highest chunk's `terminal_reason` was "completed".
    pub chunk_completed: bool,
    /// The log's length in bytes when read, and when it was last written.
    pub log_bytes: u64,
    pub modified: Option<SystemTime>,
    pub problem: Option<Problem>,
}

impl RunInfo {
    /// Reads `dir` as it stands at `now` (a log half-written a moment ago is a run in progress,
    /// not a cut-off one). Never fails: whatever can't be read is a gap, or a [`Problem`].
    pub fn read(dir: &Path, now: SystemTime) -> RunInfo {
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut info = RunInfo {
            stamp: stamp_of(&name),
            name,
            dir: dir.to_path_buf(),
            nation: None,
            seed: None,
            scenario: None,
            difficulty: None,
            playtest: false,
            mode: None,
            model: None,
            turns: 0,
            turn_seconds: None,
            logged_score: None,
            cost_usd: None,
            ended: None,
            chunk_completed: false,
            log_bytes: 0,
            modified: None,
            problem: None,
        };
        let log = dir.join("run.jsonl");
        let meta = fs::metadata(&log).ok().filter(|m| m.is_file());
        info.log_bytes = meta.as_ref().map_or(0, |m| m.len());
        info.modified = meta.as_ref().and_then(|m| m.modified().ok());
        info.read_extras();
        let Some(_) = meta else {
            let campaign = dir.join("ledger.json").is_file();
            info.problem = Some(Problem {
                short: "no log",
                why: if campaign {
                    "no run.jsonl: this is a campaign folder, its attempt-N folders hold the runs"
                        .to_owned()
                } else {
                    "no run.jsonl in this folder (a run that hasn't started writing, or not a \
                     run folder)"
                        .to_owned()
                },
                unopenable: true,
            });
            return info;
        };
        let bytes = match fs::read(&log) {
            Ok(b) => b,
            Err(e) => {
                info.problem = Some(Problem {
                    short: "unreadable",
                    why: format!("can't read run.jsonl: {e}"),
                    unopenable: true,
                });
                return info;
            }
        };
        info.read_log(&String::from_utf8_lossy(&bytes), now);
        info
    }

    fn read_log(&mut self, text: &str, now: SystemTime) {
        if text.trim().is_empty() {
            self.problem = Some(Problem {
                short: "empty log",
                why: "run.jsonl is empty: the run hasn't written its header yet".to_owned(),
                unopenable: true,
            });
            return;
        }
        let mut lines = text.split('\n').filter(|l| !l.trim().is_empty());
        let first = lines.next().unwrap_or_default();
        // A header with no newline after it yet is still being written.
        let header_complete = text.contains('\n');
        let header = match serde_json::from_str::<crate::surface::LogLine>(first) {
            Ok(crate::surface::LogLine::Header(h)) if header_complete => h,
            Ok(crate::surface::LogLine::Header(_)) => {
                self.problem = Some(Problem {
                    short: "empty log",
                    why: "run.jsonl holds half a header line: the run is just starting".to_owned(),
                    unopenable: true,
                });
                return;
            }
            Ok(_) => {
                self.problem = Some(Problem {
                    short: "unreadable",
                    why: "run.jsonl doesn't start with a run header".to_owned(),
                    unopenable: true,
                });
                return;
            }
            Err(e) => {
                self.problem = Some(Problem {
                    short: "unreadable",
                    why: format!("run.jsonl's first line isn't a run header: {e}"),
                    unopenable: true,
                });
                return;
            }
        };
        self.nation = Some(header.nation.as_str().to_owned());
        self.seed = Some(header.seed);
        self.difficulty = header.difficulty.clone();
        self.playtest = header.playtest;
        self.turn_seconds = Some(header.turn_seconds);
        if header.prelude.is_some() {
            self.scenario = crate::scenario::id_for(&header)
                .or_else(|| self.name.split_once("-scenario-").map(|(_, id)| id.to_owned()));
        }

        let (mut has_script, mut has_keys) = (false, false);
        let mut bad_line = None;
        for line in lines {
            if line.starts_with(r#"{"type":"turn""#) {
                #[derive(Deserialize)]
                struct Turn {
                    index: u32,
                }
                match serde_json::from_str::<Turn>(line) {
                    Ok(t) => self.turns = self.turns.max(t.index),
                    // The last line of a log still being written may be half there.
                    Err(e) => bad_line = Some(format!("a turn line doesn't parse: {e}")),
                }
                self.ended = None;
            } else if line.starts_with(r#"{"type":"score""#) {
                #[derive(Deserialize)]
                struct Score {
                    score: f64,
                    horizon: u32,
                    #[serde(default)]
                    build: Option<String>,
                }
                if let Ok(s) = serde_json::from_str::<Score>(line) {
                    self.logged_score =
                        Some(LoggedScore { score: s.score, horizon: s.horizon, build: s.build });
                }
                // A `score` line after the end doesn't un-end the run.
            } else if line.starts_with(r#"{"type":"end""#) {
                #[derive(Deserialize)]
                struct End {
                    reason: String,
                }
                self.ended = serde_json::from_str::<End>(line).ok().map(|e| e.reason);
            } else {
                has_script |= line.starts_with(r#"{"type":"script""#);
                has_keys |= line.starts_with(r#"{"type":"keys""#);
                self.ended = None;
            }
        }
        if self.mode.is_none() {
            self.mode = if has_script {
                Some(Mode::Scripts)
            } else if has_keys {
                Some(Mode::Turns)
            } else {
                None
            };
        }

        // A log that stops mid-line is a write that never finished (or one in progress).
        let cut = !text.ends_with('\n');
        if (cut || bad_line.is_some()) && !self.live_at(now) {
            self.problem = Some(Problem {
                short: "cut off",
                why: bad_line.unwrap_or_else(|| {
                    "run.jsonl ends in the middle of a line (the write was cut off); the \
                     watcher shows what is complete"
                        .to_owned()
                }),
                unopenable: false,
            });
        }
    }

    /// `mcp.json`, `chunk-N.json` and `cost.json`: small files, all optional.
    fn read_extras(&mut self) {
        let json = |name: &str| -> Option<serde_json::Value> {
            serde_json::from_str(&fs::read_to_string(self.dir.join(name)).ok()?).ok()
        };
        if let Some(mcp) = json("mcp.json") {
            let scripted = mcp["mcpServers"].as_object().is_some_and(|servers| {
                servers.values().any(|s| {
                    s["args"].as_array().is_some_and(|a| a.iter().any(|x| x == "--scripts"))
                })
            });
            self.mode = Some(if scripted { Mode::Scripts } else { Mode::Turns });
        }
        self.cost_usd = json("cost.json").and_then(|c| c["usd"].as_f64());

        let mut chunks: Vec<(u32, PathBuf)> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let n = name.strip_prefix("chunk-")?.strip_suffix(".json")?.parse().ok()?;
                Some((n, e.path()))
            })
            .collect();
        chunks.sort();
        let read = |p: &Path| -> Option<serde_json::Value> {
            serde_json::from_str(&fs::read_to_string(p).ok()?).ok()
        };
        self.model = chunks.iter().find_map(|(_, p)| {
            let v = read(p)?;
            let model = v["modelUsage"].as_object()?.keys().next()?.clone();
            Some(model.strip_prefix("claude-").map_or(model.clone(), str::to_owned))
        });
        self.chunk_completed = chunks
            .last()
            .and_then(|(_, p)| read(p))
            .is_some_and(|v| v["terminal_reason"] == "completed");
    }

    /// Whether the log is being written right now: no end marker, and written to recently.
    fn live_at(&self, now: SystemTime) -> bool {
        self.ended.is_none()
            && self
                .modified
                .is_some_and(|m| now.duration_since(m).is_ok_and(|age| age < LIVE_WITHIN))
    }

    /// How the run stands at `now`; `None` when the log can't be read at all.
    pub fn state(&self, now: SystemTime) -> Option<State> {
        if self.problem.as_ref().is_some_and(|p| p.unopenable) {
            return None;
        }
        match &self.ended {
            None if self.live_at(now) => Some(State::Live),
            None => Some(State::Stopped),
            Some(reason) => Some(end_state(reason, self.chunk_completed)),
        }
    }

    /// `seed 7633`, or `scenario earth-year-two`.
    pub fn start_text(&self) -> Option<String> {
        match (&self.scenario, self.seed) {
            (Some(id), _) => Some(format!("scenario {id}")),
            (None, Some(seed)) => Some(format!("seed {seed}")),
            _ => None,
        }
    }

    /// `2026-11-08 14:22`, from the folder name.
    pub fn started_text(&self) -> Option<String> {
        let s = self.stamp.as_deref()?;
        Some(format!("{}-{}-{} {}:{}", &s[0..4], &s[4..6], &s[6..8], &s[9..11], &s[11..13]))
    }

    /// Game time covered by `turns`, as `h:mm`.
    pub fn game_time(&self) -> Option<String> {
        let secs = u64::from(self.turns) * u64::from(self.turn_seconds?);
        Some(format!("{}:{:02}", secs / 3600, secs / 60 % 60))
    }

    /// The `score` line's value, with its horizon if that isn't [`SCORE_HORIZON`], and
    /// [`OTHER_BUILD_MARK`] if another build computed it.
    pub fn logged_score_text(&self) -> Option<String> {
        let s = self.logged_score.as_ref()?;
        let mark = if s.from_another_build() { OTHER_BUILD_MARK } else { "" };
        Some(if s.horizon == SCORE_HORIZON {
            format!("{:.1}{mark}", s.score)
        } else {
            format!("{:.1}@{}{mark}", s.score, s.horizon)
        })
    }
}

/// Whether an `end` line's reason was the agent finishing. The agent's own end lines
/// (`agent finished`, `agent session closed`, `chunk N finished`, `stdin closed`) are, and so is
/// `stopped by SIGINT`/`SIGTERM` when the last chunk's session completed: Claude Code stops its
/// MCP server with a signal when it exits, so that is how a normal chunked run ends. Anything
/// else (a read error, a signal with no completed session) stopped it early.
pub fn end_state(reason: &str, chunk_completed: bool) -> State {
    let lower = reason.to_ascii_lowercase();
    if lower.contains("error") || lower.contains("panic") || lower.contains("fail") {
        State::Stopped
    } else if lower.starts_with("stopped by") {
        if chunk_completed { State::Finished } else { State::Stopped }
    } else {
        State::Finished
    }
}

/// `YYYYMMDD-HHMMSS` at the front of a folder name.
fn stamp_of(name: &str) -> Option<String> {
    let b = name.as_bytes();
    let ok = b.len() >= 15
        && b[..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'-'
        && b[9..15].iter().all(u8::is_ascii_digit)
        && (b.len() == 15 || b[15] == b'-');
    ok.then(|| name[..15].to_owned())
}

/// Every run folder under `dir` (every subfolder; a file is not one), unsorted.
pub fn run_dirs(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

/// Date order, newest first (#585): by the date in the folder name, and by nothing a read or a
/// cache write can change (a folder's own modification time moves when `score-cache.json` is
/// written into it, which once put old runs above today's). A folder with no date in its name
/// goes after the dated ones, newest log first (the log's time, not the folder's), and the name
/// breaks every tie so the order never wobbles.
pub fn newest_first(a: &RunInfo, b: &RunInfo) -> std::cmp::Ordering {
    let undated = |i: &RunInfo| if i.stamp.is_none() { i.modified } else { None };
    (&b.stamp, undated(b), &b.name).cmp(&(&a.stamp, undated(a), &a.name))
}

// -------------------------------------------------------------------------------------------
// Scoring
// -------------------------------------------------------------------------------------------

/// What the scorecard says, at [`SCORE_HORIZON`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scored {
    pub score: f64,
    pub signs_met: u32,
    pub signs_total: u32,
    pub deaths: u32,
}

/// What [`CACHE_FILE`] holds. `result` is the error text when scoring failed: that is as
/// deterministic as a score (a log made against other data), so it is cached too.
#[derive(Debug, Serialize, Deserialize)]
struct Cache {
    stamp: String,
    log_bytes: u64,
    horizon: u32,
    scored: Option<Scored>,
    error: Option<String>,
}

/// A fingerprint of everything a score depends on that the log doesn't carry: the bundled data
/// files the replay reads, and the build (`crate::buildid`, #566: the replay is this binary's sim,
/// so the same log can score differently once the code has changed). A cache from another one
/// is ignored, and each run is scored again.
///
/// The build id is the last commit that touched the code or data, plus a hash of uncommitted
/// changes to them, so it already is a "data and sim-relevant" key: a commit that only changes
/// docs, `xtask` or the harness leaves it alone and costs the browser nothing.
fn rules_stamp() -> &'static str {
    use std::sync::OnceLock;
    static STAMP: OnceLock<String> = OnceLock::new();
    STAMP.get_or_init(|| stamp_for(crate::buildid::CURRENT))
}

fn stamp_for(build: &str) -> String {
    let files: [&str; 11] = [
        include_str!("../../../data/biomes.toml"),
        include_str!("../../../data/resources.toml"),
        include_str!("../../../data/nations.toml"),
        include_str!("../../../data/research.toml"),
        include_str!("../../../data/needs.toml"),
        include_str!("../../../data/jobs.toml"),
        include_str!("../../../data/bending.toml"),
        include_str!("../../../data/buildings.toml"),
        include_str!("../../../data/endeavor.toml"),
        include_str!("../../../data/score.toml"),
        include_str!("../../../data/harm.toml"),
    ];
    // FNV-1a, as `Defs::id_hash` does: stable across platforms and Rust versions.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in files.iter().flat_map(|f| f.bytes()).chain(build.bytes()) {
        h = (h ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Where the score cache for `log` lives: [`CACHE_FILE`] beside a run's `run.jsonl`, and
/// `NAME.score-cache.json` beside any other log (a campaign's clean evaluation logs, #585).
fn cache_path(log: &Path) -> PathBuf {
    if log.file_name().is_some_and(|n| n == "run.jsonl") {
        log.with_file_name(CACHE_FILE)
    } else {
        log.with_extension("score-cache.json")
    }
}

/// The cached scoring of `dir`'s log, if there is one made for a log of exactly `log_bytes`
/// bytes under the current rules.
pub fn cached_score(dir: &Path, log_bytes: u64) -> Option<Result<Scored, String>> {
    cached_score_log(&dir.join("run.jsonl"), log_bytes)
}

/// [`cached_score`] for any log file, not only a run folder's `run.jsonl`.
pub fn cached_score_log(log: &Path, log_bytes: u64) -> Option<Result<Scored, String>> {
    let cache: Cache = serde_json::from_str(&fs::read_to_string(cache_path(log)).ok()?).ok()?;
    if cache.stamp != rules_stamp()
        || cache.log_bytes != log_bytes
        || cache.horizon != SCORE_HORIZON
    {
        return None;
    }
    match (cache.scored, cache.error) {
        (Some(s), _) => Some(Ok(s)),
        (None, Some(e)) => Some(Err(e)),
        (None, None) => None,
    }
}

/// Replays `dir`'s log to [`SCORE_HORIZON`] and reads the scorecard off it, with no cache: the
/// bytes of the log that were read, and the scoring of exactly those. Never writes to the log.
/// A panic in the replay is an error, not a crash.
pub fn compute_score(dir: &Path) -> (u64, Result<Scored, String>) {
    compute_score_log(&dir.join("run.jsonl"))
}

/// [`compute_score`] for any log file.
pub fn compute_score_log(log: &Path) -> (u64, Result<Scored, String>) {
    let name = log.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let text = match fs::read(log) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => return (0, Err(format!("can't read {name}: {e}"))),
    };
    let bytes = text.len() as u64;
    let result = std::panic::catch_unwind(|| score_text(&text))
        .unwrap_or_else(|_| Err("the replay crashed".to_owned()));
    (bytes, result)
}

fn score_text(text: &str) -> Result<Scored, String> {
    let (header, lines) = watch::parse(text)?;
    score_lines(&header, &lines)
}

/// The scorecard of a parsed log at [`SCORE_HORIZON`]: the same `referee::score` call under the
/// same bundled weights as `fn_agent score LOG --horizon 540`.
pub fn score_lines(
    header: &RunLogHeader,
    lines: &[crate::surface::LogLine],
) -> Result<Scored, String> {
    let replay = referee::score(header, lines, Some(SCORE_HORIZON), &ScoreDef::bundled())?;
    let signs = || replay.card.milestones.iter().filter(|m| m.kind == MilestoneKind::Sign);
    Ok(Scored {
        score: replay.card.score,
        signs_met: signs().filter(|m| m.first_met.is_some() && m.met_now).count() as u32,
        signs_total: signs().count() as u32,
        deaths: replay.card.care.deaths,
    })
}

/// [`compute_score`], through the cache: a hit costs a small file read, a miss replays the log
/// and writes the cache (best effort: a folder that can't be written to just isn't cached).
/// Returns the log length the answer is for.
pub fn score_run(dir: &Path) -> (u64, Result<Scored, String>) {
    score_log(&dir.join("run.jsonl"))
}

/// [`score_run`] for any log file.
pub fn score_log(log: &Path) -> (u64, Result<Scored, String>) {
    let len = fs::metadata(log).map_or(0, |m| m.len());
    if let Some(hit) = cached_score_log(log, len) {
        return (len, hit);
    }
    let (bytes, result) = compute_score_log(log);
    write_cache(log, bytes, &result);
    (bytes, result)
}

fn write_cache(log: &Path, log_bytes: u64, result: &Result<Scored, String>) {
    let cache = Cache {
        stamp: rules_stamp().to_owned(),
        log_bytes,
        horizon: SCORE_HORIZON,
        scored: result.as_ref().ok().cloned(),
        error: result.as_ref().err().cloned(),
    };
    let Ok(text) = serde_json::to_string(&cache) else { return };
    // Written beside and renamed over, so a browser reading it never sees half of one.
    let path = cache_path(log);
    let tmp = path.with_extension(format!("json.tmp{}", std::process::id()));
    if fs::write(&tmp, text).is_ok() && fs::rename(&tmp, &path).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_come_from_the_folder_name_only_when_they_are_whole() {
        assert_eq!(
            stamp_of("20261108-142213-water-7633-normal").as_deref(),
            Some("20261108-142213")
        );
        assert_eq!(stamp_of("20261108-142213").as_deref(), Some("20261108-142213"));
        assert_eq!(stamp_of("20261108-1422"), None);
        assert_eq!(stamp_of("water-7633"), None);
        assert_eq!(stamp_of("20261108-142213x"), None);
    }

    /// #566: a cache written by another build is not used, so every run is scored again under
    /// this one; one written by this build is.
    #[test]
    fn a_score_cache_from_another_build_is_ignored() {
        let dir = std::env::temp_dir().join(format!("fn_tui_runs_cache_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let scored = Scored { score: 12.5, signs_met: 1, signs_total: 3, deaths: 0 };
        let cache = |stamp: String| Cache {
            stamp,
            log_bytes: 100,
            horizon: SCORE_HORIZON,
            scored: Some(scored.clone()),
            error: None,
        };
        let put = |c: &Cache| fs::write(dir.join(CACHE_FILE), serde_json::to_string(c).unwrap());

        put(&cache(rules_stamp().to_owned())).unwrap();
        assert_eq!(cached_score(&dir, 100), Some(Ok(scored.clone())));
        put(&cache(stamp_for("0badc0d+deadbeef"))).unwrap();
        assert_eq!(cached_score(&dir, 100), None, "another build's score isn't this build's");
        assert_ne!(stamp_for("0badc0d"), stamp_for("0badc0e"));
        assert_eq!(stamp_for(crate::buildid::CURRENT), rules_stamp());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_logged_score_from_another_build_is_marked() {
        let at = |build: Option<&str>| LoggedScore {
            score: 355.0,
            horizon: SCORE_HORIZON,
            build: build.map(str::to_owned),
        };
        assert!(!at(Some(crate::buildid::CURRENT)).from_another_build());
        assert!(at(Some("0badc0d")).from_another_build());
        assert!(at(None).from_another_build(), "a line from before builds were recorded");
    }

    #[test]
    fn end_reasons_split_into_finished_and_stopped() {
        for (reason, completed, want) in [
            ("agent finished", false, State::Finished),
            ("agent session closed", false, State::Finished),
            ("chunk 3 finished", false, State::Finished),
            ("stdin closed", false, State::Finished),
            ("stopped by SIGINT", true, State::Finished),
            ("stopped by SIGTERM", false, State::Stopped),
            ("stdin read error: broken pipe", true, State::Stopped),
        ] {
            assert_eq!(end_state(reason, completed), want, "{reason} / {completed}");
        }
    }
}

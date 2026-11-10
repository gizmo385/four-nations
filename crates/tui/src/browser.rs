//! The run browser (#556): `fn_tui --runs DIR` lists the runs under `DIR` (what `agent-run` leaves
//! in `$FN_AGENT_RUNS`) with their attributes, and Enter opens the selected one in the watcher
//! (`crate::watch`); quitting the watcher returns here, on the same run. `cargo xtask agent-run
//! watch` opens it when run in a terminal with no `RUN_DIR`.
//!
//! **Campaigns (#585).** A campaign folder is a row of its own (track, attempts kept, best clean
//! score, cost, state: `live: a3 evaluating` while one runs), and `right`/`l` (or Enter) shows
//! the attempts under it, oldest first. An attempt's row has its live game (turns, cost, the
//! score it played to) and its clean evaluation (mean, kept or not); Enter on one with live turns
//! opens it in the watcher, and `right`/`l` lists the clean runs of its chosen governor, one per
//! seed with the seed's tier (screen, gate, owner-only, or train and held-out in older ledgers),
//! each of which Enter opens the same way. An attempt with no live turns says so and offers its
//! clean runs instead. `left`/`h` folds a row, or goes up to its parent. The reading is
//! `crate::runs::campaign`; a live campaign is expanded when first seen.
//!
//! The folder's metadata is `crate::runs`. Scores take a replay of each log, so they are never
//! waited for: the list opens at once with `...` in the score cells, and a small pool of threads
//! ([`Scoring::Background`]) fills them in as they finish, newest run first, reading and writing
//! the per-run score cache (`runs::CACHE_FILE`). A clean log is replayed only once its attempt is
//! expanded and only when the ledger has no score for it. Nothing here writes to a `run.jsonl`.
//! The folder is rescanned about once a second, so a live run's turn count moves while the
//! browser is open, and a run that is growing is scored again, at most every [`RESCORE_LIVE`].
//!
//! `--frame` draws it headless instead (no terminal, scores computed up front so the frame is
//! the same every time): `fn_tui --runs DIR --frame [--size WxH] [--keys SCRIPT]`. The keys
//! drive the list, and Enter opens the run's watcher frame in place (as `--watch LOG --frame`
//! would show it, at the end of the log so far), where `q` returns to the list. `--focus PATH`
//! starts on a campaign, attempt or run, expanded. That is how the tests drive the whole thing
//! without a terminal.
//!
//! Only the interactive loop ([`run`]) reads the wall clock; everything else takes `now`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::keyev::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::{ColorDepth, Palette};
use crate::keys::{self, Step};
use crate::runs::campaign::{self, Attempt, Campaign, Game};
use crate::runs::{self, RunInfo, SCORE_HORIZON, Scored, State};
use crate::watch::{self, Watcher};
use crate::widgets;

/// What a playtest's row says in the score column, and in the mode column (#576).
pub const PLAYTEST_CELL: &str = "playtest";

/// A run that is still growing is scored again no sooner than this after its last scoring.
pub const RESCORE_LIVE: Duration = Duration::from_secs(30);

#[cfg(not(target_arch = "wasm32"))]
/// How often the interactive loop rescans the runs folder.
const REFRESH_EVERY: Duration = Duration::from_secs(1);

/// What a key press in the list asks of whoever is running it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    Quit,
    /// Open this in the watcher: a run folder (its `run.jsonl`), or a log file itself.
    Open(PathBuf),
}

/// The log file the watcher reads for something [`Outcome::Open`] names.
pub fn log_of(path: &Path) -> PathBuf {
    if path.is_dir() { path.join("run.jsonl") } else { path.to_path_buf() }
}

/// What the list is ordered by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    /// Newest first.
    Date,
    /// Highest first; runs with no score yet last.
    Score,
}

impl Sort {
    fn word(self) -> &'static str {
        match self {
            Sort::Date => "date",
            Sort::Score => "score",
        }
    }
}

/// Where scores come from.
pub enum Scoring {
    /// Replayed on the spot, in [`Browser::refresh`]: for `--frame` and tests, where a frame
    /// has to be whole and the same every time.
    Inline,
    /// Replayed by worker threads; the list never waits for them.
    Background(Pool),
}

/// Worker threads scoring logs, and the answers coming back.
pub struct Pool {
    jobs: Sender<PathBuf>,
    done: Receiver<Done>,
}

struct Done {
    /// A run folder, or a clean log file.
    path: PathBuf,
    bytes: u64,
    result: Result<Scored, String>,
}

/// Scores a job: a run folder's log, or the log file itself.
fn score_job(path: &Path) -> (u64, Result<Scored, String>) {
    if path.is_dir() { runs::score_run(path) } else { runs::score_log(path) }
}

impl Pool {
    /// `workers` threads (at least one). They end with the process: a browser that quits has no
    /// use for the scores still being worked out.
    pub fn new(workers: usize) -> Pool {
        let (jobs, job_rx) = mpsc::channel::<PathBuf>();
        let (done_tx, done) = mpsc::channel();
        let job_rx = Arc::new(Mutex::new(job_rx));
        for _ in 0..workers.max(1) {
            let (job_rx, done_tx) = (Arc::clone(&job_rx), done_tx.clone());
            std::thread::spawn(move || {
                loop {
                    let job = job_rx.lock().map(|rx| rx.recv());
                    let Ok(Ok(path)) = job else { return };
                    let (bytes, result) = score_job(&path);
                    if done_tx.send(Done { path, bytes, result }).is_err() {
                        return;
                    }
                }
            });
        }
        Pool { jobs, done }
    }
}

/// One top-level row: a run or a campaign, and what is known of its score.
struct Entry {
    info: RunInfo,
    /// Set when the folder is a campaign; `info` then holds only its name and date.
    campaign: Option<Campaign>,
    /// The log length the score was asked for, when it was settled, and what it said. `None`
    /// while it is still being worked out.
    scored: Option<(u64, SystemTime, Result<Scored, String>)>,
    queued: bool,
    /// What the folder's log looked like when `info` was read.
    seen: (u64, Option<SystemTime>),
    /// Whether it looked live then.
    was_live: bool,
}

type GameScore = (u64, SystemTime, Result<Scored, String>);

/// A run opened in a headless frame.
struct Watching {
    watcher: Box<Watcher>,
}

/// A row on screen, as indices into `entries`, then the campaign's attempts, then the attempt's
/// games.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum At {
    Run(usize),
    Campaign(usize),
    Attempt(usize, usize),
    Game(usize, usize, usize),
}

struct Row {
    at: At,
    /// The folder or log file the row is: what is selected, expanded, opened.
    key: PathBuf,
}

pub struct Browser {
    dir: PathBuf,
    entries: Vec<Entry>,
    sort: Sort,
    nation: Option<String>,
    /// The selected row's folder or log: it stays selected through a rescan or a re-sort.
    selected: Option<PathBuf>,
    /// Campaigns and attempts whose rows below are shown.
    expanded: HashSet<PathBuf>,
    /// Live campaigns already opened once, so folding one stays folded.
    auto_expanded: HashSet<PathBuf>,
    /// Scores of clean logs the ledger has no score for, by log file; and those being worked out.
    game_scores: HashMap<PathBuf, GameScore>,
    game_queued: HashSet<PathBuf>,
    /// The first listed row on screen, kept by [`Browser::render`].
    top: usize,
    /// How many list rows the last render had room for.
    page: usize,
    notice: Option<String>,
    scoring: Scoring,
    now: SystemTime,
    /// The pace a watcher starts at.
    pace: Option<u32>,
    depth: ColorDepth,
    ascii: bool,
    frame_size: (u16, u16),
    watching: Option<Watching>,
    quit: bool,
}

impl Browser {
    /// Scans `dir` for runs as of `now`, with the newest selected. With [`Scoring::Inline`] every
    /// score is settled before this returns.
    pub fn open(
        dir: &Path,
        now: SystemTime,
        scoring: Scoring,
        depth: ColorDepth,
        ascii: bool,
    ) -> Browser {
        let mut b = Browser {
            dir: dir.to_path_buf(),
            entries: Vec::new(),
            sort: Sort::Date,
            nation: None,
            selected: None,
            expanded: HashSet::new(),
            auto_expanded: HashSet::new(),
            game_scores: HashMap::new(),
            game_queued: HashSet::new(),
            top: 0,
            page: 10,
            notice: None,
            scoring,
            now,
            pace: None,
            depth,
            ascii,
            frame_size: (120, 40),
            watching: None,
            quit: false,
        };
        b.refresh(now);
        b.selected = b.rows().first().map(|r| r.key.clone());
        b
    }

    pub fn with_pace(mut self, pace: Option<u32>) -> Self {
        self.pace = pace;
        self
    }

    pub fn set_frame_size(&mut self, size: (u16, u16)) {
        self.frame_size = size;
    }

    /// Whether `q` was pressed in the list.
    pub fn quit(&self) -> bool {
        self.quit
    }

    /// The selected row's folder name (a run, campaign or attempt) or log file name.
    pub fn selected_name(&self) -> Option<&str> {
        self.selected.as_deref()?.file_name()?.to_str()
    }

    pub fn sort(&self) -> Sort {
        self.sort
    }

    pub fn nation_filter(&self) -> Option<&str> {
        self.nation.as_deref()
    }

    pub fn is_watching(&self) -> bool {
        self.watching.is_some()
    }

    pub fn set_notice(&mut self, text: impl Into<String>) {
        self.notice = Some(text.into());
    }

    /// Starts on `path`: a run, a campaign, one of its attempts or one of their logs. The
    /// campaign and attempt above it are expanded so the row shows. A path outside the folder
    /// is a notice, and the list stays as it was.
    pub fn focus(&mut self, path: &Path) {
        let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let want = canon(path);
        let found = self.entries.iter().enumerate().find_map(|(i, e)| {
            let rel = want.strip_prefix(canon(&e.info.dir)).ok()?;
            Some((i, e.info.dir.join(rel)))
        });
        let Some((i, key)) = found else {
            self.notice = Some(format!("{} isn't in {}", path.display(), self.dir.display()));
            return;
        };
        let dir = self.entries[i].info.dir.clone();
        let mut select = dir.clone();
        if let Some(c) = &self.entries[i].campaign {
            self.expanded.insert(dir.clone());
            self.auto_expanded.insert(dir);
            if let Some((a, g)) = c.locate(&key) {
                select = c.attempts[a].dir.clone();
                if let Some(g) = g {
                    self.expanded.insert(select.clone());
                    select = c.attempts[a].games[g].log.clone();
                }
            }
        }
        // The nation filter would hide it: show everything.
        self.nation = None;
        self.selected = Some(select);
        self.request_scores(self.now);
    }

    // ----- the data -----

    /// Rescans the folder as of `now`: new runs appear, gone ones go, and a run whose log
    /// changed (or that stopped looking live) is read again. Asks for scores that are missing
    /// or out of date.
    pub fn refresh(&mut self, now: SystemTime) {
        self.now = now;
        let dirs: Vec<(PathBuf, bool)> = runs::run_dirs(&self.dir)
            .into_iter()
            .map(|d| {
                let is_campaign = campaign::is_campaign(&d);
                (d, is_campaign)
            })
            .collect();
        self.entries
            .retain(|e| dirs.iter().any(|(d, c)| *d == e.info.dir && *c == e.campaign.is_some()));
        for (dir, is_campaign) in dirs {
            if is_campaign {
                self.refresh_campaign(&dir, now);
                continue;
            }
            let meta = std::fs::metadata(dir.join("run.jsonl")).ok();
            let seen = (
                meta.as_ref().map_or(0, |m| m.len()),
                meta.as_ref().and_then(|m| m.modified().ok()),
            );
            match self.entries.iter_mut().find(|e| e.info.dir == dir) {
                Some(e) => {
                    let live = e.info.state(now) == Some(State::Live);
                    if e.seen != seen || e.was_live != live {
                        e.info = RunInfo::read(&dir, now);
                        e.seen = seen;
                        e.was_live = e.info.state(now) == Some(State::Live);
                    }
                }
                None => {
                    let info = RunInfo::read(&dir, now);
                    let was_live = info.state(now) == Some(State::Live);
                    self.entries.push(Entry {
                        info,
                        campaign: None,
                        scored: None,
                        queued: false,
                        seen,
                        was_live,
                    });
                }
            }
        }
        self.entries.sort_by(|a, b| runs::newest_first(&a.info, &b.info));
        self.collect_scores();
        self.request_scores(now);
        self.keep_selection();
    }

    fn refresh_campaign(&mut self, dir: &Path, now: SystemTime) {
        let prev =
            self.entries.iter().find(|e| e.info.dir == dir).and_then(|e| e.campaign.as_ref());
        let c = Campaign::read(dir, now, prev);
        match self.entries.iter_mut().find(|e| e.info.dir == dir) {
            Some(e) => {
                e.info.nation = c.track.nation.clone();
                e.campaign = Some(c);
            }
            None => {
                let mut info = RunInfo::read(dir, now);
                info.nation = c.track.nation.clone();
                // A campaign that is running is opened the first time it is seen: that is what
                // the owner came to look at.
                let live = matches!(c.state(now), campaign::State::Live(..));
                if live && self.auto_expanded.insert(dir.to_path_buf()) {
                    self.expanded.insert(dir.to_path_buf());
                }
                self.entries.push(Entry {
                    info,
                    campaign: Some(c),
                    scored: None,
                    queued: false,
                    seen: (0, None),
                    was_live: false,
                });
            }
        }
    }

    /// Takes in every score the workers have finished.
    pub fn collect_scores(&mut self) {
        let Scoring::Background(pool) = &self.scoring else { return };
        let done: Vec<Done> = pool.done.try_iter().collect();
        for d in done {
            self.take(d);
        }
    }

    fn take(&mut self, done: Done) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.info.dir == done.path) {
            e.queued = false;
            e.scored = Some((done.bytes, self.now, done.result));
        } else if self.game_queued.remove(&done.path) {
            self.game_scores.insert(done.path, (done.bytes, self.now, done.result));
        }
    }

    /// Blocks until every requested score has come back, or `timeout` passes: for a headless
    /// caller that wants the settled list (a test of the background pool).
    pub fn wait_for_scores(&mut self, timeout: Duration) {
        let deadline = std::time::Instant::now() + timeout;
        while self.scoring_pending() {
            let Scoring::Background(pool) = &self.scoring else { return };
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match pool.done.recv_timeout(left) {
                Ok(done) => self.take(done),
                Err(_) => return,
            }
        }
    }

    /// Whether any score is still being worked out.
    pub fn scoring_pending(&self) -> bool {
        self.entries.iter().any(|e| e.queued) || !self.game_queued.is_empty()
    }

    fn request_scores(&mut self, now: SystemTime) {
        let mut jobs = Vec::new();
        for e in &mut self.entries {
            if e.queued || e.info.problem.as_ref().is_some_and(|p| p.unopenable) {
                continue;
            }
            let bytes = e.info.log_bytes;
            match &e.scored {
                Some((b, _, _)) if *b == bytes => continue,
                // Scored for a shorter log: not again until it has settled for a while (a
                // growing log would otherwise be replayed in full over and over).
                Some((_, at, _)) if now.duration_since(*at).is_ok_and(|d| d < RESCORE_LIVE) => {
                    continue;
                }
                _ => {}
            }
            if let Some(hit) = runs::cached_score(&e.info.dir, bytes) {
                e.scored = Some((bytes, now, hit));
                continue;
            }
            e.queued = true;
            jobs.push(e.info.dir.clone());
        }

        // The clean logs of expanded attempts that the ledger has no score for (an attempt still
        // being evaluated, or a ledger from before the rows had them).
        let wanted: Vec<(PathBuf, u64)> = self
            .entries
            .iter()
            .filter_map(|e| e.campaign.as_ref())
            .flat_map(|c| &c.attempts)
            .filter(|a| self.expanded.contains(&a.dir))
            .flat_map(|a| &a.games)
            .filter(|g| g.score.is_none() && g.bytes > 0)
            .map(|g| (g.log.clone(), g.bytes))
            .collect();
        for (log, bytes) in wanted {
            if self.game_queued.contains(&log) {
                continue;
            }
            match self.game_scores.get(&log) {
                Some((b, _, _)) if *b == bytes => continue,
                Some((_, at, _)) if now.duration_since(*at).is_ok_and(|d| d < RESCORE_LIVE) => {
                    continue;
                }
                _ => {}
            }
            if let Some(hit) = runs::cached_score_log(&log, bytes) {
                self.game_scores.insert(log, (bytes, now, hit));
                continue;
            }
            self.game_queued.insert(log.clone());
            jobs.push(log);
        }

        match &self.scoring {
            Scoring::Background(pool) => {
                for path in jobs {
                    let _ = pool.jobs.send(path);
                }
            }
            Scoring::Inline => {
                for path in jobs {
                    let (bytes, result) = score_job(&path);
                    self.take(Done { path, bytes, result });
                }
            }
        }
    }

    /// The entries on screen, as indices into `entries`: filtered by nation, in sort order.
    fn visible(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.entries.len())
            .filter(|&i| {
                self.nation
                    .as_deref()
                    .is_none_or(|n| self.entries[i].info.nation.as_deref() == Some(n))
            })
            .collect();
        if self.sort == Sort::Score {
            // `entries` is in date order and the sort is stable, so ties stay newest first.
            v.sort_by(|&a, &b| {
                let (sa, sb) = (self.score_value(a), self.score_value(b));
                match (sa, sb) {
                    (Some(x), Some(y)) => y.total_cmp(&x),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
        }
        v
    }

    /// The number the score column shows for entry `i`, if it has one yet: the log's own `score`
    /// line when it has one, else the replay's; for a campaign, its best clean score.
    fn score_value(&self, i: usize) -> Option<f64> {
        let e = &self.entries[i];
        if e.info.playtest {
            return None; // sorts with the unscored, never among the scored (#576)
        }
        if let Some(c) = &e.campaign {
            return c.best();
        }
        let computed = match &e.scored {
            Some((_, _, Ok(s))) => Some(s.score),
            _ => None,
        };
        e.info.logged_score.as_ref().map(|s| s.score).or(computed)
    }

    /// Every row on screen: the entries in order, each expanded campaign followed by its
    /// attempts (oldest first) and each expanded attempt by its clean logs.
    fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        for i in self.visible() {
            let e = &self.entries[i];
            let Some(c) = &e.campaign else {
                rows.push(Row { at: At::Run(i), key: e.info.dir.clone() });
                continue;
            };
            rows.push(Row { at: At::Campaign(i), key: c.dir.clone() });
            if !self.expanded.contains(&c.dir) {
                continue;
            }
            for (a, att) in c.attempts.iter().enumerate() {
                rows.push(Row { at: At::Attempt(i, a), key: att.dir.clone() });
                if self.expanded.contains(&att.dir) {
                    for (g, game) in att.games.iter().enumerate() {
                        rows.push(Row { at: At::Game(i, a, g), key: game.log.clone() });
                    }
                }
            }
        }
        rows
    }

    fn campaign_of(&self, i: usize) -> &Campaign {
        self.entries[i].campaign.as_ref().expect("a campaign row")
    }

    fn attempt_of(&self, i: usize, a: usize) -> &Attempt {
        &self.campaign_of(i).attempts[a]
    }

    fn keep_selection(&mut self) {
        let rows = self.rows();
        let Some(sel) = self.selected.clone() else {
            self.selected = rows.first().map(|r| r.key.clone());
            return;
        };
        if rows.iter().any(|r| r.key == sel) {
            return;
        }
        // Its row went (folded away, filtered out, gone): the nearest row above it, else the top.
        self.selected = sel
            .ancestors()
            .skip(1)
            .find(|p| rows.iter().any(|r| r.key == *p))
            .map(Path::to_path_buf)
            .or_else(|| rows.first().map(|r| r.key.clone()));
    }

    fn selected_row(&self, rows: &[Row]) -> Option<usize> {
        let key = self.selected.as_ref()?;
        rows.iter().position(|r| &r.key == key)
    }

    // ----- keys -----

    /// One key pressed in the list.
    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        self.notice = None;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('c') if ctrl => return self.quit_now(),
            KeyCode::Char('q' | 'Q') => return self.quit_now(),
            KeyCode::Up | KeyCode::Char('k') => self.step(-1),
            KeyCode::Down | KeyCode::Char('j') => self.step(1),
            KeyCode::PageUp => self.step(-(self.page as isize)),
            KeyCode::PageDown => self.step(self.page as isize),
            KeyCode::Home | KeyCode::Char('g') => self.step(isize::MIN / 2),
            KeyCode::End | KeyCode::Char('G') => self.step(isize::MAX / 2),
            KeyCode::Enter => return self.open_selected(),
            KeyCode::Right | KeyCode::Char('l') => self.unfold(),
            KeyCode::Left | KeyCode::Char('h') => self.fold(),
            KeyCode::Char(' ') => self.toggle(),
            KeyCode::Char('s' | 'S') => {
                self.sort = if self.sort == Sort::Date { Sort::Score } else { Sort::Date };
            }
            KeyCode::Char('n' | 'N') => self.cycle_nation(),
            _ => {}
        }
        Outcome::Stay
    }

    fn quit_now(&mut self) -> Outcome {
        self.quit = true;
        Outcome::Quit
    }

    fn step(&mut self, by: isize) {
        let rows = self.rows();
        if rows.is_empty() {
            return;
        }
        let at = self.selected_row(&rows).unwrap_or(0) as isize;
        let to = at.saturating_add(by).clamp(0, rows.len() as isize - 1) as usize;
        self.selected = Some(rows[to].key.clone());
    }

    /// Whether the selected row has rows to show under it.
    fn foldable(&self, at: At) -> bool {
        match at {
            At::Campaign(i) => !self.campaign_of(i).attempts.is_empty(),
            At::Attempt(i, a) => !self.attempt_of(i, a).games.is_empty(),
            At::Run(_) | At::Game(..) => false,
        }
    }

    fn unfold(&mut self) {
        let rows = self.rows();
        let Some(row) = self.selected_row(&rows).map(|n| &rows[n]) else { return };
        if !self.foldable(row.at) {
            return;
        }
        if self.expanded.insert(row.key.clone()) {
            self.request_scores(self.now);
        } else {
            // Already open: go in.
            self.step(1);
        }
    }

    /// Folds the selected row, or when it is closed already goes up to the row it belongs to.
    fn fold(&mut self) {
        let rows = self.rows();
        let Some(row) = self.selected_row(&rows).map(|n| &rows[n]) else { return };
        if self.expanded.remove(&row.key) {
            return;
        }
        let parent = match row.at {
            At::Attempt(i, _) => Some(self.campaign_of(i).dir.clone()),
            At::Game(i, a, _) => Some(self.attempt_of(i, a).dir.clone()),
            At::Run(_) | At::Campaign(_) => None,
        };
        if parent.is_some() {
            self.selected = parent;
        }
    }

    fn toggle(&mut self) {
        let rows = self.rows();
        let Some(row) = self.selected_row(&rows).map(|n| &rows[n]) else { return };
        if self.foldable(row.at) && self.expanded.contains(&row.key) {
            self.fold();
        } else {
            self.unfold();
        }
    }

    fn cycle_nation(&mut self) {
        let mut nations: Vec<&str> =
            self.entries.iter().filter_map(|e| e.info.nation.as_deref()).collect();
        nations.sort_unstable();
        nations.dedup();
        let next = match self.nation.as_deref() {
            None => nations.first().copied(),
            Some(cur) => {
                nations.iter().position(|n| *n == cur).and_then(|i| nations.get(i + 1)).copied()
            }
        };
        self.nation = next.map(str::to_owned);
        self.keep_selection();
    }

    fn open_selected(&mut self) -> Outcome {
        let rows = self.rows();
        let Some(row) = self.selected_row(&rows).map(|n| &rows[n]) else {
            self.notice = Some("no run selected".to_owned());
            return Outcome::Stay;
        };
        match row.at {
            At::Run(i) => {
                let e = &self.entries[i];
                if let Some(p) = e.info.problem.as_ref().filter(|p| p.unopenable) {
                    self.notice = Some(format!("can't open {}: {}", e.info.name, p.why));
                    return Outcome::Stay;
                }
                Outcome::Open(e.info.dir.clone())
            }
            At::Campaign(_) => {
                self.toggle();
                Outcome::Stay
            }
            At::Attempt(i, a) => {
                let att = self.attempt_of(i, a);
                let name =
                    att.dir.file_name().map_or_else(String::new, |n| n.to_string_lossy().into());
                let has_games = !att.games.is_empty();
                let key = att.dir.clone();
                let problem = att.run.problem.as_ref().filter(|p| p.unopenable);
                let (why, open) = match problem {
                    Some(p) => (format!("can't open {name}: {}", p.why), false),
                    None if att.live_turns() == 0 => {
                        (format!("{name} {}", att.no_live_turns_why()), false)
                    }
                    None => (String::new(), true),
                };
                if open {
                    return Outcome::Open(key);
                }
                self.notice = Some(if has_games {
                    format!("{why}; its clean runs are listed below")
                } else {
                    why
                });
                if has_games {
                    self.expanded.insert(key);
                    self.request_scores(self.now);
                }
                Outcome::Stay
            }
            At::Game(i, a, g) => {
                let game = &self.attempt_of(i, a).games[g];
                if game.bytes == 0 {
                    self.notice = Some(format!("can't open {}: the log is empty", game.label));
                    return Outcome::Stay;
                }
                Outcome::Open(game.log.clone())
            }
        }
    }

    // ----- headless frames -----

    /// Presses a `--keys` script's keys, headless: in the list they are the list's, and `Enter`
    /// opens the run's watcher frame in place, where keys are the watcher's own (`q` comes back
    /// here). A `<tick N>` means nothing to a browser and is skipped.
    pub fn play(&mut self, steps: &[Step]) {
        for step in steps {
            if let Step::Key(key) = step {
                self.press(*key);
            }
        }
    }

    fn press(&mut self, key: KeyEvent) {
        if self.quit {
            return;
        }
        if let Some(w) = &mut self.watching {
            if watch::control_key(&mut w.watcher, key) {
                self.watching = None;
            }
            return;
        }
        if let Outcome::Open(path) = self.key(key) {
            self.open_in_frame(&path);
        }
    }

    fn open_in_frame(&mut self, path: &Path) {
        let log = log_of(path);
        // `watch::parse` leaves off a last line that was cut short, as the live tail would.
        let read = std::fs::read_to_string(&log)
            .map_err(|e| format!("{}: {e}", log.display()))
            .and_then(|text| watch::parse(&text));
        let opened = read.and_then(|(header, events)| {
            let mut watcher = Watcher::new(&header)?;
            if let Some(pace) = self.pace {
                while watcher.pace() < pace {
                    watcher.faster();
                }
            }
            watcher.feed(events);
            watcher.fast_forward_to_turn(u32::MAX);
            watcher.set_frame_size(self.frame_size.0, self.frame_size.1);
            Ok(watcher)
        });
        match opened {
            Ok(watcher) => self.watching = Some(Watching { watcher: Box::new(watcher) }),
            Err(e) => self.notice = Some(format!("can't open {}: {e}", path.display())),
        }
    }

    // ----- drawing -----

    /// The frame for a `size` terminal.
    pub fn render(&mut self, size: (u16, u16)) -> Buffer {
        if let Some(w) = &mut self.watching {
            let palette =
                Palette::new(w.watcher.defs(), w.watcher.nation(), self.depth, self.ascii);
            return w.watcher.render(&palette, size);
        }
        let area = Rect::new(0, 0, size.0, size.1);
        let mut buf = Buffer::empty(area);
        self.draw(&mut buf);
        buf
    }

    fn ellipsis(&self) -> &'static str {
        if self.ascii { "~" } else { "…" }
    }

    /// `text` cut to `width` cells with an ellipsis, only if it has to be.
    fn clip(&self, text: &str, width: usize) -> String {
        if text.chars().count() <= width {
            return text.to_owned();
        }
        let keep = width.saturating_sub(1);
        let mut s: String = text.chars().take(keep).collect();
        if width > 0 {
            s.push_str(self.ellipsis());
        }
        s
    }

    /// The first of `options` that fits in `width`, else the last one clipped.
    fn fit(&self, options: &[String], width: usize) -> String {
        options
            .iter()
            .find(|o| o.chars().count() <= width)
            .cloned()
            .unwrap_or_else(|| self.clip(options.last().map_or("", |s| s.as_str()), width))
    }

    fn state_cell(&self, e: &Entry) -> String {
        if let Some(p) = &e.info.problem {
            return p.short.to_owned();
        }
        e.info.state(self.now).map_or("-".to_owned(), |s| s.word().to_owned())
    }

    fn score_cell(&self, e: &Entry) -> String {
        // A playtest wasn't played for score (#576): the row says so where the number would go.
        if e.info.playtest {
            return PLAYTEST_CELL.to_owned();
        }
        if let Some(text) = e.info.logged_score_text() {
            return text;
        }
        if e.info.problem.as_ref().is_some_and(|p| p.unopenable) {
            return "-".to_owned();
        }
        match &e.scored {
            Some((_, _, Ok(s))) => format!("{:.1}", s.score),
            Some((_, _, Err(_))) => "error".to_owned(),
            None => "...".to_owned(),
        }
    }

    fn scorecard_cell(&self, e: &Entry, f: impl Fn(&Scored) -> String) -> String {
        if e.info.problem.as_ref().is_some_and(|p| p.unopenable) {
            return "-".to_owned();
        }
        match &e.scored {
            Some((_, _, Ok(s))) => f(s),
            Some((_, _, Err(_))) => "-".to_owned(),
            None => "...".to_owned(),
        }
    }

    /// The word for a campaign's state: `live: a3 evaluating`, `finished`, `stopped early`.
    fn campaign_state_text(&self, c: &Campaign) -> String {
        if c.problem.is_some() {
            return "bad ledger".to_owned();
        }
        match c.state(self.now) {
            campaign::State::Live(k, Some(activity)) => format!("live: a{k} {}", activity.word()),
            campaign::State::Live(k, None) => format!("live: a{k} starting"),
            campaign::State::Finished if c.attempts.is_empty() => "no attempts".to_owned(),
            campaign::State::Finished => "finished".to_owned(),
            campaign::State::Stopped => "stopped early".to_owned(),
        }
    }

    /// What an attempt is doing or how it ended.
    fn attempt_state_text(&self, c: &Campaign, a: &Attempt) -> String {
        // The attempt under way says what it is doing, in place of how a finished one ended.
        if let campaign::State::Live(k, Some(activity)) = c.state(self.now)
            && k == a.number
            && c.open_attempt().is_some_and(|open| open.number == a.number)
        {
            return activity.word().to_owned();
        }
        if let Some(p) = &a.run.problem {
            return p.short.to_owned();
        }
        if a.live_turns() == 0 {
            return "no live turns".to_owned();
        }
        a.run.state(self.now).map_or("-".to_owned(), |s| s.word().to_owned())
    }

    fn game_score_text(&self, g: &Game) -> String {
        if let Some(s) = g.score {
            return format!("{s:.1}");
        }
        if g.bytes == 0 {
            return "-".to_owned();
        }
        match self.game_scores.get(&g.log) {
            Some((_, _, Ok(s))) => format!("{:.1}", s.score),
            Some((_, _, Err(_))) => "error".to_owned(),
            None => "...".to_owned(),
        }
    }

    /// The text of every cell of `row`; `-` where it has no value.
    fn cells(&self, row: &Row, nest: bool) -> Cells {
        let dash = || "-".to_owned();
        let or_dash = |o: Option<String>| o.unwrap_or_else(dash);
        let money = |c: Option<f64>| or_dash(c.map(|c| format!("${c:.2}")));
        let mut c = Cells::dashes();
        match row.at {
            At::Run(i) => {
                let e = &self.entries[i];
                let readable = !e.info.problem.as_ref().is_some_and(|p| p.unopenable);
                // Beside a campaign's `+`, a run's name lines up with the campaign names.
                c.name = format!("{}{}", if nest { "  " } else { "" }, e.info.name);
                c.state = self.state_cell(e);
                c.score = self.score_cell(e);
                if readable {
                    c.turns = e.info.turns.to_string();
                }
                c.game = or_dash(e.info.game_time().filter(|_| readable));
                c.signs = self.scorecard_cell(e, |s| format!("{}/{}", s.signs_met, s.signs_total));
                c.dead = self.scorecard_cell(e, |s| s.deaths.to_string());
                c.nation = or_dash(e.info.nation.clone());
                c.start = or_dash(e.info.start_text());
                c.mode = if e.info.playtest {
                    PLAYTEST_CELL.to_owned()
                } else {
                    or_dash(e.info.mode.map(|m| m.word().to_owned()))
                };
                c.cost = money(e.info.cost_usd);
                c.model = or_dash(e.info.model.clone());
                c.diff = or_dash(e.info.difficulty.clone());
                c.started = or_dash(e.info.started_text());
            }
            At::Campaign(i) => {
                let camp = self.campaign_of(i);
                let open = self.expanded.contains(&camp.dir) && !camp.attempts.is_empty();
                let mark = if camp.attempts.is_empty() {
                    ' '
                } else if open {
                    '-'
                } else {
                    '+'
                };
                c.name = format!("{mark} {}", camp.name);
                c.state = self.campaign_state_text(camp);
                c.clean = or_dash(camp.best().map(|b| format!("{b:.1}")));
                let (kept, all) = camp.kept_of();
                c.kept = format!("{kept} of {all}");
                c.nation = or_dash(camp.track.nation.clone());
                c.start = or_dash(camp.track.id.clone());
                c.mode = "campaign".to_owned();
                c.cost = money(camp.cost_usd());
                c.model = or_dash(camp.attempts.iter().find_map(|a| a.run.model.clone()));
                c.diff = or_dash(camp.track.difficulty.clone());
                c.started = or_dash(camp.started_text());
            }
            At::Attempt(i, a) => {
                let camp = self.campaign_of(i);
                let att = &camp.attempts[a];
                let mark = if att.games.is_empty() {
                    ' '
                } else if self.expanded.contains(&att.dir) {
                    '-'
                } else {
                    '+'
                };
                c.name = format!("  {mark} attempt-{}", att.number);
                c.state = self.attempt_state_text(camp, att);
                let row = att.row.as_ref();
                c.score = or_dash(
                    row.and_then(|r| r.played)
                        .map(|p| format!("{p:.1}"))
                        .or_else(|| att.run.logged_score_text()),
                );
                c.clean = or_dash(row.and_then(|r| r.clean_mean()).map(|m| format!("{m:.1}")));
                c.kept = row.map_or_else(dash, |r| if r.kept { "kept" } else { "no" }.to_owned());
                let readable = !att.run.problem.as_ref().is_some_and(|p| p.unopenable);
                if readable {
                    c.turns = att.live_turns().to_string();
                }
                c.game = or_dash(att.run.game_time().filter(|_| readable));
                c.nation = or_dash(camp.track.nation.clone());
                c.start = or_dash(att.governor_text());
                c.mode = or_dash(att.run.mode.map(|m| m.word().to_owned()));
                c.cost = money(att.cost_usd);
                c.model = or_dash(att.run.model.clone());
                c.diff = or_dash(camp.track.difficulty.clone());
            }
            At::Game(i, a, g) => {
                let camp = self.campaign_of(i);
                let game = &camp.attempts[a].games[g];
                c.name = format!("      {}", game.label);
                c.state = match (&game.stopped, game.stopped_turn) {
                    (Some(why), Some(t)) => format!("stopped: {why} t{t}"),
                    (Some(why), None) => format!("stopped: {why}"),
                    _ => dash(),
                };
                c.clean = self.game_score_text(game);
                c.nation = or_dash(camp.track.nation.clone());
                c.start = or_dash(game.seed.map(|s| format!("seed {s}")));
                c.mode = or_dash(game.tier.map(|t| t.word().to_owned()));
                c.diff = or_dash(camp.track.difficulty.clone());
            }
        }
        c
    }

    fn draw(&mut self, buf: &mut Buffer) {
        let area = buf.area;
        let (w, h) = (area.width as usize, area.height as usize);
        if w == 0 || h == 0 {
            return;
        }
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let selected_style = Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD);

        let rows = self.rows();
        let nest = self.entries.iter().any(|e| e.campaign.is_some());
        // Reserved lines below the list: the status text (up to two lines) and the key hints.
        let footer = if h >= 9 {
            3
        } else if h >= 5 {
            2
        } else {
            1
        };
        let header = if h >= 4 { 2 } else { 0 };
        let list_h = h.saturating_sub(header + footer);
        self.page = list_h.max(1);

        // Title.
        if header > 0 {
            let shown = self.visible().len();
            let total = self.entries.len();
            let nation = self.nation.as_deref().unwrap_or("all");
            let title = self.fit(
                &[
                    format!(
                        "Runs in {}  ({shown} of {total})  sort: {}  nation: {nation}",
                        self.dir.display(),
                        self.sort.word()
                    ),
                    format!(
                        "Runs  ({shown} of {total})  sort: {}  nation: {nation}",
                        self.sort.word()
                    ),
                    format!("Runs  {shown}/{total}  {}  {nation}", self.sort.word()),
                    format!("Runs  {shown}/{total}"),
                ],
                w,
            );
            buf.set_string(0, 0, title, bold);
        }

        // The list.
        let marker = 2usize;
        let cells: Vec<Cells> = rows.iter().map(|r| self.cells(r, nest)).collect();
        let name_w = cells
            .iter()
            .map(|c| c.name.chars().count())
            .chain(["RUN".len()])
            .max()
            .unwrap_or(3)
            .min(w.saturating_sub(marker).max(1));
        let all = Cells::columns(&cells, nest);
        let mut shown_cols = Vec::new();
        let mut used = marker + name_w;
        for col in &all {
            if used + 2 + col.width > w {
                break;
            }
            used += 2 + col.width;
            shown_cols.push(col);
        }

        if header > 0 {
            let mut line = format!("{:marker$}{:name_w$}", "", "RUN");
            for c in &shown_cols {
                line.push_str("  ");
                line.push_str(&c.pad(c.head));
            }
            buf.set_string(0, 1, line, dim);
        }

        if rows.is_empty() {
            let text = match &self.nation {
                Some(n) => format!("no {n} runs in {} (n shows other nations)", self.dir.display()),
                None => format!("no runs in {}", self.dir.display()),
            };
            if list_h > 0 {
                buf.set_string(
                    0,
                    header as u16,
                    self.fit(&[text, "no runs".to_owned()], w),
                    Style::default(),
                );
            }
        } else {
            let sel = self.selected_row(&rows).unwrap_or(0);
            if sel < self.top {
                self.top = sel;
            } else if list_h > 0 && sel >= self.top + list_h {
                self.top = sel + 1 - list_h;
            }
            self.top = self.top.min(rows.len().saturating_sub(list_h.max(1)));
            for (n, cell) in cells.iter().enumerate().skip(self.top).take(list_h) {
                let y = (header + n - self.top) as u16;
                let is_sel = n == sel;
                let mut line = format!(
                    "{}{}",
                    if is_sel { "> " } else { "  " },
                    pad_right(&self.clip(&cell.name, name_w), name_w)
                );
                for c in &shown_cols {
                    line.push_str("  ");
                    line.push_str(&c.pad(&c.cells[n]));
                }
                let style = if is_sel { selected_style } else { Style::default() };
                // The selected row's highlight spans the line, so it reads at any width.
                if is_sel {
                    buf.set_string(0, y, " ".repeat(w), selected_style);
                }
                buf.set_string(0, y, self.clip(&line, w), style);
            }
        }

        // The status text and the key hints.
        let status_rows = footer.saturating_sub(1);
        if status_rows > 0 {
            let text = self.status_text(&rows);
            for (n, line) in wrap(&text, w).into_iter().take(status_rows).enumerate() {
                buf.set_string(0, (h - footer + n) as u16, line, Style::default());
            }
        }
        let hints = self.fit(
            &[
                "up/down j/k move   enter open   right/left fold   s sort   n nation   q quit"
                    .to_owned(),
                "j/k move  enter open  l/h fold  s sort  n nation  q quit".to_owned(),
                "enter open  q quit".to_owned(),
                "q quit".to_owned(),
            ],
            w,
        );
        buf.set_string(0, (h - 1) as u16, hints, dim);
    }

    /// What the status line says: a notice if there is one, else about the selected row.
    fn status_text(&self, rows: &[Row]) -> String {
        if let Some(n) = &self.notice {
            return n.clone();
        }
        let Some(row) = self.selected_row(rows).map(|n| &rows[n]) else {
            return if self.scoring_pending() { "scoring...".to_owned() } else { String::new() };
        };
        match row.at {
            At::Run(i) => self.run_status(&self.entries[i]),
            At::Campaign(i) => self.campaign_status(self.campaign_of(i)),
            At::Attempt(i, a) => self.attempt_status(self.campaign_of(i), self.attempt_of(i, a)),
            At::Game(i, a, g) => self.game_status(&self.attempt_of(i, a).games[g]),
        }
    }

    fn run_status(&self, e: &Entry) -> String {
        let mut parts = Vec::new();
        if let Some(p) = &e.info.problem {
            parts.push(p.why.clone());
        }
        if let Some((_, _, Err(why))) = &e.scored {
            parts.push(format!("no score: {why}"));
        }
        if parts.is_empty() {
            let score = match (&e.info.logged_score, &e.scored) {
                (Some(l), _) => format!(
                    "score {:.1} at {} turns (from the log's score line{})",
                    l.score,
                    l.horizon,
                    if l.from_another_build() {
                        format!(
                            "; computed by {}, this is build {}: a rescore may differ",
                            crate::buildid::describe(l.build.as_deref()),
                            crate::buildid::CURRENT
                        )
                    } else {
                        String::new()
                    }
                ),
                (None, Some((_, _, Ok(s)))) => {
                    format!(
                        "score {:.1} at {SCORE_HORIZON} turns, as fn_agent score computes it",
                        s.score
                    )
                }
                _ => format!("score at {SCORE_HORIZON} turns: being worked out"),
            };
            if e.info.playtest {
                parts.push(format!(
                    "{}: a playtest, played to learn about the game and not for score; {score}",
                    e.info.name
                ));
            } else {
                parts.push(format!("{}: {score}", e.info.name));
            }
        }
        parts.join("; ")
    }

    fn campaign_status(&self, c: &Campaign) -> String {
        if let Some(why) = &c.problem {
            return format!("{}: {why}", c.name);
        }
        let (kept, all) = c.kept_of();
        let track = c.track.id.as_deref().unwrap_or("unknown track");
        let mut s =
            format!("{}: campaign on {track}, {all} attempt(s) so far, {kept} kept", c.name);
        match (c.champion(), c.best()) {
            (Some(champ), Some(best)) => {
                s += &format!(", best clean mean {best:.1} (attempt {})", champ.number);
            }
            _ => s += ", no clean mean yet",
        }
        if let Some(cost) = c.cost_usd() {
            s += &format!(", ${cost:.2}");
        }
        s += &format!("; {}", self.campaign_state_text(c));
        if c.attempts.is_empty() {
            s += ". No attempt folder yet";
        } else {
            s += ". Right shows the attempts, oldest first";
        }
        s
    }

    fn attempt_status(&self, c: &Campaign, a: &Attempt) -> String {
        let mut s = format!("attempt {} of {}: ", a.number, c.name);
        match a.row.as_ref() {
            None => s += &format!("no ledger row yet ({})", self.attempt_state_text(c, a)),
            Some(r) => {
                s += &match r.clean_mean() {
                    Some(m) => format!("clean mean {m:.1}"),
                    None => "no clean mean (no governor)".to_owned(),
                };
                if let Some(g) = r.gate_mean {
                    s += &format!(", gate {g:.1}");
                }
                if let Some(v) = &r.verdict {
                    s += &format!(", {} vs champion (attempt {})", v.text(), v.champion_attempt);
                }
                s += if r.kept { ", kept" } else { ", not kept" };
            }
        }
        s += &format!("; {} live turn(s)", a.live_turns());
        if let Some(cost) = a.cost_usd {
            s += &format!(", ${cost:.2}");
        }
        if let Some(p) = a.run.problem.as_ref().filter(|p| p.unopenable) {
            s += &format!(". Live game: {}", p.why);
        } else if a.live_turns() == 0 {
            s += &format!(". It {}", a.no_live_turns_why());
        } else {
            s += ". Enter watches the live game";
        }
        match a.games.len() {
            0 => s += ". No clean runs yet",
            n => s += &format!(". Right lists its {n} clean run(s)"),
        }
        s
    }

    fn game_status(&self, g: &Game) -> String {
        let tier = g.tier.map_or("seed not in the track", |t| t.word());
        let mut s = format!("clean run {} ({tier}", g.label);
        if let Some(seed) = g.seed {
            s += &format!(", seed {seed}");
        }
        s += &format!("): score {}", self.game_score_text(g));
        if let Some(why) = &g.stopped {
            s += &format!(", its governor stopped ({why})");
        }
        s += if g.bytes == 0 { "; the log is empty" } else { "; Enter watches it" };
        s
    }
}

/// The text of one row's cells, before the columns are cut to the terminal.
struct Cells {
    name: String,
    state: String,
    score: String,
    clean: String,
    kept: String,
    turns: String,
    game: String,
    signs: String,
    dead: String,
    nation: String,
    start: String,
    mode: String,
    cost: String,
    model: String,
    diff: String,
    started: String,
}

impl Cells {
    fn dashes() -> Cells {
        let d = || "-".to_owned();
        Cells {
            name: String::new(),
            state: d(),
            score: d(),
            clean: d(),
            kept: d(),
            turns: d(),
            game: d(),
            signs: d(),
            dead: d(),
            nation: d(),
            start: d(),
            mode: d(),
            cost: d(),
            model: d(),
            diff: d(),
            started: d(),
        }
    }

    /// Every column, most valuable first. A narrow terminal gets a prefix of this list. The two
    /// clean-evaluation columns are only there when a campaign is.
    fn columns(rows: &[Cells], campaigns: bool) -> Vec<Column> {
        type Get = fn(&Cells) -> &String;
        let col = |head: &'static str, right: bool, get: Get| {
            Column::new(head, right, rows.iter().map(|c| get(c).clone()).collect())
        };
        let mut cols =
            vec![col("STATE", false, |c| &c.state), col("SCORE@540", true, |c| &c.score)];
        if campaigns {
            cols.push(col("CLEAN", true, |c| &c.clean));
            cols.push(col("KEPT", false, |c| &c.kept));
        }
        cols.extend([
            col("TURNS", true, |c| &c.turns),
            col("GAME", true, |c| &c.game),
            col("SIGNS", true, |c| &c.signs),
            col("DEAD", true, |c| &c.dead),
            col("NATION", false, |c| &c.nation),
            col("START", false, |c| &c.start),
            col("MODE", false, |c| &c.mode),
            col("COST", true, |c| &c.cost),
            col("MODEL", false, |c| &c.model),
            col("DIFF", false, |c| &c.diff),
            col("STARTED", false, |c| &c.started),
        ]);
        cols
    }
}

/// One column of the list.
struct Column {
    head: &'static str,
    right: bool,
    cells: Vec<String>,
    width: usize,
}

impl Column {
    fn new(head: &'static str, right: bool, cells: Vec<String>) -> Column {
        let width = cells
            .iter()
            .map(|c| c.chars().count())
            .chain([head.chars().count()])
            .max()
            .unwrap_or(0);
        Column { head, right, cells, width }
    }

    fn pad(&self, text: &str) -> String {
        let fill = self.width.saturating_sub(text.chars().count());
        if self.right {
            format!("{}{text}", " ".repeat(fill))
        } else {
            format!("{text}{}", " ".repeat(fill))
        }
    }
}

fn pad_right(text: &str, width: usize) -> String {
    format!("{text}{}", " ".repeat(width.saturating_sub(text.chars().count())))
}

/// `text` broken into lines of at most `width` cells at spaces (a word longer than a line is cut).
/// A zero width counts as one; no line limit.
fn wrap(text: &str, width: usize) -> Vec<String> {
    widgets::wrap(text, width.max(1), usize::MAX, ' ')
}

// -------------------------------------------------------------------------------------------
// The command line, and the two ways to run it
// -------------------------------------------------------------------------------------------

/// `fn_tui --runs DIR [--focus PATH] [--pace xN] [--ascii] [--color MODE] [--frame [--size WxH]
/// [--keys SCRIPT] [--ansi]]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Cli {
    pub dir: PathBuf,
    /// Start on this campaign, attempt, run or log (`Browser::focus`).
    pub focus: Option<PathBuf>,
    pub pace: Option<u32>,
    pub ascii: bool,
    pub color: Option<ColorDepth>,
    pub frame: bool,
    pub size: (u16, u16),
    pub ansi: bool,
    pub keys: Vec<Step>,
}

/// Whether `args` ask for the run browser.
pub fn wanted(args: &[String]) -> bool {
    args.iter().any(|a| a == "--runs" || a.starts_with("--runs="))
}

/// Parses an invocation that has `--runs`. Everything else is refused by name: the browser takes
/// no `SEED NATION`, `--watch` or the like (a run is picked from the list).
pub fn parse(args: &[String]) -> Result<Cli, String> {
    let mut cli = Cli {
        dir: PathBuf::new(),
        focus: None,
        pace: None,
        ascii: false,
        color: None,
        frame: false,
        size: crate::cli::DEFAULT_SIZE,
        ansi: false,
        keys: Vec::new(),
    };
    let (mut dir, mut size_given, mut keys_given) = (None, false, false);
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_owned())),
            _ => (arg.as_str(), None),
        };
        let mut value = |name: &str| {
            inline
                .clone()
                .or_else(|| it.next().cloned())
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match flag {
            "--runs" => dir = Some(PathBuf::from(value("--runs")?)),
            "--frame" => cli.frame = true,
            "--focus" => cli.focus = Some(PathBuf::from(value("--focus")?)),
            "--ascii" => cli.ascii = true,
            "--ansi" => cli.ansi = true,
            "--size" => {
                let v = value("--size")?;
                let wh = v.split_once(['x', 'X']).and_then(|(w, h)| {
                    let (w, h) = (w.parse().ok()?, h.parse().ok()?);
                    (w > 0 && h > 0).then_some((w, h))
                });
                cli.size = wh.ok_or(format!("--size wants WxH (such as 120x40), got {v:?}"))?;
                size_given = true;
            }
            "--pace" => {
                let v = value("--pace")?;
                let n = v
                    .trim_start_matches(['x', 'X'])
                    .parse()
                    .ok()
                    .filter(|n| watch::PACES.contains(n));
                cli.pace =
                    Some(n.ok_or(format!("--pace wants one of {:?}, got {v:?}", watch::PACES))?);
            }
            "--color" => {
                let v = value("--color")?;
                cli.color = Some(
                    ColorDepth::parse(&v)
                        .ok_or(format!("--color wants truecolor, 256, 16 or never, got {v:?}"))?,
                );
            }
            "--keys" => {
                let v = value("--keys")?;
                cli.keys = keys::parse(&v).map_err(|e| format!("--keys: {e}"))?;
                keys_given = true;
            }
            f if f.starts_with('-') && f.len() > 1 => {
                return Err(format!("{f} can't be combined with --runs"));
            }
            other => return Err(format!("--runs takes no SEED, NATION or log; got {other:?}")),
        }
    }
    cli.dir = dir.ok_or("--runs needs a folder")?;
    for (flag, given) in [("--size", size_given), ("--keys", keys_given), ("--ansi", cli.ansi)] {
        if given && !cli.frame {
            return Err(format!("{flag} only means something with --frame"));
        }
    }
    Ok(cli)
}

/// `--runs DIR --frame`: one frame on stdout, no terminal. The scores are replayed first (the
/// scorecards of a big folder take a while), so the frame is the same every time.
pub fn frame(cli: &Cli, now: SystemTime, depth: ColorDepth) -> String {
    let mut browser =
        Browser::open(&cli.dir, now, Scoring::Inline, depth, cli.ascii).with_pace(cli.pace);
    if let Some(focus) = &cli.focus {
        browser.focus(focus);
    }
    browser.set_frame_size(cli.size);
    browser.play(&cli.keys);
    let buf = browser.render(cli.size);
    if cli.ansi { crate::frame::to_ansi(&buf) } else { crate::frame::to_text(&buf) }
}

#[cfg(not(target_arch = "wasm32"))]
/// `--runs DIR` in a terminal: the browser, until `q`. Enter opens the selected run in the
/// watcher on the same terminal.
pub fn run(cli: &Cli, depth: ColorDepth) -> std::io::Result<()> {
    use crossterm::event::{self, Event, KeyEventKind};
    use std::time::Instant;

    const WAIT: Duration = Duration::from_millis(100);

    let _restore = crate::run::Restore;
    let mut terminal = crate::run::setup()?;
    let workers = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(1, 4));
    let mut browser = Browser::open(
        &cli.dir,
        SystemTime::now(),
        Scoring::Background(Pool::new(workers)),
        depth,
        cli.ascii,
    )
    .with_pace(cli.pace);
    if let Some(focus) = &cli.focus {
        browser.focus(focus);
    }
    let mut last_refresh = Instant::now();
    while !browser.quit() {
        browser.collect_scores();
        terminal.draw(|f| {
            let area = f.area();
            let buf = browser.render((area.width, area.height));
            f.buffer_mut().clone_from(&buf);
        })?;
        if event::poll(WAIT)? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    if let Outcome::Open(dir) = browser.key(key) {
                        let log = log_of(&dir);
                        match watch::run_in(&mut terminal, &log, depth, cli.ascii, cli.pace) {
                            Err(e) => {
                                browser.set_notice(format!("can't watch {}: {e}", dir.display()))
                            }
                            // #587: the watch went on past lines that did not parse; say so.
                            Ok(skipped) if !skipped.is_empty() => browser.set_notice(format!(
                                "{} bad log line(s) skipped in {}; first: {}",
                                skipped.len(),
                                dir.display(),
                                skipped[0]
                            )),
                            Ok(_) => {}
                        }
                        // The watcher drew over the whole screen; draw the list from scratch.
                        terminal.clear()?;
                        browser.refresh(SystemTime::now());
                        last_refresh = Instant::now();
                    }
                }
                _ => {}
            }
        }
        if last_refresh.elapsed() >= REFRESH_EVERY {
            browser.refresh(SystemTime::now());
            last_refresh = Instant::now();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn the_command_line_takes_a_folder_and_the_watcher_options() {
        assert!(wanted(&args("--runs /r --frame")) && wanted(&args("--runs=/r")));
        assert!(!wanted(&args("7 earth --frame")));
        let c = parse(&args("--runs /r")).unwrap();
        assert_eq!((c.dir.as_path(), c.pace, c.frame), (Path::new("/r"), None, false));
        let c = parse(&args("--runs=/r --pace x10 --ascii --color 16")).unwrap();
        assert_eq!((c.pace, c.ascii, c.color), (Some(10), true, Some(ColorDepth::Ansi16)));
        let c = parse(&args("--runs /r --frame --size 100x30 --keys j")).unwrap();
        assert_eq!((c.size, c.keys.len()), ((100, 30), 1));
    }

    #[test]
    fn the_command_line_refuses_what_a_browser_cannot_do() {
        for (line, want) in [
            ("--frame", "--runs needs a folder"),
            ("--runs /r 7 earth", "takes no SEED"),
            ("--runs /r --watch l", "can't be combined"),
            ("--runs /r --scenario s", "can't be combined"),
            ("--runs /r --keys j", "only means something with --frame"),
            ("--runs /r --size 10x10", "only means something with --frame"),
            ("--runs /r --pace 7", "--pace wants one of"),
            ("--runs /r --size big --frame", "--size wants WxH"),
        ] {
            let e = parse(&args(line)).unwrap_err();
            assert!(e.contains(want), "{line}: {e}");
        }
    }

    #[test]
    fn wrapping_breaks_at_spaces_and_cuts_only_what_cannot_fit() {
        assert_eq!(wrap("aaa bbb ccc", 7), ["aaa bbb", "ccc"]);
        assert_eq!(wrap("aaaaaaaaaa b", 4), ["aaaa", "aaaa", "aa b"]);
        assert!(wrap("", 10).is_empty());
        assert_eq!(wrap("x", 0), ["x"], "a zero width is treated as one");
    }
}

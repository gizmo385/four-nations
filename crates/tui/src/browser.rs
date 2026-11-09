//! The run browser (#556): `fn_tui --runs DIR` lists the runs under `DIR` (what `agent-run` leaves
//! in `$FN_AGENT_RUNS`) with their attributes, and Enter opens the selected one in the watcher
//! (`crate::watch`); quitting the watcher returns here, on the same run. `cargo xtask agent-run
//! watch` opens it when run in a terminal with no `RUN_DIR`.
//!
//! The folder's metadata is `crate::runs`. Scores take a replay of each log, so they are never
//! waited for: the list opens at once with `...` in the score cells, and a small pool of threads
//! ([`Scoring::Background`]) fills them in as they finish, newest run first, reading and writing
//! the per-run score cache (`runs::CACHE_FILE`). Nothing here writes to a `run.jsonl`. The
//! folder is rescanned about once a second, so a live run's turn count moves while the browser
//! is open, and a run that is growing is scored again, at most every [`RESCORE_LIVE`].
//!
//! `--frame` draws it headless instead (no terminal, scores computed up front so the frame is
//! the same every time): `fn_tui --runs DIR --frame [--size WxH] [--keys SCRIPT]`. The keys
//! drive the list, and Enter opens the run's watcher frame in place (as `--watch LOG --frame`
//! would show it, at the end of the log so far), where `q` returns to the list. That is how the
//! tests drive the whole thing without a terminal.
//!
//! Only the interactive loop ([`run`]) reads the wall clock; everything else takes `now`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::{ColorDepth, Palette};
use crate::keys::{self, Step};
use crate::runs::{self, RunInfo, SCORE_HORIZON, Scored, State};
use crate::watch::{self, Watcher};

/// What a playtest's row says in the score column, and in the mode column (#576).
pub const PLAYTEST_CELL: &str = "playtest";

/// A run that is still growing is scored again no sooner than this after its last scoring.
pub const RESCORE_LIVE: Duration = Duration::from_secs(30);

/// How often the interactive loop rescans the runs folder.
const REFRESH_EVERY: Duration = Duration::from_secs(1);

/// What a key press in the list asks of whoever is running it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    Quit,
    /// Open this run folder in the watcher.
    Open(PathBuf),
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

/// Worker threads scoring run folders, and the answers coming back.
pub struct Pool {
    jobs: Sender<PathBuf>,
    done: Receiver<Done>,
}

struct Done {
    dir: PathBuf,
    bytes: u64,
    result: Result<Scored, String>,
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
                    let Ok(Ok(dir)) = job else { return };
                    let (bytes, result) = runs::score_run(&dir);
                    if done_tx.send(Done { dir, bytes, result }).is_err() {
                        return;
                    }
                }
            });
        }
        Pool { jobs, done }
    }
}

/// One row: a run, and what is known of its score.
struct Entry {
    info: RunInfo,
    /// The log length the score was asked for, when it was settled, and what it said. `None`
    /// while it is still being worked out.
    scored: Option<(u64, SystemTime, Result<Scored, String>)>,
    queued: bool,
    /// What the folder's log looked like when `info` was read.
    seen: (u64, Option<SystemTime>),
    /// Whether it looked live then.
    was_live: bool,
}

/// A run opened in a headless frame.
struct Watching {
    watcher: Box<Watcher>,
}

pub struct Browser {
    dir: PathBuf,
    entries: Vec<Entry>,
    sort: Sort,
    nation: Option<String>,
    /// The selected run's folder name: it stays selected through a rescan or a re-sort.
    selected: Option<String>,
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
        b.selected = b.visible().first().map(|&i| b.entries[i].info.name.clone());
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

    pub fn selected_name(&self) -> Option<&str> {
        self.selected.as_deref()
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

    // ----- the data -----

    /// Rescans the folder as of `now`: new runs appear, gone ones go, and a run whose log
    /// changed (or that stopped looking live) is read again. Asks for scores that are missing
    /// or out of date.
    pub fn refresh(&mut self, now: SystemTime) {
        self.now = now;
        let dirs = runs::run_dirs(&self.dir);
        self.entries.retain(|e| dirs.contains(&e.info.dir));
        for dir in dirs {
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
                    self.entries.push(Entry { info, scored: None, queued: false, seen, was_live });
                }
            }
        }
        self.entries.sort_by(|a, b| runs::newest_first(&a.info, &b.info));
        self.collect_scores();
        self.request_scores(now);
        self.keep_selection();
    }

    /// Takes in every score the workers have finished.
    pub fn collect_scores(&mut self) {
        let Scoring::Background(pool) = &self.scoring else { return };
        while let Ok(done) = pool.done.try_recv() {
            if let Some(e) = self.entries.iter_mut().find(|e| e.info.dir == done.dir) {
                e.queued = false;
                e.scored = Some((done.bytes, self.now, done.result));
            }
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
                Ok(done) => {
                    if let Some(e) = self.entries.iter_mut().find(|e| e.info.dir == done.dir) {
                        e.queued = false;
                        e.scored = Some((done.bytes, self.now, done.result));
                    }
                }
                Err(_) => return,
            }
        }
    }

    /// Whether any score is still being worked out.
    pub fn scoring_pending(&self) -> bool {
        self.entries.iter().any(|e| e.queued)
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
        match &self.scoring {
            Scoring::Background(pool) => {
                for dir in jobs {
                    let _ = pool.jobs.send(dir);
                }
            }
            Scoring::Inline => {
                for dir in jobs {
                    let (bytes, result) = runs::score_run(&dir);
                    if let Some(e) = self.entries.iter_mut().find(|e| e.info.dir == dir) {
                        e.queued = false;
                        e.scored = Some((bytes, now, result));
                    }
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
    /// line when it has one, else the replay's.
    fn score_value(&self, i: usize) -> Option<f64> {
        let e = &self.entries[i];
        if e.info.playtest {
            return None; // sorts with the unscored, never among the scored (#576)
        }
        let computed = match &e.scored {
            Some((_, _, Ok(s))) => Some(s.score),
            _ => None,
        };
        e.info.logged_score.as_ref().map(|s| s.score).or(computed)
    }

    fn keep_selection(&mut self) {
        let visible = self.visible();
        let still = self
            .selected
            .as_ref()
            .is_some_and(|name| visible.iter().any(|&i| &self.entries[i].info.name == name));
        if !still {
            self.selected = visible.first().map(|&i| self.entries[i].info.name.clone());
        }
    }

    fn selected_index(&self) -> Option<usize> {
        let name = self.selected.as_ref()?;
        self.entries.iter().position(|e| &e.info.name == name)
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
        let visible = self.visible();
        if visible.is_empty() {
            return;
        }
        let at =
            self.selected_index().and_then(|i| visible.iter().position(|&v| v == i)).unwrap_or(0)
                as isize;
        let to = at.saturating_add(by).clamp(0, visible.len() as isize - 1) as usize;
        self.selected = Some(self.entries[visible[to]].info.name.clone());
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
        let Some(i) = self.selected_index() else {
            self.notice = Some("no run selected".to_owned());
            return Outcome::Stay;
        };
        let e = &self.entries[i];
        if let Some(p) = e.info.problem.as_ref().filter(|p| p.unopenable) {
            self.notice = Some(format!("can't open {}: {}", e.info.name, p.why));
            return Outcome::Stay;
        }
        Outcome::Open(e.info.dir.clone())
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
        if let Outcome::Open(dir) = self.key(key) {
            self.open_in_frame(&dir);
        }
    }

    fn open_in_frame(&mut self, dir: &Path) {
        let log = dir.join("run.jsonl");
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
            Err(e) => self.notice = Some(format!("can't open {}: {e}", dir.display())),
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

    /// Every column, most valuable first. A narrow terminal gets a prefix of this list.
    fn columns(&self, rows: &[usize]) -> Vec<Column> {
        let cells = |f: &dyn Fn(&Entry) -> String| -> Vec<String> {
            rows.iter().map(|&i| f(&self.entries[i])).collect()
        };
        let or_dash = |o: Option<String>| o.unwrap_or_else(|| "-".to_owned());
        let readable = |e: &Entry| !e.info.problem.as_ref().is_some_and(|p| p.unopenable);
        vec![
            Column::new("STATE", false, cells(&|e| self.state_cell(e))),
            Column::new("SCORE@540", true, cells(&|e| self.score_cell(e))),
            Column::new(
                "TURNS",
                true,
                cells(&|e| if readable(e) { e.info.turns.to_string() } else { "-".to_owned() }),
            ),
            Column::new(
                "GAME",
                true,
                cells(&|e| or_dash(e.info.game_time().filter(|_| readable(e)))),
            ),
            Column::new(
                "SIGNS",
                true,
                cells(&|e| {
                    self.scorecard_cell(e, |s| format!("{}/{}", s.signs_met, s.signs_total))
                }),
            ),
            Column::new("DEAD", true, cells(&|e| self.scorecard_cell(e, |s| s.deaths.to_string()))),
            Column::new("NATION", false, cells(&|e| or_dash(e.info.nation.clone()))),
            Column::new("START", false, cells(&|e| or_dash(e.info.start_text()))),
            Column::new(
                "MODE",
                false,
                cells(&|e| {
                    if e.info.playtest {
                        return PLAYTEST_CELL.to_owned();
                    }
                    or_dash(e.info.mode.map(|m| m.word().to_owned()))
                }),
            ),
            Column::new(
                "COST",
                true,
                cells(&|e| or_dash(e.info.cost_usd.map(|c| format!("${c:.2}")))),
            ),
            Column::new("MODEL", false, cells(&|e| or_dash(e.info.model.clone()))),
            Column::new("DIFF", false, cells(&|e| or_dash(e.info.difficulty.clone()))),
            Column::new("STARTED", false, cells(&|e| or_dash(e.info.started_text()))),
        ]
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

        let rows = self.visible();
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
            let shown = rows.len();
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
        let name_w = rows
            .iter()
            .map(|&i| self.entries[i].info.name.chars().count())
            .chain(["RUN".len()])
            .max()
            .unwrap_or(3)
            .min(w.saturating_sub(marker).max(1));
        let all = self.columns(&rows);
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
            let sel =
                self.selected_index().and_then(|i| rows.iter().position(|&r| r == i)).unwrap_or(0);
            if sel < self.top {
                self.top = sel;
            } else if list_h > 0 && sel >= self.top + list_h {
                self.top = sel + 1 - list_h;
            }
            self.top = self.top.min(rows.len().saturating_sub(list_h.max(1)));
            for (n, &r) in rows.iter().enumerate().skip(self.top).take(list_h) {
                let e = &self.entries[r];
                let y = (header + n - self.top) as u16;
                let is_sel = n == sel;
                let mut line = format!(
                    "{}{}",
                    if is_sel { "> " } else { "  " },
                    pad_right(&self.clip(&e.info.name, name_w), name_w)
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
                "up/down j/k move   enter open   s sort   n nation   q quit".to_owned(),
                "j/k move  enter open  s sort  n nation  q quit".to_owned(),
                "enter open  q quit".to_owned(),
                "q quit".to_owned(),
            ],
            w,
        );
        buf.set_string(0, (h - 1) as u16, hints, dim);
    }

    /// What the status line says: a notice if there is one, else about the selected run.
    fn status_text(&self, rows: &[usize]) -> String {
        if let Some(n) = &self.notice {
            return n.clone();
        }
        let Some(e) = self.selected_index().map(|i| &self.entries[i]) else {
            return if self.scoring_pending() { "scoring...".to_owned() } else { String::new() };
        };
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
        let _ = rows;
        parts.join("; ")
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
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_owned();
        while word.chars().count() > width.max(1) {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            let head: String = word.chars().take(width.max(1)).collect();
            word = word.chars().skip(width.max(1)).collect();
            lines.push(head);
        }
        if line.is_empty() {
            line = word;
        } else if line.chars().count() + 1 + word.chars().count() <= width {
            line.push(' ');
            line.push_str(&word);
        } else {
            lines.push(std::mem::replace(&mut line, word));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

// -------------------------------------------------------------------------------------------
// The command line, and the two ways to run it
// -------------------------------------------------------------------------------------------

/// `fn_tui --runs DIR [--pace xN] [--ascii] [--color MODE] [--frame [--size WxH] [--keys SCRIPT]
/// [--ansi]]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Cli {
    pub dir: PathBuf,
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
    browser.set_frame_size(cli.size);
    browser.play(&cli.keys);
    let buf = browser.render(cli.size);
    if cli.ansi { crate::frame::to_ansi(&buf) } else { crate::frame::to_text(&buf) }
}

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
                        let log = dir.join("run.jsonl");
                        if let Err(e) =
                            watch::run_in(&mut terminal, &log, depth, cli.ascii, cli.pace)
                        {
                            browser.set_notice(format!("can't watch {}: {e}", dir.display()));
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

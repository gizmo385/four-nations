//! The hill-climb campaign behind `cargo xtask agent-run --track ID --attempts N` and
//! `--resume CAMPAIGN_DIR` (#530, docs/design/agent-hillclimb.md, "The attempt loop").
//!
//! A campaign is a folder under `$FN_AGENT_RUNS`:
//!
//! ```text
//! CAMPAIGN/
//!   ledger.json            the track as it was, and a row per finished attempt
//!   champion/handoff.json  the best strategy so far: scripts, plan and notes
//!   champion/meta.json     which attempt it came from, and its train mean
//!   champion/scorecard.txt its scorecard on the track's play_seed, for the next prompt
//!   attempt-K/             a normal --scripts run on play_seed, played to the horizon
//!     eval/seed-N.jsonl    the clean evaluation's logs; handoff.json is the attempt's own
//!                          final strategy
//! ```
//!
//! After every attempt the played log is scored (`fn_agent score --append`), the attempt's final
//! `governor` and plan are cleanly evaluated on every train and held-out seed
//! (`agent_eval::evaluate`), and one ledger row is written, last: a row is what makes an attempt
//! count, so an attempt a crash left unfinished has none and is left alone. If the mean train
//! score beats the champion's the attempt becomes the champion. Otherwise the champion stays,
//! the attempt's notes are merged into the champion's, and its code and plan are dropped.
//!
//! Held-out scores go in the ledger and the table printed for the owner, and in nothing the
//! model reads: [`ledger_text`] with `owner == false` is the only ledger text a prompt gets.

use std::fmt::Write as _;

use serde_json::{Value, json};

use super::*;
use crate::agent_eval::{self, Role, Strategy, Table, Track};

const LEDGER: &str = "ledger.json";
const CHAMPION: &str = "champion";
const GOVERNOR: &str = "governor";

// -------------------------------------------------------------------------------------------
// The ledger and the champion, as files
// -------------------------------------------------------------------------------------------

/// One seed's clean score. `stopped` is the reason the script stopped giving orders, if it did
/// (the score is still read at the horizon); `stopped_turn` and `stopped_why` say when and why
/// (#561). A ledger written before #561 has only `stopped`, and reads back with the other two
/// empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SeedScore {
    seed: u64,
    score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stopped: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stopped_turn: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stopped_why: Option<String>,
}

/// An attempt's ledger row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Row {
    attempt: u32,
    /// The final `governor`'s version; `None` when the attempt never saved one.
    governor_version: Option<u32>,
    /// Plan edits since the campaign began (the champion's count, plus this attempt's own).
    plan_revision: u32,
    /// The score of the attempt's own played log at the horizon.
    played: Option<f64>,
    /// The clean score per train seed, in `train_seeds` order (empty with no governor).
    clean: Vec<SeedScore>,
    train_mean: Option<f64>,
    /// The clean score per held-out seed. For the owner; no prompt ever shows it.
    held_out: Vec<SeedScore>,
    kept: bool,
    cost_usd: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Ledger {
    /// The track as the campaign started it: what `--resume` checks against the tracks file.
    track: Track,
    rows: Vec<Row>,
}

/// `champion/meta.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Champion {
    attempt: u32,
    train_mean: f64,
    governor_version: Option<u32>,
    plan_revision: u32,
}

/// Writes `text` to `path` by way of a sibling file, so a crash leaves the old file or the new
/// one, never half of either.
fn write_atomic(path: &Path, text: &str) -> Result {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text).map_err(|e| format!("writing {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("writing {}: {e}", path.display()))
}

fn write_json(path: &Path, value: &impl Serialize) -> Result {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())? + "\n";
    write_atomic(path, &text)
}

fn write_ledger(dir: &Path, ledger: &Ledger) -> Result {
    write_json(&dir.join(LEDGER), ledger)
}

fn read_ledger(dir: &Path) -> Result<Ledger> {
    let path = dir.join(LEDGER);
    let text = fs::read_to_string(&path).map_err(|_| {
        format!(
            "{} has no {LEDGER}: it isn't a campaign folder (start one with --track ID \
             --attempts N)",
            dir.display()
        )
    })?;
    serde_json::from_str(&text).map_err(|e| format!("{} isn't a ledger: {e}", path.display()))
}

fn champion_dir(dir: &Path) -> PathBuf {
    dir.join(CHAMPION)
}

fn champion_handoff_path(dir: &Path) -> PathBuf {
    champion_dir(dir).join("handoff.json")
}

fn read_handoff(path: &Path) -> Result<Handoff> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{} isn't a handoff: {e}", path.display()))
}

/// The current champion: `None` before any attempt has won.
fn read_champion(dir: &Path) -> Result<Option<Champion>> {
    let meta = champion_dir(dir).join("meta.json");
    if !meta.is_file() {
        if champion_handoff_path(dir).is_file() {
            return Err(format!(
                "{} has a handoff but no meta.json: the champion folder is damaged",
                champion_dir(dir).display()
            ));
        }
        return Ok(None);
    }
    let text = fs::read_to_string(&meta).map_err(|e| e.to_string())?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| format!("{} isn't a champion record: {e}", meta.display()))
}

/// Writes the champion's files. `meta.json` goes last: it is the record that says there is one.
fn write_champion(dir: &Path, handoff: &Handoff, champion: &Champion, scorecard: &str) -> Result {
    let cdir = champion_dir(dir);
    fs::create_dir_all(&cdir).map_err(|e| format!("creating {}: {e}", cdir.display()))?;
    write_atomic(&cdir.join("scorecard.txt"), scorecard)?;
    write_json(&champion_handoff_path(dir), handoff)?;
    write_json(&cdir.join("meta.json"), champion)
}

// -------------------------------------------------------------------------------------------
// Selection
// -------------------------------------------------------------------------------------------

/// Whether a train mean takes the champion's place: strictly better, or the first with any.
/// Runs are deterministic, so any difference on the same seeds is real, not noise.
fn beats(mean: f64, champion: Option<f64>) -> bool {
    champion.is_none_or(|best| mean > best)
}

/// The champion's lab notes followed by whichever of the attempt's aren't there already (an
/// attempt that started from the champion has them all as its first notes).
fn merge_notes(champion: &[String], attempt: &[String]) -> Vec<String> {
    let mut out = champion.to_vec();
    for note in attempt {
        if !out.contains(note) {
            out.push(note.clone());
        }
    }
    out
}

/// What `track` and `now` disagree on, ignoring the display name: empty when scores earned under
/// one are comparable with scores under the other.
fn track_diff(track: &Track, now: &Track) -> Vec<String> {
    let (a, b) = (json!(track), json!(now));
    let (a, b) =
        (a.as_object().cloned().unwrap_or_default(), b.as_object().cloned().unwrap_or_default());
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|k| k.as_str() != "name" && a.get(*k) != b.get(*k))
        .map(|k| {
            let show = |m: &serde_json::Map<String, Value>| {
                m.get(k).map_or("absent".to_owned(), Value::to_string)
            };
            format!("{k} was {}, is now {}", show(&a), show(&b))
        })
        .collect()
}

/// The attempt number after every one in the ledger and every `attempt-K` folder on disk (an
/// unfinished attempt has a folder and no row; its folder is never reused).
fn next_attempt(dir: &Path, ledger: &Ledger) -> u32 {
    let from_rows = ledger.rows.iter().map(|r| r.attempt).max().unwrap_or(0);
    let from_dirs = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str()?.strip_prefix("attempt-")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    from_rows.max(from_dirs) + 1
}

// -------------------------------------------------------------------------------------------
// The ledger as text
// -------------------------------------------------------------------------------------------

fn seeds_label(seeds: &[u64]) -> String {
    seeds.iter().map(u64::to_string).collect::<Vec<_>>().join("/")
}

fn scores_cell(scores: &[SeedScore]) -> String {
    if scores.is_empty() {
        return "-".to_owned();
    }
    // A `*` marks a clean run whose script was stopped: its score is a coast, not a strategy.
    let cell =
        |s: &SeedScore| format!("{:.0}{}", s.score, if s.stopped.is_some() { "*" } else { "" });
    scores.iter().map(cell).collect::<Vec<_>>().join(" / ")
}

/// The longest a stop's `why` runs in a ledger line.
const WHY_MAX: usize = 100;

/// One short line per attempt whose clean runs on the train seeds were stopped (#561): the
/// reason(s), how many seeds, the turn range, and the first one's `why`. Reads `clean` only, so
/// no held-out seed's stop can reach a line (the prompt's ledger text is built from these).
fn stop_lines(rows: &[Row]) -> Vec<String> {
    let mut lines = Vec::new();
    for r in rows {
        let stopped: Vec<&SeedScore> = r.clean.iter().filter(|s| s.stopped.is_some()).collect();
        let Some(first) = stopped.first() else { continue };
        let mut reasons: Vec<&str> = stopped.iter().filter_map(|s| s.stopped.as_deref()).collect();
        reasons.sort_unstable();
        reasons.dedup();
        let turns: Vec<u32> = stopped.iter().filter_map(|s| s.stopped_turn).collect();
        let at = match (turns.iter().min(), turns.iter().max()) {
            (Some(lo), Some(hi)) if lo == hi => format!(" at turn {lo}"),
            (Some(lo), Some(hi)) => format!(" at turns {lo}-{hi}"),
            _ => String::new(),
        };
        let why = first.stopped_why.as_deref().map(|w| {
            let w = w.split_whitespace().collect::<Vec<_>>().join(" ");
            let cut: String = w.chars().take(WHY_MAX).collect();
            let dots = if w.chars().count() > WHY_MAX { "..." } else { "" };
            format!("; first (seed {}): \"{cut}{dots}\"", first.seed)
        });
        lines.push(format!(
            "attempt {}: {} stopped {} of {} clean runs{at}{}",
            r.attempt,
            reasons.join("/"),
            stopped.len(),
            r.clean.len(),
            why.unwrap_or_default(),
        ));
    }
    lines
}

/// The ledger as a table. With `owner` it has the held-out columns and each attempt's cost; the
/// text a prompt gets is `ledger_text(ledger, false)`, which has neither.
fn ledger_text(ledger: &Ledger, owner: bool) -> String {
    let t = &ledger.track;
    let mut head = vec![
        "#".to_owned(),
        "governor".to_owned(),
        "plan".to_owned(),
        "played".to_owned(),
        format!("clean {}", seeds_label(&t.train_seeds)),
        "mean".to_owned(),
    ];
    if owner {
        head.push(format!("held-out {}", seeds_label(&t.holdout_seeds)));
    }
    head.push("kept".to_owned());
    if owner {
        head.push("cost".to_owned());
    }
    let mut table = vec![head];
    for r in &ledger.rows {
        let mut cells = vec![
            r.attempt.to_string(),
            r.governor_version.map_or("-".to_owned(), |v| format!("v{v}")),
            format!("r{}", r.plan_revision),
            r.played.map_or("-".to_owned(), |p| format!("{p:.0}")),
            scores_cell(&r.clean),
            r.train_mean.map_or("-".to_owned(), |m| format!("{m:.0}")),
        ];
        if owner {
            cells.push(scores_cell(&r.held_out));
        }
        cells.push(if r.kept { "champ" } else { "no" }.to_owned());
        if owner {
            cells.push(format!("${:.2}", r.cost_usd));
        }
        table.push(cells);
    }
    let cols = table[0].len();
    let widths: Vec<usize> = (0..cols)
        .map(|c| table.iter().map(|row| row[c].chars().count()).max().unwrap_or(0))
        .collect();
    let kept_col = cols - if owner { 2 } else { 1 };
    let mut out = String::new();
    let note = if owner { "                    (held-out: owner only)" } else { "" };
    let _ = writeln!(out, "ledger · {} · horizon {}{note}", t.id, t.horizon_turns);
    for row in &table {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(c, cell)| {
                if c == kept_col {
                    format!("{cell:<w$}", w = widths[c])
                } else {
                    format!("{cell:>w$}", w = widths[c])
                }
            })
            .collect();
        let _ = writeln!(out, " {}", line.join("  ").trim_end());
    }
    let any_stop = ledger
        .rows
        .iter()
        .flat_map(|r| r.clean.iter().chain(&r.held_out))
        .any(|s| s.stopped.is_some());
    if any_stop {
        let _ = writeln!(
            out,
            " * the governor stopped (an error or a runaway) and the colony coasted with no \
             orders from that turn on"
        );
    }
    for line in stop_lines(&ledger.rows) {
        let _ = writeln!(out, " {line}");
    }
    out
}

/// What every attempt's prompt says about the clean run and `test_governor` (#561).
fn clean_run_rules() -> &'static str {
    "How the clean run ends matters. If the governor stops (an error, or a runaway: more work \
     between two waits than the budget allows; each wait starts the count again), it stays \
     stopped: from that turn the colony coasts to the end \
     with no orders at all, so a stop early in the run costs most of the score. `test_governor` \
     plays your saved governor and the current plan from turn 0 on one of the evaluation maps, \
     on a separate copy of the world (this game is not touched), and tells you the stop reason, \
     turn and score the clean run will get. It takes a few seconds to half a minute and the number of \
     tests per attempt is limited. Run it on the final governor before you finish, most of all \
     after a change: a governor that has only run on this built-up settlement can fail from an \
     empty start."
}
// -------------------------------------------------------------------------------------------
// The attempt's prompt
// -------------------------------------------------------------------------------------------

/// The first prompt of an attempt that starts from a champion: what the campaign is, the ledger's
/// train columns (never the held-out ones), the champion's scorecard on the play seed, and the
/// one ask. It sits on top of [`default_script_prompt`], so the governor and plan rules, and
/// anything else that prompt teaches, still apply.
fn attempt_prompt(
    ledger: &Ledger,
    attempt: u32,
    champion: &Champion,
    scorecard: &str,
    total: u32,
    calls: u32,
) -> String {
    let t = &ledger.track;
    format!(
        "This is attempt {attempt} of a hill-climbing campaign on \"{name}\". Your governor, plan \
         and lab notes from the best attempt so far (attempt {best}) are already loaded: scripts, \
         plan and notes show them, so nothing needs retyping. Read the governor's source with \
         scripts {{\"name\": \"{governor}\"}} before you change anything.\n\n\
         Each attempt is scored twice. The played score is this run's. The clean score is the one \
         that decides: after the run, your final governor and plan are replayed from turn 0 with \
         no one at the keys, on several different maps, and the scores are averaged. A governor \
         that only works on the map you can see scores badly there, so improve the governor and \
         the plan, not just this settlement.\n\n\
         The ledger so far (clean scores per map, `*` for a stopped governor; the mean is what \
         has to go up):\n{ledger}\n\
         The best strategy's scorecard on your map:\n{scorecard}\n\n{rules}\n\n\
         Your one job this attempt: raise the clean score by improving the governor and the \
         plan. The governor is already loaded, so run it from the start instead of building one \
         up. Watch what surprises it and fix that in the plan or the governor, and save the \
         governor when you change it. Revise the governor you read: keep its structure, change \
         only the lines that need it (a small diff against the saved version), and save it under \
         the same name. Don't rewrite it from scratch; a rewrite throws away what earlier \
         attempts got working. The notes say what earlier attempts learned.\n\n{base}",
        name = t.name,
        governor = GOVERNOR,
        best = champion.attempt,
        ledger = ledger_text(ledger, false),
        scorecard = scorecard.trim_end(),
        rules = clean_run_rules(),
        base = default_script_prompt(total, calls),
    )
}

// -------------------------------------------------------------------------------------------
// Running
// -------------------------------------------------------------------------------------------

/// What every attempt shares.
struct Campaign<'a> {
    repo: &'a Path,
    dir: PathBuf,
    ledger: Ledger,
    fn_agent: PathBuf,
    opts: &'a Opts,
}

pub(super) fn run(repo: &Path, opts: &Opts) -> Result {
    let tracks_path = opts.tracks_file.clone().unwrap_or_else(|| repo.join("data/tracks.toml"));
    let (dir, ledger, fresh) = match (&opts.track, &opts.resume_dir) {
        (Some(id), _) => {
            let track = agent_eval::load_track(&tracks_path, id)?;
            let dir = new_campaign_dir(&runs_dir(), &track.id)?;
            let ledger = Ledger { track, rows: Vec::new() };
            write_ledger(&dir, &ledger)?;
            (dir, ledger, true)
        }
        (None, Some(dir)) => (dir.clone(), open_campaign(dir, &tracks_path)?, false),
        (None, None) => return Err("agent-run campaign: --track or --resume is required".into()),
    };
    let attempts = opts.attempts.unwrap_or(1);

    let fn_agent = match std::env::var_os("FN_AGENT_BIN") {
        Some(bin) => PathBuf::from(bin),
        None => {
            cargo_build_release(repo, &["fn_agent"])?;
            repo.join("target/release/fn_agent")
        }
    };

    let t = &ledger.track;
    if fresh {
        say(&format!("campaign folder: {}", dir.display()));
    } else {
        say(&format!("resuming {} ({} attempt(s) so far)", dir.display(), ledger.rows.len()));
    }
    say(&format!(
        "track {}: {} {} seed {}, horizon {} turns; {attempts} attempt(s){}",
        t.id,
        t.nation,
        t.difficulty,
        t.play_seed,
        t.horizon_turns,
        opts.max_usd.map_or(String::new(), |m| format!(", at most ${m}"))
    ));

    let mut campaign = Campaign { repo, dir, ledger, fn_agent, opts };
    campaign.climb(attempts)?;
    print!("{}", ledger_text(&campaign.ledger, true));
    say(&format!("ledger: {}", campaign.dir.join(LEDGER).display()));
    if let Some(c) = read_champion(&campaign.dir)? {
        say(&format!(
            "champion: attempt {} (train mean {:.0}), in {}",
            c.attempt,
            c.train_mean,
            champion_dir(&campaign.dir).display()
        ));
    }
    Ok(())
}

/// A fresh campaign folder under `runs`, named for the track and the time.
fn new_campaign_dir(runs: &Path, track_id: &str) -> Result<PathBuf> {
    let base = format!("{}-campaign-{track_id}", timestamp());
    for n in 1u32.. {
        let name = if n == 1 { base.clone() } else { format!("{base}-{n}") };
        let dir = runs.join(name);
        if dir.exists() {
            continue;
        }
        fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        return Ok(dir);
    }
    unreachable!("an unused folder name turns up")
}

/// `--resume DIR`: the ledger, checked against the tracks file and the champion folder. Refuses
/// what would make the new scores incomparable with the old.
fn open_campaign(dir: &Path, tracks_path: &Path) -> Result<Ledger> {
    let ledger = read_ledger(dir)?;
    let id = &ledger.track.id;
    let now = agent_eval::load_track(tracks_path, id).map_err(|e| {
        format!("{} was climbing track {id:?}, which can't be loaded now: {e}", dir.display())
    })?;
    let diff = track_diff(&ledger.track, &now);
    if !diff.is_empty() {
        return Err(format!(
            "can't resume {}: track {id:?} has changed in {} since the campaign began ({}); \
             scores from before wouldn't be comparable with new ones. Start a new campaign with \
             --track instead.",
            dir.display(),
            tracks_path.display(),
            diff.join("; ")
        ));
    }
    if read_champion(dir)?.is_none() && ledger.rows.iter().any(|r| r.kept) {
        return Err(format!(
            "can't resume {}: the ledger has a kept attempt but {} has no champion",
            dir.display(),
            champion_dir(dir).display()
        ));
    }
    Ok(ledger)
}

impl Campaign<'_> {
    /// Plays up to `attempts` more attempts, stopping early once `--max-usd` is spent.
    fn climb(&mut self, attempts: u32) -> Result {
        let mut spent = 0.0;
        for done in 0..attempts {
            if let Some(cap) = self.opts.max_usd
                && spent >= cap
            {
                say(&format!(
                    "stopping: ${spent:.2} spent of the ${cap} cap, after {done} of {attempts} \
                     attempt(s)"
                ));
                break;
            }
            let remaining = self.opts.max_usd.map(|cap| cap - spent);
            let row = self.attempt(remaining)?;
            spent += row.cost_usd;
        }
        Ok(())
    }

    /// One attempt, start to ledger row.
    fn attempt(&mut self, remaining: Option<f64>) -> Result<Row> {
        let track = self.ledger.track.clone();
        let k = next_attempt(&self.dir, &self.ledger);
        let adir = self.dir.join(format!("attempt-{k}"));
        fs::create_dir(&adir).map_err(|e| format!("creating {}: {e}", adir.display()))?;
        let champion = read_champion(&self.dir)?;
        let champion_handoff = match &champion {
            Some(_) => Some(read_handoff(&champion_handoff_path(&self.dir))?),
            None => None,
        };

        // Play. The first attempt (or the first with a champion) differs from the rest only in
        // what it starts with.
        let first_prompt = match &champion {
            Some(c) => {
                let card = fs::read_to_string(champion_dir(&self.dir).join("scorecard.txt"))
                    .unwrap_or_default();
                Some(attempt_prompt(
                    &self.ledger,
                    k,
                    c,
                    &card,
                    track.horizon_turns,
                    self.opts.chunk_calls,
                ))
            }
            // No champion yet: the plain scripted prompt, with what a clean run is up against.
            None => Some(format!(
                "{}\n\n{}",
                clean_run_rules(),
                default_script_prompt(track.horizon_turns, self.opts.chunk_calls)
            )),
        };
        let o = attempt_opts(
            self.opts,
            &track,
            remaining,
            champion.as_ref().map(|_| champion_handoff_path(&self.dir)),
            first_prompt,
        );
        say(&format!(
            "attempt {k}: {} on seed {}, {} turns{}",
            track.id,
            track.play_seed,
            track.horizon_turns,
            if champion.is_some() { ", from the champion" } else { "" }
        ));
        let mcp_config = adir.join("mcp.json");
        let claude_args = base_claude_args(&mcp_config, o.model.as_deref());
        run_scripts_chunked(self.repo, &adir, &self.fn_agent, &o, &claude_args)?;

        // Score what was played.
        let log = adir.join("run.jsonl");
        let played = score_played(&self.fn_agent, &log, track.horizon_turns);
        say(&format!(
            "attempt {k}: played score {}",
            played.map_or("unavailable".to_owned(), |p| format!("{p:.0}"))
        ));

        // The attempt's final strategy, and the clean evaluation of it.
        let lines = read_log_lines(&log).unwrap_or_default();
        let (scripts, notes, plan) = parse_library(&lines);
        let handoff = Handoff { scripts, notes, plan };
        write_json(&adir.join("handoff.json"), &handoff)?;
        let governor = handoff.scripts.get(GOVERNOR).filter(|g| !g.code.trim().is_empty());
        let table = match governor {
            Some(g) => {
                let strategy = Strategy { governor: g.code.clone(), plan: handoff.plan.clone() };
                Some(
                    agent_eval::evaluate(&self.fn_agent, &track, &strategy, &adir.join("eval"))
                        .map_err(|e| format!("attempt {k}: the clean evaluation failed: {e}"))?,
                )
            }
            None => {
                say(&format!("attempt {k}: no script named \"{GOVERNOR}\" was saved; not kept"));
                None
            }
        };

        // Select.
        let train_mean = table.as_ref().and_then(Table::train_mean);
        let champion_mean = champion.as_ref().map(|c| c.train_mean);
        let kept = train_mean.is_some_and(|m| beats(m, champion_mean));
        let prior_plan_revision = champion.as_ref().map_or(0, |c| c.plan_revision);
        let preloaded_plan = champion_handoff.as_ref().is_some_and(|h| h.plan.is_some());
        let plan_revision =
            prior_plan_revision + plan_edits(&lines).saturating_sub(u32::from(preloaded_plan));
        let governor_version = governor.map(|g| g.version);
        let old_notes = champion_handoff.as_ref().map_or(Vec::new(), |h| h.notes.clone());
        let merged = merge_notes(&old_notes, &handoff.notes);
        if kept {
            let card_log = {
                let clean = adir.join("eval").join(format!("seed-{}.jsonl", track.play_seed));
                if clean.is_file() { clean } else { log.clone() }
            };
            let record = Champion {
                attempt: k,
                train_mean: train_mean.unwrap_or_default(),
                governor_version,
                plan_revision,
            };
            let card = scorecard_text(&self.fn_agent, &card_log, track.horizon_turns);
            write_champion(
                &self.dir,
                &Handoff { notes: merged, ..handoff.clone() },
                &record,
                &card,
            )?;
        } else if let Some(mut stays) = champion_handoff {
            // The idea is kept so the next attempt doesn't retry it; its code and plan aren't.
            stays.notes = merged;
            write_json(&champion_handoff_path(&self.dir), &stays)?;
        }

        let row = Row {
            attempt: k,
            governor_version,
            plan_revision,
            played,
            clean: table.as_ref().map_or(Vec::new(), |t| seed_scores(t, Role::Train)),
            train_mean,
            held_out: table.as_ref().map_or(Vec::new(), |t| seed_scores(t, Role::Holdout)),
            kept,
            cost_usd: cost_so_far(&adir),
        };
        say(&format!(
            "attempt {k}: clean mean {}; {}",
            train_mean.map_or("-".to_owned(), |m| format!("{m:.0}")),
            if kept { "new champion" } else { "champion stays" }
        ));
        self.ledger.rows.push(row.clone());
        write_ledger(&self.dir, &self.ledger)?;
        Ok(row)
    }
}

/// The run options for one attempt: `base` (the caller's `--model`, `--chunk-calls`, ...) with
/// the track's start filled in.
fn attempt_opts(
    base: &Opts,
    track: &Track,
    remaining: Option<f64>,
    carry_in: Option<PathBuf>,
    first_prompt: Option<String>,
) -> Opts {
    let mut o = base.clone();
    o.seed = track.play_seed;
    o.seed_given = true;
    o.nation = track.nation.clone();
    o.nation_given = true;
    o.difficulty = track.difficulty.clone();
    o.scripts = true;
    o.turns = track.horizon_turns;
    o.max_usd = remaining;
    o.map_size = Some(track.map_size.clone());
    o.site = track.site_flag().ok().flatten().map(str::to_owned);
    o.prelude = track.prelude.as_ref().map(|p| format!("{}:{}", p.opening, p.minutes));
    o.carry_in = carry_in;
    o.first_prompt = first_prompt;
    // The train seeds only: a held-out seed never reaches the model's side of the harness.
    o.trial = Some((track.train_seeds.clone(), track.horizon_turns));
    o
}

fn seed_scores(table: &Table, role: Role) -> Vec<SeedScore> {
    table
        .rows
        .iter()
        .filter(|r| r.role == role)
        .map(|r| SeedScore {
            seed: r.seed,
            score: r.score,
            stopped: r.stopped.as_ref().map(|s| s.reason.clone()),
            stopped_turn: r.stopped.as_ref().map(|s| u32::try_from(s.turn).unwrap_or(u32::MAX)),
            stopped_why: r.stopped.as_ref().and_then(|s| s.why.clone()),
        })
        .collect()
}

/// How many times the log's plan was set or merged (a `plan` line each).
fn plan_edits(lines: &[LogLine]) -> u32 {
    lines.iter().filter(|l| matches!(l, LogLine::Plan { .. })).count() as u32
}

/// `fn_agent score LOG --horizon N --json --append`: the played score, and a `score` line on the
/// log. `None` when there is no log or the command fails (the clean evaluation still runs).
fn score_played(fn_agent: &Path, log: &Path, horizon: u32) -> Option<f64> {
    if !log.is_file() {
        return None;
    }
    let out = Command::new(fn_agent)
        .arg("score")
        .arg(log)
        .args(["--horizon", &horizon.to_string(), "--json", "--append"])
        .output()
        .ok()?;
    if !out.status.success() {
        say(&format!(
            "fn_agent score failed on {}: {}",
            log.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
        return None;
    }
    let v: Value = serde_json::from_slice(&out.stdout).ok()?;
    v["score"].as_f64()
}

/// The text scorecard (`fn_agent score LOG --horizon N`) of one log, for the next prompt.
fn scorecard_text(fn_agent: &Path, log: &Path, horizon: u32) -> String {
    let out = Command::new(fn_agent)
        .arg("score")
        .arg(log)
        .args(["--horizon", &horizon.to_string()])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => "(no scorecard could be built for it)\n".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> Track {
        agent_eval::parse_tracks(
            "[[track]]\nid = \"t\"\nname = \"Tee\"\nnation = \"water\"\ndifficulty = \"normal\"\n\
             map_size = \"small\"\nsite = \"default\"\nhorizon_turns = 5\nplay_seed = 1\n\
             train_seeds = [1, 2]\nholdout_seeds = [3, 4]\n",
        )
        .unwrap()
        .remove(0)
    }

    fn seeds(xs: &[(u64, f64)]) -> Vec<SeedScore> {
        xs.iter()
            .map(|&(seed, score)| SeedScore {
                seed,
                score,
                stopped: None,
                stopped_turn: None,
                stopped_why: None,
            })
            .collect()
    }

    fn ledger() -> Ledger {
        Ledger {
            track: track(),
            rows: vec![
                Row {
                    attempt: 1,
                    governor_version: Some(9),
                    plan_revision: 6,
                    played: Some(256.4),
                    clean: seeds(&[(1, 231.0), (2, 174.0)]),
                    train_mean: Some(202.5),
                    held_out: seeds(&[(3, 7771.0), (4, 7772.0)]),
                    kept: true,
                    cost_usd: 0.974,
                },
                Row {
                    attempt: 2,
                    governor_version: None,
                    plan_revision: 6,
                    played: None,
                    clean: Vec::new(),
                    train_mean: None,
                    held_out: Vec::new(),
                    kept: false,
                    cost_usd: 0.1,
                },
            ],
        }
    }

    #[test]
    fn the_owner_table_has_held_out_and_cost() {
        let t = ledger_text(&ledger(), true);
        assert!(
            t.contains("held-out 3/4") && t.contains("7771 / 7772") && t.contains("$0.97"),
            "{t}"
        );
        assert!(t.contains("clean 1/2") && t.contains("231 / 174") && t.contains("champ"), "{t}");
        assert!(t.contains("v9") && t.contains("r6") && t.contains("256"), "{t}");
        // An attempt with no governor shows dashes, not zeros.
        let row2 = t.lines().last().unwrap();
        assert!(row2.contains('-') && row2.contains("no"), "{t}");
    }

    #[test]
    fn the_prompt_table_never_shows_held_out_or_cost() {
        let t = ledger_text(&ledger(), false);
        for needle in ["held", "7771", "7772", "3/4", "$", "cost", "owner"] {
            assert!(!t.contains(needle), "{needle:?} in\n{t}");
        }
        assert!(t.contains("231 / 174") && t.contains("202"), "{t}");
    }

    #[test]
    fn the_attempt_prompt_builds_on_the_script_prompt_and_hides_held_out() {
        let l = ledger();
        let c =
            Champion { attempt: 1, train_mean: 202.5, governor_version: Some(9), plan_revision: 6 };
        let p = attempt_prompt(&l, 3, &c, "Water seed 1\nscore 231\n", 5, 30);
        assert!(p.contains("attempt 3") && p.contains("(attempt 1)"), "{p}");
        assert!(p.contains("raise the clean score"), "{p}");
        assert!(p.contains("score 231") && p.contains("231 / 174"), "{p}");
        assert!(
            p.ends_with(&default_script_prompt(5, 30)),
            "the script prompt comes through whole"
        );
        for needle in ["7771", "7772", "held-out"] {
            assert!(!p.contains(needle), "{needle:?} in\n{p}");
        }
    }

    fn stopped(seed: u64, score: f64, reason: &str, turn: u32, why: Option<&str>) -> SeedScore {
        SeedScore {
            seed,
            score,
            stopped: Some(reason.to_owned()),
            stopped_turn: Some(turn),
            stopped_why: why.map(str::to_owned),
        }
    }

    /// A ledger whose attempt 1 had both train runs stopped and both held-out runs stopped for a
    /// reason, turn and why that appear nowhere else.
    fn stopped_ledger() -> Ledger {
        let mut l = ledger();
        l.rows[0].clean = vec![
            stopped(1, 101.0, "runaway", 171, Some("a loop with no wait?")),
            stopped(2, 51.0, "runaway", 195, None),
        ];
        l.rows[0].held_out = vec![
            stopped(3, 77.0, "error", 4242, Some("HELDOUT_WHY secret")),
            stopped(4, 78.0, "error", 4243, Some("HELDOUT_WHY secret")),
        ];
        l
    }

    #[test]
    fn the_ledger_text_says_why_clean_runs_stopped_in_one_line_per_attempt() {
        let t = ledger_text(&stopped_ledger(), false);
        assert!(t.contains("101* / 51*"), "{t}");
        assert!(t.contains("coasted with no orders"), "{t}");
        let line = t.lines().find(|l| l.contains("attempt 1: ")).expect(&t);
        assert_eq!(
            line.trim(),
            "attempt 1: runaway stopped 2 of 2 clean runs at turns 171-195; first (seed 1): \
             \"a loop with no wait?\""
        );
        assert_eq!(t.lines().filter(|l| l.contains("attempt ")).count(), 1, "{t}");
        // An attempt whose runs were not stopped has no line, and no star.
        assert!(!ledger_text(&ledger(), false).contains('*'));
    }

    #[test]
    fn a_stop_line_handles_one_turn_mixed_reasons_long_whys_and_old_ledgers() {
        let mut row = ledger().rows.remove(0);
        row.clean = vec![
            stopped(1, 1.0, "error", 1, Some(&format!("line one\n{}", "x".repeat(300)))),
            stopped(2, 1.0, "runaway", 1, None),
        ];
        let line = stop_lines(&[row.clone()]).remove(0);
        assert!(
            line.contains("error/runaway stopped 2 of 2") && line.contains("at turn 1;"),
            "{line}"
        );
        assert!(line.contains("line one xxx") && line.ends_with("...\""), "{line}");
        assert!(line.chars().count() < 220, "{line}");
        // Written before #561: the reason only.
        let old: SeedScore =
            serde_json::from_str(r#"{"seed": 7, "score": 10.5, "stopped": "runaway"}"#).unwrap();
        assert_eq!((old.stopped_turn, old.stopped_why.clone()), (None, None));
        row.clean = vec![old];
        assert_eq!(stop_lines(&[row]), ["attempt 1: runaway stopped 1 of 1 clean runs"]);
        // And a score with no stop at all still reads.
        let bare: SeedScore = serde_json::from_str(r#"{"seed": 7, "score": 10.5}"#).unwrap();
        assert!(bare.stopped.is_none());
    }

    #[test]
    fn the_stop_details_round_trip_through_the_ledger_file() {
        let l = stopped_ledger();
        let text = serde_json::to_string(&l).unwrap();
        assert!(text.contains("\"stopped_turn\":171"));
        assert_eq!(serde_json::from_str::<Ledger>(&text).unwrap(), l);
    }

    /// A held-out seed's stop reason, turn and why never reach the prompt; the owner's table marks
    /// the stop but gives no held-out detail either.
    #[test]
    fn held_out_stops_never_reach_the_prompt() {
        let l = stopped_ledger();
        let c =
            Champion { attempt: 1, train_mean: 76.0, governor_version: Some(9), plan_revision: 6 };
        let p = attempt_prompt(&l, 3, &c, "card\n", 5, 30);
        for needle in ["HELDOUT_WHY", "4242", "4243", "error stopped", "78", "77"] {
            assert!(!p.contains(needle), "{needle:?} in\n{p}");
        }
        assert!(p.contains("runaway stopped 2 of 2 clean runs at turns 171-195"), "{p}");
        // The owner's table marks held-out stops with a star too.
        let owner = ledger_text(&l, true);
        assert!(owner.contains("77* / 78*") && owner.contains("101* / 51*"), "{owner}");
        assert!(!owner.contains("HELDOUT_WHY"), "{owner}");
    }

    #[test]
    fn the_prompt_says_a_stopped_clean_run_coasts_and_names_the_test_tool() {
        let c =
            Champion { attempt: 1, train_mean: 76.0, governor_version: Some(9), plan_revision: 6 };
        let p = attempt_prompt(&ledger(), 3, &c, "card\n", 5, 30);
        assert!(p.contains("coasts to the end with no orders at all"), "{p}");
        assert!(p.contains("costs most of the score") && p.contains("test_governor"), "{p}");
        assert!(p.contains("from turn 0") && p.contains("this game is not touched"), "{p}");
    }

    #[test]
    fn attempt_options_hand_the_tool_the_train_seeds_and_nothing_else() {
        let t = track();
        let base = parse_args_with(&["--chunk-calls", "7"], &mut || 1).unwrap();
        let o = attempt_opts(&base, &t, None, None, None);
        assert_eq!(o.trial, Some((vec![1, 2], 5)));
        let args = serve_args(&o, Path::new("/a/run.jsonl")).join(" ");
        assert!(args.contains("--trial-seeds 1,2 --trial-turns 5"), "{args}");
        assert!(!args.contains('3') && !args.contains('4'), "{args}");
    }

    #[test]
    fn a_strictly_better_mean_takes_the_champion_and_the_first_always_does() {
        assert!(beats(1.0, None));
        assert!(beats(10.1, Some(10.0)));
        assert!(!beats(10.0, Some(10.0)), "a tie keeps the champion");
        assert!(!beats(9.0, Some(10.0)));
    }

    #[test]
    fn notes_merge_without_repeating() {
        let a = vec!["x".to_owned(), "y".to_owned()];
        let b = vec!["y".to_owned(), "z".to_owned()];
        assert_eq!(merge_notes(&a, &b), ["x", "y", "z"]);
        assert_eq!(merge_notes(&[], &b), ["y", "z"]);
    }

    #[test]
    fn track_diff_names_what_changed_and_ignores_the_label() {
        let a = track();
        let mut b = a.clone();
        assert!(track_diff(&a, &b).is_empty());
        b.name = "Renamed".to_owned();
        assert!(track_diff(&a, &b).is_empty());
        b.horizon_turns = 9;
        b.train_seeds = vec![1, 5];
        let d = track_diff(&a, &b).join("; ");
        assert!(d.contains("horizon_turns was 5, is now 9"), "{d}");
        assert!(d.contains("train_seeds"), "{d}");
    }

    #[test]
    fn numbering_skips_folders_with_no_row() {
        let tmp = std::env::temp_dir().join(format!("fn_campaign_next_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("attempt-1")).unwrap();
        fs::create_dir_all(tmp.join("attempt-4")).unwrap();
        fs::create_dir_all(tmp.join("champion")).unwrap();
        let l = ledger();
        assert_eq!(next_attempt(&tmp, &l), 5, "attempt-4 was left unfinished: not reused");
        let empty = Ledger { track: track(), rows: Vec::new() };
        assert_eq!(next_attempt(&tmp.join("nowhere"), &empty), 1);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn plan_revisions_count_plan_lines() {
        let lines = vec![
            LogLine::Plan { plan: json!({"a": 1}) },
            LogLine::End,
            LogLine::Plan { plan: json!({"a": 2}) },
        ];
        assert_eq!(plan_edits(&lines), 2);
    }

    #[test]
    fn attempt_options_follow_the_track() {
        let mut t = track();
        t.site = "10,20".to_owned();
        t.prelude = Some(agent_eval::TrackPrelude { opening: "economy".to_owned(), minutes: 60.0 });
        let base = parse_args_with(&["--model", "m", "--chunk-calls", "7"], &mut || 1).unwrap();
        let o = attempt_opts(&base, &t, Some(1.5), Some(PathBuf::from("/c/handoff.json")), None);
        assert_eq!((o.seed, o.nation.as_str(), o.turns, o.chunk_calls), (1, "water", 5, 7));
        assert_eq!(o.model.as_deref(), Some("m"));
        assert_eq!(o.max_usd, Some(1.5));
        let args = serve_args(&o, Path::new("/a/run.jsonl")).join(" ");
        assert!(args.contains("--scripts") && args.contains("--map-size small"), "{args}");
        assert!(args.contains("--start 10,20") && args.contains("--prelude economy:60"), "{args}");
        assert!(args.contains("--carry-in /c/handoff.json"), "{args}");
    }
}

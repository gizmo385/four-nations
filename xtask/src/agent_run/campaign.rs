//! The hill-climb campaign behind `cargo xtask agent-run --track ID --attempts N` and
//! `--resume CAMPAIGN_DIR` (#530, docs/design/agent-hillclimb.md, "The attempt loop").
//!
//! A campaign is a folder under `$FN_AGENT_RUNS`:
//!
//! ```text
//! CAMPAIGN/
//!   ledger.json            the track as it was, and a row per finished attempt
//!   champion/handoff.json  the best strategy so far: scripts, plan and notes
//!   champion/meta.json     which attempt it came from, and its screen and gate means
//!   champion/gate.json     its score on each gate seed, on the build it was measured on (#571)
//!   champion/scorecard.txt its scorecard on the track's play_seed, for the next prompt
//!   attempt-K/             a normal --scripts run on play_seed, played to the horizon
//!     eval/vV-N/           one clean evaluation per governor version tried (#570): V is the
//!                          version, N the order; seed-N.jsonl logs of the screen seeds, and of
//!                          the gate and owner-only seeds for the chosen pair only
//!     eval/versions.json   what was tried, with screen scores and seconds, and which was chosen
//!     handoff.json         the attempt's own final strategy
//!     chosen-handoff.json  the strategy that became the champion, when one did
//! ```
//!
//! After every attempt the played log is scored (`fn_agent score --append`) and every governor
//! version it saved is cleanly evaluated on the screen seeds with the plan it was judged with
//! (`versions`, `agent_eval::evaluate_many`, in parallel). The best by screen mean is the
//! attempt's candidate, and only it runs the gate seeds, where it is compared with the
//! champion's scores on the same seeds (`paired::judge`: a bootstrap interval of the paired mean
//! difference that must lie above zero, and no more collapses than the champion). If it passes
//! the attempt becomes the champion and the candidate runs the owner-only tier too. Otherwise the
//! champion stays, the attempt's notes are merged into the champion's, and its code and plan are
//! dropped. One ledger row is written, last: a row is what makes an attempt count, so an attempt
//! a crash left unfinished has none and is left alone. The first governor of a campaign has
//! nothing to be compared with and becomes the champion.
//!
//! Every score is made on a build of `fn_agent` (#566), recorded on each row and on the champion.
//! A resume on a different build than the champion's evaluates the champion again first, on the
//! screen and gate tiers ([`Campaign::reevaluate_champion`]) and compares new attempts with
//! that, keeping the old means in the ledger's `reevals`. A ledger from before the seed tiers
//! (#571) is refused with a message that says so.
//!
//! Owner-only scores go in the ledger and the table printed for the owner, and in nothing the
//! model reads: [`ledger_text`] with `owner == false` is the only ledger text a prompt gets.

use std::fmt::Write as _;

use serde_json::{Value, json};

use super::*;
use crate::agent_eval::{self, Role, Strategy, Table, Track, paired};
use crate::references::{self, References};

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
    /// Deaths in the run (#571); absent in a score written without them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    deaths: Option<u32>,
}

impl From<&SeedScore> for paired::Sample {
    fn from(s: &SeedScore) -> Self {
        Self {
            seed: s.seed,
            score: s.score,
            deaths: s.deaths.unwrap_or(0),
            stopped: s.stopped.is_some(),
        }
    }
}

fn samples(scores: &[SeedScore]) -> Vec<paired::Sample> {
    scores.iter().map(paired::Sample::from).collect()
}

/// How a candidate compared with the champion on the gate seeds (#571), as the ledger keeps it.
/// Every number is an aggregate over the gate seeds; none names a seed or a score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct GateRecord {
    /// The champion it was compared with.
    champion_attempt: u32,
    /// Candidate minus champion, mean per seed, and its bootstrap interval at `level`.
    diff: f64,
    lo: f64,
    hi: f64,
    level: f64,
    wins: u32,
    losses: u32,
    ties: u32,
    candidate_collapses: u32,
    champion_collapses: u32,
    candidate_deaths: u32,
    champion_deaths: u32,
    /// The rule's answer and its reason in words.
    promoted: bool,
    reason: String,
}

impl GateRecord {
    fn new(champion_attempt: u32, v: &paired::Verdict) -> Self {
        Self {
            champion_attempt,
            diff: v.diff.mean,
            lo: v.diff.lo,
            hi: v.diff.hi,
            level: v.diff.level,
            wins: v.wins as u32,
            losses: v.losses as u32,
            ties: v.ties as u32,
            candidate_collapses: v.candidate.collapses as u32,
            champion_collapses: v.champion.collapses as u32,
            candidate_deaths: v.candidate.deaths,
            champion_deaths: v.champion.deaths,
            promoted: v.promote,
            reason: v.reason.clone(),
        }
    }

    /// `+31 [+8, +55]`: the gain and its interval, rounded to whole points.
    fn text(&self) -> String {
        format!("{:+.0} [{:+.0}, {:+.0}]", self.diff, self.lo, self.hi)
    }
}

/// An attempt's ledger row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Row {
    attempt: u32,
    /// The `governor` version the attempt hands on (#570): the one whose pair scored best on the
    /// screen seeds, not necessarily the last saved. `None` when the attempt never saved one.
    governor_version: Option<u32>,
    /// Plan edits since the campaign began (the champion's count, plus this attempt's own up to
    /// the plan the chosen pair was judged with).
    plan_revision: u32,
    /// The score of the attempt's own played log at the horizon.
    played: Option<f64>,
    /// The clean score per screen seed, in `screen_seeds` order (empty with no governor).
    screen: Vec<SeedScore>,
    screen_mean: Option<f64>,
    /// The chosen pair on the gate seeds, in `gate_seeds` order (empty with no governor).
    gate: Vec<SeedScore>,
    gate_mean: Option<f64>,
    /// The paired comparison with the champion; `None` for the first governor of a campaign
    /// (nothing to compare with) and for an attempt with no governor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    verdict: Option<GateRecord>,
    /// The clean score per owner-only seed, run for a promoted candidate only. For the owner; no
    /// prompt ever shows it, nor a count or a mean made from it.
    owner: Vec<SeedScore>,
    kept: bool,
    cost_usd: f64,
    /// The build (`fn_agent build-id`, #566) the clean scores above were made on. `None` in a
    /// ledger from before builds were recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    build: Option<String>,
    /// How many versions of the governor the attempt saved (#570); `None` with no governor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    saved: Option<u32>,
    /// How many governor and plan pairs were evaluated on the screen seeds to choose
    /// `governor_version` (at most `versions::CANDIDATE_CAP`; fewer than `saved` when the cap cut
    /// the older untested ones).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tried: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Ledger {
    /// The track as the campaign started it: what `--resume` checks against the tracks file.
    track: Track,
    rows: Vec<Row>,
    /// Each time a resume re-evaluated the champion on a new build, in order (#566).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    reevals: Vec<Reeval>,
    /// Par and the controls on the track's seeds, as of the last invocation that measured them
    /// (#572). For the owner's table only; optional, so a ledger without it reads as before and
    /// a reader that doesn't know it ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    references: Option<References>,
}

/// The champion, evaluated again on a different build than the one its means came from (#566,
/// #571). Scores aren't comparable across builds (the same strategy can score differently once
/// the sim has changed), so the attempts that follow are compared with the new gate scores, not
/// the old.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Reeval {
    /// The champion's attempt number.
    attempt: u32,
    /// How many attempts had a row when this happened: the re-evaluation sits between the rows.
    after_rows: u32,
    was_build: Option<String>,
    was_screen_mean: f64,
    was_gate_mean: f64,
    build: String,
    screen_mean: f64,
    gate_mean: f64,
}

/// `champion/meta.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Champion {
    attempt: u32,
    /// On `build`, as of the last time the champion was evaluated.
    screen_mean: f64,
    gate_mean: f64,
    governor_version: Option<u32>,
    plan_revision: u32,
    /// The build the means were measured on; `None` before #566, which a resume treats as
    /// another build and re-evaluates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    build: Option<String>,
}

/// `champion/gate.json`: the champion's score on each gate seed, on `build`. A candidate is
/// compared with these, paired by seed, so they are kept instead of run again for every attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ChampionGate {
    build: String,
    scores: Vec<SeedScore>,
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
    let raw: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{} isn't a ledger: {e}", path.display()))?;
    if is_pre_tier_ledger(&raw) {
        return Err(format!(
            "{} was written before the seed tiers (#571): its track has train and held-out \
             seeds and no gate, and its rows are scored a different way, so a promotion can't be \
             compared with them. It isn't read as a campaign any more. Start a new campaign with \
             --track ID; the old folder's ledger.json and champion/handoff.json are still plain \
             files, and `cargo xtask agent-compare DIR` still evaluates its champion on the \
             current tiers.",
            path.display()
        ));
    }
    serde_json::from_value(raw).map_err(|e| format!("{} isn't a ledger: {e}", path.display()))
}

/// Whether `ledger` is from before the seed tiers (#571): the track it holds names `train_seeds`
/// and `holdout_seeds` and no `gate_seeds`.
fn is_pre_tier_ledger(ledger: &Value) -> bool {
    let track = &ledger["track"];
    track.get("train_seeds").is_some() && track.get("gate_seeds").is_none()
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

/// The champion's scores on the gate seeds, where `gate.json` has them for this `build` and for
/// exactly the gate seeds of `track`, in order; `None` otherwise (a stale or missing record is
/// made again, never compared against).
fn read_champion_gate(dir: &Path, track: &Track, build: &str) -> Option<Vec<SeedScore>> {
    let text = fs::read_to_string(champion_dir(dir).join("gate.json")).ok()?;
    let gate: ChampionGate = serde_json::from_str(&text).ok()?;
    let seeds: Vec<u64> = gate.scores.iter().map(|s| s.seed).collect();
    (gate.build == build && seeds == track.gate_seeds).then_some(gate.scores)
}

/// Writes the champion's files. `meta.json` goes last: it is the record that says there is one.
fn write_champion(
    dir: &Path,
    handoff: &Handoff,
    champion: &Champion,
    gate: &ChampionGate,
    scorecard: &str,
) -> Result {
    let cdir = champion_dir(dir);
    fs::create_dir_all(&cdir).map_err(|e| format!("creating {}: {e}", cdir.display()))?;
    write_atomic(&cdir.join("scorecard.txt"), scorecard)?;
    write_json(&champion_handoff_path(dir), handoff)?;
    write_json(&cdir.join("gate.json"), gate)?;
    write_json(&cdir.join("meta.json"), champion)
}

// -------------------------------------------------------------------------------------------
// Selection
// -------------------------------------------------------------------------------------------
//
// An attempt's governor and plan pairs are ranked on the screen seeds (`versions::best`, screen
// means only); the best goes to the gate, where it and the champion are compared paired by seed
// (`paired::judge`, the rule written out in `agent_eval/paired.rs` and the design doc); a
// promoted candidate then also runs the owner-only tier, which decides nothing. The first
// governor of a campaign is promoted without a comparison, because there is nothing to compare
// with, and its gate scores become the champion's.

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

/// One short line per attempt whose clean runs on the screen seeds were stopped (#561): the
/// reason(s), how many seeds, the turn range, and the first one's `why`. Reads `screen` only, so
/// no gate or owner-only seed's stop can reach a line (the prompt's ledger text is built from these).
fn stop_lines(rows: &[Row]) -> Vec<String> {
    let mut lines = Vec::new();
    for r in rows {
        let stopped: Vec<&SeedScore> = r.screen.iter().filter(|s| s.stopped.is_some()).collect();
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
            r.screen.len(),
            why.unwrap_or_default(),
        ));
    }
    lines
}

/// One owner-table cell for a promoted attempt's owner-only scores: the mean and the failure
/// counts, not the scores seed by seed (thirty of them would not fit a row).
fn owner_cell(owner: &[SeedScore], collapse_below: f64) -> String {
    if owner.is_empty() {
        return "-".to_owned();
    }
    let s = samples(owner);
    let scores: Vec<f64> = s.iter().map(|x| x.score).collect();
    let f = paired::Failures::of(&s, collapse_below);
    format!(
        "{:.0} · collapse {}/{} · deaths {} · stopped {}",
        paired::mean(&scores),
        f.collapses,
        f.seeds,
        f.deaths,
        f.stopped
    )
}

/// The ledger as a table, one row per attempt. The columns every reader gets: the screen scores
/// and their mean, the gate mean, the gain over the champion with its interval, and the gate's
/// collapse count. With `owner` it also has the gate's deaths, the owner-only tier's summary
/// and each attempt's cost, then par and the controls (#572); the text a prompt gets is
/// `ledger_text(ledger, false)`, which has none of those. To add a column, push its header in
/// `head` and its cell in the row, in the same place.
fn ledger_text(ledger: &Ledger, owner: bool) -> String {
    let t = &ledger.track;
    let mut head = vec![
        "#".to_owned(),
        "governor".to_owned(),
        "plan".to_owned(),
        "tried".to_owned(),
        "played".to_owned(),
        format!("screen {}", seeds_label(&t.screen_seeds)),
        "mean".to_owned(),
        "gate".to_owned(),
        "vs champ [95%]".to_owned(),
        "collapses".to_owned(),
    ];
    if owner {
        head.push("gate deaths".to_owned());
        head.push(format!("owner-only ({} seeds)", t.owner_seeds.len()));
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
            // Versions evaluated of the versions saved (#570): "3/9" chose among 3 of 9.
            match (r.tried, r.saved) {
                (Some(t), Some(s)) => format!("{t}/{s}"),
                _ => "-".to_owned(),
            },
            r.played.map_or("-".to_owned(), |p| format!("{p:.0}")),
            scores_cell(&r.screen),
            r.screen_mean.map_or("-".to_owned(), |m| format!("{m:.0}")),
            r.gate_mean.map_or("-".to_owned(), |m| format!("{m:.0}")),
            r.verdict.as_ref().map_or("-".to_owned(), GateRecord::text),
            if r.gate.is_empty() {
                "-".to_owned()
            } else {
                let f = paired::Failures::of(&samples(&r.gate), t.collapse_below);
                format!("{}/{}", f.collapses, f.seeds)
            },
        ];
        if owner {
            let deaths: u32 = samples(&r.gate).iter().map(|s| s.deaths).sum();
            cells.push(if r.gate.is_empty() { "-".to_owned() } else { deaths.to_string() });
            cells.push(owner_cell(&r.owner, t.collapse_below));
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
    let kept_col = table[0].iter().position(|h| h == "kept").expect("a kept column");
    let mut out = String::new();
    let note = if owner { "                    (owner-only tier: owner only)" } else { "" };
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
    // Only the screen scores carry a `*`, so only they decide the footnote; a stop on a gate or
    // owner-only seed would show there as a footnote with no `*` to explain.
    let any_stop = ledger.rows.iter().flat_map(|r| &r.screen).any(|s| s.stopped.is_some());
    if any_stop {
        let _ = writeln!(
            out,
            " * the governor stopped (an error or a runaway) and the colony coasted with no \
             orders from that turn on"
        );
    }
    for line in stop_lines(&ledger.rows).into_iter().chain(reeval_lines(&ledger.reevals)) {
        let _ = writeln!(out, " {line}");
    }
    // Par and the controls (#572): the owner's table only, never a prompt's. A prompt gets no
    // number from data the model could not see.
    if owner && let Some(refs) = ledger.references.as_ref().filter(|r| !r.is_empty()) {
        let _ = writeln!(out);
        out.push_str(&refs.text(&Role::ALL));
    }
    out
}

/// What decides an attempt and how to check it, in every prompt a campaign gives the model:
/// attempt 1's, the later attempts' and every handoff (#569), so it is written once. The detail
/// that only the first prompt of an attempt carries is [`clean_run_rules`].
pub(super) fn deciding_passage(horizon: u32) -> String {
    format!(
        "What decides this attempt is the clean score, not this game's own: after the run, your \
         governor and plan are replayed from turn 0, with no one at the keys, on several \
         other maps. A governor that only works on the map you can \
         see scores badly there, so improve the governor and the plan, not just this \
         settlement. If the governor stops (an error or a runaway) it stays stopped, and the \
         colony coasts to the end with no orders. `test_governor` plays your saved governor and \
         the current plan from turn 0 on one of those maps, on a separate copy of the world \
         (this game is not touched), and tells you the stop reason, turn and score the clean \
         run will get, with the signs, the deaths and a short trace by time slice that show \
         why a map failed: run it after a change, on the maps that did worst. When the attempt \
         ends, each governor version you saved is replayed like that, with the plan as it was \
         when you moved on from it or tested it, and the best by mean score is the attempt's \
         candidate, so a worse last edit does not lose a better version. The candidate then has \
         to beat the best strategy so far on a larger set of other maps: it takes its place \
         only if it scores clearly higher across them, not on one or two, and does not collapse \
         on more of them, so a small gain that could be luck does not count. This game ends at \
         turn {horizon}; when it does, finish up (test, fix, save, notes) and stop."
    )
}

/// What a chunk after the first is told about where its attempt stands (#569): the live game's
/// turn, or, when no turn has been played, that this is the same attempt carrying on and not a
/// fresh start (a chunk can spend itself editing and testing); how many `test_governor` calls
/// are used; and [`deciding_passage`]. The plan, scripts and notes come with it in the listing.
pub(super) fn handoff_note(horizon: u32, done_turns: u32, tests: usize) -> String {
    let game = if done_turns == 0 {
        "No turn has been played in the live game yet. The earlier chunks of this attempt may \
         have gone on reading, editing and testing: everything they saved is below, so this is \
         not a fresh start."
            .to_owned()
    } else {
        format!("The live game is at turn {done_turns} of {horizon}.")
    };
    let tests = match tests {
        0 => String::new(),
        n => format!(
            " test_governor has been called {n} time(s) in this attempt so far; its result \
             says how many tests are left."
        ),
    };
    format!("{game}{tests}\n\n{}", deciding_passage(horizon))
}

/// One line per champion re-evaluation (#566): the rows above it were scored on the build it
/// names as `was`, and the attempts after it are compared with the new gate scores. Screen and
/// gate means only, so the prompt may show it.
fn reeval_lines(reevals: &[Reeval]) -> Vec<String> {
    let change = |name: &str, was: f64, now: f64| {
        if (now - was).abs() < 0.05 {
            format!("{name} mean {now:.0}, unchanged")
        } else {
            format!("{name} mean {was:.0} -> {now:.0}")
        }
    };
    reevals
        .iter()
        .map(|r| {
            let change = format!(
                "{}, {}",
                change("screen", r.was_screen_mean, r.screen_mean),
                change("gate", r.was_gate_mean, r.gate_mean)
            );
            format!(
                "champion (attempt {}) re-evaluated on build {} after {} attempt(s) (its means \
                 were from {}): {change}; later attempts are compared with the new gate scores",
                r.attempt,
                r.build,
                r.after_rows,
                build_words(r.was_build.as_ref()),
            )
        })
        .collect()
}

/// The detail of the clean run that an attempt's first prompt adds to [`deciding_passage`]
/// (#561): what a runaway is and what testing costs. Not repeated in handoffs.
fn clean_run_rules() -> &'static str {
    "A runaway is more work between two waits than the limit allows (each wait starts the count \
     again, and observe, act and the other game calls count for more than one each); a stop \
     early in the run costs most of the score. A `test_governor` run takes a few seconds and \
     costs one call, and so does the fix it asks for; the number of tests per attempt is \
     limited (the tool says how many are left) and a session that has played to the horizon \
     is given extra calls to use them. `detail: true` adds more to the trace, and a small \
     `turns` zooms in on the start; the log() lines in your governor come back with the \
     detail, so log what you would want to see on a map you cannot watch. A governor that \
     has only run on this built-up settlement can fail from an empty start."
}

/// Attempt 1's first prompt, with no champion yet: what decides the attempt and the clean-run
/// detail, on top of the plain scripted prompt (#569).
fn first_attempt_prompt(horizon: u32, calls: u32) -> String {
    format!(
        "{}\n\n{}\n\n{}",
        deciding_passage(horizon),
        clean_run_rules(),
        default_script_prompt(horizon, calls)
    )
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
         {decides}\n\n\
         The ledger so far (clean scores per screen map, `*` for a stopped governor; `gate` is \
         the mean on the larger set of other maps, `vs champ` the gain over the best strategy \
         then with the interval it is judged by, and `collapses` the maps where the governor \
         fell apart. An attempt becomes the best only if that interval is wholly above zero \
         and it collapses no more often):\n{ledger}\n\
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
        decides = deciding_passage(total),
        ledger = ledger_text(ledger, false),
        scorecard = scorecard.trim_end(),
        rules = clean_run_rules(),
        base = default_script_prompt(total, calls),
    )
}

// -------------------------------------------------------------------------------------------
// Running
// -------------------------------------------------------------------------------------------

/// The governor and plan pair an attempt hands on, with its clean evaluation (#570).
struct Chosen {
    version: u32,
    code: String,
    plan: Option<Value>,
    /// `plan` lines in the attempt's log up to this pair.
    plan_edits: u32,
    /// Screen and gate rows, and the owner-only rows when the pair was promoted.
    table: Table,
    /// The paired comparison with the champion on the gate seeds; `None` when there is no
    /// champion to compare with (the first governor of a campaign is promoted as it is).
    verdict: Option<paired::Verdict>,
    /// Whether the pair replaces the champion.
    promote: bool,
    /// Where this pair's evaluation logs are.
    dir: PathBuf,
    /// Governor versions the attempt saved, and pairs evaluated to choose among them.
    saved: u32,
    tried: u32,
}

/// What every attempt shares.
struct Campaign<'a> {
    repo: &'a Path,
    dir: PathBuf,
    ledger: Ledger,
    fn_agent: PathBuf,
    /// The build `fn_agent` is (`fn_agent build-id`, #566): what this invocation's scores are on.
    build: String,
    /// Clean runs at once (`$FN_EVAL_THREADS`, else half the cores; see `agent_eval`).
    threads: usize,
    opts: &'a Opts,
}

pub(super) fn run(repo: &Path, opts: &Opts) -> Result {
    let tracks_path = opts.tracks_file.clone().unwrap_or_else(|| repo.join("data/tracks.toml"));
    let (dir, ledger, fresh) = match (&opts.track, &opts.resume_dir) {
        (Some(id), _) => {
            let track = agent_eval::load_track(&tracks_path, id)?;
            let dir = new_campaign_dir(&runs_dir(), &track.id)?;
            let ledger = Ledger { track, rows: Vec::new(), reevals: Vec::new(), references: None };
            write_ledger(&dir, &ledger)?;
            (dir, ledger, true)
        }
        (None, Some(dir)) => (dir.clone(), open_campaign(dir, &tracks_path)?, false),
        (None, None) => return Err("agent-run campaign: --track or --resume is required".into()),
    };
    let attempts = opts.attempts.unwrap_or(1);

    let fn_agent = agent_eval::fn_agent_binary(repo)?;
    let threads = agent_eval::resolve_threads(None)?;

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

    let build = fn_agent_build(&fn_agent)?;
    say(&format!("build {build}; clean runs on {threads} thread(s)"));
    let mut campaign = Campaign { repo, dir, ledger, fn_agent, build, threads, opts };
    campaign.reevaluate_champion()?;
    campaign.climb(attempts)?;
    campaign.measure_references();
    print!("{}", ledger_text(&campaign.ledger, true));
    say(&format!("ledger: {}", campaign.dir.join(LEDGER).display()));
    if let Some(c) = read_champion(&campaign.dir)? {
        say(&format!(
            "champion: attempt {} (screen mean {:.0}, gate mean {:.0}), in {}",
            c.attempt,
            c.screen_mean,
            c.gate_mean,
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

/// What `fn_agent build-id` prints: the build of the binary that scores everything in this
/// invocation.
fn fn_agent_build(fn_agent: &Path) -> Result<String> {
    let out = Command::new(fn_agent)
        .arg("build-id")
        .output()
        .map_err(|e| format!("can't run {} build-id: {e}", fn_agent.display()))?;
    let id = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if !out.status.success() || id.is_empty() {
        return Err(format!(
            "{} build-id failed (a binary from before #566?): {}",
            fn_agent.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(id)
}

/// `build X`, or what a ledger from before builds were recorded says instead.
fn build_words(build: Option<&String>) -> String {
    build.map_or("an unrecorded build".to_owned(), |b| format!("build {b}"))
}

impl Campaign<'_> {
    /// Par and the controls on every tier (#572), once the attempts are done: par is computed
    /// once per build and seed and kept outside the run folders (`fn_agent par`), the controls
    /// per build and tier (`references::controls_many`), so a resume on the same build costs
    /// nothing. It runs here and not during an attempt, and says how long it took. These are
    /// context for the owner: a failure is noted in the ledger and the table and never stops the
    /// campaign, and nothing measured here reaches a prompt (`ledger_text(.., false)` leaves it
    /// out).
    fn measure_references(&mut self) {
        let controls = match references::controls_for(&self.opts.controls, !self.opts.no_controls) {
            Ok(c) => c,
            Err(e) => {
                say(&format!("no controls: {e}"));
                Vec::new()
            }
        };
        if self.opts.no_par && controls.is_empty() {
            return;
        }
        let want = references::Want { par: !self.opts.no_par, controls: &controls };
        let refs = references::gather(
            &self.fn_agent,
            &self.ledger.track,
            &Role::ALL,
            &want,
            self.threads,
            &self.build,
        );
        self.ledger.references = Some(refs);
        if let Err(e) = write_ledger(&self.dir, &self.ledger) {
            say(&format!("couldn't record par and the controls in the ledger: {e}"));
        }
    }

    /// Evaluates the champion again on this build, when its means came from another (#566) or
    /// its gate scores aren't on file for this build (#571): runs are deterministic, so the same
    /// strategy scores the same on the same build and nothing changes; on another build the sim
    /// may have, and the champion's old numbers are not ones this build's attempts can be
    /// compared with. The champion is run on the screen and gate tiers, the tiers a promotion
    /// reads; the new means replace the old in `meta.json` (the old stay in the ledger's
    /// `reevals`), and its gate scores are what every attempt from here on is compared with.
    /// It needs no model, only CPU.
    fn reevaluate_champion(&mut self) -> Result {
        let Some(champion) = read_champion(&self.dir)? else { return Ok(()) };
        let track = self.ledger.track.clone();
        let same_build = champion.build.as_deref() == Some(self.build.as_str());
        if same_build && read_champion_gate(&self.dir, &track, &self.build).is_some() {
            return Ok(());
        }
        let handoff = read_handoff(&champion_handoff_path(&self.dir))?;
        let Some(governor) = handoff.scripts.get(GOVERNOR).filter(|g| !g.code.trim().is_empty())
        else {
            return Err(format!(
                "the champion (attempt {}) has no script named \"{GOVERNOR}\" to evaluate",
                champion.attempt
            ));
        };
        if same_build {
            say(&format!(
                "champion (attempt {}): no gate scores on file for this build; evaluating it \
                 again",
                champion.attempt
            ));
        } else {
            say(&format!(
                "champion (attempt {}): its means (screen {:.0}, gate {:.0}) were measured on {}, \
                 this is build {}; evaluating it again",
                champion.attempt,
                champion.screen_mean,
                champion.gate_mean,
                build_words(champion.build.as_ref()),
                self.build,
            ));
        }
        let strategy = Strategy { governor: governor.code.clone(), plan: handoff.plan.clone() };
        let work = self.dir.join(format!("reeval-{}", self.ledger.reevals.len() + 1));
        let table = agent_eval::evaluate_tiers(
            &self.fn_agent,
            &track,
            &strategy,
            &work,
            &[Role::Screen, Role::Gate],
            self.threads,
        )
        .map_err(|e| format!("re-evaluating the champion on this build failed: {e}"))?;
        let screen_mean =
            table.mean_of(Role::Screen).ok_or("re-evaluating the champion gave no screen mean")?;
        let gate_mean =
            table.mean_of(Role::Gate).ok_or("re-evaluating the champion gave no gate mean")?;
        say(&format!(
            "champion (attempt {}): screen mean {:.0} -> {screen_mean:.0}, gate mean {:.0} -> \
             {gate_mean:.0} on build {}",
            champion.attempt, champion.screen_mean, champion.gate_mean, self.build
        ));

        if !same_build {
            self.ledger.reevals.push(Reeval {
                attempt: champion.attempt,
                after_rows: self.ledger.rows.len() as u32,
                was_build: champion.build.clone(),
                was_screen_mean: champion.screen_mean,
                was_gate_mean: champion.gate_mean,
                build: self.build.clone(),
                screen_mean,
                gate_mean,
            });
            write_ledger(&self.dir, &self.ledger)?;
        }
        // The scorecard the next prompt shows is the champion's on this build; `meta.json` goes
        // last, as in `write_champion`.
        let log = work.join(format!("seed-{}.jsonl", track.play_seed));
        if log.is_file() {
            let card = scorecard_text(&self.fn_agent, &log, track.horizon_turns);
            write_atomic(&champion_dir(&self.dir).join("scorecard.txt"), &card)?;
        }
        let gate =
            ChampionGate { build: self.build.clone(), scores: seed_scores(&table, Role::Gate) };
        write_json(&champion_dir(&self.dir).join("gate.json"), &gate)?;
        let record =
            Champion { screen_mean, gate_mean, build: Some(self.build.clone()), ..champion };
        write_json(&champion_dir(&self.dir).join("meta.json"), &record)
    }

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

    /// Chooses the attempt's candidate and judges it (#571). The governor and plan pairs the
    /// attempt held (`versions::find`, at most `CANDIDATE_CAP` of them) are all run on the
    /// screen seeds, in one parallel queue; the best by screen mean is the candidate
    /// (`versions::best`, which takes screen means and nothing else). Only the candidate runs the
    /// gate seeds, and it is judged against the champion's gate scores (`paired::judge`); a
    /// promoted candidate then runs the owner-only tier, which decides nothing. `None` when the
    /// attempt saved no governor. Needs no model; the time it takes is printed and written to
    /// `eval/versions.json`.
    fn evaluate_versions(
        &self,
        k: u32,
        adir: &Path,
        lines: &[LogLine],
        champion: Option<(&Champion, &Handoff)>,
    ) -> Result<Option<Chosen>> {
        let track = &self.ledger.track;
        let (mut pairs, mut saved) = versions::find(lines);
        pairs.retain(|p| !p.code.trim().is_empty());
        let champion_pair = champion
            .and_then(|(_, h)| h.scripts.get(GOVERNOR).map(|g| (g.code.clone(), h.plan.clone())));
        if champion_pair.as_ref().is_some_and(|(code, _)| versions::starts_from(lines, code)) {
            saved -= 1;
        }
        let skip = champion_pair.as_ref().map(|(code, plan)| (code.as_str(), plan));
        let picked = versions::pick(pairs, versions::CANDIDATE_CAP, skip);
        if picked.is_empty() {
            return Ok(None);
        }

        // The screen: every pair, all their seeds in one queue.
        let started = Instant::now();
        say(&format!(
            "attempt {k}: screening {} governor/plan pair(s) on {} seed(s) each",
            picked.len(),
            track.screen_seeds.len()
        ));
        let strategies: Vec<Strategy> = picked
            .iter()
            .map(|c| Strategy { governor: c.code.clone(), plan: c.plan.clone() })
            .collect();
        let dirs: Vec<PathBuf> = picked
            .iter()
            .enumerate()
            .map(|(i, c)| adir.join("eval").join(format!("v{}-{}", c.version, i + 1)))
            .collect();
        let jobs: Vec<agent_eval::Job> = strategies
            .iter()
            .zip(&dirs)
            .map(|(strategy, work)| agent_eval::Job { strategy, work: work.clone() })
            .collect();
        let mut tables =
            agent_eval::evaluate_many(&self.fn_agent, track, &jobs, &[Role::Screen], self.threads)
                .map_err(|e| format!("attempt {k}: the clean evaluation failed: {e}"))?;
        let screen_secs = started.elapsed().as_secs();
        let means: Vec<Option<f64>> = tables.iter().map(Table::screen_mean).collect();
        let Some(best) = versions::best(&means) else { return Ok(None) };
        let c = &picked[best];
        let (mut table, dir) = (tables.swap_remove(best), dirs[best].clone());

        // The gate: the candidate alone, then compared with the champion's scores on the same
        // seeds.
        say(&format!(
            "attempt {k}: governor v{} has the best screen mean; running the gate",
            c.version
        ));
        let gate_started = Instant::now();
        let gate = agent_eval::evaluate_tiers(
            &self.fn_agent,
            track,
            &strategies[best],
            &dir,
            &[Role::Gate],
            self.threads,
        )
        .map_err(|e| format!("attempt {k}: the gate evaluation failed: {e}"))?;
        table.rows.extend(gate.rows);
        let gate_secs = gate_started.elapsed().as_secs();
        let verdict = match champion {
            None => None,
            Some((champion, _)) => {
                let scores =
                    read_champion_gate(&self.dir, track, &self.build).ok_or_else(|| {
                        format!(
                            "attempt {k}: the champion has no gate scores on file for build {}",
                            self.build
                        )
                    })?;
                let v = paired::judge(
                    &table.samples(Role::Gate),
                    &samples(&scores),
                    track.collapse_below,
                )
                .map_err(|e| format!("attempt {k}: comparing with the champion: {e}"))?;
                say(&format!(
                    "attempt {k}: against champion (attempt {}) on the gate: {} (candidate {:.0}, champion {:.0}); {}",
                    champion.attempt,
                    v.diff.text(),
                    v.candidate_mean,
                    v.champion_mean,
                    v.reason
                ));
                Some(v)
            }
        };
        let promote = verdict.as_ref().is_none_or(|v| v.promote);

        // The owner-only tier, for a promoted candidate only: it decides nothing.
        let mut owner_secs = 0;
        if promote {
            say(&format!("attempt {k}: promoted; running the owner-only tier"));
            let owner_started = Instant::now();
            let owner = agent_eval::evaluate_tiers(
                &self.fn_agent,
                track,
                &strategies[best],
                &dir,
                &[Role::Owner],
                self.threads,
            )
            .map_err(|e| format!("attempt {k}: the owner-only evaluation failed: {e}"))?;
            table.rows.extend(owner.rows);
            owner_secs = owner_started.elapsed().as_secs();
        }

        let total = started.elapsed().as_secs();
        say(&format!(
            "attempt {k}: {saved} governor version(s) saved, {} screened, v{} chosen; evaluation \
             took {total} s (screen {screen_secs} s, gate {gate_secs} s, owner-only {owner_secs} s)",
            picked.len(),
            c.version
        ));
        let summary = serde_json::json!({
            "build": self.build,
            "threads": self.threads,
            "saved": saved,
            "tried": picked.len(),
            "cap": versions::CANDIDATE_CAP,
            "chosen": c.version,
            "seconds": total,
            "screen_seconds": screen_secs,
            "gate_seconds": gate_secs,
            "owner_seconds": owner_secs,
            "promoted": promote,
            // Screen scores only: the choice of the pair never saw anything else.
            "pairs": picked.iter().zip(&means).enumerate().map(|(i, (p, m))| json!({
                "version": p.version,
                "tested": p.tested,
                "final": p.last,
                "plan_edits": p.plan_edits,
                "screen_mean": m,
                "chosen": i == best,
            })).collect::<Vec<_>>(),
        });
        write_json(&adir.join("eval").join("versions.json"), &summary)?;
        Ok(Some(Chosen {
            version: c.version,
            code: c.code.clone(),
            plan: c.plan.clone(),
            plan_edits: c.plan_edits,
            table,
            verdict,
            promote,
            dir,
            saved,
            tried: picked.len() as u32,
        }))
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
            None => Some(first_attempt_prompt(track.horizon_turns, self.opts.chunk_calls)),
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
        // An error here is a chunk whose cost couldn't be read (#568): the attempt's spend is
        // unknown, so the campaign stops. The attempt gets no ledger row; its folder is kept
        // (stop.json says why) and is never reused.
        run_scripts_chunked(self.repo, &adir, &self.fn_agent, &o, &claude_args).map_err(|e| {
            format!(
                "attempt {k} stopped the campaign: {e} No ledger row was written for it; its \
                 folder is {}.",
                adir.display()
            )
        })?;

        // Score what was played.
        let log = adir.join("run.jsonl");
        let played = score_played(&self.fn_agent, &log, track.horizon_turns);
        say(&format!(
            "attempt {k}: played score {}",
            played.map_or("unavailable".to_owned(), |p| format!("{p:.0}"))
        ));

        // The attempt's strategy: every governor version it saved, each with the plan it was
        // judged with, ranked on the screen seeds, and the best of them judged on the gate (#570,
        // #571).
        let lines = read_log_lines(&log).unwrap_or_default();
        let (scripts, notes, plan) = parse_library(&lines);
        let handoff = Handoff { scripts, notes, plan };
        write_json(&adir.join("handoff.json"), &handoff)?;
        let chosen = self.evaluate_versions(
            k,
            &adir,
            &lines,
            champion.as_ref().zip(champion_handoff.as_ref()),
        )?;
        if chosen.is_none() {
            say(&format!("attempt {k}: no script named \"{GOVERNOR}\" was saved; not kept"));
        }

        // Select: the promotion rule's answer (`paired::judge`), or yes for the first governor.
        let screen_mean = chosen.as_ref().and_then(|c| c.table.screen_mean());
        let gate_mean = chosen.as_ref().and_then(|c| c.table.mean_of(Role::Gate));
        let kept = chosen.as_ref().is_some_and(|c| c.promote);
        let prior_plan_revision = champion.as_ref().map_or(0, |c| c.plan_revision);
        let preloaded_plan = champion_handoff.as_ref().is_some_and(|h| h.plan.is_some());
        let edits = chosen.as_ref().map_or_else(|| plan_edits(&lines), |c| c.plan_edits);
        let plan_revision = prior_plan_revision + edits.saturating_sub(u32::from(preloaded_plan));
        let governor_version = chosen.as_ref().map(|c| c.version);
        let old_notes = champion_handoff.as_ref().map_or(Vec::new(), |h| h.notes.clone());
        let merged = merge_notes(&old_notes, &handoff.notes);
        if let Some(c) = chosen.as_ref().filter(|_| kept) {
            // The strategy the champion keeps is the chosen pair: its governor and its plan, not
            // necessarily the ones the attempt ended with. `handoff.json` still has those.
            let mut scripts = handoff.scripts.clone();
            scripts.insert(
                GOVERNOR.to_owned(),
                ScriptEntry { version: c.version, code: c.code.clone(), ..Default::default() },
            );
            let strategy = Handoff { scripts, notes: merged, plan: c.plan.clone() };
            write_json(&adir.join("chosen-handoff.json"), &strategy)?;
            let card_log = {
                let clean = c.dir.join(format!("seed-{}.jsonl", track.play_seed));
                if clean.is_file() { clean } else { log.clone() }
            };
            let record = Champion {
                attempt: k,
                screen_mean: screen_mean.unwrap_or_default(),
                gate_mean: gate_mean.unwrap_or_default(),
                governor_version,
                plan_revision,
                build: Some(self.build.clone()),
            };
            let gate = ChampionGate {
                build: self.build.clone(),
                scores: seed_scores(&c.table, Role::Gate),
            };
            let card = scorecard_text(&self.fn_agent, &card_log, track.horizon_turns);
            write_champion(&self.dir, &strategy, &record, &gate, &card)?;
        } else if let Some(mut stays) = champion_handoff {
            // The idea is kept so the next attempt doesn't retry it; its code and plan aren't.
            stays.notes = merged;
            write_json(&champion_handoff_path(&self.dir), &stays)?;
        }
        let table = chosen.as_ref().map(|c| &c.table);

        let row = Row {
            attempt: k,
            governor_version,
            plan_revision,
            played,
            screen: table.map_or(Vec::new(), |t| seed_scores(t, Role::Screen)),
            screen_mean,
            gate: table.map_or(Vec::new(), |t| seed_scores(t, Role::Gate)),
            gate_mean,
            verdict: chosen
                .as_ref()
                .zip(champion.as_ref())
                .and_then(|(c, ch)| c.verdict.as_ref().map(|v| GateRecord::new(ch.attempt, v))),
            owner: table.map_or(Vec::new(), |t| seed_scores(t, Role::Owner)),
            kept,
            cost_usd: cost_so_far(&adir),
            build: Some(self.build.clone()),
            saved: chosen.as_ref().map(|c| c.saved),
            tried: chosen.as_ref().map(|c| c.tried),
        };
        say(&format!(
            "attempt {k}: screen mean {}, gate mean {}; {}",
            screen_mean.map_or("-".to_owned(), |m| format!("{m:.0}")),
            gate_mean.map_or("-".to_owned(), |m| format!("{m:.0}")),
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
    // The screen seeds only: a gate or owner-only seed never reaches the model's side of the
    // harness.
    o.trial = Some((track.screen_seeds.clone(), track.horizon_turns));
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
            deaths: Some(r.deaths),
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
        .args(["--horizon", &horizon.to_string(), "--json", "--append", "--no-par"])
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
        // No par (#572): this text goes into the next prompt, and par is not for the model.
        .args(["--horizon", &horizon.to_string(), "--no-par"])
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
             screen_seeds = [1, 2]\ngate_seeds = [101, 102]\nowner_seeds = [301, 302]\n\
             collapse_below = 100\n",
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
                deaths: None,
            })
            .collect()
    }

    fn champion(attempt: u32, screen_mean: f64) -> Champion {
        Champion {
            attempt,
            screen_mean,
            gate_mean: 200.0,
            governor_version: Some(9),
            plan_revision: 6,
            build: None,
        }
    }

    /// Row 1 is the campaign's first governor (no verdict); row 2 saved nothing. Its owner-only
    /// scores are 7771 and 7772, numbers that must never reach a prompt.
    fn ledger() -> Ledger {
        Ledger {
            track: track(),
            rows: vec![
                Row {
                    attempt: 1,
                    governor_version: Some(9),
                    plan_revision: 6,
                    played: Some(256.4),
                    screen: seeds(&[(1, 231.0), (2, 174.0)]),
                    screen_mean: Some(202.5),
                    gate: seeds(&[(101, 220.0), (102, 60.0)]),
                    gate_mean: Some(140.0),
                    verdict: None,
                    owner: seeds(&[(301, 7771.0), (302, 7772.0)]),
                    kept: true,
                    cost_usd: 0.974,
                    build: None,
                    saved: Some(9),
                    tried: Some(3),
                },
                Row {
                    attempt: 2,
                    governor_version: None,
                    plan_revision: 6,
                    played: None,
                    screen: Vec::new(),
                    screen_mean: None,
                    gate: Vec::new(),
                    gate_mean: None,
                    verdict: None,
                    owner: Vec::new(),
                    kept: false,
                    cost_usd: 0.1,
                    build: None,
                    saved: None,
                    tried: None,
                },
            ],
            reevals: Vec::new(),
            references: None,
        }
    }

    #[test]
    fn the_owner_table_has_the_owner_tier_failures_and_cost() {
        let t = ledger_text(&ledger(), true);
        assert!(t.contains("owner-only (2 seeds)") && t.contains("$0.97"), "{t}");
        // The owner-only mean (7771.5), its collapse count and deaths; not the seeds one by one.
        assert!(t.contains("7772 · collapse 0/2 · deaths 0 · stopped 0"), "{t}");
        assert!(t.contains("screen 1/2") && t.contains("231 / 174") && t.contains("champ"), "{t}");
        assert!(t.contains("v9") && t.contains("r6") && t.contains("256"), "{t}");
        // The gate: its mean, and the collapses at the track's threshold of 100 (one of two).
        assert!(t.contains("gate") && t.contains("140") && t.contains("1/2"), "{t}");
        assert!(t.contains("gate deaths"), "{t}");
        // An attempt with no governor shows dashes, not zeros.
        let row2 = t.lines().last().unwrap();
        assert!(row2.contains('-') && row2.contains("no"), "{t}");
    }

    /// The prompt's table has the screen scores and the gate's aggregates, and nothing of the
    /// owner-only tier: no seed, score, count or the tier's name; no cost.
    /// Par and the controls (#572) are the owner's: the owner text has them and the prompt's
    /// never does, whatever the ledger holds.
    #[test]
    fn par_and_the_controls_are_in_the_owner_text_and_never_the_prompts() {
        let mut l = ledger();
        l.references = Some(References {
            build: "abc".into(),
            horizon: 2,
            par: vec![references::ParSeed {
                seed: 1,
                tier: "screen".into(),
                score: 4321.0,
                progress: 0.4,
                care: 1.0,
                deaths: 0,
                reached: 1,
                total: 2,
                unreached: vec!["Ready".into()],
            }],
            ..References::default()
        });
        let owner = ledger_text(&l, true);
        assert!(owner.contains("references (build abc)") && owner.contains("4321"), "{owner}");
        let prompt = ledger_text(&l, false);
        assert_eq!(prompt, ledger_text(&ledger(), false), "the same text as with no references");
        assert!(!prompt.contains("4321") && !prompt.contains("par"), "{prompt}");
        // A ledger written without them reads back, and one with them round-trips.
        let back: Ledger = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(back, l);
        let bare = serde_json::to_string(&ledger()).unwrap();
        assert!(!bare.contains("references"), "left out of the file when there are none");
    }

    #[test]
    fn the_prompt_table_never_shows_the_owner_tier_or_cost() {
        let t = ledger_text(&ledger(), false);
        for needle in ["owner", "7771", "7772", "301", "302", "2 seeds", "$", "cost", "deaths"] {
            assert!(!t.contains(needle), "{needle:?} in\n{t}");
        }
        assert!(t.contains("231 / 174") && t.contains("202"), "{t}");
        assert!(t.contains("gate") && t.contains("collapses") && t.contains("vs champ"), "{t}");
        // The gate seeds themselves are not named either.
        assert!(!t.contains("101") && !t.contains("102"), "{t}");
    }

    #[test]
    fn the_attempt_prompt_builds_on_the_script_prompt_and_hides_the_owner_tier() {
        let l = ledger();
        let c = champion(1, 202.5);
        let p = attempt_prompt(&l, 3, &c, "Water seed 1\nscore 231\n", 5, 30);
        assert!(p.contains("attempt 3") && p.contains("(attempt 1)"), "{p}");
        assert!(p.contains("raise the clean score"), "{p}");
        assert!(p.contains("score 231") && p.contains("231 / 174"), "{p}");
        assert!(
            p.ends_with(&default_script_prompt(5, 30)),
            "the script prompt comes through whole"
        );
        for needle in ["7771", "7772", "owner", "held-out", "301", "302"] {
            assert!(!p.contains(needle), "{needle:?} in\n{p}");
        }
    }

    /// What the gate says in a row: the gain over the champion with its interval, and the
    /// collapses on each side.
    #[test]
    fn a_verdict_shows_as_the_gain_and_its_interval() {
        let champ: Vec<f64> = vec![200.0, 190.0, 210.0, 205.0, 195.0, 200.0];
        let cand: Vec<f64> = champ.iter().map(|s| s + 30.0).collect();
        let s = |scores: &[f64]| -> Vec<paired::Sample> {
            scores
                .iter()
                .enumerate()
                .map(|(i, &score)| paired::Sample {
                    seed: 100 + i as u64,
                    score,
                    deaths: 0,
                    stopped: false,
                })
                .collect()
        };
        let v = paired::judge(&s(&cand), &s(&champ), 100.0).unwrap();
        assert!(v.promote);
        let mut l = ledger();
        l.rows[0].verdict = Some(GateRecord::new(1, &v));
        let t = ledger_text(&l, false);
        assert!(t.contains("+30 [+30, +30]"), "{t}");
        let back: Ledger = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(back, l);
        assert!(back.rows[0].verdict.as_ref().unwrap().promoted);
    }

    fn stopped(seed: u64, score: f64, reason: &str, turn: u32, why: Option<&str>) -> SeedScore {
        SeedScore {
            seed,
            score,
            stopped: Some(reason.to_owned()),
            stopped_turn: Some(turn),
            stopped_why: why.map(str::to_owned),
            deaths: None,
        }
    }

    /// A ledger whose attempt 1 had both screen runs stopped, and both gate and both owner-only
    /// runs stopped for a reason, turn and why that appear nowhere else.
    fn stopped_ledger() -> Ledger {
        let mut l = ledger();
        l.rows[0].screen = vec![
            stopped(1, 101.0, "runaway", 171, Some("a loop with no wait?")),
            stopped(2, 51.0, "runaway", 195, None),
        ];
        l.rows[0].gate = vec![
            stopped(101, 66.0, "error", 3131, Some("GATE_WHY secret")),
            stopped(102, 67.0, "error", 3132, Some("GATE_WHY secret")),
        ];
        l.rows[0].owner = vec![
            stopped(301, 77.0, "error", 4242, Some("OWNER_WHY secret")),
            stopped(302, 78.0, "error", 4243, Some("OWNER_WHY secret")),
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
        row.screen = vec![
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
        row.screen = vec![old];
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

    /// A gate or owner-only seed's stop reason, turn and why never reach the prompt; the owner's
    /// table marks the screen stops but gives no detail on any other tier's.
    #[test]
    fn gate_and_owner_stops_never_reach_the_prompt() {
        let l = stopped_ledger();
        let c = champion(1, 76.0);
        let p = attempt_prompt(&l, 3, &c, "card\n", 5, 30);
        for needle in [
            "GATE_WHY",
            "OWNER_WHY",
            "3131",
            "4242",
            "4243",
            "error stopped",
            "66",
            "67",
            "77",
            "78",
        ] {
            assert!(!p.contains(needle), "{needle:?} in\n{p}");
        }
        assert!(p.contains("runaway stopped 2 of 2 clean runs at turns 171-195"), "{p}");
        // The owner's table marks the screen stops with a star, and has no stop detail for the
        // other tiers.
        let owner = ledger_text(&l, true);
        assert!(owner.contains("101* / 51*"), "{owner}");
        assert!(!owner.contains("GATE_WHY") && !owner.contains("OWNER_WHY"), "{owner}");
    }

    /// The owner-only tier is not in the ledger text a prompt gets, in any form, even in a ledger
    /// full of it: no score, no seed, no failure count, no word for it.
    #[test]
    fn nothing_from_the_owner_tier_reaches_a_prompt() {
        let mut l = stopped_ledger();
        l.rows[0].owner = vec![
            SeedScore {
                deaths: Some(41),
                ..stopped(301, 55555.0, "error", 4242, Some("OWNER_WHY"))
            },
            SeedScore { deaths: Some(42), ..stopped(302, 66666.0, "error", 4243, None) },
        ];
        let c = champion(1, 76.0);
        for text in [attempt_prompt(&l, 3, &c, "card\n", 5, 30), ledger_text(&l, false)] {
            for needle in
                ["55555", "66666", "301", "302", "OWNER_WHY", "41", "42", "owner", "Owner"]
            {
                assert!(!text.contains(needle), "{needle:?} in\n{text}");
            }
        }
        assert!(ledger_text(&l, true).contains("owner-only"), "the owner's table has it");
    }

    #[test]
    fn the_prompt_says_a_stopped_clean_run_coasts_and_names_the_test_tool() {
        let c = champion(1, 76.0);
        let p = attempt_prompt(&ledger(), 3, &c, "card\n", 5, 30);
        assert!(p.contains("coasts to the end with no orders"), "{p}");
        assert!(p.contains("costs most of the score") && p.contains("test_governor"), "{p}");
        assert!(p.contains("from turn 0") && p.contains("this game is not touched"), "{p}");
    }

    #[test]
    fn attempt_options_hand_the_tool_the_screen_seeds_and_nothing_else() {
        let t = track();
        let base = parse_args_with(&["--chunk-calls", "7"], &mut || 1).unwrap();
        let o = attempt_opts(&base, &t, None, None, None);
        assert_eq!(o.trial, Some((vec![1, 2], 5)));
        let args = serve_args(&o, Path::new("/a/run.jsonl")).join(" ");
        assert!(args.contains("--trial-seeds 1,2 --trial-turns 5"), "{args}");
        for seed in ["101", "102", "301", "302"] {
            assert!(!args.contains(seed), "{seed} in {args}");
        }
    }

    /// `test_governor` on the map being played is a lookahead copy (#570): fine for climbing,
    /// wrong for a race. Only a campaign attempt (a track, with screen seeds to name) turns the
    /// tool on, and no flag does: the one assignment of `Opts::trial` outside the parser's
    /// default is in `attempt_opts`, which this pins by reading the sources.
    #[test]
    fn only_a_campaign_attempt_turns_test_governor_on() {
        for args in [
            &[][..],
            &["--scripts"],
            &["--seed", "7", "--nation", "water", "--scripts", "--turns", "30"],
            &["--chunk-calls", "10", "--max-usd", "5", "--model", "x"],
        ] {
            let o = parse_args_with(args, &mut || 1).unwrap();
            assert!(o.trial.is_none(), "{args:?}");
            let served = serve_args(&o, Path::new("/a/run.jsonl")).join(" ");
            assert!(!served.contains("--trial") && !served.contains("--horizon"), "{served}");
        }
        let needle = concat!(".trial = ", "Some(");
        let parser = include_str!("../agent_run.rs");
        let campaign = include_str!("campaign.rs");
        assert_eq!(parser.matches(needle).count(), 0, "agent_run.rs sets Opts::trial");
        assert_eq!(campaign.matches(needle).count(), 1, "campaign.rs sets it in one place");
        let at = campaign.find(needle).unwrap();
        assert!(
            campaign[..at]
                .rfind("fn attempt_opts")
                .is_some_and(|f| !campaign[f..at].contains("\nfn "))
        );
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
        b.gate_seeds = vec![101, 105];
        let d = track_diff(&a, &b).join("; ");
        assert!(d.contains("horizon_turns was 5, is now 9"), "{d}");
        assert!(d.contains("gate_seeds"), "{d}");
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
        let empty =
            Ledger { track: track(), rows: Vec::new(), reevals: Vec::new(), references: None };
        assert_eq!(next_attempt(&tmp.join("nowhere"), &empty), 1);
        let _ = fs::remove_dir_all(&tmp);
    }

    /// #566: a ledger and a champion record written without a build read back, and are written
    /// back without the new fields. #571: a seed score without deaths reads back too.
    #[test]
    fn a_ledger_without_a_build_reads_back_and_one_with_round_trips() {
        let mut l = ledger();
        let text = serde_json::to_string(&l).unwrap();
        assert!(!text.contains("build") && !text.contains("reevals"), "{text}");
        assert!(!text.contains("deaths") && !text.contains("verdict"), "{text}");
        let back: Ledger = serde_json::from_str(&text).unwrap();
        assert_eq!(back, l);
        assert!(back.reevals.is_empty() && back.rows[0].build.is_none());
        let no_build = r#"{"attempt":1,"screen_mean":202.5,"gate_mean":199.0,
            "governor_version":9,"plan_revision":6}"#;
        let champ: Champion = serde_json::from_str(no_build).unwrap();
        assert_eq!(champ.build, None);

        // And with them, they round-trip.
        l.rows[0].build = Some("abc1234+0f0f0f0f".to_owned());
        l.reevals.push(Reeval {
            attempt: 1,
            after_rows: 2,
            was_build: None,
            was_screen_mean: 355.0,
            was_gate_mean: 300.0,
            build: "def5678".to_owned(),
            screen_mean: 294.0,
            gate_mean: 250.0,
        });
        let back: Ledger = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(back, l);
    }

    /// #571: a ledger written before the seed tiers is refused with a message that says why and
    /// what to do, not with a serde error about a missing field. It names the old track shape
    /// (train and held-out seeds, no gate) however the rest of the file looks.
    #[test]
    fn a_ledger_from_before_the_tiers_is_refused_clearly() {
        let old = json!({
            "track": {
                "id": "water-3h", "name": "W", "nation": "water", "difficulty": "normal",
                "map_size": "small", "site": "default", "prelude": null, "horizon_turns": 540,
                "play_seed": 7633, "train_seeds": [7633, 7, 42], "holdout_seeds": [1, 5, 8]
            },
            "rows": [{
                "attempt": 1, "governor_version": 9, "plan_revision": 6, "played": 256.4,
                "clean": [{"seed": 7633, "score": 231.0}], "train_mean": 231.0,
                "held_out": [{"seed": 1, "score": 188.0}], "kept": true, "cost_usd": 0.97
            }]
        });
        assert!(is_pre_tier_ledger(&old));
        let new = serde_json::to_value(ledger()).unwrap();
        assert!(!is_pre_tier_ledger(&new));

        let tmp = std::env::temp_dir().join(format!("fn_campaign_old_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join(LEDGER), old.to_string()).unwrap();
        let e = read_ledger(&tmp).unwrap_err();
        assert!(
            e.contains("before the seed tiers")
                && e.contains("--track ID")
                && e.contains("agent-compare"),
            "{e}"
        );
        assert!(!e.contains("missing field") && !e.contains("unknown field"), "{e}");
        // The same ledger in the new shape reads.
        fs::write(tmp.join(LEDGER), new.to_string()).unwrap();
        assert_eq!(read_ledger(&tmp).unwrap(), ledger());
        let _ = fs::remove_dir_all(&tmp);
    }

    /// The champion's gate scores are used only for the build and gate seeds they were made on.
    #[test]
    fn the_champions_gate_scores_are_read_only_for_their_build_and_seeds() {
        let tmp = std::env::temp_dir().join(format!("fn_campaign_gate_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(champion_dir(&tmp)).unwrap();
        let t = track();
        assert!(read_champion_gate(&tmp, &t, "b1").is_none(), "no file");
        let gate =
            ChampionGate { build: "b1".to_owned(), scores: seeds(&[(101, 5.0), (102, 6.0)]) };
        write_json(&champion_dir(&tmp).join("gate.json"), &gate).unwrap();
        assert_eq!(read_champion_gate(&tmp, &t, "b1"), Some(gate.scores.clone()));
        assert!(read_champion_gate(&tmp, &t, "b2").is_none(), "another build");
        let mut other = t.clone();
        other.gate_seeds = vec![101, 103];
        assert!(read_champion_gate(&tmp, &other, "b1").is_none(), "other gate seeds");
        let _ = fs::remove_dir_all(&tmp);
    }

    /// The re-evaluation is on the ledger text, screen and gate means only, old value and new,
    /// with the build each came from; an unchanged one says so.
    #[test]
    fn the_ledger_text_shows_the_old_and_the_re_evaluated_means() {
        let mut l = ledger();
        assert!(!ledger_text(&l, false).contains("re-evaluated"));
        l.reevals.push(Reeval {
            attempt: 1,
            after_rows: 2,
            was_build: Some("abc1234".to_owned()),
            was_screen_mean: 355.0,
            was_gate_mean: 300.0,
            build: "def5678+0f0f0f0f".to_owned(),
            screen_mean: 294.0,
            gate_mean: 250.0,
        });
        l.reevals.push(Reeval {
            attempt: 1,
            after_rows: 2,
            was_build: None,
            was_screen_mean: 294.0,
            was_gate_mean: 250.0,
            build: "9999999".to_owned(),
            screen_mean: 294.0,
            gate_mean: 250.0,
        });
        for owner in [false, true] {
            let t = ledger_text(&l, owner);
            assert!(
                t.contains("champion (attempt 1) re-evaluated on build def5678+0f0f0f0f after 2")
                    && t.contains("means were from build abc1234)")
                    && t.contains("screen mean 355 -> 294, gate mean 300 -> 250"),
                "{t}"
            );
            assert!(
                t.contains("from an unrecorded build): screen mean 294, unchanged, gate mean 250, unchanged"),
                "{t}"
            );
        }
        assert!(
            !ledger_text(&l, false).contains("7771"),
            "the owner tier stays out of the prompt text"
        );
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
        // The live game stops at the track's horizon, on every chunk's serve line (#569).
        assert!(args.contains("--trial-turns 5 --horizon 5"), "{args}");
        let resumed = resume_args(Path::new("/a/run.jsonl"), true);
        let resumed = [resumed, trial_args(&o)].concat().join(" ");
        assert!(resumed.contains("--horizon 5"), "{resumed}");
    }

    /// The phrases every prompt path of a campaign carries (#569): what decides the attempt, how
    /// to test it, and where the live game ends.
    const DECIDING: [&str; 5] = [
        "the clean score",
        "from turn 0",
        "test_governor",
        "several other maps",
        "ends at turn 540",
    ];

    fn all_prompt_paths() -> Vec<(&'static str, String)> {
        let l = ledger();
        let c = champion(1, 202.5);
        vec![
            ("attempt 1", first_attempt_prompt(540, 30)),
            ("later attempt", attempt_prompt(&l, 2, &c, "card\n", 540, 30)),
            (
                "handoff with turns",
                scripts_handoff_prompt(540, 100, 30, "(listing)", Some(&handoff_note(540, 100, 2))),
            ),
            (
                "handoff with none",
                scripts_handoff_prompt(540, 0, 30, "(listing)", Some(&handoff_note(540, 0, 0))),
            ),
        ]
    }

    #[test]
    fn every_prompt_path_says_what_decides_and_how_to_test() {
        for (path, p) in all_prompt_paths() {
            for phrase in DECIDING {
                assert!(p.contains(phrase), "{phrase:?} missing from the {path} prompt:\n{p}");
            }
        }
    }

    #[test]
    fn the_shared_passage_is_written_once_per_prompt() {
        for (path, p) in all_prompt_paths() {
            assert_eq!(p.matches(&deciding_passage(540)).count(), 1, "{path}:\n{p}");
        }
        // The handoff doesn't carry the first prompt's extra detail: it would cost tokens every
        // chunk.
        let handoff = scripts_handoff_prompt(540, 100, 30, "(l)", Some(&handoff_note(540, 100, 0)));
        assert!(!handoff.contains(clean_run_rules()));
    }

    /// A chunk that played no turn is the same attempt carrying on, not a fresh start, and says
    /// what's been done; one that did says where the game is.
    #[test]
    fn a_handoff_with_no_turns_played_is_not_a_fresh_start() {
        let none = scripts_handoff_prompt(540, 0, 30, "(l)", Some(&handoff_note(540, 0, 3)));
        assert!(none.contains("not a fresh start") && none.contains("everything they saved"));
        assert!(none.contains("called 3 time(s)"), "{none}");
        assert!(!none.contains("turn 0 of") && !none.contains("so far)"), "{none}");

        let some = scripts_handoff_prompt(540, 120, 30, "(l)", Some(&handoff_note(540, 120, 0)));
        assert!(some.contains("live game is at turn 120 of 540"), "{some}");
        assert!(!some.contains("test_governor has been called"), "no tests used yet: {some}");
    }

    #[test]
    fn an_ordinary_handoff_is_what_it_was() {
        let p = scripts_handoff_prompt(540, 100, 30, "(listing)", None);
        assert!(p.starts_with(
            "You're continuing the same settlement from where you left off (turn 100 of about \
             540 so far), not starting a new one. Here is your own plan, script library and lab \
             notes:\n\n(listing)\n\nCarry on the same way."
        ));
        assert!(!p.contains("clean score"), "{p}");
    }

    #[test]
    fn the_rules_make_no_false_claim_about_the_work_limit() {
        let r = clean_run_rules();
        assert!(r.contains("each wait starts the count again"), "{r}");
        assert!(!r.contains("budget"), "'budget' means the turn budget and the dollar cap: {r}");
    }

    #[test]
    fn work_marks_count_what_a_chunk_changed() {
        let call = |tool: &str| LogLine::Call { tool: tool.to_owned() };
        assert_eq!(work_marks(&[call("observe"), call("run_script"), LogLine::End]), 0);
        let editing = [
            call("save_script"),
            LogLine::Script { name: "governor".into(), version: 2, code: String::new(), turns: 0 },
            LogLine::Note { text: "n".into() },
            call("test_governor"),
        ];
        assert_eq!(work_marks(&editing), 3, "no turn was played, and it still counts");
        assert_eq!(tests_used(&editing), 1);
        assert_eq!(work_marks(&[LogLine::Turn { index: 1, tick: 10 }]), 1);
    }
}

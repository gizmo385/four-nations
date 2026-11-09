//! Campaign rows for `agent-run watch --list` (#585): a campaign folder, then its attempts.
//!
//! A small read-only version of what the run browser reads (`crates/tui/src/runs/campaign.rs`),
//! on the same `fn_runfmt` ledger rows. It reads both ledger shapes (the first one's
//! `train_mean`, the seed tiers' `screen_mean`), and every field is optional: a missing or
//! partial file is a `-`, never a crash, and a row that does not parse is skipped.

use super::*;

/// How recently a campaign folder was written to for it to count as running, and the shorter
/// wait after every attempt has its row (the next one starts at once).
const LIVE_WITHIN: Duration = Duration::from_secs(300);
const BETWEEN_ATTEMPTS: Duration = Duration::from_secs(60);

fn json(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn mtime(path: &Path) -> Option<SystemTime> {
    path.metadata().ok()?.modified().ok()
}

fn seed_logs(dir: &Path) -> usize {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with("seed-") && n.ends_with(".jsonl")
        })
        .count()
}

fn sub_dirs(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

/// The attempt's clean logs: where `eval/versions.json` says the chosen pair is (`v5-6`), or the
/// folder itself in the layout from before pairs.
fn chosen_pair(attempt: &Path) -> Option<String> {
    let versions = json(&attempt.join("eval/versions.json"))?;
    let pairs = versions["pairs"].as_array()?;
    let (n, p) = pairs.iter().enumerate().find(|(_, p)| p["chosen"] == true)?;
    Some(format!("v{}-{}", p["version"].as_u64()?, n + 1))
}

fn money(c: Option<f64>) -> Option<String> {
    c.map(|c| format!("${c:.2}"))
}

/// What the open attempt (the newest, with no ledger row yet) is doing.
fn activity(attempt: &Path) -> &'static str {
    if attempt.join("stop.json").is_file()
        || seed_logs(&attempt.join("eval")) > 0
        || sub_dirs(&attempt.join("eval")).iter().any(|d| seed_logs(d) > 0)
    {
        return "evaluating";
    }
    let tail = fs::read_to_string(attempt.join("run.jsonl")).unwrap_or_default();
    for line in tail.lines().rev() {
        if line.starts_with(r#"{"type":"turn""#) {
            break;
        }
        if line.starts_with(r#"{"type":"call""#) {
            return if line.contains(r#""tool":"test_governor""#) { "testing" } else { "playing" };
        }
    }
    "playing"
}

/// The newest write in the ledger and the newest attempt (its files and its clean logs).
fn newest_write(dir: &Path, last_attempt: Option<&Path>) -> Option<SystemTime> {
    let mut times = vec![mtime(&dir.join("ledger.json"))];
    if let Some(a) = last_attempt {
        let eval = a.join("eval");
        let mut folders = vec![a.to_path_buf(), eval.clone()];
        folders.extend(sub_dirs(&eval));
        for f in folders {
            for e in fs::read_dir(f).into_iter().flatten().filter_map(|e| e.ok()) {
                if e.path().is_file() {
                    times.push(e.metadata().ok().and_then(|m| m.modified().ok()));
                }
            }
        }
    }
    times.into_iter().flatten().max()
}

/// The rows for campaign `dir`: itself, then an indented row per attempt (oldest first).
pub(super) fn campaign_rows(dir: &Path, now: SystemTime) -> Vec<RunRow> {
    let ledger = json(&dir.join("ledger.json")).unwrap_or(serde_json::Value::Null);
    let ledger_rows: Vec<fn_runfmt::campaign::Row> = ledger["rows"]
        .as_array()
        .map(|r| r.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect())
        .unwrap_or_default();
    let row_of = |k: u32| ledger_rows.iter().find(|r| r.attempt == k);

    let mut numbers: Vec<u32> = attempt_dirs(dir).into_iter().map(|(k, _)| k).collect();
    numbers.extend(ledger_rows.iter().map(|r| r.attempt));
    numbers.sort_unstable();
    numbers.dedup();

    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let track = &ledger["track"];
    let text = |v: &serde_json::Value| v.as_str().map_or("-".to_string(), str::to_string);
    let nation = text(&track["nation"]);

    let kept = |r: &&fn_runfmt::campaign::Row| r.kept;
    let champion = ledger_rows.iter().rev().find(|r| kept(r));
    let cost: f64 = numbers
        .iter()
        .filter_map(|k| {
            row_of(*k)
                .and_then(|r| r.cost_usd)
                .or_else(|| json(&dir.join(format!("attempt-{k}/cost.json")))?["usd"].as_f64())
        })
        .sum();

    let last = numbers.last().map(|k| dir.join(format!("attempt-{k}")));
    let open = numbers.last().filter(|k| row_of(**k).is_none());
    let quiet = newest_write(dir, last.as_deref()).and_then(|t| now.duration_since(t).ok());
    let window = if open.is_some() { LIVE_WITHIN } else { BETWEEN_ATTEMPTS };
    let state = match (numbers.last(), open) {
        (Some(_), Some(k)) if quiet.is_some_and(|q| q < window) => {
            format!("live: attempt {k} {}", activity(&dir.join(format!("attempt-{k}"))))
        }
        (Some(k), None) if quiet.is_some_and(|q| q < window) => {
            format!("live: attempt {} starting", k + 1)
        }
        (_, Some(_)) => "stopped early".to_string(),
        _ => "finished".to_string(),
    };
    let mut detail = format!(
        "{} attempt(s), {} kept",
        numbers.len(),
        ledger_rows.iter().filter(|r| kept(r)).count()
    );
    if let Some(c) = money((!numbers.is_empty()).then_some(cost)) {
        detail += &format!(", {c}");
    }
    if let Some(r) = champion {
        detail += &format!(", best is attempt {}", r.attempt);
    }
    detail += &format!(", {state}");

    let mut rows = vec![RunRow {
        name,
        nation: nation.clone(),
        start: text(&track["id"]),
        turns: 0,
        score: champion.and_then(|r| r.clean_mean()),
        playtest: false,
        campaign: true,
        detail: Some(detail),
    }];
    for k in numbers {
        let adir = dir.join(format!("attempt-{k}"));
        let mut row = run_row(&adir);
        row.name = format!("  attempt-{k}");
        if row.nation == "-" {
            row.nation = nation.clone();
        }
        let ledger_row = row_of(k);
        row.start = ledger_row
            .and_then(|r| r.governor_version)
            .map_or("-".to_string(), |v| format!("v{v}"));
        row.score = ledger_row.and_then(|r| r.clean_mean());
        let mut parts = Vec::new();
        parts.push(match ledger_row {
            Some(r) if r.kept => "kept".to_string(),
            Some(_) => "not kept".to_string(),
            None => "no ledger row yet".to_string(),
        });
        if let Some(p) = ledger_row.and_then(|r| r.played) {
            parts.push(format!("played {p:.1}"));
        }
        let attempt_cost = ledger_row
            .and_then(|r| r.cost_usd)
            .or_else(|| json(&adir.join("cost.json"))?["usd"].as_f64());
        parts.extend(money(attempt_cost));
        if row.turns == 0 {
            parts.push("no live turns".to_string());
        }
        let eval = adir.join("eval");
        let clean = match chosen_pair(&adir) {
            Some(pair) => Some((format!("eval/{pair}"), seed_logs(&eval.join(&pair)))),
            None if seed_logs(&eval) > 0 => Some(("eval".to_string(), seed_logs(&eval))),
            None => None,
        };
        if let Some((at, n)) = clean {
            parts.push(format!("{n} clean run(s) in {at}"));
        }
        row.detail = Some(parts.join(", "));
        rows.push(row);
    }
    rows
}

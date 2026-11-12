//! The verdict as text (`docs/design/balance-harness.md`, section 5, "What it prints"): the
//! target table, hard sites, flagged runs by bucket with a `balance trace` command each,
//! outliers, balance misses, care rates, sensitivity pairs and the verdict line.
//!
//! Plain text and pure: the same [`Verdict`] always renders to the same string, with no timing
//! or paths in it, so two runs of a panel can be compared with `diff`.

use std::fmt::Write as _;

use fn_sim::Nation;
use fn_sim::balance::Raised;

use crate::verdict::{
    Bucket, Classified, FlaggedRun, NationVerdict, PairStatus, Status, TargetVerdict, Verdict,
};

/// Seeds listed per target under "balance miss" before the rest are only counted.
const MISS_LINES: usize = 8;

/// What the report says about the panel it summarises.
#[derive(Debug, Clone)]
pub struct Header {
    pub label: String,
    /// The seeds, as the caller wrote them (`1-32`).
    pub seeds: String,
    pub nations: Vec<Nation>,
}

/// The `balance trace` command that reruns one run (#535's syntax: `balance trace --seed S
/// --nation N [--style ID] [--preset P] --from M --to M [--map-at M]`). The style and preset are
/// left out for the reference run, `steady` on Normal.
pub fn trace_command(
    style: &str,
    preset: &str,
    nation: Nation,
    seed: u64,
    window: (u32, u32),
    map_at: u32,
) -> String {
    let mut cmd = format!("balance trace --seed {seed} --nation {}", nation.as_str());
    if style != "steady" {
        let _ = write!(cmd, " --style {style}");
    }
    if preset != "normal" {
        let _ = write!(cmd, " --preset {preset}");
    }
    let _ = write!(cmd, " --from {} --to {} --map-at {map_at}", window.0, window.1);
    cmd
}

/// A flagged run's trace: a few minutes before the first flag's condition began to a few after
/// the last flag or the first death, with the map at the first death (else the first flag).
fn flagged_trace(f: &FlaggedRun) -> String {
    let since = f.flags.iter().map(|r| r.since).fold(f32::INFINITY, f32::min);
    let last =
        f.flags.iter().map(|r| r.minute).fold(0.0_f32, f32::max).max(f.first_death.unwrap_or(0.0));
    let first = f.flags.iter().map(|r| r.minute).fold(f32::INFINITY, f32::min);
    let from = (since - 5.0).max(0.0).floor() as u32;
    let to = ((last + 5.0).ceil()).min(f.minutes.max(1.0)) as u32;
    let map_at = f.first_death.unwrap_or(first).round() as u32;
    trace_command(&f.style, &f.preset, f.nation, f.seed, (from, to.max(from)), map_at)
}

/// An outlier's or miss's trace: the half hour before its deadline, with the map at it.
fn deadline_trace(style: &str, preset: &str, c: &Classified) -> String {
    let to = c.deadline.ceil() as u32;
    let from = to.saturating_sub(30);
    trace_command(style, preset, c.nation, c.seed, (from, to), to)
}

/// `doc` short: the file name without `.md`, and the section number when the anchor starts with
/// one (`materials.md#7-measuring-it` is `materials #7`).
fn doc_short(doc: &str) -> String {
    let (file, anchor) = doc.split_once('#').unwrap_or((doc, ""));
    let stem = file.rsplit('/').next().unwrap_or(file).trim_end_matches(".md");
    let first = anchor.split('-').next().unwrap_or("");
    if !first.is_empty() && first.chars().all(|c| c.is_ascii_digit()) {
        format!("{stem} #{first}")
    } else {
        stem.to_owned()
    }
}

fn value_text(v: f64) -> String {
    if v.is_nan() {
        "no reading".to_owned()
    } else if v.is_infinite() {
        "never".to_owned()
    } else if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

fn flags_text(flags: &[Raised]) -> String {
    flags
        .iter()
        .map(|r| format!("{} from {:.0}", r.flag.name(), r.since))
        .collect::<Vec<_>>()
        .join(", ")
}

fn nation_list(nations: &[Nation]) -> String {
    nations.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(" ")
}

fn share_text(share: f64) -> String {
    format!("{:.0}%", share * 100.0)
}

fn cell(passes: u32, counted: u32, status: Status) -> String {
    let word = match status {
        Status::Pass => "pass",
        Status::Fail => "FAIL",
        Status::NoData => "no data",
    };
    format!("{passes}/{counted} {word}")
}

/// Left-aligned columns, two spaces apart, trailing blanks trimmed.
fn table(rows: &[Vec<String>]) -> String {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..cols)
        .map(|c| rows.iter().filter_map(|r| r.get(c)).map(|s| s.chars().count()).max().unwrap_or(0))
        .collect();
    let mut out = String::new();
    for row in rows {
        let mut line = String::new();
        for (c, text) in row.iter().enumerate() {
            let _ = write!(line, "{text:<w$}  ", w = widths[c]);
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// A labelled block: the label on the first line, the rest indented under it.
fn block(out: &mut String, label: &str, lines: &[String]) {
    for (i, line) in lines.iter().enumerate() {
        let _ = writeln!(out, "{:<12} {line}", if i == 0 { label } else { "" });
    }
}

fn target_rows(verdict: &Verdict, nations: &[Nation]) -> Vec<Vec<String>> {
    let mut rows = vec![{
        let mut h =
            vec!["target".to_owned(), "bar".to_owned(), "run".to_owned(), "share".to_owned()];
        h.extend(nations.iter().map(|n| n.as_str().to_owned()));
        h.push("doc".to_owned());
        h
    }];
    for tv in &verdict.targets {
        let t = &tv.target;
        let by_nation = |n: Nation| tv.nations.iter().find(|v| v.nation == n);
        let mut row = vec![
            t.id.clone(),
            t.bar.to_string(),
            format!("{}/{}", t.style, t.preset),
            share_text(t.share),
        ];
        row.extend(nations.iter().map(|&n| match by_nation(n) {
            Some(v) => cell(v.passes, v.counted, v.status),
            None => "no data".to_owned(),
        }));
        row.push(doc_short(&t.doc));
        rows.push(row);
        if let Some(h) = &t.hard_expect {
            let mut row = vec![
                "  on hard sites".to_owned(),
                h.bar.to_string(),
                String::new(),
                share_text(h.share),
            ];
            row.extend(nations.iter().map(|&n| match by_nation(n) {
                Some(NationVerdict { hard, hard_passes, hard_status: Some(s), .. }) => {
                    cell(*hard_passes, *hard, *s)
                }
                _ => "no data".to_owned(),
            }));
            rows.push(row);
        }
    }
    rows
}

/// Per target, how many runs each nation had excluded: a flagged run is never counted.
fn excluded_lines(verdict: &Verdict) -> Vec<String> {
    let mut lines = Vec::new();
    for tv in &verdict.targets {
        let parts: Vec<String> = tv
            .nations
            .iter()
            .filter(|n| n.sim_locks + n.harness_gaps > 0)
            .map(|n| {
                format!(
                    "{} {} sim lock, {} harness gap",
                    n.nation.as_str(),
                    n.sim_locks,
                    n.harness_gaps
                )
            })
            .collect();
        if !parts.is_empty() {
            lines.push(format!("{}: {}", tv.target.id, parts.join("; ")));
        }
    }
    lines
}

/// Flagged runs grouped by (bucket, run, seed, flags): nations that tripped the same thing on
/// the same seed share a line.
fn flagged_lines(verdict: &Verdict, bucket: Bucket) -> Vec<String> {
    let mut groups: Vec<(String, u64, String, Vec<&FlaggedRun>)> = Vec::new();
    for f in verdict.flagged.iter().filter(|f| f.bucket == bucket) {
        let run = format!("{}/{}", f.style, f.preset);
        let mut desc = flags_text(&f.flags);
        if let Some(d) = f.first_death {
            let _ = write!(desc, ", first death {d:.0}");
        }
        match groups.iter_mut().find(|g| g.0 == run && g.1 == f.seed && g.2 == desc) {
            Some(g) => g.3.push(f),
            None => groups.push((run, f.seed, desc, vec![f])),
        }
    }
    groups
        .into_iter()
        .map(|(run, seed, desc, fs)| {
            let nations: Vec<Nation> = fs.iter().map(|f| f.nation).collect();
            let run_note = if run == "steady/normal" { String::new() } else { format!(" ({run})") };
            format!("{} {seed}  {desc}{run_note}  {}", nation_list(&nations), flagged_trace(fs[0]))
        })
        .collect()
}

/// `(target, seed)` groups of the runs in `bucket` that missed.
fn miss_groups(verdict: &Verdict, bucket: Bucket) -> Vec<(&TargetVerdict, u64, Vec<&Classified>)> {
    let mut groups: Vec<(&TargetVerdict, u64, Vec<&Classified>)> = Vec::new();
    for tv in &verdict.targets {
        for c in tv.runs.iter().filter(|c| c.bucket == bucket && !c.met) {
            match groups.iter_mut().find(|g| g.0.target.id == tv.target.id && g.1 == c.seed) {
                Some(g) => g.2.push(c),
                None => groups.push((tv, c.seed, vec![c])),
            }
        }
    }
    groups
}

fn miss_line(tv: &TargetVerdict, seed: u64, runs: &[&Classified], with_trace: bool) -> String {
    let t = &tv.target;
    let nations: Vec<Nation> = runs.iter().map(|c| c.nation).collect();
    let first = runs[0];
    let mut line = format!(
        "{} {seed}  {}: {} (needs {})",
        nation_list(&nations),
        t.id,
        value_text(first.value),
        t.bar
    );
    if let Some((sign, supply)) = &first.supply {
        let _ = write!(line, ", {supply} {sign} in reach");
    }
    if first.met_by.is_empty() {
        line.push_str(", nobody met it");
    } else {
        let _ = write!(line, ", met by {}", nation_list(&first.met_by));
    }
    if with_trace {
        let _ = write!(line, "  {}", deadline_trace(&t.style, &t.preset, first));
    }
    line
}

/// The whole report (module doc).
pub fn render(verdict: &Verdict, header: &Header) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "balance verdict  seeds {}  nations {}  {} runs  label {}\n",
        header.seeds,
        nation_list(&header.nations),
        verdict.runs,
        header.label
    );
    out.push_str(&table(&target_rows(verdict, &header.nations)));
    out.push('\n');

    let excluded = excluded_lines(verdict);
    if !excluded.is_empty() {
        block(&mut out, "excluded", &excluded);
    }

    for h in &verdict.hard_sites {
        let lines: Vec<String> = h
            .nations
            .iter()
            .map(|(nation, seeds, total)| {
                let list = seeds.iter().map(u64::to_string).collect::<Vec<_>>().join(" ");
                format!(
                    "{} below {}x need on {} of {}: {list}",
                    nation.as_str(),
                    h.below,
                    seeds.len(),
                    total
                )
            })
            .collect();
        let lines =
            std::iter::once(format!("{} supply", h.supply)).chain(lines).collect::<Vec<_>>();
        block(&mut out, "hard sites", &lines);
    }

    for (bucket, label) in [(Bucket::SimLock, "sim lock"), (Bucket::HarnessGap, "harness gap")] {
        let lines = flagged_lines(verdict, bucket);
        if !lines.is_empty() {
            block(&mut out, label, &lines);
        }
    }

    let outliers = miss_groups(verdict, Bucket::Outlier);
    if !outliers.is_empty() {
        let lines: Vec<String> =
            outliers.iter().map(|(tv, seed, rs)| miss_line(tv, *seed, rs, true)).collect();
        block(&mut out, "outlier", &lines);
    }
    let misses = miss_groups(verdict, Bucket::Balance);
    if !misses.is_empty() {
        // A target that misses everywhere would bury the rest: show a few per target and count
        // the others (they are all in the records).
        let mut lines: Vec<String> = Vec::new();
        for tv in &verdict.targets {
            let mine: Vec<_> = misses.iter().filter(|g| g.0.target.id == tv.target.id).collect();
            lines.extend(
                mine.iter().take(MISS_LINES).map(|(tv, seed, rs)| miss_line(tv, *seed, rs, false)),
            );
            if mine.len() > MISS_LINES {
                lines.push(format!(
                    "{}: and {} more seeds missed (see the records)",
                    tv.target.id,
                    mine.len() - MISS_LINES
                ));
            }
        }
        block(&mut out, "balance miss", &lines);
    }

    for row in &verdict.care {
        let by = |f: fn(&(Nation, f64, f64)) -> f64| {
            row.nations
                .iter()
                .map(|n| format!("{} {:.1}", n.0.as_str(), f(n)))
                .collect::<Vec<_>>()
                .join("  ")
        };
        block(
            &mut out,
            "care",
            &[format!(
                "{}/{}  depleted min/ph  {}  close calls/10ph  {}",
                row.style,
                row.preset,
                by(|n| n.1),
                by(|n| n.2)
            )],
        );
    }

    let pair_lines: Vec<String> = verdict
        .pairs
        .iter()
        .map(|p| {
            let sides = format!(
                "{}/{} {:.2} vs {}/{} {:.2}",
                p.pair.higher.style,
                p.pair.higher.preset,
                p.mean_higher,
                p.pair.lower.style,
                p.pair.lower.preset,
                p.mean_lower
            );
            let state = match p.status {
                PairStatus::Separates => {
                    format!("separates on {} of {} paired runs", p.separated, p.paired)
                }
                PairStatus::CantFail => format!(
                    "CAN'T FAIL: higher reads above on only {} of {} paired runs (needs {})",
                    p.separated,
                    p.paired,
                    share_text(p.pair.share)
                ),
                PairStatus::NoData => format!("no data: {} paired runs", p.paired),
            };
            format!("{}  {state}  [{sides}]  {}", p.pair.id, doc_short(&p.pair.doc))
        })
        .collect();
    if !pair_lines.is_empty() {
        block(&mut out, "sensitivity", &pair_lines);
    }

    let fork_lines: Vec<String> = verdict
        .forks
        .iter()
        .flat_map(|f| {
            let head = format!(
                "{}  {}  [{}/{} vs {}/{}, ties within {}, max {}]  {}",
                f.fork.id,
                match f.status {
                    Status::Pass => "pass",
                    Status::Fail => "FAIL",
                    Status::NoData => "no data",
                },
                f.fork.a.style,
                f.fork.a.preset,
                f.fork.b.style,
                f.fork.b.preset,
                f.fork.tie,
                share_text(f.fork.max_share),
                doc_short(&f.fork.doc)
            );
            std::iter::once(head).chain(f.nations.iter().map(|n| {
                format!(
                    "  {:<6} {} a, {} b, {} ties of {}{}",
                    n.nation.as_str(),
                    n.a_wins,
                    n.b_wins,
                    n.ties,
                    n.paired,
                    match n.status {
                        Status::Fail => "  FAIL",
                        Status::NoData => "  no data",
                        Status::Pass => "",
                    }
                )
            }))
        })
        .collect();
    if !fork_lines.is_empty() {
        block(&mut out, "forks", &fork_lines);
    }

    block(&mut out, "verdict", &[verdict_line(verdict)]);
    out
}

/// The last line: how many targets pass, fail and lack data, and what else to look at.
pub fn verdict_line(verdict: &Verdict) -> String {
    let fails: Vec<String> = verdict
        .targets
        .iter()
        .filter(|t| t.status == Status::Fail)
        .map(|t| {
            let nations: Vec<Nation> = t.failing_nations();
            format!("{} {}", t.target.id, nation_list(&nations))
        })
        .collect();
    let mut line = format!("{} pass", verdict.count(Status::Pass));
    let fail_n = verdict.count(Status::Fail);
    if fail_n == 0 {
        line.push_str(", 0 fail");
    } else {
        let _ = write!(line, ", {fail_n} fail ({})", fails.join("; "));
    }
    let _ = write!(line, ", {} no data", verdict.count(Status::NoData));
    let hard_fails = verdict.targets.iter().filter(|t| t.hard_status == Some(Status::Fail)).count();
    let hard_bars = verdict.targets.iter().filter(|t| t.hard_status.is_some()).count();
    if hard_bars > 0 {
        let _ = write!(line, "; hard bars fail on {hard_fails} of {hard_bars} targets");
    }
    let _ = write!(line, "; {} outliers", verdict.outliers().len());
    let _ = write!(line, "; {} flagged runs", verdict.flagged.len());
    let forks_failing: Vec<&str> = verdict
        .forks
        .iter()
        .filter(|f| f.status == Status::Fail)
        .map(|f| f.fork.id.as_str())
        .collect();
    if !forks_failing.is_empty() {
        let _ = write!(line, "; forks fail ({})", forks_failing.join(", "));
    }
    let cant = verdict.pairs.iter().filter(|p| p.status == PairStatus::CantFail).count();
    if cant > 0 {
        let _ = write!(line, "; {cant} sensitivity checks can't fail");
    }
    line
}

#[cfg(test)]
mod tests;

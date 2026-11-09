//! What `test_governor` says about a run besides its score (#570): the scorecard's signs and
//! deaths, and a short trace by time slice that helps explain a failure.
//!
//! Everything in it is something a player could see, read off the trial's own world: the
//! People row (how many are alive, how many show a severe word), the stores, the idle count, the
//! death alerts, and the notices and errors the governor's own `act` calls got back. The
//! scorecard's signs are the clean evaluation's own, as `fn_agent score` prints them. Nothing is
//! about a map the trial didn't play, and nothing here knows a held-out seed.
//!
//! The trace is [`SLICES`] columns wide whatever the length of the run (a 540-turn run is 12
//! slices of 45 turns, 15 game minutes each, the width of the scorecard's idle windows). Two
//! forms, both bounded:
//! - the summary: score, signs, deaths and three rows (people, idle share, refused acts) with the
//!   one most common refusal. About 1.2 KB.
//! - the detail: the same with the stores, the settlers in severe need and the deaths in each
//!   slice, the five most common refusals with when they happened, and the last few distinct
//!   lines the governor's own `log()` wrote. About 2 KB, under 3.2 KB.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::clean::CleanRun;
use crate::scorecard::{Report, TurnSample};

/// How many columns the trace has, at most.
pub const SLICES: u32 = 12;

/// The longest a refusal or a log line gets, in characters.
const LINE_CHARS: usize = 90;

/// Refusals listed in the summary and the detail.
const TOP_SUMMARY: usize = 1;
const TOP_DETAIL: usize = 5;

/// Distinct governor log lines listed in the detail.
const LOG_LINES: usize = 6;

/// Deaths named before the rest are only counted.
const DEATHS_NAMED: usize = 8;

/// The pieces of a test's result that come from the scorecard and the trace.
#[derive(Debug, Clone, PartialEq)]
pub struct Brief {
    /// "Stone in stores t135; Food in stores 40/150; ...": a met sign says when, an unmet one
    /// how far it got.
    pub signs: String,
    /// "none", or "3: Kiluk starvation t120; ..." (turn of the death alert).
    pub deaths: String,
    /// The trace, as lines joined by newlines.
    pub trace: String,
}

/// The slice width and count for a run of `turns` turns.
fn slices(turns: u32) -> (u32, u32) {
    let width = turns.div_ceil(SLICES).max(1);
    (width, turns.div_ceil(width).max(1))
}

/// Builds the brief of a finished test: `report` is the scored replay of the trial's log, `run`
/// what the clean run itself noted (its refused acts and the script's log lines), `detail` picks
/// the long form.
pub fn brief(report: &Report, run: &CleanRun, detail: bool) -> Brief {
    let r = &report.replay;
    let (width, count) = slices(r.played_turns.max(1));
    let (width, count) = (width as usize, count as usize);
    let samples = &r.reported.turns;
    let in_slice = |i: usize| {
        let from = (i * width).min(samples.len());
        let to = ((i + 1) * width).min(samples.len());
        &samples[from..to]
    };

    let signs = report.signs_line();

    // Deaths, as the alerts say them.
    let deaths = &r.reported.deaths;
    let deaths_text = if deaths.is_empty() {
        "none".to_owned()
    } else {
        let mut named: Vec<String> = deaths
            .iter()
            .take(DEATHS_NAMED)
            .map(|d| format!("{} {} t{}", d.name, d.cause, d.turn))
            .collect();
        if deaths.len() > DEATHS_NAMED {
            named.push(format!("and {} more", deaths.len() - DEATHS_NAMED));
        }
        format!("{}: {}", deaths.len(), named.join("; "))
    };

    // Rows, one cell per slice.
    let mut rows: Vec<(&str, Vec<String>)> = Vec::new();
    rows.push((
        "turn",
        (0..count).map(|i| (((i + 1) * width).min(r.played_turns as usize)).to_string()).collect(),
    ));
    rows.push((
        "people",
        (0..count).map(|i| in_slice(i).last().map_or(0, |s| s.people).to_string()).collect(),
    ));
    rows.push((
        "idle%",
        (0..count)
            .map(|i| {
                let s = in_slice(i);
                let (idle, all) =
                    s.iter().fold((0u32, 0u32), |(a, b), t| (a + t.idle, b + t.people));
                format!(
                    "{:.0}",
                    if all == 0 { 0.0 } else { f64::from(idle) / f64::from(all) * 100.0 }
                )
            })
            .collect(),
    ));
    let mut refused = vec![0u32; count];
    for f in &run.failures {
        let i = (f.turn as usize / width).min(count - 1);
        refused[i] += 1;
    }
    // The most settlers in trouble with one need at any turn of the slice.
    let peak = |f: fn(&TurnSample) -> u32| -> Vec<String> {
        (0..count).map(|i| in_slice(i).iter().map(f).max().unwrap_or(0).to_string()).collect()
    };
    rows.push(("hungry", peak(|s| s.hungry)));
    rows.push(("thirsty", peak(|s| s.thirsty)));
    rows.push(("refused", refused.iter().map(u32::to_string).collect()));
    if detail {
        rows.push((
            "food",
            (0..count).map(|i| in_slice(i).last().map_or(0, |s| s.food).to_string()).collect(),
        ));
        rows.push(("tired", peak(|s| s.tired)));
        rows.push(("hurt", peak(|s| s.hurt)));
        let mut died = vec![0u32; count];
        for d in deaths {
            let i = ((d.turn.max(1) - 1) as usize / width).min(count - 1);
            died[i] += 1;
        }
        rows.push(("deaths", died.iter().map(u32::to_string).collect()));
    }

    let cell = rows.iter().flat_map(|(_, cells)| cells.iter().map(String::len)).max().unwrap_or(1);
    let label = rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    let secs = width as u32 * r.turn_seconds;
    let span =
        if secs.is_multiple_of(60) { format!("{} min", secs / 60) } else { format!("{secs} s") };
    let mut trace = format!(
        "slices of {width} turns ({span}). people{}: at slice end. idle%: average. hungry, \
         thirsty{}: most settlers showing it (mild or severe). refused: acts that did nothing \
         ({} of {}):\n",
        if detail { ", food" } else { "" },
        if detail { ", tired, hurt" } else { "" },
        run.acts_failed,
        run.acts_ok + run.acts_failed,
    );
    for (name, cells) in &rows {
        let _ = write!(trace, "{name:<label$}");
        for c in cells {
            let _ = write!(trace, " {c:>cell$}");
        }
        trace.push('\n');
    }

    // The most common refusals, with when.
    let mut tally: BTreeMap<&str, (u32, u32, u32)> = BTreeMap::new();
    for f in &run.failures {
        let e = tally.entry(f.message.as_str()).or_insert((0, f.turn, f.turn));
        e.0 += 1;
        e.2 = f.turn;
    }
    let mut top: Vec<(&str, (u32, u32, u32))> = tally.into_iter().collect();
    top.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
    let top_n = if detail { TOP_DETAIL } else { TOP_SUMMARY };
    for (msg, (n, first, last)) in top.iter().take(top_n) {
        let when = if first == last { format!("t{first}") } else { format!("t{first}-t{last}") };
        let _ = writeln!(trace, "refused {n}x ({when}): {}", clip(msg));
    }
    if run.acts_failed as usize > run.failures.len() {
        let _ = writeln!(
            trace,
            "({} refusals past the first {} are counted but not listed)",
            run.acts_failed as usize - run.failures.len(),
            run.failures.len()
        );
    }

    // The governor's own words, deduplicated, most recent last.
    if detail && !run.log.is_empty() {
        let mut lines: Vec<(&str, u32)> = Vec::new();
        for l in &run.log {
            match lines.iter().position(|(t, _)| t == l) {
                Some(at) => {
                    let (t, n) = lines.remove(at);
                    lines.push((t, n + 1));
                }
                None => lines.push((l.as_str(), 1)),
            }
        }
        let skip = lines.len().saturating_sub(LOG_LINES);
        let _ = writeln!(trace, "your governor's log lines (last {}, newest last):", run.log.len());
        for (text, n) in lines.into_iter().skip(skip) {
            let times = if n > 1 { format!("{n}x ") } else { String::new() };
            let _ = writeln!(trace, "  {times}{}", clip(text));
        }
    }

    Brief { signs, deaths: deaths_text, trace: trace.trim_end().to_owned() }
}

/// `text` on one line, cut at [`LINE_CHARS`].
fn clip(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= LINE_CHARS {
        one_line
    } else {
        let cut: String = one_line.chars().take(LINE_CHARS - 3).collect();
        format!("{cut}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trace_is_at_most_twelve_slices_wide_for_any_length() {
        assert_eq!(slices(540), (45, 12));
        assert_eq!(slices(24), (2, 12));
        assert_eq!(slices(5), (1, 5));
        assert_eq!(slices(1), (1, 1));
        assert_eq!(slices(13), (2, 7));
        for turns in 1..=2000 {
            let (w, n) = slices(turns);
            assert!(n <= SLICES && w * n >= turns && w * (n - 1) < turns, "{turns}: {w} x {n}");
        }
    }

    #[test]
    fn clip_keeps_one_line_of_bounded_length() {
        assert_eq!(clip("a\n  b"), "a b");
        let long = "x".repeat(300);
        assert_eq!(clip(&long).chars().count(), LINE_CHARS);
        assert!(clip(&long).ends_with("..."));
    }
}

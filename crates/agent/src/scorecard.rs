//! `fn_agent score LOG` (#525, docs/design/agent-hillclimb.md, "The scorecard" and "Where it's
//! computed"): a scorecard from a run log alone, so a result is something a command can verify,
//! not the agent's own prose.
//!
//! The log is replayed up to the horizon the way `Host::resume` does (and no further, even if
//! the log goes on), then `fn_sim::score::Scorecard::compute` scores the world it reached --
//! both through `fn_tui::surface::referee`, the privileged read the parity lint keeps out of
//! every MCP tool handler. This file is the one place in `crates/agent/src/` allowed to name it
//! (`xtask/src/lint.rs`, `REFEREE_USERS`), and the only thing in `tools.rs` that calls in here is
//! [`bundled_score_rule`] (#531), which reads `data/score.toml` and no world.
//!
//! What the referee measures is in [`Report::replay`]. What only the log's own lines can say --
//! script churn: names, one-shot names, turns and acts per script, the governor's share, plan
//! edits -- is counted here by [`Churn::of`]. `par` is left empty until the balance harness
//! supplies it (#526).

use std::fmt::Write as _;

use fn_tui::scenario;
pub use fn_tui::surface::referee::TurnSample;
use fn_tui::surface::referee::score::{MilestoneKind, ScoreDef};
use fn_tui::surface::referee::{self, MILD_WORDS, Replay};
use fn_tui::surface::{LogLine, RunLogHeader};
use serde_json::{Value, json};

/// The saved script that, by convention, owns a whole settlement
/// (docs/design/agent-hillclimb.md, "The governor is how, held as code").
pub const GOVERNOR: &str = "governor";

/// How many scripts the text output lists before folding the rest into "and N more" (`--json`
/// lists them all).
const SCRIPT_ROWS: usize = 8;

/// One scorecard: the referee's replay of a log, the log's own churn, and the header it was made
/// from.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub header: RunLogHeader,
    pub replay: Replay,
    pub churn: Churn,
    /// The score of the Economy opening on the same start: not computed yet (#526).
    pub par: Option<f64>,
}

/// Replays `lines` (a log made against `header`) to `horizon` turns (`None`: the log's own last
/// turn) and builds its scorecard under `data/score.toml`'s bundled weights.
pub fn build(
    header: &RunLogHeader,
    lines: &[LogLine],
    horizon: Option<u32>,
) -> Result<Report, String> {
    let replay = referee::score(header, lines, horizon, &ScoreDef::bundled())?;
    let churn = Churn::of(&lines[..replay.lines_used]);
    Ok(Report { header: header.clone(), replay, churn, par: None })
}

/// What one script name did (`run_script {name}` or an inline `run_script {code, note}`, whose
/// tag is its name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptUse {
    pub name: String,
    /// `Script` lines that ran (`turns > 0`).
    pub runs: u32,
    /// `Turn` lines played while one of its runs was open (between its `Script` and `Wake`).
    pub turns: u32,
    /// `Keys` lines in the same span.
    pub acts: u32,
}

/// How the run was played, counted from the log's lines: the habits the climb can act on.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Churn {
    /// Every script name that ran, in order of first run.
    pub scripts: Vec<ScriptUse>,
    /// `save_script` lines: a version written, never run by that line.
    pub saved: u32,
    /// Inline runs (`version` 0: `run_script {code}`, never saved).
    pub inline_runs: u32,
    /// `plan` lines: every change to the plan.
    pub plan_edits: u32,
    /// The governor ran as a clean evaluation (`fn_agent play --horizon`): one unsaved script
    /// (`version` 0) named `governor`, played alone from turn 0. That is how the horizon is
    /// scored, not a habit, so it is neither an inline run nor a one-shot script.
    pub clean_governor: bool,
    /// All `Turn` lines, in or out of a script.
    pub total_turns: u32,
    /// All `Keys` lines, in or out of a script.
    pub total_acts: u32,
}

impl Churn {
    /// Counts `lines` (already cut at the horizon).
    pub fn of(lines: &[LogLine]) -> Self {
        let mut churn = Self::default();
        let mut open: Option<usize> = None;
        for line in lines {
            match line {
                LogLine::Script { name, version, turns, .. } => {
                    if *turns == 0 {
                        churn.saved += 1;
                        continue;
                    }
                    if *version == 0 && name == GOVERNOR {
                        churn.clean_governor = true;
                    } else if *version == 0 {
                        churn.inline_runs += 1;
                    }
                    let at = match churn.scripts.iter().position(|s| s.name == *name) {
                        Some(at) => at,
                        None => {
                            churn.scripts.push(ScriptUse {
                                name: name.clone(),
                                runs: 0,
                                turns: 0,
                                acts: 0,
                            });
                            churn.scripts.len() - 1
                        }
                    };
                    churn.scripts[at].runs += 1;
                    open = Some(at);
                }
                LogLine::Wake { .. } => open = None,
                LogLine::Turn { .. } => {
                    churn.total_turns += 1;
                    if let Some(at) = open {
                        churn.scripts[at].turns += 1;
                    }
                }
                LogLine::Keys { .. } => {
                    churn.total_acts += 1;
                    if let Some(at) = open {
                        churn.scripts[at].acts += 1;
                    }
                }
                LogLine::Plan { .. } => churn.plan_edits += 1,
                _ => {}
            }
        }
        churn
    }

    /// Script names that ran exactly once (a clean evaluation's governor is not one: it is the
    /// whole run's loop).
    pub fn one_shot(&self) -> impl Iterator<Item = &ScriptUse> {
        self.scripts.iter().filter(|s| s.runs == 1 && !(self.clean_governor && s.name == GOVERNOR))
    }

    /// The governor's turns, if a script of that name ran.
    pub fn governor_turns(&self) -> Option<u32> {
        self.scripts.iter().find(|s| s.name == GOVERNOR).map(|s| s.turns)
    }
}

impl Report {
    /// The score as `--append` writes it: the scorecard's top block as a `score` log line.
    pub fn score_line(&self) -> LogLine {
        let card = &self.replay.card;
        LogLine::Score {
            score: card.score,
            progress: card.progress,
            care: card.care.value,
            horizon: self.replay.horizon_turns,
            attempt: None,
            champion: None,
            build: Some(fn_tui::buildid::CURRENT.to_owned()),
        }
    }

    /// The signs in one line, for `test_governor` (#570): "Stone in stores t135; Food in stores
    /// 40/150; Ore mined never started". A sign met says the turn, a slipped one also where it
    /// stands, an unmet one how far it got. The Ready and stage rows are named only once met.
    pub fn signs_line(&self) -> String {
        let r = &self.replay;
        r.card
            .milestones
            .iter()
            .filter(|m| m.kind == MilestoneKind::Sign || m.first_met.is_some())
            .map(|m| {
                let met = m.first_met.map(|t| format!("t{}", r.turn_of(t)));
                let at = format!("{}/{}", m.value, m.target);
                match (m.kind == MilestoneKind::Sign, met) {
                    (true, Some(t)) if m.slipped => format!("{} {t}, slipped to {at}", m.name),
                    (true, Some(t)) => format!("{} {t}", m.name),
                    (true, None) if m.value == 0 => format!("{} never started", m.name),
                    (true, None) => format!("{} {at}", m.name),
                    (false, met) => format!("{} {}", m.name, met.unwrap_or_default()),
                }
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn clock(&self, turns: u32) -> String {
        let secs = u64::from(turns) * u64::from(self.replay.turn_seconds);
        format!("{}:{:02}", secs / 60, secs % 60)
    }

    /// The header's own description of the start.
    fn start_text(&self) -> String {
        let h = &self.header;
        let nation = h.nation.as_str();
        let mut nation_name = nation.to_owned();
        nation_name[..1].make_ascii_uppercase();
        let mut parts = vec![nation_name, format!("seed {}", h.seed)];
        if let Some(id) = scenario::id_for(h) {
            parts.push(format!("scenario {id}"));
        }
        parts.push(h.difficulty.clone().unwrap_or_else(|| "default difficulty".to_owned()));
        parts.push(h.map_size.clone().unwrap_or_else(|| "default size".to_owned()));
        parts.push(match h.site {
            Some((x, y)) => format!("start {x},{y}"),
            None => "default site".to_owned(),
        });
        parts.join(" \u{b7} ")
    }

    /// The scorecard as text: the doc's sections, header first.
    pub fn text(&self) -> String {
        let r = &self.replay;
        let card = &r.card;
        let mut out = String::new();

        let _ = writeln!(
            out,
            "{}    horizon {} turns ({})",
            self.start_text(),
            r.horizon_turns,
            self.clock(r.horizon_turns)
        );
        let mut played =
            format!("Played {} turns ({})", r.played_turns, self.clock(r.played_turns));
        if r.played_turns < r.horizon_turns {
            played.push_str("; stopped early");
            if let Some(reason) = &r.end_reason {
                let _ = write!(played, " ({reason})");
            }
        } else if let Some(reason) = &r.end_reason {
            let _ = write!(played, "; log ended: {reason}");
        }
        let _ = writeln!(out, "{played}\n");

        let par = self.par.map_or(String::new(), |p| format!("        par {p:.0}"));
        let _ = writeln!(
            out,
            "SCORE {:.0}        progress {:.3} \u{d7} care {:.2}{par}\n",
            card.score, card.progress, card.care.value
        );

        // PROGRESS: best credit first, ties in the endeavor's order.
        let mut rows: Vec<_> = card.milestones.iter().collect();
        rows.sort_by(|a, b| b.credit.total_cmp(&a.credit));
        let width = rows.iter().map(|m| m.name.chars().count()).max().unwrap_or(0).max(8);
        let _ = writeln!(
            out,
            "{:<w$}  {:>5}  {:>6}  {:>9}",
            "PROGRESS",
            "met",
            "credit",
            "at horizon",
            w = width + 4
        );
        for m in rows {
            let mark = if m.first_met.is_some() && m.met_now { '+' } else { ' ' };
            let met = m.first_met.map_or(String::new(), |t| format!("t{}", r.turn_of(t)));
            let at = if m.kind == MilestoneKind::Sign {
                format!("{}/{}", m.value, m.target)
            } else {
                String::new()
            };
            let short = m.target.saturating_sub(m.value);
            let note = match (m.kind, m.slipped) {
                (MilestoneKind::Sign, true) => format!("slipped: {short} short"),
                (MilestoneKind::Sign, false) if m.first_met.is_none() && m.value == 0 => {
                    "never started".to_owned()
                }
                (MilestoneKind::Sign, false) if !m.met_now && short == 1 => "one short".to_owned(),
                (MilestoneKind::Sign, false) if !m.met_now => format!("{short} short"),
                _ => String::new(),
            };
            let line = format!(
                "  {mark} {:<width$}  {met:>5}  {:>6.2}  {at:>9}  {note}",
                m.name, m.credit
            );
            let _ = writeln!(out, "{}", line.trim_end());
        }
        let _ = writeln!(
            out,
            "    {:<width$}  {:>5}  {:>6.2} of {}\n",
            "",
            "",
            card.credit_total,
            fmt_weight(card.weight_total)
        );

        // CARE.
        let c = &card.care;
        let _ = writeln!(
            out,
            "CARE {:.2}         deaths {} \u{b7} close calls {} \u{b7} suffering {:.1}% of person-time",
            c.value,
            c.deaths,
            c.close_calls,
            c.suffering * 100.0
        );
        let of_person = |minutes: f64| {
            if c.person_minutes > 0.0 { minutes / c.person_minutes * 100.0 } else { 0.0 }
        };
        let severe: Vec<String> = c
            .severe_minutes
            .iter()
            .map(|(flag, minutes)| format!("{} {:.1}%", flag.name(), of_person(*minutes)))
            .collect();
        if !severe.is_empty() {
            let _ = writeln!(out, "  severe:         {}", severe.join(" \u{b7} "));
        }
        let rep = &r.reported;
        let mut mild: Vec<String> = MILD_WORDS
            .iter()
            .enumerate()
            .map(|(i, word)| format!("{word} {:.1}%", rep.mild_share(i) * 100.0))
            .collect();
        mild.push(format!("harm events {}", rep.harm_events));
        let _ = writeln!(out, "  reported only:  {}", mild.join(" \u{b7} "));
        let food = if rep.winter_food.is_empty() {
            "no winter reached".to_owned()
        } else {
            rep.winter_food
                .iter()
                .map(|w| format!("{:.1}", w.per_head()))
                .collect::<Vec<_>>()
                .join(" \u{b7} ")
        };
        let _ = writeln!(out, "                  food per head entering winter: {food}\n");

        // DIAGNOSTICS.
        let _ = writeln!(out, "DIAGNOSTICS");
        let idle: Vec<String> =
            rep.idle_windows.iter().map(|w| format!("{:.0}", w.share() * 100.0)).collect();
        let _ = writeln!(
            out,
            "  idle share by {} min  {}{}",
            referee::IDLE_WINDOW_MINUTES,
            if idle.is_empty() { "-".to_owned() } else { idle.join(" ") },
            if idle.is_empty() { "" } else { " %" }
        );
        match rep.research_idle {
            Some(idle) => {
                let why = if idle.tree_done { " (tree done)" } else { " (nothing queued)" };
                let _ = writeln!(out, "  research idle from  t{}{why}", idle.from_turn);
            }
            None => {
                let _ = writeln!(out, "  research idle from  never");
            }
        }
        out.push_str(&self.churn_text());
        out
    }

    fn churn_text(&self) -> String {
        let ch = &self.churn;
        let mut out = String::new();
        let one_shot: Vec<&ScriptUse> = ch.one_shot().collect();
        let (ot, oa) = (
            one_shot.iter().map(|s| s.turns).sum::<u32>(),
            one_shot.iter().map(|s| s.acts).sum::<u32>(),
        );
        let _ = writeln!(
            out,
            "  scripts  {} names, {} one-shot \u{b7} one-shot scripts ran {ot} of {} turns, {oa} of {} acts",
            ch.scripts.len(),
            one_shot.len(),
            ch.total_turns,
            ch.total_acts
        );
        let governor = match ch.governor_turns() {
            Some(t) => format!(
                "governor ran {t} of {} turns ({:.0}%)",
                ch.total_turns,
                share(t, ch.total_turns) * 100.0
            ),
            None => "no governor".to_owned(),
        };
        let _ = writeln!(
            out,
            "           saved {} \u{b7} inline runs {} \u{b7} {governor} \u{b7} plan edits {}",
            ch.saved, ch.inline_runs, ch.plan_edits
        );
        let mut by_turns: Vec<&ScriptUse> = ch.scripts.iter().collect();
        by_turns.sort_by_key(|s| std::cmp::Reverse(s.turns));
        let width = by_turns.iter().take(SCRIPT_ROWS).map(|s| s.name.chars().count()).max();
        for s in by_turns.iter().take(SCRIPT_ROWS) {
            let _ = writeln!(
                out,
                "           {:<w$}  {} runs \u{b7} {} turns \u{b7} {} acts",
                s.name,
                s.runs,
                s.turns,
                s.acts,
                w = width.unwrap_or(0)
            );
        }
        if by_turns.len() > SCRIPT_ROWS {
            let _ = writeln!(out, "           ... and {} more", by_turns.len() - SCRIPT_ROWS);
        }
        out
    }

    /// The same fields as [`Self::text`], for the ledger and the balance harness (#489).
    pub fn json(&self) -> Value {
        let r = &self.replay;
        let card = &r.card;
        let h = &self.header;
        let milestones: Vec<Value> = card
            .milestones
            .iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "kind": match m.kind {
                        MilestoneKind::Sign => "sign",
                        MilestoneKind::Ready => "ready",
                        MilestoneKind::Stage => "stage",
                    },
                    "name": m.name,
                    "met_turn": m.first_met.map(|t| r.turn_of(t)),
                    "value": m.value,
                    "target": m.target,
                    "met_now": m.met_now,
                    "slipped": m.slipped,
                    "weight": m.weight,
                    "credit": m.credit,
                })
            })
            .collect();
        let c = &card.care;
        let rep = &r.reported;
        let severe: serde_json::Map<String, Value> = c
            .severe_minutes
            .iter()
            .map(|(flag, minutes)| (flag.name().to_owned(), json!(minutes)))
            .collect();
        let mild: serde_json::Map<String, Value> = MILD_WORDS
            .iter()
            .enumerate()
            .map(|(i, word)| ((*word).to_owned(), json!(rep.mild_share(i))))
            .collect();
        let winter_food: Vec<Value> = rep
            .winter_food
            .iter()
            .map(|w| {
                json!({
                    "year": w.year,
                    "turn": w.turn,
                    "food": w.food,
                    "people": w.people,
                    "per_head": w.per_head(),
                })
            })
            .collect();
        let idle: Vec<Value> = rep
            .idle_windows
            .iter()
            .enumerate()
            .map(|(i, w)| {
                json!({
                    "start_minute": i as u64 * referee::IDLE_WINDOW_MINUTES,
                    "idle": w.idle,
                    "total": w.total,
                    "share": w.share(),
                })
            })
            .collect();
        let ch = &self.churn;
        let scripts: Vec<Value> = ch
            .scripts
            .iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "runs": s.runs,
                    "turns": s.turns,
                    "acts": s.acts,
                    "one_shot": s.runs == 1,
                })
            })
            .collect();
        let one_shot: Vec<&ScriptUse> = ch.one_shot().collect();
        json!({
            "score": card.score,
            "progress": card.progress,
            "care": c.value,
            "par": self.par,
            "build": fn_tui::buildid::CURRENT,
            "log_build": h.build,
            "horizon": r.horizon_turns,
            "played": r.played_turns,
            "log_turns": r.log_turns,
            "turn_seconds": r.turn_seconds,
            "stopped_early": r.played_turns < r.horizon_turns,
            "end_reason": r.end_reason,
            "start": {
                "seed": h.seed,
                "nation": h.nation.as_str(),
                "difficulty": h.difficulty,
                "map_size": h.map_size,
                "site": h.site,
                "scenario": scenario::id_for(h),
            },
            "progress_detail": {
                "credit_total": card.credit_total,
                "weight_total": card.weight_total,
                "milestones": milestones,
            },
            "care_detail": {
                "deaths": c.deaths,
                "close_calls": c.close_calls,
                "suffering": c.suffering,
                "person_minutes": c.person_minutes,
                "severe_any_minutes": c.severe_any_minutes,
                "severe_minutes": severe,
                "reported": {
                    "mild": mild,
                    "settler_samples": rep.settler_samples,
                    "harm_events": rep.harm_events,
                    "winter_food": winter_food,
                },
            },
            "diagnostics": {
                "idle_share": idle,
                "research_idle": rep.research_idle.map(|i| json!({
                    "from_turn": i.from_turn,
                    "tree_done": i.tree_done,
                })),
                "scripts": {
                    "names": ch.scripts.len(),
                    "one_shot_names": one_shot.len(),
                    "one_shot_turns": one_shot.iter().map(|s| s.turns).sum::<u32>(),
                    "one_shot_acts": one_shot.iter().map(|s| s.acts).sum::<u32>(),
                    "total_turns": ch.total_turns,
                    "total_acts": ch.total_acts,
                    "saved": ch.saved,
                    "inline_runs": ch.inline_runs,
                    "governor_turns": ch.governor_turns(),
                    "governor_share": ch.governor_turns().map(|t| share(t, ch.total_turns)),
                    "plan_edits": ch.plan_edits,
                    "scripts": scripts,
                },
            },
        })
    }
}

fn share(part: u32, whole: u32) -> f64 {
    if whole == 0 { 0.0 } else { f64::from(part) / f64::from(whole) }
}

/// A weight total without a trailing ".0" when it's whole ("14", not "14.0").
fn fmt_weight(w: f64) -> String {
    if (w - w.round()).abs() < 1e-9 { format!("{w:.0}") } else { format!("{w:.2}") }
}

/// The paragraph `briefing` gives a `--scripts` run about what it is scored on (#531,
/// docs/design/agent-hillclimb.md, "Parity: when the agent sees it"): the rule and its weights,
/// never a number about this run. Generated from `def` so the text can't drift from
/// `data/score.toml`; it reads that data and nothing of any world.
pub fn score_rule(def: &ScoreDef) -> String {
    let (p, c) = (&def.progress, &def.care);
    let mut out = format!(
        "How this run is scored: out of 1000, as progress times care. Progress is the weighted \
         share of the Endeavor's milestones you reach: every sign, then Ready, then every stage \
         of the build. Each milestone weighs {}",
        rule_num(p.default_weight)
    );
    if p.weights.is_empty() {
        out.push('.');
    } else {
        let notes: Vec<String> = p
            .weights
            .iter()
            .map(|(id, w)| {
                if *w == 0.0 {
                    format!("{id} doesn't count")
                } else {
                    format!("{id} weighs {}", rule_num(*w))
                }
            })
            .collect();
        let _ = write!(out, " unless noted ({}).", notes.join("; "));
    }
    let _ = write!(
        out,
        " A milestone first reached at the very start earns full credit and one first reached at \
         the end of the run earns {} of it, falling in a straight line between, so sooner is \
         better. A sign not met at the end (never met, or met and since lost) earns {} of its \
         progress toward its target. Care multiplies progress. It starts at 1 and loses {} per \
         death, {} per close call (a settler's health dipping dangerously low and recovering, or \
         the Avatar leaving), and {} times the share of settler-time that anyone spends {}. So a \
         death or lasting suffering costs score however far the settlement gets.",
        rule_pct(p.late_credit),
        rule_pct(p.partial_credit),
        rule_num(c.death),
        rule_num(c.close_call),
        rule_num(c.suffering),
        rule_list(c.severe.iter().map(|f| f.name().to_lowercase())),
    );
    out
}

/// [`score_rule`] of the bundled `data/score.toml`.
pub fn bundled_score_rule() -> String {
    score_rule(&ScoreDef::bundled())
}

/// A weight or rate as short as it can be: "1", "0.15", "2".
fn rule_num(v: f64) -> String {
    format!("{v}")
}

/// A fraction as a percentage: 0.6 is "60%".
fn rule_pct(v: f64) -> String {
    format!("{}%", rule_num((v * 1000.0).round() / 10.0))
}

/// "a", "a or b", "a, b or c".
fn rule_list(items: impl Iterator<Item = String>) -> String {
    let items: Vec<String> = items.collect();
    match items.split_last() {
        None => "in trouble".to_owned(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_rule_states_the_bundled_weights() {
        let text = bundled_score_rule();
        for want in [
            "out of 1000",
            "progress times care",
            "60%",
            "50%",
            "0.15 per death",
            "0.03 per close call",
            "2 times the share",
            "starving, parched, exhausted or dying",
        ] {
            assert!(text.contains(want), "missing {want:?} in: {text}");
        }
    }

    /// Changing a weight in `score.toml` changes the paragraph, so the text can't drift.
    #[test]
    fn score_rule_follows_score_toml() {
        let src = include_str!("../../../data/score.toml");
        let base = score_rule(&ScoreDef::from_toml(src).unwrap());
        assert_eq!(base, bundled_score_rule());
        let severe = "severe = [\"starving\", \"parched\", \"exhausted\", \"dying\"]";
        for (from, to, want) in [
            ("late_credit = 0.6", "late_credit = 0.25", "25%"),
            ("partial_credit = 0.5", "partial_credit = 0.4", "40%"),
            ("death = 0.15", "death = 0.2", "0.2 per death"),
            ("close_call = 0.03", "close_call = 0.07", "0.07 per close call"),
            ("suffering = 2.0", "suffering = 3.5", "3.5 times the share"),
            ("default_weight = 1.0", "default_weight = 2.0", "weighs 2"),
            (severe, "severe = [\"dying\"]", "spends dying."),
        ] {
            assert!(src.contains(from), "score.toml no longer says {from:?}");
            let changed = score_rule(&ScoreDef::from_toml(&src.replace(from, to)).unwrap());
            assert_ne!(changed, base, "{from} -> {to}");
            assert!(changed.contains(want), "{want:?} missing from: {changed}");
        }
    }

    #[test]
    fn score_rule_lists_weight_overrides() {
        let src = include_str!("../../../data/score.toml").replace(
            "\n[care]\n",
            "\n[[progress.weight]]\nid = \"scrolls\"\nweight = 0.5\n\n\
             [[progress.weight]]\nid = \"ready\"\nweight = 0\n\n[care]\n",
        );
        let text = score_rule(&ScoreDef::from_toml(&src).unwrap());
        assert!(text.contains("unless noted (scrolls weighs 0.5; ready doesn't count)"), "{text}");
    }

    fn script(name: &str, version: u32, turns: u32) -> LogLine {
        LogLine::Script { name: name.into(), version, code: String::new(), turns, wake_on: vec![] }
    }

    fn wake() -> LogLine {
        LogLine::Wake { reason: "done".into(), why: None, turn: 0, memory: serde_json::json!({}) }
    }

    fn turn() -> LogLine {
        LogLine::Turn { index: 0, tick: 0 }
    }

    fn keys() -> LogLine {
        LogLine::Keys { keys: "x".into(), label: None, note: None }
    }

    #[test]
    fn churn_counts_names_one_shots_and_the_governor() {
        let lines = vec![
            script("governor", 1, 0), // a save
            script("governor", 1, 5),
            turn(),
            keys(),
            turn(),
            wake(),
            script("findwater", 0, 3),
            keys(),
            turn(),
            wake(),
            script("governor", 1, 5),
            turn(),
            wake(),
            keys(), // a direct act, outside any script
            turn(),
            LogLine::Plan { plan: serde_json::json!({}) },
        ];
        let c = Churn::of(&lines);
        assert_eq!((c.saved, c.inline_runs, c.plan_edits), (1, 1, 1));
        assert_eq!((c.total_turns, c.total_acts), (5, 3));
        assert_eq!(c.scripts.len(), 2);
        assert_eq!(c.governor_turns(), Some(3));
        let one_shot: Vec<_> = c.one_shot().map(|s| s.name.as_str()).collect();
        assert_eq!(one_shot, ["findwater"]);
        assert_eq!(
            c.scripts[1],
            ScriptUse { name: "findwater".into(), runs: 1, turns: 1, acts: 1 }
        );
    }

    /// A clean evaluation is one unsaved `governor` run: it counts as the governor's turns, and
    /// is neither an inline run nor a one-shot script. An ad hoc `play` tag still is both.
    #[test]
    fn a_clean_evaluations_governor_is_not_inline_or_one_shot() {
        let clean = Churn::of(&[script("governor", 0, 4), turn(), keys(), turn(), wake()]);
        assert!(clean.clean_governor);
        assert_eq!((clean.inline_runs, clean.one_shot().count()), (0, 0));
        assert_eq!(clean.governor_turns(), Some(2));

        let adhoc = Churn::of(&[script("play", 0, 4), turn(), wake()]);
        assert!(!adhoc.clean_governor);
        assert_eq!((adhoc.inline_runs, adhoc.one_shot().count()), (1, 1));
        assert_eq!(adhoc.governor_turns(), None);
    }

    #[test]
    fn whole_weights_print_without_a_decimal() {
        assert_eq!(fmt_weight(14.0), "14");
        assert_eq!(fmt_weight(13.5), "13.50");
    }
}

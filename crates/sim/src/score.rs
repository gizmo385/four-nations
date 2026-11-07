//! The run score (issue #523, `docs/design/agent-hillclimb.md` "Part 1: the score", owner
//! decisions #519 and #520): how many of the Harmony Gate's milestones a run met, how early, and
//! how well the colony looked after its people.
//!
//! `score = SCORE_SCALE × progress × care`, both factors in 0..=1.
//!
//! - **Progress** is a weighted mean of one credit per milestone (every sign, then Ready, then
//!   every stage). A milestone first met at tick `t` of a `horizon` earns
//!   `1 - (1 - late_credit) × t / horizon`. A sign not holding at the horizon earns only
//!   `partial_credit × min(1, value / target)`, unless the endeavor latched Ready, which keeps
//!   every sign's first-met credit as the game does. Ready and the stages have no partial credit.
//! - **Care** is `max(0, 1 - death × deaths - close_call × close_calls - suffering × share)`. The
//!   counts come from `World::care()` (#533) and `World::deaths`; nothing here counts anything.
//!
//! Everything here is a pure function of a `World`, the `Defs`, a `ScoreDef` and the horizon: no
//! clock, no RNG, no sim state of its own. The score data (`data/score.toml`) is loaded on its
//! own, not into `Defs`, so `Defs::id_hash` doesn't change when a weight does and old run logs
//! rescore under new weights.
//!
//! **Compute at the horizon.** The care tally accrues every tick from the start of the game and
//! keeps no history, and `endeavor_signs` is a query of the world as it stands. So the score is
//! exact for a world at or before the horizon: a run that stopped early is scored on what it
//! reached, with no extrapolation. A world already past the horizon still has its events
//! (`endeavor_events`) and deaths cut off at the horizon, but its care minutes, close calls and
//! unmet-sign values describe a later moment: `Scorecard::past_horizon` says so. To score a run at
//! a horizon, replay it to the horizon first (what `fn_agent score` does).

use serde::Deserialize;

use crate::defs::Defs;
use crate::world::{EndeavorEventKind, Severe, World};

/// What a perfect run (all milestones, care 1.0) scores.
pub const SCORE_SCALE: f64 = 1000.0;

/// The id of the Ready milestone: every sign met at once.
pub const READY_ID: &str = "ready";

/// Why `data/score.toml` was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreError(pub String);

impl std::fmt::Display for ScoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid score data: {}", self.0)
    }
}

impl std::error::Error for ScoreError {}

/// `data/score.toml`, validated. Loaded on its own, never as part of `Defs`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreDef {
    pub progress: ProgressDef,
    pub care: CareDef,
}

/// `[progress]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProgressDef {
    /// A milestone met at the horizon earns this much of one met at turn 0.
    pub late_credit: f64,
    /// An unmet sign earns this × min(1, value / target).
    pub partial_credit: f64,
    /// The weight of every milestone not in `weights`.
    pub default_weight: f64,
    /// Per-milestone overrides, by id, in file order.
    pub weights: Vec<(String, f64)>,
}

/// `[care]`.
#[derive(Debug, Clone, PartialEq)]
pub struct CareDef {
    /// Care lost per death.
    pub death: f64,
    /// Care lost per close call.
    pub close_call: f64,
    /// Care lost per 100% of person-time with any severe flag.
    pub suffering: f64,
    /// The flags the scorecard reports a breakdown of (labels only).
    pub severe: Vec<Severe>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoreFile {
    progress: ProgressFile,
    care: CareFile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgressFile {
    late_credit: f64,
    partial_credit: f64,
    default_weight: f64,
    #[serde(default)]
    weight: Vec<WeightFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WeightFile {
    id: String,
    weight: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CareFile {
    death: f64,
    close_call: f64,
    suffering: f64,
    severe: Vec<String>,
}

impl ScoreDef {
    /// The score data shipped in `data/score.toml`, embedded at compile time.
    pub fn bundled() -> Self {
        Self::from_toml(include_str!("../../../data/score.toml"))
            .expect("the bundled data/score.toml is valid")
    }

    /// Parses and validates `src`: strict keys, numbers in range, known severe labels. Weight ids
    /// are checked against the endeavor by `check`, which needs `Defs`.
    pub fn from_toml(src: &str) -> Result<Self, ScoreError> {
        let file: ScoreFile =
            toml::from_str(src).map_err(|e| ScoreError(format!("score.toml: {e}")))?;
        let (p, c) = (file.progress, file.care);
        let unit = |name: &str, v: f64| {
            if v.is_finite() && (0.0..=1.0).contains(&v) {
                Ok(v)
            } else {
                Err(ScoreError(format!("[progress] {name} = {v} must be in 0..=1")))
            }
        };
        let non_negative = |name: &str, v: f64| {
            if v.is_finite() && v >= 0.0 {
                Ok(v)
            } else {
                Err(ScoreError(format!("[care] {name} = {v} must be 0 or more")))
            }
        };
        if !(p.default_weight.is_finite() && p.default_weight > 0.0) {
            return Err(ScoreError(format!(
                "[progress] default_weight = {} must be above 0",
                p.default_weight
            )));
        }
        let mut weights: Vec<(String, f64)> = Vec::new();
        for w in p.weight {
            if !(w.weight.is_finite() && w.weight >= 0.0) {
                return Err(ScoreError(format!(
                    "[[progress.weight]] {}: weight = {} must be 0 or more",
                    w.id, w.weight
                )));
            }
            if weights.iter().any(|(id, _)| *id == w.id) {
                return Err(ScoreError(format!("[[progress.weight]] {} is listed twice", w.id)));
            }
            weights.push((w.id, w.weight));
        }
        let mut severe: Vec<Severe> = Vec::new();
        for label in &c.severe {
            let flag = Severe::ALL
                .into_iter()
                .find(|s| s.name().eq_ignore_ascii_case(label))
                .ok_or_else(|| ScoreError(format!("[care] severe: unknown flag {label:?}")))?;
            if severe.contains(&flag) {
                return Err(ScoreError(format!("[care] severe lists {label:?} twice")));
            }
            severe.push(flag);
        }
        Ok(Self {
            progress: ProgressDef {
                late_credit: unit("late_credit", p.late_credit)?,
                partial_credit: unit("partial_credit", p.partial_credit)?,
                default_weight: p.default_weight,
                weights,
            },
            care: CareDef {
                death: non_negative("death", c.death)?,
                close_call: non_negative("close_call", c.close_call)?,
                suffering: non_negative("suffering", c.suffering)?,
                severe,
            },
        })
    }

    /// Checks the weights against `defs`' endeavor: every id must name a sign, `ready` or a stage,
    /// and at least one milestone must carry weight. Run it once after loading; `compute` ignores
    /// an id it doesn't know.
    pub fn check(&self, defs: &Defs) -> Result<(), ScoreError> {
        let ids = milestone_ids(defs);
        for (id, _) in &self.progress.weights {
            if !ids.iter().any(|m| m == id) {
                return Err(ScoreError(format!(
                    "[[progress.weight]] {id:?} is not a sign, \"ready\" or a stage"
                )));
            }
        }
        if ids.iter().map(|id| self.progress.weight_of(id)).sum::<f64>() <= 0.0 {
            return Err(ScoreError("every milestone has weight 0".to_owned()));
        }
        Ok(())
    }
}

impl ProgressDef {
    /// The weight of milestone `id`.
    pub fn weight_of(&self, id: &str) -> f64 {
        self.weights.iter().find(|(w, _)| w == id).map_or(self.default_weight, |&(_, w)| w)
    }

    /// The credit for a milestone first met at tick `t` of `horizon`: 1 at turn 0, `late_credit`
    /// at the horizon.
    pub fn met_credit(&self, t: u64, horizon: u64) -> f64 {
        let fraction = if horizon == 0 { 0.0 } else { (t as f64 / horizon as f64).min(1.0) };
        1.0 - (1.0 - self.late_credit) * fraction
    }

    /// The credit for a sign that isn't holding: a share of `partial_credit`, capped at the
    /// target (overshoot earns nothing extra).
    pub fn partial(&self, value: u32, target: u32) -> f64 {
        if target == 0 {
            return 0.0;
        }
        self.partial_credit * (f64::from(value) / f64::from(target)).min(1.0)
    }
}

/// What kind of milestone a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MilestoneKind {
    Sign,
    Ready,
    Stage,
}

/// One milestone's row on the scorecard.
#[derive(Debug, Clone, PartialEq)]
pub struct MilestoneRow {
    pub id: String,
    pub kind: MilestoneKind,
    /// The sign's or stage's name; "Ready" for Ready.
    pub name: String,
    /// The tick it was first met, within the horizon; `None` if it wasn't.
    pub first_met: Option<u64>,
    /// Its value at the world's current tick (see the module doc): the sign's count, or 1 or 0
    /// for Ready and stages.
    pub value: u32,
    /// What `value` is measured against (1 for Ready and stages).
    pub target: u32,
    /// Whether it holds now.
    pub met_now: bool,
    /// Met once within the horizon, no longer holding, and not saved by Ready's latch: it earns
    /// partial credit only.
    pub slipped: bool,
    pub weight: f64,
    /// Credit in 0..=1, before weighting.
    pub credit: f64,
}

/// The care half of the score.
#[derive(Debug, Clone, PartialEq)]
pub struct CareScore {
    /// `max(0, 1 - ...)`, in 0..=1.
    pub value: f64,
    /// `World::deaths` within the horizon.
    pub deaths: u32,
    /// `CareTally::close_calls`.
    pub close_calls: u32,
    /// Person-minutes with any severe flag ÷ all person-minutes (0 with no person-time).
    pub suffering: f64,
    /// All settler-minutes lived.
    pub person_minutes: f64,
    /// Person-minutes with any severe flag (each settler-tick once).
    pub severe_any_minutes: f64,
    /// The per-flag minutes for the flags `[care] severe` lists, a breakdown (flags overlap, so
    /// these don't sum to `severe_any_minutes`).
    pub severe_minutes: Vec<(Severe, f64)>,
}

/// A run's score, with everything it was computed from.
#[derive(Debug, Clone, PartialEq)]
pub struct Scorecard {
    pub horizon_ticks: u64,
    /// The world's tick when it was scored.
    pub world_tick: u64,
    /// The world had already run past the horizon, so the care tally and the unmet-sign values
    /// describe a later moment than the horizon (module doc).
    pub past_horizon: bool,
    /// `SCORE_SCALE × progress × care.value`.
    pub score: f64,
    /// Weighted credits ÷ weights, in 0..=1.
    pub progress: f64,
    /// Weighted credits summed ("3.58" of 14).
    pub credit_total: f64,
    /// Weights summed (14 by default).
    pub weight_total: f64,
    pub care: CareScore,
    /// One row per milestone, in `data/endeavor.toml` order: signs, Ready, stages.
    pub milestones: Vec<MilestoneRow>,
}

/// Every milestone id in scoring order: each sign, `ready`, each stage.
pub fn milestone_ids(defs: &Defs) -> Vec<String> {
    let signs = defs.endeavor.signs.iter().map(|s| s.id.clone());
    let stages = defs.endeavor.stages.iter().map(|s| s.id.clone());
    signs.chain([READY_ID.to_owned()]).chain(stages).collect()
}

/// What `Scorecard::build` reads, gathered from the world by `compute`.
struct Inputs {
    horizon: u64,
    world_tick: u64,
    milestones: Vec<Milestone>,
    ready_latched: bool,
    deaths: u32,
    close_calls: u32,
    person_minutes: f64,
    severe_any_minutes: f64,
    severe_minutes: Vec<(Severe, f64)>,
}

struct Milestone {
    id: String,
    kind: MilestoneKind,
    name: String,
    first_met: Option<u64>,
    value: u32,
    target: u32,
    met_now: bool,
}

impl Scorecard {
    /// Scores `world` at `horizon_ticks` (ticks from the start; `TICKS_PER_SECOND` to a second).
    /// See the module doc: the world should be at or before the horizon.
    pub fn compute(world: &World, defs: &Defs, score: &ScoreDef, horizon_ticks: u64) -> Self {
        let within = |tick: u64| tick <= horizon_ticks;
        let first = |want: &dyn Fn(&EndeavorEventKind) -> bool| {
            world
                .endeavor_events()
                .iter()
                .filter(|e| within(e.tick) && want(&e.kind))
                .map(|e| e.tick)
                .min()
        };
        let ready = first(&|k| matches!(k, EndeavorEventKind::Ready));

        let mut milestones = Vec::new();
        for (sign, p) in defs.endeavor.signs.iter().zip(world.endeavor_signs(defs)) {
            let met = first(&|k| matches!(k, EndeavorEventKind::SignMet(id) if *id == sign.id));
            milestones.push(Milestone {
                id: sign.id.clone(),
                kind: MilestoneKind::Sign,
                name: sign.name.clone(),
                first_met: met,
                value: p.value,
                target: p.target,
                met_now: p.met,
            });
        }
        milestones.push(Milestone {
            id: READY_ID.to_owned(),
            kind: MilestoneKind::Ready,
            name: "Ready".to_owned(),
            first_met: ready,
            value: u32::from(ready.is_some()),
            target: 1,
            met_now: ready.is_some(),
        });
        for stage in &defs.endeavor.stages {
            let done = first(&|k| matches!(k, EndeavorEventKind::StageDone(id) if *id == stage.id));
            milestones.push(Milestone {
                id: stage.id.clone(),
                kind: MilestoneKind::Stage,
                name: stage.name.clone(),
                first_met: done,
                value: u32::from(done.is_some()),
                target: 1,
                met_now: done.is_some(),
            });
        }

        let care = world.care();
        let severe_minutes = score.care.severe.iter().map(|&s| (s, care.severe_minutes(s)));
        let deaths = world.deaths.iter().filter(|d| within(d.tick)).count();
        Self::build(
            score,
            Inputs {
                horizon: horizon_ticks,
                world_tick: world.tick,
                milestones,
                ready_latched: ready.is_some(),
                deaths: deaths as u32,
                close_calls: care.close_calls(),
                person_minutes: care.person_minutes(),
                severe_any_minutes: care.severe_any_minutes(),
                severe_minutes: severe_minutes.collect(),
            },
        )
    }

    fn build(score: &ScoreDef, inputs: Inputs) -> Self {
        let p = &score.progress;
        let rows: Vec<MilestoneRow> = inputs
            .milestones
            .into_iter()
            .map(|m| {
                let slipped = m.kind == MilestoneKind::Sign
                    && m.first_met.is_some()
                    && !m.met_now
                    && !inputs.ready_latched;
                let credit = match (m.kind, m.first_met) {
                    // Ready latched: every sign keeps its first-met credit.
                    (MilestoneKind::Sign, Some(t)) if m.met_now || inputs.ready_latched => {
                        p.met_credit(t, inputs.horizon)
                    }
                    (MilestoneKind::Sign, _) => p.partial(m.value, m.target),
                    (_, Some(t)) => p.met_credit(t, inputs.horizon),
                    (_, None) => 0.0,
                };
                MilestoneRow {
                    weight: p.weight_of(&m.id),
                    id: m.id,
                    kind: m.kind,
                    name: m.name,
                    first_met: m.first_met,
                    value: m.value,
                    target: m.target,
                    met_now: m.met_now,
                    slipped,
                    credit,
                }
            })
            .collect();
        let weight_total: f64 = rows.iter().map(|r| r.weight).sum();
        let credit_total: f64 = rows.iter().map(|r| r.weight * r.credit).sum();
        let progress = if weight_total > 0.0 { credit_total / weight_total } else { 0.0 };

        let suffering = if inputs.person_minutes > 0.0 {
            inputs.severe_any_minutes / inputs.person_minutes
        } else {
            0.0
        };
        let c = &score.care;
        let value = (1.0
            - c.death * f64::from(inputs.deaths)
            - c.close_call * f64::from(inputs.close_calls)
            - c.suffering * suffering)
            .max(0.0);
        Self {
            horizon_ticks: inputs.horizon,
            world_tick: inputs.world_tick,
            past_horizon: inputs.world_tick > inputs.horizon,
            score: SCORE_SCALE * progress * value,
            progress,
            credit_total,
            weight_total,
            care: CareScore {
                value,
                deaths: inputs.deaths,
                close_calls: inputs.close_calls,
                suffering,
                person_minutes: inputs.person_minutes,
                severe_any_minutes: inputs.severe_any_minutes,
                severe_minutes: inputs.severe_minutes,
            },
            milestones: rows,
        }
    }
}

#[cfg(test)]
mod tests;

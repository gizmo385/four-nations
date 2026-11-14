//! The run score (issue #523, `docs/design/agent-hillclimb.md` "Part 1: the score", owner
//! decisions #519 and #520): how many of the Harmony Gate's milestones a run met, how early, and
//! how well the colony looked after its people.
//!
//! `score = SCORE_SCALE × progress × care`, both factors in 0..=1.
//!
//! - **Progress** is a weighted mean of one credit per milestone (every sign, then Ready, then
//!   every stage). A milestone first met at tick `t` of a `horizon` earns
//!   `1 - (1 - late_credit) × (t - start) / (horizon - start)`, where `start` is turn 0 (see
//!   "Preludes" below); one met at or before `start` earns 1. A sign not holding at the horizon earns only
//!   `partial_credit × min(1, value / target)`, unless the endeavor latched Ready, which keeps
//!   every sign's first-met credit as the game does. Ready has no partial credit. A stage that
//!   isn't done earns `task_credit` × the mean of its tasks' credits, each a done task's met
//!   credit or 0 (#1009), so three pillars of four score between none and the finished stage.
//!   With `avatar_arrival` above 0, the Avatar's arrival is one more milestone of that weight.
//! - **Care** is `max(0, 1 - death × deaths - close_call × close_calls - suffering × share)`. The
//!   counts come from `World::care()` (#533) and `World::deaths`; nothing here counts anything.
//!
//! **Preludes.** A track that starts from a scenario's prelude (`data/scenarios.toml`, #481) has
//! its turn 0 at the end of the prelude, not at world tick 0 (docs/design/agent-hillclimb.md,
//! "Tracks"). `Scorecard::compute` takes a [`ScoreStart`] for that: milestones met by `start.tick`
//! (the prelude's, or turn 0's) earn full credit, the late-credit slope runs from `start.tick`
//! to the horizon, and care counts only what happened after it (the care tally at the start is
//! subtracted; deaths at or before it are left out). With no prelude, `ScoreStart::default()` is
//! tick 0 and an empty tally, and the score is what it was before the start existed.
//!
//! Everything here is a pure function of a `World`, the `Defs`, a `ScoreDef`, the start and the horizon: no
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
use crate::world::{CaravanEventKind, CareTally, EndeavorEventKind, Severe, World};

/// What a perfect run (all milestones, care 1.0) scores.
pub const SCORE_SCALE: f64 = 1000.0;

/// The id of the Ready milestone: every sign met at once.
pub const READY_ID: &str = "ready";

/// The id of the Avatar's arrival milestone (#1009): scored only when `[progress]
/// avatar_arrival` gives it a weight above 0.
pub const AVATAR_ID: &str = "avatar_arrived";

/// Why `data/score.toml` was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreError(pub String);

impl std::fmt::Display for ScoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid score data: {}", self.0)
    }
}

impl std::error::Error for ScoreError {}

/// The profile `data/score.toml`'s top-level `[progress]` and `[care]` make: the default, and
/// the one every track scores with unless it names another (#1008).
pub const DEFAULT_PROFILE: &str = "gate";

/// `data/score.toml`, validated, as one profile (#1008). Loaded on its own, never as part of
/// `Defs`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreDef {
    /// Which profile this is: [`DEFAULT_PROFILE`], or a `[profile.NAME]` of the file.
    pub profile: String,
    pub progress: ProgressDef,
    pub care: CareDef,
    /// `[flourish]` (#1010): `None` when the profile has none, or its cap is 0.
    pub flourish: Option<FlourishDef>,
}

/// `[flourish]` (#1010): what a run does after Ready, as a capped bonus on progress.
#[derive(Debug, Clone, PartialEq)]
pub struct FlourishDef {
    /// The most the bonus adds to progress, in 0..=1.
    pub cap: f64,
    /// Each measure with its target: the count after Ready that earns it in full. A measure with
    /// target 0 is left out.
    pub measures: Vec<(FlourishMeasure, u32)>,
}

/// What `[flourish]` counts, from the Ready tick to the horizon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlourishMeasure {
    /// Techs learned (`World::tech_completions`).
    Research,
    /// Caravan deals and requests completed (`World::caravan_events`).
    Trade,
    /// Newcomers who joined (`World::arrivals`).
    Newcomers,
}

impl FlourishMeasure {
    pub const ALL: [Self; 3] = [Self::Research, Self::Trade, Self::Newcomers];

    /// Its key in `[flourish]`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Trade => "trade",
            Self::Newcomers => "newcomers",
        }
    }
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
    /// A stage that isn't done earns this × the mean of its tasks' credits (each a done task's
    /// met credit, or 0), #1009. 0 turns it off.
    pub task_credit: f64,
    /// The weight of the Avatar's arrival as a milestone of its own, #1009. 0 leaves it out.
    pub avatar_arrival: f64,
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
    #[serde(default)]
    flourish: Option<FlourishFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FlourishFile {
    cap: f64,
    #[serde(default)]
    research: u32,
    #[serde(default)]
    trade: u32,
    #[serde(default)]
    newcomers: u32,
}

/// The file's raw tables, before a profile is picked: the base sections and the overrides.
fn split_profiles(src: &str) -> Result<(toml::Table, toml::Table), ScoreError> {
    let mut base: toml::Table =
        toml::from_str(src).map_err(|e| ScoreError(format!("score.toml: {e}")))?;
    let profiles = match base.remove("profile") {
        None => toml::Table::new(),
        Some(toml::Value::Table(t)) => t,
        Some(_) => return Err(ScoreError("score.toml: `profile` must be a table".to_owned())),
    };
    if profiles.contains_key(DEFAULT_PROFILE) {
        return Err(ScoreError(format!(
            "[profile.{DEFAULT_PROFILE}]: the top-level sections are that profile; don't name it"
        )));
    }
    Ok((base, profiles))
}

/// Lays `over` onto `base`: a table merges key by key, anything else (a number, a list) replaces.
fn merge(base: &mut toml::Table, over: toml::Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgressFile {
    late_credit: f64,
    partial_credit: f64,
    default_weight: f64,
    #[serde(default)]
    task_credit: f64,
    #[serde(default)]
    avatar_arrival: f64,
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

    /// The bundled file's profile `name` ([`DEFAULT_PROFILE`] or a `[profile.NAME]`).
    pub fn bundled_profile(name: &str) -> Result<Self, ScoreError> {
        Self::from_toml_profile(include_str!("../../../data/score.toml"), name)
    }

    /// The names of `src`'s profiles: [`DEFAULT_PROFILE`] first, then each `[profile.NAME]` in
    /// name order.
    pub fn profile_names(src: &str) -> Result<Vec<String>, ScoreError> {
        let (_, profiles) = split_profiles(src)?;
        Ok(std::iter::once(DEFAULT_PROFILE.to_owned()).chain(profiles.keys().cloned()).collect())
    }

    /// Parses and validates `src`'s default profile: strict keys, numbers in range, known severe
    /// labels. Weight ids are checked against the endeavor by `check`, which needs `Defs`.
    pub fn from_toml(src: &str) -> Result<Self, ScoreError> {
        Self::from_toml_profile(src, DEFAULT_PROFILE)
    }

    /// As [`ScoreDef::from_toml`], for profile `name`. A `[profile.NAME]` table is laid over the
    /// top-level sections: its tables merge key by key, and a key it gives (a number, or the whole
    /// `weight` list) replaces the base's. So a profile names only what it changes.
    pub fn from_toml_profile(src: &str, name: &str) -> Result<Self, ScoreError> {
        let (mut table, mut profiles) = split_profiles(src)?;
        if name != DEFAULT_PROFILE {
            match profiles.remove(name) {
                Some(toml::Value::Table(over)) => merge(&mut table, over),
                Some(_) => {
                    return Err(ScoreError(format!("[profile.{name}] must be a table")));
                }
                None => {
                    let known = std::iter::once(DEFAULT_PROFILE)
                        .chain(profiles.keys().map(String::as_str))
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(ScoreError(format!("no score profile {name:?} (known: {known})")));
                }
            }
        }
        let file: ScoreFile = toml::Value::Table(table).try_into().map_err(|e| match name {
            DEFAULT_PROFILE => ScoreError(format!("score.toml: {e}")),
            _ => ScoreError(format!("score.toml [profile.{name}]: {e}")),
        })?;
        let (p, c) = (file.progress, file.care);
        let flourish = match file.flourish {
            None => None,
            Some(f) => {
                if !(f.cap.is_finite() && (0.0..=1.0).contains(&f.cap)) {
                    return Err(ScoreError(format!("[flourish] cap = {} must be in 0..=1", f.cap)));
                }
                let measures: Vec<(FlourishMeasure, u32)> = [
                    (FlourishMeasure::Research, f.research),
                    (FlourishMeasure::Trade, f.trade),
                    (FlourishMeasure::Newcomers, f.newcomers),
                ]
                .into_iter()
                .filter(|&(_, target)| target > 0)
                .collect();
                if f.cap > 0.0 && measures.is_empty() {
                    return Err(ScoreError(
                        "[flourish] has a cap but no measure with a target above 0".to_owned(),
                    ));
                }
                (f.cap > 0.0).then_some(FlourishDef { cap: f.cap, measures })
            }
        };
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
            profile: name.to_owned(),
            flourish,
            progress: ProgressDef {
                late_credit: unit("late_credit", p.late_credit)?,
                partial_credit: unit("partial_credit", p.partial_credit)?,
                default_weight: p.default_weight,
                weights,
                task_credit: unit("task_credit", p.task_credit)?,
                avatar_arrival: if p.avatar_arrival.is_finite() && p.avatar_arrival >= 0.0 {
                    p.avatar_arrival
                } else {
                    return Err(ScoreError(format!(
                        "[progress] avatar_arrival = {} must be 0 or more",
                        p.avatar_arrival
                    )));
                },
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
        if id == AVATAR_ID {
            return self.avatar_arrival;
        }
        self.weights.iter().find(|(w, _)| w == id).map_or(self.default_weight, |&(_, w)| w)
    }

    /// The credit for a milestone first met at tick `t`, on a track whose turn 0 is tick `start`
    /// and whose horizon is tick `horizon`: 1 at or before `start`, `late_credit` at the horizon,
    /// a straight line between.
    pub fn met_credit(&self, t: u64, start: u64, horizon: u64) -> f64 {
        let span = horizon.saturating_sub(start);
        let fraction =
            if span == 0 { 0.0 } else { (t.saturating_sub(start) as f64 / span as f64).min(1.0) };
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

/// Where a track's turn 0 is: the world tick it starts at, and the care tally at that moment.
/// `ScoreStart::default()` is a track with no prelude (tick 0, nothing tallied yet).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScoreStart {
    /// The world's tick at turn 0: 0, or the end of a scenario's prelude.
    pub tick: u64,
    /// `World::care()` at that tick, subtracted from the final tally.
    pub care: CareTally,
}

impl ScoreStart {
    /// Turn 0 is `world` as it stands now: call it right after a prelude has run.
    pub fn of(world: &World) -> Self {
        Self { tick: world.tick, care: world.care().clone() }
    }
}

/// What kind of milestone a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MilestoneKind {
    Sign,
    Ready,
    Stage,
    /// The Avatar's arrival (#1009), when the profile weighs it.
    Avatar,
}

/// One task of a stage, on its stage's row (#1009).
#[derive(Debug, Clone, PartialEq)]
pub struct TaskRow {
    pub id: String,
    pub name: String,
    /// The tick it was done, within the horizon; `None` if it wasn't.
    pub done: Option<u64>,
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
    /// A stage's tasks, in data order (#1009); empty for every other kind and for a stage with
    /// none. While the stage isn't done, they earn it `task_credit` × their mean credit.
    pub tasks: Vec<TaskRow>,
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

/// The flourishing bonus (#1010): what the run did between Ready and the horizon.
#[derive(Debug, Clone, PartialEq)]
pub struct FlourishScore {
    /// `[flourish] cap`.
    pub cap: f64,
    /// The mean of the measures' shares of their targets, in 0..=1 (0 without Ready).
    pub value: f64,
    /// `cap × value`: what is added to progress (before the sum is capped at 1).
    pub bonus: f64,
    /// Each measure: its count after Ready, within the horizon, and its target.
    pub rows: Vec<(FlourishMeasure, u32, u32)>,
}

/// A run's score, with everything it was computed from.
#[derive(Debug, Clone, PartialEq)]
pub struct Scorecard {
    /// The world tick of turn 0 (`ScoreStart::tick`).
    pub start_tick: u64,
    pub horizon_ticks: u64,
    /// The world's tick when it was scored.
    pub world_tick: u64,
    /// The world had already run past the horizon, so the care tally and the unmet-sign values
    /// describe a later moment than the horizon (module doc).
    pub past_horizon: bool,
    /// `SCORE_SCALE × min(1, progress + flourish bonus) × care.value`.
    pub score: f64,
    /// Weighted credits ÷ weights, in 0..=1.
    pub progress: f64,
    /// Weighted credits summed ("3.58" of 14).
    pub credit_total: f64,
    /// Weights summed (14 by default).
    pub weight_total: f64,
    pub care: CareScore,
    /// The flourishing bonus, when the profile has one (#1010).
    pub flourish: Option<FlourishScore>,
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
    start: u64,
    horizon: u64,
    world_tick: u64,
    milestones: Vec<Milestone>,
    ready_latched: bool,
    deaths: u32,
    close_calls: u32,
    person_minutes: f64,
    severe_any_minutes: f64,
    severe_minutes: Vec<(Severe, f64)>,
    /// Each flourish measure's count after Ready, within the horizon (#1010).
    flourish_counts: Vec<(FlourishMeasure, u32)>,
}

struct Milestone {
    id: String,
    kind: MilestoneKind,
    name: String,
    first_met: Option<u64>,
    value: u32,
    target: u32,
    met_now: bool,
    tasks: Vec<TaskRow>,
}

impl Scorecard {
    /// Scores `world` at `horizon_ticks` (a world tick; `TICKS_PER_SECOND` to a second) for a
    /// track whose turn 0 is `start` (module doc, "Preludes"). See the module doc: the world
    /// should be at or before the horizon, and `start.tick` at or before that.
    pub fn compute(
        world: &World,
        defs: &Defs,
        score: &ScoreDef,
        start: &ScoreStart,
        horizon_ticks: u64,
    ) -> Self {
        let within = |tick: u64| tick <= horizon_ticks;
        let first = |want: &dyn Fn(&EndeavorEventKind) -> bool| {
            world
                .endeavor_events()
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
                tasks: Vec::new(),
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
            tasks: Vec::new(),
        });
        for stage in &defs.endeavor.stages {
            let done = first(&|k| matches!(k, EndeavorEventKind::StageDone(id) if *id == stage.id));
            let tasks = stage
                .tasks
                .iter()
                .map(|t| TaskRow {
                    id: t.id.clone(),
                    name: t.name.clone(),
                    done: first(&|k| matches!(k, EndeavorEventKind::TaskDone(id) if *id == t.id)),
                })
                .collect();
            milestones.push(Milestone {
                id: stage.id.clone(),
                kind: MilestoneKind::Stage,
                name: stage.name.clone(),
                first_met: done,
                value: u32::from(done.is_some()),
                target: 1,
                met_now: done.is_some(),
                tasks,
            });
        }
        if score.progress.avatar_arrival > 0.0 {
            let arrived = first(&|k| matches!(k, EndeavorEventKind::AvatarArrived));
            let name = defs.endeavor.avatar.as_ref().map_or("The Avatar", |a| a.name.as_str());
            milestones.push(Milestone {
                id: AVATAR_ID.to_owned(),
                kind: MilestoneKind::Avatar,
                name: format!("{name} arrives"),
                first_met: arrived,
                value: u32::from(arrived.is_some()),
                target: 1,
                met_now: arrived.is_some(),
                tasks: Vec::new(),
            });
        }

        let after_ready = |tick: u64| ready.is_some_and(|r| tick > r) && within(tick);
        let flourish_counts = FlourishMeasure::ALL
            .into_iter()
            .map(|m| {
                let n = match m {
                    FlourishMeasure::Research => {
                        world.tech_completions().filter(|e| after_ready(e.tick)).count()
                    }
                    FlourishMeasure::Trade => world
                        .caravan_events()
                        .filter(|e| {
                            after_ready(e.tick)
                                && matches!(
                                    e.kind,
                                    CaravanEventKind::DealDone { .. }
                                        | CaravanEventKind::RequestDone { .. }
                                )
                        })
                        .count(),
                    FlourishMeasure::Newcomers => {
                        world.arrivals().filter(|a| after_ready(a.tick)).count()
                    }
                };
                (m, n as u32)
            })
            .collect();
        let care = world.care().since(&start.care);
        let severe_minutes = score.care.severe.iter().map(|&s| (s, care.severe_minutes(s)));
        let deaths = world.deaths().filter(|d| d.tick > start.tick && within(d.tick)).count();
        Self::build(
            score,
            Inputs {
                start: start.tick,
                horizon: horizon_ticks,
                world_tick: world.tick,
                milestones,
                ready_latched: ready.is_some(),
                deaths: deaths as u32,
                close_calls: care.close_calls(),
                person_minutes: care.person_minutes(),
                severe_any_minutes: care.severe_any_minutes(),
                severe_minutes: severe_minutes.collect(),
                flourish_counts,
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
                        p.met_credit(t, inputs.start, inputs.horizon)
                    }
                    (MilestoneKind::Sign, _) => p.partial(m.value, m.target),
                    (_, Some(t)) => p.met_credit(t, inputs.start, inputs.horizon),
                    // A stage not done: a share of its done tasks (#1009).
                    (MilestoneKind::Stage, None) if !m.tasks.is_empty() => {
                        let sum: f64 = m
                            .tasks
                            .iter()
                            .filter_map(|t| t.done)
                            .map(|t| p.met_credit(t, inputs.start, inputs.horizon))
                            .sum();
                        p.task_credit * sum / m.tasks.len() as f64
                    }
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
                    tasks: m.tasks,
                }
            })
            .collect();
        let weight_total: f64 = rows.iter().map(|r| r.weight).sum();
        let credit_total: f64 = rows.iter().map(|r| r.weight * r.credit).sum();
        let progress = if weight_total > 0.0 { credit_total / weight_total } else { 0.0 };
        let flourish = score.flourish.as_ref().map(|f| {
            let rows: Vec<(FlourishMeasure, u32, u32)> = f
                .measures
                .iter()
                .map(|&(m, target)| {
                    let n =
                        inputs.flourish_counts.iter().find(|(c, _)| *c == m).map_or(0, |&(_, n)| n);
                    (m, n, target)
                })
                .collect();
            let value = rows
                .iter()
                .map(|&(_, n, target)| (f64::from(n) / f64::from(target)).min(1.0))
                .sum::<f64>()
                / rows.len().max(1) as f64;
            FlourishScore { cap: f.cap, value, bonus: f.cap * value, rows }
        });
        let bonus = flourish.as_ref().map_or(0.0, |f| f.bonus);

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
            start_tick: inputs.start,
            horizon_ticks: inputs.horizon,
            world_tick: inputs.world_tick,
            past_horizon: inputs.world_tick > inputs.horizon,
            score: SCORE_SCALE * (progress + bonus).min(1.0) * value,
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
            flourish,
            milestones: rows,
        }
    }
}

#[cfg(test)]
mod tests;

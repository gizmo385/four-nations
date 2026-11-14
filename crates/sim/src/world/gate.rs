//! Building the endeavor's gate: placing its site and raising it a stage at a time (issue #236;
//! docs/design/endgame.md, "The project: four stages").
//!
//! Once the endeavor is Ready the player places the gate (`Command::BeginEndeavor`) with the
//! usual placement rules (#207). The site goes into the ordinary site table, so its footprint
//! keeps other buildings off and a unit works it like any site, but its cost and work come from
//! the current stage in `data/endeavor.toml`, kept in `EndeavorState::Building`.
//!
//! Each stage is done in this order:
//!
//! 1. One `JobKind::Build` job, as for any building: a settler hauls the stage's cost to the site,
//!    then works it for the stage's `work` (scaled by `construction`). The same job then hauls
//!    each task's own cost, in task order.
//! 2. Each task becomes a `JobKind::EndeavorTask` the moment its materials are in. Only a unit of
//!    the task's element with the chi may take it. It walks onto the plaza, channels for the
//!    task's `work` and spends the chi as a bend does, and the bend counts toward mastery.
//! 3. When the work and every task are done, the stage is logged (`EndeavorEventKind::StageDone`)
//!    and the next one begins. After the last, the site becomes the finished building and the
//!    endeavor is `Complete`.
//!
//! The site may be cancelled (`Command::CancelEndeavor`) until work on the first stage starts.
//! Materials delivered drop beside it, as for `CancelBuild`, and the endeavor is Ready again.
//!
//! Stages and tasks are saved by string id, so a reordered data file doesn't misread a save. A
//! task job names its task by its place in `GateSite::tasks`, which is saved state, not the data.
//! A stage or task whose id the data no longer has simply stalls.
//!
//! **The Harmony Rite** (`StageDef::rite`, issue #238) is the one stage that doesn't follow the
//! steps above: it has no cost, work or tasks, so the loop above (`gate_progressed`) hands it
//! straight to its own handling the moment it begins. Instead of hauling and channelling:
//!
//! 1. Every tile of the plaza opens a low-priority `JobKind::Attend` job (`work.rs`), and the
//!    Avatar takes his own place directly (`Task::Rite`, assigned by `run_idle`, never a `Job`:
//!    needs never pre-empt it, `consider_needs`).
//! 2. Once a second (`tick_rite`), `GateSite::rite_progress` gains a second while the Avatar
//!    holds his place and enough of the living population (never counting him) holds an Attend
//!    job; short of that, it pauses (`EndeavorEventKind::RitePaused`) without losing what it has.
//! 3. A settler's death resets it to nothing (`World::tick_needs`, `break_rite`), logging
//!    `RiteBroken`: "That makes the last ten minutes a test of everything the player built"
//!    (`docs/design/endgame.md`).
//! 4. Once `RiteDef::minutes` of progress is in, `finish_rite` logs the stage done, drops the
//!    Attend and task jobs, finishes the gate (`EndeavorEventKind::Complete`) and schedules the
//!    Avatar's farewell (`world/avatar.rs`): he stays `AvatarDef::farewell_minutes` longer, then
//!    leaves for good.

use crate::building::{BuildingId, CostItem};
use crate::endeavor::{StageDef, TaskDef, TaskWho};
use crate::geom::Footprint;
use crate::jobs::{ItemId, JobKind, Stack};

use super::build::{Blocker, Site};
use super::stand::StandIns;
use super::work::route_from;
use super::*;

/// Why an endeavor command was refused. The world is unchanged when `apply` returns one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndeavorError {
    /// The endeavor isn't Ready: its signs aren't all met yet, or the site is already placed.
    NotReady,
    /// The data has no gate to build (no stages in `data/endeavor.toml`).
    NoGate,
    /// A footprint tile can't take the gate, as `BuildError::CannotBuild` says.
    CannotBuild { at: Pos, why: Blocker },
    /// There is no site to cancel.
    NotBuilding,
    /// Work on the site has started, so it can no longer be cancelled.
    Started,
}

impl std::fmt::Display for EndeavorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotReady => f.write_str("the endeavor isn't ready for a site"),
            Self::NoGate => f.write_str("there is nothing to build"),
            Self::CannotBuild { why, .. } => write!(f, "{why}"),
            Self::NotBuilding => f.write_str("no site to cancel"),
            Self::Started => f.write_str("work has started"),
        }
    }
}

impl std::error::Error for EndeavorError {}

/// The gate going up: where, which stage, and how far that stage has got.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GateSite {
    /// The top-left tile of its footprint.
    pub anchor: Pos,
    /// When the endeavor became Ready, which cancelling the site goes back to.
    pub ready_tick: u64,
    /// The current stage's id.
    pub stage: String,
    /// How much of the stage's own cost has been delivered, by item.
    pub delivered: Vec<Stack>,
    /// Construct work done on the stage, in work-seconds (of `StageDef::work`).
    pub progress: f32,
    /// The stage's tasks, in data order as of when the stage began.
    pub tasks: Vec<GateTask>,
    /// The Harmony Rite's own progress, in seconds of attendance (`StageDef::rite`, #238). Zero
    /// outside that stage. `#[serde(default)]`: a save from before #238 reads as not yet begun.
    #[serde(default)]
    pub rite_progress: f32,
    /// Whether the rite's progress is currently paused (attendance short of the share): kept so
    /// a pause, and its end, is logged only once each (`tick_rite`). Only meaningful on the rite
    /// stage. `#[serde(default)]`, like `rite_progress`.
    #[serde(default)]
    pub rite_paused: bool,
}

/// One of the current stage's tasks.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GateTask {
    pub id: String,
    /// How much of the task's cost has been delivered, by item.
    pub delivered: Vec<Stack>,
    /// Whether the task's bend has been done.
    pub done: bool,
}

impl GateSite {
    fn new(anchor: Pos, ready_tick: u64, stage: &StageDef) -> Self {
        let mut site = Self {
            anchor,
            ready_tick,
            stage: String::new(),
            delivered: Vec::new(),
            progress: 0.0,
            tasks: Vec::new(),
            rite_progress: 0.0,
            rite_paused: false,
        };
        site.begin(stage);
        site
    }

    /// Starts `stage` from nothing.
    fn begin(&mut self, stage: &StageDef) {
        self.stage = stage.id.clone();
        self.delivered.clear();
        self.progress = 0.0;
        self.rite_progress = 0.0;
        self.rite_paused = false;
        self.tasks = stage
            .tasks
            .iter()
            .map(|t| GateTask { id: t.id.clone(), delivered: Vec::new(), done: false })
            .collect();
    }
}

/// What the gate's construct job does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    /// Fetch this many of this item.
    Fetch(ItemId, u32),
    /// Work the stage.
    Work,
    /// Nothing: the stage's work is done and every task has its materials.
    Done,
}

/// How many of `item`, or of a stand-in for it (brick for stone, issue #371), are in `list`.
fn count_of(list: &[Stack], item: ItemId, st: &StandIns, defs: &Defs) -> u32 {
    list.iter().filter(|s| st.pays(s.item, item, defs)).map(|s| s.count).sum()
}

/// The first cost entry not yet fully delivered, and how many are still short. The item named is
/// the cost's own (a stand-in pays it just as well, `World::nearest_source_for`).
fn shortfall(
    cost: &[CostItem],
    delivered: &[Stack],
    st: &StandIns,
    defs: &Defs,
) -> Option<(ItemId, u32)> {
    cost.iter().find_map(|c| {
        let have = count_of(delivered, c.item, st, defs);
        (have < c.count).then_some((c.item, c.count - have))
    })
}

/// Adds up to what `cost` still needs of `stack`'s item to `delivered` (directly, or as a
/// stand-in for one of `cost`'s items), and returns how many that was. `delivered` keeps the
/// item actually handed over, so cancelling refunds it, not what it stood in for.
fn take_into(
    cost: &[CostItem],
    delivered: &mut Vec<Stack>,
    item: ItemId,
    count: u32,
    st: &StandIns,
    defs: &Defs,
) -> u32 {
    let Some(c) = cost.iter().find(|c| st.pays(item, c.item, defs)) else {
        return 0;
    };
    let n = c.count.saturating_sub(count_of(delivered, c.item, st, defs)).min(count);
    if n > 0 {
        match delivered.iter_mut().find(|s| s.item == item) {
            Some(s) => s.count += n,
            None => delivered.push(Stack { item, count: n }),
        }
    }
    n
}

/// The data for `task` in `stage`.
fn task_def<'d>(stage: &'d StageDef, task: &GateTask) -> Option<&'d TaskDef> {
    stage.tasks.iter().find(|t| t.id == task.id)
}

/// How far from the settlement's anchor `World::find_endeavor_site` looks for the gate's site.
const GATE_SEARCH: i32 = 24;

/// Queries, and the commands behind `Command::BeginEndeavor` and `Command::CancelEndeavor`.
impl World {
    /// Whether `anchor` is the anchor of the endeavor's site.
    pub(super) fn is_gate(&self, anchor: Pos) -> bool {
        matches!(&self.endeavor, EndeavorState::Building(g) if g.anchor == anchor)
    }

    pub(super) fn gate(&self) -> Option<&GateSite> {
        match &self.endeavor {
            EndeavorState::Building(g) => Some(g),
            _ => None,
        }
    }

    pub(super) fn gate_mut(&mut self) -> Option<&mut GateSite> {
        match &mut self.endeavor {
            EndeavorState::Building(g) => Some(g),
            _ => None,
        }
    }

    /// The stage the gate is on, with its place in the list, while it is being built.
    pub fn gate_stage<'d>(&self, defs: &'d Defs) -> Option<(usize, &'d StageDef)> {
        defs.endeavor.stage(&self.gate()?.stage)
    }

    /// The gate site's anchor, while it is being built: where the Avatar's arrival (#237) is
    /// searched from.
    pub(super) fn gate_anchor(&self) -> Option<Pos> {
        self.gate().map(|g| g.anchor)
    }

    /// Whether the gate is past its first stage (the foundation): the Avatar arrives two minutes
    /// after it is done (`world/avatar.rs`, #237). Also true once the gate is finished outright,
    /// for the edge case of a single-stage gate.
    pub(super) fn gate_past_first_stage(&self, defs: &Defs) -> bool {
        match &self.endeavor {
            EndeavorState::Building(g) => defs.endeavor.stage(&g.stage).is_some_and(|(n, _)| n > 0),
            EndeavorState::Complete { .. } => true,
            _ => false,
        }
    }

    /// Whether the gate may be placed with its anchor (top-left tile) on `anchor`: the endeavor
    /// must be Ready, then every footprint tile free, row-major. Changes nothing, so a frontend
    /// can call it to preview a placement.
    pub fn check_endeavor_site(&self, anchor: Pos, defs: &Defs) -> Result<(), EndeavorError> {
        if !matches!(self.endeavor, EndeavorState::Ready { .. }) {
            return Err(EndeavorError::NotReady);
        }
        let Some(gate) = defs.endeavor.building else { return Err(EndeavorError::NoGate) };
        self.footprint_blocker(anchor, defs.building(gate), defs)
            .map_or(Ok(()), |(at, why)| Err(EndeavorError::CannotBuild { at, why }))
    }

    /// Whether the gate's site may be withdrawn now (`Command::CancelEndeavor`): there must be
    /// one, and no work on it may have started. Changes nothing, so a frontend can call it to
    /// say whether withdrawing is on offer.
    pub fn check_cancel_endeavor(&self, defs: &Defs) -> Result<(), EndeavorError> {
        let Some(g) = self.gate() else { return Err(EndeavorError::NotBuilding) };
        let first = defs.endeavor.stages.first().map(|s| s.id.as_str());
        let channelled = self
            .jobs
            .iter()
            .any(|j| matches!(j.kind, JobKind::EndeavorTask { .. }) && j.progress > 0.0);
        if first != Some(g.stage.as_str())
            || g.progress > 0.0
            || g.tasks.iter().any(|t| t.done)
            || channelled
        {
            return Err(EndeavorError::Started);
        }
        Ok(())
    }

    /// Places the gate's site with its anchor on `anchor` and starts the first stage.
    pub(super) fn begin_endeavor(&mut self, anchor: Pos, defs: &Defs) -> Result<(), EndeavorError> {
        self.check_endeavor_site(anchor, defs)?;
        let EndeavorState::Ready { tick } = self.endeavor else { unreachable!("checked") };
        let gate: BuildingId = defs.endeavor.building.expect("checked");
        let (w, h) = defs.building(gate).footprint;
        // The loader allows a gate only with stages.
        let first = &defs.endeavor.stages[0];
        self.sites.insert(anchor, Site::gate(gate, [w, h]));
        self.endeavor = EndeavorState::Building(GateSite::new(anchor, tick, first));
        self.gate_progressed(defs);
        Ok(())
    }

    /// The nearest spot to the settlement's anchor where the gate's footprint is free and its door
    /// can be walked to from the stockpile, searching `GATE_SEARCH` tiles out (row-major ties). The
    /// endeavor must be Ready, as for `check_endeavor_site`. The economy opening's `begin_gate`
    /// and `complete_endeavor` both place the gate with this.
    pub fn find_endeavor_site(&self, defs: &Defs) -> Option<Pos> {
        let gate = defs.endeavor.building?;
        let (w, h) = defs.building(gate).footprint;
        let anchor = self.settlement_anchor();
        let flood = self.flood_from_anchor(anchor, defs);
        let r = GATE_SEARCH;
        let mut spots: Vec<Pos> = (anchor.y - r..=anchor.y + r)
            .flat_map(|y| (anchor.x - r..=anchor.x + r).map(move |x| Pos::new(x, y)))
            .collect();
        spots.sort_by_key(|&p| (p.chebyshev(anchor), p));
        spots.into_iter().find(|&p| {
            self.check_endeavor_site(p, defs).is_ok()
                && flood.cost(Footprint::new(p, w, h).entrance()).is_some()
        })
    }

    /// A dev hook (#751): stands the gate now, whatever the endeavor's state, and returns whether
    /// it changed anything (`false` if the endeavor is already Complete). For the harness's
    /// `united` runs and the `<nation>-united` scenarios, where the economy opening never
    /// finishes the gate on its own inside 420 minutes.
    ///
    /// It takes the same road a finished gate does: a Dormant endeavor is made Ready, a site is
    /// placed at `find_endeavor_site` (a finished gate on the map, or none if no spot is free),
    /// every unfinished stage is skipped (no `StageDone` for it, so `stage_minute.N` stays blank:
    /// those stages were not played), and the endeavor is Complete with its event at this tick.
    /// The Avatar, who would have come and gone with the rite, is dismissed.
    pub fn complete_endeavor(&mut self, defs: &Defs) -> bool {
        if self.united() {
            return false;
        }
        if self.endeavor == EndeavorState::Dormant {
            self.endeavor = EndeavorState::Ready { tick: self.tick };
        }
        if matches!(self.endeavor, EndeavorState::Ready { .. })
            && let Some(at) = self.find_endeavor_site(defs)
        {
            let _ = self.begin_endeavor(at, defs);
        }
        match self.gate().map(|g| g.anchor) {
            Some(anchor) => {
                self.drop_gate_jobs(anchor);
                self.finish_gate(anchor, defs);
            }
            None => {
                // No spot for the building: the city still stands, without its gate.
                self.endeavor = EndeavorState::Complete { tick: self.tick };
                self.endeavor_event(EndeavorEventKind::Complete);
            }
        }
        self.dismiss_avatar();
        true
    }

    /// Withdraws the site, if no work on it has started: materials delivered drop beside it and
    /// the endeavor is Ready again.
    pub(super) fn cancel_endeavor(&mut self, defs: &Defs) -> Result<(), EndeavorError> {
        self.check_cancel_endeavor(defs)?;
        let Some(g) = self.gate() else { unreachable!("checked") };
        let tick = g.ready_tick;
        let EndeavorState::Building(g) =
            std::mem::replace(&mut self.endeavor, EndeavorState::Ready { tick })
        else {
            unreachable!("just matched")
        };
        self.sites.remove(&g.anchor);
        self.drop_gate_jobs(g.anchor);
        for &stack in g.delivered.iter().chain(g.tasks.iter().flat_map(|t| &t.delivered)) {
            if stack.count > 0 {
                self.place_ground(g.anchor, stack);
            }
        }
        Ok(())
    }

    /// Drops the gate's construct job, every task job and the rite's Attend jobs. A unit that
    /// held one finds it gone and lets go.
    fn drop_gate_jobs(&mut self, anchor: Pos) {
        self.jobs.retain(|j| match j.kind {
            JobKind::Build { pos } => pos != anchor,
            JobKind::EndeavorTask { .. } | JobKind::Attend { .. } => false,
            _ => true,
        });
    }

    /// The task at `task` in the current stage (`JobKind::EndeavorTask`), with its data and the
    /// site's anchor, unless it is done.
    pub fn gate_task<'d>(&self, task: u8, defs: &'d Defs) -> Option<(&'d TaskDef, Pos)> {
        let g = self.gate()?;
        let (_, stage) = defs.endeavor.stage(&g.stage)?;
        let state = g.tasks.get(usize::from(task)).filter(|t| !t.done)?;
        Some((task_def(stage, state)?, g.anchor))
    }

    /// The chi the Avatar keeps back for the gate (#1011): the most that any undone task of the
    /// current stage costs, among those only he can do here (an Avatar task, or a nation's task
    /// with no living settler of that nation). 0 with no stage under way. `unit_can_bend` holds
    /// it back from his other bends, so a governor that keeps designating bends only he can
    /// cast can't drain the chi a pillar or the beam lift needs.
    pub(super) fn avatar_chi_reserve(&self, defs: &Defs) -> f32 {
        let (Some((_, stage)), Some(g)) = (self.gate_stage(defs), self.gate()) else {
            return 0.0;
        };
        g.tasks
            .iter()
            .filter(|t| !t.done)
            .filter_map(|t| task_def(stage, t))
            .filter(|d| match d.element {
                TaskWho::Avatar => true,
                TaskWho::Nation(n) => {
                    !self.units.iter().any(|u| u.is_settler() && self.nation_of(u) == n)
                }
            })
            .map(|d| d.chi)
            .fold(0.0, f32::max)
    }

    /// Whether unit `i` may take `task`: it is allowed to (its own nation, or the Avatar for any
    /// task) and its chi covers the cost.
    pub(super) fn unit_can_raise(&self, i: usize, task: &TaskDef) -> bool {
        let u = &self.units[i];
        if u.chi < task.chi {
            return false;
        }
        match task.element {
            TaskWho::Nation(n) => self.nation_of(u) == n || u.is_avatar(),
            TaskWho::Avatar => u.is_avatar(),
        }
    }
}

/// Doing it.
impl World {
    /// What the construct job does next: the stage's cost, then its work, then each task's cost.
    fn gate_next(&self, defs: &Defs) -> Next {
        let st = self.stand_ins(defs);
        let (Some(g), Some((_, stage))) = (self.gate(), self.gate_stage(defs)) else {
            return Next::Done;
        };
        if let Some((item, n)) = shortfall(&stage.cost, &g.delivered, &st, defs) {
            return Next::Fetch(item, n);
        }
        if g.progress < stage.work {
            return Next::Work;
        }
        g.tasks
            .iter()
            .filter(|t| !t.done)
            .filter_map(|t| shortfall(&task_def(stage, t)?.cost, &t.delivered, &st, defs))
            .map(|(item, n)| Next::Fetch(item, n))
            .next()
            .unwrap_or(Next::Done)
    }

    /// Adds a delivered stack to the stage's cost, then to the tasks' in order (directly, or as a
    /// stand-in, issue #371). Anything left over (the stage moved on mid-trip) drops beside the
    /// site.
    fn deliver_to_gate(&mut self, stack: Stack, defs: &Defs) {
        let st = self.stand_ins(defs);
        let Some((_, stage)) = self.gate_stage(defs) else { return };
        let g = self.gate_mut().expect("has a stage");
        let mut left = stack.count;
        left -= take_into(&stage.cost, &mut g.delivered, stack.item, left, &st, defs);
        for t in g.tasks.iter_mut().filter(|t| !t.done) {
            if let Some(def) = stage.tasks.iter().find(|d| d.id == t.id) {
                left -= take_into(&def.cost, &mut t.delivered, stack.item, left, &st, defs);
            }
        }
        if left > 0 {
            let anchor = g.anchor;
            self.place_ground(anchor, Stack { item: stack.item, count: left });
        }
    }

    /// After anything that moves the gate on: queues the tasks whose materials are in, makes sure
    /// a construct job exists while there is hauling or work left, and starts the next stage, or
    /// finishes the gate, once the current one is done.
    fn gate_progressed(&mut self, defs: &Defs) {
        let st = self.stand_ins(defs);
        loop {
            let (Some(g), Some((n, stage))) = (self.gate(), self.gate_stage(defs)) else { return };
            let anchor = g.anchor;
            if stage.rite.is_some() {
                // The rite's own progress is attendance, not hauling or channelling: open its
                // Attend jobs (if they aren't already) and leave it to `tick_rite` (#238).
                return self.open_rite(anchor);
            }
            let built = g.progress >= stage.work
                && shortfall(&stage.cost, &g.delivered, &st, defs).is_none();
            let ready: Vec<u8> = g
                .tasks
                .iter()
                .enumerate()
                .filter(|&(_, t)| {
                    built
                        && !t.done
                        && task_def(stage, t)
                            .is_some_and(|d| shortfall(&d.cost, &t.delivered, &st, defs).is_none())
                })
                .map(|(k, _)| k as u8)
                .filter(|&k| !self.jobs.iter().any(|j| j.kind == JobKind::EndeavorTask { task: k }))
                .collect();
            let all_done = g.tasks.iter().all(|t| t.done);
            for task in ready {
                self.add_job(JobKind::EndeavorTask { task });
            }
            let next = self.gate_next(defs);
            if next != Next::Done {
                if !self.jobs.iter().any(|j| j.kind == JobKind::Build { pos: anchor }) {
                    self.add_job(JobKind::Build { pos: anchor });
                }
                return;
            }
            if !all_done {
                return;
            }
            self.endeavor_event(EndeavorEventKind::StageDone(stage.id.clone()));
            self.drop_gate_jobs(anchor);
            match defs.endeavor.stages.get(n + 1) {
                Some(following) => self.gate_mut().expect("still building").begin(following),
                None => return self.finish_gate(anchor, defs),
            }
        }
    }

    /// The last stage is done: the site becomes the finished gate, and the endeavor is Complete.
    /// Like any finished building, it keeps a fire if its def says so (`put_building`): the
    /// gate's own radius-3 heat (`docs/design/endgame.md`, "The transformation", #240).
    fn finish_gate(&mut self, anchor: Pos, defs: &Defs) {
        let foot = self.footprint_at(anchor).expect("the gate's site");
        let site = self.sites.remove(&anchor).expect("the gate's site");
        self.raise(foot, site.building, defs);
        self.endeavor = EndeavorState::Complete { tick: self.tick };
        self.endeavor_event(EndeavorEventKind::Complete);
    }

    /// The gate's construct job (`run_build` hands it over): haul and work the current stage as
    /// `gate_next` says.
    pub(super) fn run_gate_build(&mut self, i: usize, id: JobId, anchor: Pos, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        if let Some(stack) = self.units[i].carrying {
            // Carrying a trip's worth to the site.
            if !self.route_to_gate(i, anchor, defs) {
                return self.abandon_job(i, id, defs);
            }
            if !self.walk(i, defs) {
                return;
            }
            self.units[i].carrying = None;
            self.units[i].route = None;
            self.deliver_to_gate(stack, defs);
            return self.gate_progressed(defs);
        }
        match self.gate_next(defs) {
            Next::Fetch(item, needed) => {
                // Fetches the item itself or a stand-in for it (brick for stone, issue #371).
                if self.keep_walking(i, defs) {
                    return;
                }
                let flood = self.flood_from(i);
                let Some((source, stand, found)) = self.nearest_source_for(&flood, item, defs)
                else {
                    return self.abandon_job(i, id, defs);
                };
                if self.units[i].route.is_none() {
                    match flood.path_to(stand) {
                        Some(path) => self.units[i].route = Some(route_from(path)),
                        None => return self.abandon_job(i, id, defs),
                    }
                }
                if !self.walk(i, defs) {
                    return;
                }
                let take = self.take_for_build(i, source, needed, defs);
                if take == 0 {
                    // Someone else got there first: look again next tick.
                    return self.abandon_job(i, id, defs);
                }
                self.units[i].carrying = Some(Stack { item: found, count: take });
                self.units[i].route = None;
            }
            Next::Work => {
                if !self.route_to_gate(i, anchor, defs) {
                    return self.abandon_job(i, id, defs);
                }
                if !self.walk(i, defs) {
                    return;
                }
                let rate = self.modifiers().construction;
                let work = self.gate_stage(defs).map_or(0.0, |(_, s)| s.work);
                let g = self.gate_mut().expect("building");
                g.progress = (g.progress + rate / TICKS_PER_SECOND as f32).min(work);
                if g.progress >= work {
                    // On to the tasks' materials, or the next stage (which drops this job).
                    self.units[i].route = None;
                    self.gate_progressed(defs);
                }
            }
            Next::Done => {
                self.jobs.remove(idx);
                self.finish_job(i);
                self.gate_progressed(defs);
            }
        }
    }

    /// Plans unit `i`'s way onto the site, unless it already has a route. False if it can't get
    /// there.
    fn route_to_gate(&mut self, i: usize, anchor: Pos, defs: &Defs) -> bool {
        self.plan_route(i, StandRule::Building(anchor), defs)
    }

    /// A task job: walk onto the site, channel for the task's work, spend the chi.
    pub(super) fn run_gate_task(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::EndeavorTask { task } = self.jobs[idx].kind else {
            return self.finish_job(i);
        };
        let Some((def, anchor)) = self.gate_task(task, defs) else {
            // The stage moved on or the data lost the task: moot.
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        if !self.unit_can_raise(i, def) {
            // Chi can only have gone up since the claim, so this is a safety net.
            return self.abandon_job(i, id, defs);
        }
        if !self.route_to_gate(i, anchor, defs) {
            return self.abandon_job(i, id, defs);
        }
        if !self.walk(i, defs) {
            return;
        }
        let job = &mut self.jobs[idx];
        job.progress += 1.0 / TICKS_PER_SECOND as f32;
        if job.progress < def.work {
            return;
        }
        self.jobs.remove(idx);
        self.finish_job(i);
        let u = &mut self.units[i];
        u.chi = (u.chi - def.chi).max(0.0);
        u.bends += 1;
        self.note_rank_up(i, defs);
        let state = &mut self.gate_mut().expect("checked above").tasks[usize::from(task)];
        state.done = true;
        let task_id = state.id.clone();
        self.endeavor_event(EndeavorEventKind::TaskDone(task_id));
        self.gate_progressed(defs);
    }
}

/// The Harmony Rite (issue #238; `docs/design/endgame.md`, "The Harmony Rite").
impl World {
    /// Is the gate on the stage that is the Harmony Rite right now? `run_idle` reads this to
    /// give the Avatar his own place in it, directly, instead of a `Job`.
    pub(super) fn gate_on_rite_stage(&self, defs: &Defs) -> bool {
        self.gate_stage(defs).is_some_and(|(_, s)| s.rite.is_some())
    }

    /// Opens an `Attend` job on every tile of the plaza, the moment the rite stage begins. A
    /// no-op once they're open (`gate_progressed` calls this every time it re-enters the rite
    /// stage, which today is only the once).
    fn open_rite(&mut self, anchor: Pos) {
        if self.jobs.iter().any(|j| matches!(j.kind, JobKind::Attend { .. })) {
            return;
        }
        let Some(foot) = self.footprint_at(anchor) else { return };
        for pos in foot.tiles() {
            self.add_job(JobKind::Attend { pos });
        }
    }

    /// How many settlers hold an Attend job right now: the rite's own attendance
    /// (`docs/design/endgame.md`, "What can go wrong": "Attendance only counts settlers standing
    /// on the plaza").
    fn attendance(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| matches!(j.kind, JobKind::Attend { .. }) && j.claimed_by.is_some())
            .count()
    }

    /// Once a second, while the gate is on the rite stage: accrues `GateSite::rite_progress`
    /// while the Avatar holds his own place in it and enough of the living population (never
    /// counting him) is attending, pausing (and logging it, once) otherwise, and finishing the
    /// rite once its minutes are in.
    pub(super) fn tick_rite(&mut self, defs: &Defs) {
        if !self.tick.is_multiple_of(u64::from(TICKS_PER_SECOND)) {
            return;
        }
        let Some((_, stage)) = self.gate_stage(defs) else { return };
        let Some(rite) = stage.rite else { return };
        let meditating = self.units.iter().any(|u| u.is_avatar() && u.job_task == Some(Task::Rite));
        let population = self.units.iter().filter(|u| u.is_settler()).count() as f32;
        // A full plaza is always enough: population growth (#232) can outgrow the plaza's places,
        // and the share alone would then pause the rite for good.
        let places = self.jobs.iter().filter(|j| matches!(j.kind, JobKind::Attend { .. })).count();
        let needed = (population * rite.attendance_share).min(places as f32);
        let enough = meditating && population > 0.0 && self.attendance() as f32 >= needed;
        let paused = self.gate().expect("checked above").rite_paused;
        if enough {
            if paused {
                self.gate_mut().expect("checked above").rite_paused = false;
            }
            let g = self.gate_mut().expect("checked above");
            g.rite_progress += 1.0;
            if g.rite_progress >= rite.minutes * 60.0 {
                self.finish_rite(defs);
            }
        } else if !paused {
            self.gate_mut().expect("checked above").rite_paused = true;
            self.endeavor_event(EndeavorEventKind::RitePaused);
        }
    }

    /// A settler's death during the rite breaks it: its progress resets to nothing
    /// (`docs/design/endgame.md`: "A death breaks the rite... That makes the last ten minutes a
    /// test of everything the player built"). Called from `World::tick_needs`; a no-op off the
    /// rite stage.
    pub(super) fn break_rite(&mut self, defs: &Defs) {
        if !self.gate_on_rite_stage(defs) {
            return;
        }
        self.gate_mut().expect("checked above").rite_progress = 0.0;
        self.endeavor_event(EndeavorEventKind::RiteBroken);
    }

    /// The rite's minutes are in: logs the stage done, drops the Attend and task jobs, finishes
    /// the gate (the last stage, by the data's own rule that the rite must be last) and starts
    /// the Avatar's farewell countdown.
    fn finish_rite(&mut self, defs: &Defs) {
        let g = self.gate().expect("checked by caller");
        let anchor = g.anchor;
        let stage_id = g.stage.clone();
        self.endeavor_event(EndeavorEventKind::StageDone(stage_id));
        self.drop_gate_jobs(anchor);
        if let Some(i) = self.units.iter().position(Unit::is_avatar) {
            self.units[i].job_task = None;
            self.units[i].route = None;
        }
        self.finish_gate(anchor, defs);
        self.begin_avatar_farewell(defs);
    }

    /// Holding an Attend job: walk onto the tile and stay. Never completes on its own; it ends
    /// when the rite does (`drop_gate_jobs`).
    pub(super) fn run_attend(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Attend { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        self.go_or_abandon(i, id, StandRule::Exact(pos), defs);
    }

    /// The Avatar's own place in the rite: walk onto the plaza and stay there, meditating, until
    /// `tick_rite` finishes it (`Task::Rite`, never a `Job`: `consider_needs` never pre-empts
    /// this).
    pub(super) fn run_rite(&mut self, i: usize, defs: &Defs) {
        let Some(anchor) = self.gate_anchor() else {
            // The gate finished (or its site is gone) out from under him: nothing to hold.
            self.units[i].job_task = None;
            self.units[i].route = None;
            return;
        };
        if !self.route_to_gate(i, anchor, defs) {
            // Can't reach it this tick (blocked some other way): try again next tick.
            return;
        }
        self.walk(i, defs);
    }
}

#[cfg(test)]
mod tests;

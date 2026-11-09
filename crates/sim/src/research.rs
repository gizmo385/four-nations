//! The tech tree: definitions from `data/research.toml`, resolved and validated at load.
//!
//! See `docs/design/research.md`. Techs are referenced by `TechId`, their index in the file.
//! The file must list prereqs before the techs that need them, so `TechId` order is a valid
//! topological order and cycles are impossible by construction. Later code relies on that: it can
//! walk techs in id order and know every prereq has already been visited.
//!
//! Static definitions come first. `Research` at the bottom is the live state that `World` holds:
//! known techs, the knowledge pool and the queue. `effective_modifiers` and
//! `modifier_breakdown` apply completed techs' multipliers to a nation's modifiers.

use serde::Deserialize;

use crate::defs::{Defs, DefsError};
use crate::nation::{ModifierMultipliers, Modifiers, Nation};
use crate::world::TICKS_PER_SECOND;

// `Deserialize` is `save::ids`' remapping impl (#602).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub struct TechId(pub u16);

/// Largest multiplier a single tech may apply. Keeps a typo like `12` instead of `1.2` from
/// loading.
pub const MAX_MULTIPLIER: f32 = 4.0;

/// The nation's base modifiers times every known tech's multipliers, applied in ascending
/// `TechId` order. `f32` multiplication isn't associative, so the order is fixed here rather than
/// left to completion order: any research order that reaches the same known set gives
/// bitwise-identical results.
pub fn effective_modifiers(
    nation: Nation,
    known: impl IntoIterator<Item = TechId>,
    defs: &Defs,
) -> Modifiers {
    let mut mods = defs.nation(nation).modifiers;
    for t in sorted(known) {
        mods.apply(&defs.tech(t).unlocks.modifiers);
    }
    mods
}

/// How `effective_modifiers` arrives at the modifier called `name`: the nation's base value, then
/// each known tech that multiplies it, in the same order as `effective_modifiers`. Each entry is
/// `(source name, factor)`; the product of the factors (left to right) is the effective value.
/// Empty if `name` isn't a modifier.
pub fn modifier_breakdown<'d>(
    name: &str,
    nation: Nation,
    known: impl IntoIterator<Item = TechId>,
    defs: &'d Defs,
) -> Vec<(&'d str, f32)> {
    let nation = defs.nation(nation);
    let Some(base) = nation.modifiers.get(name) else { return Vec::new() };
    let mut out = vec![(nation.name.as_str(), base)];
    for t in sorted(known) {
        let tech = defs.tech(t);
        if let Some((_, f)) = tech.unlocks.modifiers.iter().find(|(n, _)| *n == name) {
            out.push((tech.name.as_str(), f));
        }
    }
    out
}

/// `ids` in ascending order, without duplicates.
fn sorted(ids: impl IntoIterator<Item = TechId>) -> Vec<TechId> {
    let mut v: Vec<TechId> = ids.into_iter().collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Knowledge production rates. Converted to integer millis per tick by the research system.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchSettings {
    /// Settlement-wide trickle that bootstraps the first techs.
    pub passive_knowledge_per_minute: f64,
    /// Per scholar at a study hall, before the `study` modifier.
    pub study_knowledge_per_minute: f64,
    /// Knowledge points the pool may hold with no research target queued: just enough to
    /// bootstrap a first pick without letting the passive trickle (or a seat left idle, #284)
    /// quietly bank toward several techs at once. Below the cheapest tech's cost.
    pub idle_knowledge_cap: f64,
    /// `passive_knowledge_per_minute` as integer millis per tick, filled in at load.
    #[serde(skip)]
    pub passive_millis_per_tick: u64,
    /// `study_knowledge_per_minute` as integer millis per tick, filled in at load.
    #[serde(skip)]
    pub study_millis_per_tick: u64,
    /// `idle_knowledge_cap` in milli-points, filled in at load.
    #[serde(skip)]
    pub idle_knowledge_cap_millis: u64,
}

/// Knowledge is stored in milli-points so rates stay integers.
pub const MILLIS_PER_POINT: u64 = 1000;

/// Converts a points-per-minute rate to whole millis per tick (rounded to nearest).
fn millis_per_tick(per_minute: f64) -> u64 {
    (per_minute * MILLIS_PER_POINT as f64 / (60.0 * f64::from(TICKS_PER_SECOND))).round() as u64
}

/// What learning a tech makes available: modifier multipliers, plus ids checked against the def
/// kind they name once it loads (`building::BuildingsFile::resolve`, `jobs::JobsFile::resolve`,
/// `bending::BendingFile::resolve`), which also records the unlocking tech on that def's
/// `unlocked_by` field. An item no tech names this way is available from the start (issue #22).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unlocks {
    #[serde(default)]
    pub modifiers: ModifierMultipliers,
    /// Ids of buildings in `data/buildings.toml` this tech unlocks.
    #[serde(default)]
    pub buildings: Vec<String>,
    /// Ids of resources in `data/resources.toml` whose harvest designation (`data/jobs.toml`)
    /// this tech unlocks.
    #[serde(default)]
    pub jobs: Vec<String>,
    /// Ids of abilities in `data/bending.toml` this tech unlocks.
    #[serde(default)]
    pub abilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TechDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// `None` for the shared civilization tier; otherwise only this nation sees the tech.
    pub nation: Option<Nation>,
    /// Knowledge points, always > 0.
    pub cost: u32,
    /// Every prereq is smaller than this tech's own id (file order is topological).
    pub prereqs: Vec<TechId>,
    pub unlocks: Unlocks,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResearchFile {
    settings: ResearchSettings,
    #[serde(default)]
    tech: Vec<RawTech>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTech {
    id: String,
    name: String,
    description: String,
    #[serde(default)]
    nation: Option<Nation>,
    cost: u32,
    #[serde(default)]
    prereqs: Vec<String>,
    #[serde(default)]
    unlocks: Unlocks,
}

fn nation_label(n: Option<Nation>) -> &'static str {
    n.map_or("shared", Nation::as_str)
}

impl ResearchFile {
    /// Validates the file and resolves string prereqs to `TechId`s.
    pub(crate) fn resolve(self) -> Result<(ResearchSettings, Vec<TechDef>), DefsError> {
        let mut s = self.settings;
        for (what, v) in [
            ("passive_knowledge_per_minute", s.passive_knowledge_per_minute),
            ("study_knowledge_per_minute", s.study_knowledge_per_minute),
            ("idle_knowledge_cap", s.idle_knowledge_cap),
        ] {
            if !(v.is_finite() && v >= 0.0) {
                return Err(DefsError(format!(
                    "research settings: {what} must be finite and >= 0"
                )));
            }
        }
        s.passive_millis_per_tick = millis_per_tick(s.passive_knowledge_per_minute);
        s.study_millis_per_tick = millis_per_tick(s.study_knowledge_per_minute);
        s.idle_knowledge_cap_millis =
            (s.idle_knowledge_cap * MILLIS_PER_POINT as f64).round() as u64;
        if self.tech.len() > usize::from(u16::MAX) {
            return Err(DefsError(format!("too many techs: {}", self.tech.len())));
        }

        let mut techs: Vec<TechDef> = Vec::with_capacity(self.tech.len());
        let find = |techs: &[TechDef], id: &str| techs.iter().position(|t| t.id == id);
        for raw in self.tech {
            let err = |msg: String| DefsError(format!("tech {}: {msg}", raw.id));
            if find(&techs, &raw.id).is_some() {
                return Err(err("duplicate id".into()));
            }
            if raw.cost == 0 {
                return Err(err("cost must be > 0".into()));
            }
            for (field, m) in raw.unlocks.modifiers.iter() {
                if !(m.is_finite() && m > 0.0 && m <= MAX_MULTIPLIER) {
                    return Err(err(format!(
                        "multiplier {field} = {m} must be finite and in (0, {MAX_MULTIPLIER}]"
                    )));
                }
            }
            let mut prereqs = Vec::with_capacity(raw.prereqs.len());
            for p in &raw.prereqs {
                // Only earlier techs are in `techs`, so a forward reference lands here too.
                let Some(i) = find(&techs, p) else {
                    return Err(err(format!(
                        "unknown prereq {p:?} (prereqs must be defined earlier in the file)"
                    )));
                };
                let id = TechId(i as u16);
                if prereqs.contains(&id) {
                    return Err(err(format!("prereq {p:?} listed twice")));
                }
                let pn = techs[i].nation;
                if pn.is_some() && pn != raw.nation {
                    return Err(err(format!(
                        "a {} tech can't require {p:?}, a {} tech",
                        nation_label(raw.nation),
                        nation_label(pn)
                    )));
                }
                prereqs.push(id);
            }
            techs.push(TechDef {
                id: raw.id,
                name: raw.name,
                description: raw.description,
                nation: raw.nation,
                cost: raw.cost,
                prereqs,
                unlocks: raw.unlocks,
            });
        }
        Ok((s, techs))
    }
}

/// Resolves a nation's `starting_techs`: each must exist and be shared or the nation's own, and
/// every prereq of a starting tech must also be a starting tech.
pub(crate) fn resolve_starting_techs(
    nation: Nation,
    ids: &[String],
    techs: &[TechDef],
) -> Result<Vec<TechId>, DefsError> {
    let err = |msg: String| DefsError(format!("nation {}: starting_techs: {msg}", nation.as_str()));
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(i) = techs.iter().position(|t| t.id == *id) else {
            return Err(err(format!("unknown tech {id:?}")));
        };
        let tech = TechId(i as u16);
        if out.contains(&tech) {
            return Err(err(format!("{id:?} listed twice")));
        }
        if let Some(n) = techs[i].nation
            && n != nation
        {
            return Err(err(format!("{id:?} belongs to {}", n.as_str())));
        }
        out.push(tech);
    }
    for &t in &out {
        let def = &techs[t.0 as usize];
        if let Some(p) = def.prereqs.iter().find(|p| !out.contains(p)) {
            return Err(err(format!(
                "{:?} requires {:?}, which must also be a starting tech",
                def.id, techs[p.0 as usize].id
            )));
        }
    }
    Ok(out)
}

/// A tech finishing, as recorded in the research log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TechCompleted {
    pub tech: TechId,
    /// The world tick on which the tech completed.
    pub tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchError {
    /// No tech with this id exists.
    UnknownTech,
    /// The tech belongs to another nation's branch.
    WrongNation,
    AlreadyKnown,
}

impl std::fmt::Display for ResearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnknownTech => "no such tech",
            Self::WrongNation => "that tech belongs to another nation",
            Self::AlreadyKnown => "that tech is already known",
        })
    }
}

impl std::error::Error for ResearchError {}

/// A settlement's research state. Knowledge is one pool shared by the whole settlement; the
/// queue is spent from it in order, and leftover knowledge carries over.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Research {
    /// Indexed by `TechId`.
    known: Vec<bool>,
    pool_millis: u64,
    /// The next tech to finish is at the front. Always ascending by `TechId`, so every tech's
    /// unknown prereqs are ahead of it.
    queue: Vec<TechId>,
    /// Completions in order. Starting techs are not logged.
    log: Vec<TechCompleted>,
}

impl Research {
    /// Seeds the known set from the nation's `starting_techs`, with an empty pool and queue.
    pub fn new(nation: Nation, defs: &Defs) -> Self {
        let mut known = vec![false; defs.techs.len()];
        for t in &defs.nation(nation).starting_techs {
            known[t.0 as usize] = true;
        }
        Self { known, pool_millis: 0, queue: Vec::new(), log: Vec::new() }
    }

    /// Does this state cover exactly `defs`' techs, with a queue that names only real ones? False
    /// for a state read from a save made against other data.
    pub(crate) fn fits(&self, defs: &Defs) -> bool {
        self.known.len() == defs.techs.len()
            && self.queue.iter().all(|t| (t.0 as usize) < defs.techs.len())
            && self.log.iter().all(|c| (c.tech.0 as usize) < defs.techs.len())
    }

    /// Rebuilds the positional state of a research state read from a save for today's tech
    /// table (#602): `known` is indexed by the save's own tech table, so it is remapped through
    /// `map` (today's index of each saved index, `None` where the tech is gone) to `defs.techs`.
    /// The queue's ids were already remapped as they were read; it is re-sorted, because its
    /// invariant is ascending `TechId` and file order keeps prereqs first. Fails with the saved
    /// index of a known tech today's data doesn't have, or `None` if `known` doesn't match the
    /// save's table at all.
    pub(crate) fn remap(
        &mut self,
        map: &[Option<usize>],
        defs: &Defs,
    ) -> Result<(), Option<usize>> {
        if self.known.len() != map.len() {
            return Err(None);
        }
        let mut known = vec![false; defs.techs.len()];
        for (saved, _) in self.known.iter().enumerate().filter(|(_, k)| **k) {
            known[map[saved].ok_or(Some(saved))?] = true;
        }
        self.known = known;
        self.queue.sort_unstable();
        Ok(())
    }

    pub fn is_known(&self, tech: TechId) -> bool {
        self.known.get(tech.0 as usize).copied().unwrap_or(false)
    }

    /// Known techs in `TechId` order.
    pub fn known_techs(&self) -> impl Iterator<Item = TechId> + '_ {
        self.known.iter().enumerate().filter(|(_, k)| **k).map(|(i, _)| TechId(i as u16))
    }

    /// Banked knowledge in milli-points (1000 per point).
    pub fn pool_millis(&self) -> u64 {
        self.pool_millis
    }

    /// The research queue, front first.
    pub fn queue(&self) -> &[TechId] {
        &self.queue
    }

    /// Every completion so far, oldest first, with the tick it happened on.
    pub fn completions(&self) -> &[TechCompleted] {
        &self.log
    }

    /// Replaces the queue with `tech` plus its unknown prereqs, in ascending `TechId` order.
    /// Knowledge already banked is kept.
    pub fn set_target(
        &mut self,
        tech: TechId,
        nation: Nation,
        defs: &Defs,
    ) -> Result<(), ResearchError> {
        let def = defs.techs.get(tech.0 as usize).ok_or(ResearchError::UnknownTech)?;
        if def.nation.is_some_and(|n| n != nation) {
            return Err(ResearchError::WrongNation);
        }
        if self.is_known(tech) {
            return Err(ResearchError::AlreadyKnown);
        }
        // Prereqs always have smaller ids, so one descending pass finds the whole closure.
        let mut needed = vec![false; tech.0 as usize + 1];
        needed[tech.0 as usize] = true;
        for i in (0..=tech.0 as usize).rev() {
            if needed[i] && !self.known[i] {
                for p in &defs.techs[i].prereqs {
                    needed[p.0 as usize] = true;
                }
            }
        }
        self.queue = (0..needed.len())
            .filter(|&i| needed[i] && !self.known[i])
            .map(|i| TechId(i as u16))
            .collect();
        Ok(())
    }

    /// Adds knowledge straight to the pool, outside the passive trickle `tick` adds on its own:
    /// an ancient scroll, read and used up at a study hall (`world/work.rs`, #24). A one-off
    /// reward, so unlike `add_toward_target` it isn't capped. Techs it can now afford complete
    /// on the next `tick`, not immediately, so knowledge earned partway through a tick can't
    /// retroactively unlock something earlier in that same tick.
    pub fn add_knowledge(&mut self, millis: u64) {
        self.pool_millis += millis;
    }

    /// The most the pool may hold right now: `research_cost` times the cost of the tech at the
    /// front of the queue (the one it is currently paying toward), or the unscaled
    /// `idle_knowledge_cap_millis` with nothing queued (a flat buffer, not a cost, so the
    /// difficulty preset's `research_cost` knob doesn't touch it). Used by `add_toward_target` so
    /// a steady, no-effort source can't bank ahead of what is actually being worked toward
    /// (#284).
    fn cap_millis(&self, defs: &Defs, research_cost: f32) -> u64 {
        match self.queue.first() {
            Some(&front) => scaled_cost_millis(defs.tech(front).cost, research_cost),
            None => defs.research.idle_knowledge_cap_millis,
        }
    }

    /// Adds knowledge from a source that runs on its own with no cost to the player: the passive
    /// "oral tradition" trickle (`tick`) and a seated scholar (`world/study.rs`, #21). Clamped at
    /// `cap_millis` so hours of it can't quietly bank toward several techs at once, the bug
    /// #284 fixed (an ancient scroll, `add_knowledge`, is a deliberate one-off reward and isn't
    /// capped this way). `research_cost` is the difficulty preset's knob (1.0 is identity, #294).
    pub(crate) fn add_toward_target(&mut self, millis: u64, defs: &Defs, research_cost: f32) {
        self.pool_millis = (self.pool_millis + millis).min(self.cap_millis(defs, research_cost));
    }

    /// Marks `tech` known at once, skipping the queue and its cost. For tests that need a nation
    /// to have a tech without waiting out the research.
    #[cfg(test)]
    pub(crate) fn grant(&mut self, tech: TechId) {
        self.known[tech.0 as usize] = true;
    }

    /// Adds knowledge straight to the pool with no cap, for tests that want to skip ahead of
    /// what natural production could bank (`add_toward_target`'s cap is what #284 is about, not
    /// these tests).
    #[cfg(test)]
    pub(crate) fn bank_for_test(&mut self, millis: u64) {
        self.pool_millis += millis;
    }

    /// Adds one tick of passive knowledge, then completes queued techs the pool can pay for.
    /// `tick` is the world tick being simulated, recorded on completions. `research_cost` is the
    /// difficulty preset's knob, multiplying every tech's cost (1.0 is identity, #294).
    pub fn tick(&mut self, tick: u64, defs: &Defs, research_cost: f32) {
        self.add_toward_target(defs.research.passive_millis_per_tick, defs, research_cost);
        while let Some(&front) = self.queue.first() {
            let cost = scaled_cost_millis(defs.tech(front).cost, research_cost);
            if cost > self.pool_millis {
                break;
            }
            self.pool_millis -= cost;
            self.queue.remove(0);
            self.known[front.0 as usize] = true;
            self.log.push(TechCompleted { tech: front, tick });
        }
    }
}

/// `cost` points in milli-points, times the difficulty preset's `research_cost` knob (1.0 is
/// identity). `f64` only to multiply by a non-integer factor; exact for the small integer costs
/// `data/research.toml` validates, so `research_cost == 1.0` reproduces the unscaled value
/// bit-for-bit.
fn scaled_cost_millis(cost: u32, research_cost: f32) -> u64 {
    (f64::from(cost) * f64::from(research_cost) * MILLIS_PER_POINT as f64).round() as u64
}

#[cfg(test)]
mod tests {
    use crate::defs::{Defs, DefsError};
    use crate::nation::Nation;

    use super::TechId;

    const BIOMES: &str = include_str!("../../../data/biomes.toml");
    const RESOURCES: &str = include_str!("../../../data/resources.toml");
    const NEEDS: &str = include_str!("../../../data/needs.toml");
    const JOBS: &str = include_str!("../../../data/jobs.toml");
    const NATIONS: &str = include_str!("../../../data/nations.toml");
    const RESEARCH: &str = include_str!("../../../data/research.toml");
    const BENDING: &str = include_str!("../../../data/bending.toml");
    const BUILDINGS: &str = include_str!("../../../data/buildings.toml");

    const SETTINGS: &str = "[settings]\npassive_knowledge_per_minute = 3.0\n\
         study_knowledge_per_minute = 9.0\nidle_knowledge_cap = 10.0\n";

    /// The bundled starting techs, which every test tree below must define.
    const BASICS: &str = "
        [[tech]]
        id = 'fire_making'
        name = 'F'
        description = ''
        cost = 25
        [[tech]]
        id = 'fishing'
        name = 'F'
        description = ''
        cost = 25
        [[tech]]
        id = 'stoneworking'
        name = 'S'
        description = ''
        cost = 25
        [[tech]]
        id = 'orchards'
        name = 'O'
        description = ''
        nation = 'air'
        cost = 25
    ";

    fn load(research: &str, nations: &str) -> Result<Defs, DefsError> {
        Defs::from_toml(BIOMES, RESOURCES, nations, research, NEEDS, JOBS, BENDING, BUILDINGS)
    }

    /// Loads the bundled nations with `BASICS` plus `techs`.
    fn with_techs(techs: &str) -> Result<Defs, DefsError> {
        load(&format!("{SETTINGS}{BASICS}{techs}"), NATIONS)
    }

    fn tech(id: &str, extra: &str) -> String {
        format!("\n[[tech]]\nid = '{id}'\nname = '{id}'\ndescription = ''\n{extra}\n")
    }

    fn assert_err(r: Result<Defs, DefsError>, needle: &str) {
        let err = r.expect_err("should be rejected");
        assert!(err.0.contains(needle), "expected {needle:?} in: {err}");
    }

    #[test]
    fn bundled_tree_loads() {
        let defs = Defs::bundled();
        assert!(defs.techs.len() >= 11 + 6 * 3 + 7);
        let writing = defs.tech(defs.tech_id("writing").unwrap());
        assert_eq!(writing.nation, None);
        assert_eq!(writing.unlocks.modifiers.study, Some(1.25));
        assert_eq!(writing.prereqs, [defs.tech_id("pottery").unwrap()]);
        let walls = defs.tech(defs.tech_id("earthen_walls").unwrap());
        assert_eq!(walls.nation, Some(Nation::Earth));
        assert_eq!(walls.cost, 120);
        assert!(defs.tech_id("bloodbending").is_none());
        assert_eq!(defs.research.study_knowledge_per_minute, 9.0);
    }

    #[test]
    fn bundled_tree_has_twelve_shared_techs_and_a_branch_per_nation() {
        let defs = Defs::bundled();
        let count = |n| defs.techs.iter().filter(|t| t.nation == n).count();
        // 12, not 11: #383 added the shared `prospecting` tech, gating gold mining the way
        // `stoneworking` already gates stone.
        assert_eq!(count(None), 12);
        for n in [Nation::Earth, Nation::Fire, Nation::Water] {
            assert_eq!(count(Some(n)), 6, "{n:?}");
        }
        // Air's branch also holds its starting tech, `orchards` (owner decision, research.md).
        assert_eq!(count(Some(Nation::Air)), 7);
    }

    #[test]
    fn bundled_starting_techs() {
        let defs = Defs::bundled();
        for (n, id) in [
            (Nation::Fire, "fire_making"),
            (Nation::Water, "fishing"),
            (Nation::Earth, "stoneworking"),
            (Nation::Air, "orchards"),
        ] {
            assert_eq!(defs.nation(n).starting_techs, [defs.tech_id(id).unwrap()], "{n:?}");
        }
    }

    #[test]
    fn every_nation_starts_with_unit_study() {
        let defs = Defs::bundled();
        for n in Nation::ALL {
            assert_eq!(defs.nation(n).modifiers.study, 1.0, "{n:?}");
        }
    }

    #[test]
    fn prereqs_resolve_in_topological_order() {
        let defs =
            with_techs(&tech("a", "cost = 1\nprereqs = ['fire_making', 'fishing']")).unwrap();
        let a = defs.tech_id("a").unwrap();
        let expect = [defs.tech_id("fire_making").unwrap(), defs.tech_id("fishing").unwrap()];
        assert_eq!(defs.tech(a).prereqs, expect);
        assert!(defs.tech(a).prereqs.iter().all(|&p| p < a));
        assert_eq!(defs.tech(TechId(0)).id, "fire_making");
    }

    #[test]
    fn duplicate_id_is_rejected() {
        assert_err(with_techs(&tech("fishing", "cost = 1")), "duplicate id");
    }

    #[test]
    fn unknown_prereq_is_rejected() {
        assert_err(with_techs(&tech("a", "cost = 1\nprereqs = ['alchemy']")), "alchemy");
    }

    #[test]
    fn prereq_after_its_dependant_is_rejected() {
        let src = tech("a", "cost = 1\nprereqs = ['b']") + &tech("b", "cost = 1");
        assert_err(with_techs(&src), "defined earlier");
    }

    #[test]
    fn self_prereq_is_rejected() {
        assert_err(with_techs(&tech("a", "cost = 1\nprereqs = ['a']")), "unknown prereq");
    }

    #[test]
    fn duplicate_prereq_is_rejected() {
        let src = tech("a", "cost = 1\nprereqs = ['fishing', 'fishing']");
        assert_err(with_techs(&src), "twice");
    }

    #[test]
    fn shared_tech_requiring_a_nation_tech_is_rejected() {
        assert_err(with_techs(&tech("a", "cost = 1\nprereqs = ['orchards']")), "shared tech");
    }

    #[test]
    fn nation_tech_requiring_another_nations_tech_is_rejected() {
        let src = tech("a", "nation = 'fire'\ncost = 1\nprereqs = ['orchards']");
        assert_err(with_techs(&src), "air tech");
    }

    #[test]
    fn nation_tech_may_require_shared_and_own_techs() {
        let src = tech("a", "nation = 'air'\ncost = 1\nprereqs = ['fishing', 'orchards']");
        assert!(with_techs(&src).is_ok());
    }

    #[test]
    fn zero_cost_is_rejected() {
        assert_err(with_techs(&tech("a", "cost = 0")), "cost must be > 0");
    }

    #[test]
    fn effective_modifiers_ignore_the_order_techs_are_listed_in() {
        use super::{effective_modifiers, modifier_breakdown};
        let defs = Defs::bundled();
        let id = |s: &str| defs.tech_id(s).unwrap();
        // Three techs that all multiply `construction`.
        let known = ["carpentry", "masonry", "earthen_walls"].map(id);
        let bits = |k: &[TechId]| {
            let m = effective_modifiers(Nation::Earth, k.iter().copied(), &defs);
            m.construction.to_bits()
        };
        let want = bits(&known);
        let mut k = known;
        for _ in 0..known.len() {
            k.rotate_left(1);
            assert_eq!(bits(&k), want);
            k.reverse();
            assert_eq!(bits(&k), want);
        }
        // The breakdown multiplies out (left to right) to the effective value.
        let parts = modifier_breakdown("construction", Nation::Earth, known, &defs);
        let names: Vec<_> = parts.iter().map(|p| p.0).collect();
        assert_eq!(names, ["Earth Kingdom", "Carpentry", "Masonry", "Earthen Walls"]);
        assert_eq!(parts.iter().fold(1.0f32, |acc, p| acc * p.1).to_bits(), want);
    }

    #[test]
    fn bad_multipliers_are_rejected() {
        for m in ["0.0", "-1.0", "4.01", "inf", "nan"] {
            let src =
                tech("a", &format!("cost = 1\n[tech.unlocks]\nmodifiers = {{ heating = {m} }}"));
            assert_err(with_techs(&src), "multiplier heating");
        }
        let ok = tech("a", "cost = 1\n[tech.unlocks]\nmodifiers = { heating = 4.0, study = 0.5 }");
        assert!(with_techs(&ok).is_ok());
    }

    #[test]
    fn unknown_modifier_name_is_rejected() {
        let src = tech("a", "cost = 1\n[tech.unlocks]\nmodifiers = { contstruction = 1.2 }");
        assert_err(with_techs(&src), "contstruction");
    }

    #[test]
    fn unknown_tech_fields_are_rejected() {
        assert_err(with_techs(&tech("a", "cost = 1\ncots = 2")), "cots");
    }

    #[test]
    fn bad_settings_are_rejected() {
        let src = RESEARCH
            .replace("passive_knowledge_per_minute = 3.0", "passive_knowledge_per_minute = -1.0");
        assert_ne!(src, RESEARCH);
        assert_err(load(&src, NATIONS), "passive_knowledge_per_minute");
    }

    fn with_air_start(ids: &str) -> Result<Defs, DefsError> {
        let nations =
            NATIONS.replace("starting_techs = [\"orchards\"]", &format!("starting_techs = {ids}"));
        assert_ne!(nations, NATIONS);
        let extra = tech("gliders", "nation = 'air'\ncost = 1\nprereqs = ['orchards']")
            + &tech("forging", "nation = 'fire'\ncost = 1");
        load(&format!("{SETTINGS}{BASICS}{extra}"), &nations)
    }

    #[test]
    fn starting_techs_may_be_shared_or_own() {
        let defs = with_air_start("['orchards', 'gliders', 'fishing']").unwrap();
        assert_eq!(defs.nation(Nation::Air).starting_techs.len(), 3);
    }

    #[test]
    fn starting_tech_of_another_nation_is_rejected() {
        assert_err(with_air_start("['orchards', 'forging']"), "belongs to fire");
    }

    #[test]
    fn starting_techs_must_be_closed_under_prereqs() {
        assert_err(with_air_start("['gliders']"), "must also be a starting tech");
    }

    #[test]
    fn unknown_starting_tech_is_rejected() {
        assert_err(with_air_start("['levitation']"), "levitation");
    }
}

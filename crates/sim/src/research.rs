//! The tech tree: definitions from `data/research.toml`, resolved and validated at load.
//!
//! See `docs/design/research.md`. Techs are referenced by `TechId`, their index in the file.
//! The file must list prereqs before the techs that need them, so `TechId` order is a valid
//! topological order and cycles are impossible by construction. Later code relies on that: it can
//! walk techs in id order and know every prereq has already been visited.
//!
//! This module only holds static definitions. Research progress lives elsewhere (#19), as does
//! applying multipliers to a nation's modifiers (#20).

use serde::Deserialize;

use crate::defs::DefsError;
use crate::nation::{ModifierMultipliers, Nation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TechId(pub u16);

/// Largest multiplier a single tech may apply. Keeps a typo like `12` instead of `1.2` from
/// loading.
pub const MAX_MULTIPLIER: f32 = 4.0;

/// Knowledge production rates. Converted to integer millis per tick by the research system.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchSettings {
    /// Settlement-wide trickle that bootstraps the first techs.
    pub passive_knowledge_per_minute: f64,
    /// Per scholar at a study hall, before the `study` modifier.
    pub study_knowledge_per_minute: f64,
}

/// What learning a tech makes available. Only modifier multipliers exist for now; buildings, jobs
/// and abilities are added (with their validation) by the issues that introduce those defs, so
/// unknown keys are rejected until then.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unlocks {
    #[serde(default)]
    pub modifiers: ModifierMultipliers,
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
        let s = self.settings;
        for (what, v) in [
            ("passive_knowledge_per_minute", s.passive_knowledge_per_minute),
            ("study_knowledge_per_minute", s.study_knowledge_per_minute),
        ] {
            if !(v.is_finite() && v >= 0.0) {
                return Err(DefsError(format!(
                    "research settings: {what} must be finite and >= 0"
                )));
            }
        }
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

#[cfg(test)]
mod tests {
    use crate::defs::{Defs, DefsError};
    use crate::nation::Nation;

    use super::TechId;

    const BIOMES: &str = include_str!("../../../data/biomes.toml");
    const RESOURCES: &str = include_str!("../../../data/resources.toml");
    const NATIONS: &str = include_str!("../../../data/nations.toml");
    const RESEARCH: &str = include_str!("../../../data/research.toml");

    const SETTINGS: &str =
        "[settings]\npassive_knowledge_per_minute = 3.0\nstudy_knowledge_per_minute = 9.0\n";

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
        Defs::from_toml(BIOMES, RESOURCES, nations, research)
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
    fn bundled_tree_has_eleven_shared_techs_and_a_branch_per_nation() {
        let defs = Defs::bundled();
        let count = |n| defs.techs.iter().filter(|t| t.nation == n).count();
        assert_eq!(count(None), 11);
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
    fn unlock_kinds_without_defs_are_rejected() {
        let src = tech("a", "cost = 1\n[tech.unlocks]\nbuildings = ['campfire']");
        assert_err(with_techs(&src), "buildings");
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

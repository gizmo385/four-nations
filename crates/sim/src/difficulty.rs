//! Difficulty presets (`data/difficulty.toml`, `docs/design/difficulty.md`, issue #294): the
//! knobs that scale pressure (seasons, tiring land, need decay, regrow, research cost and
//! newcomer food) and the stores `World::found` lays down.
//!
//! `World::difficulty` holds the chosen preset's id, or `None`. `World::knobs` resolves it to a
//! `Knobs`: `None` (every save from before this existed, and `World::new`) resolves to
//! `Knobs::identity()`, today's rules exactly, so nothing retunes by itself. Seasons and strain
//! (`seasons`, `winter_severity`, `strain`, `strain_kills`) are parsed and validated here but not
//! yet read by anything; the issues that add them (#295, #296) are the ones that act on them.

use serde::Deserialize;

use crate::defs::DefsError;
use crate::jobs::{ItemId, Stack};

/// The knobs one difficulty preset turns (`docs/design/difficulty.md`, "Difficulty presets").
#[derive(Debug, Clone, PartialEq)]
pub struct Knobs {
    pub id: String,
    pub name: String,
    pub hint: String,
    pub seasons: bool,
    pub winter_severity: f32,
    pub strain: f32,
    pub strain_kills: bool,
    /// Multiplies hunger, thirst and energy decay (`Needs::tick_at_full`).
    pub need_decay: f32,
    /// Multiplies every renewable harvest's regrow time (`World::work_done`).
    pub regrow: f32,
    /// Multiplies every tech's cost, including the knowledge pool's cap on what's being
    /// researched (`Research::tick`, `Research::add_toward_target`; #284).
    pub research_cost: f32,
    /// Food items a newcomer's arrival takes from stores, on top of #232's
    /// `food_per_newcomer` threshold (`World::tick_population`).
    pub newcomer_food: u32,
    /// What `World::found` stockpiles near the start, one kind of item per tile
    /// (`defs.jobs.stockpile_stack` per tile).
    pub starting_stores: Vec<Stack>,
}

impl Knobs {
    /// The identity knobs: today's rules exactly, with no seasons, strain, stores or newcomer
    /// food. What `None` resolves to.
    pub fn identity() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            hint: String::new(),
            seasons: false,
            winter_severity: 1.0,
            strain: 0.0,
            strain_kills: false,
            need_decay: 1.0,
            regrow: 1.0,
            research_cost: 1.0,
            newcomer_food: 0,
            starting_stores: Vec::new(),
        }
    }
}

/// Everything in `data/difficulty.toml`, resolved and validated. Has no presets and an empty
/// `default` unless loaded with `Defs::with_difficulty`, which `Defs::bundled` does; `identity`
/// is always available either way, since it depends on no data.
#[derive(Debug, Clone, PartialEq)]
pub struct DifficultyDef {
    pub default: String,
    /// In file order: the New game seed step's picker order.
    pub presets: Vec<Knobs>,
    /// What `World::knobs` returns for `None`, or for a preset id that the save's data no longer
    /// has (which `World::validate` already refuses before this is ever asked).
    pub identity: Knobs,
}

impl Default for DifficultyDef {
    fn default() -> Self {
        Self { default: String::new(), presets: Vec::new(), identity: Knobs::identity() }
    }
}

impl DifficultyDef {
    /// The preset with this id, if any.
    pub fn preset(&self, id: &str) -> Option<&Knobs> {
        self.presets.iter().find(|p| p.id == id)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DifficultyFile {
    default: String,
    #[serde(rename = "preset")]
    presets: Vec<RawPreset>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPreset {
    id: String,
    name: String,
    hint: String,
    seasons: bool,
    winter_severity: f32,
    strain: f32,
    strain_kills: bool,
    need_decay: f32,
    regrow: f32,
    research_cost: f32,
    newcomer_food: u32,
    #[serde(default)]
    starting_stores: Vec<RawStore>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStore {
    item: String,
    count: u32,
}

impl DifficultyFile {
    fn resolve(self, item_ids: &[&str]) -> Result<DifficultyDef, DefsError> {
        let err = |msg: String| Err(DefsError(format!("difficulty.toml: {msg}")));
        if self.presets.is_empty() {
            return err("need at least one [[preset]]".into());
        }
        let mut presets = Vec::with_capacity(self.presets.len());
        for raw in self.presets {
            if presets.iter().any(|p: &Knobs| p.id == raw.id) {
                return err(format!("preset {}: duplicate id", raw.id));
            }
            for (what, v, allow_zero) in [
                ("winter_severity", raw.winter_severity, true),
                ("strain", raw.strain, true),
                ("need_decay", raw.need_decay, false),
                ("regrow", raw.regrow, false),
                ("research_cost", raw.research_cost, false),
            ] {
                let ok = if allow_zero { (0.0..=4.0).contains(&v) } else { v > 0.0 && v <= 4.0 };
                if !ok {
                    let range = if allow_zero { "[0, 4]" } else { "(0, 4]" };
                    return err(format!("preset {}: {what} must be in {range}", raw.id));
                }
            }
            let mut starting_stores = Vec::with_capacity(raw.starting_stores.len());
            for store in &raw.starting_stores {
                let Some(idx) = item_ids.iter().position(|i| *i == store.item.as_str()) else {
                    return err(format!(
                        "preset {}: unknown item {:?} in starting_stores",
                        raw.id, store.item
                    ));
                };
                let item = ItemId(idx as u16);
                if starting_stores.iter().any(|s: &Stack| s.item == item) {
                    return err(format!(
                        "preset {}: duplicate starting_stores item {:?}",
                        raw.id, store.item
                    ));
                }
                starting_stores.push(Stack { item, count: store.count });
            }
            presets.push(Knobs {
                id: raw.id,
                name: raw.name,
                hint: raw.hint,
                seasons: raw.seasons,
                winter_severity: raw.winter_severity,
                strain: raw.strain,
                strain_kills: raw.strain_kills,
                need_decay: raw.need_decay,
                regrow: raw.regrow,
                research_cost: raw.research_cost,
                newcomer_food: raw.newcomer_food,
                starting_stores,
            });
        }
        if !presets.iter().any(|p| p.id == self.default) {
            return err(format!("default {:?} is not a known preset", self.default));
        }
        Ok(DifficultyDef { default: self.default, presets, identity: Knobs::identity() })
    }
}

impl crate::defs::Defs {
    /// These definitions with the difficulty presets read from `difficulty`
    /// (`data/difficulty.toml`), replacing any they had. A separate step from `from_toml`, as
    /// `with_embark` is, so data without one (most tests) simply has no presets, and `knobs`
    /// still resolves `None` to `Knobs::identity()`.
    pub fn with_difficulty(mut self, difficulty: &str) -> Result<Self, DefsError> {
        let item_ids: Vec<&str> = self.items.iter().map(|i| i.id.as_str()).collect();
        self.difficulty = toml::from_str::<DifficultyFile>(difficulty)
            .map_err(|e| DefsError(format!("difficulty.toml: {e}")))?
            .resolve(&item_ids)?;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::Defs;

    const DIFFICULTY: &str = include_str!("../../../data/difficulty.toml");

    fn defs_without_difficulty() -> Defs {
        Defs::from_toml(
            include_str!("../../../data/biomes.toml"),
            include_str!("../../../data/resources.toml"),
            include_str!("../../../data/nations.toml"),
            include_str!("../../../data/research.toml"),
            include_str!("../../../data/needs.toml"),
            include_str!("../../../data/jobs.toml"),
            include_str!("../../../data/bending.toml"),
            include_str!("../../../data/buildings.toml"),
        )
        .unwrap()
    }

    #[test]
    fn unloaded_defs_have_no_presets_but_identity_still_resolves() {
        let defs = defs_without_difficulty();
        assert!(defs.difficulty.presets.is_empty());
        assert_eq!(defs.difficulty.preset("normal"), None);
        assert_eq!(defs.difficulty.identity, Knobs::identity());
    }

    #[test]
    fn bundled_difficulty_data_is_valid() {
        let defs = Defs::bundled();
        assert_eq!(defs.difficulty.default, "normal");
        for id in ["sandbox", "normal", "hard"] {
            assert!(defs.difficulty.preset(id).is_some(), "{id} missing");
        }
        let normal = defs.difficulty.preset("normal").unwrap();
        assert!(normal.seasons);
        assert_eq!(normal.newcomer_food, 10);
        assert!(!normal.starting_stores.is_empty());
        let hard = defs.difficulty.preset("hard").unwrap();
        assert!(hard.strain_kills);
        assert!(hard.starting_stores.is_empty());
    }

    #[test]
    fn unknown_default_is_rejected() {
        let bad = DIFFICULTY.replacen("default = \"normal\"", "default = \"extreme\"", 1);
        let err = defs_without_difficulty().with_difficulty(&bad).unwrap_err();
        assert!(err.0.contains("extreme"), "{err}");
    }

    #[test]
    fn duplicate_preset_ids_are_rejected() {
        let dup = format!(
            "{DIFFICULTY}\n[[preset]]\nid = \"normal\"\nname = \"N2\"\nhint = \"\"\n\
             seasons = false\nwinter_severity = 1.0\nstrain = 0.0\nstrain_kills = false\n\
             need_decay = 1.0\nregrow = 1.0\nresearch_cost = 1.0\nnewcomer_food = 0\n"
        );
        let err = defs_without_difficulty().with_difficulty(&dup).unwrap_err();
        assert!(err.0.contains("duplicate"), "{err}");
    }

    #[test]
    fn multiplier_out_of_range_is_rejected() {
        let bad = DIFFICULTY.replacen("need_decay = 1.1", "need_decay = 5.0", 1);
        let err = defs_without_difficulty().with_difficulty(&bad).unwrap_err();
        assert!(err.0.contains("need_decay"), "{err}");
    }

    #[test]
    fn strain_and_winter_severity_may_be_zero_but_need_decay_may_not() {
        let defs = defs_without_difficulty();
        let zero_strain = DIFFICULTY.replacen("strain = 1.0", "strain = 0.0", 1);
        assert!(defs.clone().with_difficulty(&zero_strain).is_ok());
        let zero_decay = DIFFICULTY.replacen("need_decay = 1.0", "need_decay = 0.0", 1);
        assert!(defs_without_difficulty().with_difficulty(&zero_decay).is_err());
    }

    #[test]
    fn unknown_item_in_starting_stores_is_rejected() {
        let bad = DIFFICULTY.replacen("item = \"fruit\"", "item = \"fruitcake\"", 1);
        let err = defs_without_difficulty().with_difficulty(&bad).unwrap_err();
        assert!(err.0.contains("fruitcake"), "{err}");
    }
}

//! The end-game endeavor's signs: definitions from `data/endeavor.toml`, resolved and validated at
//! load (`docs/design/endgame.md`, "The road: ten signs"; issue #234).
//!
//! A sign is a condition on things the settlement does anyway: how many people it has, what it
//! has built and stored, lifetime tallies. The endeavor becomes Ready the first time every sign is
//! met at once (`world/endeavor.rs` holds that live state). Sign kinds are a closed enum, so a sign
//! the sim can't count can't be written: a sign whose system doesn't exist yet stays out of the
//! data until it does, the way `research_earth.rs` treats unlocks.
//!
//! Signs are referenced by their string id in saves (`World::endeavor_signs` and the events), so
//! reordering or retuning the list doesn't misread an old save.

use serde::Deserialize;

use crate::defs::DefsError;
use crate::jobs::{ItemDef, ItemId};
use crate::nation::Nation;

/// The endeavor and its signs. Empty (no signs) when the data has no endeavor, as in
/// `Defs::from_toml` without `Defs::with_endeavor`: then the endeavor never becomes Ready.
#[derive(Debug, Clone, Default)]
pub struct EndeavorDef {
    /// What the endeavor is called in the UI ("The Harmony Gate").
    pub name: String,
    /// The signs, in file order: the order the UI lists them in.
    pub signs: Vec<SignDef>,
}

/// Where a sign is listed. The UI groups signs under these, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignGroup {
    People,
    Buildings,
    Knowledge,
    Stores,
    Harvests,
    Bending,
}

/// What a sign counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignKind {
    /// Living units.
    Population,
    /// Beds, against the population: met when there is a bed for everyone. Its target is the
    /// population, so it has none in the data.
    BedsForAll,
    /// Distinct kinds of finished building.
    BuildingKinds,
    /// Techs of the settlement's own nation known, against all of them. Its target is the size of
    /// the branch, so it has none in the data.
    BranchComplete,
    /// These items in stockpiles, summed.
    Stored { items: Vec<ItemId> },
    /// A lifetime tally (`World::tallies`).
    Tally(Tally),
    /// Units with at least `bends` finished bends.
    Masters { bends: u32 },
}

/// Which lifetime tally a `SignKind::Tally` reads. Crops harvested from farm plots joins this
/// list with the farm plots themselves (#198).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tally {
    /// Items harvested, summed over these items.
    Harvested(Vec<ItemId>),
    ScrollsRead,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignDef {
    pub id: String,
    pub group: SignGroup,
    pub name: String,
    pub kind: SignKind,
    /// What the UI suggests while the sign isn't met ("Spare beds and food draw newcomers").
    pub hint: Option<String>,
    /// The shared target, 0 for the kinds whose target the sim works out (`target_for`).
    target: u32,
    /// Per-nation overrides of `target`, indexed like `Nation::ALL`.
    per_nation: [Option<u32>; 4],
}

impl SignDef {
    /// The target `nation` must reach, or `None` for a kind whose target isn't in the data
    /// (`BedsForAll`, `BranchComplete`).
    pub fn target_for(&self, nation: Nation) -> Option<u32> {
        let i = Nation::ALL.iter().position(|&n| n == nation).expect("every nation is in ALL");
        (self.target > 0).then(|| self.per_nation[i].unwrap_or(self.target))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EndeavorFile {
    endeavor: RawEndeavor,
    #[serde(default)]
    sign: Vec<RawSign>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEndeavor {
    name: String,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RawKind {
    Population,
    BedsForAll,
    BuildingKinds,
    BranchComplete,
    Stored,
    Tally,
    Masters,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RawTally {
    Harvested,
    ScrollsRead,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PerNation {
    air: Option<u32>,
    water: Option<u32>,
    earth: Option<u32>,
    fire: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSign {
    id: String,
    group: SignGroup,
    name: String,
    kind: RawKind,
    #[serde(default)]
    target: Option<u32>,
    #[serde(default)]
    per_nation: Option<PerNation>,
    #[serde(default)]
    hint: Option<String>,
    /// `stored` and `tally = "harvested"`: item ids.
    #[serde(default)]
    items: Option<Vec<String>>,
    /// `stored`: also count every edible item.
    #[serde(default)]
    food: Option<bool>,
    /// `tally`: which tally.
    #[serde(default)]
    tally: Option<RawTally>,
    /// `masters`: the finished bends that make a master.
    #[serde(default)]
    bends: Option<u32>,
}

impl EndeavorFile {
    /// Validates the file and resolves item ids. `items` is `Defs::items`.
    pub(crate) fn resolve(self, items: &[ItemDef]) -> Result<EndeavorDef, DefsError> {
        if self.endeavor.name.trim().is_empty() {
            return Err(DefsError("endeavor: name must not be empty".into()));
        }
        let mut signs: Vec<SignDef> = Vec::with_capacity(self.sign.len());
        for raw in self.sign {
            let sign = raw.resolve(items)?;
            if signs.iter().any(|s| s.id == sign.id) {
                return Err(DefsError(format!("sign {}: duplicate id", sign.id)));
            }
            signs.push(sign);
        }
        Ok(EndeavorDef { name: self.endeavor.name, signs })
    }
}

impl RawSign {
    fn resolve(self, items: &[ItemDef]) -> Result<SignDef, DefsError> {
        let id = self.id;
        let err = |msg: &str| DefsError(format!("sign {id}: {msg}"));
        if id.trim().is_empty() {
            return Err(DefsError("sign: id must not be empty".into()));
        }
        if self.name.trim().is_empty() {
            return Err(err("name must not be empty"));
        }
        // Each optional field belongs to some kinds only; anywhere else it is a mistake.
        let allowed = |field: bool, ok: bool, name: &str| {
            if field && !ok {
                Err(err(&format!("{name} doesn't apply to this kind")))
            } else {
                Ok(())
            }
        };
        let k = self.kind;
        let computed = matches!(k, RawKind::BedsForAll | RawKind::BranchComplete);
        allowed(self.target.is_some(), !computed, "target")?;
        allowed(self.per_nation.is_some(), !computed, "per_nation")?;
        allowed(self.food.is_some(), k == RawKind::Stored, "food")?;
        allowed(self.tally.is_some(), k == RawKind::Tally, "tally")?;
        allowed(self.bends.is_some(), k == RawKind::Masters, "bends")?;
        let takes_items = k == RawKind::Stored
            || (k == RawKind::Tally && self.tally == Some(RawTally::Harvested));
        allowed(self.items.is_some(), takes_items, "items")?;

        let target = match self.target {
            Some(0) => return Err(err("target must be at least 1")),
            Some(t) => t,
            None if computed => 0,
            None => return Err(err("needs a target")),
        };
        let p = self.per_nation.unwrap_or_default();
        let per_nation = [p.air, p.water, p.earth, p.fire];
        if per_nation.contains(&Some(0)) {
            return Err(err("per_nation targets must be at least 1"));
        }

        let resolve_items = |ids: &[String]| -> Result<Vec<ItemId>, DefsError> {
            ids.iter()
                .map(|i| {
                    items
                        .iter()
                        .position(|d| d.id == *i)
                        .map(|n| ItemId(n as u16))
                        .ok_or_else(|| err(&format!("unknown item {i:?}")))
                })
                .collect()
        };
        let mut listed = resolve_items(self.items.as_deref().unwrap_or_default())?;
        let kind = match k {
            RawKind::Population => SignKind::Population,
            RawKind::BedsForAll => SignKind::BedsForAll,
            RawKind::BuildingKinds => SignKind::BuildingKinds,
            RawKind::BranchComplete => SignKind::BranchComplete,
            RawKind::Stored => {
                if self.food == Some(true) {
                    let food = items.iter().enumerate().filter(|(_, d)| d.food.is_some());
                    listed.extend(food.map(|(n, _)| ItemId(n as u16)));
                }
                listed.sort();
                listed.dedup();
                if listed.is_empty() {
                    return Err(err("stored needs items or food = true"));
                }
                SignKind::Stored { items: listed }
            }
            RawKind::Tally => match self.tally {
                None => return Err(err("tally needs tally = \"harvested\" or \"scrolls_read\"")),
                Some(RawTally::ScrollsRead) => SignKind::Tally(Tally::ScrollsRead),
                Some(RawTally::Harvested) => {
                    listed.sort();
                    listed.dedup();
                    if listed.is_empty() {
                        return Err(err("a harvested tally needs items"));
                    }
                    SignKind::Tally(Tally::Harvested(listed))
                }
            },
            RawKind::Masters => match self.bends {
                Some(b) if b >= 1 => SignKind::Masters { bends: b },
                _ => return Err(err("masters needs bends >= 1")),
            },
        };
        Ok(SignDef {
            group: self.group,
            name: self.name,
            kind,
            hint: self.hint.filter(|h| !h.trim().is_empty()),
            target,
            per_nation,
            id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::Defs;

    const HEADER: &str = "[endeavor]\nname = \"The Harmony Gate\"\n";

    /// The bundled data with `endeavor.toml` replaced by `src`.
    fn load_file(src: &str) -> Result<EndeavorDef, DefsError> {
        Ok(Defs::bundled().with_endeavor(src)?.endeavor)
    }

    /// The bundled data with these signs as the whole list.
    fn load(signs: &str) -> Result<EndeavorDef, DefsError> {
        load_file(&format!("{HEADER}{signs}"))
    }

    fn error(signs: &str) -> String {
        load(signs).expect_err("should be rejected").0
    }

    fn file_error(src: &str) -> String {
        load_file(src).expect_err("should be rejected").0
    }

    #[test]
    fn the_bundled_endeavor_loads_with_every_kind_it_can_count() {
        let e = Defs::bundled().endeavor;
        assert_eq!(e.name, "The Harmony Gate");
        assert!(!e.signs.is_empty());
        let has = |f: fn(&SignKind) -> bool| e.signs.iter().any(|s| f(&s.kind));
        assert!(has(|k| *k == SignKind::Population));
        assert!(has(|k| *k == SignKind::BedsForAll));
        assert!(has(|k| *k == SignKind::BuildingKinds));
        assert!(has(|k| *k == SignKind::BranchComplete));
        assert!(has(|k| matches!(k, SignKind::Stored { .. })));
        assert!(has(|k| matches!(k, SignKind::Tally(Tally::Harvested(_)))));
        assert!(has(|k| matches!(k, SignKind::Tally(Tally::ScrollsRead))));
        assert!(has(|k| matches!(k, SignKind::Masters { .. })));
    }

    #[test]
    fn a_sign_resolves_items_targets_and_per_nation_overrides() {
        let e = load(
            r#"
            [[sign]]
            id = "stone"
            group = "stores"
            name = "Stone in stores"
            kind = "stored"
            items = ["stone"]
            target = 200
            per_nation = { air = 150 }
            hint = "Quarry stone"

            [[sign]]
            id = "beds"
            group = "people"
            name = "Everyone has a bed"
            kind = "beds_for_all"
            "#,
        )
        .unwrap();
        let defs = Defs::bundled();
        let stone = &e.signs[0];
        assert_eq!(stone.kind, SignKind::Stored { items: vec![defs.item_id("stone").unwrap()] });
        assert_eq!(stone.target_for(Nation::Air), Some(150));
        assert_eq!(stone.target_for(Nation::Earth), Some(200));
        assert_eq!(stone.hint.as_deref(), Some("Quarry stone"));
        assert_eq!(e.signs[1].target_for(Nation::Fire), None);
    }

    #[test]
    fn food_true_stores_every_edible_item() {
        let e = load(
            "[[sign]]\nid = \"f\"\ngroup = \"stores\"\nname = \"F\"\nkind = \"stored\"\n\
             food = true\ntarget = 5\n",
        )
        .unwrap();
        let defs = Defs::bundled();
        let SignKind::Stored { items } = &e.signs[0].kind else { panic!() };
        let food: Vec<ItemId> = (0..defs.items.len())
            .map(|i| ItemId(i as u16))
            .filter(|&i| defs.food_of(i).is_some())
            .collect();
        assert!(!food.is_empty());
        assert_eq!(items, &food);
    }

    #[test]
    fn an_empty_or_missing_list_of_signs_is_allowed() {
        assert!(load("").unwrap().signs.is_empty());
        // Data built without an endeavor file has no signs at all.
        assert!(crate::bending::testing::probe_defs(1.0, "").endeavor.signs.is_empty());
    }

    fn sign(fields: &str) -> String {
        format!("[[sign]]\nid = \"s\"\ngroup = \"people\"\nname = \"S\"\n{fields}\n")
    }

    #[test]
    fn unknown_keys_and_kinds_are_rejected() {
        assert!(error(&sign("kind = \"population\"\ntarget = 3\ncolour = 1")).contains("colour"));
        assert!(error(&sign("kind = \"crops\"\ntarget = 3")).contains("crops"));
        assert!(
            error(&sign("kind = \"tally\"\ntally = \"crops_harvested\"\ntarget = 3"))
                .contains("crops_harvested")
        );
        assert!(file_error("[endeavor]\nname = \"G\"\nextra = 1\n").contains("extra"));
        assert!(file_error("[endeavor]\nname = \"G\"\n[[stage]]\nid = \"x\"\n").contains("stage"));
        assert!(
            error("[[sign]]\nid = \"s\"\ngroup = \"war\"\nname = \"S\"\nkind = \"population\"\ntarget = 1\n")
                .contains("war")
        );
    }

    #[test]
    fn unresolved_item_ids_are_rejected() {
        let e = error(&sign("kind = \"stored\"\nitems = [\"mithril\"]\ntarget = 3"));
        assert!(e.contains("unknown item \"mithril\""), "{e}");
        let e =
            error(&sign("kind = \"tally\"\ntally = \"harvested\"\nitems = [\"x\"]\ntarget = 3"));
        assert!(e.contains("unknown item \"x\""), "{e}");
    }

    #[test]
    fn targets_are_required_positive_and_only_where_the_data_sets_them() {
        assert!(error(&sign("kind = \"population\"")).contains("needs a target"));
        assert!(error(&sign("kind = \"population\"\ntarget = 0")).contains("at least 1"));
        assert!(
            error(&sign("kind = \"beds_for_all\"\ntarget = 4")).contains("target doesn't apply")
        );
        assert!(
            error(&sign("kind = \"branch_complete\"\nper_nation = { air = 1 }"))
                .contains("per_nation")
        );
        let e = error(&sign("kind = \"population\"\ntarget = 4\nper_nation = { fire = 0 }"));
        assert!(e.contains("per_nation targets must be at least 1"), "{e}");
        assert!(
            error(&sign("kind = \"population\"\ntarget = 4\nper_nation = { mars = 2 }"))
                .contains("mars")
        );
    }

    #[test]
    fn kind_specific_fields_are_checked() {
        assert!(error(&sign("kind = \"stored\"\ntarget = 3")).contains("items or food"));
        assert!(
            error(&sign("kind = \"population\"\ntarget = 3\nitems = [\"stone\"]"))
                .contains("items doesn't apply")
        );
        assert!(
            error(&sign("kind = \"population\"\ntarget = 3\nfood = true"))
                .contains("food doesn't apply")
        );
        assert!(error(&sign("kind = \"tally\"\ntarget = 3")).contains("tally needs"));
        assert!(
            error(&sign("kind = \"tally\"\ntally = \"harvested\"\ntarget = 3"))
                .contains("needs items")
        );
        let e = error(&sign(
            "kind = \"tally\"\ntally = \"scrolls_read\"\nitems = [\"stone\"]\ntarget = 3",
        ));
        assert!(e.contains("items doesn't apply"), "{e}");
        assert!(error(&sign("kind = \"masters\"\ntarget = 3")).contains("bends >= 1"));
        assert!(error(&sign("kind = \"masters\"\ntarget = 3\nbends = 0")).contains("bends >= 1"));
        assert!(
            error(&sign("kind = \"population\"\ntarget = 3\nbends = 4"))
                .contains("bends doesn't apply")
        );
        assert!(
            error(&sign("kind = \"population\"\ntarget = 3\ntally = \"scrolls_read\""))
                .contains("tally doesn't apply")
        );
    }

    #[test]
    fn ids_and_names_must_be_present_and_ids_unique() {
        let two = format!(
            "{}{}",
            sign("kind = \"population\"\ntarget = 1"),
            sign("kind = \"population\"\ntarget = 2")
        );
        assert!(error(&two).contains("duplicate id"));
        assert!(error("[[sign]]\nid = \" \"\ngroup = \"people\"\nname = \"S\"\nkind = \"population\"\ntarget = 1\n").contains("id must not be empty"));
        assert!(error("[[sign]]\nid = \"s\"\ngroup = \"people\"\nname = \"\"\nkind = \"population\"\ntarget = 1\n").contains("name must not be empty"));
        assert!(file_error("[endeavor]\nname = \"\"\n").contains("name must not be empty"));
    }
}

//! The four playable civilizations and their gameplay modifiers.

use serde::Deserialize;

use crate::research::TechId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Nation {
    Air,
    Water,
    Earth,
    Fire,
}

impl Nation {
    pub const ALL: [Nation; 4] = [Nation::Air, Nation::Water, Nation::Earth, Nation::Fire];

    pub fn as_str(self) -> &'static str {
        match self {
            Nation::Air => "air",
            Nation::Water => "water",
            Nation::Earth => "earth",
            Nation::Fire => "fire",
        }
    }
}

/// Declares `Modifiers` and `ModifierMultipliers` from one field list, so a new modifier (e.g.
/// `study`) can't be added to one and forgotten in the other.
macro_rules! modifiers {
    ($($field:ident),* $(,)?) => {
        /// Multipliers on a 1.0 baseline. See `data/nations.toml` for what each one covers.
        #[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct Modifiers {
            $(pub $field: f32,)*
        }

        /// Per-modifier multipliers a tech applies on top of a nation's `Modifiers`, e.g.
        /// `modifiers = { construction = 1.1 }` in `data/research.toml`. Unknown names are a
        /// parse error, so `contstruction = 1.1` fails at load instead of silently doing nothing.
        #[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct ModifierMultipliers {
            $(#[serde(default)] pub $field: Option<f32>,)*
        }

        impl Modifiers {
            /// Every modifier name, in field order.
            pub const NAMES: &'static [&'static str] = &[$(stringify!($field)),*];

            /// The modifier called `name`, for data that refers to one by name (e.g. a harvest's
            /// `modifier` in `data/jobs.toml`). `None` if there is no such modifier.
            pub fn get(&self, name: &str) -> Option<f32> {
                match name {
                    $(stringify!($field) => Some(self.$field),)*
                    _ => None,
                }
            }
        }

        impl Modifiers {
            /// Multiplies each modifier `m` sets, leaving the others alone.
            pub fn apply(&mut self, m: &ModifierMultipliers) {
                $(if let Some(f) = m.$field { self.$field *= f; })*
            }
        }

        impl ModifierMultipliers {
            /// The multipliers that are set, as `(field name, multiplier)`, in field order.
            pub fn iter(&self) -> impl Iterator<Item = (&'static str, f32)> {
                [$((stringify!($field), self.$field)),*]
                    .into_iter()
                    .filter_map(|(name, m)| m.map(|m| (name, m)))
            }
        }
    };
}

modifiers! {
    move_speed,
    carry_capacity,
    earthworks,
    heating,
    water_work,
    power_generation,
    construction,
    study,
    chi_regen,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NationDef {
    pub id: Nation,
    pub name: String,
    pub element: String,
    pub color: [u8; 3],
    pub description: String,
    /// Pool of personal names for this nation's units.
    pub names: Vec<String>,
    pub modifiers: Modifiers,
    /// Tech ids as written in `nations.toml`; resolved into `starting_techs` at load.
    #[serde(rename = "starting_techs")]
    pub(crate) starting_tech_ids: Vec<String>,
    /// Techs this nation knows from tick 0. Validated at load: each is shared or this nation's
    /// own, and the set is closed under prereqs.
    #[serde(skip)]
    pub starting_techs: Vec<TechId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipliers_iterate_set_fields_in_order() {
        let m: ModifierMultipliers = toml::from_str("study = 1.25\nmove_speed = 1.5").unwrap();
        assert_eq!(m.iter().collect::<Vec<_>>(), [("move_speed", 1.5), ("study", 1.25)]);
    }

    #[test]
    fn modifiers_can_be_looked_up_by_name() {
        let defs = crate::Defs::bundled();
        let earth = defs.nation(Nation::Earth).modifiers;
        assert_eq!(earth.get("earthworks"), Some(earth.earthworks));
        assert_eq!(earth.get("nonsense"), None);
        assert!(Modifiers::NAMES.iter().all(|n| earth.get(n).is_some()));
    }

    #[test]
    fn apply_multiplies_only_the_modifiers_set() {
        let mut m = crate::Defs::bundled().nation(Nation::Earth).modifiers;
        let before = m;
        m.apply(&toml::from_str("construction = 1.25").unwrap());
        assert_eq!(m.construction, before.construction * 1.25);
        assert_eq!(Modifiers { construction: before.construction, ..m }, before);
    }

    #[test]
    fn unknown_multiplier_names_fail_to_parse() {
        let err = toml::from_str::<ModifierMultipliers>("contstruction = 1.2").unwrap_err();
        assert!(err.to_string().contains("contstruction"), "{err}");
    }
}

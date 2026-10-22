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
    fn unknown_multiplier_names_fail_to_parse() {
        let err = toml::from_str::<ModifierMultipliers>("contstruction = 1.2").unwrap_err();
        assert!(err.to_string().contains("contstruction"), "{err}");
    }
}

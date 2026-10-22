//! The four playable civilizations and their gameplay modifiers.

use serde::Deserialize;

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

/// Multipliers on a 1.0 baseline. See `data/nations.toml` for what each one covers.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Modifiers {
    pub move_speed: f32,
    pub carry_capacity: f32,
    pub earthworks: f32,
    pub heating: f32,
    pub water_work: f32,
    pub power_generation: f32,
    pub construction: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NationDef {
    pub id: Nation,
    pub name: String,
    pub element: String,
    pub color: [u8; 3],
    pub description: String,
    pub modifiers: Modifiers,
}

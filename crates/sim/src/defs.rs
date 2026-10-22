//! Static game definitions loaded from the TOML files in `data/`.
//!
//! Definitions are referenced at runtime by small index newtypes (`BiomeId`, `ResourceId`) so
//! that map cells stay `Copy` and cheap. Validation happens once, at load time.

use serde::Deserialize;

use crate::nation::{Nation, NationDef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Terrain {
    Water,
    Coast,
    Land,
    Mountain,
}

impl Terrain {
    pub fn is_water(self) -> bool {
        self == Terrain::Water
    }

    pub fn is_walkable(self) -> bool {
        matches!(self, Terrain::Coast | Terrain::Land)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BiomeId(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(pub u16);

#[derive(Debug, Clone, Deserialize)]
pub struct BiomeDef {
    pub id: String,
    pub name: String,
    pub glyph: char,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceDef {
    pub id: String,
    pub name: String,
    pub glyph: char,
    /// Probability in `[0, 1]` that this resource spawns on an eligible tile.
    pub chance: f64,
    /// Terrains this may spawn on; `None` means any.
    pub terrains: Option<Vec<Terrain>>,
    /// Biome ids this may spawn in; `None` means any.
    pub biomes: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct Defs {
    pub biomes: Vec<BiomeDef>,
    pub resources: Vec<ResourceDef>,
    pub nations: Vec<NationDef>,
}

#[derive(Debug, PartialEq)]
pub struct DefsError(pub String);

impl std::fmt::Display for DefsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid game data: {}", self.0)
    }
}

impl std::error::Error for DefsError {}

#[derive(Deserialize)]
struct BiomeFile {
    biome: Vec<BiomeDef>,
}

#[derive(Deserialize)]
struct ResourceFile {
    resource: Vec<ResourceDef>,
}

#[derive(Deserialize)]
struct NationFile {
    nation: Vec<NationDef>,
}

fn parse<T: for<'de> Deserialize<'de>>(name: &str, src: &str) -> Result<T, DefsError> {
    toml::from_str(src).map_err(|e| DefsError(format!("{name}: {e}")))
}

impl Defs {
    /// The definitions shipped in `data/`, embedded at compile time.
    pub fn bundled() -> Self {
        Self::from_toml(
            include_str!("../../../data/biomes.toml"),
            include_str!("../../../data/resources.toml"),
            include_str!("../../../data/nations.toml"),
        )
        .expect("bundled data/*.toml must be valid; run `cargo test -p fn_sim`")
    }

    pub fn from_toml(biomes: &str, resources: &str, nations: &str) -> Result<Self, DefsError> {
        let defs = Self {
            biomes: parse::<BiomeFile>("biomes.toml", biomes)?.biome,
            resources: parse::<ResourceFile>("resources.toml", resources)?.resource,
            nations: parse::<NationFile>("nations.toml", nations)?.nation,
        };
        defs.validate()?;
        Ok(defs)
    }

    fn validate(&self) -> Result<(), DefsError> {
        if self.biomes.is_empty() || self.biomes.len() > u8::MAX as usize {
            return Err(DefsError(format!("need 1..=255 biomes, got {}", self.biomes.len())));
        }
        for r in &self.resources {
            if !(0.0..=1.0).contains(&r.chance) {
                return Err(DefsError(format!("resource {}: chance must be in [0, 1]", r.id)));
            }
            for b in r.biomes.iter().flatten() {
                if self.biome_id(b).is_none() {
                    return Err(DefsError(format!("resource {}: unknown biome {b:?}", r.id)));
                }
            }
        }
        for n in Nation::ALL {
            match self.nations.iter().find(|d| d.id == n) {
                None => return Err(DefsError(format!("nations.toml is missing {n:?}"))),
                Some(d) if d.names.is_empty() => {
                    return Err(DefsError(format!("nation {n:?} needs at least one name")));
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    pub fn biome_id(&self, id: &str) -> Option<BiomeId> {
        self.biomes.iter().position(|b| b.id == id).map(|i| BiomeId(i as u8))
    }

    pub fn resource_id(&self, id: &str) -> Option<ResourceId> {
        self.resources.iter().position(|r| r.id == id).map(|i| ResourceId(i as u16))
    }

    pub fn biome(&self, id: BiomeId) -> &BiomeDef {
        &self.biomes[id.0 as usize]
    }

    pub fn resource(&self, id: ResourceId) -> &ResourceDef {
        &self.resources[id.0 as usize]
    }

    pub fn nation(&self, n: Nation) -> &NationDef {
        self.nations.iter().find(|d| d.id == n).expect("validated at load")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_data_is_valid() {
        let defs = Defs::bundled();
        assert!(defs.biome_id("plains").is_some());
        assert!(defs.resource_id("gold_vein").is_some());
    }

    #[test]
    fn unknown_biome_reference_is_rejected() {
        let err = Defs::from_toml(
            include_str!("../../../data/biomes.toml"),
            "[[resource]]\nid='x'\nname='X'\nglyph='x'\nchance=0.5\nbiomes=['swamp']",
            include_str!("../../../data/nations.toml"),
        )
        .unwrap_err();
        assert!(err.0.contains("swamp"), "{err}");
    }
}

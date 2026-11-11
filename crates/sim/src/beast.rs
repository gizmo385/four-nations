//! Beasts (`data/beasts.toml`, docs/design/horizons-phase-2.md, "Shared sim work, 3. Beasts"):
//! the sky bison, and the herd beasts and the badgermole after it.
//!
//! A `[[beast]]` row names the nation it serves and the `home` building that houses it. Each
//! finished home building puts one beast of the settlement nation's row at its door
//! (`world/beast.rs`); taking the building down sends it back to the wild. Exactly one section
//! says what it does:
//!
//! - `[beast.haul] {carry, min_trip}`: takes Haul and Refuel jobs whose trip, pickup to drop, is
//!   at least `min_trip` tiles (Chebyshev) and carries `carry` items, not scaled by the nation's
//!   `carry_capacity`.
//! - `[beast.bend] {ability, seconds}`: bends designated tiles of `ability` (the badgermole's
//!   tunnels, #697), nearest its home first, `seconds` a tile, with no chi and no mastery credit
//!   (`world/beast.rs`).
//!
//! `speed` is optional: absent, the beast moves at the nation's own `move_speed`. Read by
//! `Defs::with_beasts`, a separate step from `from_toml` like `with_harm`, so data without a
//! beasts file simply has no beasts.

use serde::Deserialize;

use crate::bending::AbilityId;
use crate::building::BuildingId;
use crate::defs::{Defs, DefsError};
use crate::nation::Nation;

// `Deserialize` is `save::ids`' remapping impl (#602).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub struct BeastId(pub u16);

/// What a beast does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BeastWork {
    /// `[beast.haul]`.
    Haul {
        /// Items it carries a trip, not scaled by `carry_capacity`: at least 1.
        carry: u32,
        /// It takes only trips of at least this many tiles, pickup to drop (Chebyshev): at
        /// least 1.
        min_trip: u32,
    },
    /// `[beast.bend]`.
    Bend {
        /// The ability it bends: it exists and belongs to the beast's nation.
        ability: AbilityId,
        /// Seconds a bend takes it: above 0.
        seconds: f32,
    },
}

/// One `[[beast]]` row.
#[derive(Debug, Clone, PartialEq)]
pub struct BeastDef {
    pub id: String,
    pub name: String,
    /// The settlement nation whose home buildings house it.
    pub nation: Nation,
    /// The building that houses one of it. At most one row per `(nation, home)`.
    pub home: BuildingId,
    /// Its own speed (`move_speed` units); `None` is the settlement's `move_speed`.
    pub speed: Option<f32>,
    pub work: BeastWork,
}

/// The greatest `speed` a row may name: the same ceiling a track has.
const MAX_SPEED: f32 = 10.0;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BeastFile {
    #[serde(default)]
    beast: Vec<RawBeast>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBeast {
    id: String,
    name: String,
    nation: Nation,
    home: String,
    speed: Option<f32>,
    haul: Option<RawHaul>,
    bend: Option<RawBend>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHaul {
    carry: u32,
    min_trip: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBend {
    ability: String,
    seconds: f32,
}

impl BeastFile {
    fn resolve(self, defs: &Defs) -> Result<Vec<BeastDef>, DefsError> {
        let mut out: Vec<BeastDef> = Vec::new();
        for raw in self.beast {
            let id = raw.id.clone();
            let err = |msg: &str| Err(DefsError(format!("beasts.toml: beast {id}: {msg}")));
            if out.iter().any(|b| b.id == raw.id) {
                return err("id is used twice");
            }
            let Some(home) = defs.building_id(&raw.home) else {
                return err(&format!("names unknown home building {:?}", raw.home));
            };
            if let Some(other) = out.iter().find(|b| b.nation == raw.nation && b.home == home) {
                return err(&format!(
                    "shares home {:?} with {}; one row per (nation, home)",
                    raw.home, other.id
                ));
            }
            if let Some(s) = raw.speed
                && !(s.is_finite() && s > 0.0 && s <= MAX_SPEED)
            {
                return err(&format!("speed must be above 0 and at most {MAX_SPEED}"));
            }
            let work = match (raw.haul, raw.bend) {
                (Some(h), None) => {
                    if h.carry == 0 || h.min_trip == 0 {
                        return err("haul carry and min_trip must be at least 1");
                    }
                    BeastWork::Haul { carry: h.carry, min_trip: h.min_trip }
                }
                (None, Some(b)) => {
                    let Some(ability) = defs.ability_id(&b.ability) else {
                        return err(&format!("bend names unknown ability {:?}", b.ability));
                    };
                    if defs.ability(ability).nation != raw.nation {
                        return err(&format!(
                            "bend ability {:?} belongs to another nation",
                            b.ability
                        ));
                    }
                    if !(b.seconds.is_finite() && b.seconds > 0.0) {
                        return err("bend seconds must be above 0");
                    }
                    BeastWork::Bend { ability, seconds: b.seconds }
                }
                _ => return err("needs exactly one of [beast.haul] and [beast.bend]"),
            };
            out.push(BeastDef {
                id: raw.id,
                name: raw.name,
                nation: raw.nation,
                home,
                speed: raw.speed,
                work,
            });
        }
        Ok(out)
    }
}

impl BeastDef {
    /// `Some((carry, min_trip))` for a hauling beast.
    pub fn haul(&self) -> Option<(u32, u32)> {
        match self.work {
            BeastWork::Haul { carry, min_trip } => Some((carry, min_trip)),
            BeastWork::Bend { .. } => None,
        }
    }
}

impl Defs {
    /// These definitions with the beasts read from `beasts` (data/beasts.toml), replacing any
    /// they had. A separate step from `from_toml`, as `with_harm` is, so data without one (most
    /// tests) has no beasts.
    pub fn with_beasts(mut self, beasts: &str) -> Result<Self, DefsError> {
        let file: BeastFile =
            toml::from_str(beasts).map_err(|e| DefsError(format!("beasts.toml: {e}")))?;
        self.beasts = file.resolve(&self)?;
        for b in &mut self.buildings {
            b.houses_beast = false;
        }
        for beast in &self.beasts {
            self.buildings[beast.home.0 as usize].houses_beast = true;
        }
        Ok(self)
    }

    pub fn beast_id(&self, id: &str) -> Option<BeastId> {
        self.beasts.iter().position(|b| b.id == id).map(|i| BeastId(i as u16))
    }

    pub fn beast(&self, id: BeastId) -> &BeastDef {
        &self.beasts[id.0 as usize]
    }

    /// The row that `nation`'s settlements house in the building `home`, if any.
    pub fn beast_for(&self, nation: Nation, home: BuildingId) -> Option<BeastId> {
        self.beasts
            .iter()
            .position(|b| b.nation == nation && b.home == home)
            .map(|i| BeastId(i as u16))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEASTS: &str = include_str!("../../../data/beasts.toml");

    fn rejects(src: &str, needle: &str) {
        let e = Defs::bundled().with_beasts(src).expect_err(needle).0;
        assert!(e.contains(needle), "{e:?} should contain {needle:?}");
    }

    /// A second row under `id` for the sky bison's own (nation, home).
    fn second_row(id: &str) -> String {
        format!(
            "[[beast]]\nid = \"{id}\"\nname = \"Twin\"\nnation = \"air\"\nhome = \"bison_stable\"\n\
             [beast.haul]\ncarry = 5\nmin_trip = 5\n"
        )
    }

    #[test]
    fn the_sky_bison_is_the_first_row() {
        let d = Defs::bundled();
        let b = d.beast(BeastId(0));
        assert_eq!((b.id.as_str(), b.nation), ("sky_bison", Nation::Air));
        assert_eq!(b.home, d.building_id("bison_stable").unwrap());
        assert_eq!(b.speed, None);
        assert_eq!(b.haul(), Some((20, 10)));
        assert_eq!(d.beast_for(Nation::Air, b.home), Some(BeastId(0)));
        assert_eq!(d.beast_for(Nation::Fire, b.home), None);
    }

    #[test]
    fn a_bend_row_parses_and_validates() {
        let ok = format!(
            "{BEASTS}\n[[beast]]\nid = \"test_mole\"\nname = \"Test Mole\"\nnation = \"earth\"\n\
             home = \"bed\"\nspeed = 1.0\n[beast.bend]\nability = \"bore_tunnel\"\nseconds = 30.0\n"
        );
        let d = Defs::bundled().with_beasts(&ok).expect("a bend row is valid");
        let row = d.beast(d.beast_id("test_mole").unwrap());
        assert_eq!(row.speed, Some(1.0));
        assert!(matches!(row.work, BeastWork::Bend { seconds, .. } if seconds == 30.0));
        assert_eq!(row.haul(), None);
        rejects(&ok.replace("bore_tunnel", "no_such_ability"), "unknown ability");
        rejects(&ok.replace("bore_tunnel", "hearthflame"), "another nation");
        rejects(&ok.replace("seconds = 30.0", "seconds = 0.0"), "seconds must be above 0");
    }

    #[test]
    fn rows_are_validated() {
        rejects(&BEASTS.replace("\"bison_stable\"", "\"nowhere\""), "unknown home building");
        rejects(&format!("{BEASTS}\n{}", second_row("twin")), "one row per (nation, home)");
        rejects(&format!("{BEASTS}\n{}", second_row("sky_bison")), "id is used twice");
        rejects(&BEASTS.replace("carry = 20", "carry = 0"), "at least 1");
        rejects(&BEASTS.replace("min_trip = 10", "min_trip = 0"), "at least 1");
        rejects(
            &BEASTS.replacen("[beast.haul]\ncarry", "speed = 0.0\n[beast.haul]\ncarry", 1),
            "speed must be",
        );
        rejects(
            &BEASTS.replacen("[beast.haul]\ncarry", "speed = 99.0\n[beast.haul]\ncarry", 1),
            "speed must be",
        );
        let hauler = "[beast.haul]\ncarry = 20\nmin_trip = 10\n";
        assert!(BEASTS.contains(hauler), "the sky bison row changed shape");
        let both = BEASTS.replace(
            hauler,
            &format!("{hauler}[beast.bend]\nability = \"bore_tunnel\"\nseconds = 5.0\n"),
        );
        rejects(&both, "exactly one of");
        rejects(&BEASTS.replace(hauler, ""), "exactly one of");
        rejects(&format!("{BEASTS}\nextra = 1\n"), "beasts.toml");
    }

    #[test]
    fn another_nation_may_name_the_same_home() {
        let both = format!(
            "{BEASTS}\n{}",
            second_row("twin").replace("nation = \"air\"", "nation = \"fire\"")
        );
        let d = Defs::bundled().with_beasts(&both).expect("a different (nation, home)");
        assert_eq!(d.beasts.len(), Defs::bundled().beasts.len() + 1);
    }
}

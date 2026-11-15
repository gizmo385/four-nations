//! Residents' standing (docs/design/factory-endgame.md, "Layer 1: wants and standing", #1026):
//! in a United City each resident is a Settler, a Citizen or a Notable, climbing by having their
//! nation's wanted goods met and falling back when one goes unmet. This module is the data
//! (`[standing]` in `data/jobs.toml`, beside the items it names); `world/standing.rs` runs it.

use serde::{Deserialize, Serialize};

use crate::defs::DefsError;
use crate::jobs::{ItemDef, ItemId};
use crate::nation::Nation;

/// A resident's standing. Everyone starts a Settler, as before the Republic era.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    #[default]
    Settler,
    Citizen,
    Notable,
}

impl Tier {
    /// The tier as the player reads it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Settler => "Settler",
            Self::Citizen => "Citizen",
            Self::Notable => "Notable",
        }
    }

    /// The tier above, if any.
    pub fn up(self) -> Option<Tier> {
        match self {
            Self::Settler => Some(Self::Citizen),
            Self::Citizen => Some(Self::Notable),
            Self::Notable => None,
        }
    }

    /// The tier below, if any.
    pub fn down(self) -> Option<Tier> {
        match self {
            Self::Settler => None,
            Self::Citizen => Some(Self::Settler),
            Self::Notable => Some(Self::Citizen),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawStanding {
    citizen_minutes: f32,
    notable_minutes: f32,
    drop_minutes: f32,
    citizen_work: f32,
    notable_work: f32,
    citizen_draw: f32,
    draw_max: f32,
    foreign_every_minutes: f32,
    foreign_nations: u8,
    want: Vec<RawWant>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWant {
    nation: Nation,
    tier: Tier,
    item: String,
    every_minutes: f32,
}

/// One nation's want at one tier: a good used up, one every `every_minutes`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WantDef {
    pub nation: Nation,
    /// `Citizen` (wanted to become one, and to stay one) or `Notable`.
    pub tier: Tier,
    pub item: ItemId,
    pub every_minutes: f32,
}

/// `[standing]`, resolved. The default has no wants, so nothing happens: data without the table
/// (tests' trimmed files) plays as before.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StandingDef {
    /// Minutes of every want met that make a Settler a Citizen.
    pub citizen_minutes: f32,
    /// Minutes of every want met that make a Citizen a Notable.
    pub notable_minutes: f32,
    /// Minutes one want of a resident's own tier goes unmet before they drop a tier.
    pub drop_minutes: f32,
    /// How much faster a Citizen and a Notable work: multiplies every work modifier.
    pub citizen_work: f32,
    pub notable_work: f32,
    /// What each Citizen or Notable adds to draw, and the most they add together.
    pub citizen_draw: f32,
    pub draw_max: f32,
    /// A Notable also wants the Citizen goods of this many other nations (the next ones in
    /// `Nation::ALL`'s order after their own), one of each every `foreign_every_minutes`.
    pub foreign_every_minutes: f32,
    pub foreign_nations: u8,
    pub wants: Vec<WantDef>,
}

impl StandingDef {
    /// The work multiplier for `tier`.
    pub fn work(&self, tier: Tier) -> f32 {
        match tier {
            Tier::Settler => 1.0,
            Tier::Citizen => self.citizen_work,
            Tier::Notable => self.notable_work,
        }
    }

    /// Minutes of wants met that lift a resident to `tier`.
    pub fn minutes_to(&self, tier: Tier) -> f32 {
        match tier {
            Tier::Settler => 0.0,
            Tier::Citizen => self.citizen_minutes,
            Tier::Notable => self.notable_minutes,
        }
    }

    /// The goods a resident of `nation` wants to hold `tier`, as `(item, every_minutes)`: nothing
    /// for a Settler; their Citizen goods for a Citizen; and for a Notable those, their Notable
    /// goods and the foreign Citizen goods.
    pub fn keep(&self, nation: Nation, tier: Tier) -> Vec<(ItemId, f32)> {
        let mut out = Vec::new();
        if tier >= Tier::Citizen {
            out.extend(self.own(nation, Tier::Citizen));
        }
        if tier >= Tier::Notable {
            out.extend(self.own(nation, Tier::Notable));
            out.extend(self.foreign(nation));
        }
        out
    }

    /// The goods a resident of `nation` at `tier` wants on top of `keep` to climb a tier: empty
    /// for a Notable.
    pub fn aim(&self, nation: Nation, tier: Tier) -> Vec<(ItemId, f32)> {
        match tier.up() {
            Some(up) => {
                let held = self.keep(nation, tier);
                self.keep(nation, up).into_iter().filter(|w| !held.contains(w)).collect()
            }
            None => Vec::new(),
        }
    }

    fn own(&self, nation: Nation, tier: Tier) -> impl Iterator<Item = (ItemId, f32)> + '_ {
        self.wants
            .iter()
            .filter(move |w| w.nation == nation && w.tier == tier)
            .map(|w| (w.item, w.every_minutes))
    }

    /// The Citizen goods of the `foreign_nations` nations after `nation`, each at
    /// `foreign_every_minutes`.
    fn foreign(&self, nation: Nation) -> Vec<(ItemId, f32)> {
        let at = Nation::ALL.iter().position(|&n| n == nation).unwrap_or(0);
        (1..=usize::from(self.foreign_nations).min(Nation::ALL.len() - 1))
            .map(|k| Nation::ALL[(at + k) % Nation::ALL.len()])
            .flat_map(|n| self.own(n, Tier::Citizen).map(|(item, _)| item).collect::<Vec<_>>())
            .map(|item| (item, self.foreign_every_minutes))
            .collect()
    }
}

/// Checks `[standing]` and links its item ids. `None` is the default: no wants.
pub(crate) fn resolve(
    raw: Option<RawStanding>,
    items: &[ItemDef],
) -> Result<StandingDef, DefsError> {
    let Some(raw) = raw else { return Ok(StandingDef::default()) };
    let err = |msg: String| Err(DefsError(format!("jobs.toml: standing: {msg}")));
    for (what, v) in [
        ("citizen_minutes", raw.citizen_minutes),
        ("notable_minutes", raw.notable_minutes),
        ("drop_minutes", raw.drop_minutes),
        ("citizen_work", raw.citizen_work),
        ("notable_work", raw.notable_work),
        ("foreign_every_minutes", raw.foreign_every_minutes),
    ] {
        if !(v.is_finite() && v > 0.0) {
            return err(format!("{what} must be a positive number"));
        }
    }
    for (what, v) in [("citizen_draw", raw.citizen_draw), ("draw_max", raw.draw_max)] {
        if !(v.is_finite() && v >= 0.0) {
            return err(format!("{what} must be zero or more"));
        }
    }
    let mut wants = Vec::new();
    for w in raw.want {
        if w.tier == Tier::Settler {
            return err(format!(
                "{} wants {}: a want's tier is citizen or notable",
                w.nation.as_str(),
                w.item
            ));
        }
        let Some(i) = items.iter().position(|d| d.id == w.item) else {
            return err(format!("unknown item {:?}", w.item));
        };
        if !(w.every_minutes.is_finite() && w.every_minutes > 0.0) {
            return err(format!("{}: every_minutes must be a positive number", w.item));
        }
        wants.push(WantDef {
            nation: w.nation,
            tier: w.tier,
            item: ItemId(i as u16),
            every_minutes: w.every_minutes,
        });
    }
    Ok(StandingDef {
        citizen_minutes: raw.citizen_minutes,
        notable_minutes: raw.notable_minutes,
        drop_minutes: raw.drop_minutes,
        citizen_work: raw.citizen_work,
        notable_work: raw.notable_work,
        citizen_draw: raw.citizen_draw,
        draw_max: raw.draw_max,
        foreign_every_minutes: raw.foreign_every_minutes,
        foreign_nations: raw.foreign_nations,
        wants,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::Defs;

    #[test]
    fn a_fire_notable_keeps_lanterns_steelware_and_two_foreign_citizen_goods() {
        let defs = Defs::bundled();
        let s = &defs.standing;
        let id = |i: &str| defs.item_id(i).unwrap();
        let items = |v: Vec<(ItemId, f32)>| v.into_iter().map(|(i, _)| i).collect::<Vec<_>>();
        assert!(s.keep(Nation::Fire, Tier::Settler).is_empty());
        assert_eq!(items(s.aim(Nation::Fire, Tier::Settler)), [id("lantern")]);
        assert_eq!(items(s.keep(Nation::Fire, Tier::Citizen)), [id("lantern")]);
        let notable = items(s.keep(Nation::Fire, Tier::Notable));
        assert_eq!(notable[..2], [id("lantern"), id("fine_steelware")]);
        assert_eq!(notable.len(), 2 + usize::from(s.foreign_nations));
        assert!(!notable[2..].contains(&id("lantern")), "foreign means another nation's");
        assert_eq!(items(s.aim(Nation::Fire, Tier::Citizen)), notable[1..]);
        assert!(s.aim(Nation::Fire, Tier::Notable).is_empty());
    }

    #[test]
    fn every_nation_wants_one_citizen_good_and_one_notable_good() {
        let defs = Defs::bundled();
        for n in Nation::ALL {
            for t in [Tier::Citizen, Tier::Notable] {
                let count = defs.standing.wants.iter().filter(|w| w.nation == n && w.tier == t);
                assert_eq!(count.count(), 1, "{n:?} {t:?}");
            }
        }
    }
}

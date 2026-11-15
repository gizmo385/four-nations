//! Residents' standing, run (docs/design/factory-endgame.md, "Layer 1", #1026; the data is
//! `crate::standing`). Once a minute in a United City, each resident takes the goods they want
//! from stores as each falls due, then climbs, holds or drops a tier:
//!
//! - **Keep**: the goods of their own tier. One that falls due and isn't in stores is unmet;
//!   unmet for `drop_minutes` running, they drop one tier.
//! - **Aim**: the goods of the tier above, on top. With every keep and aim good met for that
//!   tier's minutes running, they climb.
//!
//! A good is taken straight from stores, no walk: the first stockpile tile holding it, in tile
//! order. No RNG; residents go in unit order, so who gets the last lantern is fixed. Before the
//! gate nothing runs, so the road to it plays as it did.

use serde::{Deserialize, Serialize};

use crate::defs::Defs;
use crate::jobs::ItemId;
use crate::standing::Tier;

use super::{TICKS_PER_SECOND, Unit, World};

/// One resident's standing (`Unit::standing`): their tier, how long their wants have been met or
/// one has gone unmet, and when each good they want next falls due.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Standing {
    #[serde(default)]
    pub tier: Tier,
    /// Seconds running with every keep and aim good met.
    #[serde(default)]
    pub met_seconds: u32,
    /// Seconds running with a keep good unmet.
    #[serde(default)]
    pub unmet_seconds: u32,
    /// The tick each wanted good next falls due; a good not listed is due now.
    #[serde(default)]
    pub due: Vec<(ItemId, u64)>,
}

impl Standing {
    /// Whether this is a plain Settler with nothing tracked: not written to saves.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    fn due(&self, item: ItemId) -> u64 {
        self.due.iter().find(|(i, _)| *i == item).map_or(0, |&(_, t)| t)
    }

    fn set_due(&mut self, item: ItemId, tick: u64) {
        match self.due.iter_mut().find(|(i, _)| *i == item) {
            Some(d) => d.1 = tick,
            None => self.due.push((item, tick)),
        }
    }
}

const MINUTE: u64 = 60 * TICKS_PER_SECOND as u64;

fn minutes_to_ticks(m: f32) -> u64 {
    (m * MINUTE as f32).round().max(1.0) as u64
}

impl World {
    /// A resident's tier: a Settler unless the Republic era has lifted them.
    pub fn tier_of(&self, unit: &Unit) -> Tier {
        unit.standing.tier
    }

    /// Residents at each tier, Settler first.
    pub fn tier_counts(&self) -> [u32; 3] {
        let mut out = [0; 3];
        for u in self.residents() {
            out[u.standing.tier as usize] += 1;
        }
        out
    }

    /// Takes one `item` from stores if any is there (module doc). True if it did.
    fn take_one_stored(&mut self, item: ItemId) -> bool {
        let Some(pos) = self.stored.iter().find(|(_, s)| s.item == item).map(|(&p, _)| p) else {
            return false;
        };
        if let Some(s) = self.stored.get_mut(&pos) {
            s.count -= 1;
            if s.count == 0 {
                self.stored.remove(&pos);
            }
        }
        self.record_used(item, 1);
        true
    }

    /// Once a minute in a United City: wants taken, tiers climbed and dropped (module doc).
    pub(super) fn tick_standing(&mut self, defs: &Defs) {
        let s = &defs.standing;
        if s.wants.is_empty() || !self.tick.is_multiple_of(MINUTE) || !self.united() {
            return;
        }
        let now = self.tick;
        for i in 0..self.units.len() {
            if !self.units[i].is_settler() {
                continue;
            }
            let nation = self.nation_of(&self.units[i]);
            let tier = self.units[i].standing.tier;
            let keep = s.keep(nation, tier);
            let aim = s.aim(nation, tier);
            let mut keep_ok = true;
            let mut aim_ok = true;
            for (n, &(item, every)) in keep.iter().chain(&aim).enumerate() {
                let is_keep = n < keep.len();
                if self.units[i].standing.due(item) > now {
                    continue;
                }
                if self.take_one_stored(item) {
                    self.units[i].standing.set_due(item, now + minutes_to_ticks(every));
                } else if is_keep {
                    keep_ok = false;
                } else {
                    aim_ok = false;
                }
            }
            let st = &mut self.units[i].standing;
            st.due.retain(|(item, _)| keep.iter().chain(&aim).any(|(i, _)| i == item));
            let minute = 60;
            if keep_ok {
                st.unmet_seconds = 0;
            } else {
                st.unmet_seconds += minute;
                if st.unmet_seconds >= (s.drop_minutes * 60.0) as u32
                    && let Some(down) = st.tier.down()
                {
                    st.tier = down;
                    st.unmet_seconds = 0;
                    st.met_seconds = 0;
                    continue;
                }
            }
            match st.tier.up() {
                Some(up) if keep_ok && aim_ok => {
                    st.met_seconds += minute;
                    if st.met_seconds >= (s.minutes_to(up) * 60.0) as u32 {
                        st.tier = up;
                        st.met_seconds = 0;
                    }
                }
                _ => st.met_seconds = 0,
            }
        }
    }

    /// What Citizens and Notables add to draw, in milli-units (`DrawTermKind::Citizens`), and
    /// how many count.
    pub(super) fn citizen_draw(&self, defs: &Defs) -> (u32, u32) {
        let s = &defs.standing;
        let count = self.residents().filter(|u| u.standing.tier >= Tier::Citizen).count() as u32;
        let each = (s.citizen_draw * 1000.0).round() as u32;
        let max = (s.draw_max * 1000.0).round() as u32;
        (count, (each * count).min(max))
    }
}

#[cfg(test)]
mod tests;

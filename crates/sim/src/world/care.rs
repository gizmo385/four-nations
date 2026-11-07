//! Care tallies: how well the settlement looked after its people (issue #533, decision #520,
//! `docs/design/balance-harness.md` "4. The care measure", `docs/design/agent-hillclimb.md`
//! "Care").
//!
//! Deaths (`World::deaths`) and harm events (`World::harm_events`) are already recorded; nothing
//! else counted time spent suffering, or near-deaths. This does, every tick, in the sim, so a
//! balance-harness run and a replayed agent log compute identical numbers. Like `Tallies`, the
//! counts only go up, and a save from before they existed loads with zeros (`serde(default)`).
//!
//! Time is kept as whole ticks (exact, deterministic) and read back as minutes. Everything here
//! is about *settlers*: the Avatar is a guest who never dies (`tick_needs` pins his health), so
//! he is left out of every person-minute and his leaving unfed counts as a single close call
//! instead (`CareTally::count_avatar_left`). Consumers derive rates (per person-hour, per ten
//! person-hours) and weights; none live here.

use std::collections::BTreeSet;

use super::{TICKS_PER_SECOND, Unit, UnitId};
use crate::defs::Defs;
use crate::needs::{HEALTH_LOW, Needs};

/// A need that can sit at 0 (`CareTally::depleted_minutes`), in `CareNeed::ALL`'s order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CareNeed {
    Hunger,
    Thirst,
    Energy,
}

impl CareNeed {
    pub const ALL: [CareNeed; 3] = [Self::Hunger, Self::Thirst, Self::Energy];

    fn index(self) -> usize {
        self as usize
    }
}

/// A severe state: the People row's Bad statuses (`fn_ui::view::need_word`), in `Severe::ALL`'s
/// order. A need is severe below half its `low` (`data/needs.toml`); health is Dying below half of
/// `needs::HEALTH_LOW`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Severe {
    Starving,
    Parched,
    Exhausted,
    Dying,
}

impl Severe {
    pub const ALL: [Severe; 4] = [Self::Starving, Self::Parched, Self::Exhausted, Self::Dying];

    fn index(self) -> usize {
        self as usize
    }

    /// The word the People row wears for it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Starving => "Starving",
            Self::Parched => "Parched",
            Self::Exhausted => "Exhausted",
            Self::Dying => "Dying",
        }
    }
}

/// Which severe flags `needs` shows right now, in `Severe::ALL`'s order. The same rule as
/// `fn_ui::view::need_status` giving `Status::Bad`: the meter as a fraction of full, below half of
/// its `low` as a fraction of full.
fn severe_flags(needs: &Needs, defs: &Defs) -> [bool; 4] {
    let s = &defs.needs;
    let bad = |value: f32, max: f32, low: f32| (value / max).clamp(0.0, 1.0) < low / 2.0;
    let low = |n: crate::needs::NeedDef| n.low / s.max();
    [
        bad(needs.hunger, s.max(), low(s.hunger)),
        bad(needs.thirst, s.max(), low(s.thirst)),
        bad(needs.energy, s.max(), low(s.energy)),
        bad(needs.health, s.health.max, HEALTH_LOW),
    ]
}

/// The care counts, read through `World::care()`.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CareTally {
    /// Settler-ticks lived.
    person_ticks: u64,
    /// Settler-ticks with each need at 0, by `CareNeed`.
    depleted_ticks: [u64; 3],
    /// Settler-ticks showing each severe flag, by `Severe`.
    severe_ticks: [u64; 4],
    /// Settler-ticks showing at least one severe flag.
    severe_any_ticks: u64,
    /// Settler-ticks sleeping rough in the cold, with exposure applying.
    cold_ticks: u64,
    close_calls: u32,
    /// The lowest health any settler has reached; `None` until one has been seen.
    lowest_health: Option<f32>,
    /// Settlers who have fallen below `[care] close_call_health` and not yet been seen back at or
    /// above it: a close call in progress, not counted until they live through it.
    dipped: BTreeSet<UnitId>,
    /// Settlers whose close call is counted and who haven't yet recovered above `[care]
    /// rearm_health`, so their next dip doesn't count again.
    counted: BTreeSet<UnitId>,
}

impl CareTally {
    fn minutes(ticks: u64) -> f64 {
        ticks as f64 / (60.0 * f64::from(TICKS_PER_SECOND))
    }

    /// Minutes lived, summed over settlers: the denominator of every rate.
    pub fn person_minutes(&self) -> f64 {
        Self::minutes(self.person_ticks)
    }

    /// Person-minutes with `need` at 0 (the need is taking health).
    pub fn depleted_minutes(&self, need: CareNeed) -> f64 {
        Self::minutes(self.depleted_ticks[need.index()])
    }

    /// Person-minutes showing `flag`, by the People row's own Bad rule. A settler can show
    /// several at once and is counted for each, so this is a breakdown, not a total: for the
    /// share of person-time anyone suffers, use `severe_any_minutes`.
    pub fn severe_minutes(&self, flag: Severe) -> f64 {
        Self::minutes(self.severe_ticks[flag.index()])
    }

    /// Person-minutes showing at least one severe flag (Starving, Parched, Exhausted or Dying),
    /// each settler-tick once however many flags it shows: decision #520's "suffering" is this
    /// over `person_minutes`.
    pub fn severe_any_minutes(&self) -> f64 {
        Self::minutes(self.severe_any_ticks)
    }

    /// Person-minutes asleep without shelter in the cold, with exposure damaging them
    /// (`World::apply_exposure`; so only with the difficulty preset's `harm` above 0).
    pub fn cold_minutes(&self) -> f64 {
        Self::minutes(self.cold_ticks)
    }

    /// Settlers who fell below `[care] close_call_health` and lived (counted once they are back at
    /// or above it), re-arming per settler above `rearm_health`; plus one for each time the Avatar
    /// left unfed or tired (`AvatarLeft`). A settler who dies in the dip adds none.
    pub fn close_calls(&self) -> u32 {
        self.close_calls
    }

    /// The lowest health any settler has reached, `None` before any has been seen.
    pub fn lowest_health(&self) -> Option<f32> {
        self.lowest_health
    }

    /// One tick of settler `u`, after this tick's needs, exposure and sickness have applied.
    /// `exposed` is whether `World::apply_exposure` damaged them this tick. The Avatar is skipped.
    pub(super) fn observe(&mut self, u: &Unit, exposed: bool, defs: &Defs) {
        if u.is_avatar() {
            return;
        }
        let health = u.needs.health;
        self.lowest_health = Some(self.lowest_health.map_or(health, |l| l.min(health)));
        if u.needs.is_dead() {
            // Dying in the dip is a death, not a close call: it counts in `World::deaths`.
            self.forget(u.id);
            return;
        }
        self.person_ticks += 1;
        let n = &u.needs;
        let levels = [n.hunger, n.thirst, n.energy];
        for (need, value) in CareNeed::ALL.into_iter().zip(levels) {
            if value <= 0.0 {
                self.depleted_ticks[need.index()] += 1;
            }
        }
        let severe = severe_flags(n, defs);
        for (flag, on) in Severe::ALL.into_iter().zip(severe) {
            if on {
                self.severe_ticks[flag.index()] += 1;
            }
        }
        if severe.contains(&true) {
            self.severe_any_ticks += 1;
        }
        if exposed {
            self.cold_ticks += 1;
        }
        let care = &defs.harm.care;
        if health < care.close_call_health {
            if !self.counted.contains(&u.id) {
                self.dipped.insert(u.id);
            }
        } else if self.dipped.remove(&u.id) {
            self.close_calls = self.close_calls.saturating_add(1);
            self.counted.insert(u.id);
        }
        if health > care.rearm_health {
            self.counted.remove(&u.id);
        }
    }

    /// The Avatar left (`World::avatar_leaves`): one close call (decision #520).
    pub(super) fn count_avatar_left(&mut self) {
        self.close_calls = self.close_calls.saturating_add(1);
    }

    /// Settler `id` has gone (dead, or otherwise removed): drops any close call in progress.
    pub(super) fn forget(&mut self, id: UnitId) {
        self.dipped.remove(&id);
        self.counted.remove(&id);
    }
}

#[cfg(test)]
mod tests;

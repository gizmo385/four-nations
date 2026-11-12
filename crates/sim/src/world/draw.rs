//! Draw (#435, docs/design/pacing.md section 1): how fast newcomers come, as a meter the player
//! can fill faster.
//!
//! Before draw, a newcomer came on a fixed clock once a spare bed stood, so nothing else the
//! player built sped growth up. Now `World::draw` fills by `multiplier / arrival_minutes` a
//! minute, where the multiplier (`World::draw_multiplier`) is `[growth.draw]`'s `base` plus a
//! bonus for each food kind beyond the first in stores, for every bed being warm, for spare beds
//! and for a Tea House being open (#710), capped at `max`. At 1.0, with growth's conditions met
//! (bed capacity above the headcount, `food_per_newcomer` in stores) and at least
//! `min_gap_minutes` since the last arrival, one newcomer joins through `World::spawn_newcomer`
//! and 1.0 is spent.
//!
//! **Why it stops at 1.0.** Filling stops at 1.0, whether or not the conditions hold, so a
//! player who builds a bed late gets one newcomer, not a burst. Only a grant (`World::add_draw`,
//! for caravan requests, #437) lifts draw past 1.0, up to `bank`; `min_gap_minutes` then spaces
//! the arrivals it buys.
//!
//! **Steered draw (#761).** In the United City a caravan's request pays its draw as a steer
//! (`World::add_steered_draw`): the milli-units are marked as the caravan's nation's, in a
//! first-in-first-out list (`World::steered_draw`) that stays within `draw`. Each arrival spends
//! one newcomer's worth from the front, and the nation at the front is the newcomer's, in place
//! of the weighted draw (`World::spawn_newcomer_of`). An arrival with nothing steered is
//! drawn by weight as before, so the weights only decide the draw nobody steered. Nothing here
//! uses an RNG, so it is deterministic as the rest of draw is.
//!
//! **Integer arithmetic.** Draw is held in milli-units (1000 is one newcomer) with a remainder
//! (`draw_carry`, in milli-units times `arrival_ticks`), so the meter is exact and the same on
//! every platform: each step adds `multiplier_milli x ticks` to the remainder and moves every
//! whole `arrival_ticks` of it into draw. The step runs once a second rather than every tick,
//! since the multiplier walks every bed and stockpile; a second is far finer than any arrival.

use super::*;
use crate::jobs::ItemId;

/// One milli-unit of draw is a thousandth of a newcomer.
pub const DRAW_ONE: u32 = 1000;

/// What one term of the draw multiplier is worth right now (`World::draw_multiplier`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawTermKind {
    /// The passive pace, always counted.
    Base,
    /// Food kinds beyond the first with at least `food_kind_min` in stores.
    FoodKinds,
    /// Every bed's tile is heated.
    WarmBeds,
    /// Free beds beyond the first.
    SpareBeds,
    /// A Tea House is open (`world/tea.rs`, #710). Listed only while one is.
    TeaHouse,
}

/// One term of the draw multiplier: how many of it count (`count` of `max_count`) and what that
/// adds (`milli`). A term is met when `count == max_count`; anything less is what's missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawTerm {
    pub kind: DrawTermKind,
    pub count: u32,
    pub max_count: u32,
    /// What the counted part adds to the multiplier, in milli-units.
    pub milli: u32,
}

impl DrawTerm {
    pub fn is_met(&self) -> bool {
        self.count >= self.max_count
    }
}

/// The draw multiplier and its terms, for the sim and the frontends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawReport {
    /// The multiplier after the cap, in milli-units: 1000 is one newcomer every `arrival_minutes`.
    pub milli: u32,
    /// Whether `[growth.draw].max` cut the sum of the terms.
    pub capped: bool,
    pub terms: Vec<DrawTerm>,
}

fn milli(v: f32) -> u32 {
    (v * DRAW_ONE as f32).round().max(0.0) as u32
}

impl World {
    /// Draw in milli-units: 1000 brings the next newcomer once growth's conditions hold.
    pub fn draw(&self) -> u32 {
        self.draw
    }

    /// The tick of the most recent newcomer, if any has joined since draw was saved.
    pub fn last_arrival(&self) -> Option<u64> {
        self.last_arrival
    }

    /// Grants `amount` milli-units of draw at once, past 1.0 if need be but never past
    /// `[growth.draw].bank` (#435; a caravan request's reward, #437). Arrivals still keep
    /// `min_gap_minutes` apart, so a large grant brings newcomers one at a time.
    pub fn add_draw(&mut self, amount: u32, defs: &Defs) {
        self.grant_draw(amount, None, defs);
    }

    /// `add_draw` for a United City caravan's request (#761, owner decision #747): the draw
    /// that fits under the bank is also marked as `nation`'s, so the newcomer it buys is of
    /// `nation`. Draw the bank cuts off is lost and steers nothing.
    pub fn add_steered_draw(&mut self, nation: Nation, amount: u32, defs: &Defs) {
        self.grant_draw(amount, Some(nation), defs);
    }

    fn grant_draw(&mut self, amount: u32, steer: Option<Nation>, defs: &Defs) {
        let bank = milli(defs.needs.growth.draw.bank);
        let before = self.draw;
        self.draw = before.saturating_add(amount).min(bank.max(before));
        let added = self.draw - before;
        if let Some(nation) = steer
            && added > 0
        {
            // A grant from the same caravan twice in a row is one stretch.
            match self.steered.last_mut() {
                Some((n, m)) if *n == nation => *m += added,
                _ => self.steered.push((nation, added)),
            }
        }
    }

    /// Whose newcomers the draw held now is for, in the order they will arrive: each entry is a
    /// nation and the milli-units of draw steered to it (#761). Draw beyond their sum is
    /// unsteered and is drawn by weight (`[growth]`'s `own_weight` and `other_weight`).
    pub fn steered_draw(&self) -> &[(Nation, u32)] {
        &self.steered
    }

    /// Spends one newcomer's `DRAW_ONE` from the front of `steered`; returns the nation it was
    /// for, if the draw spent was steered at all.
    fn spend_steered(&mut self) -> Option<Nation> {
        let (nation, _) = *self.steered.first()?;
        let mut left = DRAW_ONE;
        while left > 0 {
            let Some((_, m)) = self.steered.first_mut() else { break };
            let take = left.min(*m);
            *m -= take;
            left -= take;
            if *m == 0 {
                self.steered.remove(0);
            }
        }
        Some(nation)
    }

    /// The draw multiplier right now and what makes it up, met terms and missing ones alike.
    pub fn draw_multiplier(&self, defs: &Defs) -> DrawReport {
        let d = defs.needs.growth.draw;
        let mut per_food: BTreeMap<ItemId, u32> = BTreeMap::new();
        for s in self.stored.values().filter(|s| defs.food_of(s.item).is_some()) {
            *per_food.entry(s.item).or_default() += s.count;
        }
        let kinds = per_food.values().filter(|&&n| n >= d.food_kind_min).count() as u32;
        let food_kinds = kinds.saturating_sub(1).min(d.food_kinds_max);

        let mut beds = self.buildings().filter(|b| defs.building(b.id).bed.is_some()).peekable();
        let any_bed = beds.peek().is_some();
        let warm = any_bed && beds.all(|b| self.is_heated(b.anchor));

        let settlers = self.residents().count() as u32;
        let free = self.bed_capacity(defs).saturating_sub(settlers);
        let spare = free.saturating_sub(1).min(d.spare_beds_max);

        let mut terms = vec![
            DrawTerm { kind: DrawTermKind::Base, count: 1, max_count: 1, milli: milli(d.base) },
            DrawTerm {
                kind: DrawTermKind::FoodKinds,
                count: food_kinds,
                max_count: d.food_kinds_max,
                milli: milli(d.food_kind) * food_kinds,
            },
            DrawTerm {
                kind: DrawTermKind::WarmBeds,
                count: u32::from(warm),
                max_count: 1,
                milli: if warm { milli(d.warm_beds) } else { 0 },
            },
            DrawTerm {
                kind: DrawTermKind::SpareBeds,
                count: spare,
                max_count: d.spare_beds_max,
                milli: milli(d.spare_bed) * spare,
            },
        ];
        // Listed only while a house is open: it is a reason, never something a settlement is
        // missing before it has the tech.
        if self.tea_open(defs) {
            let tea = self.tea_draw_milli(defs);
            terms.push(DrawTerm {
                kind: DrawTermKind::TeaHouse,
                count: 1,
                max_count: 1,
                milli: tea,
            });
        }
        let sum: u32 = terms.iter().map(|t| t.milli).sum();
        let max = milli(d.max);
        DrawReport { milli: sum.min(max), capped: sum > max, terms }
    }

    /// Whether growth's conditions hold right now: bed capacity above the headcount and
    /// `food_per_newcomer` in stores.
    pub fn growth_conditions_met(&self, defs: &Defs) -> bool {
        self.has_room_for_newcomer(defs)
            && self.stored_food(defs) >= defs.needs.growth.food_per_newcomer
    }

    /// Fills draw and, at 1.0, brings a newcomer (see the module doc). Runs once a second.
    pub(super) fn tick_draw(&mut self, defs: &Defs) {
        let step = u64::from(TICKS_PER_SECOND);
        if !self.tick.is_multiple_of(step) {
            return;
        }
        let growth = defs.needs.growth;
        let arrival_ticks =
            (growth.arrival_minutes * 60.0 * TICKS_PER_SECOND as f32).round().max(1.0) as u64;
        if self.draw < DRAW_ONE {
            let m = u64::from(self.draw_multiplier(defs).milli);
            let carry = u64::from(self.draw_carry) + m * step;
            let gained = carry / arrival_ticks;
            self.draw_carry = (carry % arrival_ticks) as u32;
            self.draw = (u64::from(self.draw) + gained).min(u64::from(DRAW_ONE)) as u32;
            if self.draw >= DRAW_ONE {
                self.draw_carry = 0;
            }
        }
        if self.draw < DRAW_ONE || !self.growth_conditions_met(defs) {
            return;
        }
        let gap = (growth.draw.min_gap_minutes * 60.0 * TICKS_PER_SECOND as f32).round() as u64;
        if self.last_arrival.is_some_and(|t| self.tick < t + gap) {
            return;
        }
        // Steered draw (#761) names the nation of the arrival it pays for; it is only spent when
        // someone actually joins.
        let steer = self.steered.first().map(|&(n, _)| n);
        if self.spawn_newcomer_of(steer, defs) {
            self.spend_steered();
            self.draw -= DRAW_ONE;
            self.last_arrival = Some(self.tick);
            let newcomer_food = self.knobs(defs).newcomer_food;
            self.take_newcomer_food(newcomer_food, defs);
        }
    }
}

#[cfg(test)]
mod tests;

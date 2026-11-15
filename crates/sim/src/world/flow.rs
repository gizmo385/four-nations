//! What a United City makes and uses of each good, minute by minute (docs/design/factory-endgame.md,
//! "The Goods screen", #1028). Every finished batch counts its output as made and its inputs as
//! used; every good a resident takes from stores counts as used. Only the last
//! [`FLOW_MINUTES`] minutes are kept, and only once the gate stands, so the road to it saves
//! and plays as it did.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::building::RecipeDef;
use crate::jobs::ItemId;

use super::{TICKS_PER_SECOND, World};

/// How many minutes the rates are averaged over.
pub const FLOW_MINUTES: u64 = 10;

const MINUTE_TICKS: u64 = 60 * TICKS_PER_SECOND as u64;

/// One minute's counts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct FlowMinute {
    minute: u64,
    #[serde(default)]
    made: BTreeMap<ItemId, u32>,
    #[serde(default)]
    used: BTreeMap<ItemId, u32>,
}

/// The rolling counts (`World::goods_rate`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoodsFlow {
    #[serde(default)]
    minutes: VecDeque<FlowMinute>,
}

impl GoodsFlow {
    pub fn is_empty(&self) -> bool {
        self.minutes.is_empty()
    }

    fn bucket(&mut self, minute: u64) -> &mut FlowMinute {
        if self.minutes.back().is_none_or(|m| m.minute != minute) {
            self.minutes.push_back(FlowMinute { minute, ..FlowMinute::default() });
        }
        while self.minutes.front().is_some_and(|m| m.minute + FLOW_MINUTES <= minute) {
            self.minutes.pop_front();
        }
        self.minutes.back_mut().expect("just pushed")
    }

    /// (made, used) of `item` a minute, over the [`FLOW_MINUTES`] minutes up to `minute`.
    fn rate(&self, item: ItemId, minute: u64) -> (f32, f32) {
        let recent = self.minutes.iter().filter(|m| m.minute + FLOW_MINUTES > minute);
        let (made, used) = recent.fold((0, 0), |(made, used), m| {
            (
                made + m.made.get(&item).copied().unwrap_or(0),
                used + m.used.get(&item).copied().unwrap_or(0),
            )
        });
        let span = FLOW_MINUTES as f32;
        (made as f32 / span, used as f32 / span)
    }
}

impl World {
    fn minute(&self) -> u64 {
        self.tick / MINUTE_TICKS
    }

    /// Counts `count` of `item` made, once the gate stands.
    pub(super) fn record_made(&mut self, item: ItemId, count: u32) {
        if self.united() {
            let minute = self.minute();
            *self.flow.bucket(minute).made.entry(item).or_insert(0) += count;
        }
    }

    /// Counts `count` of `item` used up, once the gate stands.
    pub(super) fn record_used(&mut self, item: ItemId, count: u32) {
        if self.united() {
            let minute = self.minute();
            *self.flow.bucket(minute).used.entry(item).or_insert(0) += count;
        }
    }

    /// Counts a finished batch of `recipe`: its output made, its inputs used.
    pub(super) fn record_batch(&mut self, recipe: &RecipeDef) {
        if !self.united() {
            return;
        }
        self.record_made(recipe.output.item, recipe.output.count);
        for cost in &recipe.inputs {
            self.record_used(cost.item, cost.count);
        }
    }

    /// How many of `item` the city made and used a minute, over the last [`FLOW_MINUTES`]
    /// minutes (both 0 before the gate).
    pub fn goods_rate(&self, item: ItemId) -> (f32, f32) {
        self.flow.rate(item, self.minute())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rate_averages_the_last_ten_minutes_and_forgets_older_ones() {
        let item = ItemId(3);
        let mut flow = GoodsFlow::default();
        *flow.bucket(0).made.entry(item).or_insert(0) += 20;
        *flow.bucket(5).used.entry(item).or_insert(0) += 5;
        assert_eq!(flow.rate(item, 5), (2.0, 0.5));
        // Minute 0 falls out of the window at minute 10.
        assert_eq!(flow.rate(item, 10), (0.0, 0.5));
        flow.bucket(12);
        assert_eq!(flow.minutes.len(), 2, "minute 0 is dropped once a newer one is written");
    }

    #[test]
    fn nothing_is_counted_before_the_gate() {
        use crate::defs::Defs;
        use crate::mapgen::MapGenConfig;
        use crate::nation::Nation;
        let defs = Defs::bundled();
        let cfg = MapGenConfig { width: 48, height: 32, seed: 7, ..Default::default() };
        let mut w = World::new(&cfg, Nation::Fire, 6, &defs);
        let item = ItemId(3);
        w.record_made(item, 10);
        assert!(w.flow.is_empty());
        w.complete_endeavor(&defs);
        w.record_made(item, 10);
        assert_eq!(w.goods_rate(item), (1.0, 0.0));
    }
}

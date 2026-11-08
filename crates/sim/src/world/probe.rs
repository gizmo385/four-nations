//! Read-only questions a balance probe asks of a world (`fn_sim::balance`, #534,
//! `docs/design/balance-harness.md`, section 3): can a settler get to water and a stockpile, how
//! much food lies near a spot, and how many of the player's commands were refused. None of them
//! changes the world, and none is a rule: they report what the rules already decided.

use super::*;
use crate::flood::Flood;

/// What a settler could walk to from where it stands (`World::reach`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Reach {
    /// A tile beside water: somewhere to drink.
    pub water: bool,
    /// A stockpile tile; `None` while the settlement has marked none, so there is nothing to
    /// reach (not the same as being cut off from it).
    pub stockpile: Option<bool>,
    /// The settlement's anchor (`World::settlement_anchor`) or a tile beside it: the heart of the
    /// settlement, so `false` means the settler is walled off from everyone and everything.
    pub anchor: bool,
}

/// How many commands `World::apply` refused, and how many substantive ones in a row
/// (`World::refusals`). A session's own bookkeeping: it isn't saved, and a loaded world starts
/// from zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Refusals {
    total: u64,
    streak: u32,
}

impl Refusals {
    pub(super) fn record(&mut self, refusal: Option<&CommandError>) {
        match refusal {
            None => self.streak = 0,
            Some(e) => {
                self.total += 1;
                if !e.is_routine() {
                    self.streak += 1;
                }
            }
        }
    }

    /// Commands refused since the world was made.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Substantive refusals (`CommandError::is_routine` is false) since the last command `apply`
    /// took; routine ones neither add to it nor end it.
    pub fn streak(&self) -> u32 {
        self.streak
    }
}

impl World {
    /// How many commands `apply` has refused so far, and how many in a row.
    pub fn refusals(&self) -> Refusals {
        self.refusals
    }

    /// What living unit `unit` can walk to from where it stands, under the rules it moves by.
    /// `None` if it isn't alive.
    pub fn reach(&self, unit: UnitId) -> Option<Reach> {
        let u = self.unit(unit)?;
        Some(self.reach_with(u.pos, self.rules_for(u)))
    }

    /// What a settler standing on `pos` could walk to, under the ordinary walking rules: for a
    /// settler that has died, whose last position is all that's left.
    pub fn reach_from(&self, pos: Pos) -> Reach {
        self.reach_with(pos, &self.rules)
    }

    fn reach_with(&self, pos: Pos, rules: &MoveRules) -> Reach {
        let (_, water) = Flood::seek(&self.map, pos, rules, |p, _| {
            if !rules.passable(&self.map[p]) {
                return None;
            }
            self.map.neighbors(p, true).any(|q| self.map[q].terrain.is_water()).then_some(((), ()))
        });
        let stockpile = (!self.zones.is_empty()).then(|| {
            Flood::seek(&self.map, pos, rules, |p, _| self.zones.contains(&p).then_some(((), ())))
                .1
                .is_some()
        });
        let anchor = self.settlement_anchor();
        let (_, home) = Flood::seek(&self.map, pos, rules, |p, _| {
            (p.chebyshev(anchor) <= 1 && rules.passable(&self.map[p])).then_some(((), ()))
        });
        Reach { water: water.is_some(), stockpile, anchor: home.is_some() }
    }

    /// The food within `radius` tiles (Chebyshev) of `pos`, counted in items: what stockpiles
    /// and the ground hold there, plus what each ripe wild food resource would give if worked
    /// now. How near a hungry settler's meal really was, whatever kept it from the plate.
    pub fn food_near(&self, pos: Pos, radius: i32, defs: &Defs) -> u32 {
        let near = |p: Pos| p.chebyshev(pos) <= radius;
        let held = |stacks: &BTreeMap<Pos, Stack>| -> u32 {
            stacks
                .iter()
                .filter(|&(&p, s)| near(p) && defs.food_of(s.item).is_some())
                .map(|(_, s)| s.count)
                .sum()
        };
        let mut food = held(&self.stored) + held(&self.ground);
        for y in (pos.y - radius).max(0)..=(pos.y + radius).min(self.map.height() - 1) {
            for x in (pos.x - radius).max(0)..=(pos.x + radius).min(self.map.width() - 1) {
                let p = Pos::new(x, y);
                if let Some(h) = self.ripe_harvest(p, defs)
                    && defs.food_of(h.item).is_some()
                {
                    food += self.harvest_yield(p, h);
                }
            }
        }
        food
    }
}

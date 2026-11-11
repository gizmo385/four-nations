//! People riding a pair (`[building.glide]`, the Wind Gate; docs/design/horizons-phase-2.md,
//! "Shared sim work, 2. Routes through links").
//!
//! - **The rule.** A unit carrying nothing that starts walking a route compares it with the route
//!   by a gate: walk to gate A, a hop, walk from gate B to where it was going. The two walks are
//!   read from the cached fields of A and B (`linkfield.rs`), the hop takes
//!   `seconds + seconds_per_tile x distance` (`GlideDef::ticks`). If the gate route is quicker,
//!   the unit's route is rewritten to it; ties walk. Job choice never looks at gates: they make
//!   a chosen trip quicker, and don't pull anyone to a far job.
//! - **In the route.** The hop is one step of the route that isn't to a neighbouring tile: from
//!   the gate's tile to its partner's. `walk` starts the hop when it reaches that step, if the
//!   gate and its partner still work (else the route is dropped and planned afresh).
//! - **The hop.** The unit stands on the gate for `GlideDef::ticks` ticks and lands on the
//!   partner. A need doesn't interrupt it (`World::act` skips everything else while a hop is on),
//!   and both ends count the crossing (`Building::crossings`).
//! - **Who.** People only. A unit with a load walks, and one inside a building has no route to
//!   look at. Beasts (`beast.rs`) never hop.
//! - **Looked at once.** A route is looked over when it starts, not every tick:
//!   `Glider::seen` remembers where the last one was going and how many steps it had left, and a
//!   route to the same place that is no longer than that is the same route.

use crate::flood::Flood;
use crate::path::STRAIGHT;

use super::buildings::Kind;
use super::work::{route_from, step_needed};
use super::*;

/// A unit's hop through a gate, and the route last looked over for one.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct Glider {
    /// The hop in progress.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hop: Option<Hop>,
    /// The route last looked over for a gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    seen: Option<Seen>,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
struct Hop {
    /// The gate it left (its anchor).
    from: Pos,
    /// Where it lands: the partner's anchor.
    to: Pos,
    /// Ticks until it lands.
    left: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
struct Seen {
    /// Where the route goes.
    dest: Pos,
    /// How many steps it had left.
    steps: u32,
}

impl Glider {
    /// Whether a hop is in progress.
    pub(super) fn hopping(&self) -> bool {
        self.hop.is_some()
    }

    /// Whether there is nothing to remember or write.
    pub(super) fn is_idle(&self) -> bool {
        self.hop.is_none() && self.seen.is_none()
    }
}

/// Whether `u` may take a gate: people (never a beast) carrying nothing.
fn can_glide(u: &Unit) -> bool {
    u.beast.is_none() && u.carrying.is_none() && u.inside.is_none()
}

impl World {
    /// Whether unit `id` is partway through a hop.
    pub fn is_gliding(&self, id: UnitId) -> bool {
        self.unit(id).is_some_and(|u| u.glider.hopping())
    }

    /// How many hops have left or landed at the gate anchored on `pos`, or 0 for anything else.
    pub fn crossings_at(&self, pos: Pos) -> u32 {
        self.anchored(pos).map_or(0, |b| b.crossings)
    }

    /// Looks over the route unit `i` is about to walk, once per route, and swaps it for a route by
    /// a gate when that is quicker (`gate_route`).
    pub(super) fn consider_gate(&mut self, i: usize, defs: &Defs) {
        if !self.has_kind(Kind::Glide) {
            return;
        }
        let u = &self.units[i];
        if !can_glide(u) || u.glider.hopping() {
            return;
        }
        let Some(route) = u.route.as_ref().filter(|r| !r.is_empty()) else { return };
        let (dest, steps) = (route[0], route.len() as u32);
        let same = u.glider.seen.is_some_and(|s| s.dest == dest && steps <= s.steps);
        self.units[i].glider.seen = Some(Seen { dest, steps });
        if same {
            return;
        }
        if let Some(route) = self.gate_route(i, defs) {
            let steps = route.len() as u32;
            let u = &mut self.units[i];
            u.route = Some(route);
            u.glider.seen = Some(Seen { dest, steps });
        }
    }

    /// The route by a gate that beats unit `i`'s own route, if one does: the walk to the gate, its
    /// partner, then the walk on to where the route went. `None` when walking is as quick (ties
    /// walk), or the route already has a hop in it.
    fn gate_route(&mut self, i: usize, defs: &Defs) -> Option<Vec<Pos>> {
        // So the pair checks below read the cache.
        self.ensure_regions();
        let u = &self.units[i];
        let route = u.route.as_ref()?;
        let (pos, dest) = (u.pos, route[0]);
        let rules = self.rules_for(u);
        let (mut walk, mut from) = (0.0f32, pos);
        for &to in route.iter().rev() {
            if from.chebyshev(to) > 1 {
                return None;
            }
            walk += step_needed(rules, &self.map, from, to);
            from = to;
        }
        // Ticks: a step lands once the unit's speed has added up to what it needs.
        let speed = (self.modifiers_for(u, defs).move_speed * self.scooter_multiplier(u.id))
            .max(f32::EPSILON);
        let direct = walk / speed;
        let ticks = |cost: u32| cost as f32 * BASE_TICKS_PER_STEP / STRAIGHT as f32 / speed;
        // No step costs less than this (a track's, else a straight step), so a gate that the
        // tiles between it and the ends of the trip rule out is skipped without a flood.
        let floor = rules.track_steps().iter().copied().min().map_or(STRAIGHT, |t| t.min(STRAIGHT));

        let ends: Vec<(Pos, Pos, u32)> = self
            .of_kind(Kind::Glide)
            .filter_map(|b| {
                let glide = defs.building(b.id).glide?;
                let partner = self.pair_partner(b.anchor, defs)?;
                Some((b.anchor, partner, glide.ticks(b.anchor.chebyshev(partner) as u32)))
            })
            .collect();
        let mut best: Option<(f32, Pos, Pos)> = None;
        for (a, b, hop) in ends {
            let hop = hop as f32;
            let nearest = (pos.chebyshev(a) + b.chebyshev(dest)) as u32;
            if hop + ticks(nearest * floor) >= direct {
                continue;
            }
            let Some(to_a) = self.link_field(a, i).cost(pos) else { continue };
            let Some(from_b) = self.link_field(b, i).cost(dest) else { continue };
            let total = ticks(to_a) + hop + ticks(from_b);
            if total < direct && best.is_none_or(|(t, _, _)| total < t) {
                best = Some((total, a, b));
            }
        }
        let (_, a, b) = best?;
        let grid = self.step_grids.get(&self.map, self.map_rev, self.rules_for(&self.units[i]));
        let to_gate = self.flood_from(i).path_to(a)?;
        let from_gate = Flood::over(grid, b).path_to(dest)?;
        let mut route = route_from(from_gate);
        route.push(b);
        route.extend(route_from(to_gate));
        Some(route)
    }

    /// Unit `i` stands on a gate with a hop to its partner `to` as its next step: starts the hop,
    /// or, if the gate or the pair no longer works, drops the route to plan afresh.
    pub(super) fn begin_hop(&mut self, i: usize, to: Pos, defs: &Defs) {
        self.ensure_regions();
        let pos = self.units[i].pos;
        let hop = self.building_record(pos).and_then(|b| {
            let glide = defs.building(b.id).glide?;
            (self.pair_partner(b.anchor, defs) == Some(to)).then(|| Hop {
                from: b.anchor,
                to,
                left: glide.ticks(b.anchor.chebyshev(to) as u32),
            })
        });
        let u = &mut self.units[i];
        u.move_progress = 0.0;
        match hop {
            Some(hop) => u.glider.hop = Some(hop),
            None => u.route = None,
        }
    }

    /// One tick of a hop (`act` runs nothing else for a unit that is on one): lands it on the
    /// last, and counts the crossing at both ends.
    pub(super) fn glide_tick(&mut self, i: usize) {
        let u = &mut self.units[i];
        let Some(hop) = u.glider.hop.as_mut() else { return };
        hop.left = hop.left.saturating_sub(1);
        if hop.left > 0 {
            return;
        }
        let Hop { from, to, .. } = *hop;
        u.glider.hop = None;
        u.pos = to;
        u.move_progress = 0.0;
        if let Some(route) = u.route.as_mut()
            && route.last() == Some(&to)
        {
            route.pop();
        }
        for end in [from, to] {
            if let Some(b) = self.anchored_mut(end) {
                b.crossings += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests;

//! Power with a load (docs/design/factory-endgame.md, "Layer 4: power with a load", #1030).
//!
//! An automating yard (`AutomateDef`) with a `capacity` shares it among the machines it runs:
//! each automated machine draws its `BuildingDef::draw` (1 unless set) on its yard, the one
//! `World::yard_of` names. Over capacity, every machine on that yard slows in proportion, a
//! brownout. Nothing stops. A yard with no capacity (0) has no limit, which is every source's yard
//! before the era's plants.
//!
//! A **plant** is a yard with `crew` slots: each Notable crewing it adds `per_crew` to its
//! capacity. Crews aren't stored: `World::crews` deals the city's Notables out afresh each time,
//! in unit order, to the plants in anchor order, each to the first with a free slot whose
//! nation it shares (a plant unlocked by a nation's tech takes only that nation's Notables).
//! Crewing is a post, not a walk: a crewing Notable still works and rests as ever.

use std::collections::BTreeMap;

use crate::standing::Tier;

use super::*;

/// One automating yard's load: what its machines draw, and what it can give.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YardLoad {
    pub draw: u32,
    /// `None` for no limit.
    pub capacity: Option<u32>,
    /// Notables crewing it, and its crew slots.
    pub crew: (u32, u32),
}

impl YardLoad {
    /// The share of full speed its machines run at: 1.0 within capacity.
    pub fn speed(&self) -> f32 {
        match self.capacity {
            Some(cap) if self.draw > cap => cap as f32 / self.draw as f32,
            _ => 1.0,
        }
    }
}

impl World {
    /// Who crews each plant (a yard with `crew` slots), by its anchor: the city's Notables in
    /// unit order, each to the first plant by anchor with room that takes their nation.
    pub fn crews(&self, defs: &Defs) -> BTreeMap<Pos, Vec<UnitId>> {
        let mut out: BTreeMap<Pos, Vec<UnitId>> = BTreeMap::new();
        if !self.has_kind(Kind::Automates) {
            return out;
        }
        let plants: Vec<(Pos, u32, Option<Nation>)> = self
            .of_kind(Kind::Automates)
            .filter_map(|b| {
                let (tech, a) = defs.building(b.id).automate_upgrade.as_ref()?;
                (a.crew > 0 && self.research.is_known(*tech))
                    .then(|| (b.anchor, a.crew, defs.tech(*tech).nation))
            })
            .collect();
        if plants.is_empty() {
            return out;
        }
        for unit in self.units.iter().filter(|u| u.is_settler()) {
            if unit.standing.tier != Tier::Notable {
                continue;
            }
            let nation = self.nation_of(unit);
            let post = plants.iter().find(|&&(at, slots, takes)| {
                takes.is_none_or(|n| n == nation)
                    && out.get(&at).map_or(0, |c| c.len() as u32) < slots
            });
            if let Some(&(at, _, _)) = post {
                out.entry(at).or_default().push(unit.id);
            }
        }
        out
    }

    /// Every running yard's capacity with its crew, and that crew, by its anchor, with no draw
    /// counted yet.
    fn yard_capacities(&self, defs: &Defs) -> BTreeMap<Pos, YardLoad> {
        let mut out = BTreeMap::new();
        if !self.has_kind(Kind::Automates) {
            return out;
        }
        let crews = self.crews(defs);
        for s in self.of_kind(Kind::Automates) {
            let Some((tech, a)) = defs.building(s.id).automate_upgrade.as_ref() else { continue };
            if !self.research.is_known(*tech) {
                continue;
            }
            let crewed = crews.get(&s.anchor).map_or(0, |c| c.len() as u32);
            let capacity = (a.capacity > 0).then_some(a.capacity + crewed * a.per_crew);
            out.insert(s.anchor, YardLoad { draw: 0, capacity, crew: (crewed, a.crew) });
        }
        out
    }

    /// Which yard each machine on automation draws on, by the machine's anchor, with that yard's
    /// rate (#1096). Machines are dealt out in anchor order. Each takes the fastest yard in reach
    /// that still has room for its draw (of two as fast, the first by anchor), so a machine in
    /// reach of one yard draws on it as before. With no yard in reach left with room, it takes
    /// the one that would be least overloaded relative to its capacity. Before this, every
    /// machine drew on the first yard in reach, so one turbine browned out while the next sat
    /// idle.
    pub fn yard_assignment(&self, defs: &Defs) -> BTreeMap<Pos, (Pos, f32)> {
        let mut out = BTreeMap::new();
        if !self.has_kind(Kind::Automates) {
            return out;
        }
        let mut loads = self.yard_capacities(defs);
        let mut anchors: Vec<Pos> = self.buildings().map(|b| b.anchor).collect();
        anchors.sort();
        for pos in anchors {
            if self.anchored(pos).is_none_or(|b| b.manual) {
                continue;
            }
            let yards = self.yards_reaching(pos, defs);
            if yards.is_empty() {
                continue;
            }
            let draw = self.building_at(pos).map_or(1, |id| defs.building(id).draw);
            let room = |y: &super::automate::YardReach| {
                loads.get(&y.anchor).is_some_and(|l| l.capacity.is_none_or(|c| l.draw + draw <= c))
            };
            let fits: Vec<_> = yards.iter().copied().filter(room).collect();
            let pick = super::automate::best_yard(&fits).or_else(|| {
                // Everything in reach is full: the least overloaded, by (draw + d) / capacity,
                // compared as cross-products to stay in integers; ties to the faster, then the
                // first by anchor.
                let ratio = |y: &super::automate::YardReach| {
                    let l = loads.get(&y.anchor)?;
                    Some((u64::from(l.draw + draw), u64::from(l.capacity?.max(1))))
                };
                yards
                    .iter()
                    .filter_map(|y| ratio(y).map(|r| (y, r)))
                    .reduce(|best, next| {
                        let (b, (bn, bd)) = best;
                        let (n, (nn, nd)) = next;
                        let less = nn * bd < bn * nd;
                        let same = nn * bd == bn * nd;
                        if less || (same && n.rate > b.rate) { next } else { best }
                    })
                    .map(|(y, _)| (y.anchor, y.rate))
            });
            let Some((yard, rate)) = pick else { continue };
            if let Some(l) = loads.get_mut(&yard) {
                l.draw += draw;
            }
            out.insert(pos, (yard, rate));
        }
        out
    }

    /// Every running yard's load, by its anchor: the draw of the machines automated on it now
    /// (`yard_assignment`), its capacity with its crew, and that crew.
    pub fn yard_loads(&self, defs: &Defs) -> BTreeMap<Pos, YardLoad> {
        let mut out = self.yard_capacities(defs);
        for (pos, (yard, _)) in self.yard_assignment(defs) {
            let draw = self.building_at(pos).map_or(1, |id| defs.building(id).draw);
            if let Some(load) = out.get_mut(&yard) {
                load.draw += draw;
            }
        }
        out
    }

    /// The share of full speed the machine anchored on `pos` runs at under its yard's load:
    /// 1.0 within capacity, or off automation.
    pub fn brownout(&self, pos: Pos, defs: &Defs) -> f32 {
        self.yard_of(pos, defs)
            .and_then(|(yard, _)| self.yard_loads(defs).get(&yard).map(YardLoad::speed))
            .unwrap_or(1.0)
    }
}

#[cfg(test)]
mod tests;

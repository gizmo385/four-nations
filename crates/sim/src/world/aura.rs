//! Auras and landmarks (`[building.aura]`, `landmark = true`; docs/design/horizons-phase-4.md,
//! "Small pieces"; #723).
//!
//! - **Auras.** A finished building with an `AuraDef` makes units within its `radius`
//!   (Chebyshev from the footprint) regenerate health `heal` times and chi `chi_regen` times as
//!   fast, and study seats in a hall whose footprint comes within reach run `study` times as
//!   fast, multiplied with the power boost in integer millis. The best value per stat applies:
//!   two auras never stack. Health still only regenerates when it would anyway (not cold, not
//!   sick, no need low), and a healing building's own rate wins inside it.
//! - **Landmarks.** At most one of a landmark building per settlement, counting sites
//!   (`Blocker::OnlyOne`), and it can't be taken down (`DesignateError::Landmark`).

use crate::building::AuraDef;
use crate::geom::Footprint;

use super::build::Blocker;
use super::buildings::Kind;
use super::*;

/// The aura in effect at a place: the best of each stat among the auras reaching it, 1.0 where
/// none does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuraEffect {
    pub heal: f32,
    pub chi_regen: f32,
    /// As integer millis (1000 is 1.0x).
    pub study_millis: u32,
}

impl Default for AuraEffect {
    fn default() -> Self {
        Self { heal: 1.0, chi_regen: 1.0, study_millis: 1000 }
    }
}

impl AuraEffect {
    fn take(&mut self, a: &AuraDef) {
        self.heal = self.heal.max(a.heal);
        self.chi_regen = self.chi_regen.max(a.chi_regen);
        self.study_millis = self.study_millis.max(a.study_millis());
    }
}

impl World {
    /// Every finished building with an aura, as its footprint and def.
    pub fn auras(&self, defs: &Defs) -> Vec<(Footprint, AuraDef)> {
        let mut out: Vec<(Footprint, AuraDef)> = self
            .of_kind(Kind::Aura)
            .filter_map(|b| {
                let def = defs.building(b.id);
                let (w, h) = def.footprint;
                Some((Footprint::new(b.anchor, w, h), def.aura?))
            })
            .collect();
        // A heat source's `aura` upgrade (#724), over its own heat reach once known.
        out.extend(self.of_kind(Kind::HeatAura).filter_map(|b| {
            let (tech, aura) = defs.building(b.id).heat_aura_upgrade?;
            let heat = b.heat?;
            (self.research.is_known(tech) && b.fuel_lit())
                .then_some((b.footprint(), AuraDef { radius: heat.radius, ..aura }))
        }));
        out
    }

    /// The aura a unit standing on `pos` feels.
    pub fn aura_at(&self, pos: Pos, defs: &Defs) -> AuraEffect {
        self.aura_over(Footprint::single(pos), defs)
    }

    /// The aura over a footprint: every aura that comes within its radius of any tile of it.
    pub fn aura_over(&self, foot: Footprint, defs: &Defs) -> AuraEffect {
        let mut out = AuraEffect::default();
        for (f, a) in self.auras(defs) {
            if f.gap(foot) <= a.radius as i32 {
                out.take(&a);
            }
        }
        out
    }

    /// Why a landmark can't go up: one of it already stands, or is designated.
    pub(super) fn landmark_blocker(
        &self,
        anchor: Pos,
        def: &crate::building::BuildingDef,
        defs: &Defs,
    ) -> Option<(Pos, Blocker)> {
        if !def.landmark {
            return None;
        }
        let same = |id: crate::building::BuildingId| defs.building(id).id == def.id;
        let taken =
            self.buildings().any(|b| same(b.id)) || self.sites.values().any(|s| same(s.building));
        taken.then_some((anchor, Blocker::OnlyOne))
    }
}

#[cfg(test)]
mod tests;

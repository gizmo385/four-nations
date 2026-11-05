//! Strain: wild renewable plants tire when they're over-picked (`docs/design/difficulty.md`,
//! "Strain: tiring land", issue #296). A harvest adds strain to its tile (`HarvestDef::strain`
//! times the difficulty preset's `strain` knob), which lengthens how long the plant rests before
//! it ripens again; strain fades on its own, ripe or resting, while the tile is left alone.
//!
//! Tended plants (an orchard's radius, `World::tend_rate` above 1.0, #142) never tire: an orchard
//! is the sustainable answer to a tired wild grove. One-shot harvests (trees, stone, gold) have
//! no `strain` in `data/jobs.toml`, so they're never touched here.
//!
//! `World::strain` holds the live strain, by tile, skipped when empty. With the knob at 0
//! (Sandbox, or no difficulty preset at all, `Knobs::identity`) nothing is ever added to it, so
//! this changes no byte of an old save or of `World::new`.

use super::*;
use crate::jobs::HarvestDef;

/// A wild plant killed by over-picking (`strain_kills`), oldest first, for the TUI's alert log
/// (`docs/design/difficulty.md` "TUI": "a plant picked to death").
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StrainEvent {
    pub tick: u64,
    pub pos: Pos,
    /// What it was: the tile's own resource is already cleared by the time the caller can read
    /// this event.
    pub resource: ResourceId,
}

impl World {
    /// The strain on the tile at `pos`, for the TUI's hover (`docs/design/difficulty.md`,
    /// "Readable simulation"). `None` means untired: never picked, fully rested, or a plant an
    /// orchard tends.
    pub fn strain_at(&self, pos: Pos) -> Option<f32> {
        self.strain.get(&pos).copied()
    }

    /// Wild plants killed by over-picking so far, oldest first.
    pub fn strain_events(&self) -> &[StrainEvent] {
        &self.strain_events
    }

    /// Adds this harvest's strain to `pos` (`h.strain` times the difficulty preset's `strain`
    /// knob), unless strain is off, this harvest has none, or an orchard tends the tile
    /// (`tend_rate` above 1.0, #142: tended plants don't tire). Returns the strain the tile has
    /// after this harvest (0.0 if none builds or applies).
    fn add_strain(&mut self, pos: Pos, h: &HarvestDef, defs: &Defs) -> f32 {
        let scale = self.knobs(defs).strain;
        if scale <= 0.0 || h.strain <= 0.0 || self.tend_rate(pos, defs) > 1.0 {
            return self.strain.get(&pos).copied().unwrap_or(0.0);
        }
        let strain = self.strain.get(&pos).copied().unwrap_or(0.0) + h.strain * scale;
        self.strain.insert(pos, strain);
        strain
    }

    /// A harvest on `pos` just finished: adds its strain, then reports what `work_done` should
    /// do about regrowing it. With `strain_kills` on and the strain this harvest leaves at or
    /// above `[settings] strain_kill_at`, the tile's strain entry is dropped (there's nothing
    /// left to tire) and a kill event is logged; `None` tells the caller the plant is gone.
    /// Otherwise returns the regrow time's multiplier: `1.0 + strain` after this harvest.
    pub(super) fn harvest_strain(&mut self, pos: Pos, h: &HarvestDef, defs: &Defs) -> Option<f32> {
        let strain = self.add_strain(pos, h, defs);
        if strain >= defs.jobs.strain_kill_at && self.knobs(defs).strain_kills {
            self.strain.remove(&pos);
            // Still here: `work_done` only clears the tile's resource once this returns `None`.
            let resource = self.map[pos].resource.expect("harvesting a tile with a resource");
            self.strain_events.push(StrainEvent { tick: self.tick, resource, pos });
            return None;
        }
        Some(1.0 + strain)
    }

    /// Strain fades every tick, ripe or resting (`[settings] strain_recovery_per_minute`, flat,
    /// unscaled by any difficulty knob), and the entry drops once it reaches 0.
    pub(super) fn tick_strain(&mut self, defs: &Defs) {
        if self.strain.is_empty() {
            return;
        }
        let step = defs.jobs.strain_recovery_per_minute / (60.0 * TICKS_PER_SECOND as f32);
        let positions: Vec<Pos> = self.strain.keys().copied().collect();
        for pos in positions {
            let v = self.strain.get_mut(&pos).expect("just listed");
            *v = (*v - step).max(0.0);
            if *v <= 0.0 {
                self.strain.remove(&pos);
            }
        }
    }
}

#[cfg(test)]
mod tests;

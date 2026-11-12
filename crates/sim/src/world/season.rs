//! Seasons, climate bands, frost and cold on a running `World` (`crate::seasons`,
//! `docs/design/difficulty.md`, "Seasons", issue #295).
//!
//! `season`, `year`, `season_left` and `climate_at` are pure functions of the tick and the map:
//! nothing here is stored on `World` itself (the four seasons cycle forever, so there's nothing
//! to save beyond the tick already kept). They're always computable, whether or not a difficulty
//! preset turns `seasons` on; it's the systems that read them (`World::growth_rate` in
//! `work.rs`, `World::crop_growth_rate` in `farm.rs` (#311), `World::tick_needs` in `world.rs`,
//! and `tick_season` below) that check `knobs(defs).seasons` first, so with it off (or no preset
//! at all) nothing changes.
//!
//! `tick_season`, called once a tick from `World::tick`, fires the events the TUI turns into
//! alerts: a `Changed` event at every season boundary, a `WinterWarning`
//! `calendar.winter_warning_minutes` before winter, and on winter's first tick, the first frost
//! (`Frost`), which sends every ripe, non-`frost_hardy` wild plant on a frost-band tile back to
//! resting for its full regrow time, and every ripe, unharvested farm plot on one back to empty
//! (#311).

use crate::geom::Footprint;
use crate::seasons::{ClimateDef, SeasonDef};

use super::buildings::Kind;
use super::*;

/// A season change, the winter warning, or the first frost, oldest first
/// (`World::season_events`), for the TUI's alerts.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SeasonEvent {
    pub tick: u64,
    pub kind: SeasonEventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SeasonEventKind {
    /// A new season began: its index into `defs.seasons.seasons`.
    Changed { season: usize },
    /// Winter is `[calendar].winter_warning_minutes` away.
    WinterWarning,
    /// The first frost of winter struck this many ripe wild plants.
    Frost { count: u32 },
}

/// What the Almanac shows (`World::almanac`, docs/design/horizons-phase-1.md): ticks until
/// things the settlement can see coming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Almanac {
    /// Ticks to the next first frost; `None` when the preset brings no frost (seasons off, or
    /// a winter severity of 0).
    pub frost_in: Option<u64>,
    /// Ticks to the next caravan; always `None` until caravans (#437) add an arrival clock.
    pub caravan_in: Option<u64>,
}

impl World {
    /// Whether `feature` is available to this settlement: no tech unlocks it, or the one that
    /// does is known (`Unlocks::features`). Systems ask this, never a tech id.
    pub fn has_feature(&self, feature: crate::research::Feature, defs: &Defs) -> bool {
        self.is_unlocked(defs.feature_tech(feature))
    }

    /// The Almanac, once its feature is known (`calendar`); `None` before.
    pub fn almanac(&self, defs: &Defs) -> Option<Almanac> {
        if !self.has_feature(crate::research::Feature::Almanac, defs) {
            return None;
        }
        Some(Almanac { frost_in: self.frost_in(defs), caravan_in: None })
    }

    /// Ticks until the next first frost (winter's first tick, `tick_season`), counting from the
    /// next tick: `None` with no frost in this preset.
    fn frost_in(&self, defs: &Defs) -> Option<u64> {
        let knobs = self.knobs(defs);
        if !knobs.seasons || knobs.winter_severity <= 0.0 {
            return None;
        }
        let per = self.season_ticks(defs);
        let year = per * defs.seasons.seasons.len() as u64;
        let winter_start = defs.seasons.winter_index() as u64 * per;
        // The frost fires on the tick whose `tick % year` is `winter_start`.
        let into_year = self.tick % year;
        let to = (winter_start + year - into_year) % year;
        Some(if to == 0 { year } else { to })
    }

    /// Ticks in one season (`[calendar].season_minutes`), at least 1.
    fn season_ticks(&self, defs: &Defs) -> u64 {
        defs.seasons.season_ticks()
    }

    /// The index into `defs.seasons.seasons` of the season running at this tick: a pure function
    /// of the tick. A fresh world (tick 0) is at index 0, spring in the bundled data.
    pub fn season_index(&self, defs: &Defs) -> usize {
        defs.seasons.at(self.tick).season
    }

    /// The season running at this tick.
    pub fn season<'d>(&self, defs: &'d Defs) -> &'d SeasonDef {
        &defs.seasons.seasons[self.season_index(defs)]
    }

    /// The year this tick falls in, starting at 1.
    pub fn year(&self, defs: &Defs) -> u64 {
        defs.seasons.at(self.tick).year
    }

    /// In-game seconds left in the season running at this tick.
    pub fn season_left(&self, defs: &Defs) -> f32 {
        defs.seasons.at(self.tick).left_secs
    }

    /// The climate band of the tile at `pos`, by its mapgen temperature (`defs.seasons`). `None`
    /// off the map.
    pub fn climate_at<'d>(&self, pos: Pos, defs: &'d Defs) -> Option<&'d ClimateDef> {
        let tile = self.map.get(pos)?;
        Some(defs.seasons.climate_for(tile.temperature))
    }

    /// Does a unit standing at `pos` feel the cold right now (`tick_needs`'s own check, for the
    /// TUI's hover, `docs/design/difficulty.md` "Readable simulation"): the preset has seasons
    /// on, it's winter, the tile's climate band feels cold, and nothing heats it.
    pub fn is_cold(&self, pos: Pos, defs: &Defs) -> bool {
        self.knobs(defs).seasons
            && self.season(defs).winter
            && self.climate_at(pos, defs).is_some_and(|c| c.cold)
            && !self.is_heated(pos)
            && !self.is_sheltered(pos, defs)
    }

    /// Whether `pos` is within the radius of a finished building whose `shelter` upgrade is
    /// known (`southern_ways`'s Waystation, #727): nobody there is cold.
    pub fn is_sheltered(&self, pos: Pos, defs: &Defs) -> bool {
        if !self.has_kind(Kind::Shelter) {
            return false;
        }
        let here = Footprint::single(pos);
        self.of_kind(Kind::Shelter).any(|b| {
            let def = defs.building(b.id);
            def.shelter_upgrade.is_some_and(|(tech, radius)| {
                let (w, h) = def.footprint;
                self.research.is_known(tech)
                    && Footprint::new(b.anchor, w, h).gap(here) <= radius as i32
            })
        })
    }

    /// The growth-rate multiplier `World::growth_rate` (`work.rs`) applies for the seasons:
    /// identity (1.0) unless the difficulty preset's `seasons` flag is on. Outside winter it's
    /// the running season's `growth`. In winter, a `frost_hardy` harvest (fishing) uses a flat
    /// `hardy_winter_growth` in every band instead; everything else uses the tile's climate
    /// band's `winter_growth`, scaled toward 1.0 by `winter_severity`
    /// (`1 - (1 - winter_growth) * winter_severity`, floored at 0).
    pub(super) fn season_growth_factor(&self, pos: Pos, defs: &Defs, frost_hardy: bool) -> f32 {
        let knobs = self.knobs(defs);
        if !knobs.seasons {
            return 1.0;
        }
        let season = self.season(defs);
        if !season.winter {
            return season.growth;
        }
        if frost_hardy {
            return defs.seasons.calendar.hardy_winter_growth;
        }
        let g = self.climate_at(pos, defs).map_or(1.0, |c| c.winter_growth);
        (1.0 - (1.0 - g) * knobs.winter_severity).max(0.0)
    }

    /// Fires season events and the first frost. A no-op with `seasons` off.
    pub(super) fn tick_season(&mut self, defs: &Defs) {
        let knobs = self.knobs(defs);
        if !knobs.seasons {
            return;
        }
        let severity = knobs.winter_severity;
        let per = self.season_ticks(defs);
        let n = defs.seasons.seasons.len() as u64;
        let into_year = self.tick % (per * n);
        if into_year.is_multiple_of(per) {
            let idx = (into_year / per) as usize;
            self.log_event(SeasonEvent {
                tick: self.tick,
                kind: SeasonEventKind::Changed { season: idx },
            });
            if defs.seasons.seasons[idx].winter && severity > 0.0 {
                let count = self.apply_frost(defs);
                self.log_event(SeasonEvent {
                    tick: self.tick,
                    kind: SeasonEventKind::Frost { count },
                });
            }
        }
        let warn = {
            let minutes = f64::from(defs.seasons.calendar.winter_warning_minutes);
            (minutes * 60.0 * f64::from(TICKS_PER_SECOND)).round() as u64
        };
        let winter_start = defs.seasons.winter_index() as u64 * per;
        if warn <= winter_start && into_year == winter_start - warn {
            self.log_event(SeasonEvent { tick: self.tick, kind: SeasonEventKind::WinterWarning });
        }
    }

    /// The first frost of winter: every ripe (not resting), non-`frost_hardy` renewable wild
    /// plant on a frost-band tile goes back to resting for its full regrow time (times the
    /// difficulty preset's `regrow` knob, as a normal harvest does), and every ripe,
    /// unharvested farm plot on a frost-band tile (#311) goes back to empty. Returns how many
    /// of either it struck.
    fn apply_frost(&mut self, defs: &Defs) -> u32 {
        let regrow_knob = self.knobs(defs).regrow;
        let mut hit: Vec<(Pos, f32)> = Vec::new();
        for (pos, tile) in self.map.iter() {
            let Some(resource) = tile.resource else { continue };
            if self.resting.contains_key(&pos) {
                continue;
            }
            let Some(h) = defs.harvest(resource) else { continue };
            let Some(seconds) = h.regrow else { continue };
            if h.frost_hardy || !defs.seasons.climate_for(tile.temperature).frost {
                continue;
            }
            hit.push((pos, seconds * regrow_knob));
        }
        let mut count = hit.len() as u32;
        for (pos, seconds) in hit {
            self.resting.insert(pos, seconds);
        }

        let frosted_plots: Vec<Pos> = self
            .of_kind(Kind::Farm)
            .filter(|b| {
                b.plot.as_ref().is_some_and(|p| p.stage == PlotStage::Ripe)
                    && self.climate_at(b.anchor, defs).is_some_and(|c| c.frost)
                    && !self.frost_spares(b.anchor, defs)
            })
            .map(|b| b.anchor)
            .collect();
        count += frosted_plots.len() as u32;
        for pos in frosted_plots {
            self.frost_plot(pos);
        }
        count
    }
}

#[cfg(test)]
mod tests;

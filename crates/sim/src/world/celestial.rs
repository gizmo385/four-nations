//! Celestial days (`[[celestial_day]]` in `data/seasons.toml`, docs/design/horizons-phase-6.md,
//! "Celestial days", owner decision #748, issue #760): each nation has one 5-minute day a year,
//! in a season of its own, once `star_charts` (the `celestial_days` feature) is known.
//!
//! A day is a pure function of the tick and the data, like a season: nothing here is stored, and
//! it runs whether or not the difficulty preset's `seasons` flag is on (the calendar runs
//! either way, `SeasonsDef::at`). Who a day is for: the day's nation's *residents*
//! (`World::nation_of`) who are settlers: not the Avatar, who is a guest and whose meter is his
//! own, and not beasts, who have no chi.
//!
//! * **The rise.** On a day's first tick, every such unit's chi is raised to at least
//!   `refill x max`; one already above keeps its chi. A day learned mid-way is not refilled.
//! * **The day.** While it lasts those units regain chi at `chi_regen` times their usual rate,
//!   awake or asleep (`World::tick_needs` reads `World::celestial_regen`).
//! * **Events.** `SeasonEventKind::CelestialWarning` `[celestial] warning_minutes` ahead and
//!   `CelestialRise` at the first tick, only while the feature is known and only if the nation
//!   has a resident.

use crate::nation::Nation;
use crate::research::Feature;
use crate::seasons::{CelestialDayDef, CelestialDayId};

use super::*;

/// When a celestial day is, from now (`Almanac::celestial`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    /// Running now, with this many ticks left.
    Now { left: u64 },
    /// Begins in this many ticks.
    In { ticks: u64 },
}

impl World {
    /// The celestial day running now, or `None` between days and while `celestial_days` is
    /// unknown. A pure function of the tick and the data (`running_celestial`).
    pub fn celestial_day<'d>(&self, defs: &'d Defs) -> Option<&'d CelestialDayDef> {
        self.celestial_running(defs).map(|(_, day)| day)
    }

    /// The running day with its id and the ticks it has left (`celestial_day`).
    pub fn celestial_running<'d>(
        &self,
        defs: &'d Defs,
    ) -> Option<(CelestialDayId, &'d CelestialDayDef)> {
        if !self.has_feature(Feature::CelestialDays, defs) {
            return None;
        }
        Self::celestial_at(self.tick, defs).map(|(id, day, _)| (id, day))
    }

    /// The day running at `tick` with the ticks it has left, whatever is known.
    fn celestial_at(tick: u64, defs: &Defs) -> Option<(CelestialDayId, &CelestialDayDef, u64)> {
        let s = &defs.seasons;
        let year = s.year_ticks();
        let into = tick % year;
        s.celestial.days.iter().enumerate().find_map(|(i, day)| {
            let start = s.celestial_start(day);
            let end = start + s.celestial_len(day);
            (start <= into && into < end).then(|| (CelestialDayId(i as u8), day, end - into))
        })
    }

    /// Ticks until `nation`'s next day begins, counting from the next tick, and its id: a day
    /// running now is next year's. `None` while `celestial_days` is unknown, or if the nation
    /// has no day.
    pub fn next_celestial(&self, nation: Nation, defs: &Defs) -> Option<(CelestialDayId, u64)> {
        if !self.has_feature(Feature::CelestialDays, defs) {
            return None;
        }
        let s = &defs.seasons;
        let (i, day) = s.celestial.days.iter().enumerate().find(|(_, d)| d.nation == nation)?;
        let year = s.year_ticks();
        let to = (s.celestial_start(day) + year - self.tick % year) % year;
        Some((CelestialDayId(i as u8), if to == 0 { year } else { to }))
    }

    /// Residents of `nation` a day is for: settlers (no guests, no beasts) of that nation.
    pub fn celestial_residents(&self, nation: Nation) -> u32 {
        self.residents().filter(|u| self.nation_of(u) == nation).count() as u32
    }

    /// The Almanac's celestial lines: the running day with its time left, then each other day
    /// whose nation has a resident, soonest first, with that nation's bender count. Empty
    /// until `celestial_days` is known.
    pub(super) fn celestial_lines(&self, defs: &Defs) -> Vec<(CelestialDayId, Timing, u16)> {
        let running = self.celestial_running(defs);
        let mut out = Vec::new();
        if let Some((id, day)) = running {
            let left = Self::celestial_at(self.tick, defs).map_or(0, |(_, _, left)| left);
            out.push((id, Timing::Now { left }, self.celestial_residents(day.nation) as u16));
        }
        let mut later: Vec<(CelestialDayId, Timing, u16)> = defs
            .seasons
            .celestial
            .days
            .iter()
            .enumerate()
            .filter(|(i, _)| running.is_none_or(|(id, _)| id.0 as usize != *i))
            .filter_map(|(i, day)| {
                let benders = self.celestial_residents(day.nation);
                if benders == 0 {
                    return None;
                }
                let (_, ticks) = self.next_celestial(day.nation, defs)?;
                Some((CelestialDayId(i as u8), Timing::In { ticks }, benders as u16))
            })
            .collect();
        later.sort_by_key(|&(id, t, _)| match t {
            Timing::In { ticks } => (ticks, id),
            Timing::Now { .. } => (0, id),
        });
        out.extend(later);
        out
    }

    /// The factor a unit's chi regen is multiplied by right now for the running day, `running`
    /// being `celestial_day`: its `chi_regen` for a settler of the day's nation, else 1.0.
    pub(super) fn celestial_regen(&self, u: &Unit, running: Option<&CelestialDayDef>) -> f32 {
        match running {
            Some(day) if u.is_settler() && self.nation_of(u) == day.nation => day.chi_regen,
            _ => 1.0,
        }
    }

    /// Fires the warning and the rise, and refills on the rise. A no-op until
    /// `celestial_days` is known; independent of the preset's `seasons` flag.
    pub(super) fn tick_celestial(&mut self, defs: &Defs) {
        if !self.has_feature(Feature::CelestialDays, defs) {
            return;
        }
        let s = &defs.seasons;
        let year = s.year_ticks();
        let into = self.tick % year;
        let warn = s.celestial_warning_ticks();
        for (i, day) in s.celestial.days.iter().enumerate() {
            let id = CelestialDayId(i as u8);
            let start = s.celestial_start(day);
            let residents = self.celestial_residents(day.nation);
            if residents == 0 {
                continue;
            }
            if into == (start + year - warn % year) % year {
                self.log_event(SeasonEvent {
                    tick: self.tick,
                    kind: SeasonEventKind::CelestialWarning { day: id },
                });
            }
            if into == start {
                let mut units = 0;
                let mut refilled = 0.0_f32;
                for j in 0..self.units.len() {
                    let u = &self.units[j];
                    if !(u.is_settler() && self.nation_of(u) == day.nation) {
                        continue;
                    }
                    let to = day.refill * self.chi_max_for(u, defs);
                    let u = &mut self.units[j];
                    refilled += (to - u.chi).max(0.0);
                    u.chi = u.chi.max(to);
                    units += 1;
                }
                // The balance harness's `celestial_chi` metric (#764): the chi the rise gave, and
                // how many benders it reached, whole points summed over the run.
                self.tallies.count_school("celestial_chi", refilled.round() as u32);
                self.tallies.count_school("celestial_benders", units);
                self.log_event(SeasonEvent {
                    tick: self.tick,
                    kind: SeasonEventKind::CelestialRise { day: id, units },
                });
            }
        }
    }
}

#[cfg(test)]
mod tests;

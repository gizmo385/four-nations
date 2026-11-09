//! What a target measures (`docs/design/balance-harness.md`, section 5): a metric is a name in
//! `data/harness.toml` and a function from one run's record to a number.
//!
//! A reading is `Some(value)`, or `None` when the record can't say (the run ended before the
//! minute asked about). An event that never happened in the run reads as infinity, so "the stone
//! sign by minute 120" misses when the sign was never met and "no death in three hours" holds.
//!
//! | Name | Reads |
//! |---|---|
//! | `sign_minute.ID` | the minute sign `ID` was first met |
//! | `ready_minute` | the minute every sign but masters and scrolls was met |
//! | `first_death` | the minute of the first death |
//! | `deaths`, `deaths_by.M` | deaths over the run, or by minute `M` |
//! | `pop_at.M`, `wood_at.M` | population, wood in stores at the season boundary at minute `M` |
//! | `food_per_head.winter.N` | food per head entering year `N`'s winter |
//! | `longest_gap`, `gaps_over_15` | the pacing doc's decision gaps from minute 30 to Ready |
//! | `care.close_calls` | close calls over the run |
//! | `care.close_calls_per_10ph` | close calls per 10 person-hours |
//! | `care.depleted_per_ph` | person-minutes with a need at 0, per person-hour |
//! | `care.suffering` | the share of person-minutes with a severe flag |

use fn_sim::Defs;
use fn_sim::balance::RunRecord;

use crate::config::Bar;

/// How close a season row's minute has to be to the one asked for.
const ROW_SLACK: f32 = 0.5;

#[derive(Debug, Clone, PartialEq)]
pub enum Metric {
    SignMinute(String),
    ReadyMinute,
    FirstDeath,
    Deaths,
    DeathsBy(u32),
    PopAt(u32),
    WoodAt(u32),
    FoodPerHeadWinter(u64),
    LongestGap,
    GapsOver15,
    CloseCalls,
    CloseCallsPer10ph,
    DepletedPerPh,
    Suffering,
}

impl Metric {
    /// Parses a metric name, checking sign ids against `defs`.
    pub fn parse(name: &str, defs: &Defs) -> Result<Metric, String> {
        let number = |arg: &str| {
            arg.parse::<u32>()
                .map_err(|_| format!("metric {name:?}: {arg:?} is not a whole number"))
        };
        let (head, arg) = match name.split_once('.') {
            Some((h, a)) => (h, Some(a)),
            None => (name, None),
        };
        let metric = match (head, arg) {
            ("sign_minute", Some(id)) => {
                if !defs.endeavor.signs.iter().any(|s| s.id == id) {
                    return Err(format!("metric {name:?}: no sign {id:?}"));
                }
                Metric::SignMinute(id.to_owned())
            }
            ("ready_minute", None) => Metric::ReadyMinute,
            ("first_death", None) => Metric::FirstDeath,
            ("deaths", None) => Metric::Deaths,
            ("deaths_by", Some(m)) => Metric::DeathsBy(number(m)?),
            ("pop_at", Some(m)) => Metric::PopAt(number(m)?),
            ("wood_at", Some(m)) => Metric::WoodAt(number(m)?),
            ("food_per_head", Some(rest)) => match rest.strip_prefix("winter.") {
                Some(year) => Metric::FoodPerHeadWinter(u64::from(number(year)?)),
                None => return Err(format!("metric {name:?}: expected food_per_head.winter.N")),
            },
            ("longest_gap", None) => Metric::LongestGap,
            ("gaps_over_15", None) => Metric::GapsOver15,
            ("care", Some("close_calls")) => Metric::CloseCalls,
            ("care", Some("close_calls_per_10ph")) => Metric::CloseCallsPer10ph,
            ("care", Some("depleted_per_ph")) => Metric::DepletedPerPh,
            ("care", Some("suffering")) => Metric::Suffering,
            _ => return Err(format!("unknown metric {name:?}")),
        };
        Ok(metric)
    }

    /// The name [`Metric::parse`] reads it from: what `data/harness.toml` and the baseline call it.
    pub fn name(&self) -> String {
        match self {
            Metric::SignMinute(id) => format!("sign_minute.{id}"),
            Metric::ReadyMinute => "ready_minute".to_owned(),
            Metric::FirstDeath => "first_death".to_owned(),
            Metric::Deaths => "deaths".to_owned(),
            Metric::DeathsBy(m) => format!("deaths_by.{m}"),
            Metric::PopAt(m) => format!("pop_at.{m}"),
            Metric::WoodAt(m) => format!("wood_at.{m}"),
            Metric::FoodPerHeadWinter(y) => format!("food_per_head.winter.{y}"),
            Metric::LongestGap => "longest_gap".to_owned(),
            Metric::GapsOver15 => "gaps_over_15".to_owned(),
            Metric::CloseCalls => "care.close_calls".to_owned(),
            Metric::CloseCallsPer10ph => "care.close_calls_per_10ph".to_owned(),
            Metric::DepletedPerPh => "care.depleted_per_ph".to_owned(),
            Metric::Suffering => "care.suffering".to_owned(),
        }
    }

    /// Whether the name takes the minute it is read at (`pop_at.120`), so the run must reach it.
    fn needs_minute(&self) -> Option<f32> {
        match self {
            Metric::DeathsBy(m) | Metric::PopAt(m) | Metric::WoodAt(m) => Some(*m as f32),
            _ => None,
        }
    }

    /// The reading of `rec`, or `None` if the run can't say (module doc).
    pub fn read(&self, rec: &RunRecord, defs: &Defs) -> Option<f64> {
        if let Some(m) = self.needs_minute()
            && rec.minutes < m
        {
            return None;
        }
        let row_at =
            |minute: u32| rec.seasons.iter().find(|s| (s.minute - minute as f32).abs() < ROW_SLACK);
        let care = &rec.care;
        let person_hours = care.person_minutes / 60.0;
        match self {
            Metric::SignMinute(id) => {
                let met = rec.sign_minutes.iter().find(|(s, _)| s == id)?;
                Some(met.1.map_or(f64::INFINITY, f64::from))
            }
            Metric::ReadyMinute => {
                let mut ready = 0.0_f64;
                for (id, minute) in &rec.sign_minutes {
                    if id == "masters" || id == "scrolls" {
                        continue;
                    }
                    match minute {
                        Some(m) => ready = ready.max(f64::from(*m)),
                        None => return Some(f64::INFINITY),
                    }
                }
                Some(ready)
            }
            Metric::FirstDeath => Some(rec.first_death_minute.map_or(f64::INFINITY, f64::from)),
            Metric::Deaths => Some(f64::from(rec.deaths)),
            Metric::DeathsBy(m) => {
                let by = rec.autopsies.iter().filter(|a| a.minute <= *m as f32).count();
                Some(by as f64)
            }
            Metric::PopAt(m) => row_at(*m).map(|s| f64::from(s.population)),
            Metric::WoodAt(m) => row_at(*m).map(|s| f64::from(s.wood_stored)),
            Metric::FoodPerHeadWinter(year) => rec
                .seasons
                .iter()
                .find(|s| s.year == *year && defs.seasons.seasons[s.season].winter)
                .map(|s| f64::from(s.food_per_head)),
            Metric::LongestGap => Some(rec.longest_gap.map_or(0.0, |(a, b)| f64::from(b - a))),
            Metric::GapsOver15 => Some(f64::from(rec.gaps_over_15)),
            Metric::CloseCalls => Some(f64::from(care.close_calls)),
            Metric::CloseCallsPer10ph => Some(if person_hours > 0.0 {
                f64::from(care.close_calls) * 10.0 / person_hours
            } else {
                0.0
            }),
            Metric::DepletedPerPh => {
                let depleted: f64 = care.depleted_minutes.iter().sum();
                Some(if person_hours > 0.0 { depleted / person_hours } else { 0.0 })
            }
            Metric::Suffering => Some(if care.person_minutes > 0.0 {
                care.severe_any_minutes / care.person_minutes
            } else {
                0.0
            }),
        }
    }

    /// The minute by which a run must have shown what the target asks for: a sign's deadline, the
    /// minute a reading is taken at, or the end of the run. A flag raised after it doesn't touch
    /// the target.
    pub fn deadline(&self, bar: &Bar, rec: &RunRecord, defs: &Defs) -> f32 {
        let end = rec.minutes;
        match self {
            Metric::SignMinute(_) | Metric::ReadyMinute | Metric::FirstDeath => {
                bar.at_most.map_or(end, |m| m as f32).min(end)
            }
            Metric::DeathsBy(m) | Metric::PopAt(m) | Metric::WoodAt(m) => (*m as f32).min(end),
            Metric::FoodPerHeadWinter(year) => rec
                .seasons
                .iter()
                .find(|s| s.year == *year && defs.seasons.seasons[s.season].winter)
                .map_or(end, |s| s.minute),
            _ => end,
        }
    }
}

#[cfg(test)]
mod tests;

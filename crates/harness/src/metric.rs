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
//! | `pop_at.M`, `wood_at.M`, `food_at.M` | population, wood or food in stores at the season boundary at minute `M` |
//! | `herbs_at.M` | herbs in stores at the season boundary at minute `M` (#713) |
//! | `copies_by.M` | scroll copies made by the season boundary at minute `M` (#713) |
//! | `food_per_head.winter.N` | food per head entering year `N`'s winter |
//! | `longest_gap`, `gaps_over_15` | the pacing doc's decision gaps from minute 30 to Ready |
//! | `care.close_calls` | close calls over the run |
//! | `care.close_calls_per_10ph` | close calls per 10 person-hours |
//! | `care.depleted_per_ph` | person-minutes with a need at 0, per person-hour |
//! | `care.suffering` | the share of person-minutes with a severe flag |
//! | `great_ruins_found_by.M` | great ruins found (spotted, charted or searched) by minute `M` (#745) |
//! | `teachings_kept_by.M` | teachings kept from drafts by minute `M` (#745) |
//! | `first_offer_minute` | the minute of the first teaching draft (#745) |
//! | `empty_scrolls` | teaching scrolls read with nothing left to offer (#745) |
//! | `gate_minute` | the minute the gate stood (built, or stood by a `united` style's `gate_at`, #751) |
//! | `after_gate.NAME` | a minute metric `NAME` counted from `gate_minute`: `after_gate.first_offer_minute` is the first draft's wait after the gate (#751) |
//! | `residents_at.M`, `residents_at.M.NATION` | residents of the most numerous other nation, or of `NATION`, at the season boundary `M` minutes after `gate_minute` (#764) |
//! | `borrowed_minute`, `fusion_minute` | the minute the first borrowed tech, or the first fusion, was learned (`after_gate.` counts them from the gate, #764) |
//! | `tally.NAME` | one of the school tallies (#731), or a fusion's (`aqueduct_tiles`, `steel_made`, `lift_rides`, `bank_engines`, `voyages`, `rains`, #764), the last read only on runs that learned its fusion |
//! | `celestial_chi` | chi a celestial day refills per bender, averaged over the run's rises (#764) |
//! | `longest_gap.after_gate` | the longest stretch with no decision event from the gate to the run's end (#764) |

use fn_sim::balance::RunRecord;
use fn_sim::{Defs, Nation};

use crate::config::Bar;

/// How close a season row's minute has to be to the one asked for.
const ROW_SLACK: f32 = 0.5;

/// The season row at `minute`, if the run has one there.
fn row_at_minute(rec: &RunRecord, minute: f32) -> Option<&fn_sim::balance::EconomySeason> {
    rec.seasons.iter().find(|s| (s.minute - minute).abs() < ROW_SLACK)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Metric {
    SignMinute(String),
    ReadyMinute,
    FirstDeath,
    Deaths,
    DeathsBy(u32),
    PopAt(u32),
    WoodAt(u32),
    FoodAt(u32),
    HerbsAt(u32),
    CopiesBy(u32),
    FoodPerHeadWinter(u64),
    LongestGap,
    GapsOver15,
    CloseCalls,
    CloseCallsPer10ph,
    DepletedPerPh,
    Suffering,
    /// The minute the endeavor's Nth stage (from 1, file order) finished (#731).
    StageMinute(u32),
    /// The minute the first school tech was learned (#731).
    SchoolMinute,
    /// One of `fn_sim::balance::SCHOOL_TALLIES` at the run's end (#731).
    Tally(String),
    /// Great ruins found by minute `M` (#745).
    GreatRuinsFoundBy(u32),
    /// Teachings kept by minute `M` (#745).
    TeachingsKeptBy(u32),
    /// The minute of the first teaching draft (#745).
    FirstOfferMinute,
    /// Teaching scrolls read with nothing to offer (#745).
    EmptyScrolls,
    /// The minute the gate stood (#751): never is infinity.
    GateMinute,
    /// Residents `after` minutes past the gate: of `nation` if named, else of the other nation
    /// with the most (#764).
    ResidentsAt {
        after: u32,
        nation: Option<Nation>,
    },
    /// The minute the first borrowed tech was learned (#764): never is infinity.
    BorrowedMinute,
    /// The minute the first fusion was learned (#764): never is infinity.
    FusionMinute,
    /// Chi a celestial day refilled per bender, over the run's rises (#764). No reading when none
    /// rose.
    CelestialChi,
    /// The longest stretch with no decision event (`RunRecord::timeline`) from the gate to the
    /// end of the run (#764), in minutes. No reading when the gate never stood.
    LongestGapAfterGate,
    /// A minute metric counted from `gate_minute` (#751): its reading less the gate's minute, so
    /// 0 is the moment the gate stood and an event before it reads negative. Never is infinity;
    /// a run whose gate never stood has no reading.
    AfterGate(Box<Metric>),
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
            ("food_at", Some(m)) => Metric::FoodAt(number(m)?),
            ("herbs_at", Some(m)) => Metric::HerbsAt(number(m)?),
            ("copies_by", Some(m)) => Metric::CopiesBy(number(m)?),
            ("food_per_head", Some(rest)) => match rest.strip_prefix("winter.") {
                Some(year) => Metric::FoodPerHeadWinter(u64::from(number(year)?)),
                None => return Err(format!("metric {name:?}: expected food_per_head.winter.N")),
            },
            ("longest_gap", None) => Metric::LongestGap,
            ("longest_gap", Some("after_gate")) => Metric::LongestGapAfterGate,
            ("residents_at", Some(rest)) => {
                let (m, nation) = match rest.split_once('.') {
                    Some((m, n)) => {
                        let nation = Nation::ALL
                            .into_iter()
                            .find(|x| x.as_str() == n)
                            .ok_or_else(|| format!("metric {name:?}: no nation {n:?}"))?;
                        (m, Some(nation))
                    }
                    None => (rest, None),
                };
                Metric::ResidentsAt { after: number(m)?, nation }
            }
            ("borrowed_minute", None) => Metric::BorrowedMinute,
            ("fusion_minute", None) => Metric::FusionMinute,
            ("celestial_chi", None) => Metric::CelestialChi,
            ("gaps_over_15", None) => Metric::GapsOver15,
            ("care", Some("close_calls")) => Metric::CloseCalls,
            ("care", Some("close_calls_per_10ph")) => Metric::CloseCallsPer10ph,
            ("care", Some("depleted_per_ph")) => Metric::DepletedPerPh,
            ("care", Some("suffering")) => Metric::Suffering,
            ("stage_minute", Some(n)) => {
                let n = number(n)?;
                if n == 0 || n as usize > defs.endeavor.stages.len() {
                    return Err(format!("metric {name:?}: no stage {n}"));
                }
                Metric::StageMinute(n)
            }
            ("school_minute", None) => Metric::SchoolMinute,
            ("tally", Some(t)) => {
                if !fn_sim::balance::SCHOOL_TALLIES.contains(&t)
                    && !fn_sim::balance::FUSION_TALLIES.iter().any(|&(n, _)| n == t)
                {
                    return Err(format!("metric {name:?}: no tally {t:?}"));
                }
                Metric::Tally(t.to_owned())
            }
            ("great_ruins_found_by", Some(m)) => Metric::GreatRuinsFoundBy(number(m)?),
            ("teachings_kept_by", Some(m)) => Metric::TeachingsKeptBy(number(m)?),
            ("first_offer_minute", None) => Metric::FirstOfferMinute,
            ("empty_scrolls", None) => Metric::EmptyScrolls,
            ("gate_minute", None) => Metric::GateMinute,
            ("after_gate", Some(rest)) => {
                let inner = Metric::parse(rest, defs)?;
                if !inner.is_minute() {
                    return Err(format!("metric {name:?}: {rest:?} is not a minute"));
                }
                Metric::AfterGate(Box::new(inner))
            }
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
            Metric::FoodAt(m) => format!("food_at.{m}"),
            Metric::HerbsAt(m) => format!("herbs_at.{m}"),
            Metric::CopiesBy(m) => format!("copies_by.{m}"),
            Metric::FoodPerHeadWinter(y) => format!("food_per_head.winter.{y}"),
            Metric::LongestGap => "longest_gap".to_owned(),
            Metric::LongestGapAfterGate => "longest_gap.after_gate".to_owned(),
            Metric::ResidentsAt { after, nation: None } => format!("residents_at.{after}"),
            Metric::ResidentsAt { after, nation: Some(n) } => {
                format!("residents_at.{after}.{}", n.as_str())
            }
            Metric::BorrowedMinute => "borrowed_minute".to_owned(),
            Metric::FusionMinute => "fusion_minute".to_owned(),
            Metric::CelestialChi => "celestial_chi".to_owned(),
            Metric::GapsOver15 => "gaps_over_15".to_owned(),
            Metric::CloseCalls => "care.close_calls".to_owned(),
            Metric::CloseCallsPer10ph => "care.close_calls_per_10ph".to_owned(),
            Metric::DepletedPerPh => "care.depleted_per_ph".to_owned(),
            Metric::Suffering => "care.suffering".to_owned(),
            Metric::StageMinute(n) => format!("stage_minute.{n}"),
            Metric::SchoolMinute => "school_minute".to_owned(),
            Metric::Tally(t) => format!("tally.{t}"),
            Metric::GreatRuinsFoundBy(m) => format!("great_ruins_found_by.{m}"),
            Metric::TeachingsKeptBy(m) => format!("teachings_kept_by.{m}"),
            Metric::FirstOfferMinute => "first_offer_minute".to_owned(),
            Metric::EmptyScrolls => "empty_scrolls".to_owned(),
            Metric::GateMinute => "gate_minute".to_owned(),
            Metric::AfterGate(inner) => format!("after_gate.{}", inner.name()),
        }
    }

    /// Whether the reading is the minute of an event (never is infinity), which `after_gate` can
    /// count from the gate.
    fn is_minute(&self) -> bool {
        matches!(
            self,
            Metric::SignMinute(_)
                | Metric::ReadyMinute
                | Metric::FirstDeath
                | Metric::StageMinute(_)
                | Metric::SchoolMinute
                | Metric::FirstOfferMinute
                | Metric::GateMinute
                | Metric::BorrowedMinute
                | Metric::FusionMinute
        )
    }

    /// Whether the name takes the minute it is read at (`pop_at.120`), so the run must reach it.
    fn needs_minute(&self) -> Option<f32> {
        match self {
            Metric::DeathsBy(m)
            | Metric::PopAt(m)
            | Metric::WoodAt(m)
            | Metric::FoodAt(m)
            | Metric::HerbsAt(m)
            | Metric::CopiesBy(m)
            | Metric::GreatRuinsFoundBy(m)
            | Metric::TeachingsKeptBy(m) => Some(*m as f32),
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
            Metric::FoodAt(m) => row_at(*m).map(|s| f64::from(s.food_stored)),
            Metric::HerbsAt(m) => row_at(*m).map(|s| f64::from(s.herbs_stored)),
            Metric::CopiesBy(m) => row_at(*m).map(|s| f64::from(s.scrolls_copied)),
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
            // An older record has no stages: no reading. A stage not finished reads as never.
            Metric::StageMinute(n) => {
                let (_, minute) = rec.stage_minutes.get(*n as usize - 1)?;
                Some(minute.map_or(f64::INFINITY, f64::from))
            }
            Metric::SchoolMinute => Some(
                rec.techs
                    .iter()
                    .filter(|(id, _)| {
                        defs.tech_id(id).is_some_and(|t| defs.tech(t).school.is_some())
                    })
                    .map(|&(_, m)| f64::from(m))
                    .fold(f64::INFINITY, f64::min),
            ),
            Metric::Tally(t) => {
                if let Some(&(_, tech)) =
                    fn_sim::balance::FUSION_TALLIES.iter().find(|&&(n, _)| n == t)
                {
                    // A fusion's tally is read only where the fusion was learned.
                    if tech.is_some_and(|tech| !rec.techs.iter().any(|(id, _)| id == tech)) {
                        return None;
                    }
                    let (_, n) = rec.fusion_tallies.iter().find(|(id, _)| id == t)?;
                    return Some(f64::from(*n));
                }
                let (_, n) = rec.school_tallies.iter().find(|(id, _)| id == t)?;
                Some(f64::from(*n))
            }
            Metric::BorrowedMinute | Metric::FusionMinute => {
                let wanted = |id: &str| {
                    defs.tech_id(id).is_some_and(|t| match self {
                        Metric::FusionMinute => defs.tech(t).is_fusion(),
                        _ => defs.tech(t).borrowed_from(rec.nation).is_some(),
                    })
                };
                Some(
                    rec.techs
                        .iter()
                        .filter(|(id, _)| wanted(id))
                        .map(|&(_, m)| f64::from(m))
                        .fold(f64::INFINITY, f64::min),
                )
            }
            Metric::CelestialChi => {
                let get = |name: &str| {
                    rec.fusion_tallies.iter().find(|(id, _)| id == name).map_or(0, |&(_, n)| n)
                };
                let benders = get("celestial_benders");
                (benders > 0).then(|| f64::from(get("celestial_chi")) / f64::from(benders))
            }
            Metric::ResidentsAt { after, nation } => {
                let at = rec.gate_minute? + *after as f32;
                let row = row_at_minute(rec, at)?;
                let counts = row.residents;
                let of = |n: Nation| f64::from(counts[n.index()]);
                Some(match nation {
                    Some(n) => of(*n),
                    None => Nation::ALL
                        .into_iter()
                        .filter(|&n| n != rec.nation)
                        .map(of)
                        .fold(0.0, f64::max),
                })
            }
            Metric::LongestGapAfterGate => {
                let gate = rec.gate_minute?;
                let mut last = gate;
                let mut longest = 0.0_f32;
                for e in rec.timeline.iter().filter(|e| e.minute > gate && e.minute < rec.minutes) {
                    longest = longest.max(e.minute - last);
                    last = e.minute;
                }
                longest = longest.max(rec.minutes - last);
                Some(f64::from(longest))
            }
            Metric::GreatRuinsFoundBy(m) => {
                let t = &rec.teachings.great_ruins_found;
                Some(t.iter().filter(|&&at| at <= *m as f32).count() as f64)
            }
            Metric::TeachingsKeptBy(m) => {
                let t = &rec.teachings.kept;
                Some(t.iter().filter(|(_, at)| *at <= *m as f32).count() as f64)
            }
            Metric::FirstOfferMinute => {
                Some(rec.teachings.offers.first().map_or(f64::INFINITY, |&m| f64::from(m)))
            }
            Metric::EmptyScrolls => Some(f64::from(rec.teachings.empty_scrolls)),
            Metric::GateMinute => Some(rec.gate_minute.map_or(f64::INFINITY, f64::from)),
            Metric::AfterGate(inner) => {
                let gate = f64::from(rec.gate_minute?);
                inner.read(rec, defs).map(|m| m - gate)
            }
        }
    }

    /// The minute from which a flag counts against a run for this target: the gate for a reading
    /// of the United City (what the opening did before it stood is the steady targets' business,
    /// and a pre-gate `chi_idle` says nothing about the exchange), 0 for every other metric.
    pub fn flags_from(&self, rec: &RunRecord) -> f32 {
        let after = match self {
            Metric::AfterGate(_)
            | Metric::ResidentsAt { .. }
            | Metric::BorrowedMinute
            | Metric::FusionMinute
            | Metric::CelestialChi
            | Metric::LongestGapAfterGate => true,
            Metric::Tally(t) => fn_sim::balance::FUSION_TALLIES.iter().any(|&(n, _)| n == t),
            _ => false,
        };
        if after { rec.gate_minute.unwrap_or(0.0) } else { 0.0 }
    }

    /// The minute by which a run must have shown what the target asks for: a sign's deadline, the
    /// minute a reading is taken at, or the end of the run. A flag raised after it doesn't touch
    /// the target.
    pub fn deadline(&self, bar: &Bar, rec: &RunRecord, defs: &Defs) -> f32 {
        let end = rec.minutes;
        match self {
            Metric::SignMinute(_)
            | Metric::ReadyMinute
            | Metric::FirstDeath
            | Metric::StageMinute(_)
            | Metric::SchoolMinute
            | Metric::FirstOfferMinute
            | Metric::GateMinute
            | Metric::BorrowedMinute
            | Metric::FusionMinute => bar.at_most.map_or(end, |m| m as f32).min(end),
            // The bar counts from the gate: the deadline is that long after it.
            Metric::AfterGate(_) => match (rec.gate_minute, bar.at_most) {
                (Some(gate), Some(m)) => (gate + m as f32).min(end),
                _ => end,
            },
            Metric::DeathsBy(m)
            | Metric::PopAt(m)
            | Metric::WoodAt(m)
            | Metric::FoodAt(m)
            | Metric::HerbsAt(m)
            | Metric::CopiesBy(m)
            | Metric::GreatRuinsFoundBy(m)
            | Metric::TeachingsKeptBy(m) => (*m as f32).min(end),
            // Read that long after the gate.
            Metric::ResidentsAt { after, .. } => {
                rec.gate_minute.map_or(end, |gate| (gate + *after as f32).min(end))
            }
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

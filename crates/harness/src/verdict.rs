//! The verdict (`docs/design/balance-harness.md`, section 5; #537): every run of a panel is read
//! against every target that names its style and preset, and lands in exactly one bucket, checked
//! in the doc's order:
//!
//! 1. **Sim lock**: a sim-lock flag fired before the target's deadline.
//! 2. **Harness gap**: a player-health flag fired before it. (A style that plays nothing, `idle`,
//!    has no player to fail, so only the sim's flags apply to it.)
//! 3. **Hard site**: the site profile says the target is infeasible here. Judged against
//!    `hard_expect`, and counted on its own.
//! 4. **Outlier**: a miss on a feasible site, with no flag, where at least [`OUTLIERS_NEED`]
//!    other nations met the bar on the same seed. It counts as a miss and is listed for a look.
//! 5. **Balance**: everything else, pass or miss.
//!
//! A target passes a nation when passes / (balance + outlier runs) reaches its `share`, fails
//! below it, and has no data under `min_runs`. Flagged and hard-site runs never count toward
//! that, and are always listed. Everything here is pure: records in, a [`Verdict`] out.

use std::collections::BTreeMap;

use fn_sim::balance::{Flag, FlagClass, Raised, RunRecord};
use fn_sim::{Defs, Nation};

use crate::config::{Bar, Config, Pair, Target};
use crate::style::Play;

/// How many other nations must meet a target on a seed for a miss there to be an outlier.
pub const OUTLIERS_NEED: usize = 2;

/// One finished run of the panel.
#[derive(Debug, Clone)]
pub struct Run {
    /// The style id (`data/harness.toml`).
    pub style: String,
    /// The difficulty preset id.
    pub preset: String,
    pub record: RunRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bucket {
    SimLock,
    HarnessGap,
    HardSite,
    Outlier,
    Balance,
}

impl Bucket {
    pub fn name(self) -> &'static str {
        match self {
            Bucket::SimLock => "sim lock",
            Bucket::HarnessGap => "harness gap",
            Bucket::HardSite => "hard site",
            Bucket::Outlier => "outlier",
            Bucket::Balance => "balance",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    NoData,
}

/// One run against one target.
#[derive(Debug, Clone, PartialEq)]
pub struct Classified {
    pub nation: Nation,
    pub seed: u64,
    pub bucket: Bucket,
    /// Whether it met the bar that applied to it (the hard bar, on a hard site).
    pub met: bool,
    /// The metric's reading; infinity if the event never happened.
    pub value: f64,
    /// The flags of the bucket's class raised by the deadline (empty for the other buckets).
    pub flags: Vec<Raised>,
    /// The minute the target had to be met by.
    pub deadline: f32,
    /// Other nations that met the standard bar on this seed (an outlier's evidence).
    pub met_by: Vec<Nation>,
    /// The hard-site material and how much of it was in reach, if the target names one.
    pub supply: Option<(String, u32)>,
    /// The minute of the run's first death, if anyone died.
    pub first_death: Option<f32>,
}

/// One nation's column of a target.
#[derive(Debug, Clone, PartialEq)]
pub struct NationVerdict {
    pub nation: Nation,
    /// Balance and outlier runs: the ones that count.
    pub counted: u32,
    pub passes: u32,
    pub status: Status,
    pub sim_locks: u32,
    pub harness_gaps: u32,
    pub outliers: u32,
    /// Runs on hard sites, and how many met the hard bar (all zero without `hard_expect`'s bar
    /// being used).
    pub hard: u32,
    pub hard_passes: u32,
    /// `None` when the target has no `hard_expect`.
    pub hard_status: Option<Status>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetVerdict {
    pub target: Target,
    /// Every nation that ran, in `Nation::ALL`'s order.
    pub nations: Vec<NationVerdict>,
    pub runs: Vec<Classified>,
    /// Fail if any nation fails; no data if any lacks data (and none fails); else pass.
    pub status: Status,
    pub hard_status: Option<Status>,
}

impl TargetVerdict {
    pub fn failing_nations(&self) -> Vec<Nation> {
        self.nations.iter().filter(|n| n.status == Status::Fail).map(|n| n.nation).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairStatus {
    /// The higher side reads above the lower on enough paired runs.
    Separates,
    /// It doesn't: the comparison can't catch a regression (#486).
    CantFail,
    NoData,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PairVerdict {
    pub pair: Pair,
    /// Runs with a reading on both sides and no flag on either.
    pub paired: u32,
    pub separated: u32,
    pub mean_lower: f64,
    pub mean_higher: f64,
    pub status: PairStatus,
}

/// The care rates of one (style, preset), pooled over a nation's runs.
#[derive(Debug, Clone, PartialEq)]
pub struct CareRow {
    pub style: String,
    pub preset: String,
    /// Per nation: depleted minutes per person-hour, close calls per 10 person-hours.
    pub nations: Vec<(Nation, f64, f64)>,
}

/// The seeds on which a nation's supply of a material is below a target's bar for "hard".
#[derive(Debug, Clone, PartialEq)]
pub struct HardSites {
    pub supply: String,
    pub below: f64,
    /// Per nation: the hard seeds, and how many seeds were looked at.
    pub nations: Vec<(Nation, Vec<u64>, usize)>,
}

/// A run that tripped a flag for some target, once however many targets it touched.
#[derive(Debug, Clone, PartialEq)]
pub struct FlaggedRun {
    pub style: String,
    pub preset: String,
    pub nation: Nation,
    pub seed: u64,
    /// The worst bucket any target put it in (a sim lock outranks a harness gap).
    pub bucket: Bucket,
    pub flags: Vec<Raised>,
    pub first_death: Option<f32>,
    pub minutes: f32,
    /// Ids of the targets that excluded it.
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub targets: Vec<TargetVerdict>,
    pub pairs: Vec<PairVerdict>,
    pub care: Vec<CareRow>,
    pub hard_sites: Vec<HardSites>,
    pub flagged: Vec<FlaggedRun>,
    pub runs: usize,
}

impl Verdict {
    pub fn count(&self, status: Status) -> usize {
        self.targets.iter().filter(|t| t.status == status).count()
    }

    /// Outlier runs over every target.
    pub fn outliers(&self) -> Vec<(&TargetVerdict, &Classified)> {
        self.targets
            .iter()
            .flat_map(|t| {
                t.runs.iter().filter(|c| c.bucket == Bucket::Outlier).map(move |c| (t, c))
            })
            .collect()
    }

    /// Whether any target fails (the binary's exit status).
    pub fn failed(&self) -> bool {
        self.count(Status::Fail) > 0
    }
}

/// The flags of `rec` that count against a run of `style`, those of the given class.
fn flags_of(rec: &RunRecord, class: FlagClass, deadline: f32, idle_style: bool) -> Vec<Raised> {
    if idle_style && class == FlagClass::HarnessGap {
        return Vec::new();
    }
    rec.flags.iter().filter(|r| r.flag.class() == class && r.minute <= deadline).copied().collect()
}

fn is_idle(config: &Config, style: &str) -> bool {
    config.styles.get(style).is_some_and(|s| s.play == Play::Idle)
}

/// The material in reach and the sign's need for `nation`: a site is hard when the first is below
/// `below` times the second.
fn is_hard(target: &Target, rec: &RunRecord, defs: &Defs) -> Option<(String, u32, bool)> {
    let h = target.hard_when.as_ref()?;
    let supply = rec.site.supply.get(&h.supply).copied().unwrap_or(0);
    let need = defs
        .endeavor
        .signs
        .iter()
        .find(|s| s.id == h.supply)
        .and_then(|s| s.target_for(rec.nation))
        .unwrap_or(0);
    Some((h.supply.clone(), supply, f64::from(supply) < h.below * f64::from(need)))
}

fn status_of(counted: u32, passes: u32, share: f64, min_runs: u32) -> Status {
    if counted < min_runs.max(1) {
        Status::NoData
    } else if f64::from(passes) >= share * f64::from(counted) - 1e-9 {
        Status::Pass
    } else {
        Status::Fail
    }
}

fn judge_target(config: &Config, target: &Target, runs: &[Run], defs: &Defs) -> TargetVerdict {
    let idle_style = is_idle(config, &target.style);
    let mine: Vec<&Run> = runs
        .iter()
        .filter(|r| r.style == target.style && r.preset == target.preset)
        .filter(|r| {
            target.nations.is_empty()
                || target.nations.iter().any(|n| n == r.record.nation.as_str())
        })
        .collect();

    // Who met the standard bar on each seed, for the outlier rule.
    let mut met_on: BTreeMap<u64, Vec<Nation>> = BTreeMap::new();
    for r in &mine {
        if let Some(v) = target.metric.read(&r.record, defs)
            && target.bar.met(v)
        {
            met_on.entry(r.record.seed).or_default().push(r.record.nation);
        }
    }

    let mut classified = Vec::new();
    for r in &mine {
        let rec = &r.record;
        let hard = is_hard(target, rec, defs);
        let on_hard_site = hard.as_ref().is_some_and(|h| h.2);
        let bar: Bar = match (&target.hard_expect, on_hard_site) {
            (Some(h), true) => h.bar,
            _ => target.bar,
        };
        let deadline = target.metric.deadline(&bar, rec, defs);
        let sim = flags_of(rec, FlagClass::SimLock, deadline, idle_style);
        let gap = flags_of(rec, FlagClass::HarnessGap, deadline, idle_style);
        let reading = target.metric.read(rec, defs);

        let (bucket, flags) = if !sim.is_empty() {
            (Bucket::SimLock, sim)
        } else if !gap.is_empty() {
            (Bucket::HarnessGap, gap)
        } else if reading.is_none() {
            continue;
        } else if on_hard_site {
            (Bucket::HardSite, Vec::new())
        } else {
            (Bucket::Balance, Vec::new())
        };
        let value = reading.unwrap_or(f64::NAN);
        let met = reading.is_some_and(|v| bar.met(v));
        // In `Nation::ALL`'s order, so a report reads the same whichever nation ran first.
        let met_by: Vec<Nation> = Nation::ALL
            .into_iter()
            .filter(|&n| n != rec.nation && met_on.get(&rec.seed).is_some_and(|ns| ns.contains(&n)))
            .collect();
        // An outlier: a plain miss that enough other nations did not share.
        let bucket = if bucket == Bucket::Balance && !met && met_by.len() >= OUTLIERS_NEED {
            Bucket::Outlier
        } else {
            bucket
        };
        classified.push(Classified {
            nation: rec.nation,
            seed: rec.seed,
            bucket,
            met,
            value,
            flags,
            deadline,
            met_by,
            supply: hard.map(|(sign, supply, _)| (sign, supply)),
            first_death: rec.first_death_minute,
        });
    }
    classified.sort_by_key(|c| (Nation::ALL.iter().position(|&n| n == c.nation), c.seed));

    let mut nations = Vec::new();
    for &nation in &Nation::ALL {
        if !mine.iter().any(|r| r.record.nation == nation) {
            continue;
        }
        let of = |b: Bucket| classified.iter().filter(move |c| c.nation == nation && c.bucket == b);
        let balance_passes = of(Bucket::Balance).filter(|c| c.met).count() as u32;
        let balance = of(Bucket::Balance).count() as u32;
        let outliers = of(Bucket::Outlier).count() as u32;
        let counted = balance + outliers;
        let hard = of(Bucket::HardSite).count() as u32;
        let hard_passes = of(Bucket::HardSite).filter(|c| c.met).count() as u32;
        nations.push(NationVerdict {
            nation,
            counted,
            passes: balance_passes,
            status: status_of(counted, balance_passes, target.share, target.min_runs),
            sim_locks: of(Bucket::SimLock).count() as u32,
            harness_gaps: of(Bucket::HarnessGap).count() as u32,
            outliers,
            hard,
            hard_passes,
            hard_status: target
                .hard_expect
                .as_ref()
                .map(|h| status_of(hard, hard_passes, h.share, target.min_runs)),
        });
    }

    let status = if nations.is_empty() || nations.iter().any(|n| n.status == Status::NoData) {
        if nations.iter().any(|n| n.status == Status::Fail) { Status::Fail } else { Status::NoData }
    } else if nations.iter().any(|n| n.status == Status::Fail) {
        Status::Fail
    } else {
        Status::Pass
    };
    let hard_status = target.hard_expect.as_ref().map(|_| {
        let statuses: Vec<Status> = nations.iter().filter_map(|n| n.hard_status).collect();
        if statuses.contains(&Status::Fail) {
            Status::Fail
        } else if statuses.is_empty() || statuses.contains(&Status::NoData) {
            Status::NoData
        } else {
            Status::Pass
        }
    });
    TargetVerdict { target: target.clone(), nations, runs: classified, status, hard_status }
}

fn judge_pair(config: &Config, pair: &Pair, runs: &[Run], defs: &Defs) -> PairVerdict {
    let side = |style: &str, preset: &str| -> BTreeMap<(usize, u64), f64> {
        let idle_style = is_idle(config, style);
        runs.iter()
            .filter(|r| r.style == style && r.preset == preset)
            .filter(|r| {
                let sim = flags_of(&r.record, FlagClass::SimLock, r.record.minutes, idle_style);
                let gap = flags_of(&r.record, FlagClass::HarnessGap, r.record.minutes, idle_style);
                sim.is_empty() && gap.is_empty()
            })
            .filter_map(|r| {
                let nation = Nation::ALL.iter().position(|&n| n == r.record.nation)?;
                Some(((nation, r.record.seed), pair.metric.read(&r.record, defs)?))
            })
            .collect()
    };
    let lower = side(&pair.lower.style, &pair.lower.preset);
    let higher = side(&pair.higher.style, &pair.higher.preset);
    let (mut paired, mut separated) = (0u32, 0u32);
    let (mut sum_lower, mut sum_higher) = (0.0, 0.0);
    for (key, &lo) in &lower {
        if let Some(&hi) = higher.get(key) {
            paired += 1;
            sum_lower += lo;
            sum_higher += hi;
            if hi > lo {
                separated += 1;
            }
        }
    }
    let status = if paired < config.panel.min_runs.max(1) {
        PairStatus::NoData
    } else if f64::from(separated) >= pair.share * f64::from(paired) - 1e-9 {
        PairStatus::Separates
    } else {
        PairStatus::CantFail
    };
    let mean = |sum: f64| if paired > 0 { sum / f64::from(paired) } else { 0.0 };
    PairVerdict {
        pair: pair.clone(),
        paired,
        separated,
        mean_lower: mean(sum_lower),
        mean_higher: mean(sum_higher),
        status,
    }
}

fn care_rows(runs: &[Run]) -> Vec<CareRow> {
    let mut combos: Vec<(String, String)> = Vec::new();
    for r in runs {
        let key = (r.style.clone(), r.preset.clone());
        if !combos.contains(&key) {
            combos.push(key);
        }
    }
    combos
        .into_iter()
        .map(|(style, preset)| {
            let mut nations = Vec::new();
            for &nation in &Nation::ALL {
                let mine: Vec<&Run> = runs
                    .iter()
                    .filter(|r| r.style == style && r.preset == preset && r.record.nation == nation)
                    .collect();
                if mine.is_empty() {
                    continue;
                }
                let minutes: f64 = mine.iter().map(|r| r.record.care.person_minutes).sum();
                let depleted: f64 =
                    mine.iter().map(|r| r.record.care.depleted_minutes.iter().sum::<f64>()).sum();
                let calls: f64 = mine.iter().map(|r| f64::from(r.record.care.close_calls)).sum();
                let hours = minutes / 60.0;
                let (d, c) =
                    if hours > 0.0 { (depleted / hours, calls * 10.0 / hours) } else { (0.0, 0.0) };
                nations.push((nation, d, c));
            }
            CareRow { style, preset, nations }
        })
        .collect()
}

fn hard_sites(config: &Config, runs: &[Run], defs: &Defs) -> Vec<HardSites> {
    let mut out: Vec<HardSites> = Vec::new();
    for t in &config.targets {
        let Some(h) = &t.hard_when else { continue };
        if out.iter().any(|o| o.supply == h.supply && o.below == h.below) {
            continue;
        }
        let need = |nation: Nation| {
            defs.endeavor
                .signs
                .iter()
                .find(|s| s.id == h.supply)
                .and_then(|s| s.target_for(nation))
                .unwrap_or(0)
        };
        let mut nations = Vec::new();
        for &nation in &Nation::ALL {
            // A site profile doesn't depend on style or preset: one run per seed says it all.
            let mut by_seed: BTreeMap<u64, u32> = BTreeMap::new();
            for r in runs.iter().filter(|r| r.record.nation == nation) {
                by_seed
                    .entry(r.record.seed)
                    .or_insert_with(|| r.record.site.supply.get(&h.supply).copied().unwrap_or(0));
            }
            if by_seed.is_empty() {
                continue;
            }
            let hard: Vec<u64> = by_seed
                .iter()
                .filter(|&(_, &supply)| f64::from(supply) < h.below * f64::from(need(nation)))
                .map(|(&seed, _)| seed)
                .collect();
            nations.push((nation, hard, by_seed.len()));
        }
        out.push(HardSites { supply: h.supply.clone(), below: h.below, nations });
    }
    out
}

fn flagged_runs(verdicts: &[TargetVerdict], runs: &[Run]) -> Vec<FlaggedRun> {
    let mut map: BTreeMap<(String, String, usize, u64), FlaggedRun> = BTreeMap::new();
    for tv in verdicts {
        for c in tv.runs.iter().filter(|c| matches!(c.bucket, Bucket::SimLock | Bucket::HarnessGap))
        {
            let nation = Nation::ALL.iter().position(|&n| n == c.nation).unwrap_or(0);
            let key = (tv.target.style.clone(), tv.target.preset.clone(), nation, c.seed);
            let entry = map.entry(key).or_insert_with(|| {
                let minutes = runs
                    .iter()
                    .find(|r| {
                        r.style == tv.target.style
                            && r.preset == tv.target.preset
                            && r.record.nation == c.nation
                            && r.record.seed == c.seed
                    })
                    .map_or(0.0, |r| r.record.minutes);
                FlaggedRun {
                    style: tv.target.style.clone(),
                    preset: tv.target.preset.clone(),
                    nation: c.nation,
                    seed: c.seed,
                    bucket: c.bucket,
                    flags: Vec::new(),
                    first_death: c.first_death,
                    minutes,
                    targets: Vec::new(),
                }
            });
            entry.bucket = entry.bucket.min(c.bucket);
            for f in &c.flags {
                if !entry.flags.iter().any(|e| e.flag == f.flag) {
                    entry.flags.push(*f);
                }
            }
            entry.targets.push(tv.target.id.clone());
        }
    }
    let order = |f: &Raised| Flag::ALL.iter().position(|&x| x == f.flag);
    map.into_values()
        .map(|mut f| {
            f.flags.sort_by_key(order);
            f
        })
        .collect()
}

/// Reads every run against every target and pair in `config` (module doc).
pub fn judge(config: &Config, runs: &[Run], defs: &Defs) -> Verdict {
    let targets: Vec<TargetVerdict> =
        config.targets.iter().map(|t| judge_target(config, t, runs, defs)).collect();
    let pairs = config.pairs.iter().map(|p| judge_pair(config, p, runs, defs)).collect();
    let flagged = flagged_runs(&targets, runs);
    Verdict {
        pairs,
        care: care_rows(runs),
        hard_sites: hard_sites(config, runs, defs),
        flagged,
        targets,
        runs: runs.len(),
    }
}

#[cfg(test)]
mod tests;

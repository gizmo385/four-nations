//! The statistics of the hill-climb (#571, docs/design/agent-hillclimb.md, "Measuring a
//! difference"): a paired comparison of two strategies on the same seeds, with a bootstrap
//! interval for the mean difference, failure counts reported beside it, and the promotion rule
//! the campaign applies to the gate tier.
//!
//! Everything here is deterministic. The bootstrap draws from a fixed-seed generator
//! ([`BOOTSTRAP_SEED`]) that restarts for every call, so the same scores give the same interval
//! on every machine and every thread count, and the same candidate is judged the same way twice.
//! xtask is not `fn_sim`, but the rule is still easier to trust, and to test, this way.
//!
//! **The promotion rule** ([`judge`]). A candidate replaces the champion when both hold on the
//! gate seeds, paired by seed:
//!
//! 1. the [`GATE_LEVEL`] bootstrap interval of the mean difference (candidate minus champion) lies
//!    wholly above zero: its lower end, the 2.5th percentile of the resampled means, is positive;
//! 2. the candidate does not collapse on more seeds than the champion does (a collapse is a score
//!    below the track's `collapse_below`).
//!
//! The first condition asks for a gain that holds across the seeds, not one that hangs on a
//! single seed: with one seed carrying all of a difference, a resample that leaves that seed out
//! has a mean of zero, so the lower end is zero and the candidate is not promoted. The second
//! stops a candidate from buying a higher mean with a new way to fail, which the mean of a
//! bimodal score is blind to. Deaths and stopped scripts are counted and reported, and decide
//! nothing by themselves: a death already costs score, and a governor that stops is a collapse
//! by another name.

use super::Row;

/// The fixed seed of the bootstrap's generator.
pub(crate) const BOOTSTRAP_SEED: u64 = 0x5EED_0571;

/// Resamples per interval.
pub(crate) const RESAMPLES: usize = 2000;

/// The two-sided level of the interval the gate uses: its lower end is the 2.5th percentile, so a
/// candidate is promoted only when 97.5% of the resampled mean differences are above zero.
pub(crate) const GATE_LEVEL: f64 = 0.95;

/// The level of the intervals `agent-compare` prints.
pub(crate) const REPORT_LEVEL: f64 = 0.95;

/// SplitMix64: a small generator with a good spread, std only.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform on [0, 1). Only the synthetic-data tests draw these.
    #[cfg(test)]
    pub(crate) fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform on 0..n (the modulo bias at these sizes is far below anything measured).
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// Standard normal (Box-Muller).
    #[cfg(test)]
    pub(crate) fn normal(&mut self) -> f64 {
        let u = 1.0 - self.unit();
        let v = self.unit();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

pub(crate) fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

/// Sample standard deviation (n - 1); 0 for fewer than two values.
pub(crate) fn sd(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let m = mean(xs);
    (xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() - 1) as f64).sqrt()
}

/// The `q`-quantile (0..=1) of an ascending list, taking the element at floor(q * n).
fn quantile(sorted: &[f64], q: f64) -> f64 {
    let at = ((q * sorted.len() as f64).floor() as usize).min(sorted.len() - 1);
    sorted[at]
}

/// A mean with its percentile-bootstrap interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Interval {
    pub(crate) mean: f64,
    pub(crate) lo: f64,
    pub(crate) hi: f64,
    pub(crate) level: f64,
}

impl Interval {
    /// Whether the whole interval is above zero.
    pub(crate) fn above_zero(&self) -> bool {
        self.lo > 0.0
    }

    /// `+12.3 [+4.0, +20.1]`.
    pub(crate) fn text(&self) -> String {
        format!("{:+.1} [{:+.1}, {:+.1}]", self.mean, self.lo, self.hi)
    }
}

/// The percentile bootstrap of the mean of `xs` at the two-sided `level`, `RESAMPLES` resamples
/// from the fixed generator. `None` for no values.
pub(crate) fn bootstrap(xs: &[f64], level: f64) -> Option<Interval> {
    if xs.is_empty() {
        return None;
    }
    let mut rng = Rng::new(BOOTSTRAP_SEED);
    let n = xs.len();
    let mut means: Vec<f64> =
        (0..RESAMPLES).map(|_| (0..n).map(|_| xs[rng.below(n)]).sum::<f64>() / n as f64).collect();
    means.sort_by(f64::total_cmp);
    let tail = (1.0 - level) / 2.0;
    Some(Interval {
        mean: mean(xs),
        lo: quantile(&means, tail),
        hi: quantile(&means, 1.0 - tail),
        level,
    })
}

/// One seed's result, in the terms the comparison needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sample {
    pub(crate) seed: u64,
    pub(crate) score: f64,
    pub(crate) deaths: u32,
    /// The script was stopped by an error or a runaway.
    pub(crate) stopped: bool,
}

impl From<&Row> for Sample {
    fn from(r: &Row) -> Self {
        Self { seed: r.seed, score: r.score, deaths: r.deaths, stopped: r.stopped.is_some() }
    }
}

/// How a strategy failed on a set of seeds. Reported beside the mean, never folded into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Failures {
    pub(crate) seeds: usize,
    /// Seeds scoring below the track's `collapse_below`.
    pub(crate) collapses: usize,
    /// Deaths in all, and the seeds with at least one.
    pub(crate) deaths: u32,
    pub(crate) death_seeds: usize,
    /// Seeds whose script was stopped.
    pub(crate) stopped: usize,
}

impl Failures {
    pub(crate) fn of(samples: &[Sample], collapse_below: f64) -> Self {
        Self {
            seeds: samples.len(),
            collapses: samples.iter().filter(|s| s.score < collapse_below).count(),
            deaths: samples.iter().map(|s| s.deaths).sum(),
            death_seeds: samples.iter().filter(|s| s.deaths > 0).count(),
            stopped: samples.iter().filter(|s| s.stopped).count(),
        }
    }

    /// `collapse 2/12 · deaths 5 on 3 seeds · stopped 1`.
    pub(crate) fn text(&self) -> String {
        format!(
            "collapse {}/{} · deaths {} on {} seed(s) · stopped {}",
            self.collapses, self.seeds, self.deaths, self.death_seeds, self.stopped
        )
    }
}

/// A candidate against a champion on the same seeds.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Verdict {
    pub(crate) seeds: usize,
    pub(crate) candidate_mean: f64,
    pub(crate) champion_mean: f64,
    /// Candidate minus champion, per seed, with the interval of its mean.
    pub(crate) diff: Interval,
    pub(crate) wins: usize,
    pub(crate) losses: usize,
    pub(crate) ties: usize,
    pub(crate) candidate: Failures,
    pub(crate) champion: Failures,
    pub(crate) promote: bool,
    /// One sentence on why, for the ledger and the owner.
    pub(crate) reason: String,
}

/// Pairs `a` and `b` by seed, in `a`'s order. An error when their seeds differ.
pub(crate) fn pair(a: &[Sample], b: &[Sample]) -> Result<Vec<(Sample, Sample)>, String> {
    let mut out = Vec::with_capacity(a.len());
    for x in a {
        let Some(y) = b.iter().find(|y| y.seed == x.seed) else {
            return Err(format!("seed {} is in one list and not the other", x.seed));
        };
        out.push((*x, *y));
    }
    if let Some(y) = b.iter().find(|y| !a.iter().any(|x| x.seed == y.seed)) {
        return Err(format!("seed {} is in one list and not the other", y.seed));
    }
    Ok(out)
}

/// The paired difference `a - b` on their common seeds, with the interval at `level`, and the
/// win, loss and tie counts. An error when the seeds differ or there are none.
pub(crate) fn difference(
    a: &[Sample],
    b: &[Sample],
    level: f64,
) -> Result<(Interval, usize, usize, usize), String> {
    let pairs = pair(a, b)?;
    let diffs: Vec<f64> = pairs.iter().map(|(x, y)| x.score - y.score).collect();
    let interval = bootstrap(&diffs, level).ok_or("no seeds to compare")?;
    let wins = diffs.iter().filter(|d| **d > 0.0).count();
    let losses = diffs.iter().filter(|d| **d < 0.0).count();
    Ok((interval, wins, losses, diffs.len() - wins - losses))
}

/// The promotion rule (see the module doc) for `candidate` against `champion` on the gate seeds.
pub(crate) fn judge(
    candidate: &[Sample],
    champion: &[Sample],
    collapse_below: f64,
) -> Result<Verdict, String> {
    let (diff, wins, losses, ties) = difference(candidate, champion, GATE_LEVEL)?;
    let cand_fail = Failures::of(candidate, collapse_below);
    let champ_fail = Failures::of(champion, collapse_below);
    let clear = diff.above_zero();
    let no_new_collapse = cand_fail.collapses <= champ_fail.collapses;
    let promote = clear && no_new_collapse;
    let reason = if promote {
        format!(
            "the {:.0}% interval of the gain, {}, is above zero and collapses are {} against {}",
            GATE_LEVEL * 100.0,
            diff.text(),
            cand_fail.collapses,
            champ_fail.collapses
        )
    } else if !clear {
        format!(
            "the {:.0}% interval of the gain, {}, reaches zero or below",
            GATE_LEVEL * 100.0,
            diff.text()
        )
    } else {
        format!(
            "the gain, {}, is clear but the candidate collapses on {} seed(s) against the \
             champion's {}",
            diff.text(),
            cand_fail.collapses,
            champ_fail.collapses
        )
    };
    Ok(Verdict {
        seeds: candidate.len(),
        candidate_mean: mean(&candidate.iter().map(|s| s.score).collect::<Vec<_>>()),
        champion_mean: mean(&champion.iter().map(|s| s.score).collect::<Vec<_>>()),
        diff,
        wins,
        losses,
        ties,
        candidate: cand_fail,
        champion: champ_fail,
        promote,
        reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(scores: &[f64]) -> Vec<Sample> {
        scores
            .iter()
            .enumerate()
            .map(|(i, &score)| Sample { seed: 100 + i as u64, score, deaths: 0, stopped: false })
            .collect()
    }

    #[test]
    fn the_bootstrap_is_deterministic_and_brackets_the_mean() {
        let xs = [10.0, 20.0, 30.0, 40.0, 50.0, 5.0, 80.0];
        let a = bootstrap(&xs, 0.9).unwrap();
        assert_eq!(a, bootstrap(&xs, 0.9).unwrap(), "same input, same interval");
        assert!((a.mean - mean(&xs)).abs() < 1e-12);
        assert!(a.lo < a.mean && a.mean < a.hi, "{a:?}");
        let wide = bootstrap(&xs, 0.99).unwrap();
        assert!(wide.lo <= a.lo && wide.hi >= a.hi);
        assert_eq!(bootstrap(&[], 0.9), None);
        // A constant difference has a zero-width interval.
        let c = bootstrap(&[7.0; 5], 0.9).unwrap();
        assert_eq!((c.lo, c.hi), (7.0, 7.0));
    }

    #[test]
    fn mean_and_sd() {
        assert_eq!(mean(&[1.0, 2.0, 3.0]), 2.0);
        assert!((sd(&[1.0, 2.0, 3.0]) - 1.0).abs() < 1e-12);
        assert_eq!(sd(&[5.0]), 0.0);
    }

    #[test]
    fn the_generator_is_a_fixed_sequence() {
        let mut r = Rng::new(1);
        // SplitMix64's published first outputs for seed 1.
        assert_eq!(r.next_u64(), 0x910A_2DEC_8902_5CC1);
        assert_eq!(r.next_u64(), 0xBEEB_8DA1_658E_EC67);
    }

    #[test]
    fn pairing_is_by_seed_and_refuses_mismatched_sets() {
        let a = samples(&[1.0, 2.0, 3.0]);
        let mut b = samples(&[10.0, 20.0, 30.0]);
        b.reverse();
        let (iv, wins, losses, ties) = difference(&a, &b, 0.9).unwrap();
        assert_eq!((wins, losses, ties), (0, 3, 0));
        assert!((iv.mean + 18.0).abs() < 1e-12, "{iv:?}");
        let short = samples(&[1.0, 2.0]);
        assert!(difference(&a, &short, 0.9).unwrap_err().contains("seed"));
        assert!(difference(&short, &a, 0.9).unwrap_err().contains("seed"));
    }

    #[test]
    fn a_clear_and_even_gain_is_promoted() {
        let champ = samples(&[300.0, 310.0, 290.0, 320.0, 305.0, 295.0, 300.0, 315.0]);
        let cand: Vec<f64> = champ.iter().map(|s| s.score + 40.0).collect();
        let v = judge(&samples(&cand), &champ, 120.0).unwrap();
        assert!(v.promote, "{v:?}");
        assert_eq!((v.wins, v.losses, v.ties), (8, 0, 0));
        assert!(v.reason.contains("above zero"), "{}", v.reason);
    }

    #[test]
    fn no_gain_and_a_loss_are_not_promoted() {
        let champ = samples(&[300.0, 310.0, 290.0, 320.0, 305.0, 295.0]);
        let same = judge(&champ, &champ, 120.0).unwrap();
        assert!(!same.promote && same.ties == 6, "equal scores never replace the champion");
        let worse: Vec<f64> = champ.iter().map(|s| s.score - 5.0).collect();
        assert!(!judge(&samples(&worse), &champ, 120.0).unwrap().promote);
    }

    /// The bimodal case: the mean difference of +25 comes from one seed (the champion collapsed
    /// there and the candidate did not). A third of the resamples leave that seed out, so the
    /// lower end of the interval is zero and the candidate is not promoted.
    #[test]
    fn a_gain_that_hangs_on_one_seed_is_not_promoted() {
        let mut champ = vec![300.0; 12];
        champ[3] = 5.0;
        let mut cand = vec![300.0; 12];
        cand[3] = 305.0;
        let v = judge(&samples(&cand), &samples(&champ), 120.0).unwrap();
        assert!((v.diff.mean - 25.0).abs() < 1e-9);
        assert_eq!(v.diff.lo, 0.0);
        assert!(!v.promote, "{}", v.reason);
        assert_eq!((v.wins, v.ties), (1, 11));
        assert_eq!((v.champion.collapses, v.candidate.collapses), (1, 0));
    }

    /// How spread out a gain has to be: +100 on three of twelve seeds (a mean gain of +25) is not
    /// promoted, +100 on four (+33) is. The design doc quotes "about four of twelve".
    #[test]
    fn a_gain_has_to_be_spread_over_about_four_of_twelve_seeds() {
        let champ = samples(&[300.0; 12]);
        let gain_on = |n: usize| {
            let cand: Vec<f64> = (0..12).map(|i| if i < n { 400.0 } else { 300.0 }).collect();
            judge(&samples(&cand), &champ, 120.0).unwrap()
        };
        assert!(!gain_on(1).promote && !gain_on(2).promote && !gain_on(3).promote);
        assert!(gain_on(4).promote, "{:?}", gain_on(4));
        assert!(gain_on(8).promote);
    }

    /// A higher mean bought with a new collapse is not promoted, however clear the gain.
    #[test]
    fn a_new_collapse_blocks_a_clear_gain() {
        let champ = samples(&[200.0; 12]);
        let mut cand = vec![260.0; 12];
        cand[0] = 60.0;
        let v = judge(&samples(&cand), &champ, 120.0).unwrap();
        assert!(v.diff.above_zero(), "the mean is up and the interval says so: {v:?}");
        assert_eq!((v.candidate.collapses, v.champion.collapses), (1, 0));
        assert!(!v.promote);
        assert!(v.reason.contains("collapses on 1"), "{}", v.reason);
        // Tied on collapses is fine.
        let mut champ2 = vec![200.0; 12];
        champ2[0] = 50.0;
        assert!(judge(&samples(&cand), &samples(&champ2), 120.0).unwrap().promote);
    }

    #[test]
    fn failures_count_collapses_deaths_and_stops() {
        let mut s = samples(&[500.0, 100.0, 3.0, 119.9, 120.0]);
        s[1].deaths = 2;
        s[2].deaths = 3;
        s[2].stopped = true;
        let f = Failures::of(&s, 120.0);
        assert_eq!(
            f,
            Failures { seeds: 5, collapses: 3, deaths: 5, death_seeds: 2, stopped: 1 },
            "120.0 itself is not below the threshold"
        );
        assert_eq!(f.text(), "collapse 3/5 · deaths 5 on 2 seed(s) · stopped 1");
    }

    // ---- How often the rule promotes on synthetic data (#571) -----------------------------
    //
    // The evidence (`~/fn-agent-reviews/20261108/dist.txt`) puts the standard deviation of a
    // paired difference on one seed at about 74 and says about a fifth of seeds collapse. Two
    // models of a no-difference candidate and a candidate that is better by `shift` on every seed:
    //
    // - `normal`: the difference on each seed is Normal(shift, 74), the 12 seeds independent.
    // - `bimodal`: each seed has a seed effect shared by both strategies (4 in 5 around 300, 1 in 5
    //   collapsed around 60), and each strategy adds its own noise of sd 52 (so the difference has
    //   sd 74); the candidate also gains `shift`. Collapse counts are then compared as in `judge`.

    #[derive(Clone, Copy)]
    enum Model {
        Normal,
        Bimodal,
    }

    fn draw(model: Model, rng: &mut Rng, seeds: usize, shift: f64) -> (Vec<Sample>, Vec<Sample>) {
        let mut cand = Vec::new();
        let mut champ = Vec::new();
        for i in 0..seeds {
            let seed = 1000 + i as u64;
            let (c, h) = match model {
                Model::Normal => (shift + 74.0 * rng.normal(), 0.0),
                Model::Bimodal => {
                    let base = if rng.unit() < 0.2 {
                        60.0 + 40.0 * rng.normal()
                    } else {
                        300.0 + 60.0 * rng.normal()
                    };
                    (base + shift + 52.0 * rng.normal(), base + 52.0 * rng.normal())
                }
            };
            // The normal model works in differences, far enough above the threshold that nothing
            // "collapses": it tests the interval rule alone.
            let (c, h) = if matches!(model, Model::Normal) { (c + 5000.0, 5000.0) } else { (c, h) };
            cand.push(Sample { seed, score: c, deaths: 0, stopped: false });
            champ.push(Sample { seed, score: h, deaths: 0, stopped: false });
        }
        (cand, champ)
    }

    /// The share of `trials` synthetic candidates `judge` would promote.
    fn promote_rate(model: Model, seeds: usize, shift: f64, trials: usize) -> f64 {
        let mut rng = Rng::new(0xC0FF_EE00 ^ (shift.to_bits() >> 7));
        let promoted = (0..trials)
            .filter(|_| {
                let (cand, champ) = draw(model, &mut rng, seeds, shift);
                judge(&cand, &champ, 120.0).unwrap().promote
            })
            .count();
        promoted as f64 / trials as f64
    }

    /// The figures in the design doc, at fewer trials so the suite stays quick; the doc's come
    /// from `power_table` (ignored; run it with `--release -- --ignored --nocapture`).
    #[test]
    fn the_gate_rarely_promotes_nothing_and_usually_promotes_a_big_gain() {
        for model in [Model::Normal, Model::Bimodal] {
            let null = promote_rate(model, 12, 0.0, 600);
            assert!(null < 0.10, "no-difference candidates promoted {null}");
            let big = promote_rate(model, 12, 100.0, 600);
            assert!(big > 0.85, "a +100 gain promoted only {big}");
            let more = promote_rate(model, 12, 60.0, 600);
            assert!(more > null && big > more, "{null} < {more} < {big}");
        }
        // More seeds find smaller gains: the point of the gate being larger than the screen.
        assert!(
            promote_rate(Model::Normal, 30, 40.0, 600) > promote_rate(Model::Normal, 12, 40.0, 600)
        );
    }

    #[test]
    #[ignore = "prints the design doc's power table; run with --release -- --ignored --nocapture"]
    fn power_table() {
        let trials = 10_000;
        for (name, model) in [("normal", Model::Normal), ("bimodal", Model::Bimodal)] {
            for seeds in [3, 12, 30] {
                let row: Vec<String> = [0.0, 20.0, 40.0, 60.0, 80.0, 100.0, 125.0]
                    .iter()
                    .map(|&s| {
                        format!("{s:>4}: {:>5.1}%", 100.0 * promote_rate(model, seeds, s, trials))
                    })
                    .collect();
                println!("{name:>7} n={seeds:<2}  {}", row.join("  "));
            }
        }
    }
}

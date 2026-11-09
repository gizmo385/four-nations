//! Which governor versions of an attempt get a clean evaluation, and which one the attempt
//! hands on (#570, docs/design/agent-hillclimb.md, "Across runs: attempts and a champion").
//!
//! Only the last `governor` an attempt saved used to be evaluated. A model that tests as it
//! goes, and then edits blind after its best test, lost the best version it had: attempt 2 of the
//! first campaign tested versions near 142 / 152 / 8 and saved a v22 that scored 131 / 142 / 0.
//!
//! **A candidate is a pair**, a governor and the plan it is judged with, because the clean
//! evaluation runs both and a governor reads the plan (`plan()`). The pairs are the states the
//! model itself held, read off the run log:
//! - for each saved version, the plan as it stood when the model moved on from it (the next
//!   `save_script` of the governor), or at the end of the attempt for the last: the model's
//!   plan edits usually follow a save, so the plan at the end of a version's turn is the one it
//!   was fitted to;
//! - and the plan at each `test_governor` call, for the version current then: those are the
//!   pairs the model was shown a score for.
//!
//! Pairs with the same governor code and the same plan are one pair, and the later version's
//! number names it.
//!
//! **The cost is bounded, and the order matters (#571).** A pair is the three screen seeds,
//! about 30 s for the first campaign's champion governor and 3 to 4 minutes for a heavy one, and
//! the runs of all the pairs share one parallel queue. Only the best pair by screen mean runs
//! the 12 gate seeds, so the gate costs one pair's worth, not twelve. At most
//! [`CANDIDATE_CAP`] pairs are screened: the final pair always, then the tested pairs, newest
//! first, then the untested ones, newest first. Below the cap every saved version is screened;
//! above it the oldest untested ones are not, and the ledger's `tried` and `saved` columns say
//! so.
//!
//! **Choosing the pair is on the screen mean alone.** [`best`] takes screen means and nothing
//! else, so no gate or owner-only score can reach it. The screen only picks which pair goes to
//! the gate; whether it replaces the champion is the gate's call (`paired::judge`), made on
//! seeds the choice of the pair never saw, which is what keeps picking the best of twelve on
//! three seeds from inflating the comparison. The owner-only tier is run for a promoted pair
//! afterwards, and decides nothing.

use serde_json::Value;

use super::LogLine;

const GOVERNOR: &str = "governor";
const TEST_TOOL: &str = "test_governor";

/// The most pairs one attempt evaluates. At about half a minute each for a usual governor, six
/// minutes of CPU; before #571 the seeds run one at a time.
pub(super) const CANDIDATE_CAP: usize = 12;

/// A governor version and the plan it is judged with.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Candidate {
    pub(super) version: u32,
    pub(super) code: String,
    /// The plan, `None` when it was empty or never set.
    pub(super) plan: Option<Value>,
    /// `plan` lines in the log up to this pair (the preloaded plan of a carried-in strategy
    /// counts as one).
    pub(super) plan_edits: u32,
    /// The model called `test_governor` while this pair was current.
    pub(super) tested: bool,
    /// The state the attempt ended in: what the clean evaluation used to take.
    pub(super) last: bool,
}

/// Every distinct pair in `lines`, in log order, and how many versions of the governor were
/// saved (`save_script` lines, a carried-in preload included: it is the first one).
pub(super) fn find(lines: &[LogLine]) -> (Vec<Candidate>, u32) {
    let mut pairs: Vec<Candidate> = Vec::new();
    let mut now: Option<(u32, &str)> = None;
    let mut plan: Option<Value> = None;
    let mut edits = 0u32;
    let mut saved = 0u32;
    let note = |pairs: &mut Vec<Candidate>,
                now: Option<(u32, &str)>,
                plan: &Option<Value>,
                edits,
                tested| {
        let Some((version, code)) = now else { return };
        let same = |p: &Candidate| p.code == code && &p.plan == plan;
        match pairs.iter().position(same) {
            Some(at) => {
                let mut p = pairs.remove(at);
                p.version = version;
                p.plan_edits = edits;
                p.tested |= tested;
                pairs.push(p);
            }
            None => pairs.push(Candidate {
                version,
                code: code.to_owned(),
                plan: plan.clone(),
                plan_edits: edits,
                tested,
                last: false,
            }),
        }
    };
    for line in lines {
        match line {
            // A saved script is a `script` line that ran no turn (version 0 is an inline run).
            LogLine::Script { name, version, code, turns } if name == GOVERNOR && *turns == 0 => {
                if *version >= 1 {
                    note(&mut pairs, now, &plan, edits, false);
                    now = Some((*version, code));
                    saved += 1;
                }
            }
            LogLine::Plan { plan: p } => {
                edits += 1;
                plan = p.as_object().is_some_and(|m| !m.is_empty()).then(|| p.clone());
            }
            LogLine::Call { tool } if tool == TEST_TOOL => {
                note(&mut pairs, now, &plan, edits, true);
            }
            _ => {}
        }
    }
    note(&mut pairs, now, &plan, edits, false);
    // The final state is the last pair noted (it was moved to the back), whatever else merged.
    if let Some(p) = pairs.last_mut() {
        p.last = true;
    }
    (pairs, saved)
}

/// Whether the first governor save in `lines` is the champion's own governor, preloaded by
/// `serve --carry-in` before the first call: it is in the log like a save but is not one the
/// attempt made, so it is not counted in the ledger's `saved`.
pub(super) fn starts_from(lines: &[LogLine], champion_code: &str) -> bool {
    lines.iter().find_map(|l| match l {
        LogLine::Script { name, version, code, turns: 0 } if name == GOVERNOR && *version >= 1 => {
            Some(code == champion_code)
        }
        _ => None,
    }) == Some(true)
}

/// The pairs to evaluate: at most `cap`, the final one always, then the tested ones and then
/// the untested ones, newest first within each; returned in log order. A pair equal to `skip`
/// (the champion's own, which a carried-in attempt starts from) is left out unless it is the
/// final pair, since its score is the champion's.
pub(super) fn pick(
    pairs: Vec<Candidate>,
    cap: usize,
    skip: Option<(&str, &Option<Value>)>,
) -> Vec<Candidate> {
    let mut order: Vec<usize> = (0..pairs.len()).collect();
    let rank = |p: &Candidate| {
        if p.last {
            0
        } else if p.tested {
            1
        } else {
            2
        }
    };
    order.retain(|&i| {
        let p = &pairs[i];
        p.last || skip.is_none_or(|(code, plan)| p.code != code || &p.plan != plan)
    });
    order.sort_by_key(|&i| (rank(&pairs[i]), std::cmp::Reverse(i)));
    order.truncate(cap);
    order.sort_unstable();
    order.into_iter().map(|i| pairs[i].clone()).collect()
}

/// The index of the best screen mean, a later one winning a tie (a later pair is the one the
/// model ended up with, and the champion test needs a strict gain anyway). `None` for no
/// numbers. Takes screen means only, so the choice can't see a gate or owner-only score.
pub(super) fn best(screen_means: &[Option<f64>]) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, m) in screen_means.iter().enumerate() {
        if let Some(m) = m
            && best.is_none_or(|(_, b)| *m >= b)
        {
            best = Some((i, *m));
        }
    }
    best.map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn save(v: u32, code: &str) -> LogLine {
        LogLine::Script { name: GOVERNOR.into(), version: v, code: code.into(), turns: 0 }
    }
    fn plan(p: Value) -> LogLine {
        LogLine::Plan { plan: p }
    }
    fn test() -> LogLine {
        LogLine::Call { tool: TEST_TOOL.into() }
    }

    #[test]
    fn a_version_is_paired_with_the_plan_it_ended_with_and_the_plans_it_was_tested_with() {
        let lines = vec![
            save(1, "a"),
            plan(json!({"k": 1})),
            test(),
            plan(json!({"k": 2})),
            save(2, "b"),
            plan(json!({"k": 3})),
        ];
        let (pairs, saved) = find(&lines);
        assert_eq!(saved, 2);
        let got: Vec<(u32, Option<Value>, bool, bool)> =
            pairs.iter().map(|p| (p.version, p.plan.clone(), p.tested, p.last)).collect();
        assert_eq!(
            got,
            vec![
                // v1 tested with plan 1 ...
                (1, Some(json!({"k": 1})), true, false),
                // ... and moved on from with plan 2, which it was not tested with.
                (1, Some(json!({"k": 2})), false, false),
                (2, Some(json!({"k": 3})), false, true),
            ]
        );
        assert_eq!(pairs[2].plan_edits, 3);
    }

    #[test]
    fn identical_governor_and_plan_are_one_pair_named_by_the_later_version() {
        let lines = vec![save(1, "a"), plan(json!({"k": 1})), test(), save(2, "a"), test()];
        let (pairs, saved) = find(&lines);
        assert_eq!(saved, 2);
        assert_eq!(pairs.len(), 1);
        assert_eq!((pairs[0].version, pairs[0].tested, pairs[0].last), (2, true, true));
    }

    #[test]
    fn no_governor_no_pairs_and_an_empty_plan_is_no_plan() {
        assert_eq!(find(&[plan(json!({})), test()]), (Vec::new(), 0));
        let (pairs, _) = find(&[save(1, "a"), plan(json!({"k": 1})), plan(json!({}))]);
        assert_eq!(pairs[0].plan, None);
        // Inline runs (version 0) and other scripts' saves are not the governor.
        let other = LogLine::Script { name: "aux".into(), version: 3, code: "x".into(), turns: 0 };
        let run = LogLine::Script { name: GOVERNOR.into(), version: 0, code: "y".into(), turns: 5 };
        let (pairs, saved) = find(&[other, run, save(1, "a")]);
        assert_eq!((pairs.len(), saved), (1, 1));
    }

    #[test]
    fn the_cap_keeps_the_final_pair_then_tested_then_newest() {
        // Versions 1..=10, v3 and v6 tested; the last is v10.
        let mut lines = Vec::new();
        for v in 1..=10 {
            lines.push(save(v, &format!("g{v}")));
            if v == 3 || v == 6 {
                lines.push(test());
            }
        }
        let (pairs, saved) = find(&lines);
        assert_eq!((pairs.len(), saved), (10, 10));
        let versions =
            |cap| pick(pairs.clone(), cap, None).iter().map(|p| p.version).collect::<Vec<_>>();
        assert_eq!(versions(12), (1..=10).collect::<Vec<_>>(), "below the cap: every version");
        assert_eq!(versions(1), vec![10]);
        assert_eq!(versions(3), vec![3, 6, 10]);
        // Tested ones, then the newest untested.
        assert_eq!(versions(5), vec![3, 6, 8, 9, 10]);
    }

    #[test]
    fn a_carried_in_governor_is_not_a_save_of_the_attempt() {
        let lines = vec![save(9, "champ"), plan(json!({"k": 1})), save(10, "new")];
        assert!(starts_from(&lines, "champ"));
        assert!(!starts_from(&lines, "new"));
        assert!(!starts_from(&[plan(json!({}))], "champ"));
        assert!(!starts_from(&[save(1, "mine")], "champ"));
    }

    #[test]
    fn the_champions_own_pair_is_not_evaluated_again_unless_it_is_the_final_one() {
        let champion_plan = Some(json!({"k": 1}));
        let lines = vec![save(9, "champ"), plan(json!({"k": 1})), save(10, "new")];
        let (pairs, _) = find(&lines);
        let kept = pick(pairs.clone(), 12, Some(("champ", &champion_plan)));
        assert_eq!(kept.iter().map(|p| p.version).collect::<Vec<_>>(), vec![10]);
        // An attempt that changed nothing ends on the champion's pair: that stays, as before.
        let (pairs, _) = find(&lines[..2]);
        let kept = pick(pairs, 12, Some(("champ", &champion_plan)));
        assert_eq!(kept.len(), 1);
    }

    /// The choice of a pair reads `Table::screen_mean` and nothing else: tables whose gate and
    /// owner-only rows say the opposite of their screen rows (and `best` takes numbers, not
    /// tables) choose by the screen. The owner-only tier above all: nothing it holds may move
    /// the choice.
    #[test]
    fn gate_and_owner_scores_play_no_part_in_choosing_the_pair() {
        use crate::agent_eval::{Role, Row, Table};
        let row = |seed, role, score| Row {
            seed,
            role,
            score,
            progress: 0.0,
            care: 1.0,
            deaths: 0,
            stopped: None,
        };
        let table = |screen: f64, gate: f64, owner: f64| Table {
            track: "t".to_owned(),
            horizon: 5,
            collapse_below: 50.0,
            rows: vec![
                row(1, Role::Screen, screen),
                row(2, Role::Screen, screen),
                row(101, Role::Gate, gate),
                row(201, Role::Owner, owner),
            ],
        };
        let tables =
            [table(10.0, 0.0, 0.0), table(5.0, 99_999.0, 99_999.0), table(7.0, 50_000.0, 1.0)];
        let means: Vec<Option<f64>> = tables.iter().map(Table::screen_mean).collect();
        assert_eq!(best(&means), Some(0));
        // And with the other tiers' numbers swapped between the tables, the choice does not move.
        let swapped =
            [table(10.0, 99_999.0, 99_999.0), table(5.0, 0.0, 0.0), table(7.0, 1.0, 50_000.0)];
        let means: Vec<Option<f64>> = swapped.iter().map(Table::screen_mean).collect();
        assert_eq!(best(&means), Some(0));
    }

    #[test]
    fn best_goes_by_the_screen_mean_and_a_tie_goes_to_the_later_pair() {
        assert_eq!(best(&[Some(1.0), Some(5.0), Some(3.0)]), Some(1));
        assert_eq!(best(&[Some(5.0), None, Some(5.0)]), Some(2));
        assert_eq!(best(&[None, None]), None);
        assert_eq!(best(&[]), None);
    }
}

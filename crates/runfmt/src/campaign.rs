//! The hill-climb campaign's files (`cargo xtask agent-run --track ID --attempts N`, #530): what
//! `xtask` writes and `fn_tui`'s run browser (#585) reads.
//!
//! ```text
//! CAMPAIGN/
//!   ledger.json            [`Ledger`]: the track as it was, and a [`Row`] per finished attempt
//!   champion/meta.json     [`Champion`]
//!   champion/gate.json     [`ChampionGate`]
//!   champion/handoff.json  a [`Handoff`](crate::Handoff)
//! ```
//!
//! Two ledger shapes exist and both read: the first one's rows carry `clean`, `train_mean` and
//! `held_out`; the seed tiers (#571) replaced them with `screen`, `gate` and `owner` (each with a
//! mean) and a `verdict`. A [`Row`] holds the fields of both, all optional on reading, and writes
//! only the current shape's (the first shape's fields are left out when empty). The browser
//! shows no `stopped_why`, `deaths`, `plan_revision`, `build`, `saved` or `tried`; it reads them
//! and leaves them be.

use serde::{Deserialize, Serialize};

/// One seed's clean score. `stopped` is the reason the script stopped giving orders, if it did
/// (the score is still read at the horizon); `stopped_turn` and `stopped_why` say when and why
/// (#561). A ledger written before #561 has only `stopped`, and reads back with the other two
/// empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeedScore {
    pub seed: u64,
    pub score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped_turn: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped_why: Option<String>,
    /// Deaths in the run (#571); absent in a score written without them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deaths: Option<u32>,
}

/// How a candidate compared with the champion on the gate seeds (#571), as the ledger keeps it.
/// Every number is an aggregate over the gate seeds; none names a seed or a score.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GateRecord {
    /// The champion it was compared with.
    pub champion_attempt: u32,
    /// Candidate minus champion, mean per seed, and its bootstrap interval at `level`.
    pub diff: f64,
    pub lo: f64,
    pub hi: f64,
    pub level: f64,
    pub wins: u32,
    pub losses: u32,
    pub ties: u32,
    pub candidate_collapses: u32,
    pub champion_collapses: u32,
    pub candidate_deaths: u32,
    pub champion_deaths: u32,
    /// The rule's answer and its reason in words.
    pub promoted: bool,
    pub reason: String,
}

impl GateRecord {
    /// `+31 [+8, +55]`: the gain and its interval, rounded to whole points.
    pub fn text(&self) -> String {
        format!("{:+.0} [{:+.0}, {:+.0}]", self.diff, self.lo, self.hi)
    }
}

/// An attempt's ledger row, in either shape.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Row {
    pub attempt: u32,
    /// The `governor` version the attempt hands on (#570): the one whose pair scored best on the
    /// screen seeds, not necessarily the last saved. `None` when the attempt never saved one.
    pub governor_version: Option<u32>,
    /// Plan edits since the campaign began (the champion's count, plus this attempt's own up to
    /// the plan the chosen pair was judged with).
    pub plan_revision: u32,
    /// The score of the attempt's own played log at the horizon.
    pub played: Option<f64>,
    // The first shape (before #571): read, never written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub clean: Vec<SeedScore>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_mean: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub held_out: Vec<SeedScore>,
    // The seed tiers.
    /// The clean score per screen seed, in `screen_seeds` order (empty with no governor).
    pub screen: Vec<SeedScore>,
    pub screen_mean: Option<f64>,
    /// The chosen pair on the gate seeds, in `gate_seeds` order (empty with no governor).
    pub gate: Vec<SeedScore>,
    pub gate_mean: Option<f64>,
    /// The paired comparison with the champion; `None` for the first governor of a campaign
    /// (nothing to compare with) and for an attempt with no governor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<GateRecord>,
    /// The clean score per owner-only seed, run for a promoted candidate only. For the owner; no
    /// prompt ever shows it, nor a count or a mean made from it.
    pub owner: Vec<SeedScore>,
    pub kept: bool,
    pub cost_usd: Option<f64>,
    /// The build (`fn_agent build-id`, #566) the clean scores above were made on. `None` in a
    /// ledger from before builds were recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    /// How many versions of the governor the attempt saved (#570); `None` with no governor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved: Option<u32>,
    /// How many governor and plan pairs were evaluated on the screen seeds to choose
    /// `governor_version` (at most `versions::CANDIDATE_CAP`; fewer than `saved` when the cap cut
    /// the older untested ones).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tried: Option<u32>,
}

impl Row {
    /// The mean clean score the attempt was judged on first: the screen mean, or the train mean
    /// of the first shape.
    pub fn clean_mean(&self) -> Option<f64> {
        self.screen_mean.or(self.train_mean)
    }

    /// The clean score of `seed` anywhere in the row.
    pub fn score_of(&self, seed: u64) -> Option<&SeedScore> {
        [&self.clean, &self.screen, &self.gate, &self.held_out, &self.owner]
            .into_iter()
            .flatten()
            .find(|s| s.seed == seed)
    }
}

/// `ledger.json`. `T` is the track as the writer keeps it and `R` its par-and-controls record
/// (`xtask`'s own types, which this crate has no use for); [`serde_json::Value`] reads either
/// without knowing them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ledger<T = serde_json::Value, R = serde_json::Value> {
    /// The track as the campaign started it: what `--resume` checks against the tracks file.
    pub track: T,
    pub rows: Vec<Row>,
    /// Each time a resume re-evaluated the champion on a new build, in order (#566).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reevals: Vec<Reeval>,
    /// Par and the controls on the track's seeds, as of the last invocation that measured them
    /// (#572). For the owner's table only; optional, so a ledger without it reads as before and
    /// a reader that doesn't know it ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub references: Option<R>,
}

/// The champion, evaluated again on a different build than the one its means came from (#566,
/// #571). Scores aren't comparable across builds (the same strategy can score differently once
/// the sim has changed), so the attempts that follow are compared with the new gate scores, not
/// the old.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reeval {
    /// The champion's attempt number.
    pub attempt: u32,
    /// How many attempts had a row when this happened: the re-evaluation sits between the rows.
    pub after_rows: u32,
    pub was_build: Option<String>,
    pub was_screen_mean: f64,
    pub was_gate_mean: f64,
    pub build: String,
    pub screen_mean: f64,
    pub gate_mean: f64,
}

/// `champion/meta.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Champion {
    pub attempt: u32,
    /// On `build`, as of the last time the champion was evaluated.
    pub screen_mean: f64,
    pub gate_mean: f64,
    pub governor_version: Option<u32>,
    pub plan_revision: u32,
    /// The build the means were measured on; `None` before #566, which a resume treats as
    /// another build and re-evaluates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
}

/// `champion/gate.json`: the champion's score on each gate seed, on `build`. A candidate is
/// compared with these, paired by seed, so they are kept instead of run again for every attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChampionGate {
    pub build: String,
    pub scores: Vec<SeedScore>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_shape_row_reads_and_a_bare_one_does_too() {
        let old: Row = serde_json::from_str(
            r#"{"attempt":1,"played":361.0,"clean":[{"seed":7,"score":197.5,"stopped":"runaway","stopped_turn":112}],
                "train_mean":273.3,"held_out":[{"seed":1,"score":62.7}],"kept":true,"cost_usd":0.5}"#,
        )
        .unwrap();
        assert_eq!(old.clean_mean(), Some(273.3));
        assert_eq!(old.score_of(7).and_then(|s| s.stopped_turn), Some(112));
        assert_eq!(old.score_of(1).map(|s| s.score), Some(62.7));
        let bare: Row = serde_json::from_str(r#"{"attempt":3}"#).unwrap();
        assert_eq!((bare.clean_mean(), bare.kept, bare.cost_usd), (None, false, None));
    }

    #[test]
    fn a_current_row_round_trips_and_leaves_out_the_old_shape() {
        let row: Row = serde_json::from_str(
            r#"{"attempt":2,"governor_version":6,"plan_revision":1,"screen":[{"seed":7,"score":265.0,"deaths":2,"stopped_why":"x"}],
                "screen_mean":295.0,"gate":[],"gate_mean":null,
                "verdict":{"champion_attempt":1,"diff":31.4,"lo":8.0,"hi":55.2,"level":0.9,"promoted":true,"reason":"x"},
                "owner":[],"kept":true,"cost_usd":0.3,"extra":1}"#,
        )
        .unwrap();
        assert_eq!(row.verdict.as_ref().map(GateRecord::text).as_deref(), Some("+31 [+8, +55]"));
        assert_eq!(
            (row.screen[0].deaths, row.screen[0].stopped_why.as_deref()),
            (Some(2), Some("x"))
        );
        let text = serde_json::to_string(&row).unwrap();
        assert!(!text.contains("train_mean") && !text.contains("held_out"), "{text}");
        assert_eq!(serde_json::from_str::<Row>(&text).unwrap(), row);
    }
}

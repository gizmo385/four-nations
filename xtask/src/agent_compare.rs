//! `cargo xtask agent-compare [options] CONDITION...`
//!
//! Compares conditions on a track's fresh seeds, paired by seed, with no model (#571,
//! docs/design/agent-hillclimb.md, "Measuring a difference"). A condition is a label and one or
//! more strategies: the final champions of several campaigns that ran under the same conditions,
//! or a single fixed governor and plan. Every strategy is replayed from turn 0 on the seeds of a
//! tier of the track (`fn_agent play --horizon`, in parallel), and the command prints, per
//! strategy, the mean, the standard deviation and the failure counts, and for every condition
//! after the first its paired difference from the first, with a bootstrap interval.
//!
//!   CONDITION        [LABEL=]PATH[,PATH...], where a PATH is one of
//!                      a campaign folder (its `champion/` is the strategy),
//!                      a folder holding `handoff.json` (a champion or `--carry` folder),
//!                      GOVERNOR.rhai[@PLAN.json] (a fixed governor, with its plan if given)
//!   --track ID       the track to play (default: the first campaign folder's ledger names it)
//!   --tier NAME      owner (default), gate or screen. The owner-only tier is the honest one for
//!                    a campaign's champions; the gate is the seeds a champion was promoted on.
//!   --threads N      runs in parallel (default: half the cores, at most 12; $FN_EVAL_THREADS)
//!   --per-seed       also print each strategy's score on every seed
//!   --json           print the comparison as JSON
//!   --out DIR        keep each strategy's logs under DIR (default: a scratch folder)
//!   --tracks FILE    a tracks file other than `data/tracks.toml` (for tests)
//!
//! The interval of a condition's difference is over the seeds. When a condition has several
//! strategies, the seed's score is their average, and the interval does not include how much one
//! campaign differs from another: that spread is the min, median and max of the strategies'
//! means, printed beside it. Read the two together.
//!
//! A campaign folder from before the seed tiers (#571) is fine here: only its champion's
//! `handoff.json` is read, and it is evaluated on the current tiers.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use crate::Result;
use crate::agent_eval::paired::{self, Failures, Sample};
use crate::agent_eval::{self, Job, Role, Strategy, Table, Track};

fn say(msg: &str) {
    eprintln!("agent-compare: {msg}");
}

fn help() -> String {
    include_str!("agent_compare.rs")
        .lines()
        .skip(1)
        .take_while(|l| l.starts_with("//!"))
        .map(|l| l.trim_start_matches("//!"))
        .collect::<Vec<_>>()
        .join("\n")
}

// -------------------------------------------------------------------------------------------
// Conditions
// -------------------------------------------------------------------------------------------

/// One strategy in a condition, and where it came from.
#[derive(Debug, Clone, PartialEq)]
struct Member {
    label: String,
    strategy: Strategy,
}

#[derive(Debug, Clone, PartialEq)]
struct Condition {
    label: String,
    members: Vec<Member>,
}

/// Splits `LABEL=PATH,PATH` into its label (if any) and its paths. A label is letters, digits,
/// `-` and `_`, so a path with an `=` in it is not mistaken for one.
fn split_spec(spec: &str) -> (Option<&str>, Vec<&str>) {
    let (label, rest) = match spec.split_once('=') {
        Some((l, r))
            if !l.is_empty()
                && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') =>
        {
            (Some(l), r)
        }
        _ => (None, spec),
    };
    (label, rest.split(',').filter(|p| !p.is_empty()).collect())
}

/// Reads one strategy from a path: a campaign folder, a folder with a `handoff.json`, or a
/// governor script with an optional `@plan.json`. Returns its default label.
fn load_member(path: &str) -> Result<Member> {
    let (file, plan_file) = match path.split_once('@') {
        Some((f, p)) => (f, Some(p)),
        None => (path, None),
    };
    let p = Path::new(file);
    if p.is_dir() {
        if plan_file.is_some() {
            return Err(format!(
                "{path}: a folder holds its own plan; `@PLAN` is for a .rhai file"
            ));
        }
        let champion = p.join("champion");
        let dir = if champion.join("handoff.json").is_file() {
            champion
        } else if p.join("handoff.json").is_file() {
            p.to_path_buf()
        } else {
            return Err(format!(
                "{file} is a folder with neither champion/handoff.json (a campaign) nor \
                 handoff.json (a champion)"
            ));
        };
        let strategy = agent_eval::read_strategy(&dir)?;
        return Ok(Member { label: name_of(p), strategy });
    }
    if !p.is_file() {
        return Err(format!("{file}: no such file or folder"));
    }
    let governor = fs::read_to_string(p).map_err(|e| format!("can't read {}: {e}", p.display()))?;
    if governor.trim().is_empty() {
        return Err(format!("{file} is empty"));
    }
    let plan = match plan_file {
        None => None,
        Some(pf) => {
            let text =
                fs::read_to_string(pf).map_err(|e| format!("can't read the plan {pf}: {e}"))?;
            let v: Value =
                serde_json::from_str(&text).map_err(|e| format!("{pf} isn't JSON: {e}"))?;
            if !v.is_object() {
                return Err(format!("{pf}: a plan is a JSON object"));
            }
            v.as_object().is_some_and(|m| !m.is_empty()).then_some(v)
        }
    };
    Ok(Member { label: name_of(p), strategy: Strategy { governor, plan } })
}

/// A path's own name, or for a campaign folder the name of the folder.
fn name_of(p: &Path) -> String {
    let p = fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    p.file_name().map_or("strategy".to_owned(), |n| n.to_string_lossy().into_owned())
}

fn parse_condition(spec: &str) -> Result<Condition> {
    let (label, paths) = split_spec(spec);
    if paths.is_empty() {
        return Err(format!("condition {spec:?} names no strategy"));
    }
    let members = paths.iter().map(|p| load_member(p)).collect::<Result<Vec<_>>>()?;
    let label = label.map(str::to_owned).unwrap_or_else(|| {
        if members.len() == 1 { members[0].label.clone() } else { "group".into() }
    });
    Ok(Condition { label, members })
}

/// The track a campaign folder was climbing, read raw from its ledger so a ledger from before the
/// tiers still names it.
fn track_of_campaign(spec_paths: &[&str]) -> Option<String> {
    spec_paths.iter().find_map(|p| {
        let text = fs::read_to_string(Path::new(p.split('@').next()?).join("ledger.json")).ok()?;
        let v: Value = serde_json::from_str(&text).ok()?;
        v["track"]["id"].as_str().map(str::to_owned)
    })
}

// -------------------------------------------------------------------------------------------
// The comparison
// -------------------------------------------------------------------------------------------

/// One strategy's results on the tier.
#[derive(Debug, Clone, PartialEq)]
struct StrategyResult {
    label: String,
    samples: Vec<Sample>,
}

impl StrategyResult {
    fn scores(&self) -> Vec<f64> {
        self.samples.iter().map(|s| s.score).collect()
    }
    fn mean(&self) -> f64 {
        paired::mean(&self.scores())
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Evaluated {
    label: String,
    members: Vec<StrategyResult>,
}

impl Evaluated {
    /// Each seed's score averaged over the condition's strategies.
    fn per_seed(&self) -> Vec<Sample> {
        let first = &self.members[0].samples;
        first
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let k = self.members.len() as f64;
                let score = self.members.iter().map(|m| m.samples[i].score).sum::<f64>() / k;
                Sample { seed: s.seed, score, deaths: 0, stopped: false }
            })
            .collect()
    }

    /// Collapses, deaths and stops over every strategy and seed of the condition.
    fn failures(&self, collapse_below: f64) -> Failures {
        let all: Vec<Sample> = self.members.iter().flat_map(|m| m.samples.clone()).collect();
        Failures::of(&all, collapse_below)
    }

    fn member_means(&self) -> Vec<f64> {
        self.members.iter().map(StrategyResult::mean).collect()
    }
}

fn median(xs: &[f64]) -> f64 {
    let mut v = xs.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

/// A condition against the baseline, on the seeds they share.
#[derive(Debug, Clone, PartialEq)]
struct Contrast {
    label: String,
    baseline: String,
    diff: paired::Interval,
    wins: usize,
    losses: usize,
    ties: usize,
}

fn contrast(base: &Evaluated, other: &Evaluated) -> Result<Contrast> {
    let (diff, wins, losses, ties) =
        paired::difference(&other.per_seed(), &base.per_seed(), paired::REPORT_LEVEL)?;
    Ok(Contrast {
        label: other.label.clone(),
        baseline: base.label.clone(),
        diff,
        wins,
        losses,
        ties,
    })
}

struct Report {
    track: Track,
    tier: Role,
    build: String,
    conditions: Vec<Evaluated>,
    contrasts: Vec<Contrast>,
}

fn report(track: &Track, tier: Role, build: String, conditions: Vec<Evaluated>) -> Result<Report> {
    let contrasts =
        conditions[1..].iter().map(|c| contrast(&conditions[0], c)).collect::<Result<Vec<_>>>()?;
    Ok(Report { track: track.clone(), tier, build, conditions, contrasts })
}

impl Report {
    fn text(&self, per_seed: bool) -> String {
        let t = &self.track;
        let seeds = t.seeds(self.tier);
        let mut out = String::new();
        let _ = writeln!(
            out,
            "agent-compare · {} · {} tier, {} seeds · horizon {} · build {}",
            t.id,
            self.tier.word(),
            seeds.len(),
            t.horizon_turns,
            self.build
        );
        let _ = writeln!(
            out,
            " {:<14} {:<26} {:>6} {:>5} {:>6}  failures",
            "condition", "strategy", "mean", "sd", "min"
        );
        for c in &self.conditions {
            for m in &c.members {
                let scores = m.scores();
                let min = scores.iter().copied().fold(f64::INFINITY, f64::min);
                let f = Failures::of(&m.samples, t.collapse_below);
                let _ = writeln!(
                    out,
                    " {:<14} {:<26} {:>6.1} {:>5.1} {:>6.1}  {}",
                    c.label,
                    m.label,
                    m.mean(),
                    paired::sd(&scores),
                    min,
                    f.text()
                );
            }
        }
        let _ = writeln!(
            out,
            "\n collapse: a score below {}. min, sd and the failures are over the seeds.",
            t.collapse_below
        );
        let _ = writeln!(out, "\n by condition:");
        for c in &self.conditions {
            let means = c.member_means();
            let per_seed = c.per_seed();
            let iv = paired::bootstrap(
                &per_seed.iter().map(|s| s.score).collect::<Vec<_>>(),
                paired::REPORT_LEVEL,
            )
            .expect("a condition has seeds");
            let f = c.failures(t.collapse_below);
            let spread = if means.len() > 1 {
                format!(
                    "; {} strategies, means min {:.0} · median {:.0} · max {:.0}",
                    means.len(),
                    means.iter().copied().fold(f64::INFINITY, f64::min),
                    median(&means),
                    means.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                )
            } else {
                String::new()
            };
            let _ = writeln!(
                out,
                " {:<14} mean {:.1} [{:.1}, {:.1}] {}%{spread}; {}",
                c.label,
                iv.mean,
                iv.lo,
                iv.hi,
                (iv.level * 100.0).round(),
                f.text()
            );
        }
        for k in &self.contrasts {
            let _ = writeln!(
                out,
                "\n paired, {} minus {}: {} {}%   wins {} · losses {} · ties {}",
                k.label,
                k.baseline,
                k.diff.text(),
                (k.diff.level * 100.0).round(),
                k.wins,
                k.losses,
                k.ties
            );
        }
        if self.conditions.iter().any(|c| c.members.len() > 1) {
            let _ = writeln!(
                out,
                "\n The interval is over the seeds. It does not include how much one campaign \
                 differs from another: that is the spread of the strategies' means above."
            );
        }
        if per_seed {
            let _ = writeln!(out, "\n per seed:");
            let mut head = format!(" {:>6}", "seed");
            let names: Vec<&StrategyResult> =
                self.conditions.iter().flat_map(|c| &c.members).collect();
            for m in &names {
                let _ = write!(head, " {:>12.12}", m.label);
            }
            let _ = writeln!(out, "{head}");
            for (i, seed) in seeds.iter().enumerate() {
                let mut line = format!(" {seed:>6}");
                for m in &names {
                    let s = m.samples[i];
                    let mark = if s.score < t.collapse_below { "*" } else { " " };
                    let _ = write!(line, " {:>11.1}{mark}", s.score);
                }
                let _ = writeln!(out, "{line}");
            }
            let _ = writeln!(out, " * collapse");
        }
        out
    }

    fn json(&self) -> Value {
        let t = &self.track;
        let strategy = |m: &StrategyResult| {
            let f = Failures::of(&m.samples, t.collapse_below);
            json!({
                "label": m.label,
                "mean": m.mean(),
                "sd": paired::sd(&m.scores()),
                "collapses": f.collapses,
                "deaths": f.deaths,
                "death_seeds": f.death_seeds,
                "stopped": f.stopped,
                "scores": m.samples.iter().map(|s| json!({
                    "seed": s.seed, "score": s.score, "deaths": s.deaths, "stopped": s.stopped,
                })).collect::<Vec<_>>(),
            })
        };
        json!({
            "track": t.id,
            "tier": self.tier.word(),
            "seeds": t.seeds(self.tier),
            "horizon": t.horizon_turns,
            "collapse_below": t.collapse_below,
            "build": self.build,
            "conditions": self.conditions.iter().map(|c| json!({
                "label": c.label,
                "strategies": c.members.iter().map(strategy).collect::<Vec<_>>(),
                "mean_of_means": paired::mean(&c.member_means()),
            })).collect::<Vec<_>>(),
            "contrasts": self.contrasts.iter().map(|k| json!({
                "condition": k.label,
                "baseline": k.baseline,
                "level": k.diff.level,
                "diff": k.diff.mean,
                "lo": k.diff.lo,
                "hi": k.diff.hi,
                "wins": k.wins,
                "losses": k.losses,
                "ties": k.ties,
            })).collect::<Vec<_>>(),
        })
    }
}

/// Evaluates every strategy of every condition on `tier`, in one parallel queue, and gathers the
/// results per condition.
fn evaluate_conditions(
    fn_agent: &Path,
    track: &Track,
    tier: Role,
    conditions: &[Condition],
    work: &Path,
    threads: usize,
) -> Result<Vec<Evaluated>> {
    let mut jobs = Vec::new();
    let mut index = Vec::new();
    for (ci, c) in conditions.iter().enumerate() {
        for (mi, m) in c.members.iter().enumerate() {
            jobs.push(Job {
                strategy: &m.strategy,
                work: work.join(format!("{}-{}-{}", ci + 1, c.label, mi + 1)),
            });
            index.push((ci, mi));
        }
    }
    let tables: Vec<Table> = agent_eval::evaluate_many(fn_agent, track, &jobs, &[tier], threads)?;
    let mut out: Vec<Evaluated> = conditions
        .iter()
        .map(|c| Evaluated { label: c.label.clone(), members: Vec::new() })
        .collect();
    for ((ci, mi), table) in index.into_iter().zip(tables) {
        out[ci].members.push(StrategyResult {
            label: conditions[ci].members[mi].label.clone(),
            samples: table.samples(tier),
        });
    }
    Ok(out)
}

// -------------------------------------------------------------------------------------------
// Entry point
// -------------------------------------------------------------------------------------------

#[derive(Debug, Default, PartialEq)]
struct Opts {
    track: Option<String>,
    tier: Option<Role>,
    threads: Option<usize>,
    per_seed: bool,
    json: bool,
    out: Option<PathBuf>,
    tracks: Option<PathBuf>,
    specs: Vec<String>,
}

fn parse_args(args: &[&str]) -> Result<Opts> {
    let mut o = Opts::default();
    let mut it = args.iter();
    while let Some(&arg) = it.next() {
        let mut value = || it.next().copied().ok_or_else(|| format!("{arg} needs a value"));
        match arg {
            "--track" => o.track = Some(value()?.to_owned()),
            "--tier" => o.tier = Some(Role::parse(value()?)?),
            "--threads" => {
                let v = value()?;
                o.threads =
                    Some(v.parse().map_err(|_| format!("--threads wants a number, got {v:?}"))?);
            }
            "--out" => o.out = Some(PathBuf::from(value()?)),
            "--tracks" => o.tracks = Some(PathBuf::from(value()?)),
            "--per-seed" => o.per_seed = true,
            "--json" => o.json = true,
            "-h" | "--help" => return Err(help()),
            flag if flag.starts_with("--") => {
                return Err(format!("unknown flag {flag:?}\n\n{}", help()));
            }
            spec => o.specs.push(spec.to_owned()),
        }
    }
    if o.specs.len() < 2 {
        return Err(format!("name at least two conditions to compare\n\n{}", help()));
    }
    Ok(o)
}

pub fn run(args: &[&str]) -> Result {
    let opts = parse_args(args)?;
    let repo = crate::root();
    let conditions = opts.specs.iter().map(|s| parse_condition(s)).collect::<Result<Vec<_>>>()?;
    let mut labels: Vec<&str> = conditions.iter().map(|c| c.label.as_str()).collect();
    labels.sort_unstable();
    if let Some(w) = labels.windows(2).find(|w| w[0] == w[1]) {
        return Err(format!("two conditions are called {:?}; give one a LABEL=", w[0]));
    }

    let all_paths: Vec<&str> = opts.specs.iter().flat_map(|s| split_spec(s).1).collect();
    let Some(track_id) = opts.track.clone().or_else(|| track_of_campaign(&all_paths)) else {
        return Err("--track ID is needed: no campaign folder names one".to_owned());
    };
    let tracks_path = opts.tracks.clone().unwrap_or_else(|| repo.join("data/tracks.toml"));
    let track = agent_eval::load_track(&tracks_path, &track_id)?;
    let tier = opts.tier.unwrap_or(Role::Owner);
    let threads = agent_eval::resolve_threads(opts.threads)?;
    let fn_agent = agent_eval::fn_agent_binary(&repo)?;
    let build = Command::new(&fn_agent)
        .arg("build-id")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or("unknown".to_owned(), |o| String::from_utf8_lossy(&o.stdout).trim().to_owned());

    let runs: usize =
        conditions.iter().map(|c| c.members.len()).sum::<usize>() * track.seeds(tier).len();
    say(&format!(
        "{} condition(s), {} strategy run(s) on the {} tier of {}, {threads} thread(s)",
        conditions.len(),
        runs,
        tier.word(),
        track.id
    ));
    let started = std::time::Instant::now();
    let (work, scratch) = match &opts.out {
        Some(dir) => (dir.clone(), false),
        None => {
            (std::env::temp_dir().join(format!("fn-agent-compare-{}", std::process::id())), true)
        }
    };
    let result = evaluate_conditions(&fn_agent, &track, tier, &conditions, &work, threads);
    if scratch {
        let _ = fs::remove_dir_all(&work);
    }
    let evaluated = result?;
    say(&format!("evaluated in {} s", started.elapsed().as_secs()));

    let report = report(&track, tier, build, evaluated)?;
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&report.json()).map_err(|e| e.to_string())?);
    } else {
        print!("{}", report.text(opts.per_seed));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fn_compare_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_handoff(dir: &Path, governor: &str) {
        fs::create_dir_all(dir).unwrap();
        let h = json!({"scripts": {"governor": {"version": 1, "code": governor}}, "notes": [],
                       "plan": {"k": 1}});
        fs::write(dir.join("handoff.json"), h.to_string()).unwrap();
    }

    #[test]
    fn specs_split_into_a_label_and_paths() {
        assert_eq!(split_spec("a=x,y"), (Some("a"), vec!["x", "y"]));
        assert_eq!(split_spec("x"), (None, vec!["x"]));
        assert_eq!(split_spec("/p/with=equals,z"), (None, vec!["/p/with=equals", "z"]));
        assert_eq!(split_spec("g.rhai@p.json"), (None, vec!["g.rhai@p.json"]));
        assert_eq!(split_spec("a=").1, Vec::<&str>::new());
    }

    #[test]
    fn a_member_is_a_campaign_a_champion_folder_or_a_governor_file() {
        let t = tmp("members");
        write_handoff(&t.join("camp/champion"), "campaign governor");
        fs::write(t.join("camp/ledger.json"), r#"{"track":{"id":"water-3h"},"rows":[]}"#).unwrap();
        write_handoff(&t.join("champ"), "champion governor");
        fs::write(t.join("g.rhai"), "file governor").unwrap();
        fs::write(t.join("p.json"), r#"{"a": 1}"#).unwrap();
        fs::write(t.join("empty.json"), "{}").unwrap();

        let p = |name: &str| t.join(name).to_string_lossy().into_owned();
        let m = load_member(&p("camp")).unwrap();
        assert_eq!((m.strategy.governor.as_str(), m.label.as_str()), ("campaign governor", "camp"));
        assert_eq!(m.strategy.plan, Some(json!({"k": 1})));
        assert_eq!(load_member(&p("champ")).unwrap().strategy.governor, "champion governor");
        let m = load_member(&format!("{}@{}", p("g.rhai"), p("p.json"))).unwrap();
        assert_eq!(
            (m.strategy.governor.as_str(), m.strategy.plan),
            ("file governor", Some(json!({"a": 1})))
        );
        assert_eq!(m.label, "g.rhai");
        assert_eq!(load_member(&p("g.rhai")).unwrap().strategy.plan, None);
        assert_eq!(
            load_member(&format!("{}@{}", p("g.rhai"), p("empty.json"))).unwrap().strategy.plan,
            None
        );

        assert!(load_member(&p("nowhere")).unwrap_err().contains("no such file"));
        fs::create_dir_all(t.join("bare")).unwrap();
        assert!(load_member(&p("bare")).unwrap_err().contains("handoff.json"));
        assert!(
            load_member(&format!("{}@{}", p("camp"), p("p.json"))).unwrap_err().contains("@PLAN")
        );

        // The track comes from a campaign's ledger, old or new.
        assert_eq!(track_of_campaign(&[&p("champ"), &p("camp")]), Some("water-3h".to_owned()));
        assert_eq!(track_of_campaign(&[&p("champ")]), None);
        let c = parse_condition(&format!("mine={},{}", p("camp"), p("champ"))).unwrap();
        assert_eq!((c.label.as_str(), c.members.len()), ("mine", 2));
        assert_eq!(parse_condition(&p("champ")).unwrap().label, "champ");
        assert_eq!(
            parse_condition(&format!("{},{}", p("camp"), p("champ"))).unwrap().label,
            "group"
        );
        let _ = fs::remove_dir_all(&t);
    }

    #[test]
    fn args() {
        assert!(parse_args(&["a"]).unwrap_err().contains("at least two"));
        assert!(parse_args(&["a", "b", "--bogus"]).unwrap_err().contains("unknown flag"));
        let o =
            parse_args(&["a", "--tier", "gate", "b=c,d", "--threads", "2", "--per-seed"]).unwrap();
        assert_eq!(
            (o.specs.len(), o.tier, o.threads, o.per_seed),
            (2, Some(Role::Gate), Some(2), true)
        );
        assert!(parse_args(&["a", "b", "--tier", "held-out"]).is_err());
        assert!(parse_args(&["--help"]).unwrap_err().contains("CONDITION"));
    }

    fn samples(scores: &[f64]) -> Vec<Sample> {
        scores
            .iter()
            .enumerate()
            .map(|(i, &score)| Sample { seed: 2000 + i as u64, score, deaths: 0, stopped: false })
            .collect()
    }

    fn evaluated(label: &str, members: &[&[f64]]) -> Evaluated {
        Evaluated {
            label: label.to_owned(),
            members: members
                .iter()
                .enumerate()
                .map(|(i, s)| StrategyResult { label: format!("{label}{i}"), samples: samples(s) })
                .collect(),
        }
    }

    fn track() -> Track {
        agent_eval::parse_tracks(
            "[[track]]\nid = \"t\"\nname = \"T\"\nnation = \"water\"\ndifficulty = \"normal\"\n\
             map_size = \"small\"\nsite = \"default\"\nhorizon_turns = 5\nplay_seed = 1\n\
             screen_seeds = [1]\ngate_seeds = [101]\n\
             owner_seeds = [2000, 2001, 2002, 2003]\ncollapse_below = 100\n",
        )
        .unwrap()
        .remove(0)
    }

    /// Paired on the seeds, per condition averaged over its strategies, baseline first.
    #[test]
    fn a_contrast_is_paired_by_seed_and_averages_a_conditions_strategies() {
        let base = evaluated("base", &[&[100.0, 200.0, 300.0, 50.0]]);
        let other =
            evaluated("other", &[&[120.0, 210.0, 330.0, 50.0], &[140.0, 230.0, 330.0, 90.0]]);
        let k = contrast(&base, &other).unwrap();
        // The other's per-seed average is 130, 220, 330, 70: ahead by 30, 20, 30, 20.
        assert!((k.diff.mean - 25.0).abs() < 1e-9, "{:?}", k.diff);
        assert_eq!((k.wins, k.losses, k.ties), (4, 0, 0));
        assert_eq!(k.diff.level, paired::REPORT_LEVEL);
        let r = report(&track(), Role::Owner, "b1".to_owned(), vec![base, other]).unwrap();
        let text = r.text(true);
        assert!(text.contains("paired, other minus base: +25.0 ["), "{text}");
        assert!(text.contains("wins 4 · losses 0 · ties 0"), "{text}");
        assert!(text.contains("2 strategies, means min 178 · median 188 · max 198"), "{text}");
        assert!(text.contains("does not include how much one campaign differs"), "{text}");
        assert!(text.contains("owner tier, 4 seeds") && text.contains("build b1"), "{text}");
        // The collapses: base has one (50), other has one (50) in its first strategy.
        assert!(text.contains("collapse 1/4"), "{text}");
        assert!(text.contains("per seed:") && text.contains("2003"), "{text}");
        let j = r.json();
        assert_eq!(j["contrasts"][0]["wins"], 4);
        assert_eq!(j["conditions"][1]["strategies"].as_array().unwrap().len(), 2);
        assert_eq!(j["tier"], "owner");
        // The same inputs give the same bytes (the bootstrap is seeded).
        let again = report(
            &track(),
            Role::Owner,
            "b1".to_owned(),
            vec![
                evaluated("base", &[&[100.0, 200.0, 300.0, 50.0]]),
                evaluated("other", &[&[120.0, 210.0, 330.0, 50.0], &[140.0, 230.0, 330.0, 90.0]]),
            ],
        )
        .unwrap();
        assert_eq!(again.text(true), text);
    }

    #[test]
    fn median_of_even_and_odd() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), 2.5);
    }
}

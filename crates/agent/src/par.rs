//! Par (#572, docs/design/agent-hillclimb.md, "Tracks: what is comparable"): the score of
//! `fn_sim::autoplay::Economy` on the same start at the same horizon, and a cache of it.
//!
//! What par is, in the words every output uses ([`WHAT`]): what a stock scripted opening reaches
//! on this start, reading the world directly. That last part matters. The opening is privileged:
//! it knows where every resource is (it scrolls out to `SCROLL_RADIUS` = 90 tiles, which a player
//! would have to discover), so par is a yardstick for what the rules allow a player who knows the
//! game, not a target a fair player should hit and not a ceiling (on some starts the stock
//! opening fails and a good player beats it). Selection never reads it, and no prompt carries it:
//! the model is not handed an answer drawn from data it could not see.
//!
//! Computing a par plays 540 turns of the opening and takes tens of seconds, so it is cached per
//! build and start: `<cache>/par/<build>/<start>.json`, where the cache folder is
//! `$FN_AGENT_CACHE`, else `$XDG_CACHE_HOME/four-nations`, else `$HOME/.cache/four-nations`. The
//! build id (`fn_tui::buildid`) covers the code and the data, so a changed sim or `score.toml`
//! is a new build and a fresh par; an `unknown` build (no git) is never cached. It is not a
//! run folder: nothing here is written under `$FN_AGENT_RUNS`, so the listing of the runs there
//! (#585) never changes because of it. Writes go through a temporary file and a rename, so
//! parallel readers and writers see a whole file or none.

use std::fs;
use std::path::{Path, PathBuf};

use fn_tui::surface::RunLogHeader;
use serde::{Deserialize, Serialize};

use crate::scorecard;

/// How par is described wherever it is printed.
pub const WHAT: &str = "what a stock scripted opening reaches on this start, reading the world \
                        directly";

/// What par is not, for the lines that print a par beside a score.
pub const NOT: &str = "not a target a fair player should hit, and not a ceiling";

/// One milestone as par left it at the horizon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParMilestone {
    pub id: String,
    pub name: String,
    /// `sign`, `ready` or `stage`.
    pub kind: String,
    /// The turn par first met it, or `None` if it had not by the horizon.
    pub met_turn: Option<u32>,
    pub met_now: bool,
    pub credit: f64,
}

/// The scorecard of the Economy opening on one start at one horizon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Par {
    /// The build that computed it (`fn_agent build-id`).
    pub build: String,
    pub horizon: u32,
    pub score: f64,
    pub progress: f64,
    pub care: f64,
    pub deaths: u32,
    pub milestones: Vec<ParMilestone>,
}

impl Par {
    /// The milestones par had not met by the horizon, in the endeavor's order.
    pub fn unreached(&self) -> Vec<&ParMilestone> {
        self.milestones.iter().filter(|m| m.met_turn.is_none()).collect()
    }

    /// How many milestones par met by the horizon.
    pub fn reached_count(&self) -> usize {
        self.milestones.iter().filter(|m| m.met_turn.is_some()).count()
    }

    /// The one-line statement of what par reached: "par met 9 of 14 milestones by turn 540; not
    /// by turn 540: Ready, Foundation". It says what par reached and nothing about what could be.
    pub fn reached_line(&self) -> String {
        let missed: Vec<&str> = self.unreached().iter().map(|m| m.name.as_str()).collect();
        let head = format!(
            "par met {} of {} milestones by turn {}",
            self.reached_count(),
            self.milestones.len(),
            self.horizon
        );
        if missed.is_empty() {
            format!("{head}: all of them")
        } else {
            format!("{head}; it had not met: {}", missed.join(", "))
        }
    }
}

// -------------------------------------------------------------------------------------------
// The cache
// -------------------------------------------------------------------------------------------

/// The cache folder (module doc): `fn_tui::cache::root`, shared with the scenario prelude cache.
pub fn cache_root() -> Option<PathBuf> {
    fn_tui::cache::root()
}

/// The file name for a start at a horizon: everything that decides the world and where it is
/// scored, in words, so a person can tell the files apart.
pub fn key(header: &RunLogHeader, horizon: u32) -> String {
    let site = match header.site {
        Some((x, y)) => format!("{x}x{y}"),
        None => "default".to_owned(),
    };
    let prelude = match &header.prelude {
        Some(p) => format!("{p:?}"),
        None => "none".to_owned(),
    };
    // The default profile adds nothing, so caches from before profiles (#1008) still match.
    let profile = crate::scorecard::profile_key_suffix();
    let raw = format!(
        "seed{}-{}-{}-{}-site{site}-prelude{prelude}-turn{}-h{horizon}{profile}",
        header.seed,
        header.nation.as_str(),
        header.difficulty.as_deref().unwrap_or("none"),
        header.map_size.as_deref().unwrap_or("none"),
        header.turn_seconds
    );
    raw.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

/// Where this build's par for `header` at `horizon` is kept under `root`.
pub fn cache_path(root: &Path, build: &str, header: &RunLogHeader, horizon: u32) -> PathBuf {
    root.join("par").join(build).join(format!("{}.json", key(header, horizon)))
}

/// Where a par came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Cached,
    Computed,
}

/// The par for `header` at `horizon`: from the cache if this build has computed it, else
/// computed now and kept. A cache that can't be read or written is skipped, never an error: it
/// is only a saving.
pub fn get(header: &RunLogHeader, horizon: u32) -> Result<(Par, Source), String> {
    get_with(cache_root().as_deref(), fn_tui::buildid::CURRENT, header, horizon, || {
        scorecard::compute_par(header, horizon)
    })
}

/// [`get`] with the cache folder, the build and the computation given (tests give their own).
pub fn get_with(
    root: Option<&Path>,
    build: &str,
    header: &RunLogHeader,
    horizon: u32,
    compute: impl FnOnce() -> Result<Par, String>,
) -> Result<(Par, Source), String> {
    // A build with no id can't say that two runs are the same code.
    let root = root.filter(|_| build != "unknown");
    if let Some(root) = root
        && let Some(par) = read(&cache_path(root, build, header, horizon), build)
    {
        return Ok((par, Source::Cached));
    }
    let par = compute()?;
    if let Some(root) = root {
        write(&cache_path(root, build, header, horizon), &par);
    }
    Ok((par, Source::Computed))
}

/// The par at `path`, if it is one this `build` wrote.
fn read(path: &Path, build: &str) -> Option<Par> {
    let par: Par = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    (par.build == build).then_some(par)
}

fn write(path: &Path, par: &Par) {
    let Some(dir) = path.parent() else { return };
    let Ok(text) = serde_json::to_string_pretty(par) else { return };
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = dir.join(format!(".{}.{}.tmp", std::process::id(), key_tail(path)));
    if fs::write(&tmp, text).is_ok() && fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

fn key_tail(path: &Path) -> String {
    path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn header(seed: u64) -> RunLogHeader {
        RunLogHeader {
            seed,
            nation: fn_tui::cli::parse_nation("water").unwrap(),
            turn_seconds: 20,
            ticks_per_turn: 200,
            map_size: Some("small".into()),
            difficulty: Some("normal".into()),
            site: None,
            rough: false,
            data_digest: 0,
            crate_version: "0".into(),
            prelude: None,
            playtest: false,
            build: None,
        }
    }

    fn par(build: &str, score: f64) -> Par {
        Par {
            build: build.into(),
            horizon: 540,
            score,
            progress: 0.5,
            care: 1.0,
            deaths: 0,
            milestones: vec![
                ParMilestone {
                    id: "population".into(),
                    name: "Population".into(),
                    kind: "sign".into(),
                    met_turn: Some(120),
                    met_now: true,
                    credit: 0.8,
                },
                ParMilestone {
                    id: "ready".into(),
                    name: "Ready".into(),
                    kind: "ready".into(),
                    met_turn: None,
                    met_now: false,
                    credit: 0.0,
                },
            ],
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fn-par-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_par_is_computed_once_per_build_and_start() {
        let dir = scratch("once");
        let calls = Cell::new(0);
        let get = |build: &str, seed: u64, horizon: u32| {
            get_with(Some(&dir), build, &header(seed), horizon, || {
                calls.set(calls.get() + 1);
                Ok(par(build, f64::from(calls.get())))
            })
            .unwrap()
        };
        assert_eq!(get("aaa", 7, 540), (par("aaa", 1.0), Source::Computed));
        assert_eq!(get("aaa", 7, 540), (par("aaa", 1.0), Source::Cached), "same build and start");
        assert_eq!(calls.get(), 1);
        // Another seed, another horizon and another build are each their own par.
        assert_eq!(get("aaa", 42, 540).1, Source::Computed);
        assert_eq!(get("aaa", 7, 300).1, Source::Computed);
        assert_eq!(get("bbb", 7, 540).1, Source::Computed);
        assert_eq!(calls.get(), 4);
        assert_eq!(get("bbb", 7, 540).1, Source::Cached);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_build_and_a_missing_cache_folder_compute_every_time() {
        let dir = scratch("unknown");
        let calls = Cell::new(0);
        let run = |root: Option<&Path>, build: &str| {
            get_with(root, build, &header(7), 540, || {
                calls.set(calls.get() + 1);
                Ok(par(build, 1.0))
            })
            .unwrap()
            .1
        };
        assert_eq!(run(Some(&dir), "unknown"), Source::Computed);
        assert_eq!(run(Some(&dir), "unknown"), Source::Computed);
        assert_eq!(run(None, "aaa"), Source::Computed);
        assert_eq!(run(None, "aaa"), Source::Computed);
        assert_eq!(calls.get(), 4);
        assert!(!dir.exists(), "nothing was written");
    }

    /// A file that is not a par, or one a different build wrote, is recomputed over, not trusted.
    #[test]
    fn a_damaged_or_foreign_cache_file_is_recomputed() {
        let dir = scratch("damaged");
        let path = cache_path(&dir, "aaa", &header(7), 540);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json").unwrap();
        let (p, src) =
            get_with(Some(&dir), "aaa", &header(7), 540, || Ok(par("aaa", 9.0))).unwrap();
        assert_eq!((p.score, src), (9.0, Source::Computed));
        fs::write(&path, serde_json::to_string(&par("zzz", 5.0)).unwrap()).unwrap();
        let (p, src) =
            get_with(Some(&dir), "aaa", &header(7), 540, || Ok(par("aaa", 8.0))).unwrap();
        assert_eq!((p.score, src), (8.0, Source::Computed));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_computation_is_an_error_and_caches_nothing() {
        let dir = scratch("failed");
        let e = get_with(Some(&dir), "aaa", &header(7), 540, || Err("no such preset".into()))
            .unwrap_err();
        assert!(e.contains("no such preset"));
        assert!(!dir.exists());
    }

    #[test]
    fn the_cache_key_names_every_part_of_the_start() {
        let base = key(&header(7), 540);
        let mut other = header(7);
        other.site = Some((12, 30));
        let keys = [
            key(&header(8), 540),
            key(&header(7), 541),
            key(&other, 540),
            {
                let mut h = header(7);
                h.difficulty = None;
                key(&h, 540)
            },
            {
                let mut h = header(7);
                h.nation = fn_tui::cli::parse_nation("fire").unwrap();
                key(&h, 540)
            },
            {
                let mut h = header(7);
                h.turn_seconds = 30;
                key(&h, 540)
            },
        ];
        for k in &keys {
            assert_ne!(*k, base);
        }
        let mut uniq = keys.to_vec();
        uniq.sort();
        uniq.dedup();
        assert_eq!(uniq.len(), keys.len());
        assert!(base.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "{base}");
    }

    #[test]
    fn reached_line_says_what_par_reached_and_nothing_more() {
        let p = par("aaa", 500.0);
        assert_eq!(p.reached_count(), 1);
        assert_eq!(
            p.reached_line(),
            "par met 1 of 2 milestones by turn 540; it had not met: Ready"
        );
        let mut all = p;
        all.milestones[1].met_turn = Some(300);
        assert_eq!(all.reached_line(), "par met 2 of 2 milestones by turn 540: all of them");
    }
}

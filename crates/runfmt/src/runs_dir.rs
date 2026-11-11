//! Where agent runs live (#846), decided once for the tools that write them (`cargo xtask
//! agent-run`) and the ones that read them (`fn_tui`'s title screen Replays, the run browser).
//!
//! The rule is the saves' rule (`fn_ui::saves::save_dir`) with `runs` in place of `saves`, so
//! runs sit beside the saves in the platform's data directory: `$XDG_DATA_HOME` or
//! `~/.local/share` on Linux, `~/Library/Application Support` on macOS, `%APPDATA%` on Windows,
//! each followed by `four-nations/runs`. `$FN_AGENT_RUNS` overrides it. It lives here, not in
//! `fn_ui`, because `xtask` depends on this crate and not on `fn_ui`; `fn_tui` has a test that
//! the two rules agree.
//!
//! Runs used to default to `~/fn-agent-runs`. While the new folder doesn't exist and the old one
//! has runs in it, [`RunsDir`] keeps using the old one and carries a note saying how to move
//! them, so nothing goes missing and nothing is moved behind the owner's back.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The variable that names another runs folder.
pub const ENV: &str = "FN_AGENT_RUNS";

/// The folder runs used to default to, under `$HOME`.
pub const LEGACY_NAME: &str = "fn-agent-runs";

/// The default runs folder, ignoring `$FN_AGENT_RUNS` and the old folder: the platform's data
/// directory followed by `four-nations/runs`, from `env` (a lookup of environment variables) on
/// operating system `os` (`std::env::consts::OS`).
pub fn data_home_runs(env: impl Fn(&str) -> Option<OsString>, os: &str) -> Option<PathBuf> {
    let set = |k: &str| env(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = match os {
        "windows" => set("APPDATA")?,
        "macos" => set("HOME")?.join("Library").join("Application Support"),
        _ => {
            set("XDG_DATA_HOME").or_else(|| set("HOME").map(|h| h.join(".local").join("share")))?
        }
    };
    Some(base.join("four-nations").join("runs"))
}

/// The old default, `~/fn-agent-runs`.
pub fn legacy(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    env("HOME").filter(|v| !v.is_empty()).map(|h| PathBuf::from(h).join(LEGACY_NAME))
}

/// The runs folder, and why it was picked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunsDir {
    /// Absolute, because the player runs in a different directory (#567).
    pub path: PathBuf,
    /// Set when `path` is the old `~/fn-agent-runs`, kept because the new default (this field)
    /// doesn't exist yet and the old folder has runs.
    pub moved_to: Option<PathBuf>,
}

impl RunsDir {
    /// The runs folder from `env` on `os`: `$FN_AGENT_RUNS` if set, else [`data_home_runs`],
    /// except that the old `~/fn-agent-runs` is kept while the new folder doesn't exist
    /// (`exists`) and the old one has runs (`has_entries`). `None` with no override and no home.
    pub fn resolve(
        env: impl Fn(&str) -> Option<OsString>,
        os: &str,
        exists: impl Fn(&Path) -> bool,
        has_entries: impl Fn(&Path) -> bool,
    ) -> Option<Self> {
        if let Some(p) = env(ENV).filter(|v| !v.is_empty()) {
            return Some(Self { path: absolute(Path::new(&p)), moved_to: None });
        }
        let new = data_home_runs(&env, os).map(|p| absolute(&p));
        let old = legacy(&env).map(|p| absolute(&p));
        match (new, old) {
            (Some(new), Some(old)) if !exists(&new) && has_entries(&old) => {
                Some(Self { path: old, moved_to: Some(new) })
            }
            (Some(new), _) => Some(Self { path: new, moved_to: None }),
            (None, Some(old)) => Some(Self { path: old, moved_to: None }),
            (None, None) => None,
        }
    }

    /// [`Self::resolve`] for this process: its environment and filesystem, on the platform it
    /// was built for.
    pub fn for_process() -> Option<Self> {
        Self::resolve(
            |k| std::env::var_os(k),
            std::env::consts::OS,
            Path::exists,
            |p| std::fs::read_dir(p).is_ok_and(|mut d| d.next().is_some()),
        )
    }

    /// What to tell the owner when the old folder is still in use: where the runs are and how
    /// to move them. `None` when the folder is the new default or the override.
    pub fn note(&self) -> Option<String> {
        let to = self.moved_to.as_ref()?;
        let parent = to.parent().unwrap_or(to);
        Some(format!(
            "agent runs are still read from and written to {old}; the default is now {new}, \
             beside the saves. To move them: mkdir -p \"{parent}\" && mv \"{old}\" \"{new}\" \
             (or set ${ENV} to keep them where they are)",
            old = self.path.display(),
            new = to.display(),
            parent = parent.display(),
        ))
    }
}

fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
        move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| OsString::from(v))
    }

    fn resolve(
        e: impl Fn(&str) -> Option<OsString>,
        os: &str,
        existing: &'static [&'static str],
        with_runs: &'static [&'static str],
    ) -> Option<RunsDir> {
        RunsDir::resolve(
            e,
            os,
            |p| existing.iter().any(|x| Path::new(x) == p),
            |p| with_runs.iter().any(|x| Path::new(x) == p),
        )
    }

    fn at(p: &str) -> Option<RunsDir> {
        Some(RunsDir { path: PathBuf::from(p), moved_to: None })
    }

    #[test]
    fn each_platform_puts_runs_in_its_data_dir() {
        let xdg = env(&[("XDG_DATA_HOME", "/d"), ("HOME", "/h")]);
        assert_eq!(data_home_runs(xdg, "linux"), Some(PathBuf::from("/d/four-nations/runs")));
        assert_eq!(
            data_home_runs(env(&[("HOME", "/h")]), "linux"),
            Some(PathBuf::from("/h/.local/share/four-nations/runs"))
        );
        assert_eq!(
            data_home_runs(env(&[("XDG_DATA_HOME", ""), ("HOME", "/h")]), "linux"),
            Some(PathBuf::from("/h/.local/share/four-nations/runs"))
        );
        assert_eq!(
            data_home_runs(env(&[("HOME", "/h")]), "macos"),
            Some(PathBuf::from("/h/Library/Application Support/four-nations/runs"))
        );
        assert_eq!(
            data_home_runs(env(&[("APPDATA", "C:/a")]), "windows"),
            Some(PathBuf::from("C:/a/four-nations/runs"))
        );
        assert_eq!(data_home_runs(env(&[]), "linux"), None);
        assert_eq!(data_home_runs(env(&[("HOME", "/h")]), "windows"), None);
    }

    #[test]
    fn the_override_wins_and_is_made_absolute() {
        let both = env(&[(ENV, "/x"), ("HOME", "/h")]);
        assert_eq!(resolve(both, "linux", &[], &["/h/fn-agent-runs"]), at("/x"));
        let rel = resolve(env(&[(ENV, "rel-runs")]), "linux", &[], &[]).unwrap();
        assert!(rel.path.is_absolute(), "{}", rel.path.display());
        assert!(rel.path.ends_with("rel-runs"));
        // An empty override is no override.
        let empty = env(&[(ENV, ""), ("HOME", "/h")]);
        assert_eq!(resolve(empty, "linux", &[], &[]), at("/h/.local/share/four-nations/runs"));
    }

    #[test]
    fn with_no_old_runs_the_default_is_the_data_home() {
        let e = env(&[("HOME", "/h")]);
        assert_eq!(resolve(e, "linux", &[], &[]), at("/h/.local/share/four-nations/runs"));
        let e = env(&[("HOME", "/h")]);
        assert_eq!(resolve(e, "macos", &[], &[]).unwrap().note(), None);
    }

    #[test]
    fn old_runs_are_kept_until_the_new_folder_exists() {
        const NEW: &str = "/h/.local/share/four-nations/runs";
        let new = NEW;
        let kept = resolve(env(&[("HOME", "/h")]), "linux", &[], &["/h/fn-agent-runs"]).unwrap();
        assert_eq!(kept.path, PathBuf::from("/h/fn-agent-runs"));
        assert_eq!(kept.moved_to, Some(PathBuf::from(new)));
        let note = kept.note().unwrap();
        for want in ["/h/fn-agent-runs", new, "mv ", "FN_AGENT_RUNS"] {
            assert!(note.contains(want), "{note}");
        }
        // Once the new folder exists, it is used and the old one is left alone.
        let moved = resolve(env(&[("HOME", "/h")]), "linux", &[NEW], &["/h/fn-agent-runs", NEW]);
        assert_eq!(moved, at(new));
        // An empty old folder doesn't hold the default back.
        assert_eq!(resolve(env(&[("HOME", "/h")]), "linux", &[], &[]), at(new));
    }
}

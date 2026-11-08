//! Repo maintenance commands. Run from anywhere in the repo:
//!
//!     cargo xtask ci [--fast] [--no-goldens]
//!                               fmt + clippy + tests for the whole workspace; --fast uses the
//!                               `ci` profile (opt-level 1 for our own crates), as CI does;
//!                               --no-goldens sets FN_GOLDEN=skip, so every golden comparison
//!                               is a no-op that passes. `ci --fast` with no other flags is
//!                               exactly what GitHub CI runs.
//!     cargo xtask bless         re-blesses every golden (fn_tui frames, fn_sim mapgen and
//!                               embark text) with FN_BLESS=1, at the `ci` profile's
//!                               opt-level 1. Runs only the test files that contain a golden
//!                               comparison (found by scanning `crates/*/tests/*.rs` for
//!                               `FN_BLESS`), not every test, and not at opt-level 0 like a bare
//!                               `FN_BLESS=1 cargo test` would. Prints which golden files
//!                               changed.
//!     cargo xtask check [-p CRATE ...] [--fast]
//!                               fmt and clippy for the whole workspace (`-D warnings`), then
//!                               tests -- with goldens always skipped -- for only the named
//!                               crates (`-p fn_sim`, `-p fn_tui`, ...; repeatable). With no
//!                               `-p`, picks the crates changed against `main` (committed,
//!                               staged, unstaged and untracked), including their dependents (a
//!                               change in fn_sim also tests fn_ui and fn_tui, since both depend
//!                               on it).
//!     cargo xtask agent-run [options]
//!                               starts an agent run (docs/design/agent-play.md): a Claude Code
//!                               session plays the game through `fn_agent serve` while you watch
//!                               it with `fn_tui --watch`. `scripts/agent-run.sh` is a thin
//!                               wrapper around this. See `cargo xtask agent-run --help` for
//!                               every option, or `cargo xtask agent-run watch [RUN_DIR]` to
//!                               watch or replay a run.
//!     cargo xtask agent-eval --track ID --champion DIR [--json] [--out DIR]
//!                               the clean evaluation of a strategy (#529): replays DIR/handoff.json's
//!                               `governor` script and plan with no model on every train and
//!                               held-out seed of a data/tracks.toml track, one at a time, and
//!                               prints the score per seed, the train mean and the held-out
//!                               scores. See `cargo xtask agent-eval --help`.
//!
//! `root()` finds the repo root at runtime with `git rev-parse --show-toplevel`, so an `xtask`
//! binary built from one worktree's `target/` still works when run from another (a shared
//! `CARGO_TARGET_DIR`, or a copy seeded by `scripts/warm-target.sh`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod agent_eval;
mod agent_run;
mod lint;

type Result<T = ()> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match dispatch(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn help() -> String {
    include_str!("main.rs")
        .lines()
        .skip(1)
        .take_while(|l| l.starts_with("//!"))
        .map(|l| l.trim_start_matches("//!"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn dispatch(args: &[&str]) -> Result {
    match args.first().copied() {
        Some("ci") => {
            // Kept as an alias: before #281 `ci sim` ran everything except the Bevy client.
            let rest = match args.get(1) {
                Some(&"sim") => &args[2..],
                _ => &args[1..],
            };
            let known = ["--fast", "--no-goldens"];
            if rest.iter().any(|a| !known.contains(a)) {
                return Err(help());
            }
            ci(rest.contains(&"--fast"), rest.contains(&"--no-goldens"))
        }
        Some("bless") => {
            if args.len() > 1 {
                return Err(help());
            }
            bless()
        }
        Some("check") => {
            let mut crates = Vec::new();
            let mut fast = false;
            let mut i = 1;
            while i < args.len() {
                match args[i] {
                    "-p" => {
                        i += 1;
                        let Some(c) = args.get(i) else { return Err(help()) };
                        crates.push(c.to_string());
                    }
                    "--fast" => fast = true,
                    _ => return Err(help()),
                }
                i += 1;
            }
            check(&crates, fast)
        }
        Some("agent-run") => agent_run::run(&args[1..]),
        Some("agent-eval") => agent_eval::run(&args[1..]),
        _ => Err(help()),
    }
}

pub(crate) fn root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(out) =
        Command::new("git").args(["rev-parse", "--show-toplevel"]).current_dir(&cwd).output()
        && out.status.success()
        && let Ok(s) = String::from_utf8(out.stdout)
        && !s.trim().is_empty()
    {
        return PathBuf::from(s.trim());
    }
    // No git (or it failed): fall back to where this binary was compiled from. Only right when
    // run from the worktree it was built in -- the bug `root()` used to always have.
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

// ---------------------------------------------------------------------------------------------
// cargo invocations
// ---------------------------------------------------------------------------------------------

fn cargo(args: &[&str]) -> Result {
    cargo_env(args, &[])
}

fn cargo_env(args: &[&str], env: &[(&str, &str)]) -> Result {
    println!("$ cargo {}", args.join(" "));
    let status = Command::new(env!("CARGO"))
        .args(args)
        .envs(env.iter().copied())
        .current_dir(root())
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("cargo {} failed", args.join(" "))) }
}

/// `--profile ci` on CI (GitHub sets `$CI`) or with `--fast`: unoptimized dependencies and no
/// debug info compile several times faster and use far less memory. Agents use `--fast` so
/// parallel worktree builds fit in RAM.
fn profile_flags(fast: bool) -> Vec<&'static str> {
    if fast || std::env::var_os("CI").is_some() { vec!["--profile", "ci"] } else { vec![] }
}

/// With `--fast`, optimize our own crates: the long simulation tests run about 6x faster for
/// ~30s more build time. An explicit `CARGO_PROFILE_CI_OPT_LEVEL` in the environment wins.
fn profile_env(fast: bool) -> Vec<(&'static str, &'static str)> {
    if fast && std::env::var_os("CARGO_PROFILE_CI_OPT_LEVEL").is_none() {
        vec![("CARGO_PROFILE_CI_OPT_LEVEL", "1")]
    } else {
        vec![]
    }
}

// ---------------------------------------------------------------------------------------------
// ci
// ---------------------------------------------------------------------------------------------

fn ci(fast: bool, no_goldens: bool) -> Result {
    let profile = profile_flags(fast);
    let mut env = profile_env(fast);
    if no_goldens {
        env.push(("FN_GOLDEN", "skip"));
    }
    let env: Vec<(&str, &str)> = env;

    cargo(&["fmt", "--all", "--check"])?;
    println!("$ xtask lint agent-surface-parity");
    lint::check(&root())?;
    let clippy: Vec<&str> =
        [&["clippy", "--workspace", "--all-targets"][..], &profile, &["--", "-D", "warnings"]]
            .concat();
    cargo_env(&clippy, &env)?;
    let test: Vec<&str> = [&["test", "--workspace"][..], &profile].concat();
    cargo_env(&test, &env)?;
    println!("ci OK");
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// check: fmt + clippy for the whole workspace, tests for only the crates that need them
// ---------------------------------------------------------------------------------------------

fn check(crates: &[String], fast: bool) -> Result {
    let profile = profile_flags(fast);
    let mut env = profile_env(fast);
    env.push(("FN_GOLDEN", "skip"));

    cargo(&["fmt", "--all", "--check"])?;
    println!("$ xtask lint agent-surface-parity");
    lint::check(&root())?;
    let clippy: Vec<&str> =
        [&["clippy", "--workspace", "--all-targets"][..], &profile, &["--", "-D", "warnings"]]
            .concat();
    cargo_env(&clippy, &env)?;

    let picked: Vec<String> =
        if crates.is_empty() { changed_crates(&root())? } else { crates.to_vec() };
    if picked.is_empty() {
        println!("check: no crates changed against main; skipping tests");
        return Ok(());
    }
    let mut test: Vec<&str> = vec!["test"];
    for c in &picked {
        test.push("-p");
        test.push(c);
    }
    test.extend(profile);
    cargo_env(&test, &env)?;
    println!("check OK ({})", picked.join(", "));
    Ok(())
}

/// Every workspace crate's package name (`fn_sim`, not `sim`) that depends, directly or
/// transitively, on `c` -- so a change in `c` also has to retest them.
fn dependents(c: &str) -> &'static [&'static str] {
    match c {
        "fn_sim" => &["fn_ui", "fn_tui", "fn_agent"],
        "fn_ui" => &["fn_tui", "fn_agent"],
        "fn_tui" => &["fn_agent"],
        _ => &[],
    }
}

/// The workspace crate(s) a changed repo-relative path (as `git` prints it) belongs to.
/// `data/*` is compiled into `fn_sim` with `include_str!`, so it counts as a `fn_sim` change.
/// A root `Cargo.toml`/`Cargo.lock` change could affect any crate's dependencies, so it counts
/// as a change to all of them.
fn crates_for_path(p: &str) -> Vec<&'static str> {
    if p == "Cargo.toml" || p == "Cargo.lock" {
        return vec!["fn_sim", "fn_ui", "fn_tui", "fn_agent", "xtask"];
    }
    if p.starts_with("data/") || p.starts_with("crates/sim/") {
        return vec!["fn_sim"];
    }
    if p.starts_with("crates/ui/") {
        return vec!["fn_ui"];
    }
    if p.starts_with("crates/tui/") {
        return vec!["fn_tui"];
    }
    if p.starts_with("crates/agent/") {
        return vec!["fn_agent"];
    }
    if p.starts_with("xtask/") {
        return vec!["xtask"];
    }
    vec![]
}

/// Every crate changed against `main`: committed (`git diff --name-only main...HEAD`),
/// uncommitted (tracked or not), plus every dependent of each -- so testing `fn_sim` alone also
/// covers `fn_ui` and `fn_tui`.
fn changed_crates(root: &Path) -> Result<Vec<String>> {
    let mut paths = git_lines(root, &["diff", "--name-only", "main...HEAD"])?;
    paths.extend(git_lines(root, &["diff", "--name-only", "HEAD"])?);
    paths.extend(git_lines(root, &["ls-files", "--others", "--exclude-standard"])?);

    let mut out: Vec<String> = Vec::new();
    for p in &paths {
        for c in crates_for_path(p) {
            if !out.iter().any(|x| x == c) {
                out.push(c.to_string());
            }
        }
    }
    for c in out.clone() {
        for d in dependents(&c) {
            if !out.iter().any(|x| x == d) {
                out.push(d.to_string());
            }
        }
    }
    Ok(out)
}

fn git_lines(root: &Path, args: &[&str]) -> Result<Vec<String>> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect())
}

// ---------------------------------------------------------------------------------------------
// bless
// ---------------------------------------------------------------------------------------------

fn bless() -> Result {
    let root = root();
    let targets = golden_test_targets(&root)?;
    if targets.is_empty() {
        println!("bless: no golden-producing test files found");
        return Ok(());
    }
    let env = [("FN_BLESS", "1"), ("CARGO_PROFILE_CI_OPT_LEVEL", "1")];
    for (pkg, stems) in &targets {
        let mut args: Vec<&str> = vec!["test", "-p", pkg];
        for s in stems {
            args.push("--test");
            args.push(s);
        }
        args.push("--profile");
        args.push("ci");
        cargo_env(&args, &env)?;
    }
    print_changed_goldens(&root)?;
    Ok(())
}

/// Every `(package, [test-file stems])` pair under `crates/*/tests` with at least one file that
/// references `FN_BLESS` -- the convention every golden-comparison helper follows (see e.g.
/// `crates/sim/tests/mapgen.rs`'s `fn golden` or `crates/tui/tests/frames.rs`'s). A whole test
/// file runs even if only one of its tests is a golden comparison (several non-`golden_`-named
/// tests in `crates/tui/tests/footprints.rs` call its `golden` helper too), so nothing is missed
/// and nothing goes stale.
fn golden_test_targets(root: &Path) -> Result<Vec<(String, Vec<String>)>> {
    let mut out = Vec::new();
    let crates_dir = root.join("crates");
    let mut dirs: Vec<PathBuf> = fs::read_dir(&crates_dir)
        .map_err(|e| format!("reading {}: {e}", crates_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let tests_dir = dir.join("tests");
        if !tests_dir.is_dir() {
            continue;
        }
        let Some(crate_name) = dir.file_name().and_then(|n| n.to_str()) else { continue };
        let pkg = format!("fn_{crate_name}");
        let mut files: Vec<PathBuf> = fs::read_dir(&tests_dir)
            .map_err(|e| format!("reading {}: {e}", tests_dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
            .collect();
        files.sort();
        let mut stems = Vec::new();
        for f in files {
            let src =
                fs::read_to_string(&f).map_err(|e| format!("reading {}: {e}", f.display()))?;
            if src.contains("FN_BLESS")
                && let Some(stem) = f.file_stem().and_then(|s| s.to_str())
            {
                stems.push(stem.to_string());
            }
        }
        if !stems.is_empty() {
            out.push((pkg, stems));
        }
    }
    Ok(out)
}

/// Every `tests/golden` directory under `crates/*`.
fn golden_dirs(root: &Path) -> Vec<PathBuf> {
    let crates_dir = root.join("crates");
    let Ok(entries) = fs::read_dir(&crates_dir) else { return Vec::new() };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path().join("tests").join("golden"))
        .filter(|p| p.is_dir())
        .collect()
}

fn print_changed_goldens(root: &Path) -> Result {
    let dirs = golden_dirs(root);
    if dirs.is_empty() {
        return Ok(());
    }
    let rel: Vec<String> =
        dirs.iter().map(|d| d.strip_prefix(root).unwrap().to_string_lossy().into_owned()).collect();
    let mut args = vec!["status", "--porcelain", "--untracked-files=all", "--"];
    args.extend(rel.iter().map(String::as_str));
    let out = Command::new("git")
        .args(&args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("git status: {e}"))?;
    let lines: Vec<&str> = std::str::from_utf8(&out.stdout)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        println!("bless: no golden files changed");
    } else {
        println!("bless: golden files changed:");
        for line in lines {
            println!("  {line}");
        }
    }
    Ok(())
}

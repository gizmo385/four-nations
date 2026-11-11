//! Repo maintenance commands. Run from anywhere in the repo:
//!
//!     cargo xtask ci [--fast] [--no-goldens]
//!                               fmt + clippy + tests for the whole workspace, plus clippy on
//!                               fn_sim/fn_ui/fn_tui for wasm32 (#594); --fast uses the
//!                               `ci` profile (opt-level 1 for our own crates), as CI does;
//!                               --no-goldens sets FN_GOLDEN=skip, so every golden comparison
//!                               is a no-op that passes. `ci --fast` with no other flags is
//!                               exactly what GitHub CI runs. Tests run with cargo-nextest
//!                               when it's installed (as on CI), else with `cargo test`.
//!     cargo xtask ci --docs-only
//!                               what GitHub CI runs for a change that touches only documents
//!                               (`scripts/ci-scope.sh`): just the tests that read `docs/`.
//!     cargo xtask bless [--expect FILE...]
//!                               re-blesses every golden (fn_tui frames, fn_sim mapgen and
//!                               embark text) with FN_BLESS=1, at the `ci` profile's
//!                               opt-level 1. Runs only the test files that contain a golden
//!                               comparison (found by scanning `crates/*/tests/*.rs` for
//!                               `FN_BLESS`), not every test, and not at opt-level 0 like a bare
//!                               `FN_BLESS=1 cargo test` would. Prints which golden files
//!                               changed. With `--expect`, every changed golden must be one of the
//!                               FILEs (repo-relative paths; a path ending in `/` covers its
//!                               directory; no FILEs means none may change), else it exits
//!                               non-zero naming the undeclared ones (#601). The integrator
//!                               passes the union of what a batch's branches declared.
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
//!     cargo xtask agent-eval --track ID --champion DIR [--threads N] [--json] [--out DIR]
//!                               the clean evaluation of a strategy (#529): replays DIR/handoff.json's
//!                               `governor` script and plan with no model on every screen, gate
//!                               and owner-only seed of a data/tracks.toml track (#571), in
//!                               parallel, and prints the score per seed, each tier's mean and
//!                               the failure counts, with par and a do-nothing control on the
//!                               same seeds (#572). See `cargo xtask agent-eval --help`.
//!     cargo xtask agent-compare [--tier owner] CONDITION...
//!                               compares conditions with no model (#571): campaign folders'
//!                               champions or a fixed governor and plan, each replayed on a
//!                               tier's seeds, paired by seed against the first condition, with
//!                               bootstrap intervals and failure counts. See `cargo xtask
//!                               agent-compare --help`.
//!     cargo xtask web [--serve] [--port N] [LOG...]
//!                               builds the web replayer (#595: web/ plus fn_web.wasm) into
//!                               target/replayer/, and with --serve serves it on localhost; each LOG
//!                               is listed in its run picker. See `cargo xtask web --help`.
//!
//! `root()` finds the repo root at runtime with `git rev-parse --show-toplevel`, so an `xtask`
//! binary built from one worktree's `target/` still works when run from another (a shared
//! `CARGO_TARGET_DIR`, or a copy seeded by `scripts/warm-target.sh`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod agent_compare;
mod agent_eval;
mod agent_run;
mod lint;
mod references;
mod web;

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
            if rest == ["--docs-only"] {
                return ci_docs_only();
            }
            let known = ["--fast", "--no-goldens"];
            if rest.iter().any(|a| !known.contains(a)) {
                return Err(help());
            }
            ci(rest.contains(&"--fast"), rest.contains(&"--no-goldens"))
        }
        Some("bless") => match args.get(1).copied() {
            None => bless(None),
            Some("--expect") => bless(Some(&args[2..])),
            Some(_) => Err(help()),
        },
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
        Some("agent-compare") => agent_compare::run(&args[1..]),
        Some("web") => web::run(&args[1..]),
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
    println!("$ xtask lint test-modules");
    lint_test_modules(&root())?;
    let clippy: Vec<&str> = [
        &["clippy", "--workspace", "--all-targets"][..],
        EXCLUDED,
        &profile,
        &["--", "-D", "warnings"],
    ]
    .concat();
    cargo_env(&clippy, &env)?;
    clippy_wasm(&profile, &env)?;
    cargo_env(&test_args(&[&["--workspace"][..], EXCLUDED].concat(), &profile)?, &env)?;
    println!("ci OK");
    Ok(())
}

/// What CI runs for a change that touches only documents (`scripts/ci-scope.sh` decides): the
/// tests that read `docs/`, and nothing else. No Rust source changed, so fmt, clippy and every
/// other test would say what they said on the commit before.
///
/// Today that is one test, in `fn_harness`: every balance target names a real section of a
/// design doc, so renaming a heading can break it. A new test that reads `docs/` belongs in
/// [`DOCS_TESTS`], and `ci-scope.sh` must not count as a document any file a test reads from
/// elsewhere.
fn ci_docs_only() -> Result {
    let profile = profile_flags(true);
    let env = profile_env(true);
    // `--workspace`, not `-p`: cargo picks dependency features for the packages it is asked to
    // build, so `-p fn_harness` alone would compile `fn_sim` and its dependencies a second way
    // (two minutes) instead of reusing what the whole-workspace gate left in target/.
    let mut args = test_args(&["--workspace"], &profile)?;
    // Test name filters: `cargo test` and nextest both take substrings after the options.
    args.extend(DOCS_TESTS);
    cargo_env(&args, &env)?;
    println!("ci OK (documents only)");
    Ok(())
}

/// The tests that read files under `docs/`, by name.
const DOCS_TESTS: [&str; 1] = ["every_target_names_a_real_section_of_a_design_doc"];

/// The test command for `selection` (`--workspace`, or `-p CRATE ...`) at `profile`.
///
/// Uses `cargo nextest run` when cargo-nextest is installed: it runs every test binary's tests
/// in one pool, where `cargo test` runs the binaries one after another, so a binary with one
/// minute-long test no longer leaves the other cores idle. Settings are in `.config/nextest.toml`.
/// Doc-tests are skipped, which loses nothing: the workspace has none. Without nextest it falls
/// back to `cargo test`, except on CI ($CI set), where a missing nextest is an error so a broken
/// install can't quietly slow the gate back down.
fn test_args<'a>(selection: &[&'a str], profile: &[&'a str]) -> Result<Vec<&'a str>> {
    if nextest_installed() {
        // nextest's own `--profile` picks a nextest profile; the cargo one is `--cargo-profile`.
        let cargo_profile: Vec<&str> =
            profile.iter().map(|&a| if a == "--profile" { "--cargo-profile" } else { a }).collect();
        return Ok([&["nextest", "run"][..], selection, &cargo_profile].concat());
    }
    if std::env::var_os("CI").is_some() {
        return Err("cargo-nextest isn't installed; CI runs the tests with it \
                    (`cargo install cargo-nextest --locked`)"
            .to_owned());
    }
    println!(
        "(cargo-nextest isn't installed, so tests run with `cargo test`, one binary at a time)"
    );
    Ok([&["test"][..], selection, profile].concat())
}

/// Whether `cargo nextest` runs.
fn nextest_installed() -> bool {
    Command::new(env!("CARGO"))
        .args(["nextest", "--version"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Workspace members the gate leaves out: `fn_gui`, the egui window spike (#842), which pulls in a
/// windowing and GPU stack the gate has no use for. `cargo test -p fn_gui` runs its own tests.
const EXCLUDED: &[&str] = &["--exclude", "fn_gui"];

/// The crates the web replayer (#595) builds for the browser.
const WASM_CRATES: [&str; 5] = ["fn_runfmt", "fn_sim", "fn_ui", "fn_tui", "fn_web"];
pub(crate) const WASM_TARGET: &str = "wasm32-unknown-unknown";

/// Clippy (`-D warnings`) on [`WASM_CRATES`]' libraries for [`WASM_TARGET`] (#594), so a change
/// that breaks the web build -- a crossterm import outside `fn_tui::keyev`, a dependency with no
/// wasm support -- fails here instead of in the replayer's deploy.
fn clippy_wasm(profile: &[&str], env: &[(&str, &str)]) -> Result {
    if !wasm_target_installed() {
        return Err(format!(
            "the {WASM_TARGET} target isn't installed; add it with \
             `rustup target add {WASM_TARGET}` (the Nix shell has it)"
        ));
    }
    let mut args = vec!["clippy", "--lib", "--target", WASM_TARGET];
    for c in WASM_CRATES {
        args.extend(["-p", c]);
    }
    args.extend(profile);
    args.extend(["--", "-D", "warnings"]);
    cargo_env(&args, env)
}

/// Whether the toolchain has the standard library for [`WASM_TARGET`].
fn wasm_target_installed() -> bool {
    let Ok(out) =
        Command::new("rustc").args(["--print", "target-libdir", "--target", WASM_TARGET]).output()
    else {
        return false;
    };
    out.status.success() && Path::new(String::from_utf8_lossy(&out.stdout).trim()).is_dir()
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
    println!("$ xtask lint test-modules");
    lint_test_modules(&root())?;
    let clippy: Vec<&str> = [
        &["clippy", "--workspace", "--all-targets"][..],
        EXCLUDED,
        &profile,
        &["--", "-D", "warnings"],
    ]
    .concat();
    cargo_env(&clippy, &env)?;
    clippy_wasm(&profile, &env)?;

    let picked: Vec<String> =
        if crates.is_empty() { changed_crates(&root())? } else { crates.to_vec() };
    if picked.is_empty() {
        println!("check: no crates changed against main; skipping tests");
        return Ok(());
    }
    let mut selection: Vec<&str> = Vec::new();
    for c in &picked {
        selection.push("-p");
        selection.push(c);
    }
    cargo_env(&test_args(&selection, &profile)?, &env)?;
    println!("check OK ({})", picked.join(", "));
    Ok(())
}

/// Every workspace crate's package name (`fn_sim`, not `sim`) that depends, directly or
/// transitively, on `c` -- so a change in `c` also has to retest them.
fn dependents(c: &str) -> &'static [&'static str] {
    match c {
        "fn_runfmt" => &["fn_tui", "fn_agent", "fn_web", "xtask"],
        "fn_sim" => &["fn_ui", "fn_tui", "fn_agent", "fn_harness"],
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
        return vec!["fn_runfmt", "fn_sim", "fn_ui", "fn_tui", "fn_agent", "fn_harness", "xtask"];
    }
    // `data/harness.toml` is the harness crate's own (compiled into `fn_harness`, never `fn_sim`).
    if p == "data/harness.toml" || p.starts_with("crates/harness/") {
        return vec!["fn_harness"];
    }
    if p.starts_with("data/") || p.starts_with("crates/sim/") {
        return vec!["fn_sim"];
    }
    if p.starts_with("crates/runfmt/") {
        return vec!["fn_runfmt"];
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

fn bless(expect: Option<&[&str]>) -> Result {
    let root = root();
    let targets = golden_test_targets(&root)?;
    if targets.is_empty() {
        println!("bless: no golden-producing test files found");
        return check_expected(&[], expect);
    }
    let env = [("FN_BLESS", "1"), ("CARGO_PROFILE_CI_OPT_LEVEL", "1")];
    for (pkg, stems) in &targets {
        // The files are modules of the crate's one test binary, `it` (tests/it.rs), so each is
        // picked by its module path. A filter is a substring: `frames::` also runs a module
        // whose name ends in `frames`, which only blesses a little more.
        let filters: Vec<String> = stems.iter().map(|s| format!("{s}::")).collect();
        let mut args: Vec<&str> = vec!["test", "-p", pkg, "--test", "it", "--profile", "ci", "--"];
        args.extend(filters.iter().map(String::as_str));
        cargo_env(&args, &env)?;
    }
    let changed = print_changed_goldens(&root)?;
    check_expected(&changed, expect)
}

/// With `--expect`, fails if any changed golden isn't declared (#601). A declared file that
/// didn't change is only a note: a branch may have declared a change another branch undid.
fn check_expected(changed: &[String], expect: Option<&[&str]>) -> Result {
    let Some(expect) = expect else { return Ok(()) };
    let covers = |e: &str, path: &str| {
        let e = e.trim_start_matches("./");
        path == e || (e.ends_with('/') && path.starts_with(e))
    };
    let undeclared: Vec<&String> =
        changed.iter().filter(|c| !expect.iter().any(|e| covers(e, c))).collect();
    for e in expect {
        if !changed.iter().any(|c| covers(e, c)) {
            println!("bless: note: declared but unchanged: {e}");
        }
    }
    if undeclared.is_empty() {
        println!("bless: all changed goldens were declared");
        return Ok(());
    }
    let list: Vec<String> = undeclared.iter().map(|c| format!("  {c}")).collect();
    Err(format!(
        "bless: golden files changed that no branch declared:\n{}\n\
         Find the branch that changed them (bless after each squash, or bisect) and hand it back.",
        list.join("\n")
    ))
}

/// Every `(package, [test-file stems])` pair under `crates/*/tests` with at least one file that
/// references `FN_BLESS` -- the convention every golden-comparison helper follows (see e.g.
/// `crates/sim/tests/mapgen.rs`'s `fn golden` or `crates/tui/tests/reference_goldens.rs`'s). A whole test
/// file runs even if only one of its tests is a golden comparison (it keeps every test of the
/// file, not just the `reference_*` ones, which is cheap), so nothing is missed
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
            .filter(|p| p.file_name().and_then(|n| n.to_str()) != Some(TEST_ROOT))
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

/// The root file of a crate's one integration-test binary, in its `tests/` folder.
const TEST_ROOT: &str = "it.rs";

/// Fails when a test file isn't part of its crate's test binary. Each crate builds one
/// integration-test binary from `tests/it.rs` (`autotests = false`), and every other `.rs` file
/// in `tests/` has to be a `mod` line there. A file with no line compiles into nothing and its
/// tests never run, silently, so the gate checks it (part of `cargo xtask ci` and `check`).
fn lint_test_modules(root: &Path) -> Result {
    let mut dirs: Vec<PathBuf> = fs::read_dir(root.join("crates"))
        .map_err(|e| format!("reading crates/: {e}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path().join("tests"))
        .collect();
    dirs.push(root.join("xtask").join("tests"));
    dirs.sort();
    let mut problems = Vec::new();
    for dir in dirs.iter().filter(|d| d.is_dir()) {
        let shown = dir.strip_prefix(root).unwrap_or(dir).display().to_string();
        let files: Vec<String> = fs::read_dir(dir)
            .map_err(|e| format!("reading {shown}: {e}"))?
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".rs") && n != TEST_ROOT)
            .collect();
        let root_src = fs::read_to_string(dir.join(TEST_ROOT)).unwrap_or_default();
        problems.extend(unlisted_test_files(&root_src, files).into_iter().map(|f| {
            format!(
                "{shown}/{f} isn't in {shown}/{TEST_ROOT}: add `mod {};`",
                f.trim_end_matches(".rs")
            )
        }));
    }
    if problems.is_empty() {
        return Ok(());
    }
    Err(format!(
        "test files that are never built, so their tests never run:\n  {}",
        problems.join("\n  ")
    ))
}

/// The `files` (names in a `tests/` folder) that `root_src` (its `it.rs`) has no `mod` line for.
fn unlisted_test_files(root_src: &str, mut files: Vec<String>) -> Vec<String> {
    let listed: Vec<&str> = root_src
        .lines()
        .filter_map(|l| l.trim().strip_prefix("mod ")?.strip_suffix(';'))
        .map(str::trim)
        .collect();
    files.retain(|f| !listed.contains(&f.trim_end_matches(".rs")));
    files.sort();
    files
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

/// Prints the changed golden files and returns their repo-relative paths.
fn print_changed_goldens(root: &Path) -> Result<Vec<String>> {
    let dirs = golden_dirs(root);
    if dirs.is_empty() {
        return Ok(Vec::new());
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
        for line in &lines {
            println!("  {line}");
        }
    }
    // Porcelain lines are `XY path`; a rename reads `XY old -> new`, so take the last path.
    Ok(lines
        .iter()
        .map(|l| l[3..].rsplit(" -> ").next().unwrap_or_default().trim_matches('"').to_owned())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_file_with_no_mod_line_is_reported() {
        let root_src = "//! Docs.\n\nmod frames;\n  mod watch ;\n// mod hidden;\n";
        let files =
            ["watch.rs", "new_one.rs", "frames.rs", "hidden.rs"].map(str::to_owned).to_vec();
        assert_eq!(unlisted_test_files(root_src, files), ["hidden.rs", "new_one.rs"]);
    }

    #[test]
    fn every_test_file_in_this_repository_is_listed() {
        lint_test_modules(&root()).unwrap();
    }
}

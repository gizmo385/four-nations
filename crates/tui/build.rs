//! Bakes a build id into the binary as `FN_BUILD_ID` (#566), for `fn_tui::buildid`.
//!
//! A run log's data digest covers the game's data and nothing else, so two logs made by different
//! code can look alike, and the same keys can score differently once the sim has changed. The
//! build id says which code made (or re-scored) a log, so a reader can tell when a score may not
//! reproduce.
//!
//! The id is `COMMIT` for a clean tree and `COMMIT+HEX` for a dirty one:
//!
//! * `COMMIT` is the last commit that touched anything the build is made of (`crates/` other than
//!   the harness and tooling, `data/`, and the manifests), not `HEAD`. A commit that only
//!   changes docs or `xtask` leaves it alone, so such a commit neither warns nor re-scores
//!   anything.
//! * `HEX` is a hash of the uncommitted changes to those same paths, so an edit shows, the same
//!   edit gives the same id, and a tree that is dirty for a whole afternoon of unrelated runs
//!   isn't a new build each time `git status` is looked at.
//! * Without git (a source tarball, a `cargo install`) it is `unknown`.
//!
//! `FN_BUILD_ID_OVERRIDE`, when set, is the id, and git is neither asked nor watched. GitHub CI
//! sets it (.github/workflows/ci.yml): a pull request's checkout is a detached, one-commit clone,
//! so the branch file this script watches on `main` is missing there (cargo reruns the script)
//! and the last commit to touch any path is always the checkout's own (a new id every run). Left
//! alone, that rebuilt `fn_tui` and everything above it on every pull request, whatever it
//! changed. CI publishes nothing it builds and keeps no cache between runs that is keyed by the
//! id, so one fixed id there is harmless.
//!
//! This is `fn_tui`'s build script, not `fn_sim`'s: `fn_sim` is built at opt-level 3 and a rerun
//! there would rebuild it on every commit. `fn_tui` is the lowest crate every consumer shares
//! (`fn_agent` and the run browser both link it).

use std::path::{Path, PathBuf};
use std::process::Command;

/// The environment variable that, when set and not empty, is the build id as it stands.
const OVERRIDE: &str = "FN_BUILD_ID_OVERRIDE";

/// What a score can depend on, relative to the repository root.
const PATHS: [&str; 7] =
    ["crates/sim", "crates/ui", "crates/tui", "crates/agent", "data", "Cargo.toml", "Cargo.lock"];

fn main() {
    // CI names its builds itself (see the module doc): no git state is read or watched.
    println!("cargo:rerun-if-env-changed={OVERRIDE}");
    if let Some(id) = std::env::var(OVERRIDE).ok().filter(|id| !id.is_empty()) {
        println!("cargo:rustc-env=FN_BUILD_ID={id}");
        println!("cargo:rerun-if-changed=build.rs");
        return;
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let id = build_id(&root).unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=FN_BUILD_ID={id}");

    // Rerun when the code changes (the dirty hash), and when a commit moves HEAD (the commit).
    // Files that don't exist are left out: cargo treats a missing one as always changed.
    println!("cargo:rerun-if-changed=build.rs");
    for p in PATHS {
        println!("cargo:rerun-if-changed={}", root.join(p).display());
    }
    for p in git_state_files(&root) {
        println!("cargo:rerun-if-changed={}", p.display());
    }
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn build_id(root: &Path) -> Option<String> {
    let mut log = vec!["log", "-1", "--format=%h", "--"];
    log.extend(PATHS);
    let commit = git(root, &log).filter(|c| !c.is_empty())?;

    // Tracked changes, staged or not, and the contents of new files.
    let mut diff = vec!["diff", "HEAD", "--"];
    diff.extend(PATHS);
    let mut dirty = git(root, &diff)?;
    let mut new = vec!["ls-files", "--others", "--exclude-standard", "--"];
    new.extend(PATHS);
    for file in git(root, &new)?.lines() {
        dirty.push_str(file);
        dirty.push_str(&std::fs::read_to_string(root.join(file)).unwrap_or_default());
    }
    if dirty.is_empty() {
        return Some(commit);
    }
    // FNV-1a, as `Defs::id_hash` does: stable across platforms and Rust versions.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in dirty.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    Some(format!("{commit}+{:08x}", h >> 32))
}

/// The files that change when a commit lands: `HEAD`, the branch it names, and `packed-refs`.
fn git_state_files(root: &Path) -> Vec<PathBuf> {
    let path = |name: &str| git(root, &["rev-parse", "--path-format=absolute", "--git-path", name]);
    let mut files = vec![path("HEAD")];
    if let Some(branch) = git(root, &["symbolic-ref", "-q", "HEAD"]) {
        files.push(path(&branch));
    }
    files.push(path("packed-refs"));
    files.into_iter().flatten().map(PathBuf::from).filter(|p| p.exists()).collect()
}

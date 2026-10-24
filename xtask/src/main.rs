//! Repo maintenance commands. Run from anywhere in the repo:
//!
//!     cargo xtask assets [--release]   verify assets/manifest.toml covers and matches assets/
//!     cargo xtask ci [sim|client] [--fast]
//!                                      fmt + clippy + tests + assets (default: everything);
//!                                      --fast uses the `ci` profile, as CI does
//!     cargo xtask screenshot SEED NATION OUT.png [WIDTH HEIGHT]
//!                                      render the client headless (no window) to a PNG

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde::Deserialize;
use sha2::{Digest, Sha256};

type Result<T = ()> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let fast = args.first() == Some(&"ci") && args.contains(&"--fast");
    let args: Vec<&str> = args.into_iter().filter(|a| !(fast && *a == "--fast")).collect();
    let result = match args.as_slice() {
        ["assets"] => assets(false),
        ["assets", "--release"] => assets(true),
        ["ci"] => ci(Scope::All, fast),
        ["ci", "sim"] => ci(Scope::Sim, fast),
        ["ci", "client"] => ci(Scope::Client, fast),
        ["screenshot", seed, nation, out] => screenshot(seed, nation, out, "1280", "720"),
        ["screenshot", seed, nation, out, w, h] => screenshot(seed, nation, out, w, h),
        _ => Err(include_str!("main.rs")
            .lines()
            .skip(1)
            .take_while(|l| l.starts_with("//!"))
            .map(|l| l.trim_start_matches("//!"))
            .collect::<Vec<_>>()
            .join("\n")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

// ---------------------------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct Manifest {
    source: Vec<AssetSource>,
    file: Vec<AssetFile>,
}

#[derive(Deserialize)]
#[allow(dead_code)] // Fields are read by humans; we only check that they are present.
struct AssetSource {
    id: String,
    title: String,
    author: String,
    url: String,
    license: String,
    release_ok: bool,
    retrieved: String,
    retrieved_by: String,
}

#[derive(Deserialize)]
struct AssetFile {
    path: String,
    source: String,
    sha256: String,
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result {
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            walk(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn assets(release: bool) -> Result {
    let dir = root().join("assets");
    let manifest_path = dir.join("manifest.toml");
    let text =
        fs::read_to_string(&manifest_path).map_err(|e| format!("assets/manifest.toml: {e}"))?;
    let manifest: Manifest =
        toml::from_str(&text).map_err(|e| format!("assets/manifest.toml: {e}"))?;

    let sources: BTreeMap<&str, &AssetSource> =
        manifest.source.iter().map(|s| (s.id.as_str(), s)).collect();
    let listed: BTreeMap<&str, &AssetFile> =
        manifest.file.iter().map(|f| (f.path.as_str(), f)).collect();

    let mut on_disk = Vec::new();
    walk(&dir, &mut on_disk)?;
    let mut errors = Vec::new();
    let mut not_releasable = BTreeSet::new();

    for path in &on_disk {
        if *path == manifest_path {
            continue;
        }
        let rel = path.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/");
        let Some(entry) = listed.get(rel.as_str()) else {
            errors.push(format!("{rel}: not listed in assets/manifest.toml"));
            continue;
        };
        let bytes = fs::read(path).map_err(|e| format!("{rel}: {e}"))?;
        let hash: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        if hash != entry.sha256 {
            errors.push(format!("{rel}: sha256 mismatch (file changed; update manifest + notes)"));
        }
        match sources.get(entry.source.as_str()) {
            None => errors.push(format!("{rel}: unknown source {:?}", entry.source)),
            Some(s) if !s.release_ok => {
                not_releasable.insert(s.id.as_str());
            }
            Some(_) => {}
        }
    }
    for rel in listed.keys() {
        if !dir.join(rel).exists() {
            errors.push(format!("{rel}: listed in manifest but missing on disk"));
        }
    }

    if !not_releasable.is_empty() {
        let msg = format!("sources not cleared for release: {not_releasable:?}");
        if release {
            errors.push(msg);
        } else {
            eprintln!("warning: {msg}");
        }
    }
    if errors.is_empty() {
        println!("assets OK: {} files from {} sources", listed.len(), sources.len());
        Ok(())
    } else {
        Err(format!("asset check failed:\n  {}", errors.join("\n  ")))
    }
}

// ---------------------------------------------------------------------------------------------
// CI
// ---------------------------------------------------------------------------------------------

fn cargo(args: &[&str]) -> Result {
    println!("$ cargo {}", args.join(" "));
    let status = Command::new(env!("CARGO"))
        .args(args)
        .current_dir(root())
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("cargo {} failed", args.join(" "))) }
}

/// Which part of the workspace `cargo xtask ci` checks. CI runs `sim` and `client` as separate
/// jobs so changes that can't affect the client skip compiling Bevy entirely.
#[derive(Clone, Copy, PartialEq)]
enum Scope {
    All,
    /// Everything except the Bevy client: fmt, the sim, the shared UI crate, xtask and the asset check.
    Sim,
    /// Only the Bevy client.
    Client,
}

fn ci(scope: Scope, fast: bool) -> Result {
    let packages: &[&str] = match scope {
        Scope::All => &["--workspace"],
        Scope::Sim => &["-p", "fn_sim", "-p", "fn_ui", "-p", "xtask"],
        Scope::Client => &["-p", "fn_client"],
    };
    // On CI (GitHub sets $CI) or with --fast, build with the `ci` profile: unoptimized
    // dependencies and no debug info compile several times faster and use far less memory, and
    // tests don't need Bevy to run fast. Agents use --fast so parallel worktree builds fit in RAM.
    let profile: &[&str] =
        if fast || std::env::var_os("CI").is_some() { &["--profile", "ci"] } else { &[] };

    if scope != Scope::Client {
        cargo(&["fmt", "--all", "--check"])?;
    }
    let clippy = [&["clippy", "--all-targets"][..], packages, profile, &["--", "-D", "warnings"]];
    cargo(&clippy.concat())?;
    cargo(&[&["test"][..], packages, profile].concat())?;
    if scope != Scope::Client {
        assets(false)?;
    }
    println!("ci OK");
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Screenshot
// ---------------------------------------------------------------------------------------------

/// Renders one frame of the client headless (no window is ever created) and saves it as a PNG.
/// Goes through `scripts/run-client.sh`, which uses the flake devShell when Nix is present.
fn screenshot(seed: &str, nation: &str, out: &str, width: &str, height: &str) -> Result {
    for (name, n) in [("WIDTH", width), ("HEIGHT", height)] {
        if !n.parse::<u32>().is_ok_and(|n| n > 0) {
            return Err(format!("{name} must be a positive integer, got {n:?}"));
        }
    }
    // The client runs from the repo root, so resolve OUT against the caller's directory.
    let out = std::path::absolute(out).map_err(|e| format!("{out}: {e}"))?;
    let status = Command::new(root().join("scripts/run-client.sh"))
        .args([seed, nation])
        .env("FN_SCREENSHOT", &out)
        .env("FN_SCREENSHOT_SIZE", format!("{width}x{height}"))
        .current_dir(root())
        .status()
        .map_err(|e| format!("scripts/run-client.sh: {e}"))?;
    if !status.success() {
        return Err(format!("headless client failed ({status})"));
    }
    if !out.is_file() {
        return Err(format!("client exited but {} was not written", out.display()));
    }
    println!("wrote {}", out.display());
    Ok(())
}

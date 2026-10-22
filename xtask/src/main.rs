//! Repo maintenance commands. Run from anywhere in the repo:
//!
//!     cargo xtask assets [--release]   verify assets/manifest.toml covers and matches assets/
//!     cargo xtask ci                   fmt + clippy + tests + assets

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
    let result = match args.as_slice() {
        ["assets"] => assets(false),
        ["assets", "--release"] => assets(true),
        ["ci"] => ci(),
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

fn ci() -> Result {
    cargo(&["fmt", "--all", "--check"])?;
    cargo(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])?;
    cargo(&["test", "--workspace"])?;
    assets(false)?;
    println!("ci OK");
    Ok(())
}

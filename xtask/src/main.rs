//! Repo maintenance commands. Run from anywhere in the repo:
//!
//!     cargo xtask assets [--release]   verify assets/manifest.toml covers and matches assets/
//!     cargo xtask work list [STATUS]   list work items (optionally filtered by status)
//!     cargo xtask work next            ready items whose dependencies are all done
//!     cargo xtask work new "Title"     create the next work item from the template
//!     cargo xtask work check           validate every work item
//!     cargo xtask ci                   fmt + clippy + tests + assets + work check

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
        ["work", "list"] => work_list(None),
        ["work", "list", status] => work_list(Some(status)),
        ["work", "next"] => work_next(),
        ["work", "new", title] => work_new(title),
        ["work", "check"] => work_check().map(|items| println!("{} work items OK", items.len())),
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
// Work items
// ---------------------------------------------------------------------------------------------

const STATUSES: [&str; 6] = ["proposed", "ready", "in-progress", "review", "done", "dropped"];
const PRIORITIES: [&str; 4] = ["p0", "p1", "p2", "p3"];
const AREAS: [&str; 7] = ["sim", "client", "data", "art", "tooling", "design", "docs"];

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct WorkItem {
    id: String,
    title: String,
    status: String,
    priority: String,
    area: String,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    owner: String,
    #[allow(dead_code)] // Parsed to validate the date; read by humans.
    created: toml::value::Datetime,
    #[serde(skip)]
    file: String,
}

fn work_dir() -> PathBuf {
    root().join("work").join("items")
}

fn load_items() -> Result<Vec<WorkItem>> {
    let mut paths = Vec::new();
    walk(&work_dir(), &mut paths)?;
    paths.sort();
    let mut items = Vec::new();
    for path in paths.iter().filter(|p| p.extension().is_some_and(|e| e == "md")) {
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let text = fs::read_to_string(path).map_err(|e| format!("{file}: {e}"))?;
        let front = text
            .strip_prefix("+++\n")
            .and_then(|rest| rest.split_once("\n+++\n"))
            .map(|(front, _)| front)
            .ok_or_else(|| format!("{file}: missing +++ TOML frontmatter +++"))?;
        let mut item: WorkItem = toml::from_str(front).map_err(|e| format!("{file}: {e}"))?;
        item.file = file;
        items.push(item);
    }
    Ok(items)
}

fn work_check() -> Result<Vec<WorkItem>> {
    let items = load_items()?;
    let ids: BTreeMap<&str, &WorkItem> = items.iter().map(|i| (i.id.as_str(), i)).collect();
    let mut errors = Vec::new();
    if ids.len() != items.len() {
        errors.push("duplicate work item ids (two branches allocated the same id?)".to_string());
    }
    for i in &items {
        let e = |msg: String| format!("{}: {msg}", i.file);
        if !i.file.starts_with(&format!("{}-", i.id)) {
            errors.push(e(format!("filename must start with its id {:?}", i.id)));
        }
        if !STATUSES.contains(&i.status.as_str()) {
            errors.push(e(format!("status {:?} not in {STATUSES:?}", i.status)));
        }
        if !PRIORITIES.contains(&i.priority.as_str()) {
            errors.push(e(format!("priority {:?} not in {PRIORITIES:?}", i.priority)));
        }
        if !AREAS.contains(&i.area.as_str()) {
            errors.push(e(format!("area {:?} not in {AREAS:?}", i.area)));
        }
        if i.status == "in-progress" && i.owner.is_empty() {
            errors.push(e("in-progress items need an owner".into()));
        }
        for d in &i.depends_on {
            if !ids.contains_key(d.as_str()) {
                errors.push(e(format!("depends_on unknown item {d}")));
            }
        }
    }
    if errors.is_empty() { Ok(items) } else { Err(errors.join("\n")) }
}

fn print_items<'a>(items: impl IntoIterator<Item = &'a WorkItem>) {
    for i in items {
        let owner = if i.owner.is_empty() { String::new() } else { format!(" @{}", i.owner) };
        println!("{:<8} {:<11} {} {:<7} {}{owner}", i.id, i.status, i.priority, i.area, i.title);
    }
}

fn work_list(status: Option<&str>) -> Result {
    let mut items = work_check()?;
    items.sort_by(|a, b| (&a.priority, &a.id).cmp(&(&b.priority, &b.id)));
    print_items(items.iter().filter(|i| status.is_none_or(|s| i.status == s)));
    Ok(())
}

fn work_next() -> Result {
    let mut items = work_check()?;
    items.sort_by(|a, b| (&a.priority, &a.id).cmp(&(&b.priority, &b.id)));
    let done: BTreeSet<&str> =
        items.iter().filter(|i| i.status == "done").map(|i| i.id.as_str()).collect();
    print_items(
        items.iter().filter(|i| {
            i.status == "ready" && i.depends_on.iter().all(|d| done.contains(d.as_str()))
        }),
    );
    Ok(())
}

fn work_new(title: &str) -> Result {
    let items = load_items()?;
    let next = items
        .iter()
        .filter_map(|i| i.id.strip_prefix("FN-")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let id = format!("FN-{next:04}");
    let slug: String = title
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("-");
    let today = Command::new("date").arg("+%Y-%m-%d").output().map_err(|e| e.to_string())?;
    let today = String::from_utf8_lossy(&today.stdout).trim().to_string();
    let template =
        fs::read_to_string(root().join("work").join("TEMPLATE.md")).map_err(|e| e.to_string())?;
    let body = template
        .replace("FN-XXXX", &id)
        .replace("TITLE", &title.replace('"', "'"))
        .replace("1970-01-01", &today);
    let path = work_dir().join(format!("{id}-{slug}.md"));
    fs::write(&path, body).map_err(|e| e.to_string())?;
    println!("{}", path.strip_prefix(root()).unwrap().display());
    Ok(())
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
    work_check()?;
    println!("ci OK");
    Ok(())
}

//! `cargo xtask web [--fast] [--serve] [--port N] [LOG...]` (#595, #999): puts the web replayer's
//! page (`web/index.html`) into `target/replayer/`, and with `--serve` builds the game's wasm too
//! (it is the replayer's: `web-gui` below) and serves the folder on localhost. Without `--serve`
//! it needs no wasm-bindgen CLI, so Pages runs it before installing one.
//!
//! Each `LOG` (a `run.jsonl`, or a run folder holding one) is copied into `target/replayer/runs/` and
//! listed in `runs/index.json`, the same shape #596's published index takes, so the page's run
//! picker lists them. The command prints a `?run=` link for each.
//!
//! `cargo xtask web-gui` (#859) builds fn_gui, the egui game, into `target/replayer/play/`
//! through the wasm-bindgen CLI. The replayer is the same wasm on a canvas of its own (#999), so
//! `web-gui` also writes the build's hash into the replayer's page. `pages.yml` runs both, and the
//! folder is published at `/play/` next to the replayer (#866). The folder is `play/`, not `gui/`,
//! because that is the URL: `https://gizmo385.github.io/four-nations/play/`.
//!
//! The server is a few lines of std on purpose: it serves files from one folder to the local
//! machine, nothing else, and needs no tool beyond cargo.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use crate::{Result, cargo_env, root};

const HELP: &str = "\
usage: cargo xtask web [--fast] [--serve] [--port N] [LOG...]

Puts the web replayer's page (web/index.html) into target/replayer/. The page runs the game's own
wasm (play/), which `cargo xtask web-gui` builds; --serve builds it too.
  --serve    build the wasm (as web-gui), then serve target/replayer/ on http://127.0.0.1:PORT/
  --fast     with --serve: the quick unoptimised wasm (web-gui --fast)
  --port N   the port for --serve (default 8595)
  LOG        a run.jsonl (or a run folder holding one) to copy into target/replayer/runs/ and list
             in the page's run picker; a ?run= link is printed for each";

pub fn run(args: &[&str]) -> Result {
    let mut serve = false;
    let mut fast = false;
    let mut port: u16 = 8595;
    let mut logs: Vec<PathBuf> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--help" | "-h" => {
                println!("{HELP}");
                return Ok(());
            }
            "--serve" => serve = true,
            "--fast" => fast = true,
            "--port" => {
                i += 1;
                let v = args.get(i).ok_or("--port wants a number")?;
                port = v.parse().map_err(|_| format!("--port wants a number, got {v:?}"))?;
            }
            a if a.starts_with('-') => return Err(format!("unknown option {a}\n\n{HELP}")),
            a => logs.push(PathBuf::from(a)),
        }
        i += 1;
    }

    let out = build()?;
    let links = add_runs(&out, &logs)?;
    if serve {
        build_gui(fast)?;
    }
    println!("web replayer built into {}", out.display());
    if serve {
        let base = format!("http://127.0.0.1:{port}/");
        println!("serving it at {base}");
        for link in &links {
            println!("  {base}?run={link}");
        }
        serve_dir(&out, port)
    } else {
        Ok(())
    }
}

/// Copies the replayer's page into the output folder; that folder. The wasm is `web-gui`'s.
pub fn build() -> Result<PathBuf> {
    let root = root();
    let out = root.join("target/replayer");
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    for entry in fs::read_dir(root.join("web")).map_err(|e| format!("web/: {e}"))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        // The page's scripts for a developer (`check-replay.cjs`) are not published.
        if path.is_file() && path.extension().is_none_or(|e| e != "cjs") {
            copy(&path, &out.join(path.file_name().expect("a file has a name")))?;
        }
    }
    // The page's leftovers from before #999 (its own wasm, script and style), if an old build
    // left them here: they would be published with nothing using them.
    for stale in ["fn_web.wasm", "replay.js", "scan.js", "style.css"] {
        let _ = fs::remove_file(out.join(stale));
    }
    Ok(out)
}

const GUI_HELP: &str = "\
usage: cargo xtask web-gui [--fast] [--serve] [--port N]

Builds fn_gui (the egui game) for the browser into target/replayer/play/: the wasm built for
wasm32-unknown-unknown, bound with the wasm-bindgen CLI, next to web/play/index.html. Needs the CLI
at exactly the version Cargo.lock pins for the wasm-bindgen crate; `cargo install wasm-bindgen-cli
--version VERSION --locked` installs it. `cargo xtask web` does not build this; Pages runs both
(the game is published at /play/).
  --fast     the `ci` profile (unoptimised, quick to build, a large wasm) instead of `web`
  --serve    then serve target/replayer/ on http://127.0.0.1:PORT/ (the game is at /play/)
  --port N   the port for --serve (default 8595)";

/// `cargo xtask web-gui` (#859).
pub fn run_gui(args: &[&str]) -> Result {
    let mut fast = false;
    let mut serve = false;
    let mut port: u16 = 8595;
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--help" | "-h" => {
                println!("{GUI_HELP}");
                return Ok(());
            }
            "--fast" => fast = true,
            "--serve" => serve = true,
            "--port" => {
                i += 1;
                let v = args.get(i).ok_or("--port wants a number")?;
                port = v.parse().map_err(|_| format!("--port wants a number, got {v:?}"))?;
            }
            a => return Err(format!("unknown option {a}\n\n{GUI_HELP}")),
        }
        i += 1;
    }
    let out = build_gui(fast)?;
    println!("fn_gui for the browser built into {}", out.display());
    if serve {
        let base = format!("http://127.0.0.1:{port}/play/");
        println!("serving it at {base}");
        serve_dir(out.parent().expect("gui/ has a parent"), port)
    } else {
        Ok(())
    }
}

/// Builds fn_gui's wasm and binds it into `target/replayer/play/`; that folder.
pub fn build_gui(fast: bool) -> Result<PathBuf> {
    let root = root();
    // Checked first: a missing or mismatched CLI should not cost a wasm build to find out.
    let want = locked_wasm_bindgen_version(
        &fs::read_to_string(root.join("Cargo.lock")).map_err(|e| format!("Cargo.lock: {e}"))?,
    )?;
    check_wasm_bindgen_cli(&want)?;
    let profile = if fast { "ci" } else { "web" };
    let mut env = Vec::new();
    // As `cargo xtask check --fast` does: our own crates optimised a little, dependencies not.
    if fast && std::env::var_os("CARGO_PROFILE_CI_OPT_LEVEL").is_none() {
        env.push(("CARGO_PROFILE_CI_OPT_LEVEL", "1"));
    }
    cargo_env(
        &[
            "build",
            "--profile",
            profile,
            "--target",
            crate::WASM_TARGET,
            "-p",
            "fn_gui",
            "--bin",
            "fn_gui",
        ],
        &env,
    )?;
    let wasm = root.join(format!("target/{}/{profile}/fn_gui.wasm", crate::WASM_TARGET));
    let out = root.join("target/replayer/play");
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("$ wasm-bindgen --target web --no-typescript --out-dir {}", out.display());
    let status = std::process::Command::new("wasm-bindgen")
        .args(["--target", "web", "--no-typescript", "--out-dir"])
        .arg(&out)
        .arg(&wasm)
        .status()
        .map_err(|e| format!("wasm-bindgen: {e}"))?;
    if !status.success() {
        return Err("wasm-bindgen failed".into());
    }
    // The installable app's files (#974). The page and the service worker get the build's hash
    // for `__FN_BUILD__`: the page asks for the glue and the wasm by it, and the worker caches
    // under it, so a new build is fetched fresh and an old cache is dropped.
    let mut bytes = fs::read(out.join("fn_gui.js")).map_err(|e| format!("fn_gui.js: {e}"))?;
    bytes.extend(fs::read(out.join("fn_gui_bg.wasm")).map_err(|e| format!("fn_gui_bg.wasm: {e}"))?);
    let build = build_hash(&bytes);
    for name in ["index.html", "sw.js"] {
        let from = root.join("web/play").join(name);
        let text = fs::read_to_string(&from).map_err(|e| format!("{}: {e}", from.display()))?;
        fs::write(out.join(name), text.replace("__FN_BUILD__", &build))
            .map_err(|e| format!("{name}: {e}"))?;
    }
    // The replayer's page asks for the same glue and wasm (#999), by the same hash.
    let page = root.join("web/index.html");
    let text = fs::read_to_string(&page).map_err(|e| format!("{}: {e}", page.display()))?;
    let replayer = out.parent().expect("play/ has a parent").join("index.html");
    fs::write(&replayer, text.replace("__FN_BUILD__", &build))
        .map_err(|e| format!("{}: {e}", replayer.display()))?;
    copy(&root.join("web/play/manifest.webmanifest"), &out.join("manifest.webmanifest"))?;
    fs::create_dir_all(out.join("icons")).map_err(|e| format!("icons: {e}"))?;
    for name in PLAY_ICONS {
        copy(&root.join("web/play/icons").join(name), &out.join("icons").join(name))?;
    }
    for name in ["fn_gui.js", "fn_gui_bg.wasm", "index.html", "sw.js", "manifest.webmanifest"] {
        if !out.join(name).is_file() {
            return Err(format!("{} should hold {name} and doesn't", out.display()));
        }
    }
    Ok(out)
}

/// The icons the page and the manifest name (#974), in web/play/icons/.
const PLAY_ICONS: [&str; 4] =
    ["icon-180.png", "icon-192.png", "icon-512.png", "icon-maskable-512.png"];

/// A short name for a build: 16 hex digits of FNV-1a over `bytes`.
fn build_hash(bytes: &[u8]) -> String {
    let hash = bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
    format!("{hash:016x}")
}

/// The version of the `wasm-bindgen` crate in a `Cargo.lock`'s text.
fn locked_wasm_bindgen_version(lock: &str) -> Result<String> {
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line == r#"name = "wasm-bindgen""#
            && let Some(v) = lines.next().and_then(|l| l.strip_prefix(r#"version = ""#))
        {
            return Ok(v.trim_end_matches('"').to_owned());
        }
    }
    Err("Cargo.lock has no wasm-bindgen package".into())
}

/// The version in `wasm-bindgen --version`'s output (`wasm-bindgen 0.2.129`).
fn cli_version(output: &str) -> Option<&str> {
    output.trim().strip_prefix("wasm-bindgen ")?.split_whitespace().next()
}

/// Fails with the command that fixes it unless the `wasm-bindgen` CLI is on the path at exactly
/// `want`: the CLI and the crate must match, or the glue it writes doesn't fit the wasm.
fn check_wasm_bindgen_cli(want: &str) -> Result {
    let install = format!("cargo install wasm-bindgen-cli --version {want} --locked");
    let out =
        std::process::Command::new("wasm-bindgen").arg("--version").output().map_err(|e| {
            format!("the wasm-bindgen CLI isn't usable ({e}); install it with `{install}`")
        })?;
    let text = String::from_utf8_lossy(&out.stdout);
    match cli_version(&text) {
        Some(have) if have == want => Ok(()),
        Some(have) => Err(format!(
            "the wasm-bindgen CLI is {have} but Cargo.lock pins the crate at {want}; \
             install the matching one with `{install}`"
        )),
        None => Err(format!(
            "couldn't read `wasm-bindgen --version` (got {text:?}); install it with `{install}`"
        )),
    }
}

fn copy(from: &Path, to: &Path) -> Result {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))
}

/// Copies `logs` into `out/runs/` and writes `out/runs/index.json` listing them; each one's
/// `?run=` value.
fn add_runs(out: &Path, logs: &[PathBuf]) -> Result<Vec<String>> {
    // An earlier run's logs never linger into this one's page (or the deployed site).
    let runs = out.join("runs");
    if runs.exists() {
        fs::remove_dir_all(&runs).map_err(|e| format!("{}: {e}", runs.display()))?;
    }
    if logs.is_empty() {
        return Ok(Vec::new());
    }
    fs::create_dir_all(&runs).map_err(|e| format!("{}: {e}", runs.display()))?;
    let mut entries = Vec::new();
    let mut links = Vec::new();
    for log in logs {
        let file = if log.is_dir() { log.join("run.jsonl") } else { log.clone() };
        let name = run_name(&file);
        let path = format!("runs/{name}.jsonl");
        copy(&file, &out.join(&path))?;
        let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        // Read with plain JSON rather than fn_tui's own parser: xtask doesn't depend on the
        // game crates, so it builds before them and `cargo xtask ci` starts quickly.
        let header: serde_json::Value = text
            .lines()
            .next()
            .and_then(|l| serde_json::from_str(l).ok())
            .ok_or_else(|| format!("{}: no header line", file.display()))?;
        let turns = text.lines().filter(|l| l.starts_with(r#"{"type":"turn""#)).count();
        entries.push(serde_json::json!({
            "path": path,
            "name": name,
            "nation": header["nation"],
            "seed": header["seed"],
            "build": header["build"],
            "turns": turns,
        }));
        links.push(path);
    }
    let index = serde_json::json!({ "runs": entries });
    let index_path = runs.join("index.json");
    fs::write(&index_path, serde_json::to_string_pretty(&index).expect("json serializes"))
        .map_err(|e| format!("{}: {e}", index_path.display()))?;
    Ok(links)
}

/// A file name for a log: its run folder's name (with a campaign folder's in front for an
/// attempt), or the file's own stem.
fn run_name(file: &Path) -> String {
    let parent = file.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy());
    let grand = file
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy());
    let raw = match (file.file_name().map(|n| n == "run.jsonl"), parent, grand) {
        (Some(true), Some(p), Some(g)) if p.starts_with("attempt-") => format!("{g}-{p}"),
        (Some(true), Some(p), _) => p.into_owned(),
        _ => file.file_stem().map_or("run".into(), |s| s.to_string_lossy().into_owned()),
    };
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect()
}

/// Serves `dir`'s files to localhost, one request at a time, until interrupted.
fn serve_dir(dir: &Path, port: u16) -> Result {
    let listener =
        TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("port {port}: {e}"))?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(e) = respond(dir, stream) {
                    eprintln!("web: {e}");
                }
            }
            Err(e) => eprintln!("web: {e}"),
        }
    }
    Ok(())
}

fn respond(dir: &Path, mut stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    // Drain the headers; nothing in them matters here.
    let mut line = String::new();
    while reader.read_line(&mut line)? > 2 {
        line.clear();
    }
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let path = path.split(['?', '#']).next().unwrap_or("/");
    let rel = path.trim_start_matches('/');
    // A folder's address (`/`, `/play/`) is its index page.
    let rel = if rel.is_empty() || rel.ends_with('/') {
        format!("{rel}index.html")
    } else {
        rel.to_owned()
    };
    let rel = rel.as_str();
    let file = dir.join(rel);
    // Only files under `dir`: no `..` and no absolute paths.
    let safe = !rel.split('/').any(|part| part == ".." || part.is_empty());
    match (safe, fs::read(&file)) {
        (true, Ok(body)) => {
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
                 Cache-Control: no-store\r\nConnection: close\r\n\r\n",
                content_type(&file),
                body.len()
            );
            stream.write_all(head.as_bytes())?;
            stream.write_all(&body)
        }
        _ => stream.write_all(
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot found",
        ),
    }
}

fn content_type(file: &Path) -> &'static str {
    match file.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        Some("jsonl") => "application/x-ndjson",
        Some("gz") => "application/gzip",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_hash_is_sixteen_hex_digits_and_follows_the_bytes() {
        let a = build_hash(b"fn_gui");
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(a, build_hash(b"fn_gui"));
        assert_ne!(a, build_hash(b"fn_gui2"));
    }

    /// The installable app's files (#974) are all there, and the page and the worker carry the
    /// placeholder the build fills in.
    #[test]
    fn the_play_page_names_its_icons_and_carries_the_build_placeholder() {
        let play = root().join("web/play");
        for name in PLAY_ICONS {
            assert!(play.join("icons").join(name).is_file(), "web/play/icons/{name}");
        }
        let manifest = fs::read_to_string(play.join("manifest.webmanifest")).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(manifest["orientation"], "landscape");
        for icon in manifest["icons"].as_array().unwrap() {
            let src = icon["src"].as_str().unwrap();
            assert!(play.join(src).is_file(), "the manifest's {src}");
        }
        for name in ["index.html", "sw.js"] {
            let text = fs::read_to_string(play.join(name)).unwrap();
            assert!(text.contains("__FN_BUILD__"), "{name} has the build placeholder");
        }
    }

    #[test]
    fn the_locked_wasm_bindgen_version_is_read_from_cargo_lock() {
        let lock = "[[package]]\nname = \"wasm-bindgen-futures\"\nversion = \"0.4.1\"\n\n\
                    [[package]]\nname = \"wasm-bindgen\"\nversion = \"0.2.129\"\n";
        assert_eq!(locked_wasm_bindgen_version(lock).unwrap(), "0.2.129");
        assert!(locked_wasm_bindgen_version("[[package]]\nname = \"serde\"\n").is_err());
        // The real lock file has one.
        let real = fs::read_to_string(root().join("Cargo.lock")).unwrap();
        assert!(locked_wasm_bindgen_version(&real).unwrap().starts_with("0.2."));
    }

    #[test]
    fn the_cli_version_is_read_from_its_banner() {
        assert_eq!(cli_version("wasm-bindgen 0.2.129\n"), Some("0.2.129"));
        assert_eq!(cli_version("wasm-bindgen 0.2.129 (abc123)"), Some("0.2.129"));
        assert_eq!(cli_version("something else"), None);
    }

    #[test]
    fn an_attempts_log_is_named_after_its_campaign_and_attempt() {
        let p = Path::new("/runs/20261109-campaign-water-3h/attempt-4/run.jsonl");
        assert_eq!(run_name(p), "20261109-campaign-water-3h-attempt-4");
        assert_eq!(run_name(Path::new("/runs/20261109-solo/run.jsonl")), "20261109-solo");
        assert_eq!(run_name(Path::new("/tmp/my run.jsonl")), "my-run");
    }
}

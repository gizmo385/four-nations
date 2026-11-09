//! `cargo xtask web [--serve] [--port N] [LOG...]` (#595): builds the web replayer into
//! `target/replayer/` -- `web/`'s page plus `fn_web.wasm` built for `wasm32-unknown-unknown` with the
//! `web` profile -- and with `--serve`, serves it on localhost.
//!
//! Each `LOG` (a `run.jsonl`, or a run folder holding one) is copied into `target/replayer/runs/` and
//! listed in `runs/index.json`, the same shape #596's published index takes, so the page's run
//! picker lists them. The command prints a `?run=` link for each.
//!
//! The server is a few lines of std on purpose: it serves files from one folder to the local
//! machine, nothing else, and needs no tool beyond cargo.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use crate::{Result, cargo_env, root};

const HELP: &str = "\
usage: cargo xtask web [--serve] [--port N] [LOG...]

Builds the web replayer (the page in web/ and fn_web.wasm) into target/replayer/.
  --serve    then serve target/replayer/ on http://127.0.0.1:PORT/ until interrupted
  --port N   the port for --serve (default 8595)
  LOG        a run.jsonl (or a run folder holding one) to copy into target/replayer/runs/ and list
             in the page's run picker; a ?run= link is printed for each";

pub fn run(args: &[&str]) -> Result {
    let mut serve = false;
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

/// Builds the wasm and copies the page next to it; the output folder.
pub fn build() -> Result<PathBuf> {
    let root = root();
    cargo_env(
        &["build", "--profile", "web", "--target", crate::WASM_TARGET, "-p", "fn_web", "--lib"],
        &[],
    )?;
    let out = root.join("target/replayer");
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    for entry in fs::read_dir(root.join("web")).map_err(|e| format!("web/: {e}"))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_file() {
            copy(&path, &out.join(path.file_name().expect("a file has a name")))?;
        }
    }
    let wasm = root.join(format!("target/{}/web/fn_web.wasm", crate::WASM_TARGET));
    copy(&wasm, &out.join("fn_web.wasm"))?;
    Ok(out)
}

fn copy(from: &Path, to: &Path) -> Result {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))
}

/// Copies `logs` into `out/runs/` and writes `out/runs/index.json` listing them; each one's
/// `?run=` value.
fn add_runs(out: &Path, logs: &[PathBuf]) -> Result<Vec<String>> {
    if logs.is_empty() {
        return Ok(Vec::new());
    }
    let runs = out.join("runs");
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
    let rel = if rel.is_empty() { "index.html" } else { rel };
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
    fn an_attempts_log_is_named_after_its_campaign_and_attempt() {
        let p = Path::new("/runs/20261109-campaign-water-3h/attempt-4/run.jsonl");
        assert_eq!(run_name(p), "20261109-campaign-water-3h-attempt-4");
        assert_eq!(run_name(Path::new("/runs/20261109-solo/run.jsonl")), "20261109-solo");
        assert_eq!(run_name(Path::new("/tmp/my run.jsonl")), "my-run");
    }
}

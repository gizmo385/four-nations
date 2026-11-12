//! `cargo xtask gui-shot` (#875): one frame of the egui window as a PNG, through `egui_kittest`'s
//! wgpu path. The terminal has `fn_tui --frame` for an agent or reviewer to look at; this is the
//! window's counterpart. Nothing compares it with anything, and it is not in the gate: it needs
//! the `shots` feature of `fn_gui` and a wgpu adapter (a GPU, or Mesa's lavapipe).
//!
//! The work is `crates/gui/examples/shot.rs`; this builds it at the `ci` profile and runs it.

use std::path::{Path, PathBuf};

use crate::{Result, cargo};

const HELP: &str = "\
usage: cargo xtask gui-shot SEED NATION [--keys SCRIPT] [--size WxH] --out FILE.png

Renders one frame of the egui window (fn_gui) on a new SEED/NATION game and writes it as a PNG.
  --keys SCRIPT  keys played first, as `fn_tui --frame --keys` takes them
  --size WxH     the window's size in points (default 1280x800)
  --out FILE     the PNG to write (required)

Renders through wgpu, so it needs a GPU or a software Vulkan driver (Mesa's lavapipe: `apt install
mesa-vulkan-drivers`). Without an adapter it fails saying so. Nothing compares the picture with
anything: it is for looking at, and for attaching to a PR.";

/// `cargo xtask gui-shot ARGS`: `args` go to the example as they are, except that a relative
/// `--out` is taken from the directory the command was run in (cargo runs in the repo root).
pub fn run(args: &[&str]) -> Result {
    if args.iter().any(|a| matches!(*a, "--help" | "-h")) {
        println!("{HELP}");
        return Ok(());
    }
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let owned = absolute_out(args, &cwd);
    let mut cmd: Vec<&str> = vec![
        "run",
        "--profile",
        "ci",
        "-q",
        "-p",
        "fn_gui",
        "--features",
        "shots",
        "--example",
        "shot",
        "--",
    ];
    cmd.extend(owned.iter().map(String::as_str));
    cargo(&cmd)
}

/// `args` with the value after `--out` made absolute against `cwd`.
fn absolute_out(args: &[&str], cwd: &Path) -> Vec<String> {
    let mut out: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    for i in 1..out.len() {
        if out[i - 1] == "--out" {
            let p = PathBuf::from(&out[i]);
            if p.is_relative() {
                out[i] = cwd.join(p).to_string_lossy().into_owned();
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_out_is_taken_from_the_callers_directory() {
        let got = absolute_out(&["7", "earth", "--out", "a/b.png"], Path::new("/work"));
        assert_eq!(got, ["7", "earth", "--out", "/work/a/b.png"]);
        let got = absolute_out(&["--out", "/tmp/x.png"], Path::new("/work"));
        assert_eq!(got, ["--out", "/tmp/x.png"]);
    }
}

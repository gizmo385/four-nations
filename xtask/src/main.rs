//! Repo maintenance commands. Run from anywhere in the repo:
//!
//!     cargo xtask ci [--fast]   fmt + clippy + tests for the whole workspace;
//!                               --fast uses the `ci` profile, as CI does

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

type Result<T = ()> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let fast = args.first() == Some(&"ci") && args.contains(&"--fast");
    let args: Vec<&str> = args.into_iter().filter(|a| !(fast && *a == "--fast")).collect();
    let result = match args.as_slice() {
        ["ci"] => ci(fast),
        // Kept as an alias: before #281 this ran everything except the Bevy client.
        ["ci", "sim"] => ci(fast),
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
// CI
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

fn ci(fast: bool) -> Result {
    // On CI (GitHub sets $CI) or with --fast, build with the `ci` profile: unoptimized
    // dependencies and no debug info compile several times faster and use far less memory.
    // Agents use --fast so parallel worktree builds fit in RAM.
    let profile: &[&str] =
        if fast || std::env::var_os("CI").is_some() { &["--profile", "ci"] } else { &[] };
    // With --fast, optimize our own crates: the long simulation tests run about 6x faster for
    // ~30s more build time. An explicit CARGO_PROFILE_CI_OPT_LEVEL in the environment wins.
    let env: &[(&str, &str)] = if fast && std::env::var_os("CARGO_PROFILE_CI_OPT_LEVEL").is_none() {
        &[("CARGO_PROFILE_CI_OPT_LEVEL", "1")]
    } else {
        &[]
    };

    cargo(&["fmt", "--all", "--check"])?;
    let clippy =
        [&["clippy", "--workspace", "--all-targets"][..], profile, &["--", "-D", "warnings"]];
    cargo_env(&clippy.concat(), env)?;
    cargo_env(&[&["test", "--workspace"][..], profile].concat(), env)?;
    println!("ci OK");
    Ok(())
}

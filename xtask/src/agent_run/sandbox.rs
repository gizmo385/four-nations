//! The player's sandbox (#567): what the `claude` child that plays the game is started with,
//! and how its transcript is kept with the run.
//!
//! Decision #476 settled that a scripted playtest agent reads only what the game's tools show,
//! plus its own notes and scripts carried between runs (docs/design/agent-play.md, "What the
//! agent may read"). Before this, the child ran in whatever directory `agent-run` was started
//! from -- the repo -- as a full Claude Code session. This module narrows that:
//!
//! - **An empty working directory** under the system temp dir, one per run, so there is no
//!   `CLAUDE.md`, `.claude/` or memory directory for the session to find, and no repo to read.
//! - **`--setting-sources local`**: no user or project settings (hooks, permissions, plugins,
//!   output styles, user `CLAUDE.md`). `local` is the one source left because an empty value
//!   isn't documented to mean "none"; the empty directory has no local settings to load.
//! - **`--tools ""`**: no built-in tools at all. The game's own MCP tools are not built in, so
//!   `--strict-mcp-config --mcp-config` still supplies exactly those.
//! - **`--disable-slash-commands`**: no skills or commands.
//! - **`--system-prompt`**: replaces Claude Code's own instructions with a few lines.
//!
//! What this does not do is in docs/design/agent-play.md ("What the sandbox does not
//! guarantee"): it is flags on a child process, not a container. The child still runs as the
//! owner with the owner's credentials, so the sandbox is what the child is *given*, not what
//! the OS enforces, and organisation-managed settings and the account-level context still reach
//! it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{say, system_entropy};
use crate::Result;

/// The whole of the player's system prompt. The game's briefing and key help come from its
/// tools; the task comes from the user message.
const SYSTEM_PROMPT: &str = "You play a game through the tools the user's game server provides. \
These tools are your only way to see or change anything. You have no files, shell or web access. \
Do what the user asks with them, then end with a short plain-text report.";

/// The flags that strip the child down to the game's tools (see the module docs). These are
/// added to the MCP flags in `base_claude_args`.
pub(super) fn flags() -> Vec<String> {
    [
        "--setting-sources",
        "local",
        "--tools",
        "",
        "--disable-slash-commands",
        "--system-prompt",
        SYSTEM_PROMPT,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// `p` made absolute without touching the filesystem (it need not exist yet). The child runs
/// somewhere else, so every path it is handed -- the MCP config, and the `fn_agent` binary and
/// log inside it -- must not depend on the working directory.
pub(super) fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

/// The empty working directory for a run's `claude` calls: `$TMPDIR/fn-agent-sandbox-<run
/// folder name>`, created if missing. One per run folder, so a resumed campaign's attempts and
/// a run's chunks each find it as they left it (empty).
pub(super) fn dir_for(run_dir: &Path) -> Result<PathBuf> {
    let name = absolute(run_dir)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "run".to_string());
    let dir = absolute(&std::env::temp_dir()).join(format!("fn-agent-sandbox-{name}"));
    fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    Ok(dir)
}

/// A random version-4 UUID, to give `claude --session-id` so the transcript can be found again
/// without asking the child anything.
pub(super) fn new_session_id() -> String {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&system_entropy().to_be_bytes());
    bytes[8..].copy_from_slice(&system_entropy().to_be_bytes());
    bytes[6] = bytes[6] & 0x0f | 0x40;
    bytes[8] = bytes[8] & 0x3f | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

/// A `claude` command ready for its flags: the given binary, run from the run's empty sandbox
/// directory, with a session id of our choosing.
pub(super) fn claude_command(bin: &str, run_dir: &Path, session_id: &str) -> Result<Command> {
    let mut cmd = Command::new(bin);
    cmd.current_dir(dir_for(run_dir)?).args(["--session-id", session_id]);
    Ok(cmd)
}

/// Where Claude Code keeps its session transcripts: `$CLAUDE_CONFIG_DIR/projects`, else
/// `~/.claude/projects`.
fn projects_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d).join("projects"));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude/projects"))
}

/// Copies the transcript of session `session_id` into `run_dir/transcripts/<label>.jsonl`, so
/// the model's reasoning stays with the run. Claude Code files it under a per-working-directory
/// folder, so every project folder is searched for the session's own file name. A transcript
/// that can't be found or copied is a warning, never a failed run.
pub(super) fn keep_transcript(run_dir: &Path, session_id: &str, label: &str) {
    match find_transcript(session_id) {
        None => say(&format!(
            "warning: no transcript found for {label} (session {session_id}); \
             the run goes on without it"
        )),
        Some(found) => {
            let out = run_dir.join("transcripts");
            let copied = fs::create_dir_all(&out)
                .and_then(|()| fs::copy(&found, out.join(format!("{label}.jsonl"))));
            if let Err(e) = copied {
                say(&format!(
                    "warning: couldn't copy {label}'s transcript from {}: {e}",
                    found.display()
                ));
            }
        }
    }
}

fn find_transcript(session_id: &str) -> Option<PathBuf> {
    let file = format!("{session_id}.jsonl");
    fs::read_dir(projects_dir()?)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path().join(&file))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ids_are_distinct_version_4_uuids() {
        let a = new_session_id();
        let b = new_session_id();
        assert_ne!(a, b);
        for id in [&a, &b] {
            let parts: Vec<&str> = id.split('-').collect();
            let lens: Vec<usize> = parts.iter().map(|p| p.len()).collect();
            assert_eq!(lens, [8, 4, 4, 4, 12], "{id}");
            assert!(id.chars().all(|c| c == '-' || c.is_ascii_hexdigit()), "{id}");
            assert!(parts[2].starts_with('4'), "version nibble: {id}");
            assert!("89ab".contains(&parts[3][..1]), "variant nibble: {id}");
        }
    }

    #[test]
    fn the_sandbox_dir_is_absolute_per_run_and_created() {
        let a = dir_for(Path::new("runs/20260101-000000-earth-1-normal")).unwrap();
        let b = dir_for(Path::new("runs/20260101-000000-fire-2-normal")).unwrap();
        assert!(a.is_absolute() && a.is_dir());
        assert_ne!(a, b);
        assert!(a.ends_with("fn-agent-sandbox-20260101-000000-earth-1-normal"));
        assert_eq!(fs::read_dir(&a).unwrap().count(), 0, "the sandbox starts empty");
        let _ = fs::remove_dir(&a);
        let _ = fs::remove_dir(&b);
    }

    #[test]
    fn the_flags_drop_every_built_in_tool_and_every_settings_source_but_local() {
        let f = flags();
        let after = |flag: &str| {
            let i = f.iter().position(|a| a == flag).unwrap_or_else(|| panic!("no {flag}"));
            f[i + 1].clone()
        };
        assert_eq!(after("--tools"), "", "an empty list disables all built-in tools");
        assert_eq!(after("--setting-sources"), "local");
        assert!(f.contains(&"--disable-slash-commands".to_string()));
        assert!(!after("--system-prompt").is_empty());
    }
}

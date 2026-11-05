//! The agent-surface parity lint (#320, part of `cargo xtask ci`): a plain substring scan over
//! `crates/tui/src/surface.rs` and every file under `crates/agent/src/`, standing in for what
//! Rust visibility can't enforce on its own (`Shell` and `Session` are public across the
//! workspace) -- docs/design/agent-play.md, "Enforcing and testing parity", rule 1 ("one way in
//! for actions").
//!
//! It isn't a real Rust parser. Comments and string/char literals are blanked out first (kept
//! the same length, with newlines kept too, so line numbers and brace matching both still work)
//! -- that's what keeps a doc comment mentioning `App::run_ticks`, or a format string's `{}`,
//! from tripping the scan or its brace matching. What's left is searched for the patterns below
//! outside a short allow-list of spans: the `end_turn` function and a `referee` module (#318,
//! not written yet) in `surface.rs`, and `#[cfg(test)]` code everywhere -- the parity tests
//! themselves (`Host::act` round trips, #316) read `World` directly to compare command logs and
//! frames, which is the point of them, not a leak.
//!
//! `crates/agent/src/`'s MCP tool handlers get a narrower rule: the word `referee` shouldn't
//! appear in their code at all, since #317/#318 are both deferred and no tool should read the
//! privileged, goal-judging `referee` once it exists.

use std::fs;
use std::path::{Path, PathBuf};

/// Patterns banned outside the allow-listed spans: `.apply(` and `Command` would change state
/// without going through `Shell::key`; `run_ticks`/`tick_n` would run time the host doesn't own;
/// `.world` would read `World` directly instead of an `fn_ui` view model.
const FORBIDDEN: &[&str] = &[".apply(", "run_ticks", "tick_n", ".world", "Command"];

/// Runs every parity-lint check, relative to the workspace `root`; one combined error listing
/// every violation, or `Ok(())`.
pub fn check(root: &Path) -> Result<(), String> {
    let mut problems = Vec::new();

    let surface_path = root.join("crates/tui/src/surface.rs");
    let surface_src = fs::read_to_string(&surface_path)
        .map_err(|e| format!("reading {}: {e}", surface_path.display()))?;
    for (line, pattern) in check_surface(&surface_src) {
        problems.push(format!(
            "{}:{line}: `{pattern}` outside `end_turn`, `referee` and tests (#320)",
            surface_path.display()
        ));
    }

    let agent_dir = root.join("crates/agent/src");
    for path in walk_rs_files(&agent_dir)? {
        let src =
            fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        for line in check_agent_referee(&src) {
            problems.push(format!(
                "{}:{line}: `referee` in an MCP tool handler (#320)",
                path.display()
            ));
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("agent-surface parity lint failed:\n{}", problems.join("\n")))
    }
}

fn walk_rs_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("reading {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            out.extend(walk_rs_files(&path)?);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(out)
}

/// Every forbidden pattern's line number and text in `src`, outside `fn end_turn`, `mod
/// referee` and `#[cfg(test)]` code.
fn check_surface(src: &str) -> Vec<(usize, &'static str)> {
    let mut code = blank_non_code(src);
    mask_blocks(&mut code, "mod tests");
    mask_blocks(&mut code, "mod referee");
    mask_blocks(&mut code, "fn end_turn");
    find_forbidden(&code)
}

/// Every line on which `referee` appears in real code (not a comment or string) in `src`,
/// outside `#[cfg(test)]`.
fn check_agent_referee(src: &str) -> Vec<usize> {
    let mut code = blank_non_code(src);
    mask_blocks(&mut code, "mod tests");
    find_all(&code, "referee").into_iter().map(|at| line_of(&code, at)).collect()
}

// -------------------------------------------------------------------------------------------
// A source file with its comments and string/char literals blanked to spaces (newlines kept)
// -------------------------------------------------------------------------------------------

/// Blanks every `//`, `/* */`, `"..."` and `'x'` span in `src` to spaces, keeping every
/// newline, so what's left is plain code at the same line numbers -- good enough for this
/// lint, which only needs braces and the patterns above to survive, not a real parse. A `'a` or
/// `'static` lifetime is left alone: unlike a char literal, it never closes with another `'`
/// within a few characters on the same line.
fn blank_non_code(src: &str) -> Vec<char> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    out.push(' ');
                    i += 1;
                }
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                out.push(' ');
                out.push(' ');
                i += 2;
                while i < chars.len() {
                    let closing = chars[i] == '*' && chars.get(i + 1) == Some(&'/');
                    out.push(if chars[i] == '\n' { '\n' } else { ' ' });
                    i += 1;
                    if closing {
                        out.push(' ');
                        i += 1;
                        break;
                    }
                }
            }
            '"' => {
                out.push(' ');
                i += 1;
                while i < chars.len() {
                    let c = chars[i];
                    if c == '\\' {
                        out.push(' ');
                        i += 1;
                        if i < chars.len() {
                            out.push(if chars[i] == '\n' { '\n' } else { ' ' });
                            i += 1;
                        }
                        continue;
                    }
                    out.push(if c == '\n' { '\n' } else { ' ' });
                    i += 1;
                    if c == '"' {
                        break;
                    }
                }
            }
            '\'' => {
                if let Some(end) = closing_quote(&chars, i) {
                    out.extend(std::iter::repeat_n(' ', end + 1 - i));
                    i = end + 1;
                } else {
                    out.push('\'');
                    i += 1;
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// If `chars[open]` (a `'`) starts a char literal, the index of its closing `'` (within five
/// characters, none of them a newline); `None` for a lifetime, which never closes.
fn closing_quote(chars: &[char], open: usize) -> Option<usize> {
    let mut j = open + 1;
    while j < chars.len() && j < open + 6 && chars[j] != '\n' {
        if chars[j] == '\\' {
            j += 2;
            continue;
        }
        if chars[j] == '\'' {
            return Some(j);
        }
        j += 1;
    }
    None
}

// -------------------------------------------------------------------------------------------
// Blanking an allow-listed `needle ... { ... }` span
// -------------------------------------------------------------------------------------------

/// Blanks every `needle ... { ... }` span in `code` to spaces (keeping newlines): every
/// occurrence of `needle`, the next `{` after it, and everything up to that brace's match.
/// Covers `mod tests { ... }`, `mod referee { ... }` and `fn end_turn(...) { ... }` alike.
fn mask_blocks(code: &mut [char], needle: &str) {
    let needle: Vec<char> = needle.chars().collect();
    let mut from = 0;
    while let Some(start) = find_chars(code, &needle, from) {
        from = start + needle.len();
        let Some(open) = find_char(code, '{', start) else { continue };
        let Some(end) = match_brace(code, open) else { continue };
        for c in &mut code[start..=end] {
            if *c != '\n' {
                *c = ' ';
            }
        }
        from = end + 1;
    }
}

/// The index of the `{` at `open` (assumed) matching `}`, by depth.
fn match_brace(code: &[char], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, &c) in code.iter().enumerate().skip(open) {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_char(haystack: &[char], target: char, from: usize) -> Option<usize> {
    haystack.iter().skip(from).position(|&c| c == target).map(|p| p + from)
}

fn find_chars(haystack: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() || from + needle.len() > haystack.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()] == *needle)
}

fn find_all(haystack: &[char], needle: &str) -> Vec<usize> {
    let needle: Vec<char> = needle.chars().collect();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = find_chars(haystack, &needle, from) {
        out.push(at);
        from = at + 1;
    }
    out
}

fn find_forbidden(code: &[char]) -> Vec<(usize, &'static str)> {
    let mut out: Vec<(usize, &'static str)> = Vec::new();
    for pattern in FORBIDDEN {
        for at in find_all(code, pattern) {
            out.push((line_of(code, at), pattern));
        }
    }
    out.sort();
    out
}

/// 1-based line number of `at` in `code` (which still has every original newline).
fn line_of(code: &[char], at: usize) -> usize {
    1 + code[..at].iter().filter(|&&c| c == '\n').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_surface_and_agent_sources_pass() {
        check(&crate::root()).unwrap();
    }

    #[test]
    fn a_bare_dot_world_read_outside_the_allow_list_fails() {
        let src = "fn look(&self) -> u64 {\n    self.app().session.world.tick\n}\n";
        let hits = check_surface(src);
        assert_eq!(hits, vec![(2, ".world")]);
    }

    #[test]
    fn apply_and_command_fail_too() {
        let src = "fn sneaky(&mut self) {\n    self.shell.apply(Command::Noop);\n}\n";
        let hits = check_surface(src);
        assert!(hits.contains(&(2, ".apply(")));
        assert!(hits.contains(&(2, "Command")));
    }

    #[test]
    fn end_turn_may_run_ticks_and_read_world() {
        let src = "fn end_turn(&mut self, turns: u32) {\n    self.app_mut().run_ticks(turns);\n    let t = self.app().session.world.tick;\n}\n";
        assert_eq!(check_surface(src), Vec::new());
    }

    #[test]
    fn a_referee_module_may_read_world() {
        let src = "mod referee {\n    fn judge(w: &World) -> bool {\n        w.world.tick > 0\n    }\n}\n";
        assert_eq!(check_surface(src), Vec::new());
    }

    #[test]
    fn cfg_test_code_may_read_world_and_command_log() {
        let src = "#[cfg(test)]\nmod tests {\n    fn it_works() {\n        let _ = h.app().session.world.command_log();\n    }\n}\n";
        assert_eq!(check_surface(src), Vec::new());
    }

    #[test]
    fn a_violation_right_after_end_turn_still_fails() {
        // A seeded violation in a fixture, right next to the allow-listed function: the lint
        // has to stop blanking exactly where `end_turn`'s own closing brace is.
        let src = "fn end_turn(&mut self) {\n    self.app_mut().run_ticks(1);\n}\n\nfn leaky(&self) -> u64 {\n    self.app().session.world.tick\n}\n";
        assert_eq!(check_surface(src), vec![(6, ".world")]);
    }

    #[test]
    fn comments_and_strings_never_trigger_a_match() {
        let src = "/// See `App::run_ticks` and `.world` (docs only).\nfn fine(&self) -> &str {\n    \"not .world, not Command, not run_ticks\"\n}\n";
        assert_eq!(check_surface(src), Vec::new());
    }

    #[test]
    fn a_lifetime_quote_does_not_confuse_the_scanner() {
        let src = "fn name(self) -> &'static str {\n    \"Harvest\"\n}\n\nfn leaky(&self) -> u64 {\n    self.app().session.world.tick\n}\n";
        assert_eq!(check_surface(src), vec![(6, ".world")]);
    }

    #[test]
    fn referee_in_a_tool_handler_fails_but_a_doc_comment_does_not() {
        let clean = "//! The referee stays out of every tool handler (#320).\nfn observe() {}\n";
        assert_eq!(check_agent_referee(clean), Vec::<usize>::new());

        let dirty = "fn observe() -> bool {\n    referee::goal_met()\n}\n";
        assert_eq!(check_agent_referee(dirty), vec![2]);
    }
}

//! How an agent's scripts should look (#557): the layout rules every script-mode prompt and the
//! `save_script` tool description share, the example governor `sdk_reference` shows, and the
//! non-blocking long-line hint `save_script` returns. The studio's code pane holds about 60
//! characters, so a 167-character `act([#{...}])` is cut off with `…`; nothing here rejects or
//! rewrites the agent's code.

/// The layout rules, one short passage. `xtask`'s script-mode prompts (`default_script_prompt`,
/// `scripts_handoff_prompt`) `include_str!` this very file, so the prompts and the tool
/// description can't drift apart.
pub const LAYOUT_RULES: &str = include_str!("layout_rules.txt");

/// The example governor `sdk_reference` shows (`plan.rs`'s governor frame): it follows
/// [`LAYOUT_RULES`] and runs as written once a plan has `research` and `targets.beds_ahead`.
pub const EXAMPLE_GOVERNOR: &str = include_str!("example_governor.rhai");

/// A line longer than this many characters earns `save_script` a hint (the rules ask for under
/// 80; the hint waits until 100 so a few near-misses don't nag).
pub const LONG_LINE_HINT_THRESHOLD: usize = 100;

/// `Some(hint)` when `code` has lines over [`LONG_LINE_HINT_THRESHOLD`] characters: the count and
/// the longest length. `None` otherwise. Advice only: `save_script` always saves.
pub fn long_line_hint(code: &str) -> Option<String> {
    let lengths: Vec<usize> = code.lines().map(|l| l.chars().count()).collect();
    let over = lengths.iter().filter(|&&n| n > LONG_LINE_HINT_THRESHOLD).count();
    if over == 0 {
        return None;
    }
    let longest = lengths.iter().max().copied().unwrap_or(0);
    Some(format!(
        "Saved. Hint: {over} line{} over {LONG_LINE_HINT_THRESHOLD} characters (longest \
         {longest}); the code pane cuts them off. Break them up when you next revise.",
        if over == 1 { "" } else { "s" }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_lines_earn_no_hint() {
        assert_eq!(long_line_hint("let a = 1;\nlet b = 2;\n"), None);
        assert_eq!(long_line_hint(""), None);
        // Exactly at the threshold is fine.
        assert_eq!(long_line_hint(&"x".repeat(LONG_LINE_HINT_THRESHOLD)), None);
    }

    #[test]
    fn long_lines_are_counted_with_the_longest() {
        let code = format!("ok;\n{}\n{}\n", "a".repeat(101), "b".repeat(150));
        let hint = long_line_hint(&code).unwrap();
        assert!(hint.contains("2 lines over 100 characters"), "{hint}");
        assert!(hint.contains("longest 150"), "{hint}");
        let one = long_line_hint(&"a".repeat(101)).unwrap();
        assert!(one.contains("1 line over"), "{one}");
    }

    #[test]
    fn the_example_governor_follows_its_own_rules() {
        for line in EXAMPLE_GOVERNOR.lines() {
            assert!(line.chars().count() < 80, "over 79 characters: {line}");
            assert!(!line.contains('\t'), "tab in: {line}");
        }
        crate::script::prelude::compile_check(EXAMPLE_GOVERNOR).expect("the example parses");
    }
}

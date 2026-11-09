//! Which code this binary was built from (#566). `build.rs` bakes the id in as `FN_BUILD_ID`:
//! `COMMIT` for a clean tree, `COMMIT+HEX` for a dirty one (see there), `unknown` without git.
//!
//! A run log's `data_digest` covers the game's data only. The same keys replayed by a different
//! sim can reach a different world and so a different score, so a log and a `score` line say
//! which build wrote them, and a reader that scores a log compares that with its own.

/// The build this binary is.
pub const CURRENT: &str = env!("FN_BUILD_ID");

/// Whether a build recorded in a log, a `score` line or a ledger is not this binary's. A record
/// from before builds were recorded (`None`) counts as another build: nothing says it wasn't.
pub fn is_other(recorded: Option<&str>) -> bool {
    recorded != Some(CURRENT)
}

/// The words for a build in a message: `build abc1234`, or `an unrecorded build`.
pub fn describe(recorded: Option<&str>) -> String {
    recorded.map_or("an unrecorded build".to_owned(), |b| format!("build {b}"))
}

/// The warning for scoring something recorded by `recorded` on this binary, or `None` when it is
/// the same build. `what` names the thing, e.g. `the log`.
pub fn mismatch_warning(what: &str, recorded: Option<&str>) -> Option<String> {
    is_other(recorded).then(|| {
        format!(
            "{what} was made by {}, and this binary is build {CURRENT}: the score may not \
             reproduce what that build reported",
            describe(recorded)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_build_is_a_commit_with_an_optional_dirty_mark_or_unknown() {
        assert!(!CURRENT.is_empty());
        let (commit, mark) = CURRENT.split_once('+').unwrap_or((CURRENT, ""));
        assert!(commit == "unknown" || commit.chars().all(|c| c.is_ascii_hexdigit()), "{CURRENT}");
        assert!(mark.is_empty() || mark.len() == 8 && mark.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn only_the_same_build_is_quiet() {
        assert!(!is_other(Some(CURRENT)));
        assert!(is_other(Some("0000000")));
        assert!(is_other(None));
        assert_eq!(mismatch_warning("the log", Some(CURRENT)), None);
        let w = mismatch_warning("the log", Some("0000000")).unwrap();
        assert!(w.contains("build 0000000") && w.contains(CURRENT) && w.contains("may not"), "{w}");
        assert!(mismatch_warning("the log", None).unwrap().contains("an unrecorded build"));
    }
}

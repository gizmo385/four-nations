//! The embedded porcelain layer (#480, docs/design/agent-scripting.md, "The SDK", "Porcelain:
//! `prelude.rhai`"): [`SOURCE`] is `prelude.rhai` itself, merged into every `run_script` call
//! ahead of the caller's own code (`crate::script::runner::run`). [`reference`] turns its own
//! `///` doc comments into the porcelain half of `sdk_reference` (`crate::tools`), read back
//! from a fresh compile of [`SOURCE`] rather than kept by hand, so the reference can't say
//! anything about a porcelain call the file itself doesn't actually define.

use rhai::Engine;
use serde::Serialize;

/// `prelude.rhai`, embedded at compile time.
pub const SOURCE: &str = include_str!("prelude.rhai");

/// Rhai's own prefix for the auto-generated name of an anonymous function (a `|a, b| ...`
/// closure passed to `sort`/`filter`, say): [`rhai::engine::FN_ANONYMOUS`], not itself exported
/// from the crate (`internals`-gated, and even then not under this name), so this is the same
/// literal by convention rather than by reference -- `reference`'s own closures showing up
/// here with no doc comment of their own (there is nowhere to put one on an inline `|...|`) is
/// what this filters out, not a maintenance hazard.
const ANONYMOUS_FN_PREFIX: &str = "anon$";

/// The porcelain layer's own per-function expression depth, raised above Rhai's debug-build
/// default (16, #480) to fit the nested loops `free_origins`'s own helpers need: matches
/// `runner::run`'s own engine, so a function that compiles here compiles there too.
const MAX_EXPR_DEPTH: usize = 128;
const MAX_FUNCTION_EXPR_DEPTH: usize = 64;

/// An [`Engine`] configured to compile [`SOURCE`] (and merge it with a caller's own code, which
/// is `runner::run`'s own job): no registered plumbing, since compiling -- unlike running --
/// never calls any of it.
pub(crate) fn engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_expr_depths(MAX_EXPR_DEPTH, MAX_FUNCTION_EXPR_DEPTH);
    engine
}

/// Whether `code` parses with the same engine settings `runner::run` uses (nothing is run, and
/// undefined functions only fail at call time): for a test that a documented example is real
/// Rhai.
#[cfg(test)]
pub(crate) fn compile_check(code: &str) -> Result<(), String> {
    engine().compile(code).map(|_| ()).map_err(|e| e.to_string())
}

/// One porcelain function, for `sdk_reference`: its own name, its parameter names as written
/// (in order), and its `///` doc comment with the comment markers stripped.
#[derive(Clone, Debug, Serialize)]
pub struct PorcelainFn {
    pub name: String,
    pub params: Vec<String>,
    /// Empty when [`SOURCE`] defines the function with no `///` comment above it --
    /// `the_bundled_prelude_is_fully_documented` (below) is what actually keeps this from
    /// happening, not this type.
    pub doc: String,
}

/// Every function [`SOURCE`] actually defines by name -- not the anonymous closures a `sort`
/// or `filter` call inside it also compiles to their own, undocumentable, script functions
/// (`ANONYMOUS_FN_PREFIX`) -- in the order they're written, from a fresh compile: cheap (one
/// small, bundled file) and keeps `crate::tools`'s own `sdk_reference` dispatch from ever
/// caching a reference that's gone stale against the code.
pub fn reference() -> Vec<PorcelainFn> {
    let ast = engine().compile(SOURCE).expect("bundled prelude.rhai must compile");
    ast.iter_functions()
        .filter(|f| !f.name.starts_with(ANONYMOUS_FN_PREFIX))
        .map(|f| PorcelainFn {
            name: f.name.to_owned(),
            params: f.params.iter().map(|p| (*p).to_owned()).collect(),
            doc: clean_doc(&f.comments),
        })
        .collect()
}

/// `comments` (`rhai::func::ScriptFnMetadata::comments`: each element already a function's own
/// whole doc block, lines merged with embedded `\n`, every line still starting with its own
/// `///`), flattened into one plain sentence: every `///` marker and the leading space after it
/// stripped, blank lines dropped, what's left joined with a single space.
fn clean_doc(comments: &[&str]) -> String {
    comments
        .iter()
        .flat_map(|block| block.lines())
        .map(|line| line.trim_start_matches('/').trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bundled file has to actually compile for [`reference`] (and `runner::run`, which
    /// merges it into every call) to work at all.
    #[test]
    fn the_bundled_prelude_compiles() {
        engine().compile(SOURCE).expect("prelude.rhai should compile");
    }

    /// Every porcelain function the design doc names is actually there.
    #[test]
    fn reference_lists_every_porcelain_call_the_design_doc_names() {
        let functions = reference();
        let names: Vec<&str> = functions.iter().map(|f| f.name.as_str()).collect();
        for want in [
            "find",
            "designate_near",
            "build_near",
            "wait_until",
            "stalled",
            "store",
            "idle",
            "need",
        ] {
            assert!(names.contains(&want), "{want:?} missing from {names:?}");
        }
    }

    /// A test fails if any function `SOURCE` defines -- porcelain or private helper alike --
    /// has no `///` doc comment: the acceptance criterion `sdk_reference`'s own doc comment
    /// names, kept here rather than only at the tool boundary so it catches a bare private
    /// helper too, not only the names the design doc happens to list.
    #[test]
    fn the_bundled_prelude_is_fully_documented() {
        for f in reference() {
            assert!(!f.doc.is_empty(), "{} has no doc comment", f.name);
        }
    }

    /// [`clean_doc`] strips every line's own `///` marker and joins what's left with a space,
    /// dropping any blank line.
    #[test]
    fn clean_doc_strips_markers_and_joins_lines() {
        let got = clean_doc(&["/// First line.\n///\n/// Second line."]);
        assert_eq!(got, "First line. Second line.");
    }
}

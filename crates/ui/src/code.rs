//! A Rhai syntax highlighter and an LCS line diff for the agent studio's code pane
//! (docs/design/agent-studio.md, "Highlighting", "Diff"). Both are plain functions over text:
//! no I/O, no colour and no drawing. `fn_tui` maps each [`Class`] to a colour (`data/tui.toml`'s
//! `[code]` table) and draws the diff markers; this module only classifies and compares.
//!
//! The lexer is hand-written rather than built on `rhai`'s own tokenizer (which sits behind its
//! `internals` feature): a display lexer that's occasionally wrong in a harmless way is cheaper
//! than putting a script engine in the TUI's dependency tree.

use std::cmp::max;

/// How a byte range of a line reads, for a frontend's own colour table. Classifying is this
/// module's whole job; a frontend decides what each variant looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Plain,
    Keyword,
    Literal,
    Number,
    String,
    Comment,
    Call,
    Op,
}

/// A classified byte range within one line, as returned by [`highlight_line`]. Ranges are
/// relative to that line, not the whole source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub class: Class,
}

/// What a block comment or a backtick string leaves open at the end of a line, carried into the
/// next [`highlight_line`] call so a token that spans lines keeps its class on every line it
/// covers. A `${...}` interpolation inside a backtick string never itself carries across a line
/// end: a line that ends mid-interpolation is treated as back inside the literal on the next
/// line (see "Rejected" below for why `highlight` doesn't try harder).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LexState {
    /// Nesting depth of an open `/* ... */` block comment. Zero outside one.
    block_comment_depth: u32,
    /// Whether a backtick string opened on an earlier line is still open.
    in_backtick: bool,
}

/// Rhai's reserved words (docs/design/agent-studio.md, "Highlighting"). `true`, `false` and the
/// unit literal `()` are [`Class::Literal`], not here.
const KEYWORDS: &[&str] = &[
    "let", "const", "fn", "if", "else", "switch", "while", "loop", "do", "until", "for", "in",
    "break", "continue", "return", "throw", "try", "catch", "private", "this", "global", "import",
    "export", "as",
];

fn is_op_char(c: char) -> bool {
    matches!(
        c,
        '+' | '-'
            | '*'
            | '/'
            | '%'
            | '='
            | '<'
            | '>'
            | '!'
            | '&'
            | '|'
            | '^'
            | '~'
            | '?'
            | ':'
            | '.'
            | ','
            | ';'
            | '('
            | ')'
            | '{'
            | '}'
            | '['
            | ']'
    )
}

/// Pushes `[start, end)` as `class`, merging into the previous span when it's the same class and
/// directly adjacent, so a run of plain text or operator characters comes back as one span.
fn push(spans: &mut Vec<Span>, start: usize, end: usize, class: Class) {
    if start == end {
        return;
    }
    if let Some(last) = spans.last_mut()
        && last.class == class
        && last.end == start
    {
        last.end = end;
        return;
    }
    spans.push(Span { start, end, class });
}

fn char_at(chars: &[(usize, char)], idx: usize) -> Option<char> {
    chars.get(idx).map(|&(_, c)| c)
}

fn matches2(chars: &[(usize, char)], idx: usize, a: char, b: char) -> bool {
    char_at(chars, idx) == Some(a) && char_at(chars, idx + 1) == Some(b)
}

/// Scans a `/* ... */` body starting at `idx` (just past the opening `/*`, at the given nesting
/// `depth`, normally 1 for a comment that opens on this line), handling nested `/* */` pairs.
/// Returns the index just past the end of what it consumed and the depth left open (0 if the
/// comment closed before the line did).
fn scan_comment_body(chars: &[(usize, char)], mut idx: usize, mut depth: u32) -> (usize, u32) {
    while idx < chars.len() && depth > 0 {
        if matches2(chars, idx, '/', '*') {
            depth += 1;
            idx += 2;
        } else if matches2(chars, idx, '*', '/') {
            depth -= 1;
            idx += 2;
        } else {
            idx += 1;
        }
    }
    (idx, depth)
}

/// Scans a backtick string's literal content starting at `idx` (just past the opening backtick,
/// or continuing one from an earlier line), until a closing backtick, the start of a `${`
/// interpolation, or the end of the line. Returns the index just past what it consumed, whether
/// it closed the string, and whether it stopped at `${`.
fn scan_backtick_body(chars: &[(usize, char)], mut idx: usize) -> (usize, bool, bool) {
    while idx < chars.len() {
        let c = chars[idx].1;
        if c == '`' {
            return (idx + 1, true, false);
        }
        if c == '$' && char_at(chars, idx + 1) == Some('{') {
            return (idx, false, true);
        }
        idx += 1;
    }
    (idx, false, false)
}

/// Scans a `"..."` or `'...'` literal starting at the opening quote, honouring `\` escapes.
/// Returns the index just past the closing quote, or the end of the line if it's unterminated
/// (these quoted forms don't span lines, unlike backtick strings).
fn lex_quoted(chars: &[(usize, char)], idx: usize, quote: char) -> usize {
    let mut i = idx + 1;
    while i < chars.len() {
        let c = chars[i].1;
        if c == '\\' && i + 1 < chars.len() {
            i += 2;
            continue;
        }
        if c == quote {
            return i + 1;
        }
        i += 1;
    }
    i
}

/// Scans a number literal starting at its first digit: decimal, with an optional single `.`
/// fraction, an optional `e`/`E` exponent, and `0x`/`0o`/`0b` prefixes. A trailing alphanumeric
/// run (a type suffix) is swept in too. Returns the index just past the literal.
fn lex_number(chars: &[(usize, char)], idx: usize) -> usize {
    let mut i = idx;
    let is_radix_prefix = char_at(chars, i) == Some('0')
        && matches!(char_at(chars, i + 1), Some('x' | 'X' | 'o' | 'O' | 'b' | 'B'));
    if is_radix_prefix {
        i += 2;
        while matches!(char_at(chars, i), Some(c) if c.is_ascii_alphanumeric() || c == '_') {
            i += 1;
        }
        return i;
    }
    while matches!(char_at(chars, i), Some(c) if c.is_ascii_digit() || c == '_') {
        i += 1;
    }
    if char_at(chars, i) == Some('.')
        && matches!(char_at(chars, i + 1), Some(c) if c.is_ascii_digit())
    {
        i += 1;
        while matches!(char_at(chars, i), Some(c) if c.is_ascii_digit() || c == '_') {
            i += 1;
        }
    }
    if matches!(char_at(chars, i), Some('e' | 'E')) {
        let mut j = i + 1;
        if matches!(char_at(chars, j), Some('+' | '-')) {
            j += 1;
        }
        if matches!(char_at(chars, j), Some(c) if c.is_ascii_digit()) {
            i = j;
            while matches!(char_at(chars, i), Some(c) if c.is_ascii_digit()) {
                i += 1;
            }
        }
    }
    while matches!(char_at(chars, i), Some(c) if c.is_ascii_alphanumeric()) {
        i += 1;
    }
    i
}

/// Scans an identifier (or keyword) starting at its first letter/`_`. Returns the index just
/// past it.
fn lex_word(chars: &[(usize, char)], idx: usize) -> usize {
    let mut i = idx;
    while matches!(char_at(chars, i), Some(c) if c.is_alphanumeric() || c == '_') {
        i += 1;
    }
    i
}

/// Classifies one line of Rhai source, carrying `state` across calls so a block comment or a
/// backtick string keeps its class on every line it spans. Spans cover the whole line, in
/// order, with no gaps or overlaps.
pub fn highlight_line(line: &str, state: &mut LexState) -> Vec<Span> {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let len = line.len();
    let byte_at = |i: usize| -> usize { chars.get(i).map_or(len, |&(b, _)| b) };

    let mut spans = Vec::new();
    let mut idx = 0usize;
    // The brace depth of a `${...}` interpolation opened on this line; 0 outside one. Never
    // carried across lines (see `LexState`'s own doc comment).
    let mut interp_depth: u32 = 0;

    while idx < chars.len() {
        if state.block_comment_depth > 0 {
            let start = byte_at(idx);
            let (end_idx, depth) = scan_comment_body(&chars, idx, state.block_comment_depth);
            state.block_comment_depth = depth;
            idx = end_idx;
            push(&mut spans, start, byte_at(idx), Class::Comment);
            continue;
        }
        if state.in_backtick && interp_depth == 0 {
            let start = byte_at(idx);
            let (new_idx, closed, hit_interp) = scan_backtick_body(&chars, idx);
            idx = new_idx;
            push(&mut spans, start, byte_at(idx), Class::String);
            if closed {
                state.in_backtick = false;
            }
            if hit_interp {
                let dstart = byte_at(idx);
                idx += 2;
                push(&mut spans, dstart, byte_at(idx), Class::String);
                interp_depth = 1;
            }
            continue;
        }

        let c = chars[idx].1;
        match c {
            '{' if interp_depth > 0 => {
                interp_depth += 1;
                push(&mut spans, byte_at(idx), byte_at(idx + 1), Class::Op);
                idx += 1;
            }
            '}' if interp_depth > 0 => {
                interp_depth -= 1;
                push(&mut spans, byte_at(idx), byte_at(idx + 1), Class::Op);
                idx += 1;
            }
            '/' if matches2(&chars, idx, '/', '/') => {
                push(&mut spans, byte_at(idx), len, Class::Comment);
                idx = chars.len();
            }
            '/' if matches2(&chars, idx, '/', '*') => {
                let start = byte_at(idx);
                let (end_idx, depth) = scan_comment_body(&chars, idx + 2, 1);
                state.block_comment_depth = depth;
                idx = end_idx;
                push(&mut spans, start, byte_at(idx), Class::Comment);
            }
            '"' => {
                let start = byte_at(idx);
                idx = lex_quoted(&chars, idx, '"');
                push(&mut spans, start, byte_at(idx), Class::String);
            }
            '\'' => {
                let start = byte_at(idx);
                idx = lex_quoted(&chars, idx, '\'');
                push(&mut spans, start, byte_at(idx), Class::String);
            }
            '`' => {
                let start = byte_at(idx);
                let (new_idx, closed, hit_interp) = scan_backtick_body(&chars, idx + 1);
                idx = new_idx;
                push(&mut spans, start, byte_at(idx), Class::String);
                if !closed {
                    state.in_backtick = true;
                }
                if hit_interp {
                    let dstart = byte_at(idx);
                    idx += 2;
                    push(&mut spans, dstart, byte_at(idx), Class::String);
                    interp_depth = 1;
                }
            }
            '(' if char_at(&chars, idx + 1) == Some(')') => {
                push(&mut spans, byte_at(idx), byte_at(idx + 2), Class::Literal);
                idx += 2;
            }
            c if c.is_ascii_digit() => {
                let start = byte_at(idx);
                idx = lex_number(&chars, idx);
                push(&mut spans, start, byte_at(idx), Class::Number);
            }
            c if c == '_' || c.is_alphabetic() => {
                let start = byte_at(idx);
                let end_idx = lex_word(&chars, idx);
                let word = &line[start..byte_at(end_idx)];
                let class = if KEYWORDS.contains(&word) {
                    Class::Keyword
                } else if word == "true" || word == "false" {
                    Class::Literal
                } else if char_at(&chars, end_idx) == Some('(') {
                    Class::Call
                } else {
                    Class::Plain
                };
                idx = end_idx;
                push(&mut spans, start, byte_at(idx), class);
            }
            c if is_op_char(c) => {
                push(&mut spans, byte_at(idx), byte_at(idx + 1), Class::Op);
                idx += 1;
            }
            _ => {
                push(&mut spans, byte_at(idx), byte_at(idx + 1), Class::Plain);
                idx += 1;
            }
        }
    }
    spans
}

/// Classifies every line of `src`, threading one [`LexState`] through from the first line to
/// the last. One `Vec<Span>` per line, in source order; a trailing newline doesn't add an extra
/// empty line (`str::lines`'s own rule).
pub fn highlight(src: &str) -> Vec<Vec<Span>> {
    let mut state = LexState::default();
    src.lines().map(|line| highlight_line(line, &mut state)).collect()
}

/// How one line of the new revision compares to the old one, per [`line_diff`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Same,
    Added,
    /// This line of `new` is immediately followed (before the next surviving line) by one or
    /// more lines of `old` that aren't in `new`.
    RemovedAfter,
}

/// Above this many `old.len() * new.len()` cells, [`line_diff`] gives up on the LCS and marks
/// every line of `new` as `Added` rather than doing `O(n*m)` work.
const DIFF_CELL_CAP: usize = 1_000_000;

/// An LCS line diff between two full source texts, one [`Mark`] per line of `new`
/// (docs/design/agent-studio.md, "Diff"). A changed line counts as one deletion and one
/// addition, the way `diff` counts it; a deletion with no surviving line above it (at the very
/// start of the file) still counts toward `deleted` even though there's no line to mark.
pub fn line_diff(old: &str, new: &str) -> (Vec<Mark>, usize, usize) {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let n = old_lines.len();
    let m = new_lines.len();

    if n.saturating_mul(m) > DIFF_CELL_CAP {
        return (vec![Mark::Added; m], m, n);
    }

    let width = m + 1;
    let mut dp = vec![0u32; (n + 1) * width];
    for i in 1..=n {
        for j in 1..=m {
            dp[i * width + j] = if old_lines[i - 1] == new_lines[j - 1] {
                dp[(i - 1) * width + (j - 1)] + 1
            } else {
                max(dp[(i - 1) * width + j], dp[i * width + (j - 1)])
            };
        }
    }

    enum Op {
        Equal,
        Insert,
        Delete,
    }
    let mut ops = Vec::with_capacity(n + m);
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            ops.push(Op::Equal);
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i * width + (j - 1)] >= dp[(i - 1) * width + j]) {
            ops.push(Op::Insert);
            j -= 1;
        } else {
            ops.push(Op::Delete);
            i -= 1;
        }
    }
    ops.reverse();

    let mut marks = Vec::with_capacity(m);
    let mut added = 0usize;
    let mut deleted = 0usize;
    let mut last_anchor: Option<usize> = None;
    let mut pending_delete = false;
    for op in ops {
        match op {
            Op::Equal => {
                if pending_delete {
                    if let Some(a) = last_anchor {
                        marks[a] = Mark::RemovedAfter;
                    }
                    pending_delete = false;
                }
                marks.push(Mark::Same);
                last_anchor = Some(marks.len() - 1);
            }
            Op::Insert => {
                marks.push(Mark::Added);
                added += 1;
            }
            Op::Delete => {
                pending_delete = true;
                deleted += 1;
            }
        }
    }
    if pending_delete && let Some(a) = last_anchor {
        marks[a] = Mark::RemovedAfter;
    }

    (marks, added, deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every span in `spans` must cover `line` exactly: in order, no gaps, no overlaps.
    fn assert_covers(line: &str, spans: &[Span]) {
        let mut pos = 0usize;
        for s in spans {
            assert!(s.start < s.end, "empty or backwards span in {line:?}: {spans:?}");
            assert_eq!(s.start, pos, "gap or overlap before {s:?} in {line:?}: {spans:?}");
            pos = s.end;
        }
        assert_eq!(pos, line.len(), "spans don't cover all of {line:?}: {spans:?}");
    }

    fn one_line(src: &str) -> Vec<Span> {
        let mut state = LexState::default();
        let spans = highlight_line(src, &mut state);
        assert_covers(src, &spans);
        spans
    }

    fn texts<'a>(src: &'a str, spans: &[Span]) -> Vec<&'a str> {
        spans.iter().map(|s| &src[s.start..s.end]).collect()
    }

    #[test]
    fn keywords_are_classified() {
        for kw in KEYWORDS {
            let line = format!("{kw} x");
            let spans = one_line(&line);
            assert_eq!(spans[0].class, Class::Keyword, "{kw} in {line:?}: {spans:?}");
            assert_eq!(&line[spans[0].start..spans[0].end], *kw);
        }
    }

    #[test]
    fn keyword_followed_by_paren_stays_a_keyword() {
        let spans = one_line("if(x)");
        assert_eq!(spans[0].class, Class::Keyword);
    }

    #[test]
    fn true_false_and_unit_are_literals() {
        let spans = one_line("let a = true;");
        assert_eq!(
            spans.iter().find(|s| s.class == Class::Literal).map(|s| s.class),
            Some(Class::Literal)
        );

        let spans = one_line("false");
        assert_eq!(spans[0].class, Class::Literal);

        let spans = one_line("let u = ();");
        let unit = spans.iter().find(|s| s.class == Class::Literal).unwrap();
        assert_eq!(&"let u = ();"[unit.start..unit.end], "()");
    }

    #[test]
    fn identifier_followed_by_paren_is_a_call() {
        let spans = one_line("foo(1, 2)");
        assert_eq!(spans[0].class, Class::Call);
        assert_eq!(&"foo(1, 2)"[spans[0].start..spans[0].end], "foo");
    }

    #[test]
    fn identifier_not_followed_by_paren_is_plain() {
        let spans = one_line("foo = bar");
        assert!(spans.iter().any(|s| s.class == Class::Plain));
        assert!(spans.iter().all(|s| s.class != Class::Call));
    }

    #[test]
    fn double_quoted_string() {
        let spans = one_line(r#"let s = "a \"b\" c";"#);
        let strings: Vec<&str> = texts(r#"let s = "a \"b\" c";"#, &spans)
            .into_iter()
            .zip(spans.iter())
            .filter(|(_, s)| s.class == Class::String)
            .map(|(t, _)| t)
            .collect();
        assert_eq!(strings, vec![r#""a \"b\" c""#]);
    }

    #[test]
    fn char_literal_is_a_string() {
        let spans = one_line("let c = 'x';");
        let lit = spans.iter().find(|s| s.class == Class::String).unwrap();
        assert_eq!(&"let c = 'x';"[lit.start..lit.end], "'x'");
    }

    #[test]
    fn backtick_string_single_line() {
        let src = "let s = `hello`;";
        let spans = one_line(src);
        let strings: Vec<&str> = spans
            .iter()
            .filter(|s| s.class == Class::String)
            .map(|s| &src[s.start..s.end])
            .collect();
        assert_eq!(strings, vec!["`hello`"]);
    }

    #[test]
    fn backtick_interpolation_is_lexed_as_code() {
        let src = "`sum: ${a + 1}`";
        let spans = one_line(src);
        assert_covers(src, &spans);
        // The identifier inside `${...}` reads as code (Plain, not part of the String). It
        // merges with the following space into one Plain span.
        let a_byte = src.find('a').unwrap();
        let ident = spans.iter().find(|s| s.start <= a_byte && a_byte < s.end).unwrap();
        assert_eq!(ident.class, Class::Plain);
        let number = spans.iter().find(|s| &src[s.start..s.end] == "1").unwrap();
        assert_eq!(number.class, Class::Number);
        // The literal text outside the interpolation is still String.
        // The opening backtick, the literal text and the `${` marker are all String, and merge
        // into one span since nothing non-String separates them.
        assert!(
            spans.iter().any(|s| s.class == Class::String && &src[s.start..s.end] == "`sum: ${")
        );
    }

    #[test]
    fn backtick_interpolation_call_inside() {
        let src = "`name: ${describe(x)}`";
        let spans = one_line(src);
        let call = spans.iter().find(|s| &src[s.start..s.end] == "describe").unwrap();
        assert_eq!(call.class, Class::Call);
    }

    #[test]
    fn line_comment_and_doc_comment() {
        let spans = one_line("// a comment");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].class, Class::Comment);

        let spans = one_line("/// a doc comment");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].class, Class::Comment);

        let src = "let x = 1; // trailing";
        let spans = one_line(src);
        let comment = spans.last().unwrap();
        assert_eq!(comment.class, Class::Comment);
        assert_eq!(&src[comment.start..comment.end], "// trailing");
    }

    #[test]
    fn nested_block_comment_single_line() {
        let src = "/* outer /* inner */ still outer */ code()";
        let spans = one_line(src);
        let comment = &spans[0];
        assert_eq!(comment.class, Class::Comment);
        assert_eq!(&src[comment.start..comment.end], "/* outer /* inner */ still outer */");
        let call = spans.iter().find(|s| &src[s.start..s.end] == "code").unwrap();
        assert_eq!(call.class, Class::Call);
    }

    #[test]
    fn block_comment_spans_lines() {
        let mut state = LexState::default();
        let l1 = highlight_line("code(); /* start", &mut state);
        assert_covers("code(); /* start", &l1);
        assert_eq!(state.block_comment_depth, 1);
        assert_eq!(l1.last().unwrap().class, Class::Comment);

        let l2 = highlight_line("still a comment", &mut state);
        assert_covers("still a comment", &l2);
        assert_eq!(state.block_comment_depth, 1);
        assert_eq!(l2.len(), 1);
        assert_eq!(l2[0].class, Class::Comment);

        let l3 = highlight_line("end here */ foo()", &mut state);
        assert_covers("end here */ foo()", &l3);
        assert_eq!(state.block_comment_depth, 0);
        assert_eq!(l3[0].class, Class::Comment);
        assert_eq!(&"end here */ foo()"[l3[0].start..l3[0].end], "end here */");
        let call = l3.iter().find(|s| &"end here */ foo()"[s.start..s.end] == "foo").unwrap();
        assert_eq!(call.class, Class::Call);
    }

    #[test]
    fn nested_block_comment_spans_lines() {
        let mut state = LexState::default();
        let l1 = highlight_line("/* outer /* inner", &mut state);
        assert_covers("/* outer /* inner", &l1);
        assert_eq!(state.block_comment_depth, 2);

        let l2 = highlight_line("*/ still outer", &mut state);
        assert_covers("*/ still outer", &l2);
        assert_eq!(state.block_comment_depth, 1);
        assert_eq!(l2[0].class, Class::Comment);

        let l3 = highlight_line("*/ code", &mut state);
        assert_covers("*/ code", &l3);
        assert_eq!(state.block_comment_depth, 0);
    }

    #[test]
    fn backtick_string_spans_lines() {
        let mut state = LexState::default();
        let l1 = highlight_line("let s = `abc", &mut state);
        assert_covers("let s = `abc", &l1);
        assert!(state.in_backtick);
        assert_eq!(l1.last().unwrap().class, Class::String);

        let l2 = highlight_line("def`;", &mut state);
        assert_covers("def`;", &l2);
        assert!(!state.in_backtick);
        assert_eq!(l2[0].class, Class::String);
        assert_eq!(&"def`;"[l2[0].start..l2[0].end], "def`");
    }

    #[test]
    fn empty_line_has_no_spans() {
        let spans = one_line("");
        assert!(spans.is_empty());
    }

    #[test]
    fn operators_merge_into_one_span() {
        let line = "a <= b && c";
        let spans = one_line(line);
        let op = spans.iter().find(|s| &line[s.start..s.end] == "<=").unwrap();
        assert_eq!(op.class, Class::Op);
        let op = spans.iter().find(|s| &line[s.start..s.end] == "&&").unwrap();
        assert_eq!(op.class, Class::Op);
    }

    #[test]
    fn highlight_splits_into_lines() {
        let src = "let a = 1;\nlet b = 2;\n";
        let lines = highlight(src);
        assert_eq!(lines.len(), 2);
        for (line, spans) in src.lines().zip(lines.iter()) {
            assert_covers(line, spans);
        }
    }

    // --- Fixture: the real scripts this lexer has to survive. ---

    fn fixture(path: &str) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {path}: {e}"))
    }

    fn assert_never_panics_and_keywords_classified(src: &str) {
        let lines: Vec<&str> = src.lines().collect();
        let highlighted = highlight(src);
        assert_eq!(highlighted.len(), lines.len());
        for (line, spans) in lines.iter().zip(highlighted.iter()) {
            assert_covers(line, spans);

            // Every keyword-shaped word that sits outside a comment or a string must have come
            // back as Keyword (a keyword spelled out in prose inside a comment, which both
            // fixture files have plenty of, doesn't count).
            let chars: Vec<(usize, char)> = line.char_indices().collect();
            let mut idx = 0usize;
            while idx < chars.len() {
                let c = chars[idx].1;
                if c == '_' || c.is_alphabetic() {
                    let start = idx;
                    let mut end = idx;
                    while end < chars.len() {
                        let c = chars[end].1;
                        if c == '_' || c.is_alphanumeric() {
                            end += 1;
                        } else {
                            break;
                        }
                    }
                    let start_byte = chars[start].0;
                    let end_byte = chars.get(end).map_or(line.len(), |&(b, _)| b);
                    let word = &line[start_byte..end_byte];
                    if KEYWORDS.contains(&word) {
                        let class = spans
                            .iter()
                            .find(|s| s.start == start_byte && s.end == end_byte)
                            .map(|s| s.class);
                        let context = spans
                            .iter()
                            .find(|s| s.start <= start_byte && start_byte < s.end)
                            .map(|s| s.class);
                        if context != Some(Class::Comment) && context != Some(Class::String) {
                            assert_eq!(
                                class,
                                Some(Class::Keyword),
                                "word {word:?} at {start_byte}..{end_byte} in {line:?} should be Keyword, spans: {spans:?}"
                            );
                        }
                    }
                    idx = end;
                } else {
                    idx += 1;
                }
            }
        }
    }

    #[test]
    fn fixture_settle_script_never_panics_and_keywords_classified() {
        let src = fixture(concat!(env!("CARGO_MANIFEST_DIR"), "/../agent/scripts/settle.rhai"));
        assert_never_panics_and_keywords_classified(&src);
    }

    #[test]
    fn fixture_prelude_never_panics_and_keywords_classified() {
        let src = fixture(concat!(env!("CARGO_MANIFEST_DIR"), "/../agent/src/script/prelude.rhai"));
        assert_never_panics_and_keywords_classified(&src);
    }

    // --- line_diff ---

    #[test]
    fn diff_identical() {
        let (marks, added, deleted) = line_diff("a\nb\nc", "a\nb\nc");
        assert_eq!(marks, vec![Mark::Same, Mark::Same, Mark::Same]);
        assert_eq!((added, deleted), (0, 0));
    }

    #[test]
    fn diff_pure_add() {
        let (marks, added, deleted) = line_diff("a", "a\nb\nc");
        assert_eq!(marks, vec![Mark::Same, Mark::Added, Mark::Added]);
        assert_eq!((added, deleted), (2, 0));
    }

    #[test]
    fn diff_pure_delete() {
        let (marks, added, deleted) = line_diff("a\nb\nc", "a");
        assert_eq!(marks, vec![Mark::RemovedAfter]);
        assert_eq!((added, deleted), (0, 2));
    }

    #[test]
    fn diff_change() {
        let (marks, added, deleted) = line_diff("a\nb\nc", "a\nx\nc");
        assert_eq!(marks, vec![Mark::RemovedAfter, Mark::Added, Mark::Same]);
        assert_eq!((added, deleted), (1, 1));
    }

    #[test]
    fn diff_empty_old() {
        let (marks, added, deleted) = line_diff("", "a\nb");
        assert_eq!(marks, vec![Mark::Added, Mark::Added]);
        assert_eq!((added, deleted), (2, 0));
    }

    #[test]
    fn diff_empty_new() {
        let (marks, added, deleted) = line_diff("a\nb", "");
        assert_eq!(marks, Vec::<Mark>::new());
        assert_eq!((added, deleted), (0, 2));
    }

    #[test]
    fn diff_both_empty() {
        let (marks, added, deleted) = line_diff("", "");
        assert_eq!(marks, Vec::<Mark>::new());
        assert_eq!((added, deleted), (0, 0));
    }

    #[test]
    fn diff_above_cell_cap_marks_everything_added() {
        let old: String = (0..1001).map(|i| format!("old{i}\n")).collect();
        let new: String = (0..1001).map(|i| format!("new{i}\n")).collect();
        let (marks, added, deleted) = line_diff(&old, &new);
        assert_eq!(marks.len(), 1001);
        assert!(marks.iter().all(|m| *m == Mark::Added));
        assert_eq!(added, 1001);
        assert_eq!(deleted, 1001);
    }
}

//! A display formatter for Rhai, for the studio's code pane (#558, docs/design/agent-studio.md,
//! "Display formatting"). Agents write scripts in whatever layout comes out, and reformat them
//! between revisions, so a raw line diff reads layout noise as change. This lays every script out
//! one way: one statement per line, four-space indent by block depth, one space around binary
//! operators, and a call, array or map literal that runs past [`FORMAT_WIDTH`] broken one item to
//! a line. Two revisions that differ only in layout then format to the same text.
//!
//! It only ever affects what the pane shows: logs and the code that runs are never touched.
//! Comments and strings are copied exactly. Code that won't lex (an unterminated string or
//! comment, unbalanced brackets) comes back as `None`, and the pane shows it raw, never dropped.
//!
//! The width is fixed rather than the pane's own, so the text (and so the diff counts the header
//! and the agent feed show) doesn't change when the terminal is resized. The pane wraps whatever
//! is still too long for it (#589).
//!
//! Like the highlighter, this is a token-level display tool, not a parser: it never needs the
//! script to be valid Rhai, only to lex, and where it guesses (is this `-` unary? is this `|` a
//! closure?) a wrong guess only costs a space.

use super::{KEYWORDS, lex_number, lex_word, scan_comment_body};

/// The width a line is broken to fit, where it can be: a little under the code pane's widest
/// (`data/tui.toml`'s `code_max_width`, minus its border and gutter).
pub const FORMAT_WIDTH: usize = 80;

const INDENT: &str = "    ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Word,
    Keyword,
    Number,
    Str,
    LineComment,
    BlockComment,
    Op,
    /// `(`, `[` or `{`.
    Open,
    /// `#{`, a map literal.
    MapOpen,
    /// `)`, `]` or `}`.
    Close,
}

#[derive(Debug, Clone)]
struct Tok<'a> {
    text: &'a str,
    kind: Kind,
    /// Line breaks in the whitespace just before this token: a comment with none trails the
    /// code before it, and two or more are a blank line the formatter keeps (one at most).
    nl_before: usize,
}

impl Tok<'_> {
    fn is(&self, kind: Kind, text: &str) -> bool {
        self.kind == kind && self.text == text
    }

    fn is_comment(&self) -> bool {
        matches!(self.kind, Kind::LineComment | Kind::BlockComment)
    }

    fn multiline(&self) -> bool {
        self.text.contains('\n')
    }
}

/// Operators, longest first so the lexer takes the longest match.
const OPS: &[&str] = &[
    "**=", "<<=", ">>=", "..=", "??=", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=",
    "%=", "&=", "|=", "^=", "**", "<<", ">>", "=>", "->", "::", "..", "?.", "?[", "??",
];

/// Splits `src` into tokens, or `None` if it won't lex: an unterminated string or block comment,
/// or brackets that don't pair up.
fn lex(src: &str) -> Option<Vec<Tok<'_>>> {
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let len = src.len();
    let byte_at = |i: usize| -> usize { chars.get(i).map_or(len, |&(b, _)| b) };
    let at = |i: usize| chars.get(i).map(|&(_, c)| c);

    let mut toks = Vec::new();
    let mut stack: Vec<char> = Vec::new();
    let mut i = 0usize;
    let mut nl = 0usize;
    while i < chars.len() {
        let c = chars[i].1;
        if c.is_whitespace() {
            if c == '\n' {
                nl += 1;
            }
            i += 1;
            continue;
        }
        let start = i;
        let kind = match c {
            '/' if at(i + 1) == Some('/') => {
                while i < chars.len() && chars[i].1 != '\n' {
                    i += 1;
                }
                Kind::LineComment
            }
            '/' if at(i + 1) == Some('*') => {
                let (end, depth) = scan_comment_body(&chars, i + 2, 1);
                if depth > 0 {
                    return None;
                }
                i = end;
                Kind::BlockComment
            }
            '"' | '\'' => {
                i += 1;
                loop {
                    match at(i) {
                        None | Some('\n') => return None,
                        Some('\\') => i += 2,
                        Some(q) if q == c => {
                            i += 1;
                            break;
                        }
                        Some(_) => i += 1,
                    }
                }
                Kind::Str
            }
            '`' => {
                i += 1;
                let mut interp = 0u32;
                loop {
                    match at(i) {
                        None => return None,
                        Some('`') if interp == 0 => {
                            i += 1;
                            break;
                        }
                        Some('$') if at(i + 1) == Some('{') => {
                            interp += 1;
                            i += 2;
                        }
                        Some('{') if interp > 0 => {
                            interp += 1;
                            i += 1;
                        }
                        Some('}') if interp > 0 => {
                            interp -= 1;
                            i += 1;
                        }
                        Some(_) => i += 1,
                    }
                }
                Kind::Str
            }
            '#' if at(i + 1) == Some('{') => {
                stack.push('}');
                i += 2;
                Kind::MapOpen
            }
            '(' | '[' | '{' => {
                stack.push(match c {
                    '(' => ')',
                    '[' => ']',
                    _ => '}',
                });
                i += 1;
                Kind::Open
            }
            ')' | ']' | '}' => {
                if stack.pop() != Some(c) {
                    return None;
                }
                i += 1;
                Kind::Close
            }
            c if c.is_ascii_digit() => {
                i = lex_number(&chars, i);
                Kind::Number
            }
            c if c == '_' || c.is_alphabetic() => {
                i = lex_word(&chars, i);
                let word = &src[byte_at(start)..byte_at(i)];
                if KEYWORDS.contains(&word) { Kind::Keyword } else { Kind::Word }
            }
            _ => {
                let rest = &src[byte_at(i)..];
                let op = OPS.iter().find(|op| rest.starts_with(**op));
                match op {
                    Some(op) => {
                        if *op == "?[" {
                            stack.push(']');
                        }
                        i += op.chars().count();
                    }
                    None => i += 1,
                }
                Kind::Op
            }
        };
        toks.push(Tok { text: &src[byte_at(start)..byte_at(i)], kind, nl_before: nl });
        nl = 0;
    }
    stack.is_empty().then_some(toks)
}

/// One output line before it's rendered: its indent depth and its tokens (indices into the token
/// list). An empty token list is a blank line.
#[derive(Debug, Clone)]
struct Line {
    indent: usize,
    toks: Vec<usize>,
    /// The line continues an expression from the one above and starts with its binary
    /// operator (`|| b`, `- c`), which mustn't be read as a unary one.
    lead_binary: bool,
}

/// What an open bracket is, for deciding where lines break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Frame {
    /// A `{` block: statements, one to a line.
    Block { is_do: bool },
    /// A `switch`'s `{`: arms, one to a line.
    Switch,
    /// `(`, `[`, `?[` or `#{`: kept on the line, unless it's too long.
    Inline,
    /// `{}`: a block with nothing in it, kept on the line.
    EmptyBlock { is_do: bool },
}

/// Formats `src` for display, or `None` if it won't lex (the caller shows it raw). Idempotent:
/// formatting formatted text changes nothing.
pub fn format(src: &str) -> Option<String> {
    let toks = lex(src)?;
    let lines = layout(&toks);
    let mut out_lines: Vec<Line> = Vec::new();
    for line in lines {
        break_long(&toks, line, &mut out_lines);
    }
    let mut out = String::with_capacity(src.len() + src.len() / 8);
    for line in &out_lines {
        out.push_str(&render(&toks, line));
        out.push('\n');
    }
    Some(out)
}

/// The first pass: one statement per line, by block depth, ignoring every line break in the
/// source except before a comment and the blank lines between statements.
fn layout(toks: &[Tok]) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut frames: Vec<Frame> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut cur_indent = 0usize;
    let mut pending_break = false;
    let mut pending_switch = false;
    let mut pending_do = false;
    let mut closed_do = false;
    // After a block comment: the next token starts a new line if it did in the source.
    let mut after_block_comment = false;

    let depth = |frames: &[Frame]| {
        frames.iter().filter(|f| matches!(f, Frame::Block { .. } | Frame::Switch)).count()
    };
    // A line that starts inside a bracket opened on an earlier line (only after a line comment
    // forces a break mid-expression) is indented one more.
    let continuing = |frames: &[Frame]| matches!(frames.last(), Some(Frame::Inline));

    let flush = |cur: &mut Vec<usize>, lines: &mut Vec<Line>, indent: usize| {
        if !cur.is_empty() {
            lines.push(Line { indent, toks: std::mem::take(cur), lead_binary: false });
        }
    };

    for (i, tok) in toks.iter().enumerate() {
        if pending_break {
            pending_break = false;
            let trailing = tok.is_comment() && tok.nl_before == 0;
            let joins = tok.is(Kind::Keyword, "else")
                || tok.is(Kind::Keyword, "catch")
                || (closed_do && tok.is(Kind::Keyword, "while"))
                || (closed_do && tok.is(Kind::Keyword, "until"))
                || matches!(tok.text, ")" | "]" | "," | ";" | "." | "?." | "?[");
            if !trailing && !joins {
                flush(&mut cur, &mut lines, cur_indent);
            }
        }
        closed_do = false;
        if after_block_comment && tok.nl_before > 0 {
            flush(&mut cur, &mut lines, cur_indent);
        }
        after_block_comment = tok.kind == Kind::BlockComment;
        // Rhai wants a `;` between statements, but a script that left one out (or a snippet
        // with none) shouldn't run its lines together: a line that ends an operand followed by
        // one that starts a new one can't be one expression, so the break stays.
        if tok.nl_before > 0
            && !matches!(frames.last(), Some(Frame::Inline))
            && cur.last().is_some_and(|&t| ends_operand(&toks[t]))
            && starts_statement(tok)
        {
            flush(&mut cur, &mut lines, cur_indent);
        }

        // Blank lines between statements survive, one at most; never just inside a `{` or
        // just before a `}`.
        if cur.is_empty() && tok.nl_before >= 2 && !tok.is(Kind::Close, "}") {
            let after_open = lines
                .last()
                .and_then(|l| l.toks.last())
                .is_some_and(|&t| toks[t].kind == Kind::Open && toks[t].text == "{");
            if !lines.is_empty() && !lines.last().is_some_and(|l| l.toks.is_empty()) && !after_open
            {
                lines.push(Line { indent: 0, toks: Vec::new(), lead_binary: false });
            }
        }
        // A comment on a line of its own stays on a line of its own.
        if tok.is_comment() && tok.nl_before > 0 {
            flush(&mut cur, &mut lines, cur_indent);
        }

        if cur.is_empty() {
            cur_indent = depth(&frames);
            if continuing(&frames) && tok.kind != Kind::Close {
                cur_indent += 1;
            }
        }

        match tok.kind {
            Kind::Open if tok.text == "{" => {
                let frame =
                    if pending_switch { Frame::Switch } else { Frame::Block { is_do: pending_do } };
                pending_switch = false;
                pending_do = false;
                cur.push(i);
                if toks.get(i + 1).is_some_and(|t| t.is(Kind::Close, "}")) {
                    frames.push(Frame::EmptyBlock {
                        is_do: matches!(frame, Frame::Block { is_do: true }),
                    });
                } else {
                    frames.push(frame);
                    flush(&mut cur, &mut lines, cur_indent);
                }
            }
            Kind::Open | Kind::MapOpen => {
                frames.push(Frame::Inline);
                cur.push(i);
            }
            Kind::Op if tok.text == "?[" => {
                frames.push(Frame::Inline);
                cur.push(i);
            }
            Kind::Close => {
                let frame = frames.pop();
                if matches!(frame, Some(Frame::Block { .. } | Frame::Switch)) {
                    flush(&mut cur, &mut lines, cur_indent);
                    cur_indent = depth(&frames);
                    cur.push(i);
                    pending_break = true;
                    closed_do = matches!(frame, Some(Frame::Block { is_do: true }));
                } else {
                    cur.push(i);
                    if let Some(Frame::EmptyBlock { is_do }) = frame {
                        pending_break = true;
                        closed_do = is_do;
                    }
                }
            }
            Kind::Op if tok.text == ";" => {
                cur.push(i);
                if !matches!(frames.last(), Some(Frame::Inline)) {
                    pending_break = true;
                }
            }
            Kind::Op if tok.text == "," => {
                cur.push(i);
                if matches!(frames.last(), Some(Frame::Switch)) {
                    pending_break = true;
                }
            }
            Kind::LineComment => {
                cur.push(i);
                flush(&mut cur, &mut lines, cur_indent);
            }
            _ => {
                if tok.is(Kind::Keyword, "switch") {
                    pending_switch = true;
                } else if tok.is(Kind::Keyword, "do") {
                    pending_do = true;
                }
                cur.push(i);
            }
        }
    }
    flush(&mut cur, &mut lines, cur_indent);
    while lines.last().is_some_and(|l| l.toks.is_empty()) {
        lines.pop();
    }
    lines
}

/// Whether `tok` can end an operand: a name, a literal or a closing bracket.
fn ends_operand(tok: &Tok) -> bool {
    matches!(tok.kind, Kind::Word | Kind::Number | Kind::Str | Kind::Close)
}

/// Whether `tok` can only start something new, not continue an expression: a name, a literal or
/// a statement keyword.
fn starts_statement(tok: &Tok) -> bool {
    match tok.kind {
        Kind::Word | Kind::Number | Kind::Str => true,
        Kind::Keyword => matches!(
            tok.text,
            "let"
                | "const"
                | "fn"
                | "if"
                | "switch"
                | "while"
                | "loop"
                | "do"
                | "for"
                | "break"
                | "continue"
                | "return"
                | "throw"
                | "try"
                | "import"
                | "export"
                | "private"
        ),
        _ => false,
    }
}

/// Whether the token before is one after which an operand (not an operator) comes next: what
/// decides that a `-` or `+` is unary and a `|` opens a closure.
fn operand_next(prev: Option<&Tok>) -> bool {
    match prev {
        None => true,
        Some(p) => match p.kind {
            Kind::Open | Kind::MapOpen | Kind::Op => true,
            Kind::Keyword => !matches!(p.text, "this"),
            _ => false,
        },
    }
}

/// Renders one line's tokens with normalised spacing.
fn render(toks: &[Tok], line: &Line) -> String {
    if line.toks.is_empty() {
        return String::new();
    }
    let mut out = INDENT.repeat(line.indent);
    // Per token: whether it's a unary operator, and where closure bars are.
    let mut unary_prev = false;
    let mut closure_open_prev = false;
    let mut in_closure_params = false;
    let mut prev: Option<&Tok> = None;
    for (n, &ti) in line.toks.iter().enumerate() {
        let tok = &toks[ti];
        let operand_pos = if n == 0 && line.lead_binary {
            false
        } else {
            operand_next(prev) || unary_prev || closure_open_prev
        };
        let mut unary = false;
        let mut closure_open = false;
        let mut closure_close = false;
        let mut empty_closure = false;
        if tok.kind == Kind::Op {
            match tok.text {
                "-" | "+" | "!" if operand_pos => unary = true,
                "!" => unary = true,
                "|" if in_closure_params => {
                    closure_close = true;
                    in_closure_params = false;
                }
                "|" if operand_pos => {
                    closure_open = true;
                    in_closure_params = true;
                }
                "||" if operand_pos => empty_closure = true,
                _ => {}
            }
        }

        if n > 0 && space_between(prev.unwrap(), tok, unary_prev, closure_open_prev, closure_close)
        {
            out.push(' ');
        }
        out.push_str(tok.text);

        unary_prev = unary;
        closure_open_prev = closure_open;
        if closure_close || empty_closure {
            // Treat what follows a closure's params like what follows `=`: an operand.
            prev = Some(&CLOSURE_END);
            continue;
        }
        prev = Some(tok);
    }
    out
}

/// A stand-in for the token before the body of a closure, after its `|...|`: an operand comes
/// next, and a space goes before it.
const CLOSURE_END: Tok<'static> = Tok { text: "|", kind: Kind::Op, nl_before: 0 };

/// Whether a space goes between `prev` and `next` on one line.
fn space_between(
    prev: &Tok,
    next: &Tok,
    prev_unary: bool,
    prev_closure_open: bool,
    next_closure_close: bool,
) -> bool {
    if next.kind == Kind::LineComment || next.kind == Kind::BlockComment {
        return true;
    }
    if prev.kind == Kind::BlockComment {
        return true;
    }
    if prev_unary || prev_closure_open || next_closure_close {
        return false;
    }
    let p = prev.text;
    let n = next.text;
    // Nothing after an opening bracket or a member access; nothing before a closing one, a
    // separator or a member access.
    if matches!(prev.kind, Kind::Open | Kind::MapOpen) || matches!(p, "." | "?." | "::" | "?[") {
        return false;
    }
    if next.kind == Kind::Close || matches!(n, "," | ";" | "." | "?." | "::" | ":" | "?[") {
        // `{` `}` blocks are on lines of their own; an inline `}` closes a map or `{}`.
        return false;
    }
    if matches!(p, ".." | "..=") || matches!(n, ".." | "..=") {
        return false;
    }
    // A call or an index: `f(`, `a[`, `f()(`. A keyword's bracket gets a space: `if (`.
    if next.kind == Kind::Open && matches!(n, "(" | "[") {
        return !(matches!(prev.kind, Kind::Word | Kind::Close | Kind::Str)
            || (prev.kind == Kind::Keyword && p == "this"));
    }
    true
}

/// Breaks `line` if it renders past [`FORMAT_WIDTH`], the way a person would, and checks each
/// part again:
///
/// 1. A chain of `||`, `&&`, `??` or binary `+`/`-` at the line's own top level breaks before
///    each operator (the loosest-binding kind there is), continuation lines indented one more.
/// 2. Otherwise the outermost bracket group holding two or more items (a call's arguments, an
///    array or a map) goes one item to a line.
/// 3. Otherwise the line stays as it is, and the pane wraps it.
///
/// A line holding a line comment or a token that spans lines is left as it is.
fn break_long(toks: &[Tok], line: Line, out: &mut Vec<Line>) {
    let width = render(toks, &line).chars().count();
    let unbreakable =
        line.toks.iter().any(|&t| toks[t].multiline() || toks[t].kind == Kind::LineComment);
    if width <= FORMAT_WIDTH || unbreakable || line.toks.len() < 2 {
        out.push(line);
        return;
    }
    if let Some(cuts) = operator_cuts(toks, &line.toks) {
        let mut from = 0;
        for (n, &cut) in cuts.iter().chain(std::iter::once(&line.toks.len())).enumerate() {
            let piece = Line {
                indent: if n == 0 { line.indent } else { line.indent + 1 },
                toks: line.toks[from..cut].to_vec(),
                lead_binary: if n == 0 { line.lead_binary } else { true },
            };
            break_long(toks, piece, out);
            from = cut;
        }
        return;
    }
    let Some((open, close, commas)) = outer_list(toks, &line.toks) else {
        out.push(line);
        return;
    };
    let part = |range: std::ops::Range<usize>, indent: usize| Line {
        indent,
        toks: line.toks[range].to_vec(),
        lead_binary: false,
    };
    let head = Line { lead_binary: line.lead_binary, ..part(0..open + 1, line.indent) };
    break_long(toks, head, out);
    let mut from = open + 1;
    for &comma in &commas {
        break_long(toks, part(from..comma + 1, line.indent + 1), out);
        from = comma + 1;
    }
    if from < close {
        break_long(toks, part(from..close, line.indent + 1), out);
    }
    break_long(toks, part(close..line.toks.len(), line.indent), out);
}

/// Each token's bracket depth within a line, counted from the line's start (a line can start
/// inside a bracket, so depths can go below zero).
fn depths(toks: &[Tok], line: &[usize]) -> Vec<i32> {
    let mut depth = 0i32;
    line.iter()
        .map(|&t| {
            let tok = &toks[t];
            if tok.kind == Kind::Close {
                depth -= 1;
            }
            let here = depth;
            if matches!(tok.kind, Kind::Open | Kind::MapOpen) || tok.is(Kind::Op, "?[") {
                depth += 1;
            }
            here
        })
        .collect()
}

/// Where to break a long line before its top-level binary operators ([`break_long`]'s first
/// rule): positions within `line`, or `None` if it has no such chain.
fn operator_cuts(toks: &[Tok], line: &[usize]) -> Option<Vec<usize>> {
    let depth = depths(toks, line);
    let top = *depth.iter().min()?;
    let binary_at = |pos: usize, ops: &[&str]| {
        let tok = &toks[line[pos]];
        pos > 0
            && depth[pos] == top
            && tok.kind == Kind::Op
            && ops.contains(&tok.text)
            && !operand_next(Some(&toks[line[pos - 1]]))
    };
    for ops in [&["||"][..], &["&&"], &["??"], &["+", "-"]] {
        let cuts: Vec<usize> = (1..line.len()).filter(|&p| binary_at(p, ops)).collect();
        if !cuts.is_empty() {
            return Some(cuts);
        }
    }
    None
}

/// The outermost bracket group on a line holding two or more items, opened and closed on it
/// (the widest, if several are equally far out): `(open, close, top-level commas)`, positions
/// within `line`. Commas between a closure's bars are its parameters, not items.
fn outer_list(toks: &[Tok], line: &[usize]) -> Option<(usize, usize, Vec<usize>)> {
    let depth = depths(toks, line);
    let mut best: Option<(i32, usize, usize, usize, Vec<usize>)> = None;
    let mut stack: Vec<(usize, Vec<usize>, bool)> = Vec::new();
    for (pos, &t) in line.iter().enumerate() {
        let tok = &toks[t];
        let opens = matches!(tok.kind, Kind::Open | Kind::MapOpen) || tok.is(Kind::Op, "?[");
        if opens {
            stack.push((pos, Vec::new(), false));
            continue;
        }
        if tok.kind == Kind::Close {
            let Some((open, commas, _)) = stack.pop() else { continue };
            if commas.is_empty() {
                continue;
            }
            let size: usize = line[open..=pos].iter().map(|&t| toks[t].text.len() + 1).sum();
            let d = depth[open];
            let better = best.as_ref().is_none_or(|b| d < b.0 || (d == b.0 && size > b.3));
            if better {
                best = Some((d, open, pos, size, commas));
            }
            continue;
        }
        let Some((_, commas, in_params)) = stack.last_mut() else { continue };
        if tok.is(Kind::Op, "|") {
            if *in_params {
                *in_params = false;
            } else if operand_next(pos.checked_sub(1).map(|p| &toks[line[p]])) {
                *in_params = true;
            }
        } else if tok.is(Kind::Op, ",") && !*in_params {
            commas.push(pos);
        }
    }
    best.map(|(_, open, close, _, commas)| (open, close, commas))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(src: &str) -> String {
        format(src).unwrap_or_else(|| panic!("should lex: {src:?}"))
    }

    fn assert_idempotent(src: &str) {
        let once = fmt(src);
        let twice = fmt(&once);
        assert_eq!(once, twice, "formatting formatted code changed it:\n{once}");
    }

    #[test]
    fn one_statement_per_line_with_block_indent() {
        let src = "fn f(a,b){let x=a+b;if x>1{return x;}else{x}}";
        assert_eq!(
            fmt(src),
            "fn f(a, b) {\n    let x = a + b;\n    if x > 1 {\n        return x;\n    } else {\n        x\n    }\n}\n"
        );
    }

    #[test]
    fn layout_only_changes_format_to_the_same_text() {
        let a = "let t = #{x: 1, y: 2};\nif t.x > 0 { go(t); }\n";
        let b = "let t=#{ x:1,\n   y:2 };\n\n\nif t.x>0\n{\n  go( t );\n}";
        // The blank lines between statements are kept (one at most), and the diff ignores them.
        assert_eq!(fmt(a).replace("\n\n", "\n"), fmt(b).replace("\n\n", "\n"));
    }

    #[test]
    fn lines_without_semicolons_stay_apart() {
        assert_eq!(fmt("a\nb\nc"), "a\nb\nc\n");
        assert_eq!(fmt("let x = 1\nlet y = 2"), "let x = 1\nlet y = 2\n");
        // But an expression continued on the next line is still one.
        assert_eq!(fmt("let x = a\n    + b;"), "let x = a + b;\n");
        assert_eq!(fmt("let x = o\n    .y;"), "let x = o.y;\n");
        assert_eq!(fmt("for k in a\n in b {}"), "for k in a in b {}\n");
    }

    #[test]
    fn unary_and_binary_minus() {
        assert_eq!(fmt("let a = -1 - -b;"), "let a = -1 - -b;\n");
        assert_eq!(fmt("return -x;"), "return -x;\n");
        assert_eq!(fmt("f(-1, a[0]-1);"), "f(-1, a[0] - 1);\n");
        assert_eq!(fmt("if !done { }"), "if !done {}\n");
    }

    #[test]
    fn closures_keep_their_bars_tight() {
        assert_eq!(fmt("a.sort(|p,q| p.x-q.x);"), "a.sort(|p, q| p.x - q.x);\n");
        assert_eq!(fmt("let f = || 1;"), "let f = || 1;\n");
        assert_eq!(fmt("let ok = a || b;"), "let ok = a || b;\n");
        assert_eq!(fmt("let m = a | b;"), "let m = a | b;\n");
    }

    #[test]
    fn closure_bodies_with_blocks() {
        let src = "list.filter(|x| { let y = x * 2; y > 3 });";
        assert_eq!(fmt(src), "list.filter(|x| {\n    let y = x * 2;\n    y > 3\n});\n");
    }

    #[test]
    fn switch_arms_go_one_to_a_line() {
        let src = "switch k { 1 => \"a\", 2 => { go(); }, _ => () }";
        assert_eq!(
            fmt(src),
            "switch k {\n    1 => \"a\",\n    2 => {\n        go();\n    },\n    _ => ()\n}\n"
        );
    }

    #[test]
    fn do_while_keeps_its_while() {
        assert_eq!(fmt("do { x += 1; } while x < 3;"), "do {\n    x += 1;\n} while x < 3;\n");
        // A `while` loop after a plain block starts its own line.
        assert_eq!(
            fmt("if a { b(); }\nwhile c { d(); }"),
            "if a {\n    b();\n}\nwhile c {\n    d();\n}\n"
        );
    }

    #[test]
    fn comments_and_strings_are_kept_exactly() {
        let src = "// lead   comment\nlet s = \"a  ,b;{\"; // trail  note\n/* block\n   spans */\nlet t = `x ${ y + 1 }  z`;\nlet c = 'q';";
        let out = fmt(src);
        assert!(out.contains("// lead   comment\n"), "{out}");
        assert!(out.contains("let s = \"a  ,b;{\"; // trail  note\n"), "{out}");
        assert!(out.contains("/* block\n   spans */\n"), "{out}");
        assert!(out.contains("`x ${ y + 1 }  z`"), "{out}");
        assert!(out.contains("'q'"), "{out}");
        assert_idempotent(src);
    }

    #[test]
    fn a_comment_mid_expression_breaks_the_line() {
        let src = "go(a, // first\n b);";
        let out = fmt(src);
        assert_eq!(out, "go(a, // first\n    b);\n");
        assert_idempotent(src);
    }

    #[test]
    fn blank_lines_are_kept_one_at_most() {
        let src = "let a = 1;\n\n\n\nlet b = 2;\nfn f() {\n\n    x();\n\n}\n";
        assert_eq!(fmt(src), "let a = 1;\n\nlet b = 2;\nfn f() {\n    x();\n}\n");
    }

    #[test]
    fn long_calls_and_maps_break_one_item_to_a_line() {
        let src = "set_targets(#{food_per_head: 12, wood: 300, stone: 200, clay: 150, scrolls: 5, tea: 4, rice: 9});";
        let out = fmt(src);
        for line in out.lines() {
            assert!(line.chars().count() <= FORMAT_WIDTH, "{line:?} in\n{out}");
        }
        assert!(out.contains("food_per_head: 12,\n"), "{out}");
        assert!(out.contains("rice: 9\n"), "{out}");
        assert_idempotent(src);
    }

    #[test]
    fn short_lines_are_not_broken() {
        assert_eq!(fmt("go(#{x: 1, y: 2});"), "go(#{x: 1, y: 2});\n");
    }

    #[test]
    fn code_that_will_not_lex_is_none() {
        assert_eq!(format("let s = \"open"), None);
        assert_eq!(format("/* never closed"), None);
        assert_eq!(format("f(a]"), None);
        assert_eq!(format("}"), None);
        assert_eq!(format("let s = `open"), None);
    }

    #[test]
    fn empty_input_formats_to_empty() {
        assert_eq!(fmt(""), "");
        assert_eq!(fmt("  \n\n "), "");
    }

    #[test]
    fn ranges_members_and_indexing() {
        assert_eq!(
            fmt("for i in 0 .. n { a [i] . go (); }"),
            "for i in 0..n {\n    a[i].go();\n}\n"
        );
        assert_eq!(fmt("let v = m?.x ?? 1;"), "let v = m?.x ?? 1;\n");
        assert_eq!(fmt("let a = [1, [2, 3]];"), "let a = [1, [2, 3]];\n");
        assert_eq!(fmt("let x = global::y;"), "let x = global::y;\n");
    }

    fn fixture(path: &str) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {path}: {e}"))
    }

    /// Real scripts: the bundled ones, and governors agents wrote in harness campaigns. Each
    /// formats, formats to itself again, keeps every comment and string token, and keeps the
    /// same tokens in the same order.
    #[test]
    fn real_scripts_format_idempotently_and_keep_every_token() {
        let dir = env!("CARGO_MANIFEST_DIR");
        let paths = [
            format!("{dir}/../agent/scripts/settle.rhai"),
            format!("{dir}/../agent/src/script/prelude.rhai"),
            format!("{dir}/tests/fixtures/governor-a.rhai"),
            format!("{dir}/tests/fixtures/governor-b.rhai"),
        ];
        for path in paths {
            let src = fixture(&path);
            let once = format(&src).unwrap_or_else(|| panic!("{path} should lex"));
            assert_eq!(once, fmt(&once), "{path}: not idempotent");
            let before: Vec<&str> = lex(&src).unwrap().iter().map(|t| t.text).collect();
            let after: Vec<&str> = lex(&once).unwrap().iter().map(|t| t.text).collect();
            assert_eq!(before, after, "{path}: tokens changed");
        }
    }
}

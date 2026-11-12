//! The pieces every text panel is made of, as data: the characters of the Seal frame and the
//! section marks ([`Marks`]), rows of toned text ([`Row`], [`Tone`]), meter bars and the fitting
//! and wrapping of text into a few columns. The side panel, the selection bar and the screens (log,
//! key help) all build their rows with these, so they read alike; the terminal frontend gives a
//! [`Tone`] its colour (`fn_tui::widgets`), a window frontend its own.

use crate::view::Status;

/// An sRGB colour, `[r, g, b]`.
pub type Rgb = [u8; 3];

/// The characters of the frame and the section marks, in Unicode and in the ASCII fallback.
pub struct Marks {
    pub tl: char,
    pub tr: char,
    pub bl: char,
    pub br: char,
    pub h: char,
    pub v: char,
    pub seal: &'static str,
    pub bullet: &'static str,
    pub run: &'static str,
    pub pause: &'static str,
    /// A section heading's left and right ends (docs/design/side-panel.md "Headings"): the
    /// frame's own `v` on both sides, replaced for that one row (`+` on both sides in ASCII,
    /// like the frame's corners).
    pub ls: char,
    pub rs: char,
    pub ellipsis: char,
    /// A filled and an empty cell of a meter bar.
    pub full: char,
    pub empty: char,
    /// Whether these are the ASCII marks (`--ascii`): the meter's glyphs follow it.
    pub ascii: bool,
    /// Between a name and a nation in a title.
    pub dot: &'static str,
    /// The research screen: a known and a locked tech, the cursor's pointer, and the arrows for
    /// cards or columns out of sight.
    pub known: &'static str,
    pub locked: &'static str,
    /// A school option closed by its mate (#729).
    pub closed: &'static str,
    pub pointer: &'static str,
    pub up: &'static str,
    pub down: &'static str,
    pub left: &'static str,
    pub right: &'static str,
    /// The embark screen's site box corners (#290): light, unlike the frame's own heavy
    /// corners, so the box reads as something drawn over the map, not another frame.
    pub box_tl: char,
    pub box_tr: char,
    pub box_bl: char,
    pub box_br: char,
}

impl Marks {
    pub fn new(ascii: bool) -> Self {
        if ascii {
            Self {
                tl: '+',
                tr: '+',
                bl: '+',
                br: '+',
                h: '-',
                v: '|',
                seal: "*",
                bullet: "o",
                run: ">",
                pause: "|",
                ls: '+',
                rs: '+',
                ellipsis: '~',
                full: crate::meter::ASCII_FULL,
                empty: crate::meter::ASCII_EMPTY,
                ascii: true,
                dot: "-",
                known: "+",
                locked: "-",
                closed: "x",
                pointer: ">",
                up: "^",
                down: "v",
                left: "<",
                right: ">",
                box_tl: '+',
                box_tr: '+',
                box_bl: '+',
                box_br: '+',
            }
        } else {
            Self {
                tl: '┏',
                tr: '┓',
                bl: '┗',
                br: '┛',
                h: '─',
                v: '│',
                seal: "◆",
                bullet: "◇",
                run: "▶",
                pause: "‖",
                ls: '├',
                rs: '┤',
                ellipsis: '…',
                full: crate::meter::FULL,
                empty: crate::meter::EMPTY,
                ascii: false,
                dot: "·",
                known: "✓",
                locked: "·",
                closed: "×",
                pointer: "▸",
                up: "▲",
                down: "▼",
                left: "◂",
                right: "▸",
                box_tl: '┌',
                box_tr: '┐',
                box_bl: '└',
                box_br: '┘',
            }
        }
    }
}

/// The colour a span of text wears: a theme token, so every nation reads the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Text,
    Dim,
    /// Text, bold.
    Strong,
    /// Section titles and the thing that is selected: `accent_text`, bold.
    Title,
    /// `accent_text`: chi, keys.
    Accent,
    Ok,
    Warn,
    Bad,
    /// `Ok` and `Warn`, bold: the clock's running or paused badge.
    Running,
    Paused,
    /// The Avatar in the Avatar State (#242, `docs/design/endgame.md`, "The Harmony Rite"):
    /// bold, on the nation's accent instead of the panel, so the selection bar marks the
    /// moment he holds the Harmony Rite apart from anything else he could be doing.
    Avatar,
    /// A section heading's title (docs/design/side-panel.md "Headings"): `accent_text`, bold,
    /// upper case, the same look as `Title` but its own variant, so a future change to one can't
    /// bleed into the other by accident.
    Heading,
    /// A section heading's rule, including the `├`/`┤` that replace the frame's own `│` for that
    /// row: `accent_soft`, like the frame's own lines.
    Rule,
    /// An explicit sRGB colour the caller chose (the Tool section's heading, in the active
    /// tool's own tint, `fn_tui::color::Tint`, which isn't a theme token): bold, on the panel like
    /// every other tone. The frontend turns it into whatever its colours are.
    Custom(Rgb),
}

impl Tone {
    /// The tone of a meter's status.
    pub fn of(status: Status) -> Self {
        match status {
            Status::Ok => Self::Ok,
            Status::Warn => Self::Warn,
            Status::Bad => Self::Bad,
        }
    }
}

/// One line of a panel: spans of text left to right, and optionally one flush right. The left
/// side gives way (with an ellipsis) when the line is too long.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Row {
    pub left: Vec<(String, Tone)>,
    pub right: Option<(String, Tone)>,
}

impl Row {
    pub fn text(text: impl Into<String>, tone: Tone) -> Self {
        Self { left: vec![(text.into(), tone)], right: None }
    }

    pub fn pair(left: impl Into<String>, lt: Tone, right: impl Into<String>, rt: Tone) -> Self {
        Self { left: vec![(left.into(), lt)], right: Some((right.into(), rt)) }
    }

    pub fn spans(spans: Vec<(String, Tone)>) -> Self {
        Self { left: spans, right: None }
    }

    /// The row's text with no styling, for tests.
    pub fn plain(&self) -> String {
        let mut s: String = self.left.iter().map(|(t, _)| t.as_str()).collect();
        if let Some((r, _)) = &self.right {
            s.push(' ');
            s.push_str(r);
        }
        s
    }
}

/// A section heading's row (docs/design/side-panel.md "Headings"): the frame's own side lines
/// become `├─ PEOPLE ──────────────── 20 ─┤` (`+- PEOPLE ---------------- 20 -+` in `--ascii`),
/// across `width` cells, border cells included. `title` is put upper case in `title_tone`
/// (`Tone::Heading`, except the Tool section's own tint); the rest of the rule, `├`/`┤`
/// included, is `Tone::Rule` (`accent_soft`, like the frame's own lines); `value` (empty for
/// none) sits in `Tone::Text` just before the closing rule.
pub fn heading_row(title: &str, value: &str, width: usize, marks: &Marks, title_tone: Tone) -> Row {
    let title = title.to_uppercase();
    // "─ TITLE " / " VALUE ─": the fixed parts around the title and the value, so what's left
    // of `width` (minus the two border cells) is the dash fill between them.
    let prefix_len = title.chars().count() + 3;
    let suffix_len = if value.is_empty() { 0 } else { value.chars().count() + 3 };
    let content_width = width.saturating_sub(2);
    let fill_len = content_width.saturating_sub(prefix_len).saturating_sub(suffix_len);
    let fill: String = std::iter::repeat_n(marks.h, fill_len).collect();
    let mut left = vec![
        (marks.ls.to_string(), Tone::Rule),
        (format!("{} ", marks.h), Tone::Rule),
        (title, title_tone),
        (format!(" {fill}"), Tone::Rule),
    ];
    if value.is_empty() {
        left.push((marks.rs.to_string(), Tone::Rule));
    } else {
        left.push((" ".to_owned(), Tone::Rule));
        left.push((value.to_owned(), Tone::Text));
        left.push((format!(" {}{}", marks.h, marks.rs), Tone::Rule));
    }
    Row::spans(left)
}

/// The first row that shows so that `at` does: a list starts at the top and slides only once the
/// cursor passes its far edge (the build picker, the roster, the research grid, the saves list).
pub fn window_start(at: usize, shown: usize) -> usize {
    (at + 1).saturating_sub(shown)
}

/// `text` broken at spaces into lines of at most `width` cells (a word longer than a line is
/// cut), and at most `max_lines` of them: if there is more, the last line ends in `ellipsis`.
pub fn wrap(text: &str, width: usize, max_lines: usize, ellipsis: char) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        while word.len() > width {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            lines.push(word.drain(..width).collect());
        }
        let word: String = word.into_iter().collect();
        if word.is_empty() {
            continue;
        }
        if line.is_empty() {
            line = word;
        } else if line.chars().count() + 1 + word.chars().count() <= width {
            line.push(' ');
            line.push_str(&word);
        } else {
            lines.push(std::mem::replace(&mut line, word));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            let kept: String = last.chars().take(width.saturating_sub(1)).collect();
            *last = format!("{}{ellipsis}", kept.trim_end());
        }
    }
    lines
}

/// `text` cut to `width` cells, with an ellipsis if it didn't fit.
pub fn fit(text: &str, width: usize, ellipsis: char) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(width.saturating_sub(1)).collect();
    out.push(ellipsis);
    out
}

/// The game's one clock (`crate::time`, "Spring Y2 · 15:00 left") as `clock` says it, with the
/// frame's own `dot` between its halves: the `--ascii` frame has no middle dot.
pub fn clock(clock: &str, marks: &Marks) -> String {
    clock.replace(" · ", &format!(" {} ", marks.dot))
}

/// The one meter (#654, `crate::meter`): `cells` cells, `frac` of them filled, then the one flag
/// column directly against them. In the `tone` it is given while it is fine; while `low` the
/// whole meter, the empty cells too, wears the tone of the warning (`tone`, then, is the status's
/// own: `Warn` or `Bad`) and the flag shows `!`. Every bar in the game is drawn by this: the
/// settler view and Endeavor at ten cells, the Nation roster at five.
pub fn meter(frac: f32, cells: usize, tone: Tone, low: bool, marks: &Marks) -> [(String, Tone); 3] {
    let (full, empty) = crate::meter::cells(frac, cells, marks.ascii);
    let flag = crate::meter::flag(low).to_string();
    let empty_tone = if low { tone } else { Tone::Dim };
    [(full, tone), (empty, empty_tone), (flag, tone)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_breaks_at_spaces_cuts_long_words_and_marks_what_is_left_out() {
        let lines = |t: &str, w, n| wrap(t, w, n, '…');
        assert_eq!(lines("one two three", 7, 5), ["one two", "three"]);
        assert_eq!(lines("", 7, 5), Vec::<String>::new());
        assert_eq!(lines("abcdefghij", 4, 5), ["abcd", "efgh", "ij"]);
        assert_eq!(lines("aa bb cc dd ee", 5, 2), ["aa bb", "cc d…"]);
        assert_eq!(lines("aa bb cc", 5, 1), ["aa b…"]);
        assert!(lines("anything", 0, 3).is_empty());
        for line in lines("designate harvest (chop, mine, gather); O: standing order", 28, 3) {
            assert!(line.chars().count() <= 28, "{line}");
        }
    }

    /// The browser's and the feed's callers pass `usize::MAX` for no limit (their old copies
    /// had none); a zero width is theirs to raise to one.
    #[test]
    fn wrap_without_a_line_limit_keeps_every_line_for_the_browser_and_the_feed() {
        let all = |t: &str, w| wrap(t, w, usize::MAX, ' ');
        assert_eq!(all("aaa bbb ccc", 7), ["aaa bbb", "ccc"]);
        assert_eq!(all("aaaaaaaaaa b", 4), ["aaaa", "aaaa", "aa b"]);
        assert_eq!(all("x", 1), ["x"]);
        assert!(all("", 10).is_empty());
        assert_eq!(all("a bb ccc dddd", 1), ["a", "b", "b", "c", "c", "c", "d", "d", "d", "d"]);
    }

    #[test]
    fn window_start_slides_only_once_the_cursor_passes_the_far_edge() {
        assert_eq!((0..3).map(|at| window_start(at, 3)).collect::<Vec<_>>(), [0, 0, 0]);
        assert_eq!((3..5).map(|at| window_start(at, 3)).collect::<Vec<_>>(), [1, 2]);
    }

    /// `heading_row`'s shape matches docs/design/side-panel.md "Headings" exactly, with and
    /// without a value, in both Unicode and `--ascii`.
    #[test]
    fn a_heading_row_is_a_divider_joined_to_the_frame() {
        let marks = Marks::new(false);
        let row = heading_row("people", "20", 32, &marks, Tone::Heading);
        assert_eq!(row.plain(), "├─ PEOPLE ──────────────── 20 ─┤");
        let row = heading_row("stores", "", 32, &marks, Tone::Heading);
        assert_eq!(row.plain(), "├─ STORES ─────────────────────┤");

        let ascii = Marks::new(true);
        let row = heading_row("people", "20", 32, &ascii, Tone::Heading);
        assert_eq!(row.plain(), "+- PEOPLE ---------------- 20 -+");
    }

    #[test]
    fn fit_cuts_with_an_ellipsis() {
        assert_eq!(fit("short", 10, '…'), "short");
        assert_eq!(fit("exactly ten", 11, '…'), "exactly ten");
        assert_eq!(fit("a rather long line", 8, '…'), "a rathe…");
        assert_eq!(fit("anything", 0, '~'), "~");
    }

    #[test]
    fn a_meter_is_cells_then_a_flag_and_a_low_one_wears_the_warning_throughout() {
        let m = Marks::new(true);
        let ok = meter(0.4, 5, Tone::Ok, false, &m);
        assert_eq!(ok.clone().map(|(t, _)| t), ["##".to_owned(), "...".to_owned(), " ".to_owned()]);
        assert_eq!((ok[0].1, ok[1].1, ok[2].1), (Tone::Ok, Tone::Dim, Tone::Ok));
        let low = meter(0.2, 5, Tone::Bad, true, &m);
        assert_eq!(
            low.clone().map(|(t, _)| t),
            ["#".to_owned(), "....".to_owned(), "!".to_owned()]
        );
        assert!(low.iter().all(|(_, t)| *t == Tone::Bad), "the empty cells too: {low:?}");
        let (full, empty) = (Marks::new(false).full, Marks::new(false).empty);
        assert_eq!((full, empty), ('█', '░'));
    }
}

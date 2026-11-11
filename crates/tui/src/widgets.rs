//! The pieces every panel is made of: the Seal frame, rows of coloured text, meter bars and the
//! fitting and wrapping of text into a few columns. The side panel, the selection bar and the
//! screens (log, key help) all draw with these, so they look alike.

use fn_ui::view::Status;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::color::Palette;

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
    /// Between a name and a nation in a title.
    pub dot: &'static str,
    /// The research screen: a known and a locked tech, the cursor's pointer, and the arrows for
    /// cards or columns out of sight.
    pub known: &'static str,
    pub locked: &'static str,
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
                full: '#',
                empty: '.',
                dot: "-",
                known: "+",
                locked: "-",
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
                full: '█',
                empty: '░',
                dot: "·",
                known: "✓",
                locked: "·",
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
    /// An explicit colour the caller already resolved (the Tool section's heading, in the
    /// active tool's own tint, `crate::color::Tint`, which isn't a theme token): bold, on the
    /// panel like every other tone.
    Custom(Color),
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

    pub fn style(self, palette: &Palette) -> Style {
        let t = &palette.theme;
        if self == Self::Avatar {
            // Its own background (the accent), not the panel's: every other tone sits on the
            // panel, so this is the one place a row's background is anything else.
            return Style::new()
                .fg(palette.token(t.text))
                .bg(palette.token(t.accent))
                .add_modifier(Modifier::BOLD);
        }
        if let Self::Custom(c) = self {
            return Style::new().fg(c).bg(palette.token(t.panel)).add_modifier(Modifier::BOLD);
        }
        let (rgb, bold) = match self {
            Self::Text => (t.text, false),
            Self::Dim => (t.dim, false),
            Self::Strong => (t.text, true),
            Self::Title => (t.accent_text, true),
            Self::Accent => (t.accent_text, false),
            Self::Ok => (t.ok, false),
            Self::Warn => (t.warn, false),
            Self::Bad => (t.bad, false),
            Self::Running => (t.ok, true),
            Self::Paused => (t.warn, true),
            Self::Heading => (t.accent_text, true),
            Self::Rule => (t.accent_soft, false),
            Self::Avatar | Self::Custom(_) => unreachable!("handled above"),
        };
        let style = Style::new().fg(palette.token(rgb)).bg(palette.token(t.panel));
        if bold { style.add_modifier(Modifier::BOLD) } else { style }
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

/// A meter as `width` cells: `frac` of them filled (rounded), as (filled, empty) strings, in
/// whatever `full`/`empty` glyphs the caller wants (usually `Marks::full`/`empty`, but the
/// Nation screen's roster uses its own, #325: a meter that fills a whole cell's height, stacked
/// row after row with the same status, fuses into one solid stripe instead of reading as one bar
/// per settler).
pub fn meter(frac: f32, width: usize, full: char, empty: char) -> (String, String) {
    let filled = ((frac.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    (
        std::iter::repeat_n(full, filled).collect(),
        std::iter::repeat_n(empty, width - filled).collect(),
    )
}

/// Where a frame's title sits on its top edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleAt {
    Centre,
    Left,
}

/// The Seal frame: the area filled with the panel colour, `accent_soft` box lines, `accent`
/// corners, `title` on the top edge and (if any) `footer` on the bottom edge.
pub fn draw_frame(
    buf: &mut Buffer,
    area: Rect,
    palette: &Palette,
    title: &str,
    at: TitleAt,
    footer: &str,
) {
    let marks = Marks::new(palette.ascii);
    let theme = &palette.theme;
    let panel_bg = palette.token(theme.panel);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            // Reset, not just restyle: the map below may have left reverse video or bold on the
            // cell (the cursor with no colour), and a patch would keep it.
            let cell = &mut buf[(x, y)];
            cell.reset();
            cell.set_char(' ').set_style(Style::new().bg(panel_bg));
        }
    }
    let line = Style::new().fg(palette.token(theme.accent_soft)).bg(panel_bg);
    let corner = Style::new().fg(palette.token(theme.accent)).bg(panel_bg);
    let (l, r, t, b) = (area.left(), area.right() - 1, area.top(), area.bottom() - 1);
    for x in l + 1..r {
        buf[(x, t)].set_char(marks.h).set_style(line);
        buf[(x, b)].set_char(marks.h).set_style(line);
    }
    for y in t + 1..b {
        buf[(l, y)].set_char(marks.v).set_style(line);
        buf[(r, y)].set_char(marks.v).set_style(line);
    }
    for (x, y, c) in [(l, t, marks.tl), (r, t, marks.tr), (l, b, marks.bl), (r, b, marks.br)] {
        buf[(x, y)].set_char(c).set_style(corner);
    }
    let room = usize::from(area.width).saturating_sub(4);
    let title = fit(title, room, marks.ellipsis);
    let len = title.chars().count() as u16;
    let x = match at {
        TitleAt::Centre => l + (area.width - len) / 2,
        TitleAt::Left => l + 2,
    };
    buf.set_string(x, t, &title, Tone::Title.style(palette));
    if !footer.is_empty() {
        let footer = fit(&format!(" {footer} "), room, marks.ellipsis);
        buf.set_string(l + 2, b, &footer, Tone::Dim.style(palette));
    }
}

/// Writes `text` on the bottom edge of the frame at `area`, flush right with one line cell
/// between it and the corner (`┗──── 12-48 of 61 ─┛`): where a scrolling screen says how far
/// down it is (#651). The bottom edge never carries key hints; those are the hint bar's alone.
/// Nothing is written without room for it.
pub fn draw_position(buf: &mut Buffer, area: Rect, palette: &Palette, text: &str) {
    let text = format!(" {text} ");
    let len = text.chars().count() as u16;
    if text.trim().is_empty() || area.width < len + 6 {
        return;
    }
    buf.set_string(area.right() - 2 - len, area.bottom() - 1, &text, Tone::Dim.style(palette));
}

/// Writes rows down an area, one per line, and stops at the bottom.
pub struct Painter<'a> {
    pub buf: &'a mut Buffer,
    pub inner: Rect,
    pub row: u16,
    pub marks: Marks,
    pub palette: &'a Palette,
}

impl<'a> Painter<'a> {
    pub fn new(buf: &'a mut Buffer, inner: Rect, palette: &'a Palette) -> Self {
        Self { buf, inner, row: 0, marks: Marks::new(palette.ascii), palette }
    }

    pub fn full(&self) -> bool {
        self.row >= self.inner.height
    }

    pub fn row(&mut self, row: &Row) {
        self.write_row(self.inner.x, self.inner.width, row);
    }

    /// [`Self::row`], but across `x..x+width` instead of `self.inner`'s own column and width: a
    /// section heading ([`Self::heading`]) spans the panel's full width, border cells included,
    /// not just `inner`.
    fn write_row(&mut self, x: u16, width: u16, row: &Row) {
        if self.full() {
            return;
        }
        let width = usize::from(width);
        let y = self.inner.y + self.row;
        let right_len = row.right.as_ref().map_or(0, |(t, _)| t.chars().count());
        let mut remaining = width.saturating_sub(right_len + usize::from(right_len > 0));
        let mut cx = x;
        for (text, tone) in &row.left {
            if remaining == 0 {
                break;
            }
            let shown = fit(text, remaining, self.marks.ellipsis);
            let n = shown.chars().count();
            self.buf.set_stringn(cx, y, &shown, n, tone.style(self.palette));
            cx += n as u16;
            remaining -= n;
        }
        if let Some((text, tone)) = &row.right {
            // A value wider than the row is cut to fit rather than drawn starting left of `x`,
            // over whatever sits there (a modal's border, say).
            let shown = fit(text, width, self.marks.ellipsis);
            let n = shown.chars().count();
            let rx = x + (width - n) as u16;
            self.buf.set_stringn(rx, y, &shown, n, tone.style(self.palette));
        }
        self.row += 1;
    }

    /// A section heading ([`heading_row`]), across `area`'s full width (border cells included),
    /// at the current row.
    pub fn heading(&mut self, area: Rect, title: &str, value: &str, title_tone: Tone) {
        let row = heading_row(title, value, usize::from(area.width), &self.marks, title_tone);
        self.write_row(area.x, area.width, &row);
    }

    pub fn blank(&mut self) {
        self.row += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A right-hand value wider than its row is cut to the row, never drawn left of it.
    #[test]
    fn a_value_wider_than_its_row_stays_inside_it() {
        let s = crate::new_session(7, fn_sim::Nation::Earth);
        let palette = Palette::new(
            &s.defs,
            fn_sim::Nation::Earth,
            crate::color::ColorDepth::TrueColor,
            false,
        );
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let mut p = Painter::new(&mut buf, Rect::new(5, 0, 10, 1), &palette);
        p.row(&Row::pair("Name", Tone::Text, "10 Stone, 6 Wood", Tone::Text));
        let line = crate::frame::to_text(&buf);
        assert!(line.starts_with("     "), "drew left of the row: {line:?}");
        assert_eq!(line.trim_end().chars().count(), 15, "{line:?}");
    }

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
    fn a_meter_fills_cells_in_proportion() {
        assert_eq!(meter(1.0, 10, '#', '.'), ("##########".into(), String::new()));
        assert_eq!(meter(0.0, 10, '#', '.'), (String::new(), "..........".into()));
        assert_eq!(meter(0.23, 10, '#', '.'), ("##".into(), "........".into()));
        assert_eq!(meter(0.72, 10, '#', '.').0.len(), 7);
        assert_eq!(meter(7.0, 10, '#', '.').0.len(), 10, "out of range clamps");
    }
}

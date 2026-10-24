//! The pieces every panel is made of: the Seal frame, rows of coloured text, meter bars and the
//! fitting and wrapping of text into a few columns. The side panel, the selection bar and the
//! screens (log, key help) all draw with these, so they look alike.

use fn_ui::view::Status;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

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

/// A meter as `width` cells: `frac` of them filled (rounded), as (filled, empty) strings.
pub fn meter(frac: f32, width: usize, marks: &Marks) -> (String, String) {
    let filled = ((frac.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    (
        std::iter::repeat_n(marks.full, filled).collect(),
        std::iter::repeat_n(marks.empty, width - filled).collect(),
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
        if self.full() {
            return;
        }
        let width = usize::from(self.inner.width);
        let y = self.inner.y + self.row;
        let right_len = row.right.as_ref().map_or(0, |(t, _)| t.chars().count());
        let mut remaining = width.saturating_sub(right_len + usize::from(right_len > 0));
        let mut x = self.inner.x;
        for (text, tone) in &row.left {
            if remaining == 0 {
                break;
            }
            let shown = fit(text, remaining, self.marks.ellipsis);
            let n = shown.chars().count();
            self.buf.set_stringn(x, y, &shown, n, tone.style(self.palette));
            x += n as u16;
            remaining -= n;
        }
        if let Some((text, tone)) = &row.right {
            let x = self.inner.right() - right_len as u16;
            self.buf.set_stringn(x, y, text, right_len, tone.style(self.palette));
        }
        self.row += 1;
    }

    pub fn blank(&mut self) {
        self.row += 1;
    }
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
        for line in lines("designate harvest (chop, mine, gather); hold Shift to repeat", 28, 3) {
            assert!(line.chars().count() <= 28, "{line}");
        }
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
        let m = Marks::new(true);
        assert_eq!(meter(1.0, 10, &m), ("##########".into(), String::new()));
        assert_eq!(meter(0.0, 10, &m), (String::new(), "..........".into()));
        assert_eq!(meter(0.23, 10, &m), ("##".into(), "........".into()));
        assert_eq!(meter(0.72, 10, &m).0.len(), 7);
        assert_eq!(meter(7.0, 10, &m).0.len(), 10, "out of range clamps");
    }
}

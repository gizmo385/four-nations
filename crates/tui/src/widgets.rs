//! The pieces every panel is made of: the Seal frame, rows of coloured text, meter bars and the
//! fitting and wrapping of text into a few columns. The side panel, the selection bar and the
//! screens (log, key help) all draw with these, so they look alike.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::Palette;

// The text half (rows, tones, marks, fitting) is `fn_ui`'s (#857); this module draws it.
pub use fn_ui::rows::{Marks, Row, Tone, clock, fit, heading_row, meter, window_start, wrap};

/// How a [`Tone`] looks on the terminal: its colours and weight, at this palette.
pub trait ToneStyle {
    fn style(self, palette: &Palette) -> Style;
}

impl ToneStyle for Tone {
    fn style(self, palette: &Palette) -> Style {
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
            return Style::new()
                .fg(palette.color(c))
                .bg(palette.token(t.panel))
                .add_modifier(Modifier::BOLD);
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

/// What a [`draw_hint_bar`] says after the mode's badge.
pub enum BarBody<'a> {
    /// The mode's own keys (`fn_ui::keys::hints`), as many whole items as fit.
    Keys,
    /// A result notice in place of the keys, cut with an ellipsis if it is too long.
    Notice(&'a str),
}

/// The hint bar (docs/design/tui.md, "Hints") on the one row `area`, for the screens that have
/// no `App` to draw it: the badge with `mode`'s name, then `mode`'s keys or a notice, then, with
/// `tail`, the fixed `? keys` on the right. The keys come from the `fn_ui::keys` table, and
/// whole items drop off the end when they don't fit. `warn` gives the badge the warning colour.
pub fn draw_hint_bar(
    buf: &mut Buffer,
    area: Rect,
    palette: &Palette,
    mode: fn_ui::keys::Mode,
    body: BarBody,
    tail: bool,
    warn: bool,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let theme = &palette.theme;
    let y = area.y;
    let panel = Style::new().fg(palette.token(theme.dim)).bg(palette.token(theme.panel));
    let accent = Style::new().fg(palette.token(theme.accent)).bg(palette.token(theme.panel));
    for x in area.left()..area.right() {
        buf[(x, y)].reset();
        buf[(x, y)].set_char(' ').set_style(panel);
    }
    let badge = if palette.mono() {
        Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        let bg = if warn { theme.warn } else { theme.accent };
        Style::new()
            .fg(palette.token(theme.on_accent))
            .bg(palette.token(bg))
            .add_modifier(Modifier::BOLD)
    };
    let marks = Marks::new(palette.ascii);
    let width = |s: &str| s.chars().count() as u16;
    let name = format!(" {} ", mode.name());
    buf.set_string(area.x, y, &name, badge);

    // The tail goes first so the keys know how much room it leaves; a notice takes its place.
    let tail_item = fn_ui::keys::tail();
    let mut right = 0;
    if tail && matches!(body, BarBody::Keys) {
        let text = tail_item.text();
        right = width(&text) + 1;
        let x = area.right().saturating_sub(right);
        buf.set_string(x, y, &text, panel);
        buf.set_string(x, y, &tail_item.keys, accent);
    }
    let start = area.x + width(&name) + 1;
    let room = usize::from(area.right().saturating_sub(start + right + 1));
    match body {
        BarBody::Notice(text) => buf.set_string(start, y, fit(text, room, marks.ellipsis), panel),
        BarBody::Keys => {
            let items = fn_ui::keys::hints(mode);
            let sep = format!(" {} ", marks.dot);
            let n = fn_ui::keys::fit(&items, room, sep.chars().count());
            let mut x = start;
            for (i, item) in items[..n].iter().enumerate() {
                if i > 0 {
                    buf.set_string(x, y, &sep, panel);
                    x += width(&sep);
                }
                buf.set_string(x, y, &item.keys, accent);
                x += width(&item.keys);
                let verb = format!(" {}", item.verb);
                buf.set_string(x, y, &verb, panel);
                x += width(&verb);
            }
        }
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
}

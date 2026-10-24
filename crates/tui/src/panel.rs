//! The Seal side panel (docs/design/ui.md, docs/design/tui.md): a framed column with the nation's
//! name on the top edge, then sections. The MVP has the header (seed, clock, speed), People (the
//! count) and Cursor (what is under the keyboard cursor). Stores, Research, Tool and Alerts come
//! with the panels issue (#99); `fn_ui::view::SideView` already carries what they show.

use fn_ui::Session;
use fn_ui::alerts::Alerts;
use fn_ui::inspect::{Hint, Line, tooltip_lines};
use fn_ui::text::describe_stack;
use fn_ui::view::{SideView, side_view};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::color::Palette;
use crate::state::UiState;

/// The characters of the frame and the section marks, in Unicode and in the ASCII fallback.
struct Marks {
    tl: char,
    tr: char,
    bl: char,
    br: char,
    h: char,
    v: char,
    seal: &'static str,
    bullet: &'static str,
    run: &'static str,
    pause: &'static str,
    ellipsis: char,
}

impl Marks {
    fn new(ascii: bool) -> Self {
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
            }
        }
    }
}

/// How an action hint reads in the terminal, if it can be taken yet. The select and tool keys
/// arrive with the selection (#99) and the designation tools (#98); until then a hint would
/// promise a key that does nothing, so there is none.
pub fn hint_text(hint: Hint) -> Option<String> {
    match hint {
        Hint::SelectUnit | Hint::StartTool(_) | Hint::ApplyTool(_) => None,
    }
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

/// Writes lines down the inside of the frame.
struct Painter<'a> {
    buf: &'a mut Buffer,
    inner: Rect,
    row: u16,
    marks: Marks,
    palette: &'a Palette,
}

impl Painter<'_> {
    fn style(&self, fg: [f32; 3]) -> Style {
        Style::new().fg(self.palette.token(fg)).bg(self.palette.token(self.palette.theme.panel))
    }

    fn full(&self) -> bool {
        self.row >= self.inner.height
    }

    /// One line: `left`, and `right` flush against the right edge.
    fn line(&mut self, left: &str, left_style: Style, right: &str, right_style: Style) {
        if self.full() {
            return;
        }
        let width = usize::from(self.inner.width);
        let right_len = right.chars().count();
        let left = fit(
            left,
            width.saturating_sub(right_len + usize::from(right_len > 0)),
            self.marks.ellipsis,
        );
        let y = self.inner.y + self.row;
        self.buf.set_stringn(self.inner.x, y, &left, width, left_style);
        if right_len > 0 {
            let x = self.inner.right() - right_len as u16;
            self.buf.set_stringn(x, y, right, right_len, right_style);
        }
        self.row += 1;
    }

    fn text(&mut self, text: &str, style: Style) {
        self.line(text, style, "", style);
    }

    fn blank(&mut self) {
        self.row += 1;
    }

    /// `◇ TITLE` in the accent, with `right` flush right in the text colour.
    fn section(&mut self, title: &str, right: &str) {
        let head = format!("{} {title}", self.marks.bullet);
        let accent = self.style(self.palette.theme.accent_text).add_modifier(Modifier::BOLD);
        let text = self.style(self.palette.theme.text);
        self.line(&head, accent, right, text);
    }
}

/// Draws the panel into `area`: the frame, then the sections.
pub fn draw_panel(
    session: &Session,
    ui: &UiState,
    palette: &Palette,
    area: Rect,
    buf: &mut Buffer,
) {
    let view = side_view(session, ui.tool, "", None, &Alerts::default());
    let marks = Marks::new(palette.ascii);
    let theme = &palette.theme;
    let panel_bg = palette.token(theme.panel);

    // Fill, then the frame.
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buf[(x, y)].set_char(' ').set_style(Style::new().bg(panel_bg));
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
    let title = format!(" {0} {1} {0} ", marks.seal, view.nation.to_uppercase());
    let title = fit(&title, usize::from(area.width) - 4, marks.ellipsis);
    let x = l + (area.width - title.chars().count() as u16) / 2;
    buf.set_string(
        x,
        t,
        &title,
        Style::new().fg(palette.token(theme.accent_text)).bg(panel_bg).add_modifier(Modifier::BOLD),
    );

    let inner = Rect::new(l + 2, t + 1, area.width - 4, area.height - 2);
    let mut p = Painter { buf, inner, row: 0, marks, palette };
    header(&mut p, &view);
    p.blank();
    p.section("PEOPLE", &view.people.to_string());
    p.blank();
    cursor_section(&mut p, session, ui);
}

fn header(p: &mut Painter, view: &SideView) {
    let (text, dim) = (p.style(p.palette.theme.text), p.style(p.palette.theme.dim));
    p.line(&format!("Seed {}", view.seed), dim, &view.clock, text);
    let (icon, word, color) = if view.paused {
        (p.marks.pause, "Paused", p.palette.theme.warn)
    } else {
        (p.marks.run, "Running", p.palette.theme.ok)
    };
    let state = p.style(color).add_modifier(Modifier::BOLD);
    p.line(&format!("{icon} {word} x{}", view.speed), state, "Space  + -", dim);
}

/// The Cursor section: the tile's tooltip, and anything lying on it.
fn cursor_section(p: &mut Painter, session: &Session, ui: &UiState) {
    let pos = ui.cursor;
    p.section("CURSOR", &format!("({}, {})", pos.x, pos.y));
    let (title, text) =
        (p.style(p.palette.theme.text).add_modifier(Modifier::BOLD), p.style(p.palette.theme.text));
    let mut lines = tooltip_lines(session, ui.tool, pos).unwrap_or_default();
    // The tooltip names the top thing on the tile; the section lists what else is there.
    if let Some((_, stack)) = session.world.ground_items().find(|&(q, _)| q == pos) {
        lines.push(Line::Detail(format!(
            "On the ground: {}",
            describe_stack(&session.defs, Some(stack))
        )));
    }
    if let Some(stack) = session.world.stored_at(pos) {
        lines.push(Line::Detail(format!("Stored: {}", describe_stack(&session.defs, Some(stack)))));
    }
    if session.world.stockpile_zones().any(|q| q == pos) {
        lines.push(Line::Detail("Stockpile".into()));
    }
    for line in lines {
        match line {
            Line::Title(s) => p.text(&s, title),
            Line::Detail(s) => p.text(&s, text),
            Line::Hint(h) => {
                if let Some(s) = hint_text(h) {
                    p.text(&s, p.style(p.palette.theme.dim));
                }
            }
        }
    }
}

/// The `Color` of the panel background, for tests.
pub fn panel_bg(palette: &Palette) -> Color {
    palette.token(palette.theme.panel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_cuts_with_an_ellipsis() {
        assert_eq!(fit("short", 10, '…'), "short");
        assert_eq!(fit("exactly ten", 11, '…'), "exactly ten");
        assert_eq!(fit("a rather long line", 8, '…'), "a rathe…");
        assert_eq!(fit("anything", 0, '~'), "~");
    }
}

//! One frame as text: what `--frame` prints and what the golden frames hold.

use fn_ui::Session;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use crate::app::App;
use crate::color::Palette;
use crate::draw::draw;

/// The frame for `app` on a `width` x `height` terminal.
pub fn render(
    session: &Session,
    ui: &crate::state::UiState,
    palette: &Palette,
    size: (u16, u16),
) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, size.0, size.1));
    draw(session, ui, palette, &mut buf);
    buf
}

/// The frame for `app`, with the view sized for `size` first.
pub fn render_app(app: &mut App, palette: &Palette, size: (u16, u16)) -> Buffer {
    app.ui.resize(size.0, size.1);
    render(&app.session, &app.ui, palette, size)
}

/// The glyphs only: one row per line, trailing spaces trimmed. This is the golden-frame format.
pub fn to_text(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in buf.area.top()..buf.area.bottom() {
        let mut row = String::new();
        for x in buf.area.left()..buf.area.right() {
            row.push_str(buf[(x, y)].symbol());
        }
        out.push_str(row.trim_end());
        out.push('\n');
    }
    out
}

/// The frame with colour escapes, for a human to `cat` (`--ansi`).
pub fn to_ansi(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in buf.area.top()..buf.area.bottom() {
        // Each row starts from the terminal's defaults and ends back at them.
        let mut current = (Color::Reset, Color::Reset, Modifier::empty());
        for x in buf.area.left()..buf.area.right() {
            let cell = &buf[(x, y)];
            let style = (cell.fg, cell.bg, cell.modifier);
            if style != current {
                out.push_str(&sgr(style));
                current = style;
            }
            out.push_str(cell.symbol());
        }
        out.push_str("\x1b[0m\n");
    }
    out
}

fn sgr((fg, bg, modifiers): (Color, Color, Modifier)) -> String {
    let mut codes = vec!["0".to_owned()];
    for (modifier, code) in [
        (Modifier::BOLD, "1"),
        (Modifier::DIM, "2"),
        (Modifier::UNDERLINED, "4"),
        (Modifier::REVERSED, "7"),
    ] {
        if modifiers.contains(modifier) {
            codes.push(code.to_owned());
        }
    }
    codes.push(color_code(fg, 30));
    codes.push(color_code(bg, 40));
    format!("\x1b[{}m", codes.join(";"))
}

/// The SGR parameter for `color`; `base` is 30 for the foreground, 40 for the background.
fn color_code(color: Color, base: u8) -> String {
    let named = |n: u8, bright: bool| (if bright { base + 60 } else { base } + n).to_string();
    match color {
        Color::Reset => (base + 9).to_string(),
        Color::Black => named(0, false),
        Color::Red => named(1, false),
        Color::Green => named(2, false),
        Color::Yellow => named(3, false),
        Color::Blue => named(4, false),
        Color::Magenta => named(5, false),
        Color::Cyan => named(6, false),
        Color::Gray => named(7, false),
        Color::DarkGray => named(0, true),
        Color::LightRed => named(1, true),
        Color::LightGreen => named(2, true),
        Color::LightYellow => named(3, true),
        Color::LightBlue => named(4, true),
        Color::LightMagenta => named(5, true),
        Color::LightCyan => named(6, true),
        Color::White => named(7, true),
        Color::Indexed(i) => format!("{};5;{i}", base + 8),
        Color::Rgb(r, g, b) => format!("{};2;{r};{g};{b}", base + 8),
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Style;

    use super::*;

    #[test]
    fn text_trims_trailing_spaces_and_ends_each_row_with_a_newline() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 2));
        buf.set_string(1, 0, "ab", Style::new());
        assert_eq!(to_text(&buf), " ab\n\n");
    }

    #[test]
    fn ansi_sets_a_style_only_when_it_changes_and_resets_each_row() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
        buf.set_string(
            0,
            0,
            "ab",
            Style::new().fg(Color::Rgb(1, 2, 3)).add_modifier(Modifier::BOLD),
        );
        let ansi = to_ansi(&buf);
        assert_eq!(ansi, "\x1b[0;1;38;2;1;2;3;49mab\x1b[0;39;49m \x1b[0m\n");
    }

    #[test]
    fn color_codes() {
        assert_eq!(color_code(Color::Red, 30), "31");
        assert_eq!(color_code(Color::LightRed, 40), "101");
        assert_eq!(color_code(Color::Indexed(196), 40), "48;5;196");
        assert_eq!(color_code(Color::Reset, 30), "39");
    }
}

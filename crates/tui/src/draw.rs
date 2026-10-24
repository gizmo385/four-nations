//! The whole frame as a pure function into a ratatui `Buffer`: no terminal, no clock, so golden
//! frames, `--frame` and the interactive loop all draw through here.

use fn_ui::Session;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use crate::color::Palette;
use crate::layout::{self, MIN_HEIGHT, MIN_WIDTH};
use crate::map::draw_map;
use crate::panel::{draw_panel, fit};
use crate::state::{Prompt, UiState};

/// What the frame says below the minimum size, and nothing else.
pub fn too_small_text() -> String {
    format!("Terminal too small ({MIN_WIDTH}x{MIN_HEIGHT} needed)")
}

/// Draws the whole frame into `buf`, which covers the terminal.
pub fn draw(session: &Session, ui: &UiState, palette: &Palette, buf: &mut Buffer) {
    let area = buf.area;
    let Some(regions) = layout::regions(area) else {
        buf.set_stringn(area.x, area.y, too_small_text(), usize::from(area.width), Style::new());
        return;
    };
    draw_map(session, ui, palette, regions.map, buf);
    draw_panel(session, ui, palette, regions.panel, buf);
    draw_status(session, ui, palette, regions.status, buf);
}

/// Where the terminal's own cursor goes: on the cursor tile, if it's in view.
pub fn terminal_cursor(ui: &UiState, area: Rect) -> Option<Position> {
    let regions = layout::regions(area)?;
    let (x, y) = ui.cursor_cell()?;
    Some(Position::new(regions.map.x + x as u16, regions.map.y + y as u16))
}

/// The keys the status line teaches. The designation tools (#98) and screens (#99, #100) add
/// theirs when they land.
const KEYS: &str =
    "arrows/wasd move  Shift x10  Ctrl/PgUp/PgDn jump  Space pause  +/- speed  . step  q quit";

fn draw_status(session: &Session, ui: &UiState, palette: &Palette, area: Rect, buf: &mut Buffer) {
    let theme = &palette.theme;
    let panel = Style::new().fg(palette.token(theme.dim)).bg(palette.token(theme.panel));
    for x in area.left()..area.right() {
        buf[(x, area.y)].set_char(' ').set_style(panel);
    }
    let ellipsis = if palette.ascii { '~' } else { '…' };
    let badge = |fg, bg| {
        if palette.mono() {
            Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else {
            Style::new().fg(palette.token(fg)).bg(palette.token(bg)).add_modifier(Modifier::BOLD)
        }
    };

    let mut right = 0;
    if session.clock.paused {
        let text = " PAUSED ";
        right = text.len() as u16;
        buf.set_string(area.right() - right, area.y, text, badge(theme.on_accent, theme.warn));
    }
    let (badge_len, hints) = match ui.prompt {
        Some(Prompt::Quit) => {
            let text = " QUIT? ";
            buf.set_string(area.x, area.y, text, badge(theme.on_accent, theme.warn));
            (text.len() as u16, "Press y to quit, any other key to stay.")
        }
        None => {
            let text = " SELECT ";
            buf.set_string(area.x, area.y, text, badge(theme.on_accent, theme.accent));
            (text.len() as u16, KEYS)
        }
    };
    let room = usize::from(area.width.saturating_sub(badge_len + right + 2));
    buf.set_string(area.x + badge_len + 1, area.y, fit(hints, room, ellipsis), panel);
}

//! The whole frame as a pure function into a ratatui `Buffer`: no terminal, no clock, so golden
//! frames, `--frame` and the interactive loop all draw through here.

use fn_ui::keys::{self, Mode};
use fn_ui::{Session, Tool};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use crate::banner::{self, banner_area};
use crate::color::Palette;
use crate::layout::{self, MIN_HEIGHT, MIN_WIDTH};
use crate::map::draw_map;
use crate::menu::MenuPage;
use crate::panel::draw_panel;
use crate::screens::draw_screen;
use crate::state::{Modal, Screen, UiState};
use crate::widgets::{Marks, fit};

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
    // A full screen (`crate::page`, #652) covers the map and the side panel both, so neither is
    // drawn under it; the tool banner hides behind it too.
    let full = if let Some(screen) = ui.screen {
        draw_screen(session, ui, screen, palette, regions.page, buf);
        true
    } else if ui.modal == Some(Modal::Build) {
        let rows = fn_ui::build::picker_rows(session);
        crate::build::draw(session, &rows, ui.build_cursor, palette, regions.page, buf);
        true
    } else {
        false
    };
    if !full {
        draw_map(session, ui, palette, regions.map, buf);
        if let Some(modal) = ui.modal {
            // A confirm box asked from the menu covers it: drawn alone, over the map.
            if ui.prompt.is_none() {
                draw_modal(session, ui, modal, palette, regions.map, buf);
            }
        } else {
            // The tool banner takes the map's top row; the menu hides it (docs/design/ui.md).
            // A selected settler is the side panel's inspector (#653): the map keeps its
            // full height.
            if ui.tool != Tool::Select {
                banner::draw(session, ui, palette, banner_area(regions.map), buf);
            }
        }
    }
    // A question that can lose progress goes over whatever is open (#655), centred on the full
    // screen it was asked from or else on the map.
    if let Some(ask) = crate::ask::current(session, ui) {
        let over = if full { regions.page } else { regions.map };
        crate::ask::draw(&ask, ui.ask_cursor, palette, over, buf);
    }
    if !full {
        draw_panel(session, ui, palette, regions.panel, buf);
    }
    draw_status(session, ui, palette, regions.status, buf);
}

/// Draws the centred box that's open (the game menu, `crate::menu`) into `area`, the map region.
/// The Build picker is a full screen instead (`crate::build`, #652), drawn before this is asked.
fn draw_modal(
    session: &Session,
    ui: &UiState,
    modal: Modal,
    palette: &Palette,
    area: Rect,
    buf: &mut Buffer,
) {
    match modal {
        Modal::Build => {}
        Modal::Menu => {
            // The game's one clock (#654), the same words as the side panel's.
            let clock = crate::widgets::clock(
                &fn_ui::time::clock_text(session),
                &Marks::new(palette.ascii),
            );
            let nations: Vec<(String, bool)> = fn_sim::Nation::ALL
                .iter()
                .map(|&n| (session.defs.nation(n).name.clone(), n == session.world.nation))
                .collect();
            crate::menu::draw(ui.menu_page, ui.menu_cursor, &nations, &clock, palette, area, buf)
        }
    }
}

/// Where the terminal's own cursor goes: on the cursor tile, if it's in view.
pub fn terminal_cursor(ui: &UiState, area: Rect) -> Option<Position> {
    if ui.screen.is_some() || ui.modal.is_some() || ui.prompt.is_some() {
        return None;
    }
    let regions = layout::regions(area)?;
    let (x, y) = ui.cursor_cell()?;
    Some(Position::new(regions.map.x + x as u16, regions.map.y + y as u16))
}

/// Which mode's hints the status line shows (`fn_ui::keys::Mode`): the menu or the Build picker
/// if one is open, else the open screen, else the map: Select (with or without a settler's bar)
/// or a tool (with or without a corner set).
pub fn hint_mode(session: &Session, ui: &UiState) -> Mode {
    if ui.prompt.is_some() {
        return Mode::Ask;
    }
    if let Some(modal) = ui.modal {
        return match modal {
            Modal::Build => Mode::Build,
            Modal::Menu if ui.menu_page == MenuPage::Nations => Mode::MenuNations,
            Modal::Menu => Mode::Menu,
        };
    }
    if let Some(screen) = ui.screen {
        if screen == Screen::Saves && ui.save_name.is_some() {
            return Mode::SaveName;
        }
        return match screen {
            Screen::Log if ui.spectating => Mode::WatchLog,
            Screen::Log => Mode::Log,
            Screen::Help => Mode::Help,
            Screen::Endeavor => Mode::Endeavor,
            Screen::Research => Mode::Research,
            Screen::Saves => Mode::Saves,
            Screen::Nation => Mode::Nation,
        };
    }
    if ui.tool_list.is_some() {
        return Mode::Tools;
    }
    if ui.tool == Tool::Select {
        return if ui.selected_unit(session).is_some() { Mode::Selected } else { Mode::Select };
    }
    if ui.rect_size().is_some() { Mode::ToolCorner } else { Mode::Tool }
}

/// The hint bar's badge text for `mode`: the mode's name, and in a tool "HARVEST · REPEATING"
/// while the standing-order toggle is on (#277) or "CANCEL · TEARDOWN" while the Cancel tool's
/// own is (#335).
fn mode_label(ui: &UiState, mode: Mode) -> String {
    match ui.repeating_label() {
        Some(label) if matches!(mode, Mode::Tool | Mode::ToolCorner) => label.to_uppercase(),
        _ => keys::mode_name(mode, ui.tool),
    }
}

/// The status line (docs/design/tui.md, "Hints"): always the same three parts. The mode's name
/// on the left; that mode's own keys (`fn_ui::keys::hints`), as many whole items as fit; and a
/// fixed tail on the right, `PAUSED` while paused and then `? keys`. A result notice
/// takes the place of the keys while it lasts (and the tail's `? keys` with it, for room).
fn draw_status(session: &Session, ui: &UiState, palette: &Palette, area: Rect, buf: &mut Buffer) {
    let theme = &palette.theme;
    let panel = Style::new().fg(palette.token(theme.dim)).bg(palette.token(theme.panel));
    for x in area.left()..area.right() {
        buf[(x, area.y)].set_char(' ').set_style(panel);
    }
    let marks = Marks::new(palette.ascii);
    let ellipsis = marks.ellipsis;
    let badge = |fg, bg| {
        if palette.mono() {
            Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else {
            Style::new().fg(palette.token(fg)).bg(palette.token(bg)).add_modifier(Modifier::BOLD)
        }
    };
    let width = |s: &str| s.chars().count() as u16;

    // What the line says besides the keys: a result notice, or nothing.
    let message: Option<(&str, Style, String)> = (!ui.status.is_empty()).then(|| {
        let text = ui.status_badge.unwrap_or(" SAVE ");
        let refused = ui.status.contains("failed") || ui.status.starts_with("Can't");
        let bg = if refused { theme.warn } else { theme.accent };
        (text, badge(theme.on_accent, bg), ui.status.clone())
    });

    // The right edge: `PAUSED`, then `? keys` (not while a message has the line).
    let tail = keys::tail().text();
    let tail_w = if message.is_some() { 0 } else { width(&tail) + 1 };
    let mut right = tail_w;
    if session.clock.paused {
        let text = " PAUSED ";
        right += width(text);
        buf.set_string(
            area.right().saturating_sub(right),
            area.y,
            text,
            badge(theme.on_accent, theme.warn),
        );
        right += 1;
    }
    if message.is_none() {
        let x = area.right().saturating_sub(tail_w);
        buf.set_string(x, area.y, &tail, panel);
        let (keys_part, _) = tail.split_once(' ').unwrap_or((&tail, ""));
        let accent = Style::new().fg(palette.token(theme.accent)).bg(palette.token(theme.panel));
        buf.set_string(x, area.y, keys_part, accent);
    }

    let (name, style, body) = match message {
        Some((text, style, body)) => (text.to_owned(), style, Some(body)),
        None => {
            let mode = hint_mode(session, ui);
            let style = match mode {
                Mode::Tool | Mode::ToolCorner => match banner::tint(ui) {
                    // The same tint as the banner above the map (#307), so the two badges
                    // match; a standing order's own greener tint included.
                    Some(tint) => palette.banner_style(tint),
                    None => badge(theme.on_accent, theme.accent),
                },
                _ => badge(theme.on_accent, theme.accent),
            };
            (format!(" {} ", mode_label(ui, mode)), style, None)
        }
    };
    buf.set_string(area.x, area.y, &name, style);
    let start = area.x + width(&name) + 1;
    let room = usize::from(area.right().saturating_sub(start + right + 1));
    match body {
        Some(text) => buf.set_string(start, area.y, fit(&text, room, ellipsis), panel),
        None => {
            let items = keys::hints_for(hint_mode(session, ui), ui.tool);
            let sep = format!(" {} ", marks.dot);
            let n = keys::fit(&items, room, sep.chars().count());
            let accent =
                Style::new().fg(palette.token(theme.accent)).bg(palette.token(theme.panel));
            let mut x = start;
            for (i, item) in items[..n].iter().enumerate() {
                if i > 0 {
                    buf.set_string(x, area.y, &sep, panel);
                    x += width(&sep);
                }
                buf.set_string(x, area.y, &item.keys, accent);
                x += width(&item.keys);
                let verb = format!(" {}", item.verb);
                buf.set_string(x, area.y, &verb, panel);
                x += width(&verb);
            }
        }
    }
}

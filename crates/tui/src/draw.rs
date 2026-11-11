//! The whole frame as a pure function into a ratatui `Buffer`: no terminal, no clock, so golden
//! frames, `--frame` and the interactive loop all draw through here.

use fn_ui::keys::{self, Mode};
use fn_ui::{Session, Tool};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use crate::banner::{self, banner_area};
use crate::bar::{bar_area, draw_bar};
use crate::color::Palette;
use crate::layout::{self, MIN_HEIGHT, MIN_WIDTH};
use crate::map::draw_map;
use crate::panel::draw_panel;
use crate::screens::draw_screen;
use crate::state::{Modal, Prompt, Screen, UiState};
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
    draw_map(session, ui, palette, regions.map, buf);
    // A screen or a modal covers the map, and the tool banner and the selection bar hide behind
    // either (docs/design/ui.md); the banner (top row) and the bar (bottom rows) can otherwise
    // both show at once, a tool active with a unit still selected.
    if let Some(screen) = ui.screen {
        draw_screen(session, ui, screen, palette, regions.map, buf);
    } else if let Some(modal) = ui.modal {
        draw_modal(session, ui, modal, palette, regions.map, buf);
    } else {
        if ui.tool != Tool::Select {
            banner::draw(session, ui, palette, banner_area(regions.map), buf);
        }
        if ui.selected_unit(session).is_some() {
            draw_bar(session, ui, palette, bar_area(regions.map), buf);
        }
    }
    draw_panel(session, ui, palette, regions.panel, buf);
    draw_status(session, ui, palette, regions.status, buf);
}

/// Draws the modal that's open (just the Build picker so far, #285, `crate::build`) into `area`,
/// the map region.
fn draw_modal(
    session: &Session,
    ui: &UiState,
    modal: Modal,
    palette: &Palette,
    area: Rect,
    buf: &mut Buffer,
) {
    match modal {
        Modal::Build => {
            let rows = fn_ui::build::picker_rows(session);
            crate::build::draw(session, &rows, ui.build_cursor, palette, area, buf);
        }
        Modal::Menu => {
            // The elapsed clock, while seasons are on (#416, owner decision 4 on #414): the
            // side panel's header shows the season instead, so the menu is one of the two
            // places the clock moved to.
            let clock = session
                .world
                .knobs(&session.defs)
                .seasons
                .then(|| fn_ui::view::format_clock(session.world.tick));
            crate::menu::draw(
                ui.menu_cursor,
                ui.menu_name.as_deref(),
                ui.has_quicksave,
                clock.as_deref(),
                palette,
                area,
                buf,
            )
        }
    }
}

/// Where the terminal's own cursor goes: on the cursor tile, if it's in view.
pub fn terminal_cursor(ui: &UiState, area: Rect) -> Option<Position> {
    if ui.screen.is_some() || ui.modal.is_some() {
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
    if let Some(modal) = ui.modal {
        return match modal {
            Modal::Build => Mode::Build,
            Modal::Menu if ui.menu_name.is_some() => Mode::SaveAs,
            Modal::Menu => Mode::Menu,
        };
    }
    if let Some(screen) = ui.screen {
        return match screen {
            Screen::Log => Mode::Log,
            Screen::Help => Mode::Help,
            Screen::Endeavor => Mode::Endeavor,
            Screen::Research => Mode::Research,
            Screen::Saves => Mode::Saves,
            Screen::Nation => Mode::Nation,
        };
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
/// fixed tail on the right, `PAUSED` while paused and then `? keys`. A prompt or a result notice
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

    // What the line says besides the keys: a prompt or a notice, or nothing.
    let message: Option<(&str, Style, String)> = match &ui.prompt {
        Some(prompt) => {
            let (text, question) = match prompt {
                Prompt::Quit => (" QUIT? ", "quit".to_owned()),
                Prompt::NewMap => (
                    " NEW MAP? ",
                    format!("start a new map (seed {})", session.world.seed.wrapping_add(1)),
                ),
                Prompt::Nation(n) => {
                    (" SWITCH? ", format!("start over as the {}", session.defs.nation(*n).name))
                }
                Prompt::QuickLoad => {
                    (" LOAD? ", "load the quicksave in place of this game".to_owned())
                }
                Prompt::LoadSave(name) => (" LOAD? ", format!("load {name} in place of this game")),
                Prompt::SaveOver(name) => (" SAVE? ", format!("save over the {name} slot")),
            };
            let style = badge(theme.on_accent, theme.warn);
            Some((text, style, format!("Press y to {question}, any other key to stay.")))
        }
        None if !ui.status.is_empty() => {
            let text = ui.status_badge.unwrap_or(" SAVE ");
            let refused = ui.status.contains("failed") || ui.status.starts_with("Can't");
            let bg = if refused { theme.warn } else { theme.accent };
            Some((text, badge(theme.on_accent, bg), ui.status.clone()))
        }
        None => None,
    };

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
            let items = keys::hints(hint_mode(session, ui));
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

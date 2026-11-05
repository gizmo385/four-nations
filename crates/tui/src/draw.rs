//! The whole frame as a pure function into a ratatui `Buffer`: no terminal, no clock, so golden
//! frames, `--frame` and the interactive loop all draw through here.

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
use crate::widgets::fit;

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
        Modal::Menu => crate::menu::draw(ui.menu_cursor, palette, area, buf),
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

/// The keys the status line teaches in Select mode, and (after `Esc` closes it) with a unit
/// selected, and in a screen. The key help (`?`) has the whole table.
const KEYS: &str = "arrows/wasd move  Enter select  Esc menu  Tab units  H X P B U tools  \
                    Space pause  T research  L log  ? keys  q quit";
const KEYS_SELECTED: &str = "Enter next here  Tab/Shift+Tab units  F follow  Esc close  \
                             H X P B U tools  Space pause  L log  ? keys";
const KEYS_SCREEN: &str =
    "arrows/W S scroll  PgUp/PgDn page  Esc close  L log  ? keys  Space pause";
const KEYS_ENDEAVOR: &str =
    "arrows/W S scroll  PgUp/PgDn page  Esc/G close  L log  ? keys  Space pause";
const KEYS_RESEARCH: &str = "arrows/WASD choose  Enter set target  Esc/T close  L log  ? keys";
const KEYS_SAVES: &str =
    "arrows/W S choose  Enter load  Shift+S save over  N new  Esc/F6 close  ? keys";
const KEYS_NATION: &str =
    "arrows/W S choose  PgUp/PgDn page  Enter select  Esc/N close  ? keys  Space pause";
const KEYS_BUILD_MODAL: &str = "Arrows/W S choose  Enter build  Esc close";
const KEYS_MENU: &str = "Arrows/W S choose  Enter select  Esc resume";

/// The status line in a tool: what `Enter` and `Esc` do now, "Harvest · repeating" while the
/// standing-order toggle is on (#277), then what the tool does (for `B`, the nation's ability;
/// for `U`, the building it would place).
fn tool_status(session: &Session, ui: &UiState) -> String {
    let keys = match ui.rect_size() {
        None => "Enter set corner  Esc leave".to_owned(),
        Some((w, h)) => format!("Enter apply {w}x{h}  Esc drop corner"),
    };
    let hint = ui.tool_hint(session);
    match ui.repeating_label() {
        Some(label) => format!("{keys}  |  {label}  |  {hint}"),
        None => format!("{keys}  |  {hint}"),
    }
}

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
    let (badge_len, hints) = match &ui.prompt {
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
            buf.set_string(area.x, area.y, text, badge(theme.on_accent, theme.warn));
            (text.len() as u16, format!("Press y to {question}, any other key to stay."))
        }
        None if !ui.status.is_empty() => {
            let text = " SAVE ";
            let bg = if ui.status.contains("failed") { theme.warn } else { theme.accent };
            buf.set_string(area.x, area.y, text, badge(theme.on_accent, bg));
            (text.len() as u16, ui.status.clone())
        }
        None if ui.screen.is_some() => {
            let (text, keys) = match ui.screen {
                Some(Screen::Help) => (" KEYS ", KEYS_SCREEN),
                Some(Screen::Endeavor) => (" ENDEAVOR ", KEYS_ENDEAVOR),
                Some(Screen::Research) => (" RESEARCH ", KEYS_RESEARCH),
                Some(Screen::Saves) => (" SAVES ", KEYS_SAVES),
                Some(Screen::Nation) => (" NATION ", KEYS_NATION),
                _ => (" LOG ", KEYS_SCREEN),
            };
            buf.set_string(area.x, area.y, text, badge(theme.on_accent, theme.accent));
            (text.len() as u16, keys.to_owned())
        }
        None if ui.modal.is_some() => {
            let (text, keys) = match ui.modal {
                Some(Modal::Menu) => (" MENU ", KEYS_MENU),
                _ => (" BUILD ", KEYS_BUILD_MODAL),
            };
            buf.set_string(area.x, area.y, text, badge(theme.on_accent, theme.accent));
            (text.len() as u16, keys.to_owned())
        }
        None if ui.tool == Tool::Select => {
            let text = " SELECT ";
            buf.set_string(area.x, area.y, text, badge(theme.on_accent, theme.accent));
            let keys = if ui.selected_unit(session).is_some() { KEYS_SELECTED } else { KEYS };
            (text.len() as u16, keys.to_owned())
        }
        None => {
            let text = format!(" {} ", ui.tool.name().to_uppercase());
            // The same tint as the banner above the map (#307), so the two badges match; a
            // standing order's own greener tint included.
            let style = match banner::tint(ui) {
                Some(tint) => palette.banner_style(tint),
                None => badge(theme.on_accent, theme.accent),
            };
            buf.set_string(area.x, area.y, &text, style);
            (text.len() as u16, tool_status(session, ui))
        }
    };
    let room = usize::from(area.width.saturating_sub(badge_len + right + 2));
    buf.set_string(area.x + badge_len + 1, area.y, fit(&hints, room, ellipsis), panel);
}

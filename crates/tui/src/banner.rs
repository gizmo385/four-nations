//! The active tool banner (#307, docs/design/side-panel.md "The active tool"): a solid band
//! across the map region's top row while any tool other than `Select` is active, naming the
//! tool, what `Enter` does now and what `Esc` does now, so the active tool is obvious at a
//! glance and never relies on colour alone. With no tool there is no banner, and `Select` keeps
//! the plain status-line badge (`crate::draw::draw_status`) instead of one of these tints.
//!
//! ```text
//! ━ HARVEST ━ chop, mine, gather  ·  Enter: set a corner  ·  Esc: leave ━━━━━━━━━━━━━━━━━━
//! ━ BUILD Bed ━ Enter: build 1x1  ·  U: pick another  ·  Esc: leave ━━━━━━━━━━━━━━━━━━━━━━
//! ```

use fn_ui::bend;
use fn_ui::{Session, Tool};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::color::{Palette, Tint};
use crate::panel::rect_hint;
use crate::state::{BANNER_HEIGHT, UiState};
use crate::widgets::fit;

/// The banner's area: the map region's top row (#307), as [`crate::bar::bar_area`] is its
/// bottom rows.
pub fn banner_area(map: Rect) -> Rect {
    let height = (BANNER_HEIGHT as u16).min(map.height);
    Rect::new(map.x, map.y, map.width, height)
}

/// The tint the banner wears: `Tint::for_tool`, except a standing-order harvest wears the
/// repeating tint instead (the same swap the map's own designation tint makes, #277). The
/// status line's own badge wears the same tint (`crate::draw::draw_status`), so the two match.
pub fn tint(ui: &UiState) -> Option<Tint> {
    if ui.tool == Tool::Harvest && ui.repeating {
        return Some(Tint::RepeatingHarvest);
    }
    Tint::for_tool(ui.tool)
}

/// The badge: the tool's name, the Build tool's chosen building, the Bend ability it channels
/// or the Clear tool's mode, `· REPEATING` while the standing-order toggle (`O`, #277) is on,
/// and `· TEARDOWN` while the Cancel tool's own teardown toggle (`O`, #335) is on.
fn badge(session: &Session, ui: &UiState) -> String {
    let mut s = ui.tool.name().to_uppercase();
    match ui.tool {
        Tool::Build => {
            if let Some(id) = ui.ghost_building(session) {
                s.push(' ');
                s.push_str(&session.defs.building(id).name);
            }
        }
        Tool::Bend => {
            if let Some(row) = bend::view(session).current() {
                s.push(' ');
                s.push_str(&row.name);
            }
        }
        // Ground mode only (docs/design/clearing.md, #411/#426); `E` would cycle to a buildings
        // mode once #342 lands, which would name it the same way.
        Tool::Clear => s.push_str(" ground"),
        _ => {}
    }
    if ui.tool == Tool::Harvest && ui.repeating {
        s.push_str(" · REPEATING");
    }
    if ui.tool == Tool::CancelHarvest && ui.teardown {
        s.push_str(" · TEARDOWN");
    }
    s
}

/// What `Enter` does next: the shared [`rect_hint`], except the Build tool with no corner set
/// yet says what a plain click would place ("Enter: build 1x1"), the chosen building's own
/// footprint, since the ghost already shows exactly that.
fn enter_hint(session: &Session, ui: &UiState) -> Option<String> {
    if ui.tool == Tool::Build && ui.anchor.is_none() {
        let building = ui.ghost_building(session)?;
        let (w, h) = session.defs.building(building).footprint;
        return Some(format!("Enter: build {w}x{h}"));
    }
    rect_hint(ui)
}

/// The content between the badge and the fill: a short brief or a tool-specific control hint
/// (whichever applies), then what `Enter` does, then what `Esc` does. The brief and the control
/// hint only show before a corner is set: mid-drag, `Enter` and `Esc` say enough on their own.
fn segments(session: &Session, ui: &UiState) -> Vec<String> {
    let anchored = ui.anchor.is_some();
    let toggled = (ui.tool == Tool::Harvest && ui.repeating)
        || (ui.tool == Tool::CancelHarvest && ui.teardown);
    let mut segs = Vec::new();
    if !anchored && !toggled {
        let brief = ui.tool.brief();
        if !brief.is_empty() {
            segs.push(brief.to_owned());
        }
    }
    if let Some(hint) = enter_hint(session, ui) {
        segs.push(hint);
    }
    if !anchored {
        match ui.tool {
            Tool::Harvest if ui.repeating => segs.push("O: one-shot".to_owned()),
            Tool::CancelHarvest if ui.teardown => segs.push("O: jobs only".to_owned()),
            Tool::Build => segs.push("U: pick another".to_owned()),
            Tool::Bend if bend::view(session).offers_next() => {
                segs.push("B: next ability".to_owned());
            }
            _ => {}
        }
    }
    segs.push(if anchored { "Esc: drop the corner" } else { "Esc: leave" }.to_owned());
    segs
}

/// Draws the banner into `area`'s top row: `━ BADGE ━ segments, joined  ·  like this ━━━ fill`,
/// `=` and `-` in place of `━` and `·` under `--ascii`. `None` outside a tool (`crate::draw`
/// doesn't call this then, but it's cheap to guard here too).
pub fn draw(session: &Session, ui: &UiState, palette: &Palette, area: Rect, buf: &mut Buffer) {
    let Some(tint) = tint(ui) else { return };
    let style = palette.banner_style(tint);
    let fill = if palette.ascii { '=' } else { '━' };
    let dot = if palette.ascii { " - " } else { "  ·  " };

    let row = area.y;
    for x in area.left()..area.right() {
        buf[(x, row)].set_char(fill).set_style(style);
    }

    let content = segments(session, ui).join(dot);
    let text = format!("{fill} {} {fill} {content} ", badge(session, ui));
    let ellipsis = if palette.ascii { '~' } else { '…' };
    let width = usize::from(area.width);
    let shown = fit(&text, width, ellipsis);
    buf.set_stringn(area.x, row, &shown, width, style);
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};

    use super::*;
    use crate::color::ColorDepth;
    use crate::frame::to_text;
    use crate::new_session;

    fn setup(nation: Nation) -> (Session, UiState) {
        let session = new_session(7, nation);
        let ui = UiState::new(&session, (120, 40));
        (session, ui)
    }

    fn line(session: &Session, ui: &UiState, ascii: bool) -> String {
        let palette =
            Palette::new(&session.defs, session.world.nation, ColorDepth::TrueColor, ascii);
        let mut buf = Buffer::empty(Rect::new(0, 0, 90, 1));
        draw(session, ui, &palette, Rect::new(0, 0, 90, 1), &mut buf);
        to_text(&buf).trim_end().to_owned()
    }

    #[test]
    fn no_tool_draws_nothing() {
        let (session, ui) = setup(Nation::Earth);
        let palette = Palette::new(&session.defs, Nation::Earth, ColorDepth::TrueColor, false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 90, 1));
        draw(&session, &ui, &palette, Rect::new(0, 0, 90, 1), &mut buf);
        assert_eq!(to_text(&buf).trim_end(), " ".repeat(90).trim_end());
    }

    #[test]
    fn harvest_names_itself_and_what_enter_and_esc_do() {
        let (session, mut ui) = setup(Nation::Earth);
        ui.set_tool(Tool::Harvest);
        let text = line(&session, &ui, false);
        assert_eq!(
            text,
            "━ HARVEST ━ chop, mine, gather  ·  Enter: set a corner  ·  Esc: leave \
             ━━━━━━━━━━━━━━━━━━━━"
        );
    }

    /// Once a corner is set the brief gives way to the rectangle size, and Esc only drops it.
    #[test]
    fn an_anchored_rectangle_drops_the_brief_for_its_own_size() {
        let (session, mut ui) = setup(Nation::Earth);
        ui.set_tool(Tool::Harvest);
        ui.anchor = Some(ui.cursor);
        ui.move_cursor(4, 2);
        let text = line(&session, &ui, false);
        assert!(text.contains("Enter: designate 5x3"), "{text}");
        assert!(text.contains("Esc: drop the corner"), "{text}");
        assert!(!text.contains("chop"), "the brief gives way:\n{text}");
    }

    #[test]
    fn a_standing_order_names_itself_and_how_to_turn_it_off() {
        let (session, mut ui) = setup(Nation::Earth);
        ui.set_tool(Tool::Harvest);
        ui.repeating = true;
        let text = line(&session, &ui, false);
        assert!(text.starts_with("━ HARVEST · REPEATING ━"), "{text}");
        assert!(text.contains("O: one-shot"), "{text}");
        assert!(!text.contains("chop"), "the brief gives way to the control hint:\n{text}");
    }

    #[test]
    fn build_names_the_chosen_building_and_its_footprint() {
        let (session, mut ui) = setup(Nation::Earth);
        let bed = session.defs.building_id("bed").unwrap();
        ui.choice = Some(bed);
        ui.set_tool(Tool::Build);
        let text = line(&session, &ui, false);
        assert!(text.starts_with("━ BUILD Bed ━"), "{text}");
        assert!(text.contains("Enter: build 1x1"), "{text}");
        assert!(text.contains("U: pick another"), "{text}");
    }

    #[test]
    fn bend_names_the_ability_it_channels() {
        let (session, mut ui) = setup(Nation::Earth);
        ui.set_tool(Tool::Bend);
        let text = line(&session, &ui, false);
        assert!(text.starts_with("━ BEND Raise Earth ━"), "{text}");
    }

    #[test]
    fn ascii_fills_with_equals_and_separates_with_a_dash() {
        let (session, mut ui) = setup(Nation::Earth);
        ui.set_tool(Tool::Harvest);
        let text = line(&session, &ui, true);
        assert!(text.is_ascii(), "{text}");
        assert!(text.starts_with("= HARVEST ="), "{text}");
        assert!(text.contains(" - "), "{text}");
        assert!(text.ends_with('='), "{text}");
    }

    /// With no colour the banner still reads: the same text as with colour (`--ascii`'s own
    /// fallback, checked above, is the shape half of that; this is the `--color never` half),
    /// and every cell is reverse video and bold, not relying on colour alone.
    #[test]
    fn no_colour_is_reverse_video_and_bold_with_the_same_text() {
        let (session, mut ui) = setup(Nation::Earth);
        ui.set_tool(Tool::Harvest);
        let palette = Palette::new(&session.defs, Nation::Earth, ColorDepth::Never, false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 90, 1));
        draw(&session, &ui, &palette, Rect::new(0, 0, 90, 1), &mut buf);
        let text = to_text(&buf).trim_end().to_owned();
        assert_eq!(text, line(&session, &ui, false), "the same words with or without colour");
        for x in 0..90 {
            let cell = &buf[(x, 0)];
            assert_eq!(cell.style().bg, Some(Color::Reset), "cell {x}");
            assert!(
                cell.style().add_modifier.contains(Modifier::REVERSED | Modifier::BOLD),
                "cell {x}: {:?}",
                cell.style()
            );
        }
    }
}

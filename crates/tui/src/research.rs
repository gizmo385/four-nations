//! The research screen (`T`, docs/design/research.md): the tech tree as columns by depth, one card
//! per tech. What it shows is `fn_ui::research::screen_view` (the visibility rule, which techs a
//! nation sees, is the sim's `Defs::techs_for`). A card has the name, the state, the cost and a
//! summary of what it unlocks. `Enter` issues `Command::SetResearch`; `fn_sim` queues the whole
//! prereq chain of a locked tech, and a queued prereq is drawn the same as the target.
//!
//! The cursor is a (column, row) pair in the view. The window onto the grid (which columns and
//! which cards show) follows from the cursor and the size alone, so a frame is a pure function of
//! the state like every other.

use fn_sim::world::Command;
use fn_ui::Session;
use fn_ui::research::{ScreenView, TechCard, TechState, screen_view};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::color::Palette;
use crate::page::{self, Page};
use crate::widgets::{Marks, Painter, Row, Tone, window_start, wrap};

/// The narrowest a column of cards can be: wide enough that the line saying what a tech gives
/// wraps into two rows at most, instead of being cut (#659). It puts four tiers on a 120 column
/// terminal and three on an 80 column one; a terminal too narrow for every tier scrolls sideways
/// with the cursor.
pub const MIN_COLUMN: usize = 24;
/// Columns are one blank cell apart.
const GAP: usize = 1;
/// A card is at least three rows (name, state and cost, what it gives) and a blank one between
/// cards. The name and the last line wrap inside the column, so a card grows by a row when its
/// text needs it: this is the usual height, which `Grid::cards` and the page keys count in.
const CARD_ROWS: usize = 3;
const CARD_STRIDE: usize = CARD_ROWS + 1;
/// A card's lines under its name start this far in, under the name itself (the cursor's pointer,
/// the state's mark and a space come first).
const INDENT: usize = 3;
/// The frame's detail strip under the grid (`crate::page`, #652): the chosen tech's name,
/// description and unlocks. The rule above it takes one more row.
pub const DETAIL_ROWS: u16 = 3;

/// Where the cursor is: a column of the view and a card in it.
pub type Cursor = (usize, usize);

/// How the grid fits an area of `(width, height)` text cells (inside the frame and its padding).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    /// How many columns show at once, and how wide each is.
    pub columns: usize,
    pub column_width: usize,
    /// How many cards of a column show at once, counting cards of the usual height; a taller
    /// one takes the room of more.
    pub cards: usize,
}

impl Grid {
    /// The grid for a page whose inside (`crate::page::text_size`: the body, the rule and the
    /// detail strip together) is `text`, with `columns_in_view` columns of cards to show.
    pub fn new(text: (usize, usize), columns_in_view: usize) -> Self {
        let (width, height) = text;
        let columns = ((width + GAP) / (MIN_COLUMN + GAP)).clamp(1, columns_in_view.max(1));
        let column_width = (width.saturating_sub(GAP * (columns - 1)) / columns).max(1);
        // One row is the column headers; the details sit under the grid, below a rule.
        let grid_height = height.saturating_sub(1 + 1 + usize::from(DETAIL_ROWS));
        let cards = ((grid_height + 1) / CARD_STRIDE).max(1);
        Self { columns, column_width, cards }
    }
}

/// The card under the cursor, if the view has any.
pub fn card_at(view: &ScreenView, at: Cursor) -> Option<&TechCard> {
    view.columns.get(at.0)?.get(at.1)
}

/// Where the cursor starts when the screen opens: on the target (the last tech of the queue), or
/// with none on the first tech that can be researched now, or on the first card.
pub fn initial_cursor(session: &Session, view: &ScreenView) -> Cursor {
    let find = |want: &dyn Fn(&TechCard) -> bool| {
        view.columns
            .iter()
            .enumerate()
            .find_map(|(c, col)| col.iter().position(want).map(|r| (c, r)))
    };
    let target = session.world.research().queue().last().copied();
    target
        .and_then(|id| find(&|card| card.id == id))
        .or_else(|| find(&|card| card.state == TechState::Available))
        .unwrap_or((0, 0))
}

/// The cursor after a step of `(dx, dy)` cards: it stops at the edges, and a step sideways keeps
/// the row where the next column has that many cards.
pub fn step(view: &ScreenView, at: Cursor, dx: i32, dy: i32) -> Cursor {
    let last = view.columns.len().saturating_sub(1);
    let column = (at.0 as i64 + i64::from(dx)).clamp(0, last as i64) as usize;
    let len = view.columns.get(column).map_or(0, Vec::len);
    let row = (at.1 as i64 + i64::from(dy)).clamp(0, len.saturating_sub(1) as i64) as usize;
    (column, row)
}

/// `Enter`: makes the tech under the cursor the research target. A tech already known is refused
/// by the sim and nothing changes; one with unmet prereqs queues its whole chain.
pub fn choose(session: &mut Session, at: Cursor) {
    let view = screen_view(session);
    if let Some(card) = card_at(&view, at) {
        let _ = session.apply(Command::SetResearch { tech: card.id });
    }
}

fn state_tone(state: TechState) -> Tone {
    match state {
        TechState::Known => Tone::Dim,
        TechState::Queued => Tone::Title,
        TechState::Available => Tone::Text,
        TechState::Locked => Tone::Dim,
    }
}

/// The word a card carries for its state: short, so it fits beside the cost.
pub fn state_word(state: TechState) -> &'static str {
    match state {
        TechState::Known => "Known",
        TechState::Queued => "Queued",
        TechState::Available => "Available",
        TechState::Locked => "Locked",
    }
}

/// The mark before a card's name, so the state reads without colour.
fn state_mark(state: TechState, marks: &Marks) -> &'static str {
    match state {
        TechState::Known => marks.known,
        TechState::Queued => marks.seal,
        TechState::Available => marks.bullet,
        TechState::Locked => marks.locked,
    }
}

/// The word a card carries for its state: `state_word`, except that the tech the player chose
/// reads "Target" where the prereqs queued ahead of it read "Queued" (#659).
pub fn card_word(card: &TechCard) -> &'static str {
    if card.target { "Target" } else { state_word(card.state) }
}

/// What a card says on its last lines: what it unlocks once it's within reach ("Leads to
/// Writing" for a tech that only opens others), or, while `Locked`, what it still needs by name
/// (#338): a locked card alone used to say only "Locked", leaving which tier-1 tech unlocks it
/// invisible until the player researched each candidate and rechecked.
fn gives_text(card: &TechCard) -> String {
    if card.state == TechState::Locked {
        format!("Needs {}", card.needs)
    } else {
        card.unlock.clone()
    }
}

/// `text`, a list ("Earthworks x1.2, Earth Tent"), broken after its commas into lines of at most
/// `width` cells, so an item (a building's name, say) is never split across two lines. An item
/// wider than a line on its own is broken at its spaces.
fn wrap_items(text: &str, width: usize, ellipsis: char) -> Vec<String> {
    let items: Vec<&str> = text.split(", ").collect();
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for (i, item) in items.iter().enumerate() {
        let item = if i + 1 < items.len() { format!("{item},") } else { (*item).to_owned() };
        if line.is_empty() {
            line = item;
        } else if line.chars().count() + 1 + item.chars().count() <= width {
            line.push(' ');
            line.push_str(&item);
        } else {
            lines.push(std::mem::replace(&mut line, item));
        }
        if line.chars().count() > width {
            let mut pieces = wrap(&line, width, usize::MAX, ellipsis);
            line = pieces.pop().unwrap_or_default();
            lines.extend(pieces);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// A card as its rows, wrapped inside `width` cells and never cut: the name (the pointer then the
/// state's mark before it, `chosen` putting the cursor mark there), the state and cost on one
/// line ("Available - 60 pts"), then what it gives (`gives_text`), as many rows as it needs.
pub fn card_rows(card: &TechCard, chosen: bool, marks: &Marks, width: usize) -> Vec<Row> {
    let tone = state_tone(card.state);
    let name_tone = match card.state {
        TechState::Available => Tone::Strong,
        other => state_tone(other),
    };
    let mark_tone = if card.state == TechState::Known { Tone::Ok } else { tone };
    let pointer = if chosen { marks.pointer } else { " " };
    let room = width.saturating_sub(INDENT).max(1);
    let pad = " ".repeat(INDENT);
    let mut rows = Vec::new();
    for (i, line) in wrap(&card.name, room, usize::MAX, marks.ellipsis).into_iter().enumerate() {
        rows.push(if i == 0 {
            Row::spans(vec![
                (pointer.to_owned(), Tone::Title),
                (format!("{} ", state_mark(card.state, marks)), mark_tone),
                (line, name_tone),
            ])
        } else {
            Row::text(format!("{pad}{line}"), name_tone)
        });
    }
    rows.push(Row::spans(vec![
        (format!("{pad}{}", card_word(card)), tone),
        (format!(" {} {} pts", marks.dot, card.cost), Tone::Dim),
    ]));
    for line in wrap_items(&gives_text(card), room, marks.ellipsis) {
        rows.push(Row::text(format!("{pad}{line}"), Tone::Dim));
    }
    rows
}

/// The three rows under the grid about the tech under the cursor: its name (and the research
/// target, when that is another tech), state and cost; what it is; what it unlocks
/// (`card.unlock`, empty while `Locked`) and what it still needs (`card.needs`,
/// `fn_ui::research::TechCard`): a Queued tech can show both at once (its own deeper prereqs may
/// still be unknown), so the two are side by side rather than either/or.
pub fn detail_rows(session: &Session, card: &TechCard, marks: &Marks) -> [Row; 3] {
    let def = session.defs.tech(card.id);
    let unlocks = if card.unlock.is_empty() {
        String::new()
    } else if card.unlock.starts_with("Leads to ") {
        card.unlock.clone()
    } else {
        format!("Unlocks: {}", card.unlock)
    };
    let needs =
        if card.needs.is_empty() { String::new() } else { format!("Needs: {}", card.needs) };
    let sep = if unlocks.is_empty() || needs.is_empty() { "" } else { "   " };
    // The target is named in the strip whichever card the cursor is on: as the heading's state
    // when the cursor is on it, beside the name otherwise.
    let mut name = vec![(card.name.clone(), Tone::Title)];
    if !card.target
        && let Some(&target) = session.world.research().queue().last()
    {
        name.push((format!("   Target: {}", session.defs.tech(target).name), Tone::Dim));
    }
    [
        Row {
            left: name,
            right: Some((
                format!("{} {} {} pts", card_word(card), marks.dot, card.cost),
                state_tone(card.state),
            )),
        },
        Row::text(def.description.clone(), Tone::Text),
        Row::spans(vec![(unlocks, Tone::Dim), (format!("{sep}{needs}"), Tone::Warn)]),
    ]
}

/// Draws the screen's contents into `page` (the grid in its body, the chosen tech in its detail
/// strip) and returns where the visible tiers sit ("tiers 1-4 of 5") when some are out of sight,
/// for the frame's bottom edge (#651).
pub fn draw(
    session: &Session,
    at: Cursor,
    palette: &Palette,
    page: &Page,
    buf: &mut Buffer,
) -> Option<String> {
    let view = screen_view(session);
    let marks = Marks::new(palette.ascii);
    let inner = page.body;
    let grid = Grid::new(
        (usize::from(page.inner.width), usize::from(page.inner.height)),
        view.columns.len(),
    );
    let first_column = window_start(at.0, grid.columns);
    let grid_height = usize::from(inner.height.saturating_sub(1));

    // Every card of the columns in view, wrapped to the column: a grid row is as tall as its
    // tallest card, so the cards of a row line up across columns.
    let shown: Vec<(usize, &Vec<TechCard>)> =
        view.columns.iter().enumerate().skip(first_column).take(grid.columns).collect();
    let cards: Vec<Vec<Vec<Row>>> = shown
        .iter()
        .map(|&(c, column)| {
            column
                .iter()
                .enumerate()
                .map(|(r, card)| card_rows(card, c == at.0 && r == at.1, &marks, grid.column_width))
                .collect()
        })
        .collect();
    let height = |r: usize| cards.iter().filter_map(|col| col.get(r)).map(Vec::len).max();
    // The window onto the grid rows slides only once the cursor's row passes the bottom, and
    // shows whole rows, but always the cursor's.
    let mut first_card = at.1;
    let mut used = height(at.1).unwrap_or(0);
    while first_card > 0 {
        let above = height(first_card - 1).unwrap_or(0);
        if used + 1 + above > grid_height {
            break;
        }
        first_card -= 1;
        used += 1 + above;
    }
    let mut tops = Vec::new();
    let mut y = 0;
    let mut r = first_card;
    while let Some(h) = height(r) {
        if r > at.1 && y + h > grid_height {
            break;
        }
        tops.push((r, y));
        y += h + 1;
        r += 1;
    }
    let shown_rows = tops.len();

    for (slot, &(c, column)) in shown.iter().enumerate() {
        let x = inner.x + (slot * (grid.column_width + GAP)) as u16;
        let width = grid.column_width as u16;
        let chosen_column = c == at.0;

        // The column header: its depth, and arrows where cards are out of sight.
        let mut head = Painter::new(buf, Rect::new(x, inner.y, width, 1), palette);
        let flags = format!(
            "{}{}",
            if first_card > 0 { marks.up } else { "" },
            if column.len() > first_card + shown_rows { marks.down } else { "" },
        );
        let title_tone = if chosen_column { Tone::Title } else { Tone::Dim };
        head.row(&Row::pair(format!("TIER {}", c + 1), title_tone, flags, Tone::Dim));

        let body = Rect::new(x, inner.y + 1, width, grid_height as u16);
        for &(r, top) in &tops {
            let Some(rows) = cards[slot].get(r) else { continue };
            let area =
                Rect::new(x, body.y + top as u16, width, rows.len() as u16).intersection(body);
            let mut p = Painter::new(buf, area, palette);
            for row in rows {
                p.row(row);
            }
            // The card under the cursor wears the selection's quiet background over its rows.
            if chosen_column && r == at.1 {
                page::highlight(buf, area, palette);
            }
        }
    }

    // Arrows at the frame's edge when columns are out of sight.
    let mut arrow = |x: u16, text: &str| {
        buf.set_string(x, inner.y, text, Tone::Accent.style(palette));
    };
    if first_column > 0 {
        arrow(inner.x - 1, marks.left);
    }
    if first_column + grid.columns < view.columns.len() {
        arrow(inner.right(), marks.right);
    }

    // The details of the tech under the cursor, in the frame's detail strip.
    if let (Some(card), Some(strip)) = (card_at(&view, at), page.detail) {
        let rows = detail_rows(session, card, &marks);
        let mut p = Painter::new(buf, strip, palette);
        for row in &rows {
            p.row(row);
        }
    }
    fn_ui::text::window_label("tiers", first_column, grid.columns, view.columns.len())
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use fn_sim::research::TechId;

    use super::*;
    use crate::new_session;

    fn card(state: TechState, unlock: &str, needs: &str) -> TechCard {
        TechCard {
            id: TechId(0),
            name: "Masonry".into(),
            cost: 120,
            state,
            unlock: unlock.into(),
            needs: needs.into(),
            target: false,
        }
    }

    fn plain(rows: &[Row]) -> Vec<String> {
        rows.iter().map(Row::plain).collect()
    }

    #[test]
    fn an_available_card_shows_the_name_the_state_and_cost_on_one_line_and_the_unlocks() {
        let marks = Marks::new(false);
        let card = card(TechState::Available, "Construction x1.15", "");
        let text = plain(&card_rows(&card, false, &marks, 28));
        assert_eq!(text, [" ◇ Masonry", "   Available · 120 pts", "   Construction x1.15"]);
    }

    /// A Locked card shows what it still needs, by name, instead of what it would unlock (#338,
    /// #659): the evidence was a whole playtest session spent discovering that one tier-1 tech
    /// unlocks another, with nothing on the Locked card itself saying so.
    #[test]
    fn a_locked_card_shows_what_it_needs_instead_of_what_it_would_unlock() {
        let marks = Marks::new(false);
        let card = card(TechState::Locked, "Construction x1.15", "Carpentry");
        let text = plain(&card_rows(&card, false, &marks, 28));
        assert_eq!(text, [" · Masonry", "   Locked · 120 pts", "   Needs Carpentry"]);
    }

    /// The line saying what a tech gives wraps inside the column and the card grows by a row,
    /// instead of ending in an ellipsis (#659).
    #[test]
    fn a_long_unlock_line_wraps_and_the_card_grows_a_row() {
        let marks = Marks::new(false);
        let card = card(TechState::Available, "Construction x1.1, Workshop", "");
        let text = plain(&card_rows(&card, false, &marks, 24));
        assert_eq!(
            text,
            [" ◇ Masonry", "   Available · 120 pts", "   Construction x1.1,", "   Workshop"]
        );
        let mut locked = card;
        locked.state = TechState::Locked;
        locked.needs = "Earth Shaping, Tunneling, Seismic Sense".into();
        for line in plain(&card_rows(&locked, false, &marks, 24)) {
            assert!(line.chars().count() <= 24 && !line.contains('…'), "{line}");
        }
    }

    /// A list breaks after a comma, never inside an item.
    #[test]
    fn the_gives_line_breaks_between_items_not_inside_one() {
        assert_eq!(
            wrap_items("Earthworks x1.2, Earth Tent", 21, '…'),
            ["Earthworks x1.2,", "Earth Tent"]
        );
        assert_eq!(
            wrap_items("Needs Earth Shaping, Masonry", 21, '…'),
            ["Needs Earth Shaping,", "Masonry"]
        );
        assert_eq!(wrap_items("Farm Plot", 21, '…'), ["Farm Plot"]);
        // An item too long for a line is broken at its spaces, never cut.
        assert_eq!(
            wrap_items("Prefix Words, Stone Outcrop Boulder", 12, '…'),
            ["Prefix", "Words,", "Stone", "Outcrop", "Boulder"]
        );
    }

    #[test]
    fn the_target_reads_target_where_its_queued_prereqs_read_queued() {
        let marks = Marks::new(false);
        let mut queued = card(TechState::Queued, "x", "");
        assert_eq!(plain(&card_rows(&queued, false, &marks, 28))[1], "   Queued · 120 pts");
        queued.target = true;
        assert_eq!(plain(&card_rows(&queued, false, &marks, 28))[1], "   Target · 120 pts");
    }

    #[test]
    fn every_state_has_its_own_word_and_mark() {
        let marks = Marks::new(true);
        let states = [TechState::Known, TechState::Queued, TechState::Available, TechState::Locked];
        let words: Vec<_> = states.iter().map(|s| state_word(*s)).collect();
        assert_eq!(words, ["Known", "Queued", "Available", "Locked"]);
        let marks: Vec<_> = states.iter().map(|s| state_mark(*s, &marks)).collect();
        for (i, a) in marks.iter().enumerate() {
            assert!(!marks[i + 1..].contains(a), "{marks:?}");
        }
    }

    #[test]
    fn the_cursor_marks_the_chosen_card() {
        let marks = Marks::new(false);
        let rows = card_rows(&card(TechState::Available, "-", ""), true, &marks, 28);
        assert_eq!(rows[0].plain(), "▸◇ Masonry");
    }

    #[test]
    fn the_grid_shows_fewer_columns_when_they_would_be_narrower_than_the_minimum() {
        let wide = Grid::new((84, 37), 5);
        assert_eq!((wide.columns, wide.column_width), (3, 27));
        // 120x40: the text is 116 wide, four tiers of 28 (#659).
        assert_eq!(Grid::new((116, 38), 5).columns, 4);
        assert_eq!(Grid::new((200, 40), 5).columns, 5, "never more than the tree has");
        assert_eq!(Grid::new((10, 40), 5).columns, 1, "at least one");
        // 80x24: the page is 80 wide and 23 tall, so the text is 76 by 21: three columns, four
        // cards.
        let small = Grid::new((76, 21), 6);
        assert_eq!((small.columns, small.cards), (3, 4));
        assert_eq!(Grid::new((44, 3), 6).cards, 1, "at least one card");
    }

    #[test]
    fn the_window_slides_only_once_the_cursor_passes_its_edge() {
        assert_eq!((0..4).map(|at| window_start(at, 4)).collect::<Vec<_>>(), [0, 0, 0, 0]);
        assert_eq!((4..7).map(|at| window_start(at, 4)).collect::<Vec<_>>(), [1, 2, 3]);
    }

    #[test]
    fn stepping_stops_at_the_edges_and_keeps_the_row_where_it_can() {
        let s = new_session(7, Nation::Earth);
        let view = screen_view(&s);
        let last = view.columns.len() - 1;
        assert_eq!(step(&view, (0, 0), -1, -1), (0, 0));
        assert_eq!(step(&view, (last, 0), 1, 0), (last, 0));
        let bottom = view.columns[0].len() - 1;
        assert_eq!(step(&view, (0, bottom), 0, 1), (0, bottom));
        // Into a shorter column the row comes back to its last card.
        let short = view.columns.iter().position(|c| c.len() < view.columns[0].len()).unwrap();
        let (c, r) = step(&view, (short - 1, bottom), 1, 0);
        assert_eq!((c, r), (short, view.columns[short].len() - 1));
    }

    #[test]
    fn the_cursor_starts_on_the_target_else_the_first_tech_to_research() {
        let mut s = new_session(7, Nation::Earth);
        let view = screen_view(&s);
        let (c, r) = initial_cursor(&s, &view);
        assert_eq!(view.columns[c][r].state, TechState::Available);
        let masonry = s.defs.tech_id("masonry").unwrap();
        s.apply(Command::SetResearch { tech: masonry }).unwrap();
        let view = screen_view(&s);
        let (c, r) = initial_cursor(&s, &view);
        assert_eq!(view.columns[c][r].id, masonry);
    }
}

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
use ratatui::style::{Modifier, Style};

use crate::color::Palette;
use crate::widgets::{Marks, Painter, Row, Tone, window_start};

/// The narrowest a column of cards can be. A terminal too narrow for every column scrolls
/// sideways with the cursor.
pub const MIN_COLUMN: usize = 18;
/// Columns are one blank cell apart.
const GAP: usize = 1;
/// A card is three rows (name, state and cost, unlocks) and a blank one between cards.
const CARD_ROWS: usize = 3;
const CARD_STRIDE: usize = CARD_ROWS + 1;
/// The rows under the grid: a rule, then the chosen tech's name, description and unlocks.
const DETAIL_ROWS: usize = 4;

/// Where the cursor is: a column of the view and a card in it.
pub type Cursor = (usize, usize);

/// How the grid fits an area of `(width, height)` text cells (inside the frame and its padding).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    /// How many columns show at once, and how wide each is.
    pub columns: usize,
    pub column_width: usize,
    /// How many cards of a column show at once.
    pub cards: usize,
}

impl Grid {
    pub fn new(text: (usize, usize), columns_in_view: usize) -> Self {
        let (width, height) = text;
        let columns = ((width + GAP) / (MIN_COLUMN + GAP)).clamp(1, columns_in_view.max(1));
        let column_width = (width.saturating_sub(GAP * (columns - 1)) / columns).max(1);
        // One row is the column headers; the details sit under the grid.
        let grid_height = height.saturating_sub(1 + DETAIL_ROWS);
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

/// A card as its three rows. `chosen` puts the cursor mark before the name. The third row is
/// what it unlocks once it's within reach, or, while `Locked`, what it still needs (#338): a
/// locked card alone used to say only "Locked", leaving which tier-1 tech unlocks it invisible
/// until the player researched each candidate and rechecked.
pub fn card_rows(card: &TechCard, chosen: bool, marks: &Marks) -> [Row; CARD_ROWS] {
    let tone = state_tone(card.state);
    let name_tone = match card.state {
        TechState::Available => Tone::Strong,
        other => state_tone(other),
    };
    let mark_tone = if card.state == TechState::Known { Tone::Ok } else { tone };
    let pointer = if chosen { marks.pointer } else { " " };
    let third = if card.state == TechState::Locked {
        format!("  Needs: {}", card.needs)
    } else {
        format!("  {}", card.unlock)
    };
    [
        Row::spans(vec![
            (pointer.to_owned(), Tone::Title),
            (format!("{} ", state_mark(card.state, marks)), mark_tone),
            (card.name.clone(), name_tone),
        ]),
        Row::pair(
            format!("  {}", state_word(card.state)),
            tone,
            format!("{} pts", card.cost),
            Tone::Dim,
        ),
        Row::text(third, Tone::Dim),
    ]
}

/// The three rows under the grid about the tech under the cursor: its name, state and cost; what
/// it is; what it unlocks (`card.unlock`, empty while `Locked`) and what it still needs
/// (`card.needs`, `fn_ui::research::TechCard`): a Queued tech can show both at once (its own
/// deeper prereqs may still be unknown), so the two are side by side rather than either/or.
pub fn detail_rows(session: &Session, card: &TechCard) -> [Row; 3] {
    let def = session.defs.tech(card.id);
    let unlocks =
        if card.unlock.is_empty() { String::new() } else { format!("Unlocks: {}", card.unlock) };
    let needs =
        if card.needs.is_empty() { String::new() } else { format!("Needs: {}", card.needs) };
    let sep = if unlocks.is_empty() || needs.is_empty() { "" } else { "   " };
    [
        Row::pair(
            card.name.clone(),
            Tone::Title,
            format!("{}  {} pts", card.state.label(), card.cost),
            state_tone(card.state),
        ),
        Row::text(def.description.clone(), Tone::Text),
        Row::spans(vec![(unlocks, Tone::Dim), (format!("{sep}{needs}"), Tone::Warn)]),
    ]
}

/// Draws the screen's contents into `inner` (the frame's inside, minus its padding).
pub fn draw(session: &Session, at: Cursor, palette: &Palette, inner: Rect, buf: &mut Buffer) {
    let view = screen_view(session);
    let marks = Marks::new(palette.ascii);
    let grid = Grid::new((usize::from(inner.width), usize::from(inner.height)), view.columns.len());
    let first_column = window_start(at.0, grid.columns);
    let first_card = window_start(at.1, grid.cards);
    let grid_height = (inner.height).saturating_sub(1 + DETAIL_ROWS as u16);

    let shown = view.columns.iter().enumerate().skip(first_column).take(grid.columns);
    for (slot, (c, column)) in shown.enumerate() {
        let x = inner.x + (slot * (grid.column_width + GAP)) as u16;
        let width = grid.column_width as u16;
        let chosen_column = c == at.0;

        // The column header: its depth, and arrows where cards are out of sight.
        let mut head = Painter::new(buf, Rect::new(x, inner.y, width, 1), palette);
        let flags = format!(
            "{}{}",
            if first_card > 0 { marks.up } else { "" },
            if column.len() > first_card + grid.cards { marks.down } else { "" },
        );
        let title_tone = if chosen_column { Tone::Title } else { Tone::Dim };
        head.row(&Row::pair(format!("TIER {}", c + 1), title_tone, flags, Tone::Dim));

        let body = Rect::new(x, inner.y + 1, width, grid_height);
        let mut p = Painter::new(buf, body, palette);
        let mut reversed = None;
        for (r, card) in column.iter().enumerate().skip(first_card).take(grid.cards) {
            let chosen = chosen_column && r == at.1;
            if chosen {
                reversed = Some(body.y + p.row);
            }
            for row in card_rows(card, chosen, &marks) {
                p.row(&row);
            }
            p.blank();
        }
        // The card under the cursor reads in reverse video on its name row, in every colour mode.
        if let Some(y) = reversed {
            for cx in x..x + width {
                buf[(cx, y)].set_style(Style::new().add_modifier(Modifier::REVERSED));
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

    // The details of the tech under the cursor, under a rule.
    let rule_y = inner.y + 1 + grid_height;
    let rule: String = std::iter::repeat_n(marks.h, usize::from(inner.width)).collect();
    buf.set_string(inner.x, rule_y, &rule, Tone::Dim.style(palette));
    if let Some(card) = card_at(&view, at) {
        let rows = detail_rows(session, card);
        let mut p = Painter::new(buf, Rect::new(inner.x, rule_y + 1, inner.width, 3), palette);
        for row in &rows {
            p.row(row);
        }
    }
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
        }
    }

    #[test]
    fn an_available_card_shows_the_name_the_state_the_cost_and_the_unlocks() {
        let marks = Marks::new(false);
        let rows = card_rows(&card(TechState::Available, "Construction x1.15", ""), false, &marks);
        let text: Vec<String> = rows.iter().map(Row::plain).collect();
        assert_eq!(text[0], " ◇ Masonry");
        assert_eq!(text[1], "  Available 120 pts");
        assert_eq!(text[2], "  Construction x1.15");
    }

    /// A Locked card shows what it still needs instead of what it would unlock (#338): the
    /// evidence was a whole playtest session spent discovering that one tier-1 tech unlocks
    /// another, with nothing on the Locked card itself saying so.
    #[test]
    fn a_locked_card_shows_what_it_needs_instead_of_what_it_would_unlock() {
        let marks = Marks::new(false);
        let rows =
            card_rows(&card(TechState::Locked, "Construction x1.15", "Carpentry"), false, &marks);
        let text: Vec<String> = rows.iter().map(Row::plain).collect();
        assert_eq!(text[0], " · Masonry");
        assert_eq!(text[1], "  Locked 120 pts");
        assert_eq!(text[2], "  Needs: Carpentry");
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
        let rows = card_rows(&card(TechState::Available, "-", ""), true, &marks);
        assert_eq!(rows[0].plain(), "▸◇ Masonry");
    }

    #[test]
    fn the_grid_shows_fewer_columns_when_they_would_be_narrower_than_the_minimum() {
        let wide = Grid::new((84, 37), 5);
        assert_eq!((wide.columns, wide.column_width), (4, 20));
        assert_eq!(Grid::new((200, 40), 5).columns, 5, "never more than the tree has");
        assert_eq!(Grid::new((10, 40), 5).columns, 1, "at least one");
        // 80x24: the map is 48 wide and 23 tall, so the text is 44 by 21: two columns, four cards.
        let small = Grid::new((44, 21), 6);
        assert_eq!((small.columns, small.cards), (2, 4));
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

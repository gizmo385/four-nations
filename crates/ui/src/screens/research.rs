//! The research screen (`T`, docs/design/research.md): the tech tree as columns by depth, one card
//! per tech. What it shows is `crate::research::screen_view` (the visibility rule, which techs a
//! nation sees, is the sim's `Defs::techs_for`). A card has the name, the state, the cost and a
//! summary of what it unlocks. `Enter` issues `Command::SetResearch`; `fn_sim` queues the whole
//! prereq chain of a locked tech, and a queued prereq is drawn the same as the target.
//!
//! The cursor is a (column, row) pair in the view. The window onto the grid (which columns and
//! which cards show) follows from the cursor and the size alone, so a frame is a pure function of
//! the state like every other.

use crate::Session;
use crate::research::{
    Newcomer, ResearchPage, ScreenView, TeachingRow, TechCard, TechState, cost_text, pair_short,
    screen_page,
};
use crate::rows::{Marks, Row, Tone, wrap};
use fn_sim::research::TechId;
use fn_sim::world::Command;

/// The narrowest a column of cards can be: wide enough that the line saying what a tech gives
/// wraps into two rows at most, instead of being cut (#659). It puts four tiers on a 120 column
/// terminal and three on an 80 column one; a terminal too narrow for every tier scrolls sideways
/// with the cursor.
pub const MIN_COLUMN: usize = 24;

/// Columns are one blank cell apart.
pub const GAP: usize = 1;

/// A card is at least three rows (name, state and cost, what it gives) and a blank one between
/// cards. The name and the last line wrap inside the column, so a card grows by a row when its
/// text needs it: this is the usual height, which `Grid::cards` and the page keys count in.
pub const CARD_ROWS: usize = 3;

pub const CARD_STRIDE: usize = CARD_ROWS + 1;

/// A card's lines under its name start this far in, under the name itself (the cursor's pointer,
/// the state's mark and a space come first).
pub const INDENT: usize = 3;

/// The frame's detail strip under the grid (`crate::page`, #652): the chosen tech's name,
/// description and unlocks. The rule above it takes one more row. This is its usual height; it
/// grows when the text wraps (`detail_height`, #768).
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
        view.columns.iter().enumerate().find_map(|(c, col)| {
            col.iter().position(|card| card.is_tech() && want(card)).map(|r| (c, r))
        })
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
/// by the sim and nothing changes; one with unmet prereqs queues its whole chain. A tech waiting
/// on a condition (residents of a nation, a master, people) is refused with what it needs, as the
/// status line says it: `Err("Can't set target: needs 3 Fire residents (2 live here)")`.
pub fn choose(session: &mut Session, page: ResearchPage, at: Cursor) -> Result<(), String> {
    let view = screen_page(session, page);
    let Some(card) = card_at(&view, at).filter(|c| c.is_tech()) else { return Ok(()) };
    match session.apply(Command::SetResearch { tech: card.id }) {
        Err(fn_sim::world::CommandError::Research(fn_sim::research::ResearchError::NotYet(c))) => {
            Err(format!(
                "Can't set target: needs {}",
                crate::research::live_text(&c, &session.world)
            ))
        }
        _ => Ok(()),
    }
}

/// Whether `Enter` at `at` reopens the LOST TEACHING box (#743): the TEACHINGS column's `!
/// Choose a teaching` row.
pub fn opens_draft(session: &Session, page: ResearchPage, at: Cursor) -> bool {
    let view = screen_page(session, page);
    card_at(&view, at).is_some_and(|c| matches!(c.teaching, Some(TeachingRow::Choose { .. })))
}

/// A column's header: "TIER 3", or "TEACHINGS" for the column after the last tier (#743). The
/// From newcomers page names its columns itself (`ScreenView::titles`).
pub fn column_title(view: &ScreenView, column: usize) -> String {
    if let Some(title) = view.titles.get(column) {
        title.clone()
    } else if view.teachings == Some(column) {
        "TEACHINGS".to_owned()
    } else {
        format!("TIER {}", column + 1)
    }
}

/// The Research screen's tabs once the gate stands: `[Your tree]  From newcomers`, the open page
/// in brackets and the title tone, and what `Tab` does flush right (as the key help's, #662).
pub fn page_tabs(page: ResearchPage) -> Row {
    let name = |(this, text): (ResearchPage, &str)| {
        if this == page { (format!("[{text}]"), Tone::Title) } else { (text.to_owned(), Tone::Dim) }
    };
    let [tree, newcomers] = PAGE_TABS.map(name);
    let mut row = Row::spans(vec![tree, ("  ".to_owned(), Tone::Text), newcomers]);
    row.right = Some((crate::screens::TAB_NOTE.to_owned(), Tone::Dim));
    row
}

/// The pages, left to right, and the words on each tab.
pub const PAGE_TABS: [(ResearchPage, &str); 2] =
    [(ResearchPage::Tree, "Your tree"), (ResearchPage::Newcomers, "From newcomers")];

/// The rows the tab strip takes from the top of the body: the strip and a blank one.
pub const TAB_ROWS: usize = 2;

/// The school option under the cursor, when `Enter` on it asks first (#729,
/// docs/design/horizons-phase-4.md "Mockups"): an open school option not already queued.
/// Choosing it is the one research choice that closes another tech for the rest of the game.
pub fn needs_confirm(session: &Session, page: ResearchPage, at: Cursor) -> Option<TechId> {
    let view = screen_page(session, page);
    let card = card_at(&view, at)?;
    let open = matches!(card.state, TechState::Available | TechState::Locked);
    (card.school.is_some() && open).then_some(card.id)
}

/// `Enter` in the CHOOSE A SCHOOL box: sets `tech` as the target.
pub fn confirm_school(session: &mut Session, tech: TechId) {
    let _ = session.apply(Command::SetResearch { tech });
}

/// The CHOOSE A SCHOOL box's lines for `tech`: the fork's question, what choosing gives up, the
/// bill and where it goes, what stores hold of it, and the keys.
pub fn confirm_lines(session: &Session, tech: TechId) -> Vec<String> {
    let defs = &session.defs;
    let def = defs.tech(tech);
    let name = &def.name;
    let other = defs.school_mates(tech).map(|m| defs.tech(m).name.clone()).collect::<Vec<_>>();
    let other = other.join(" or ");
    let question = def.school.map(|s| defs.school(s).question.clone()).unwrap_or_default();
    let items = crate::research::cost_items_text(defs, def);
    let mut out = vec![
        format!("{question} {name} or {other}."),
        String::new(),
        format!(
            "Set {name} as the target? Once it is learned, {other} is closed for the rest of \
             this game."
        ),
        String::new(),
    ];
    if items.is_empty() {
        out.push(format!("It costs {} pts.", def.cost));
    } else {
        let hall = defs.research.bill_hall_id.map_or("study hall".to_owned(), |h| {
            defs.building(h).display_name(session.world.nation).to_owned()
        });
        out.push(format!(
            "It costs {} pts and {items}. Haulers take them to a {hall} as soon as you choose. \
             Change your mind before it is learned and they come back.",
            def.cost
        ));
        out.push(String::new());
        let stores = def
            .cost_items
            .iter()
            .map(|s| format!("{} {}", session.world.stored_total(s.item), defs.item(s.item).name))
            .collect::<Vec<_>>()
            .join(", ");
        out.push(format!("In stores: {stores}"));
    }
    out.push(String::new());
    out.push(format!("Enter  choose {name}     Esc  back"));
    out
}

/// The CHOOSE A SCHOOL box's title, on its top edge.
pub const CONFIRM_TITLE: &str = "CHOOSE A SCHOOL";

/// How wide the CHOOSE A SCHOOL box is, in text cells, at most (the border and its padding
/// come to four more).
pub const CONFIRM_WIDTH: usize = 64;

/// The box's body as rows wrapped to `text` cells, never cut: the question and what choosing
/// gives up in the title's tone and the rest as text, a blank row between them
/// ([`confirm_lines`], less its last line, the keys: [`confirm_buttons`] words those).
pub fn confirm_body(session: &Session, tech: TechId, marks: &Marks, text: usize) -> Vec<Row> {
    let mut lines = confirm_lines(session, tech);
    lines.pop();
    let mut rows = Vec::new();
    for (i, line) in lines.into_iter().enumerate() {
        if line.is_empty() {
            rows.push(Row::text(String::new(), Tone::Text));
            continue;
        }
        let tone = if i == 0 { Tone::Title } else { Tone::Text };
        for piece in wrap(&line, text.max(1), usize::MAX, marks.ellipsis) {
            rows.push(Row::text(piece, tone));
        }
    }
    rows
}

/// What the box's two answers are called: `Enter` chooses `tech`, `Esc` goes back.
pub fn confirm_buttons(session: &Session, tech: TechId) -> (String, String) {
    (format!("Choose {} [Enter]", session.defs.tech(tech).name), "Back [Esc]".to_owned())
}

pub fn state_tone(state: TechState) -> Tone {
    match state {
        TechState::Known => Tone::Dim,
        TechState::Queued => Tone::Title,
        TechState::Waiting => Tone::Warn,
        TechState::Available => Tone::Text,
        TechState::Locked => Tone::Dim,
        TechState::Closed => Tone::Dim,
        TechState::Lost => Tone::Dim,
    }
}

/// The word a card carries for its state: short, so it fits beside the cost.
pub fn state_word(state: TechState) -> &'static str {
    match state {
        TechState::Known => "Known",
        TechState::Queued => "Queued",
        TechState::Waiting => "Waiting",
        TechState::Available => "Available",
        TechState::Locked => "Locked",
        TechState::Closed => "Closed",
        TechState::Lost => "Lost",
    }
}

/// The mark before a card's name, so the state reads without colour.
pub fn state_mark(state: TechState, marks: &Marks) -> &'static str {
    match state {
        TechState::Known => marks.known,
        TechState::Queued | TechState::Waiting => marks.seal,
        TechState::Available => marks.bullet,
        TechState::Locked => marks.locked,
        TechState::Closed => marks.closed,
        TechState::Lost => marks.closed,
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
pub fn gives_text(card: &TechCard) -> String {
    if card.state == TechState::Locked {
        format!("Needs {}", card.needs)
    } else {
        card.unlock.clone()
    }
}

/// `text`, a list ("Earthworks x1.2, Earth Tent"), broken after its commas into lines of at most
/// `width` cells, so an item (a building's name, say) is never split across two lines. An item
/// wider than a line on its own is broken at its spaces.
pub fn wrap_items(text: &str, width: usize, ellipsis: char) -> Vec<String> {
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
    let pointer = if chosen { marks.pointer } else { " " };
    // The TEACHINGS column's own rows (#743): one line each, and no cost.
    match &card.teaching {
        Some(TeachingRow::Choose { .. }) => {
            return vec![Row::spans(vec![
                (pointer.to_owned(), Tone::Title),
                ("! ".to_owned(), Tone::Warn),
                (card.name.clone(), Tone::Strong),
            ])];
        }
        Some(TeachingRow::More(_)) => {
            return vec![Row::spans(vec![
                (pointer.to_owned(), Tone::Title),
                ("? ".to_owned(), Tone::Dim),
                (card.name.clone(), Tone::Dim),
            ])];
        }
        Some(TeachingRow::Lost) => {
            let room = width.saturating_sub(INDENT).max(1);
            let pad = " ".repeat(INDENT);
            let mut rows = Vec::new();
            for (i, line) in
                wrap(&card.name, room, usize::MAX, marks.ellipsis).into_iter().enumerate()
            {
                rows.push(if i == 0 {
                    Row::spans(vec![
                        (pointer.to_owned(), Tone::Title),
                        (format!("{} ", marks.closed), Tone::Dim),
                        (line, Tone::Dim),
                    ])
                } else {
                    Row::text(format!("{pad}{line}"), Tone::Dim)
                });
            }
            rows.push(Row::text(format!("{pad}Lost"), Tone::Dim));
            rows.push(Row::text(format!("{pad}Lost to the ruins"), Tone::Dim));
            return rows;
        }
        Some(TeachingRow::Kept { .. }) | None => {}
    }
    let tone = state_tone(card.state);
    let name_tone = match card.state {
        TechState::Available => Tone::Strong,
        other => state_tone(other),
    };
    let mark_tone = if card.state == TechState::Known { Tone::Ok } else { tone };
    let room = width.saturating_sub(INDENT).max(1);
    let pad = " ".repeat(INDENT);
    let mut rows = Vec::new();
    // A school's options sit in a fork headed by its question, the second after an "or".
    if let Some(slot) = &card.school {
        let h = marks.h.to_string();
        rows.push(if slot.first {
            let head = format!("{}{h} {} ", marks.box_tl, slot.question.to_uppercase());
            let fill = width.saturating_sub(head.chars().count());
            Row::text(format!("{head}{}", h.repeat(fill)), Tone::Accent)
        } else {
            let side = width.saturating_sub(4) / 2;
            let rest = width.saturating_sub(side + 4);
            Row::text(format!("{} or {}", h.repeat(side), h.repeat(rest)), Tone::Accent)
        });
    }
    // A fusion names its two nations after its name when they fit, else on a row of their own.
    let pair = card.newcomer.as_ref().and_then(|n| n.fusion).map(pair_short);
    let beside = pair
        .as_ref()
        .filter(|p| card.name.chars().count() + 1 + p.chars().count() <= room)
        .cloned();
    for (i, line) in wrap(&card.name, room, usize::MAX, marks.ellipsis).into_iter().enumerate() {
        rows.push(if i == 0 {
            let mut spans = vec![
                (pointer.to_owned(), Tone::Title),
                (format!("{} ", state_mark(card.state, marks)), mark_tone),
                (line, name_tone),
            ];
            if let Some(p) = &beside {
                spans.push((format!(" {p}"), Tone::Dim));
            }
            Row::spans(spans)
        } else {
            Row::text(format!("{pad}{line}"), name_tone)
        });
    }
    if let (Some(p), None) = (&pair, &beside) {
        rows.push(Row::text(format!("{pad}{p}"), Tone::Dim));
    }
    let borrowed_known =
        card.state == TechState::Known && card.newcomer.as_ref().is_some_and(|n| n.from.is_some());
    let word = if borrowed_known { "Known, borrowed" } else { card_word(card) };
    rows.push(Row::spans(vec![
        (format!("{pad}{word}"), tone),
        (format!(" {} {} pts", marks.dot, card.cost), Tone::Dim),
    ]));
    if card.state == TechState::Closed {
        let chose = card.closed.strip_prefix("Closed: you chose ").unwrap_or(&card.closed);
        for line in wrap(&format!("You chose {chose}"), room, usize::MAX, marks.ellipsis) {
            rows.push(Row::text(format!("{pad}{line}"), Tone::Dim));
        }
    } else {
        let bill = if !card.bill.is_empty() {
            Some((card.bill.clone(), Tone::Text))
        } else if !card.items.is_empty() && card.state != TechState::Known {
            Some((format!("+ {}", card.items), Tone::Dim))
        } else {
            None
        };
        if let Some((bill, tone)) = bill {
            for line in wrap_items(&bill, room, marks.ellipsis) {
                rows.push(Row::text(format!("{pad}{line}"), tone));
            }
        }
        for line in wrap_items(&gives_text(card), room, marks.ellipsis) {
            rows.push(Row::text(format!("{pad}{line}"), Tone::Dim));
        }
    }
    if card.school.as_ref().is_some_and(|s| !s.first) {
        let h = marks.h.to_string();
        rows.push(Row::text(
            format!("{}{}", h.repeat(width.saturating_sub(1)), marks.box_br),
            Tone::Accent,
        ));
    }
    rows
}

/// The rows under the grid about the tech under the cursor, wrapped to `width` and never cut
/// (#768): its name (and the research target, when that is another tech), state and cost; what
/// it is; then what it unlocks (`card.unlock`, empty while `Locked`) and what it still needs
/// (`card.needs`, `crate::research::TechCard`), worded as the cards word them ("Leads to X",
/// "Needs X"). A Queued tech can show both at once (its own deeper prereqs may still be
/// unknown): side by side when they fit one row, else each on its own, broken after commas.
pub fn detail_rows(session: &Session, card: &TechCard, marks: &Marks, width: usize) -> Vec<Row> {
    if let Some(rows) = teaching_detail(session, card, marks, width) {
        return rows;
    }
    if let Some(newcomer) = &card.newcomer {
        return newcomer_detail(session, card, newcomer, marks, width);
    }
    let def = session.defs.tech(card.id);
    let unlocks = if card.unlock.is_empty() {
        String::new()
    } else if card.unlock.starts_with("Leads to ") {
        card.unlock.clone()
    } else {
        format!("Unlocks {}", card.unlock)
    };
    let needs = if card.needs.is_empty() { String::new() } else { format!("Needs {}", card.needs) };
    // The target is named in the strip whichever card the cursor is on: as the heading's state
    // when the cursor is on it, beside the name otherwise.
    let mut name = vec![(card.name.clone(), Tone::Title)];
    if !card.target
        && let Some(&target) = session.world.research().queue().last()
    {
        name.push((format!("   Target: {}", session.defs.tech(target).name), Tone::Dim));
    }
    let cost = match card.teaching {
        Some(TeachingRow::Kept { .. }) if card.state != TechState::Known => {
            crate::teaching::teaching_cost(&session.defs, card.id)
        }
        // Waiting has every point it asks for.
        _ if card.state == TechState::Waiting => {
            cost_text_with(&format!("{0}/{0}", card.cost), &card.items)
        }
        _ => cost_text(card.cost, &card.items),
    };
    let mut right = format!("{} {} {cost}", card_word(card), marks.dot);
    if !card.bill.is_empty() {
        right = format!("{right}  {}", card.bill);
    }
    let mut rows = vec![Row { left: name, right: Some((right, state_tone(card.state))) }];
    for line in wrap(&def.description, width.max(1), usize::MAX, marks.ellipsis) {
        rows.push(Row::text(line, Tone::Text));
    }
    let both = unlocks.chars().count() + 3 + needs.chars().count();
    if !unlocks.is_empty() && !needs.is_empty() && both <= width {
        rows.push(Row::spans(vec![(unlocks, Tone::Dim), (format!("   {needs}"), Tone::Warn)]));
    } else {
        for (text, tone) in [(unlocks, Tone::Dim), (needs, Tone::Warn)] {
            if !text.is_empty() {
                for line in wrap_items(&text, width.max(1), marks.ellipsis) {
                    rows.push(Row::text(line, tone));
                }
            }
        }
    }
    // A kept teaching costs its scroll too (#743): "Uses up the teaching scroll read at the
    // Meditation Pavilion", until it is learned.
    if let Some(TeachingRow::Kept { hall: Some(hall) }) = &card.teaching {
        let line = format!("Uses up the teaching scroll read at the {hall}");
        for piece in wrap(&line, width.max(1), usize::MAX, marks.ellipsis) {
            rows.push(Row::text(piece, Tone::Dim));
        }
    }
    for (line, tone) in school_lines(session, card) {
        for piece in wrap(&line, width.max(1), usize::MAX, marks.ellipsis) {
            rows.push(Row::text(piece, tone));
        }
    }
    rows.extend(waiting_rows(session, card, marks, width));
    rows
}

/// "500/500 pts + 4 Tool": [`cost_text`] with the points already worded.
fn cost_text_with(points: &str, items: &str) -> String {
    if items.is_empty() { format!("{points} pts") } else { format!("{points} pts + {items}") }
}

/// The detail strip's last row for a tech that waits on a condition: "Waiting: needs 3 Fire
/// residents (2 live here)". Empty for any other card.
fn waiting_rows(session: &Session, card: &TechCard, marks: &Marks, width: usize) -> Vec<Row> {
    if card.state != TechState::Waiting {
        return Vec::new();
    }
    let Some(condition) = crate::research::waiting_condition(&session.world, &session.defs) else {
        return Vec::new();
    };
    let line = format!("Waiting: needs {}", crate::research::live_text(&condition, &session.world));
    wrap(&line, width.max(1), usize::MAX, marks.ellipsis)
        .into_iter()
        .map(|piece| Row::text(piece, Tone::Warn))
        .collect()
}

/// The detail strip of a card of the From newcomers page (docs/design/horizons-phase-6.md,
/// "Mockups"): the name with state and cost, what it is, what it unlocks (a borrowed tech's
/// modifiers for its own nation's residents only), where it comes from and the borrowed price or
/// the two nations of a fusion, the prereqs a borrower is spared, then every condition with its
/// live count, or what a waiting tech is waiting for.
fn newcomer_detail(
    session: &Session,
    card: &TechCard,
    newcomer: &Newcomer,
    marks: &Marks,
    width: usize,
) -> Vec<Row> {
    let (defs, world) = (&session.defs, &session.world);
    let def = defs.tech(card.id);
    let mut name = vec![(card.name.clone(), Tone::Title)];
    if !card.target
        && let Some(&target) = world.research().queue().last()
    {
        name.push((format!("   Target: {}", defs.tech(target).name), Tone::Dim));
    }
    let cost = if card.state == TechState::Waiting {
        cost_text_with(&format!("{0}/{0}", card.cost), &card.items)
    } else {
        cost_text(card.cost, &card.items)
    };
    let mut right = format!("{} {} {cost}", card_word(card), marks.dot);
    if !card.bill.is_empty() {
        right = format!("{right}  {}", card.bill);
    }
    let mut rows = vec![Row { left: name, right: Some((right, state_tone(card.state))) }];
    let push = |rows: &mut Vec<Row>, text: &str, tone: Tone| {
        for piece in wrap(text, width.max(1), usize::MAX, marks.ellipsis) {
            rows.push(Row::text(piece, tone));
        }
    };
    push(&mut rows, &def.description, Tone::Text);
    // What it unlocks, then what only its own nation's residents get from it.
    let (_, rest) = crate::research::unlock_parts(defs, world.nation, card.id, def);
    let mut unlocks = rest.join(", ");
    if unlocks.is_empty() && newcomer.modifiers.is_empty() {
        unlocks = crate::research::unlock_summary(defs, world.nation, card.id, def);
    }
    let mut gives = String::new();
    if !unlocks.is_empty() && unlocks != "-" {
        gives = if unlocks.starts_with("Leads to ") {
            unlocks.clone()
        } else {
            format!("Unlocks {unlocks}")
        };
    }
    if !newcomer.modifiers.is_empty() {
        gives = if gives.is_empty() {
            newcomer.modifiers.clone()
        } else {
            format!("{gives}. {}", newcomer.modifiers)
        };
    }
    if !gives.is_empty() {
        push(&mut rows, &gives, Tone::Dim);
    }
    // Where it comes from, and the price.
    if let Some(from) = newcomer.from {
        let mut line = format!(
            "Borrowed from the {} at {}x cost.",
            defs.nation(from).name,
            defs.research.borrow_cost
        );
        if !newcomer.waived.is_empty() {
            line = format!("{line} No {} needed.", join_and(&newcomer.waived));
        }
        push(&mut rows, &line, Tone::Text);
    } else if let Some([a, b]) = newcomer.fusion {
        push(&mut rows, &format!("Fusion of {} and {}.", a.title(), b.title()), Tone::Text);
    }
    // What it asks of the settlement, or what it waits for.
    if card.state == TechState::Waiting {
        rows.extend(waiting_rows(session, card, marks, width));
    } else if card.state != TechState::Known {
        let mut asks: Vec<String> = Vec::new();
        asks.extend(missing_prereq_names(session, card));
        asks.extend(newcomer.conditions.iter().map(|(text, _)| text.clone()));
        if !asks.is_empty() {
            let unmet =
                newcomer.conditions.iter().any(|(_, met)| !met) || card.state == TechState::Locked;
            let tone = if unmet { Tone::Warn } else { Tone::Dim };
            push(&mut rows, &format!("Needs {}.", join_and(&asks)), tone);
        }
    }
    rows
}

/// The prereqs of `card`'s tech a settlement that borrows it still has to learn: unknown ones, in
/// the order the sim lists them (`World::exchange`).
fn missing_prereq_names(session: &Session, card: &TechCard) -> Vec<String> {
    let (defs, world) = (&session.defs, &session.world);
    let def = defs.tech(card.id);
    let prereqs = if def.is_fusion() { def.prereqs.clone() } else { defs.borrow_prereqs(card.id) };
    prereqs
        .into_iter()
        .filter(|&p| !world.research().is_known(p))
        .map(|p| defs.tech(p).name.clone())
        .collect()
}

/// "A", "A and B", "A, B and C".
fn join_and(items: &[String]) -> String {
    match items.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// The detail strip for the TEACHINGS column's own rows and its lost teachings (#743); `None`
/// for any other card.
fn teaching_detail(
    session: &Session,
    card: &TechCard,
    marks: &Marks,
    width: usize,
) -> Option<Vec<Row>> {
    let (name, lines): (String, Vec<String>) = match &card.teaching {
        Some(TeachingRow::Choose { waiting }) => {
            let offer = session.world.teaching_offers().first()?;
            let hall = crate::teaching::hall_name(session, offer.hall);
            let more = match waiting {
                1 => String::new(),
                n => format!(" {n} drafts are waiting."),
            };
            (
                card.name.clone(),
                vec![format!(
                    "A teaching scroll was read at the {hall}.{more} Enter to keep one teaching; \
                     the other is lost for this game."
                )],
            )
        }
        Some(TeachingRow::More(n)) => (
            card.name.clone(),
            vec![format!(
                "{n} more {} could still be found. Great ruins hold teaching scrolls.",
                if *n == 1 { "teaching" } else { "teachings" }
            )],
        ),
        Some(TeachingRow::Lost) => {
            let def = session.defs.tech(card.id);
            (
                card.name.clone(),
                vec![
                    def.description.clone(),
                    "Lost to the ruins: another teaching was kept from its draft.".to_owned(),
                ],
            )
        }
        Some(TeachingRow::Kept { .. }) | None => return None,
    };
    let right = match card.teaching {
        Some(TeachingRow::Lost) => Some(("Lost".to_owned(), Tone::Dim)),
        _ => None,
    };
    let mut rows = vec![Row { left: vec![(name, Tone::Title)], right }];
    for line in lines {
        for piece in wrap(&line, width.max(1), usize::MAX, marks.ellipsis) {
            rows.push(Row::text(piece, Tone::Text));
        }
    }
    Some(rows)
}

/// The detail strip's school lines (#729): what the fork is while it is open, the bill and where
/// it goes while one is on the way, or why a closed option is closed.
pub fn school_lines(session: &Session, card: &TechCard) -> Vec<(String, Tone)> {
    let Some(slot) = &card.school else { return Vec::new() };
    let mut out = Vec::new();
    match card.state {
        TechState::Closed => {
            let chose = card.closed.strip_prefix("Closed: you chose ").unwrap_or(&slot.other);
            out.push((
                format!(
                    "Closed: this settlement chose {chose}. A school is one choice for the whole \
                     game."
                ),
                Tone::Warn,
            ));
        }
        TechState::Known => {}
        _ if !card.bill.is_empty() => {
            let defs = &session.defs;
            let hall = defs.research.bill_hall_id.map_or("study hall".to_owned(), |h| {
                defs.building(h).display_name(session.world.nation).to_owned()
            });
            out.push((
                format!(
                    "Bill: {}, hauled to a {hall}. {} stays open until this is learned.",
                    card.bill, slot.other
                ),
                Tone::Text,
            ));
        }
        _ => out.push((
            format!(
                "School: {} Choosing one closes the other for the rest of the game.",
                slot.question
            ),
            Tone::Accent,
        )),
    }
    out
}

/// How many rows the detail strip takes for the tech under `at` on a page whose text is `width`
/// cells across: the usual [`DETAIL_ROWS`], one more for each row its text wraps onto (#768).
pub fn detail_height(session: &Session, page: ResearchPage, at: Cursor, width: usize) -> u16 {
    let view = screen_page(session, page);
    let Some(card) = card_at(&view, at) else { return DETAIL_ROWS };
    let rows = detail_rows(session, card, &Marks::new(false), width).len();
    u16::try_from(rows).unwrap_or(u16::MAX).max(DETAIL_ROWS)
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use fn_sim::research::TechId;

    use super::*;
    use crate::new_session;
    use crate::research::screen_view;
    use crate::rows::window_start;

    fn card(state: TechState, unlock: &str, needs: &str) -> TechCard {
        TechCard {
            id: TechId(0),
            name: "Masonry".into(),
            cost: 120,
            state,
            unlock: unlock.into(),
            needs: needs.into(),
            target: false,
            closed: String::new(),
            items: String::new(),
            bill: String::new(),
            school: None,
            teaching: None,
            newcomer: None,
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

    /// A card of the From newcomers page as the tests need it.
    fn newcomer(state: TechState, from: Option<Nation>, fusion: Option<[Nation; 2]>) -> TechCard {
        TechCard {
            newcomer: Some(Newcomer {
                from,
                fusion,
                modifiers: String::new(),
                conditions: Vec::new(),
                waived: Vec::new(),
            }),
            ..card(state, "Brick Kiln", "")
        }
    }

    #[test]
    fn a_known_borrowed_card_says_so_and_a_fusion_names_its_nations_beside_its_name() {
        let marks = Marks::new(false);
        let mut known = newcomer(TechState::Known, Some(Nation::Fire), None);
        known.cost = 240;
        let text = plain(&card_rows(&known, false, &marks, 28));
        assert_eq!(text[1], "   Known, borrowed · 240 pts");

        let mut fusion = newcomer(TechState::Locked, None, Some([Nation::Earth, Nation::Water]));
        fusion.name = "Aqueducts".into();
        fusion.needs = "3 Earth residents".into();
        let text = plain(&card_rows(&fusion, false, &marks, 28));
        assert_eq!(text[0], " · Aqueducts Earth+Water");
        assert_eq!(text.last().unwrap(), "   Needs 3 Earth residents");
        // In a column too narrow for both, the nations take a row of their own.
        let text = plain(&card_rows(&fusion, false, &marks, 18));
        assert_eq!(text[..2], [" · Aqueducts", "   Earth+Water"]);
    }

    #[test]
    fn a_waiting_card_reads_waiting_and_the_tabs_open_on_the_page_that_is_open() {
        let marks = Marks::new(false);
        let waiting = card(TechState::Waiting, "Brick Kiln", "");
        let text = plain(&card_rows(&waiting, false, &marks, 28));
        assert_eq!(text[1], "   Waiting · 120 pts");
        assert_eq!(
            page_tabs(ResearchPage::Tree).plain(),
            "[Your tree]  From newcomers Tab switches"
        );
        assert_eq!(
            page_tabs(ResearchPage::Newcomers).plain(),
            "Your tree  [From newcomers] Tab switches"
        );
    }

    #[test]
    fn before_the_gate_the_newcomers_page_is_empty_and_the_tree_is_unchanged() {
        let s = new_session(7, Nation::Water);
        assert!(!crate::research::has_newcomers(&s));
        assert!(screen_page(&s, ResearchPage::Newcomers).columns.is_empty());
        assert_eq!(screen_page(&s, ResearchPage::Tree), screen_view(&s));
        assert!(screen_view(&s).titles.is_empty());
        let view = screen_view(&s);
        assert_eq!(column_title(&view, 0), "TIER 1");
    }
}

//! What the side panel's Tool section says about the active tool (#657), as groups of lines a
//! frontend wraps and tints: what the tool does, how big the rectangle is and what applying it
//! would mark, the tool's own toggles, and, once it has been applied, what it marked.
//!
//! ```text
//! ├─ HARVEST ────────────── [H] ─┤
//! │ Chop, mine or gather what is │   the description (`Tool::about`)
//! │ inside the rectangle.        │
//! │                              │
//! │ 7 x 4 tiles                  │   the rectangle, with a corner set
//! │ 7 trees to chop              │   what applying would mark (`tools::preview_order`)
//! │                              │
//! │ Standing order: off          │   the tool's toggles
//! ```
//!
//! Each group is a [`Block`]; [`blocks`] returns them in the order they read. With the tool list
//! open (`/`, #664) the section is that list alone, [`choices`] as rows:
//!
//! ```text
//! ├─ DESIGNATE ───────────────── [/] ─┤
//! │ ▸ H Harvest    chop, mine         │
//! │   X Cancel     remove a mark      │
//! │   B Bend       Raise Earth        │
//! │   C Sow        build a farm plot  │   dim, with the reason
//! ```
//!
//! The counts are the ones
//! `tools::apply_order` gives, since the preview is the same loop run over a copy of the world,
//! so what the section promises and what applying does cannot disagree.

use fn_sim::Pos;
use fn_sim::building::BuildingId;
use fn_sim::world::PlotStage;

use crate::session::Session;
use crate::tools::{Order, Tool, bend_ability, preview_order, usable_abilities};

/// How a line should read, without naming a colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emphasis {
    Plain,
    /// A reason or a state that is background to the rest.
    Dim,
}

/// One line of the section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// Text that wraps to the panel's width.
    Text(String, Emphasis),
    /// A toggle and its state: "Standing order" and "off". A frontend that has room puts the
    /// state at the right edge; with none, the pair reads "Standing order: off" and wraps.
    Setting { label: String, value: String },
    /// A row of the tool list (#664): the tool's letter, its name and a few words on what it is
    /// for, or, with `usable` false, why it can't be used yet (drawn dim). `selected` is the
    /// row the cursor is on.
    Choice { selected: bool, key: char, name: String, what: String, usable: bool },
}

/// A group of lines. Blank rows go between groups when there is room, and a short panel drops
/// whole groups from the last.
pub type Block = Vec<Line>;

/// What the section needs to know about the player's tool: the frontend's own state.
#[derive(Clone, Copy, Debug)]
pub struct ToolState<'a> {
    pub tool: Tool,
    /// The first corner, once set; the cursor is the second.
    pub anchor: Option<Pos>,
    pub cursor: Pos,
    /// The Harvest tool's standing-order toggle.
    pub repeating: bool,
    /// The Cancel tool's toggle for also taking sites and stockpiles.
    pub teardown: bool,
    /// The Build tool's building.
    pub choice: Option<BuildingId>,
    /// What the rectangle just applied marked, while it is still the news
    /// (`Report::summary`); `None` once the cursor has moved on or a new corner is set.
    pub marked: Option<&'a str>,
    /// The tool list (`/`, #664), when it is open: the row the cursor is on, an index into
    /// [`choices`].
    pub list: Option<usize>,
}

/// One tool in the list (`/`, #664).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolChoice {
    pub tool: Tool,
    /// The letter that picks it: "H".
    pub letter: char,
    pub name: &'static str,
    /// What it is for, in a few words: "chop, mine". Bend names the nation's ability, from the
    /// data.
    pub what: String,
    /// Why it can't be used yet ("build a farm plot"), or `None` if it can.
    pub unusable: Option<String>,
}

/// Every tool that has a letter, in the order the key help lists them (`Tool::ALL` without
/// `Select`), with what each is for and, for one that can't be used yet, why.
pub fn choices(session: &Session) -> Vec<ToolChoice> {
    Tool::ALL
        .into_iter()
        .filter_map(|tool| {
            let letter = tool.letter()?;
            let (what, unusable) = match tool {
                Tool::Harvest => ("chop, mine".to_owned(), None),
                Tool::CancelHarvest => ("remove a mark".to_owned(), None),
                Tool::Clear | Tool::Deconstruct => ("clear ground".to_owned(), None),
                Tool::Stockpile => ("storage".to_owned(), None),
                Tool::Bend => bend_choice(session),
                Tool::Build => ("a building".to_owned(), None),
                Tool::Sow => ("a farm plot".to_owned(), sow_unusable(session)),
                Tool::Select | Tool::Gate => return None,
            };
            Some(ToolChoice { tool, letter, name: tool.name(), what, unusable })
        })
        .collect()
}

/// Bend's row: the ability `B` would channel, by name; with none usable yet, why.
fn bend_choice(session: &Session) -> (String, Option<String>) {
    match bend_ability(session) {
        Some(id) if !usable_abilities(session).is_empty() => {
            (session.defs.ability(id).name.clone(), None)
        }
        Some(id) => (session.defs.ability(id).name.clone(), Some("not unlocked yet".to_owned())),
        None => ("an ability".to_owned(), Some("no ability yet".to_owned())),
    }
}

/// Why the Sow tool has nothing to do: no farm plot is built, or every empty one is already marked.
fn sow_unusable(session: &Session) -> Option<String> {
    let defs = &session.defs;
    let plots: Vec<_> =
        session.world.buildings().filter(|b| defs.building(b.id).farm.is_some()).collect();
    if plots.is_empty() {
        return Some("no farm plot".to_owned());
    }
    // An empty plot already marked to be sown is as good as sown: applying the tool finds nothing.
    let marked: Vec<Pos> = session.world.sow_designations().collect();
    let open = plots.iter().any(|b| {
        session.world.plot_stage(b.anchor) == Some(PlotStage::Empty) && !marked.contains(&b.anchor)
    });
    (!open).then(|| "no empty plots".to_owned())
}

/// The size of the rectangle between two corners: "7 x 4 tiles".
fn size_text(a: Pos, b: Pos) -> String {
    let (w, h) = ((a.x - b.x).abs() + 1, (a.y - b.y).abs() + 1);
    format!("{w} x {h} tiles")
}

/// `text` with its first letter upper case: a refusal's fragment as a line of its own.
fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The toggles the tool has, with their state. Empty for a tool with none.
fn settings(st: &ToolState) -> Block {
    let on = |b: bool| if b { "on" } else { "off" }.to_owned();
    match st.tool {
        Tool::Harvest => {
            vec![Line::Setting { label: "Standing order".to_owned(), value: on(st.repeating) }]
        }
        Tool::CancelHarvest => vec![Line::Setting {
            label: "Also sites, stockpiles".to_owned(),
            value: on(st.teardown),
        }],
        _ => Vec::new(),
    }
}

/// With a corner set: the rectangle's size, then what applying would mark, one line a kind (or
/// why nothing), and for the Build tool the tiles it clears first and what it costs.
fn preview(session: &Session, st: &ToolState, anchor: Pos) -> Block {
    let report = preview_order(
        session,
        Order {
            tool: st.tool,
            from: anchor,
            to: st.cursor,
            repeating: st.repeating,
            teardown: st.teardown,
            choice: st.choice,
        },
    );
    let mut lines = vec![Line::Text(size_text(anchor, st.cursor), Emphasis::Plain)];
    if !report.done() {
        lines.push(Line::Text(st.tool.nothing().to_owned(), Emphasis::Plain));
        // Build and Bend can say why: a tech missing, a rank, a tree in the way.
        if let (Tool::Build | Tool::Bend, Some(why)) = (st.tool, &report.why) {
            lines.push(Line::Text(sentence(why), Emphasis::Dim));
        }
        return lines;
    }
    lines.extend(report.counts.iter().map(|c| Line::Text(c.preview(), Emphasis::Plain)));
    if report.first > 0 {
        let tiles = if report.first == 1 { "tile" } else { "tiles" };
        lines.push(Line::Text(format!("{} {tiles} to clear first", report.first), Emphasis::Plain));
    }
    if let Some(cost) = &report.cost {
        lines.push(Line::Text(format!("Cost: {cost}"), Emphasis::Plain));
    }
    lines
}

/// The tool list as one group of rows, the cursor on row `at`.
fn list_block(session: &Session, at: usize) -> Block {
    choices(session)
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let usable = c.unusable.is_none();
            let what = c.unusable.unwrap_or(c.what);
            Line::Choice { selected: i == at, key: c.letter, name: c.name.to_owned(), what, usable }
        })
        .collect()
}

/// The groups of lines the Tool section holds under its heading, in reading order: the
/// description, the live group (the rectangle's preview with a corner set, else what the last
/// apply marked), and the toggles. Empty for `Select`. A group with nothing to say is left out.
pub fn blocks(session: &Session, st: &ToolState) -> Vec<Block> {
    if let Some(at) = st.list {
        return vec![list_block(session, at)];
    }
    if st.tool == Tool::Select {
        return Vec::new();
    }
    let mut blocks = vec![vec![Line::Text(st.tool.about(session), Emphasis::Plain)]];
    // The gate has no rectangle: one `Enter` places it.
    if st.tool != Tool::Gate {
        match (st.anchor, st.marked) {
            (Some(anchor), _) => blocks.push(preview(session, st, anchor)),
            (None, Some(marked)) => {
                blocks.push(vec![Line::Text(marked.to_owned(), Emphasis::Plain)]);
            }
            (None, None) => {}
        }
    }
    blocks.push(settings(st));
    blocks.retain(|b| !b.is_empty());
    blocks
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use fn_sim::geom::Footprint;
    use fn_sim::world::Command;

    use super::*;
    use crate::build;
    use crate::tools::{Report, apply_order};

    /// The Earth game on a 64x64 map, with two farm plots and a hall on the ground near its
    /// corner, so every tool has something to count in the whole-map rectangle.
    fn scene() -> Session {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let defs = s.defs.clone();
        let plot = s.defs.building_id("farm_plot").unwrap();
        let hall = s.defs.building_id("study_hall").unwrap();
        s.world.put_building(Footprint::single(Pos::new(2, 2)), plot, &defs);
        s.world.put_building(Footprint::single(Pos::new(3, 2)), plot, &defs);
        s.world.put_building(Footprint::new(Pos::new(5, 2), 2, 2), hall, &defs);
        s
    }

    fn order(tool: Tool, choice: Option<BuildingId>) -> Order {
        Order {
            tool,
            from: Pos::new(0, 0),
            to: Pos::new(63, 63),
            repeating: false,
            teardown: false,
            choice,
        }
    }

    /// What the preview promised, then what applying did: the same counts, cost and reasons.
    fn agree(s: &mut Session, o: Order) -> Report {
        let before = s.world.tick;
        let preview = preview_order(s, o);
        let applied = apply_order(s, o);
        assert_eq!(s.world.tick, before);
        assert_eq!(preview.counts, applied.counts, "{:?}", o.tool);
        assert_eq!(preview.refused, applied.refused, "{:?}", o.tool);
        assert_eq!(preview.first, applied.first, "{:?}", o.tool);
        assert_eq!(preview.cost, applied.cost, "{:?}", o.tool);
        assert_eq!(preview.why, applied.why, "{:?}", o.tool);
        applied
    }

    /// Previewing marks nothing: a copy of the world takes the commands.
    #[test]
    fn a_preview_changes_nothing() {
        let s = scene();
        let jobs = s.world.jobs().len();
        let report = preview_order(&s, order(Tool::Harvest, None));
        assert!(report.done());
        preview_order(&s, order(Tool::Stockpile, None));
        assert_eq!((s.world.jobs().len(), s.world.stockpile_zones().count()), (jobs, 0));
    }

    #[test]
    fn harvest_counts_what_each_kind_of_tile_gives() {
        let mut s = scene();
        let report = agree(&mut s, order(Tool::Harvest, None));
        let lines: Vec<String> = report.counts.iter().map(|c| c.preview()).collect();
        assert!(lines.iter().any(|l| l.ends_with(" trees to chop")), "{lines:?}");
        assert!(lines.iter().any(|l| l.ends_with(" rock to quarry")), "{lines:?}");
        // Applying again finds them all marked: nothing is left to harvest.
        let again = agree(&mut s, order(Tool::Harvest, None));
        assert!(!again.done());
        assert!(again.summary().starts_with("Nothing marked: "), "{}", again.summary());
    }

    #[test]
    fn cancel_counts_the_marks_it_withdraws_by_kind() {
        let mut s = scene();
        agree(&mut s, order(Tool::Harvest, None));
        let corner =
            Order { from: Pos::new(26, 26), to: Pos::new(36, 36), ..order(Tool::Stockpile, None) };
        agree(&mut s, corner);
        let plain = agree(&mut s, order(Tool::CancelHarvest, None));
        assert!(plain.counts.iter().any(|c| c.preview().ends_with("harvest marks to remove")));
        assert!(!plain.counts.iter().any(|c| c.preview().contains("stockpile")));
        // With the toggle on it takes the stockpile tiles too, which a plain cancel left.
        let all = Order { teardown: true, ..order(Tool::CancelHarvest, None) };
        let torn = agree(&mut s, all);
        assert!(torn.counts.iter().any(|c| c.preview().contains("stockpile")), "{torn:?}");
        assert_eq!(torn.lead, "Removed");
    }

    #[test]
    fn clear_stockpile_and_bend_count_tiles() {
        let mut s = scene();
        let clear = agree(&mut s, order(Tool::Clear, None));
        assert!(clear.counts[0].preview().ends_with(" tiles to clear"), "{clear:?}");
        let stock = agree(&mut s, order(Tool::Stockpile, None));
        assert!(stock.counts[0].preview().ends_with(" tiles to add"), "{stock:?}");
        // Applying also says how many of them nobody can reach; the preview doesn't look.
        let added = format!("Added {}.", stock.counts[0].noun());
        assert!(stock.summary().starts_with(&added), "{}", stock.summary());
        let bend = agree(&mut s, order(Tool::Bend, None));
        assert!(bend.done(), "{bend:?}");
        assert!(bend.counts[0].preview().ends_with(" to bend"));
    }

    #[test]
    fn deconstruct_counts_each_building_once() {
        let mut s = scene();
        let report = agree(&mut s, order(Tool::Deconstruct, None));
        // Two farm plots and a hall, however many tiles the hall covers.
        assert_eq!(report.counts.len(), 1);
        assert_eq!(report.counts[0].preview(), "3 buildings to take down");
    }

    #[test]
    fn sow_counts_the_empty_plots() {
        let mut s = scene();
        let report = agree(&mut s, order(Tool::Sow, None));
        assert_eq!(report.counts[0].preview(), "2 plots to sow");
        assert_eq!(report.summary(), "Marked 2 plots.");
        let again = agree(&mut s, order(Tool::Sow, None));
        assert!(!again.done());
    }

    /// Build counts the sites that fit, the resource tiles it clears first and the cost of the
    /// lot.
    #[test]
    fn build_counts_sites_the_tiles_to_clear_first_and_the_cost() {
        let mut s = scene();
        let bed = s.defs.building_id("bed").unwrap();
        let row =
            Order { from: Pos::new(12, 20), to: Pos::new(40, 24), ..order(Tool::Build, Some(bed)) };
        let report = agree(&mut s, row);
        let sites = report.counts[0].n;
        assert!(sites > 1, "{report:?}");
        let cost = build::cost_of(&s, bed, sites);
        assert_eq!(report.cost.as_deref(), Some(cost.as_str()));
        let text = report.summary();
        assert!(text.starts_with(&format!("Marked {sites} sites")), "{text}");
    }

    /// A rectangle with no room says why, as a refusal does.
    #[test]
    fn a_build_with_no_room_says_why() {
        let mut s = scene();
        let hall = s.defs.building_id("study_hall").unwrap();
        let on_it =
            Order { from: Pos::new(5, 2), to: Pos::new(6, 3), ..order(Tool::Build, Some(hall)) };
        let report = agree(&mut s, on_it);
        assert!(!report.done());
        assert!(report.why.is_some());
    }

    #[test]
    fn the_blocks_hold_what_the_tool_does_the_preview_and_the_toggles() {
        let s = scene();
        let state = ToolState {
            tool: Tool::Harvest,
            anchor: Some(Pos::new(0, 0)),
            cursor: Pos::new(6, 3),
            repeating: true,
            teardown: false,
            choice: None,
            marked: Some("Marked 1 tree."),
            list: None,
        };
        let shown = blocks(&s, &state);
        assert_eq!(shown.len(), 3);
        assert_eq!(shown[0], [Line::Text(Tool::Harvest.about(&s), Emphasis::Plain)]);
        // A corner is set: the preview, not the last result.
        assert_eq!(shown[1][0], Line::Text("7 x 4 tiles".to_owned(), Emphasis::Plain));
        let marked = |b: &Vec<Block>| {
            b.iter().flatten().any(|l| matches!(l, Line::Text(t, _) if t.starts_with("Marked")))
        };
        assert!(!marked(&shown));
        let setting = Line::Setting { label: "Standing order".to_owned(), value: "on".to_owned() };
        assert_eq!(shown[2], [setting]);
        // Without one, the result.
        let done = blocks(&s, &ToolState { anchor: None, ..state });
        assert_eq!(done[1], [Line::Text("Marked 1 tree.".to_owned(), Emphasis::Plain)]);
        // Select has no section; the gate has a description and no rectangle.
        assert!(blocks(&s, &ToolState { tool: Tool::Select, ..state }).is_empty());
        assert_eq!(blocks(&s, &ToolState { tool: Tool::Gate, ..state }).len(), 1);
    }

    /// The list has the seven lettered tools in the key help's order, Bend names the nation's
    /// ability, and Sow says why not until there is a plot with nothing growing in it.
    #[test]
    fn the_tool_list_names_each_tool_the_ability_and_why_sow_cannot() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let list = choices(&s);
        let letters: String = list.iter().map(|c| c.letter).collect();
        assert_eq!(letters, "HXEPBUC");
        let bend = list.iter().find(|c| c.tool == Tool::Bend).unwrap();
        assert_eq!(bend.what, "Raise Earth");
        assert!(list.iter().filter(|c| c.unusable.is_some()).count() == 1);
        let sow = list.iter().find(|c| c.tool == Tool::Sow).unwrap();
        assert_eq!(sow.unusable.as_deref(), Some("no farm plot"));

        // A farm plot with nothing planted can be sown; once it is marked and sown, none can.
        let defs = s.defs.clone();
        let plot = s.defs.building_id("farm_plot").unwrap();
        s.world.put_building(Footprint::single(Pos::new(2, 2)), plot, &defs);
        let sow = |s: &Session| choices(s).into_iter().find(|c| c.tool == Tool::Sow).unwrap();
        assert_eq!(sow(&s).unusable, None);
        s.apply(Command::Sow { pos: Pos::new(2, 2) }).unwrap();
        assert_eq!(sow(&s).unusable.as_deref(), Some("no empty plots"));
    }

    /// With the list open the section is the list alone, one row a tool, the cursor on one.
    #[test]
    fn the_blocks_are_the_list_alone_while_it_is_open() {
        let s = scene();
        let state = ToolState {
            tool: Tool::Select,
            anchor: None,
            cursor: Pos::new(0, 0),
            repeating: false,
            teardown: false,
            choice: None,
            marked: None,
            list: Some(2),
        };
        let shown = blocks(&s, &state);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].len(), 7);
        let selected: Vec<usize> = shown[0]
            .iter()
            .enumerate()
            .filter(|(_, l)| matches!(l, Line::Choice { selected: true, .. }))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(selected, [2]);
        // Any tool: the list replaces its description, preview and toggles.
        let in_tool = blocks(&s, &ToolState { tool: Tool::Harvest, ..state });
        assert_eq!(in_tool, shown);
        // Closed, Select has no section again.
        assert!(blocks(&s, &ToolState { list: None, ..state }).is_empty());
    }

    #[test]
    fn every_tool_says_what_it_does() {
        let s = scene();
        let tools = Tool::ALL.into_iter().filter(|&t| t != Tool::Select);
        for tool in tools.chain([Tool::Deconstruct, Tool::Gate]) {
            let about = tool.about(&s);
            assert!(about.ends_with('.') && about.len() > 20, "{tool:?}: {about}");
            assert_eq!(tool.nothing().is_empty(), tool == Tool::Gate, "{tool:?}");
        }
        assert_eq!(Tool::Select.about(&s), "");
        // Bend says what its ability does, from the data.
        let ability = crate::tools::bend_ability(&s).unwrap();
        assert_eq!(Tool::Bend.about(&s), s.defs.ability(ability).description);
    }
}

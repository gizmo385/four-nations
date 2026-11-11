//! The legend: the second tab of the key help (`?`, #662), the glyphs on the map in view, each with
//! its name and coloured as on the map, grouped as people, plants, terrain, buildings and marks.
//!
//! It is built from the same data as the agent surface's legend (`Host::look`, `crate::surface`):
//! [`cell_look`] decides what each tile shows, [`style_cell`] says how it draws at this palette's
//! colour depth and glyph set (so `--ascii` is honoured for free), and the ground's and marks'
//! names are `surface::describe_ground` and `surface::describe_mark`. Only the names of the
//! things on top differ: the agent's legend names the very unit ("settler (Haru)"), this one names
//! the kind of thing ("Settler"), because it lists a glyph once however many tiles wear it.

use std::collections::BTreeMap;

use fn_sim::Pos;
use fn_ui::Session;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::color::Palette;
use crate::map::{CellLook, Mark, Overlays, Thing, cell_look, style_cell};
use crate::state::UiState;
use crate::surface::{describe_ground, describe_mark};
use crate::widgets::Tone;

/// Where the legend lists a glyph, in the order it lists the groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegendGroup {
    People,
    Plants,
    Terrain,
    Buildings,
    Marks,
}

impl LegendGroup {
    pub const ALL: [LegendGroup; 5] =
        [Self::People, Self::Plants, Self::Terrain, Self::Buildings, Self::Marks];

    pub fn title(self) -> &'static str {
        match self {
            Self::People => "People",
            Self::Plants => "Plants",
            Self::Terrain => "Terrain",
            Self::Buildings => "Buildings and goods",
            Self::Marks => "Marks",
        }
    }
}

/// One glyph of the legend: how a tile draws it on the map, and what to call it.
#[derive(Clone, Debug, PartialEq)]
pub struct LegendEntry {
    pub group: LegendGroup,
    pub glyph: char,
    /// The glyph's colours and weight as the map draws it.
    pub style: Style,
    pub name: String,
}

/// A row of the legend page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LegendLine {
    Heading(LegendGroup),
    Blank,
    /// Indices into the entries, one per column.
    Cells(Vec<usize>),
}

/// What `thing` is called in the legend, the group it is listed in, and its look with everything
/// that varies tile to tile (selected, how many are inside, which way a rail turns) set to the
/// plain case. `None` for bare ground, which [`entries`] lists as terrain.
fn describe(
    session: &Session,
    palette: &Palette,
    thing: &Thing,
) -> Option<(LegendGroup, String, Thing)> {
    let defs = &session.defs;
    let nation = session.nation();
    let building = |b| defs.building(b).display_name(nation).to_owned();
    let resource = |r, suffix: &str| {
        let def = defs.resource(r);
        let plant = palette.glyphs.resource.get(&def.id).is_some_and(|look| look.plant);
        let group = if plant { LegendGroup::Plants } else { LegendGroup::Terrain };
        (group, format!("{}{suffix}", def.name))
    };
    Some(match thing {
        Thing::Ground => return None,
        Thing::Avatar { .. } => {
            (LegendGroup::People, "The Avatar".to_owned(), Thing::Avatar { selected: false })
        }
        Thing::Unit { master, .. } => {
            let name = if *master { "Bending master" } else { "Settler" };
            let unit = Thing::Unit { nation, selected: false, master: *master };
            (LegendGroup::People, name.to_owned(), unit)
        }
        // The Build tool's ghost and a building with people in it draw their building's glyph.
        Thing::Ghost { building: b, .. } | Thing::Occupied { building: b, .. } => {
            let plain = Thing::Building { building: *b, rail_mask: None };
            (LegendGroup::Buildings, building(*b), plain)
        }
        Thing::Building { building: b, .. } => {
            let plain = Thing::Building { building: *b, rail_mask: None };
            (LegendGroup::Buildings, building(*b), plain)
        }
        Thing::Site { building: b, .. } => {
            let plain = Thing::Site { building: *b, rail_mask: None };
            (LegendGroup::Buildings, format!("{} (site)", building(*b)), plain)
        }
        Thing::GateStage(id) => {
            (LegendGroup::Buildings, "Harmony Gate".to_owned(), Thing::GateStage(id.clone()))
        }
        Thing::Plot(stage) => {
            let name = if *stage == fn_sim::world::PlotStage::Ripe { "Ripe crop" } else { "Crop" };
            (LegendGroup::Buildings, name.to_owned(), thing.clone())
        }
        Thing::Item(i) => (LegendGroup::Buildings, defs.item(*i).name.clone(), thing.clone()),
        Thing::Flame => (LegendGroup::Buildings, "Flame".to_owned(), Thing::Flame),
        Thing::EarthTent => (LegendGroup::Buildings, "Earth tent".to_owned(), Thing::EarthTent),
        Thing::AirShield => (LegendGroup::Buildings, "Air shield".to_owned(), Thing::AirShield),
        Thing::Sapling => (LegendGroup::Plants, "Sapling".to_owned(), Thing::Sapling),
        Thing::Resource(r) => {
            let (group, name) = resource(*r, "");
            (group, name, thing.clone())
        }
        Thing::RestingResource(r) => {
            let (group, name) = resource(*r, " (resting)");
            (group, name, thing.clone())
        }
    })
}

/// `name` with its first letter in upper case ("mountain" is "Mountain").
fn capital(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

/// Every kind of thing the map in view shows, each once, grouped and then by name. The marks are the
/// tints and the cursor on those tiles, drawn over the ground they were first seen on.
pub fn entries(session: &Session, ui: &UiState, palette: &Palette) -> Vec<LegendEntry> {
    let defs = &session.defs;
    let over = Overlays::gather(session, ui);
    let (cols, rows) = ui.viewport();
    // Keyed by group and name, so which entries there are (and how many rows they make) never
    // depends on the glyph set: two looks of one name (the gate's stages, a mark over this ground
    // or that) are listed once, as the first tile in view wears them.
    let mut found: BTreeMap<(LegendGroup, String), LegendEntry> = BTreeMap::new();
    let mut add = |group: LegendGroup, name: String, look: CellLook| {
        let cell = style_cell(&look, defs, palette);
        found.entry((group, name.clone())).or_insert(LegendEntry {
            group,
            glyph: cell.glyph,
            style: cell.style,
            name,
        });
    };
    for dy in 0..rows {
        for dx in 0..cols {
            let pos = Pos::new(ui.view.x + dx, ui.view.y + dy);
            let Some(cell) = cell_look(session, ui, palette, &over, pos) else { continue };
            match describe(session, palette, &cell.thing) {
                Some((group, name, thing)) => {
                    let look = CellLook { thing, ground: cell.ground, marks: Vec::new() };
                    add(group, name, look);
                }
                None => {
                    let name = capital(&describe_ground(session, cell.ground));
                    let look =
                        CellLook { thing: Thing::Ground, ground: cell.ground, marks: vec![] };
                    add(LegendGroup::Terrain, name, look);
                }
            }
            for mark in &cell.marks {
                // The Avatar's own tile colour is his entry's, not a mark of its own.
                if *mark == Mark::AvatarAccent {
                    continue;
                }
                let name = capital(&describe_mark(mark));
                let look =
                    CellLook { thing: Thing::Ground, ground: cell.ground, marks: vec![*mark] };
                add(LegendGroup::Marks, name, look);
            }
        }
    }
    found.into_values().collect()
}

/// How wide a legend cell is: the glyph, a space and the longest name.
fn cell_width(entries: &[LegendEntry]) -> usize {
    2 + entries.iter().map(|e| e.name.chars().count()).max().unwrap_or(0)
}

/// The space between two columns of cells.
const GAP: usize = 3;

/// The legend as rows `width` cells wide: each group's heading, then its entries filling rows
/// left to right in columns as wide as the longest name, and a blank row between groups.
pub fn layout(entries: &[LegendEntry], width: usize) -> Vec<LegendLine> {
    let per_row = ((width + GAP) / (cell_width(entries) + GAP)).max(1);
    let mut out = Vec::new();
    for group in LegendGroup::ALL {
        let members: Vec<usize> =
            (0..entries.len()).filter(|&i| entries[i].group == group).collect();
        if members.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(LegendLine::Blank);
        }
        out.push(LegendLine::Heading(group));
        out.extend(members.chunks(per_row).map(|row| LegendLine::Cells(row.to_vec())));
    }
    out
}

/// How many rows the legend has at `width`.
pub fn line_count(session: &Session, ui: &UiState, palette: &Palette, width: usize) -> usize {
    layout(&entries(session, ui, palette), width).len()
}

/// Draws the legend's rows from row `start` down `body`.
pub fn draw(
    session: &Session,
    ui: &UiState,
    palette: &Palette,
    start: usize,
    body: Rect,
    buf: &mut Buffer,
) -> usize {
    let entries = entries(session, ui, palette);
    let lines = layout(&entries, usize::from(body.width));
    let width = cell_width(&entries);
    let text = Tone::Text.style(palette);
    for (y, line) in (body.y..body.bottom()).zip(lines.iter().skip(start)) {
        match line {
            LegendLine::Blank => {}
            LegendLine::Heading(group) => {
                let heading = group.title().to_uppercase();
                buf.set_stringn(
                    body.x,
                    y,
                    &heading,
                    usize::from(body.width),
                    Tone::Title.style(palette),
                );
            }
            LegendLine::Cells(row) => {
                for (i, &e) in row.iter().enumerate() {
                    let x = body.x + (i * (width + GAP)) as u16;
                    let entry = &entries[e];
                    buf.set_string(x, y, entry.glyph.to_string(), entry.style);
                    buf.set_stringn(x + 2, y, &entry.name, width - 2, text);
                }
            }
        }
    }
    lines.len()
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;
    use crate::color::ColorDepth;

    fn setup(ascii: bool) -> (Session, UiState, Palette) {
        let session = crate::new_session(7, Nation::Earth);
        let palette = Palette::new(&session.defs, Nation::Earth, ColorDepth::TrueColor, ascii);
        let ui = UiState::new(&session, (120, 40));
        (session, ui, palette)
    }

    #[test]
    fn the_legend_names_what_is_in_view_once_each_and_in_order() {
        let (session, ui, palette) = setup(false);
        let found = entries(&session, &ui, &palette);
        let group = |g| found.iter().filter(move |e| e.group == g).collect::<Vec<_>>();
        let settler = group(LegendGroup::People);
        assert!(settler.iter().any(|e| e.name == "Settler" && e.glyph == '☺'), "{settler:?}");
        assert!(!group(LegendGroup::Terrain).is_empty(), "{found:?}");
        assert!(group(LegendGroup::Marks).iter().any(|e| e.name == "Cursor"), "{found:?}");
        let mut keys: Vec<_> = found.iter().map(|e| (e.group, e.name.clone())).collect();
        let sorted = {
            let mut k = keys.clone();
            k.sort();
            k
        };
        assert_eq!(keys, sorted, "groups in order, then by name");
        keys.dedup();
        assert_eq!(keys.len(), found.len(), "each name once");
    }

    /// The glyph and colours are what the map's own `style_cell` gives it, `--ascii` included.
    #[test]
    fn the_legend_wears_the_maps_glyphs_and_honours_ascii() {
        let (session, ui, palette) = setup(true);
        let found = entries(&session, &ui, &palette);
        let settler = found.iter().find(|e| e.name == "Settler").expect("a settler in view");
        assert_eq!(settler.glyph, '@');
        let unicode = setup(false);
        let all = entries(&unicode.0, &unicode.1, &unicode.2);
        assert!(all.iter().any(|e| !e.glyph.is_ascii()), "the Unicode set has non-ASCII glyphs");
        assert!(found.iter().all(|e| e.glyph.is_ascii()), "{found:?}");
    }

    #[test]
    fn the_layout_groups_under_headings_and_fits_its_width() {
        let (session, ui, palette) = setup(false);
        let found = entries(&session, &ui, &palette);
        for width in [20, 40, 116] {
            let lines = layout(&found, width);
            let cell = cell_width(&found);
            for line in &lines {
                if let LegendLine::Cells(row) = line {
                    assert!(row.len() * cell + (row.len() - 1) * GAP <= width.max(cell), "{width}");
                }
            }
            let headings: Vec<_> = lines
                .iter()
                .filter_map(|l| if let LegendLine::Heading(g) = l { Some(*g) } else { None })
                .collect();
            let mut sorted = headings.clone();
            sorted.sort();
            assert_eq!(headings, sorted);
            let shown: usize =
                lines.iter().map(|l| if let LegendLine::Cells(r) = l { r.len() } else { 0 }).sum();
            assert_eq!(shown, found.len(), "every entry is on some row at {width}");
        }
    }
}

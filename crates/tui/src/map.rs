//! The map: what each tile looks like. One glyph and one background per tile, chosen
//! independently, top to bottom (docs/design/tui.md, "What draws on top"):
//!
//! * Glyph: unit > the Build tool's ghost > building or build site > item stack (ground or
//!   stored) > lit flame > resource > terrain or biome.
//! * Background: cursor > the Build tool's ghost (#212), green where the building fits and red
//!   where something is in the way > the rectangle being dragged (#98), in its tool's hue >
//!   designation tint (harvest, bend, build, stockpile) > wet patch > the warmth of the flame
//!   under the cursor > terrain or biome.
//!
//! A building or site covers its whole footprint (#196, docs/design/buildings.md, "TUI"), so a
//! 2x2 forge is a 2x2 block of its glyph, and so is its ghost.

use std::collections::{BTreeMap, BTreeSet};

use fn_sim::building::BuildingId;
use fn_sim::jobs::Stack;
use fn_sim::{Pos, Terrain};
use fn_ui::Session;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::{Palette, Tint, mix};
use crate::glyphs::{Look, Rgb8};
use crate::state::UiState;

/// Everything about the world's tiles that isn't in the tile map, gathered once per frame so
/// drawing a tile is a handful of lookups.
pub struct Overlays {
    units: BTreeSet<Pos>,
    /// Where the selected unit stands: it draws in reverse video.
    selected: Option<Pos>,
    ground: BTreeMap<Pos, Stack>,
    zones: BTreeSet<Pos>,
    harvest: BTreeSet<Pos>,
    bend: BTreeSet<Pos>,
    /// Every tile of every build site, with the building it will be.
    sites: BTreeMap<Pos, BuildingId>,
    /// Every tile of the Build tool's ghosts, with the building and whether it can go there.
    ghost: BTreeMap<Pos, (BuildingId, bool)>,
    flames: BTreeSet<Pos>,
    wet: BTreeSet<Pos>,
    /// The tiles warmed by the flame under the cursor, if the cursor is on one.
    warmed: Option<(Pos, u32)>,
}

impl Overlays {
    pub fn gather(session: &Session, ui: &UiState) -> Self {
        let cursor = ui.cursor;
        let world = &session.world;
        let flames = world.flames();
        Self {
            units: world.units.iter().map(|u| u.pos).collect(),
            selected: ui.selected_unit(session).map(|u| u.pos),
            ground: world.ground_items().collect(),
            zones: world.stockpile_zones().collect(),
            harvest: world.harvest_designations().collect(),
            bend: world.bend_designations().map(|(_, p)| p).collect(),
            sites: world
                .build_sites()
                .flat_map(|(f, b, _)| f.tiles().map(move |p| (p, b)))
                .collect(),
            ghost: ghost_tiles(session, ui),
            flames: flames.iter().map(|f| f.pos).collect(),
            wet: world.wet_tiles().into_iter().map(|(p, _)| p).collect(),
            warmed: flames.iter().find(|f| f.pos == cursor).map(|f| (f.pos, f.radius)),
        }
    }

    fn is_warmed(&self, p: Pos) -> bool {
        // The same square reach as the sim's `HeatSource::warms`.
        self.warmed.is_some_and(|(c, r)| p.x.abs_diff(c.x).max(p.y.abs_diff(c.y)) <= r)
    }
}

/// Every tile of the Build tool's ghosts: free unless something is on it, and every tile of a
/// ghost refused as a whole (for its tech) is blocked, so a red tile always means the click will
/// be refused.
fn ghost_tiles(session: &Session, ui: &UiState) -> BTreeMap<Pos, (BuildingId, bool)> {
    let Some(building) = ui.building else { return BTreeMap::new() };
    let mut tiles = BTreeMap::new();
    for g in ui.ghosts(session) {
        let whole = g.refused.is_some() && g.tiles.iter().all(|(_, b)| b.is_none());
        tiles.extend(g.tiles.iter().map(|&(p, b)| (p, (building, b.is_none() && !whole))));
    }
    tiles
}

/// How one tile draws: its glyph and style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileCell {
    pub glyph: char,
    pub style: Style,
}

/// The terrain or biome look under everything else on `pos`.
fn ground_look<'a>(session: &Session, palette: &'a Palette, pos: Pos) -> Option<&'a Look> {
    let tile = session.world.map.get(pos)?;
    let g = &palette.glyphs;
    match tile.terrain {
        Terrain::Land => g.biome.get(&session.defs.biome(tile.biome).id),
        other => g.terrain(other),
    }
}

/// The tile at `pos` as the terminal shows it, or `None` off the map.
pub fn tile_cell(
    session: &Session,
    ui: &UiState,
    palette: &Palette,
    over: &Overlays,
    pos: Pos,
) -> Option<TileCell> {
    let (world, defs, g) = (&session.world, &session.defs, &palette.glyphs);
    let tile = world.map.get(pos)?;
    let ground = ground_look(session, palette, pos)?;
    let base_bg = ground.bg.expect("terrain and biome entries have a bg (tested)");

    // The glyph and its colour, top to bottom.
    let mut mods = Modifier::empty();
    let (glyph, fg): (char, Rgb8) = if over.units.contains(&pos) {
        mods |= Modifier::BOLD;
        if over.selected == Some(pos) {
            // The selected unit stands out from the others, whatever the colour depth.
            mods |= Modifier::REVERSED;
        }
        (g.unit.glyph(palette.ascii), palette.unit_rgb())
    } else if let Some(&(b, _)) = over.ghost.get(&pos).filter(|&&(_, free)| free || !palette.mono())
    {
        // Without colour a blocked tile keeps the glyph of what is in the way instead.
        let look = &g.building[&defs.building(b).id];
        (look.glyph(palette.ascii), look.fg)
    } else if let Some(b) = world.building_at(pos) {
        let look = &g.building[&defs.building(b).id];
        (look.glyph(palette.ascii), look.fg)
    } else if let Some(&b) = over.sites.get(&pos) {
        // A site is its building, dimmed until it's built.
        let look = &g.building[&defs.building(b).id];
        if palette.mono() {
            mods |= Modifier::DIM;
        }
        (look.glyph(palette.ascii), mix(look.fg, base_bg, 0.5))
    } else if let Some(stack) = over.ground.get(&pos).copied().or_else(|| world.stored_at(pos)) {
        let look = &g.item[&defs.item(stack.item).id];
        (look.glyph(palette.ascii), look.fg)
    } else if over.flames.contains(&pos) {
        let f = &g.overlay.flame;
        (f.glyph(palette.ascii), f.fg)
    } else if let Some(r) = tile.resource {
        let look = &g.resource[&defs.resource(r).id];
        if world.is_sapling(pos) {
            // Planted by an orchard and not yet grown: a sapling, not a resting tree.
            let s = &g.overlay.sapling;
            (s.glyph(palette.ascii), s.fg)
        } else if world.is_resting(pos) {
            // Growing back: the client greys its sprite, so grey the glyph.
            if palette.mono() {
                mods |= Modifier::DIM;
            }
            (look.glyph(palette.ascii), crate::color::to_u8(palette.theme.dim))
        } else {
            (look.glyph(palette.ascii), look.fg)
        }
    } else {
        (ground.glyph(palette.ascii), ground.fg)
    };

    // The background, top to bottom.
    let designation = if over.harvest.contains(&pos) {
        Some(Tint::Harvest)
    } else if over.bend.contains(&pos) {
        Some(Tint::Bend)
    } else if over.sites.contains_key(&pos) {
        Some(Tint::Build)
    } else if over.zones.contains(&pos) {
        Some(Tint::Stockpile)
    } else {
        None
    };
    if designation.is_some() && palette.mono() {
        // Without colour a designated tile is underlined.
        mods |= Modifier::UNDERLINED;
    }
    // The rectangle being dragged shows the tool's hue, stronger than a designation's.
    let dragged = if ui.in_rect(pos) { Tint::for_tool(ui.tool) } else { None };
    if dragged.is_some() && palette.mono() && pos != ui.cursor {
        // Without colour a dragged tile is reversed and dimmed; the cursor is reversed alone.
        mods |= Modifier::REVERSED | Modifier::DIM;
    }
    let ghost = over.ghost.get(&pos).map(|&(_, free)| free);
    if let Some(free) = ghost
        && palette.mono()
    {
        // Without colour the ghost is reversed: bold where it fits, dim where it doesn't.
        mods.remove(Modifier::DIM | Modifier::UNDERLINED);
        mods |= Modifier::REVERSED | if free { Modifier::BOLD } else { Modifier::DIM };
    }
    let bg = if let Some(free) = ghost {
        palette.ghost(base_bg, free)
    } else if let Some(tint) = dragged {
        palette.dragged(base_bg, tint)
    } else if let Some(tint) = designation {
        palette.tinted(base_bg, tint)
    } else if over.wet.contains(&pos) {
        palette.tinted(base_bg, Tint::Wet)
    } else if over.is_warmed(pos) {
        palette.tinted(base_bg, Tint::Warm)
    } else {
        palette.color(base_bg)
    };

    let mut style = Style::new().fg(palette.color(fg)).bg(bg).add_modifier(mods);
    if pos == ui.cursor {
        if ghost.is_some() && !palette.mono() {
            // Over the ghost the cursor keeps the tile's green or red, so a 1x1 ghost still shows.
            style = style.fg(palette.color([0, 0, 0])).add_modifier(Modifier::BOLD);
        } else {
            // The cursor takes the whole look but keeps the glyph (and bold/dim) it covers.
            style = style.patch(palette.cursor_style());
        }
    }
    Some(TileCell { glyph, style })
}

/// Draws the tiles in view into `area`. Cells past the edge of the map stay blank.
pub fn draw_map(session: &Session, ui: &UiState, palette: &Palette, area: Rect, buf: &mut Buffer) {
    let over = Overlays::gather(session, ui);
    for row in 0..area.height {
        for col in 0..area.width {
            let pos = Pos::new(ui.view.x + i32::from(col), ui.view.y + i32::from(row));
            let Some(cell) = tile_cell(session, ui, palette, &over, pos) else { continue };
            let out = &mut buf[(area.x + col, area.y + row)];
            out.set_char(cell.glyph);
            out.set_style(cell.style);
        }
    }
}

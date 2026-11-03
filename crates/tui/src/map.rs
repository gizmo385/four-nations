//! The map: what each tile looks like. One glyph and one background per tile, chosen
//! independently, top to bottom (docs/design/tui.md, "What draws on top"):
//!
//! * Glyph: unit > building or build site > item stack (ground or stored) > lit flame >
//!   resource > terrain or biome.
//! * Background: cursor > the rectangle being dragged (#98), in its tool's hue > designation tint
//!   (harvest, bend, build, stockpile) and the cast highlight share that tint > wet patch > an
//!   aura carrier's own warmth > the warmth of the flame or aura under the cursor > terrain or
//!   biome.
//!
//! **The moment of a bend** (#278; the client's `cast.rs` plays a short animation of this same
//! moment): while a unit channels, its People line and the selection bar say so and how long is
//! left (`panel::channel_text`); for a short while after the bend lands, the tiles it changed
//! flash the nation's accent ([`HIGHLIGHTS`], mirroring the client's `cast::LOOKS`); and a unit
//! carrying Inner Fire's aura (`World::glows`) wears a warm background for as long as it lasts,
//! with its reach tinted the way a lit flame's is while the cursor sits on it.

use std::collections::{BTreeMap, BTreeSet};

use fn_sim::jobs::Stack;
use fn_sim::world::{BendOutcome, Moved, TICKS_PER_SECOND};
use fn_sim::{Pos, Terrain};
use fn_ui::Session;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::{Palette, Tint, mix};
use crate::glyphs::{Look, Rgb8};
use crate::state::UiState;

/// Which tiles light up after a bend lands, for an ability of `data/bending.toml`. Every
/// ability's highlight wears the same look (the nation's accent, `Tint::Bend`); only the tiles
/// it covers differ, so this says that and nothing else.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    /// Just the tile the bend landed on.
    Target,
    /// The 3x3 patch around it (Water the Land's wet patch).
    Patch,
    /// The tile, and the stockpile tile a stack moved to, if any (Wind Carry).
    Moved,
}

/// The highlight shape of each ability, by its id in `data/bending.toml`. Mirrors the client's
/// `cast::LOOKS`: one table that names every ability, checked by a test below.
const HIGHLIGHTS: &[(&str, Shape)] = &[
    ("raise_earth", Shape::Target),
    ("water_the_land", Shape::Patch),
    ("hearthflame", Shape::Target),
    ("wind_carry", Shape::Moved),
    ("inner_fire", Shape::Target),
    ("bore_tunnel", Shape::Target),
    ("seismic_sense", Shape::Target),
];

/// The highlight shape of ability `id`, if it has one.
fn shape_of(id: &str) -> Option<Shape> {
    HIGHLIGHTS.iter().find(|(a, _)| *a == id).map(|(_, s)| *s)
}

/// The tiles `shape` covers for a bend that landed on `pos`, having moved `moved` (Wind Carry).
fn shape_tiles(shape: Shape, pos: Pos, moved: Option<Moved>) -> Vec<Pos> {
    match shape {
        Shape::Target => vec![pos],
        Shape::Patch => (-1..=1)
            .flat_map(|dy| (-1..=1).map(move |dx| Pos::new(pos.x + dx, pos.y + dy)))
            .collect(),
        Shape::Moved => std::iter::once(pos).chain(moved.map(|m| m.to)).collect(),
    }
}

/// How long a bend's highlight lasts after it lands, in world ticks: long enough to read once at
/// 1x play, and paced on the world clock (not wall time) so a paused game holds it still and
/// `--frame` is deterministic.
const HIGHLIGHT_TICKS: u64 = TICKS_PER_SECOND as u64 / 2;

/// The tiles a bend changed, lit for [`HIGHLIGHT_TICKS`] after `World::bend_events` says it
/// landed; the events themselves are the only state this needs, so nothing is carried frame to
/// frame.
fn cast_highlights(session: &Session) -> BTreeSet<Pos> {
    let (world, defs) = (&session.world, &session.defs);
    let now = world.tick;
    world
        .bend_events()
        .iter()
        .rev()
        .take_while(|e| now.saturating_sub(e.tick) < HIGHLIGHT_TICKS)
        .filter(|e| e.outcome == BendOutcome::Done)
        .filter_map(|e| {
            shape_of(&defs.ability(e.ability).id).map(|s| shape_tiles(s, e.pos, e.moved))
        })
        .flatten()
        .collect()
}

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
    sites: BTreeMap<Pos, fn_sim::building::BuildingId>,
    flames: BTreeSet<Pos>,
    wet: BTreeSet<Pos>,
    /// Tiles a bend changed lately: flash the nation's accent (#278).
    cast: BTreeSet<Pos>,
    /// Tiles a unit carrying an aura (Inner Fire, #278) stands on: always a warm background.
    aura: BTreeSet<Pos>,
    /// The tiles warmed by the flame or aura under the cursor, if the cursor is on one.
    warmed: Option<(Pos, u32)>,
}

impl Overlays {
    pub fn gather(session: &Session, ui: &UiState) -> Self {
        let cursor = ui.cursor;
        let world = &session.world;
        let flames = world.flames();
        let glows = world.glows();
        Self {
            units: world.units.iter().map(|u| u.pos).collect(),
            selected: ui.selected_unit(session).map(|u| u.pos),
            ground: world.ground_items().collect(),
            zones: world.stockpile_zones().collect(),
            harvest: world.harvest_designations().collect(),
            bend: world.bend_designations().map(|(_, p)| p).collect(),
            sites: world.build_sites().map(|(f, b, _)| (f.anchor, b)).collect(),
            flames: flames.iter().map(|f| f.pos).collect(),
            wet: world.wet_tiles().into_iter().map(|(p, _)| p).collect(),
            cast: cast_highlights(session),
            aura: glows.iter().map(|g| g.pos).collect(),
            warmed: flames
                .iter()
                .find(|f| f.pos == cursor)
                .map(|f| (f.pos, f.radius))
                .or_else(|| glows.iter().find(|g| g.pos == cursor).map(|g| (g.pos, g.radius))),
        }
    }

    fn is_warmed(&self, p: Pos) -> bool {
        // The same square reach as the sim's `HeatSource::warms`.
        self.warmed.is_some_and(|(c, r)| p.x.abs_diff(c.x).max(p.y.abs_diff(c.y)) <= r)
    }
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
    } else if over.bend.contains(&pos) || over.cast.contains(&pos) {
        // A bend being designated and one that just landed wear the same accent; their moments
        // never overlap (a landed bend's designation is gone).
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
    let bg = if let Some(tint) = dragged {
        palette.dragged(base_bg, tint)
    } else if let Some(tint) = designation {
        palette.tinted(base_bg, tint)
    } else if over.wet.contains(&pos) {
        palette.tinted(base_bg, Tint::Wet)
    } else if over.aura.contains(&pos) {
        // An aura carrier wears its own warmth, cursor or not.
        palette.tinted(base_bg, Tint::Warm)
    } else if over.is_warmed(pos) {
        palette.tinted(base_bg, Tint::Warm)
    } else {
        palette.color(base_bg)
    };

    let mut style = Style::new().fg(palette.color(fg)).bg(bg).add_modifier(mods);
    if pos == ui.cursor {
        // The cursor takes the whole look but keeps the glyph (and bold/dim) it covers.
        style = style.patch(palette.cursor_style());
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

#[cfg(test)]
mod tests {
    use fn_sim::Defs;
    use fn_sim::jobs::ItemId;

    use super::*;

    #[test]
    fn every_ability_has_a_highlight_and_the_table_names_only_abilities() {
        let defs = Defs::bundled();
        for ability in &defs.abilities {
            assert!(shape_of(&ability.id).is_some(), "{} has no highlight shape", ability.id);
        }
        for (id, _) in HIGHLIGHTS {
            assert!(defs.ability_id(id).is_some(), "{id} is not an ability in data/bending.toml");
        }
        assert_eq!(HIGHLIGHTS.len(), defs.abilities.len());
    }

    #[test]
    fn a_target_shape_is_just_the_landed_tile() {
        let pos = Pos::new(5, 5);
        assert_eq!(shape_tiles(Shape::Target, pos, None), vec![pos]);
    }

    #[test]
    fn a_patch_shape_is_the_3x3_block_around_it() {
        let pos = Pos::new(5, 5);
        let tiles = shape_tiles(Shape::Patch, pos, None);
        assert_eq!(tiles.len(), 9);
        for dy in -1..=1 {
            for dx in -1..=1 {
                assert!(tiles.contains(&Pos::new(pos.x + dx, pos.y + dy)), "{dx},{dy}");
            }
        }
    }

    #[test]
    fn a_moved_shape_is_the_landed_tile_and_where_the_stack_went() {
        let pos = Pos::new(5, 5);
        assert_eq!(shape_tiles(Shape::Moved, pos, None), vec![pos], "nothing to carry it to");
        let moved = Moved { to: Pos::new(9, 5), item: ItemId(0), count: 3 };
        assert_eq!(shape_tiles(Shape::Moved, pos, Some(moved)), vec![pos, moved.to]);
    }
}

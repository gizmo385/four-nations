//! The map: what each tile looks like. One glyph and one background per tile, chosen
//! independently, top to bottom (docs/design/tui.md, "What draws on top"):
//!
//! * Glyph: unit > the Build tool's ghost (#212) > building or build site > item stack (ground
//!   or stored) > lit flame > resource > terrain or biome.
//! * Background: cursor > the Build tool's ghost (#212), green where the building fits and red
//!   where something is in the way > the rectangle being dragged (#98), in its tool's hue >
//!   designation tint (harvest, bend, build, stockpile) and the cast highlight share that tint >
//!   wet patch > an aura carrier's own warmth > the warmth of the flame or aura under the cursor >
//!   terrain or biome.
//!
//! A building or a site covers its whole footprint (#196, docs/design/buildings.md, "TUI"), so a
//! 2x2 forge is a 2x2 block of its glyph, and so is its ghost.
//!
//! **Units inside a building** (#226, #229) aren't drawn on the map at all, including on the
//! door tile their `pos` still names: a building with anyone inside shows how many as a digit on
//! its front-middle footprint cell, in its own colour, in place of its glyph there.
//!
//! **The moment of a bend** (#278; the client's `cast.rs` plays a short animation of this same
//! moment): while a unit channels, its People line and the selection bar say so and how long is
//! left (`panel::channel_text`); for a short while after the bend lands, the tiles it changed
//! flash the nation's accent ([`HIGHLIGHTS`], mirroring the client's `cast::LOOKS`); and a unit
//! carrying Inner Fire's aura (`World::glows`) wears a warm background for as long as it lasts,
//! with its reach tinted the way a lit flame's is while the cursor sits on it.

use std::collections::{BTreeMap, BTreeSet};

use fn_sim::building::BuildingId;
use fn_sim::geom::Footprint;
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
    /// Every tile of every build site, with the building it will be (#212).
    sites: BTreeMap<Pos, BuildingId>,
    /// Every tile of the Build tool's ghosts, with the building and whether it can go there
    /// (#212).
    ghost: BTreeMap<Pos, (BuildingId, bool)>,
    flames: BTreeSet<Pos>,
    wet: BTreeSet<Pos>,
    /// Tiles a bend changed lately: flash the nation's accent (#278).
    cast: BTreeSet<Pos>,
    /// Tiles a unit carrying an aura (Inner Fire, #278) stands on: always a warm background.
    aura: BTreeSet<Pos>,
    /// The tiles warmed by the flame or aura under the cursor, if the cursor is on one.
    warmed: Option<(Pos, u32)>,
    /// The front-middle footprint cell of every building with anyone inside (#226, #229),
    /// with how many: drawn as a digit (`+` above 9) in place of the building's own glyph there.
    occupants: BTreeMap<Pos, u32>,
}

impl Overlays {
    pub fn gather(session: &Session, ui: &UiState) -> Self {
        let cursor = ui.cursor;
        let world = &session.world;
        let flames = world.flames();
        let glows = world.glows();
        Self {
            // A unit that is inside a building (#226) isn't drawn on the map: its `pos` still
            // names the door it went in by, but `occupants` (below) covers it instead.
            units: world.units.iter().filter(|u| u.inside().is_none()).map(|u| u.pos).collect(),
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
            cast: cast_highlights(session),
            aura: glows.iter().map(|g| g.pos).collect(),
            warmed: flames
                .iter()
                .find(|f| f.pos == cursor)
                .map(|f| (f.pos, f.radius))
                .or_else(|| glows.iter().find(|g| g.pos == cursor).map(|g| (g.pos, g.radius))),
            occupants: occupant_counts(world),
        }
    }

    fn is_warmed(&self, p: Pos) -> bool {
        // The same square reach as the sim's `HeatSource::warms`.
        self.warmed.is_some_and(|(c, r)| p.x.abs_diff(c.x).max(p.y.abs_diff(c.y)) <= r)
    }
}

/// The ring tile below the middle of the bottom row, same side as `Footprint::entrance` but one
/// row up, inside the footprint itself (docs/design/buildings.md, "TUI"): where the occupant
/// count draws, since the door tile is outside the building and may have units waiting on it.
fn front_middle(foot: Footprint) -> Pos {
    Pos::new(foot.entrance().x, foot.anchor.y + i32::from(foot.h) - 1)
}

/// Every building with anyone inside, by its front-middle cell, with how many (#226, #229).
fn occupant_counts(world: &fn_sim::World) -> BTreeMap<Pos, u32> {
    let mut by_anchor: BTreeMap<Pos, u32> = BTreeMap::new();
    for unit in &world.units {
        if let Some(anchor) = unit.inside() {
            *by_anchor.entry(anchor).or_insert(0) += 1;
        }
    }
    by_anchor
        .into_iter()
        .filter_map(|(anchor, n)| world.footprint_at(anchor).map(|foot| (front_middle(foot), n)))
        .collect()
}

/// Every tile of the Build tool's ghosts (#212): free unless something is on it, and every tile
/// of a ghost refused as a whole (its tech, say) is blocked too, so a red tile always means the
/// click there would be refused.
fn ghost_tiles(session: &Session, ui: &UiState) -> BTreeMap<Pos, (BuildingId, bool)> {
    let ghosts = ui.ghosts(session);
    let Some(building) = ui.ghost_building(session) else { return BTreeMap::new() };
    let mut tiles = BTreeMap::new();
    for g in ghosts {
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
        // Without colour a blocked ghost tile keeps the glyph of what is in the way instead
        // (below), so the player can still tell what they'd be building over.
        let look = &g.building[&defs.building(b).id];
        (look.glyph(palette.ascii), look.fg)
    } else if let Some(b) = world.building_at(pos) {
        let look = &g.building[&defs.building(b).id];
        match over.occupants.get(&pos) {
            // The front-middle cell of a building with anyone inside (#226, #229) shows how
            // many as a digit (`+` above 9), in the building's own colour, in place of its
            // glyph; `--ascii` draws the same digit, already plain ASCII.
            Some(&n) if n > 0 => ((if n > 9 { '+' } else { char::from(b'0' + n as u8) }), look.fg),
            _ => (look.glyph(palette.ascii), look.fg),
        }
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
    // The Build tool's ghost (#212), green where the building fits and red where it doesn't,
    // outranks everything below it: it has to read clearly over a designation or a drag.
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
        if ghost.is_some() && !palette.mono() {
            // Over the ghost the cursor keeps the tile's green or red instead of its own colour,
            // so a 1x1 ghost still shows which it is; the glyph goes black for contrast.
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

#[cfg(test)]
mod tests {
    use fn_sim::Defs;
    use fn_sim::building::Building;
    use fn_sim::jobs::ItemId;

    use super::*;
    use crate::color::ColorDepth;

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

    /// A 2x2 study hall (#226) with both its seats filled draws the digit `2`, in the hall's own
    /// colour, on its front-middle footprint cell -- not on the door, where neither scholar is
    /// drawn any more (#229). `--frame` would print the same digit there.
    #[test]
    fn a_full_study_hall_shows_its_occupant_count_on_the_front_middle_cell() {
        use fn_sim::Nation;
        let mut s = crate::new_session(7, Nation::Earth);
        let hall = s.defs.building_id("study_hall").unwrap();
        let passable = s.defs.building(hall).passable;
        let at = Pos::new(30, 30);
        for dy in 0..3 {
            for dx in 0..3 {
                let t = s.world.map.get_mut(Pos::new(at.x + dx, at.y + dy)).unwrap();
                t.terrain = Terrain::Land;
                t.resource = None;
            }
        }
        let foot = Footprint::new(at, 2, 2);
        for p in foot.tiles() {
            let part = [(p.x - at.x) as u8, (p.y - at.y) as u8];
            let b = Building { part, size: [2, 2], ..Building::new(hall, passable, false) };
            s.world.map.get_mut(p).unwrap().building = Some(b);
        }
        let door = foot.entrance();
        s.world.units[0].pos = door;
        s.world.units[1].pos = door;
        assert!(s.world.enter_building(0, at, &s.defs));
        assert!(s.world.enter_building(1, at, &s.defs));

        let mut ui = UiState::new(&s, (120, 40));
        ui.view = Pos::new(at.x - 2, at.y - 2);
        let palette = Palette::new(&s.defs, Nation::Earth, ColorDepth::TrueColor, false);
        let area = Rect::new(0, 0, 10, 10);
        let mut buf = Buffer::empty(area);
        draw_map(&s, &ui, &palette, area, &mut buf);

        let count_cell = front_middle(foot);
        let (col, row) = (count_cell.x - ui.view.x, count_cell.y - ui.view.y);
        let out = &buf[(area.x + col as u16, area.y + row as u16)];
        assert_eq!(out.symbol(), "2");
        let look = &palette.glyphs.building[&s.defs.building(hall).id];
        assert_eq!(out.fg, palette.color(look.fg));
        assert_ne!(out.symbol(), &look.glyph(false).to_string(), "not the hall's own glyph there");

        // The door, where the two scholars `pos` still names, shows no one standing on it.
        let (dc, dr) = (door.x - ui.view.x, door.y - ui.view.y);
        let door_out = &buf[(area.x + dc as u16, area.y + dr as u16)];
        assert_ne!(door_out.symbol(), &palette.glyphs.unit.glyph(false).to_string());

        // `--ascii` draws the same digit: it is already plain ASCII.
        let ascii = Palette::new(&s.defs, Nation::Earth, ColorDepth::TrueColor, true);
        let mut ascii_buf = Buffer::empty(area);
        draw_map(&s, &ui, &ascii, area, &mut ascii_buf);
        let ascii_out = &ascii_buf[(area.x + col as u16, area.y + row as u16)];
        assert_eq!(ascii_out.symbol(), "2");
    }
}

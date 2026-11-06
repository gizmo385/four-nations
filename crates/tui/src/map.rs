//! The map: what each tile looks like. One glyph and one background per tile, chosen
//! independently, top to bottom (docs/design/tui.md, "What draws on top"):
//!
//! * Glyph: unit > the Build tool's ghost (#212) > building or build site > item stack (ground
//!   or stored) > lit flame > resource > terrain or biome.
//! * Background: cursor > a ghost's own door tile (#230), amber free or red blocked > the Build
//!   tool's ghost (#212), green where the building fits and red where something is in the way >
//!   the rectangle being dragged (#98), in its tool's hue > designation tint (harvest, a
//!   harvest's standing order a shade greener, #277, a Clear mark in its own hue, #411/#426,
//!   bend, build, stockpile) and the cast highlight share that tint > wet patch > an aura
//!   carrier's own warmth > the warmth of the flame or aura under the cursor > terrain or biome.
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
//!
//! **Rail** (#279, mirrors the client's `buildings::links`, #135): a rail tile or site with a
//! rail neighbour (built or still a site) draws the box-drawing piece that joins the neighbours
//! it has, so a line of rail reads as one track and a corner turns; a lone tile keeps the rail
//! building's own glyph.
//!
//! **The Avatar and the Harmony Gate** (#237, #242, `docs/design/endgame.md`): the Avatar draws
//! as `@` in white, on the nation's accent instead of his tile's own background, wherever he
//! stands, outranking the usual unit look. The gate's site draws its own 4x4 glyph block by
//! stage ([`gate_stage_look`], `[gate_stage.*]` in `data/tui.toml`), in place of the generic
//! dimmed site every other building's gets; once it is finished it is an ordinary building with
//! its own glyph (`[building.harmony_gate]`).
//!
//! **One source for the screen and an agent** (#313, #314): [`cell_look`] decides the precedence
//! above and returns a [`CellLook`] -- a [`Thing`] (what's on top), a [`Ground`] (the terrain or
//! biome under it) and the [`Mark`]s on it -- as data ids, not glyphs or colours. [`style_cell`]
//! turns that into a [`TileCell`] for a given [`Palette`], but never re-derives which thing or
//! mark applies. `fn_tui::surface`'s `look` (#315) reads the same `cell_look`, so the map an
//! agent sees can't show anything the screen hides, or hide anything it shows.

use std::collections::{BTreeMap, BTreeSet};

use fn_sim::bending::AbilityId;
use fn_sim::building::BuildingId;
use fn_sim::defs::{BiomeId, ResourceId};
use fn_sim::geom::Footprint;
use fn_sim::jobs::{ItemId, Stack};
use fn_sim::world::{BendOutcome, Moved, PlotStage, TICKS_PER_SECOND};
use fn_sim::{Defs, Nation, Pos, Terrain};
use fn_ui::build::Placement;
use fn_ui::{Session, Tool};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::{Palette, Tint, mix};
use crate::glyphs::{Glyphs, Look, OverlayGlyph, Rgb8, link_glyph, rail_mask};
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
    ("glide", Shape::Target),
    ("metalbend", Shape::Target),
    ("air_scooter", Shape::Target),
    ("earth_tent", Shape::Target),
    ("air_shield", Shape::Target),
    ("weld", Shape::Target),
    ("lift", Shape::Target),
    ("draw_water", Shape::Target),
    ("cut_ice", Shape::Target),
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
    /// Every tile with a unit standing on it (not inside a building), with whichever nation
    /// draws there (#248): where more than one unit shares a tile, the selected one wins if
    /// it's among them, else whichever `World::units` lists last.
    units: BTreeMap<Pos, Nation>,
    /// Where the selected unit stands: it draws in reverse video.
    selected: Option<Pos>,
    /// Where the Avatar stands, if he is here (#237, #242): he draws as `@` in white on the
    /// nation's accent, in place of a unit's usual look (`Unit::is_avatar`).
    avatar: Option<Pos>,
    ground: BTreeMap<Pos, Stack>,
    zones: BTreeSet<Pos>,
    harvest: BTreeSet<Pos>,
    /// The subset of `harvest` with a standing order on it (#81, #277): tinted apart from a
    /// one-off harvest, even while the resource rests and has no active job
    /// (`World::harvest_designations`'s own doc).
    standing: BTreeSet<Pos>,
    /// Farm plots with a Sow job designated on them (#198): tinted the same as `harvest`.
    sowing: BTreeSet<Pos>,
    /// Tiles with a Clear mark (docs/design/clearing.md, #411/#426), in its own tint apart from
    /// `harvest` and `standing`.
    clearing: BTreeSet<Pos>,
    bend: BTreeSet<Pos>,
    /// Every tile of every build site, with the building it will be (#212).
    sites: BTreeMap<Pos, BuildingId>,
    /// Every tile of the Build tool's ghosts, with the building and whether it can go there
    /// (#212).
    ghost: BTreeMap<Pos, (BuildingId, bool)>,
    /// Each ghost's own door tile, and whether it's free to use (#230). Keyed on position like
    /// `ghost`, so a drag's later stamps win where two doors would land on the same tile.
    door: BTreeMap<Pos, bool>,
    /// Every lit flame or shelter, with which ability lit it (#362): `None` only for a save from
    /// before the split, which can only have meant Hearthflame.
    flames: BTreeMap<Pos, Option<AbilityId>>,
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
        let ghosts = ui.ghosts(session);
        Self {
            // A unit that is inside a building (#226) isn't drawn on the map: its `pos` still
            // names the door it went in by, but `occupants` (below) covers it instead. The
            // selected unit's own nation wins where it shares a tile with others (#248), so
            // selecting a newcomer always shows its own colour there.
            units: {
                let mut units: BTreeMap<Pos, Nation> = world
                    .units
                    .iter()
                    .filter(|u| u.inside().is_none())
                    .map(|u| (u.pos, world.nation_of(u)))
                    .collect();
                if let Some(sel) = ui.selected_unit(session).filter(|u| u.inside().is_none()) {
                    units.insert(sel.pos, world.nation_of(sel));
                }
                units
            },
            selected: ui.selected_unit(session).map(|u| u.pos),
            avatar: world
                .units
                .iter()
                .find(|u| u.is_avatar() && u.inside().is_none())
                .map(|u| u.pos),
            ground: world.ground_items().collect(),
            zones: world.stockpile_zones().collect(),
            harvest: world.harvest_designations().collect(),
            standing: world
                .harvest_designations()
                .filter(|&p| world.is_repeating_harvest(p))
                .collect(),
            sowing: world.sow_designations().collect(),
            clearing: world.clear_designations().collect(),
            bend: world.bend_designations().map(|(_, p)| p).collect(),
            sites: world
                .build_sites()
                .flat_map(|(f, b, _)| f.tiles().map(move |p| (p, b)))
                .collect(),
            ghost: ghost_tiles(ui.ghost_building(session), &ghosts),
            door: door_tiles(&ghosts),
            flames: flames.iter().map(|f| (f.pos, f.ability)).collect(),
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
/// click there would be refused. `None` outside the Build tool, or with nothing placeable yet.
fn ghost_tiles(
    building: Option<BuildingId>,
    ghosts: &[Placement],
) -> BTreeMap<Pos, (BuildingId, bool)> {
    let Some(building) = building else { return BTreeMap::new() };
    let mut tiles = BTreeMap::new();
    for g in ghosts {
        let whole = g.refused.is_some() && g.tiles.iter().all(|(_, b)| b.is_none());
        tiles.extend(g.tiles.iter().map(|&(p, b)| (p, (building, b.is_none() && !whole))));
    }
    tiles
}

/// Each ghost's own door tile (#230, `Placement::entrance`), for a building that has one
/// (`BuildingDef::capacity` above 0): amber where it's free, red where it's blocked -- another
/// building's door already there, or the tile isn't walkable.
fn door_tiles(ghosts: &[Placement]) -> BTreeMap<Pos, bool> {
    ghosts.iter().filter_map(|g| g.entrance).collect()
}

/// Whether `id` -- a finished building or a build site's building -- is a rail (#279).
fn is_rail(defs: &fn_sim::Defs, id: BuildingId) -> bool {
    defs.building(id).rail.is_some()
}

/// Which `Thing` a lit flame or shelter draws, by the ability that lit it (#362): Earth Tent and
/// Air Shield get their own look apart from the plain flame; everything else -- Hearthflame, and
/// a save from before the split (`ability` is `None`) -- keeps it.
fn flame_thing(ability: Option<AbilityId>, defs: &Defs) -> Thing {
    match ability.map(|a| defs.ability(a).id.as_str()) {
        Some("earth_tent") => Thing::EarthTent,
        Some("air_shield") => Thing::AirShield,
        _ => Thing::Flame,
    }
}

/// The 4x4 glyph block for the Harmony Gate's site at its current stage (#242, `[gate_stage.*]`
/// in `data/tui.toml`): `None` for any other building's site, or while the data names no look
/// for the stage, which keeps the generic dimmed site instead.
fn gate_stage_look<'g>(
    g: &'g Glyphs,
    world: &fn_sim::World,
    defs: &fn_sim::Defs,
    site: BuildingId,
) -> Option<&'g OverlayGlyph> {
    if !defs.building(site).endeavor {
        return None;
    }
    let (_, stage) = world.gate_stage(defs)?;
    g.gate_stage.get(&stage.id)
}

/// Whether `pos` is rail, finished or still a site (#279): the set of tiles a piece of track
/// joins.
fn rail_at(world: &fn_sim::World, defs: &fn_sim::Defs, over: &Overlays, pos: Pos) -> bool {
    world
        .building_at(pos)
        .or_else(|| over.sites.get(&pos).copied())
        .is_some_and(|b| is_rail(defs, b))
}

/// The mask [`rail_mask`] builds for the rail tiles or sites `pos` (a rail building of `kind`)
/// joins: `None` if `kind` isn't a rail building at all, which keeps its own glyph instead of
/// ever looking up a piece. This is `cell_look`'s job, not the painter's, because it reads the
/// neighbours through `over`.
fn rail_mask_at(session: &Session, over: &Overlays, pos: Pos, kind: BuildingId) -> Option<u8> {
    let (world, defs) = (&session.world, &session.defs);
    if !is_rail(defs, kind) {
        return None;
    }
    Some(rail_mask(
        rail_at(world, defs, over, Pos::new(pos.x, pos.y - 1)),
        rail_at(world, defs, over, Pos::new(pos.x + 1, pos.y)),
        rail_at(world, defs, over, Pos::new(pos.x, pos.y + 1)),
        rail_at(world, defs, over, Pos::new(pos.x - 1, pos.y)),
    ))
}

/// How one tile draws: its glyph and style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileCell {
    pub glyph: char,
    pub style: Style,
}

/// The terrain or biome a tile's background shows (#314): a data id, so a legend (#315) can
/// describe it without re-deriving which of the two it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    Terrain(Terrain),
    Biome(BiomeId),
}

/// What draws on top of a tile -- a unit, an item, a resource, a building or nothing, in which
/// case the terrain or biome itself shows -- as a data id (#314, #313): the one thing a player's
/// eye reads there, chosen by [`cell_look`]'s precedence (this module's doc comment) so
/// [`style_cell`] only has to style it, never decide it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Thing {
    /// The Avatar (#237, #242): always this, never a plain unit; `selected` if the panel's own
    /// unit is him.
    Avatar {
        selected: bool,
    },
    /// A settler or bender of `nation`; `selected` if it's the panel's own unit.
    Unit {
        nation: Nation,
        selected: bool,
    },
    /// The Build tool's ghost for `building` (#212): green if it fits here, red if something is
    /// in the way. Without colour a blocked ghost falls through to the next case instead
    /// (`cell_look` only ever returns this when `free`, or the palette has colour).
    Ghost {
        building: BuildingId,
        free: bool,
    },
    /// A building with people inside (#226, #229): its front-middle cell shows how many.
    Occupied {
        building: BuildingId,
        n: u32,
    },
    /// A farm plot's crop, over its own tilled-soil glyph (#198).
    Plot(PlotStage),
    /// A finished building; `rail_mask` is `Some` for a rail tile (#279), the mask of whichever
    /// of its four neighbours ([`rail_mask`]'s bits) are rail too.
    Building {
        building: BuildingId,
        rail_mask: Option<u8>,
    },
    /// The Harmony Gate's site at a stage with its own look (#242), named by the stage's id in
    /// `data/endeavor.toml`.
    GateStage(String),
    /// Any other building's site, dimmed until it's built; `rail_mask` as [`Thing::Building`].
    Site {
        building: BuildingId,
        rail_mask: Option<u8>,
    },
    /// An item stack on the ground or in a stockpile.
    Item(ItemId),
    /// A lit flame (a hearth, a forge) with nothing else drawn on it.
    Flame,
    /// Earth's Earth Tent, lit (#362): a shelter apart from `Flame`.
    EarthTent,
    /// Air's Air Shield, lit (#362): a shelter apart from `Flame`.
    AirShield,
    /// A resource an orchard planted that hasn't grown back yet.
    Sapling,
    /// A resource resting after being harvested, greyed until it regrows.
    RestingResource(ResourceId),
    Resource(ResourceId),
    /// Nothing on top: the terrain or biome itself draws.
    Ground,
}

/// A tint or overlay marking a tile apart from its plain look (#314, #313): everything
/// [`cell_look`] finds true of `pos` besides what's on top of it. More than one may apply (the
/// cursor and a designation, say); [`style_cell`] composes them in a fixed, data-independent
/// order (this module's doc comment), never deciding which facts hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Cursor,
    /// A one-off harvest designation.
    Harvest,
    /// A harvest with a standing order on it (#81, #277): tinted apart from a one-off harvest.
    Standing,
    /// A farm plot with a Sow job designated (#198): the same tint as `Harvest`.
    Sowing,
    /// A Clear mark (docs/design/clearing.md, #411/#426): tinted apart from `Harvest`.
    Clear,
    /// A bend being designated, or one that just landed (the two moments never overlap).
    Bend,
    Build,
    Stockpile,
    /// The rectangle being dragged, in the tool's hue (or the standing-order hue, #277).
    Dragged(Tint),
    /// The Build tool's ghost (#212): green where it fits, red where it doesn't.
    Ghost {
        free: bool,
    },
    /// A ghost's own door tile (#230): amber free, red blocked.
    Door {
        free: bool,
    },
    Wet,
    /// Warmth: an aura carrier's own, or the reach of the flame or aura under the cursor.
    Warm,
    /// The Avatar's own background: the nation's accent, not his tile's own (#237, #242).
    AvatarAccent,
}

/// What `pos` shows: its `thing`, its `ground`, and every mark on it (#314, #313). The shared
/// source for the painter ([`style_cell`]) and, later, an agent's `look` (#315): both read this
/// and nothing closer to `World`, so neither can show what the other hides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellLook {
    pub thing: Thing,
    pub ground: Ground,
    pub marks: Vec<Mark>,
}

/// What `pos` shows, decided once by the precedence this module's doc comment lists: `None` off
/// the map. Pure given `session`, `ui`, `over` and whether `palette` has colour (the one place a
/// blocked Build ghost's glyph depends on it, so the player can see what's in the way without
/// colour to show it some other way).
pub fn cell_look(
    session: &Session,
    ui: &UiState,
    palette: &Palette,
    over: &Overlays,
    pos: Pos,
) -> Option<CellLook> {
    let (world, defs) = (&session.world, &session.defs);
    let tile = world.map.get(pos)?;
    let ground = match tile.terrain {
        Terrain::Land => Ground::Biome(tile.biome),
        other => Ground::Terrain(other),
    };

    // What's on top, top to bottom.
    let thing = if over.avatar == Some(pos) {
        Thing::Avatar { selected: over.selected == Some(pos) }
    } else if let Some(&nation) = over.units.get(&pos) {
        Thing::Unit { nation, selected: over.selected == Some(pos) }
    } else if let Some(&(b, free)) =
        over.ghost.get(&pos).filter(|&&(_, free)| free || !palette.mono())
    {
        Thing::Ghost { building: b, free }
    } else if let Some(b) = world.building_at(pos) {
        match (over.occupants.get(&pos), world.plot_stage(pos)) {
            (Some(&n), _) if n > 0 => Thing::Occupied { building: b, n },
            (_, Some(stage @ (PlotStage::Growing | PlotStage::Ripe))) => Thing::Plot(stage),
            _ => Thing::Building { building: b, rail_mask: rail_mask_at(session, over, pos, b) },
        }
    } else if let Some(&b) = over.sites.get(&pos) {
        // The Harmony Gate's site draws its own 4x4 glyph block by stage (#242); every other
        // site is its building, dimmed until it's built, and a rail site joins the rail beside
        // it (built or still a site) the same as a finished tile does.
        match gate_stage_look(&palette.glyphs, world, defs, b) {
            Some(_) => {
                let (_, stage) = world.gate_stage(defs).expect("a look implies a stage");
                Thing::GateStage(stage.id.clone())
            }
            None => Thing::Site { building: b, rail_mask: rail_mask_at(session, over, pos, b) },
        }
    } else if let Some(stack) = over.ground.get(&pos).copied().or_else(|| world.stored_at(pos)) {
        Thing::Item(stack.item)
    } else if let Some(&ability) = over.flames.get(&pos) {
        flame_thing(ability, defs)
    } else if let Some(r) = tile.resource {
        if world.is_sapling(pos) {
            // Planted by an orchard and not yet grown: a sapling, not a resting tree.
            Thing::Sapling
        } else if world.is_resting(pos) {
            Thing::RestingResource(r)
        } else {
            Thing::Resource(r)
        }
    } else {
        Thing::Ground
    };

    let mut marks = Vec::new();
    if pos == ui.cursor {
        marks.push(Mark::Cursor);
    }
    if over.standing.contains(&pos) {
        // A standing order (#277) is a harvest too, but tinted apart from a one-off one.
        marks.push(Mark::Standing);
    } else if over.harvest.contains(&pos) {
        marks.push(Mark::Harvest);
    } else if over.sowing.contains(&pos) {
        marks.push(Mark::Sowing);
    } else if over.clearing.contains(&pos) {
        marks.push(Mark::Clear);
    } else if over.bend.contains(&pos) || over.cast.contains(&pos) {
        marks.push(Mark::Bend);
    } else if over.sites.contains_key(&pos) {
        marks.push(Mark::Build);
    } else if over.zones.contains(&pos) {
        marks.push(Mark::Stockpile);
    }
    // The rectangle being dragged shows the tool's hue; the Harvest tool's standing-order toggle
    // (`O`, #277) shows in the drag too.
    let drag_tint = if !ui.in_rect(pos) {
        None
    } else if ui.tool == Tool::Harvest && ui.repeating {
        Some(Tint::RepeatingHarvest)
    } else {
        Tint::for_tool(ui.tool)
    };
    if let Some(tint) = drag_tint {
        marks.push(Mark::Dragged(tint));
    }
    // The Build tool's ghost (#212) and its own door tile (#230) mark `pos` independently of
    // whether `thing` ended up being the ghost itself (a blocked ghost without colour falls
    // through to what's in the way, but still marks the tile red).
    if let Some(&(_, free)) = over.ghost.get(&pos) {
        marks.push(Mark::Ghost { free });
    }
    if let Some(&free) = over.door.get(&pos) {
        marks.push(Mark::Door { free });
    }
    if over.wet.contains(&pos) {
        marks.push(Mark::Wet);
    }
    if over.aura.contains(&pos) || over.is_warmed(pos) {
        // An aura carrier wears its own warmth, cursor or not; so does the reach of the flame or
        // aura under the cursor.
        marks.push(Mark::Warm);
    }
    if over.avatar == Some(pos) {
        marks.push(Mark::AvatarAccent);
    }

    Some(CellLook { thing, ground, marks })
}

/// The terrain or biome look `ground` names.
fn ground_look<'a>(ground: Ground, defs: &Defs, g: &'a Glyphs) -> &'a Look {
    match ground {
        Ground::Terrain(t) => g.terrain(t).expect("every non-land terrain has a look (tested)"),
        Ground::Biome(id) => &g.biome[&defs.biome(id).id],
    }
}

/// The glyph and colour `thing` alone draws, and the modifier it alone contributes (the
/// Avatar's and a unit's bold and selected reverse video, a dimmed site or resting resource): no
/// mark has a say in any of it. Split out of [`style_cell`] so a test can check this is the
/// glyph that ends up on the screen, with no mark changing it (#314).
fn thing_look(
    thing: &Thing,
    defs: &Defs,
    palette: &Palette,
    base_bg: Rgb8,
) -> (char, Rgb8, Modifier) {
    let g = &palette.glyphs;
    let (mono, ascii) = (palette.mono(), palette.ascii);
    let mut mods = Modifier::empty();
    let (glyph, fg): (char, Rgb8) = match thing {
        Thing::Avatar { selected } => {
            // The Avatar (#237, #242): always `@` in white, never the nation's own unit look, so
            // he reads as a guest apart from every unit (his own background follows below).
            mods |= Modifier::BOLD;
            if mono || *selected {
                // No colour to set him apart with, so reverse video does, whether selected or
                // not; with colour, only selection does.
                mods |= Modifier::REVERSED;
            }
            (g.avatar.glyph(ascii), g.avatar.fg)
        }
        Thing::Unit { nation, selected } => {
            mods |= Modifier::BOLD;
            if *selected {
                // The selected unit stands out from the others, whatever the colour depth.
                mods |= Modifier::REVERSED;
            }
            // A newcomer draws in its own nation's colour (#248), not necessarily the
            // settlement's own.
            (g.unit.glyph(ascii), palette.unit_rgb_for(defs, *nation))
        }
        Thing::Ghost { building, .. } => {
            let look = &g.building[&defs.building(*building).id];
            (look.glyph(ascii), look.fg)
        }
        Thing::Occupied { building, n } => {
            // The front-middle cell of a building with anyone inside (#226, #229) shows how
            // many as a digit (`+` above 9), in the building's own colour, in place of its
            // glyph; `--ascii` draws the same digit, already plain ASCII.
            let look = &g.building[&defs.building(*building).id];
            let digit = if *n > 9 { '+' } else { char::from(b'0' + *n as u8) };
            (digit, look.fg)
        }
        // A farm plot's own glyph is tilled soil; growing or ripe override it (#198).
        Thing::Plot(PlotStage::Ripe) => {
            let o = &g.overlay.plot_ripe;
            (o.glyph(ascii), o.fg)
        }
        Thing::Plot(_) => {
            let o = &g.overlay.plot_growing;
            (o.glyph(ascii), o.fg)
        }
        Thing::Building { building, rail_mask } => {
            let look = &g.building[&defs.building(*building).id];
            let glyph = rail_mask
                .and_then(|m| link_glyph(&g.rail_track, m, ascii))
                .unwrap_or_else(|| look.glyph(ascii));
            (glyph, look.fg)
        }
        Thing::GateStage(stage_id) => {
            let look = &g.gate_stage[stage_id];
            (look.glyph(ascii), look.fg)
        }
        Thing::Site { building, rail_mask } => {
            let look = &g.building[&defs.building(*building).id];
            if mono {
                mods |= Modifier::DIM;
            }
            let glyph = rail_mask
                .and_then(|m| link_glyph(&g.rail_track, m, ascii))
                .unwrap_or_else(|| look.glyph(ascii));
            (glyph, mix(look.fg, base_bg, 0.5))
        }
        Thing::Item(item) => {
            let look = &g.item[&defs.item(*item).id];
            (look.glyph(ascii), look.fg)
        }
        Thing::Flame => {
            let f = &g.overlay.flame;
            (f.glyph(ascii), f.fg)
        }
        Thing::EarthTent => {
            let o = &g.overlay.earth_tent;
            (o.glyph(ascii), o.fg)
        }
        Thing::AirShield => {
            let o = &g.overlay.air_shield;
            (o.glyph(ascii), o.fg)
        }
        Thing::Sapling => {
            let s = &g.overlay.sapling;
            (s.glyph(ascii), s.fg)
        }
        Thing::RestingResource(r) => {
            // Growing back: the client greys its sprite, so grey the glyph.
            let look = &g.resource[&defs.resource(*r).id];
            if mono {
                mods |= Modifier::DIM;
            }
            (look.glyph(ascii), crate::color::to_u8(palette.theme.dim))
        }
        Thing::Resource(r) => {
            let look = &g.resource[&defs.resource(*r).id];
            (look.glyph(ascii), look.fg)
        }
        Thing::Ground => unreachable!("the caller passes the ground's own look instead"),
    };
    (glyph, fg, mods)
}

/// What `marks` alone contributes: the modifier they add without colour, and the designation,
/// drag, ghost and door facts [`style_cell`]'s background needs, read once. Split out so a test
/// can check this is the modifier that ends up on the screen, on tiles where `thing` adds none
/// of its own (#314).
struct MarksEffect {
    mods: Modifier,
    is_cursor: bool,
    designation: Option<Tint>,
    dragged: Option<Tint>,
    ghost: Option<bool>,
    door: Option<bool>,
}

fn marks_effect(marks: &[Mark], mono: bool) -> MarksEffect {
    let is_cursor = marks.contains(&Mark::Cursor);
    let designation = marks.iter().find_map(|m| match m {
        Mark::Standing => Some(Tint::RepeatingHarvest),
        Mark::Harvest | Mark::Sowing => Some(Tint::Harvest),
        Mark::Clear => Some(Tint::Clear),
        Mark::Bend => Some(Tint::Bend),
        Mark::Build => Some(Tint::Build),
        Mark::Stockpile => Some(Tint::Stockpile),
        _ => None,
    });
    let mut mods = Modifier::empty();
    if designation.is_some() && mono {
        // Without colour a designated tile is underlined; a standing order is bold too, so it
        // still reads apart from a one-off harvest (#277).
        mods |= Modifier::UNDERLINED;
        if designation == Some(Tint::RepeatingHarvest) {
            mods |= Modifier::BOLD;
        }
    }
    let dragged = marks.iter().find_map(|m| match m {
        Mark::Dragged(t) => Some(*t),
        _ => None,
    });
    if dragged.is_some() && mono && !is_cursor {
        // Without colour a dragged tile is reversed and dimmed; the cursor is reversed alone.
        mods |= Modifier::REVERSED | Modifier::DIM;
    }
    let ghost = marks.iter().find_map(|m| match m {
        Mark::Ghost { free } => Some(*free),
        _ => None,
    });
    // A ghost's own door tile (#230) outranks the ghost itself: it never lies inside the
    // footprint that placed it (the door is on the ring), but a tight drag can still land
    // another stamp's footprint on it, and the door is the more specific thing to show there.
    let door = marks.iter().find_map(|m| match m {
        Mark::Door { free } => Some(*free),
        _ => None,
    });
    if let Some(free) = door.or(ghost)
        && mono
    {
        // Without colour a door or the ghost itself is reversed: bold free, dim blocked.
        mods.remove(Modifier::DIM | Modifier::UNDERLINED);
        mods |= Modifier::REVERSED | if free { Modifier::BOLD } else { Modifier::DIM };
    }
    MarksEffect { mods, is_cursor, designation, dragged, ghost, door }
}

/// Styles a [`CellLook`] through `palette`: the colour depth, glyph set and nation theme turn
/// what `cell_look` already decided into what the terminal draws. Never re-derives which thing
/// or mark applies -- that's `cell_look`'s job (`thing_look` and `marks_effect` above) -- only
/// how it looks at this depth.
pub fn style_cell(look: &CellLook, defs: &Defs, palette: &Palette) -> TileCell {
    let g = &palette.glyphs;
    let ascii = palette.ascii;
    let ground = ground_look(look.ground, defs, g);
    let base_bg = ground.bg.expect("terrain and biome entries have a bg (tested)");

    let (glyph, fg, thing_mods) = if look.thing == Thing::Ground {
        (ground.glyph(ascii), ground.fg, Modifier::empty())
    } else {
        thing_look(&look.thing, defs, palette, base_bg)
    };

    // The background, from `marks`, top to bottom (this module's doc comment).
    let marks = marks_effect(&look.marks, palette.mono());
    let mods = thing_mods | marks.mods;
    // The Build tool's ghost (#212), green where the building fits and red where it doesn't,
    // outranks everything below it: it has to read clearly over a designation or a drag.
    let bg = if let Some(free) = marks.door {
        palette.door(base_bg, free)
    } else if let Some(free) = marks.ghost {
        palette.ghost(base_bg, free)
    } else if let Some(tint) = marks.dragged {
        palette.dragged(base_bg, tint)
    } else if let Some(tint) = marks.designation {
        palette.tinted(base_bg, tint)
    } else if look.marks.contains(&Mark::Wet) {
        palette.tinted(base_bg, Tint::Wet)
    } else if look.marks.contains(&Mark::Warm) {
        palette.tinted(base_bg, Tint::Warm)
    } else if look.marks.contains(&Mark::AvatarAccent) {
        // The Avatar's own background (#237, #242): the nation's accent, not his tile's own, so
        // he reads as a guest wherever he stands.
        palette.color(palette.tint_rgb(Tint::Bend))
    } else {
        palette.color(base_bg)
    };

    let mut style = Style::new().fg(palette.color(fg)).bg(bg).add_modifier(mods);
    if marks.is_cursor {
        if (marks.door.is_some() || marks.ghost.is_some()) && !palette.mono() {
            // Over a door or the ghost the cursor keeps the tile's own colour instead of its
            // own, so a 1x1 ghost (or its door) still shows which it is; the glyph goes black
            // for contrast.
            style = style.fg(palette.color([0, 0, 0])).add_modifier(Modifier::BOLD);
        } else {
            // The cursor takes the whole look but keeps the glyph (and bold/dim) it covers.
            style = style.patch(palette.cursor_style());
        }
    }
    TileCell { glyph, style }
}

/// The tile at `pos` as the terminal shows it, or `None` off the map: `cell_look`'s decision,
/// styled by `style_cell`. The painter's one entry point, kept so nothing outside this module
/// has to call the two in order.
pub fn tile_cell(
    session: &Session,
    ui: &UiState,
    palette: &Palette,
    over: &Overlays,
    pos: Pos,
) -> Option<TileCell> {
    let look = cell_look(session, ui, palette, over, pos)?;
    Some(style_cell(&look, &session.defs, palette))
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
    use fn_sim::world::Command;

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

    /// A newcomer of another nation (#240) draws in its own nation's colour, not the
    /// settlement's own (#248): `tile_cell`'s fg matches `Palette::unit_rgb_for` for each.
    #[test]
    fn a_newcomer_draws_in_its_own_nations_colour() {
        use fn_sim::Nation;
        let mut s = crate::new_session(7, Nation::Fire);
        s.world.units[1].nation = Some(Nation::Earth);
        let home_pos = s.world.units[0].pos;
        let newcomer_pos = s.world.units[1].pos;
        assert_ne!(home_pos, newcomer_pos, "distinct spawns");

        let mut ui = UiState::new(&s, (120, 40));
        // Away from both units, so the cursor's own look doesn't override the glyph's colour.
        ui.cursor = Pos::new(home_pos.x - 5, home_pos.y - 5);
        let palette = Palette::new(&s.defs, Nation::Fire, ColorDepth::TrueColor, false);
        let over = Overlays::gather(&s, &ui);

        let home_cell = tile_cell(&s, &ui, &palette, &over, home_pos).unwrap();
        let newcomer_cell = tile_cell(&s, &ui, &palette, &over, newcomer_pos).unwrap();
        assert_eq!(
            home_cell.style.fg,
            Some(palette.color(palette.unit_rgb_for(&s.defs, Nation::Fire)))
        );
        assert_eq!(
            newcomer_cell.style.fg,
            Some(palette.color(palette.unit_rgb_for(&s.defs, Nation::Earth)))
        );
        assert_ne!(home_cell.style.fg, newcomer_cell.style.fg);
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

    /// The Avatar (#237, #242) draws as `@` in white on the nation's accent, never the plain
    /// unit look, whether or not he is selected.
    #[test]
    fn the_avatar_draws_as_white_on_the_accent_not_the_nation_s_unit_look() {
        use fn_sim::Nation;
        use fn_sim::world::Guest;
        let mut s = crate::new_session(7, Nation::Fire);
        s.world.units[0].guest = Some(Guest::Avatar);
        let avatar = s.world.units[0].pos;
        let mut ui = UiState::new(&s, (120, 40));
        // Away from the cursor, which `UiState::new` puts on the first settler (now the
        // Avatar): the cursor's own look would otherwise cover his.
        ui.cursor = Pos::new(avatar.x + 3, avatar.y);
        let palette = Palette::new(&s.defs, Nation::Fire, ColorDepth::TrueColor, false);
        let area = Rect::new(0, 0, ui.viewport().0 as u16, ui.viewport().1 as u16);
        let mut buf = Buffer::empty(area);
        draw_map(&s, &ui, &palette, area, &mut buf);
        let (col, row) = (avatar.x - ui.view.x, avatar.y - ui.view.y);
        let out = &buf[(area.x + col as u16, area.y + row as u16)];
        assert_eq!(out.symbol(), palette.glyphs.avatar.glyph.to_string());
        assert_ne!(out.symbol(), palette.glyphs.unit.glyph.to_string(), "not the plain unit look");
        assert_eq!(out.fg, palette.color(palette.glyphs.avatar.fg));
        assert_eq!(out.bg, palette.color(palette.tint_rgb(Tint::Bend)), "the accent, not the tile");
        assert!(out.modifier.contains(Modifier::BOLD));

        // `--ascii` keeps the same `@` and still sets him apart with reverse video (#242's
        // mono-safe treatment also applies when colour would otherwise do it).
        let ascii = Palette::new(&s.defs, Nation::Fire, ColorDepth::Never, true);
        let mut ascii_buf = Buffer::empty(area);
        draw_map(&s, &ui, &ascii, area, &mut ascii_buf);
        let ascii_out = &ascii_buf[(area.x + col as u16, area.y + row as u16)];
        assert_eq!(ascii_out.symbol(), "@");
        assert!(ascii_out.modifier.contains(Modifier::REVERSED), "mono sets him apart");
    }

    /// A world with the endeavor's gate placed and on `stage_id` (no public command reaches
    /// `Building` short of playing a whole settlement up to Ready, so this round-trips a save
    /// with the gate's state spliced in: `World` derives `Serialize`/`Deserialize` over all of
    /// its fields for exactly this reason, saves).
    fn world_with_gate_stage(stage_id: &str) -> (fn_sim::World, fn_sim::Defs) {
        let defs = fn_sim::Defs::bundled();
        let s = crate::new_session(7, fn_sim::Nation::Earth);
        let text = s.world.save(&defs).unwrap();
        let gate_ron = format!(
            "Building((anchor:(x:10,y:10),ready_tick:0,stage:\"{stage_id}\",delivered:[],\
             progress:0.0,tasks:[],rite_progress:0.0,rite_paused:false))"
        );
        let text = text.replacen("Dormant", &gate_ron, 1);
        (fn_sim::World::load(&text, &defs).expect("round trip"), defs)
    }

    /// The Harmony Gate's site draws its stage's own glyph block (#242), not the generic dimmed
    /// site every other building's gets, and falls back to the generic look for a stage the
    /// data names no look for.
    #[test]
    fn the_gate_site_draws_its_stage_s_own_look() {
        let g = Glyphs::bundled();
        let (world, defs) = world_with_gate_stage("pillars");
        let gate = defs.building_id("harmony_gate").unwrap();
        let other = defs.building_id("bed").unwrap();
        let look = gate_stage_look(&g, &world, &defs, gate).expect("pillars has a look");
        assert_eq!(look.glyph, g.gate_stage["pillars"].glyph);
        // No gate site at all: no look, so the generic fallback applies.
        let dormant = crate::new_session(7, fn_sim::Nation::Earth);
        assert!(gate_stage_look(&g, &dormant.world, &defs, gate).is_none());
        // Any other building's site never looks here up, whatever the gate's own stage.
        assert!(gate_stage_look(&g, &world, &defs, other).is_none());
    }

    /// Land, free of everyone and everything, nearest `from` first: tiles `designate_scenario`
    /// (below) can hand a designation or a ghost without clashing with anything else it set up.
    fn free_land(s: &Session, from: Pos) -> Vec<Pos> {
        let mut land: Vec<Pos> = s
            .world
            .map
            .iter()
            .filter(|(p, t)| {
                t.terrain == Terrain::Land
                    && t.resource.is_none()
                    && t.building.is_none()
                    && !s.world.units.iter().any(|u| u.pos == *p)
            })
            .map(|(p, _)| p)
            .collect();
        land.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
        land
    }

    /// A one-off harvest, a standing order, a stockpile zone and a build site, each on their own
    /// tile near the settlers, plus a dragged Harvest rectangle and a Build ghost with its own
    /// door tile elsewhere: every [`Mark`] this module's doc comment lists except `Wet`, `Warm`
    /// and `AvatarAccent` (those need a bend or an aura, covered by the golden frames instead).
    /// Returns the session, the `UiState` with the drag and the ghost set up, and every tile of
    /// interest by name.
    fn designate_scenario(nation: Nation) -> (Session, UiState, BTreeMap<&'static str, Pos>) {
        let mut s = crate::new_session(7, nation);
        let from = s.world.units[0].pos;
        let mut resources: Vec<Pos> =
            s.world.map.iter().filter(|(_, t)| t.resource.is_some()).map(|(p, _)| p).collect();
        resources.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
        let can_harvest =
            |s: &Session, pos: Pos| s.clone().apply(Command::DesignateHarvest { pos }).is_ok();
        let harvest = *resources.iter().find(|&&p| can_harvest(&s, p)).unwrap();
        let standing = *resources.iter().find(|&&p| p != harvest && can_harvest(&s, p)).unwrap();
        s.apply(Command::DesignateHarvest { pos: harvest }).unwrap();
        s.apply(Command::DesignateRepeatingHarvest { pos: standing }).unwrap();

        let land = free_land(&s, from);
        let zone = land[0];
        s.apply(Command::DesignateStockpile { pos: zone }).unwrap();
        let site = land[1];
        let bed = s.defs.building_id("bed").unwrap();
        s.apply(Command::DesignateBuild { pos: site, building: bed }).unwrap();
        let dragged = land[2];

        let mut ui = UiState::new(&s, (120, 40));
        ui.view = Pos::new((from.x - 20).max(0), (from.y - 20).max(0));
        ui.tool = Tool::Harvest;
        ui.anchor = Some(dragged);
        // Away from `dragged` itself, so that tile is inside the rectangle without being the
        // cursor tile too (the cursor's own styling, not a mark, would otherwise cover it).
        ui.cursor = Pos::new(dragged.x + 3, dragged.y + 3);

        let mut tiles = BTreeMap::new();
        tiles.insert("harvest", harvest);
        tiles.insert("standing", standing);
        tiles.insert("stockpile", zone);
        tiles.insert("build_site", site);
        tiles.insert("dragged", dragged);
        (s, ui, tiles)
    }

    /// For every cell of [`designate_scenario`] (every nation), the glyph `thing_look` (or the
    /// ground's own look, for [`Thing::Ground`]) computes from `cell_look`'s `thing` alone, with
    /// `--ascii` or without, is the glyph that ends up in the frame -- no mark ever changes it
    /// (#314's acceptance criteria).
    #[test]
    fn a_cells_glyph_comes_from_its_thing_alone() {
        for nation in Nation::ALL {
            let (s, ui, _) = designate_scenario(nation);
            let over = Overlays::gather(&s, &ui);
            for ascii in [false, true] {
                for depth in [ColorDepth::TrueColor, ColorDepth::Never] {
                    let palette = Palette::new(&s.defs, nation, depth, ascii);
                    for dy in -8..8 {
                        for dx in -8..8 {
                            let pos = Pos::new(ui.cursor.x + dx, ui.cursor.y + dy);
                            let Some(look) = cell_look(&s, &ui, &palette, &over, pos) else {
                                continue;
                            };
                            let ground = ground_look(look.ground, &s.defs, &palette.glyphs);
                            let base_bg = ground.bg.unwrap();
                            let want = if look.thing == Thing::Ground {
                                ground.glyph(ascii)
                            } else {
                                thing_look(&look.thing, &s.defs, &palette, base_bg).0
                            };
                            let got = style_cell(&look, &s.defs, &palette).glyph;
                            assert_eq!(want, got, "{nation:?} {depth:?} ascii={ascii} {pos:?}");
                        }
                    }
                }
            }
        }
    }

    /// For [`designate_scenario`]'s harvest, standing, stockpile and dragged tiles -- each a bare
    /// resource or plain ground, so `thing` contributes no modifier of its own -- the modifier
    /// `marks_effect` computes for a mono palette is the whole of the rendered cell's modifier
    /// (#314's acceptance criteria). The build site is left out: its own `Thing::Site` is dimmed
    /// in mono too, so it isn't a tile where marks alone account for the modifier.
    #[test]
    fn marks_alone_account_for_a_mono_cells_modifier_where_the_thing_adds_none() {
        let (s, ui, tiles) = designate_scenario(Nation::Earth);
        let over = Overlays::gather(&s, &ui);
        let palette = Palette::new(&s.defs, Nation::Earth, ColorDepth::Never, false);
        for name in ["harvest", "standing", "stockpile", "dragged"] {
            let pos = tiles[name];
            let look = cell_look(&s, &ui, &palette, &over, pos).expect("on the map");
            assert!(
                matches!(look.thing, Thing::Resource(_) | Thing::Ground),
                "{name}: {:?}",
                look.thing
            );
            let want = marks_effect(&look.marks, true).mods;
            let got = style_cell(&look, &s.defs, &palette).style.add_modifier;
            assert_eq!(want, got, "{name} ({pos:?}): {:?}", look.marks);
        }
    }

    /// The Build tool's ghost and its own door tile (#212, #230) mark a tile independently of
    /// whether `thing` ended up being the ghost (a blocked one without colour falls through to
    /// what's in the way, #314's doc comment): `cell_look` always marks a free ghost `Ghost` and
    /// its door `Door`, and neither adds its own modifier, so `marks_effect` alone accounts for
    /// the mono cell's modifier here too.
    #[test]
    fn a_build_ghost_and_its_door_mark_the_cell_independently_of_the_thing() {
        let s = crate::new_session(7, Nation::Earth);
        let from = s.world.units[0].pos;
        let study_hall = s.defs.building_id("study_hall").unwrap();
        // The nearest free cursor spot with a clear 2x2 placement and a free door beside it.
        let cursor = free_land(&s, from)
            .into_iter()
            .find(|&p| {
                let placements = fn_ui::build::ghosts(&s, None, p, study_hall);
                placements.len() == 1
                    && placements[0].refused.is_none()
                    && placements[0].entrance.is_some_and(|(_, free)| free)
            })
            .expect("a clear placement with a free door nearby");

        let mut ui = UiState::new(&s, (120, 40));
        ui.view = Pos::new((from.x - 20).max(0), (from.y - 20).max(0));
        ui.tool = Tool::Build;
        ui.choice = Some(study_hall);
        ui.cursor = cursor;

        let over = Overlays::gather(&s, &ui);
        let palette = Palette::new(&s.defs, Nation::Earth, ColorDepth::Never, false);
        let ghost_pos = *over.ghost.keys().next().expect("a ghost preview");
        let ghost_look = cell_look(&s, &ui, &palette, &over, ghost_pos).expect("on the map");
        assert!(matches!(ghost_look.thing, Thing::Ghost { free: true, .. }));
        assert!(ghost_look.marks.contains(&Mark::Ghost { free: true }));
        assert_eq!(
            marks_effect(&ghost_look.marks, true).mods,
            style_cell(&ghost_look, &s.defs, &palette).style.add_modifier,
            "ghost at {ghost_pos:?}"
        );

        let door_pos = *over.door.keys().next().expect("a door tile");
        let door_look = cell_look(&s, &ui, &palette, &over, door_pos).expect("on the map");
        assert!(door_look.marks.contains(&Mark::Door { free: true }));
        assert_eq!(
            marks_effect(&door_look.marks, true).mods,
            style_cell(&door_look, &s.defs, &palette).style.add_modifier,
            "door at {door_pos:?}"
        );
    }

    /// Earth Tent draws its own glyph apart from a plain flame's (#362): the AC's "each shelter
    /// has a glyph" is `flame_thing` picking `Thing::EarthTent` for it instead of `Thing::Flame`.
    #[test]
    fn a_lit_earth_tent_draws_its_own_glyph_not_a_plain_flames() {
        use fn_sim::Nation;
        let mut s = crate::new_session(2, Nation::Earth);
        let tech = s.defs.tech_id("earth_shaping").unwrap();
        s.apply(Command::SetResearch { tech }).unwrap();
        for _ in 0..60_000 {
            if s.world.research().is_known(tech) {
                break;
            }
            s.world.tick(&s.defs);
        }
        assert!(s.world.research().is_known(tech), "researched");

        let pos = s.world.units[0].pos;
        s.world.units[0].chi = s.defs.chi.max;
        let ability = s.defs.ability_id("earth_tent").unwrap();
        s.apply(Command::Bend { ability, pos }).expect("a unit's own tile validates");
        for _ in 0..20_000 {
            s.world.tick(&s.defs);
            if s.world.is_lit(pos) {
                break;
            }
        }
        assert!(s.world.is_lit(pos), "the tent should finish raising");
        // Off the tent's own tile, so the unit standing there doesn't outrank its glyph
        // (`cell_look`'s own precedence: a unit draws over a lit flame or shelter).
        s.world.units[0].pos = Pos::new(pos.x + 3, pos.y);

        let mut ui = UiState::new(&s, (120, 40));
        ui.cursor = Pos::new(pos.x + 10, pos.y + 10);
        let palette = Palette::new(&s.defs, Nation::Earth, ColorDepth::TrueColor, false);
        let over = Overlays::gather(&s, &ui);
        let look = cell_look(&s, &ui, &palette, &over, pos).expect("on the map");
        assert_eq!(look.thing, Thing::EarthTent);
        let (glyph, fg, _) = thing_look(&look.thing, &s.defs, &palette, [0, 0, 0]);
        assert_eq!(glyph, palette.glyphs.overlay.earth_tent.glyph);
        assert_eq!(fg, palette.glyphs.overlay.earth_tent.fg);
        assert_ne!(glyph, palette.glyphs.overlay.flame.glyph, "apart from a plain flame's glyph");
    }
}

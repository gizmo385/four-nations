//! The build tool's data (issue #111): which buildings the picker lists and which are locked,
//! the building chosen to place, how a rejected command reads, and what a construction site or a
//! finished building says in the tooltip. Pure functions over `Session`, so the tests cover them;
//! the frontends draw the picker and `tools::apply_rect` turns a drag into `DesignateBuild`.
//!
//! The Cancel tool withdraws a build designation too: on a tile with a site it issues
//! `CancelBuild`, on a stockpile tile `CancelStockpile`, elsewhere `CancelHarvest`.
//!
//! A building can cover several tiles (its footprint, #196, docs/design/buildings.md,
//! "Placing"): `anchor_for` turns the tile under the cursor into the footprint's top-left tile,
//! which is what `DesignateBuild` names, and `placement` says which of its tiles are blocked,
//! for both frontends to draw the same ghost.

use fn_sim::building::{BuildingDef, BuildingId};
use fn_sim::geom::Footprint;
use fn_sim::jobs::ItemId;
use fn_sim::world::{Blocker, BuildError, CommandError, DesignateError, Task, Unit, UnitId};
use fn_sim::{Pos, Terrain};

use crate::session::Session;

/// One line of the building picker.
#[derive(Clone, PartialEq, Debug)]
pub struct PickerRow {
    pub id: BuildingId,
    /// The nation's name for it (`BuildingDef::display_name`).
    pub name: String,
    /// "5 Wood", "10 Wood, 2 Stone" or "free"; a stone cost reads "10 Stone (or brick)" for a
    /// nation that can make brick (issue #371).
    pub cost: String,
    /// "20s work".
    pub work: String,
    /// "Requires Fire Making" when the nation hasn't unlocked it; `None` when it can be placed.
    pub requires: Option<String>,
}

impl PickerRow {
    pub fn locked(&self) -> bool {
        self.requires.is_some()
    }
}

/// The item that stands in for `item` in a cost, if the current nation could ever make it
/// (issue #371: "N Stone (or brick)" only shows for a nation that can make brick).
fn stand_in_for_nation(session: &Session, item: ItemId) -> Option<ItemId> {
    let stand_in = session.defs.stand_in_for(item)?;
    session.defs.nation_can_make(session.world.nation, stand_in).then_some(stand_in)
}

/// How much of `item` is in stores, plus its stand-in's if the nation can make one (issue #371):
/// what a cost's "in stores" figure should count.
fn stored_with_stand_in(session: &Session, item: ItemId) -> u32 {
    let mut total = session.world.stored_total(item);
    if let Some(stand_in) = stand_in_for_nation(session, item) {
        total += session.world.stored_total(stand_in);
    }
    total
}

/// One cost entry's item name, with its stand-in named too where one applies: "Stone" or
/// "Stone (or brick)".
fn cost_item_text(session: &Session, item: ItemId) -> String {
    let name = &session.defs.item(item).name;
    match stand_in_for_nation(session, item) {
        Some(stand_in) => {
            format!("{name} (or {})", session.defs.item(stand_in).name.to_lowercase())
        }
        None => name.clone(),
    }
}

/// What a building costs, as "5 Wood" (comma-separated for several items), followed by its
/// size if it covers more than one tile ("10 Wood, 2x2"). A cost item that a stand-in may pay
/// (brick for stone, issue #371) names it: "10 Stone (or brick)".
fn cost_text(session: &Session, def: &BuildingDef) -> String {
    let mut parts: Vec<String> = def
        .cost
        .iter()
        .map(|c| format!("{} {}", c.count, cost_item_text(session, c.item)))
        .collect();
    if parts.is_empty() {
        parts.push("free".into());
    }
    let (w, h) = def.footprint;
    if (w, h) != (1, 1) {
        parts.push(format!("{w}x{h}"));
    }
    parts.join(", ")
}

/// The anchor (top-left tile) of a `footprint`-sized building placed at the cursor: odd sizes
/// centre on the cursor, even ones put it top-left of the middle, so a 2x2 grows down and to the
/// right of the pointer instead of under it.
pub fn anchor_for(cursor: Pos, footprint: (u8, u8)) -> Pos {
    let (w, h) = (i32::from(footprint.0), i32::from(footprint.1));
    Pos::new(cursor.x - (w - 1) / 2, cursor.y - (h - 1) / 2)
}

/// Where a building would go for the cursor, and what is in the way on each of its tiles.
#[derive(Clone, PartialEq, Debug)]
pub struct Placement {
    pub footprint: Footprint,
    /// Every footprint tile, row-major, with what blocks it (`World::build_blocker`, plus
    /// `Blocker::Entrance` for a tile that would bury another building's door, #230).
    pub tiles: Vec<(Pos, Option<Blocker>)>,
    /// Why designating it would be refused, if it would: the tech it still needs, else the
    /// first blocked tile (`World::check_build`).
    pub refused: Option<BuildError>,
    /// The footprint's own door (`Footprint::entrance`) and whether it's free to use, if the
    /// building can be entered (`BuildingDef::capacity` above 0, #226). `None` for one that
    /// can't, like a wall or a bed: it has no door for the ghost to mark (#230).
    pub entrance: Option<(Pos, bool)>,
}

impl Placement {
    pub fn ok(&self) -> bool {
        self.refused.is_none()
    }
}

/// The placement of `building` with the cursor on `cursor`, for the frontends' ghost.
pub fn placement(session: &Session, cursor: Pos, building: BuildingId) -> Placement {
    let footprint = session.defs.building(building).footprint;
    placement_at(session, anchor_for(cursor, footprint), building)
}

/// The placement of `building` with its anchor (top-left tile) on `anchor`.
pub fn placement_at(session: &Session, anchor: Pos, building: BuildingId) -> Placement {
    let world = &session.world;
    let defs = &session.defs;
    let def = defs.building(building);
    let (w, h) = def.footprint;
    let footprint = Footprint::new(anchor, w, h);
    let tiles = footprint
        .tiles()
        .map(|p| {
            let blocker = world
                .build_blocker(p)
                .or_else(|| world.door_owner(p, defs).is_some().then_some(Blocker::Entrance));
            (p, blocker)
        })
        .collect();
    let refused = world.check_build(anchor, building, defs).err();
    let entrance = (def.capacity > 0)
        .then(|| (footprint.entrance(), world.door_walkable(footprint.entrance())));
    Placement { footprint, tiles, refused, entrance }
}

/// The ghosts the Build tool shows (#212): with no corner set, the one placement under the
/// cursor; while dragging from `corner`, one per stamp `tools::build_stamps` would place. Each
/// is judged against the world as it is, not against the stamps before it, so a chain of sites
/// that would collide if placed one after another all draw as they'd fare right now.
pub fn ghosts(
    session: &Session,
    corner: Option<Pos>,
    cursor: Pos,
    building: BuildingId,
) -> Vec<Placement> {
    let Some(corner) = corner else { return vec![placement(session, cursor, building)] };
    let footprint = session.defs.building(building).footprint;
    crate::tools::build_stamps(corner, cursor, footprint)
        .into_iter()
        .map(|anchor| placement_at(session, anchor, building))
        .collect()
}

/// Why a building can't go on `at`, as a reason for the notice: "an oak tree is in the way",
/// "on a stockpile".
fn describe_blocker(session: &Session, at: Pos, why: Blocker) -> String {
    let world = &session.world;
    let tile = world.map.get(at);
    let a = |name: &str| {
        let name = name.to_lowercase();
        let article = if name.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
        format!("{article} {name} is in the way")
    };
    match why {
        Blocker::Resource => match tile.and_then(|t| t.resource) {
            Some(r) => a(&session.defs.resource(r).name),
            None => why.to_string(),
        },
        Blocker::Building => match world.building_at(at) {
            Some(b) => a(session.defs.building(b).display_name(world.nation)),
            None => why.to_string(),
        },
        Blocker::Site => "a building is already planned there".into(),
        Blocker::Stockpile => "on a stockpile".into(),
        Blocker::Terrain => match tile.map(|t| t.terrain) {
            Some(Terrain::Mountain) => "can't build on a mountain".into(),
            Some(Terrain::Ford) => "can't build on a ford".into(),
            Some(_) => "can't build on water".into(),
            None => why.to_string(),
        },
        Blocker::OffMap => why.to_string(),
        // Either `at` covers another building's or site's door (#226), named here so the
        // Cursor line reads "would block the Academy's door" (#230), or it's this footprint's
        // own door and isn't walkable, which `Blocker::Display` already says enough about.
        Blocker::Entrance => match world.door_owner(at, &session.defs) {
            Some(b) => {
                format!(
                    "would block {}'s door",
                    session.defs.building(b).display_name(world.nation)
                )
            }
            None => why.to_string(),
        },
        // The word is already in the sim's `Blocker::Beside` text ("must touch rock", #373):
        // `BuildingDef::beside_word` picked it when `check_build` refused the placement.
        Blocker::Beside(_) => why.to_string(),
    }
}

/// Whether the current nation could ever build `def` from the picker: not if the tech that unlocks
/// it belongs to another nation's branch (`TechDef::nation`), which the nation can never research,
/// nor the endeavor's gate, which the Endeavor screen places (#236).
fn available_to_nation(session: &Session, def: &BuildingDef) -> bool {
    !def.endeavor
        && def
            .unlocked_by
            .and_then(|t| session.defs.tech(t).nation)
            .is_none_or(|n| n == session.world.nation)
}

/// Every building the current nation can build or may yet unlock, in file order, by its name for
/// them, with cost and work, and the tech it still needs if it's locked. Buildings behind
/// another nation's tech are left out (issue #137), and so is the endeavor's gate (#236).
pub fn picker_rows(session: &Session) -> Vec<PickerRow> {
    let nation = session.world.nation;
    session
        .defs
        .buildings
        .iter()
        .enumerate()
        .filter(|(_, def)| available_to_nation(session, def))
        .map(|(i, def)| PickerRow {
            id: BuildingId(i as u16),
            name: def.display_name(nation).to_owned(),
            cost: cost_text(session, def),
            work: format!("{:.0}s work", def.work),
            requires: def
                .unlocked_by
                .filter(|&t| !session.world.is_unlocked(Some(t)))
                .map(|t| format!("Requires {}", session.defs.tech(t).name)),
        })
        .collect()
}

/// `current` if it can be placed, else the first building that can. `None` only if every
/// building is locked.
pub fn valid_choice(rows: &[PickerRow], current: Option<BuildingId>) -> Option<BuildingId> {
    let open = |id: BuildingId| rows.iter().any(|r| r.id == id && !r.locked());
    current.filter(|&id| open(id)).or_else(|| rows.iter().find(|r| !r.locked()).map(|r| r.id))
}

/// The next building after `current` that can be placed, wrapping round.
pub fn next_choice(rows: &[PickerRow], current: Option<BuildingId>) -> Option<BuildingId> {
    let start = current.and_then(|c| rows.iter().position(|r| r.id == c)).map_or(0, |i| i + 1);
    (0..rows.len()).map(|n| &rows[(start + n) % rows.len()]).find(|r| !r.locked()).map(|r| r.id)
}

/// A refused command's reason for the HUD notice. The sim's own text for a tech gate is "not
/// researched yet"; a frontend knows the tech's name, so it says which ("requires Fire Making").
pub fn describe_error(session: &Session, e: CommandError) -> String {
    match e {
        CommandError::Build(BuildError::NotResearched(t))
        | CommandError::Designate(DesignateError::NotResearched(t)) => {
            format!("requires {}", session.defs.tech(t).name)
        }
        CommandError::Build(BuildError::CannotBuild { at, why }) => {
            describe_blocker(session, at, why)
        }
        other => other.to_string(),
    }
}

/// What the tooltip says about a construction site.
#[derive(Clone, PartialEq, Debug)]
pub struct SiteReport {
    pub name: String,
    /// "Materials: 3/5 Wood".
    pub materials: String,
    /// "Work: 4/20s".
    pub work: String,
}

/// The pending build on `pos` (any tile of its footprint), if any, with its materials delivered / needed and work progress.
pub fn site_report(session: &Session, pos: Pos) -> Option<SiteReport> {
    let (id, delivered, progress) = session.world.build_site(pos)?;
    let def = session.defs.building(id);
    let materials = if def.cost.is_empty() {
        "Materials: none needed".to_owned()
    } else {
        let parts: Vec<String> = def
            .cost
            .iter()
            .zip(delivered)
            .map(|(c, d)| format!("{d}/{} {}", c.count, session.defs.item(c.item).name))
            .collect();
        format!("Materials: {}", parts.join(", "))
    };
    Some(SiteReport {
        name: def.display_name(session.world.nation).to_owned(),
        materials,
        work: format!("Work: {:.0}/{:.0}s", progress.min(def.work), def.work),
    })
}

/// The name of the finished building on `pos` (any tile of its footprint), if any.
pub fn built_name(session: &Session, pos: Pos) -> Option<String> {
    let id = session.world.building_at(pos)?;
    Some(session.defs.building(id).display_name(session.world.nation).to_owned())
}

/// One line of a building's occupants list (`docs/design/buildings.md`, "Units inside
/// buildings", #226, #229): the unit and the row that selects it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OccupantRow {
    pub id: UnitId,
    pub name: String,
    /// A short, lower-case word for what it's doing inside: "studying" while it holds a study
    /// seat, "sleeping" once beds are places inside too (#227), "healing" while mending in a
    /// healing building (#375), "idle" with nothing claimed. A need pre-empts a seat by sending
    /// the unit out the door first (`world/study.rs`), so an occupant is never doing anything
    /// else while it's inside.
    pub task: &'static str,
}

impl OccupantRow {
    /// "Mai, studying", for the side panel and the selection bar.
    pub fn text(&self) -> String {
        format!("{}, {}", self.name, self.task)
    }
}

/// What an occupant's row says it's doing ([`OccupantRow::task`]).
fn occupant_task(unit: &Unit) -> &'static str {
    match unit.task() {
        Some(Task::Study(_)) => "studying",
        Some(Task::Sleep) => "sleeping",
        Some(Task::Healing(_)) => "healing",
        _ => "idle",
    }
}

/// The occupants of the building on `pos` (any tile of its footprint), in unit-id order
/// (`World::occupants`); empty off a building, or on one with nobody inside.
pub fn occupant_rows(session: &Session, pos: Pos) -> Vec<OccupantRow> {
    let world = &session.world;
    let Some(anchor) = world.footprint_at(pos).map(|f| f.anchor) else { return Vec::new() };
    world
        .occupants(anchor)
        .filter_map(|id| world.unit(id))
        .map(|u| OccupantRow { id: u.id, name: u.name.clone(), task: occupant_task(u) })
        .collect()
}

/// What the Build modal's detail pane says about one building (#285): its name for the nation,
/// its cost against what the stockpiles hold now, its footprint, what it does once built, and
/// the tech it still needs if it's locked. Pure text: the frontend draws it (and looks up its
/// glyph, which lives in the frontend's own glyph table, not here).
#[derive(Clone, PartialEq, Debug)]
pub struct BuildingDetail {
    pub name: String,
    /// "5 Wood (12 in stores)" per item costed, or "free" for a building with none. A stone
    /// cost reads "10 Stone (or brick) (12 in stores)" for a nation that can make brick, and the
    /// figure counts both (issue #371).
    pub cost: Vec<String>,
    /// "1x1" or "2x2" (`BuildingDef::footprint`).
    pub footprint: String,
    /// What it does once finished: a line per fact the data carries (study seats, a bed, a kept
    /// fire, a rail, an orchard). Capacity (#226) isn't here yet; add it once `BuildingDef`
    /// carries it. "Nothing beyond shelter" for a building with none of those.
    pub effects: Vec<String>,
    /// "Requires Fire Making" when the nation hasn't unlocked it; `None` when it can be placed.
    pub requires: Option<String>,
}

/// [`BuildingDetail`] for `id`, for the Build modal's detail pane.
pub fn building_detail(session: &Session, id: BuildingId) -> BuildingDetail {
    let def = session.defs.building(id);
    let cost = if def.cost.is_empty() {
        vec!["free".to_owned()]
    } else {
        def.cost
            .iter()
            .map(|c| {
                let have = stored_with_stand_in(session, c.item);
                format!("{} {} ({have} in stores)", c.count, cost_item_text(session, c.item))
            })
            .collect()
    };
    let (w, h) = def.footprint;
    let mut effects = Vec::new();
    if def.seats > 0 {
        let seats = def.seats;
        effects.push(format!("Study hall: {seats} seat{}", if seats == 1 { "" } else { "s" }));
    }
    // Capacity defaults to seats (#226), so a hall's seat line already says it.
    if def.capacity > 0 && def.capacity != def.seats {
        effects.push(format!("Room for {} inside, through the door", def.capacity));
    }
    if let Some(bed) = &def.bed {
        effects.push(format!(
            "Bed: sleeps {:.1}x faster, sought within {} tiles",
            bed.sleep_bonus, bed.seek_tiles
        ));
    }
    if let Some(heat) = &def.heat {
        effects.push(format!(
            "Keeps a fire: warms {} tiles, sleeps {:.1}x faster nearby",
            heat.radius, heat.sleep_bonus
        ));
    }
    if let Some(rail) = &def.rail {
        effects.push(format!("Rail: hauling runs {:.1}x faster along it", rail.speed));
    }
    if let Some(orchard) = &def.orchard {
        let plants = &session.defs.resource(orchard.plants).name;
        effects.push(format!("Tends up to {} {plants} trees nearby", orchard.max_trees));
    }
    if let Some(farm) = &def.farm {
        let crop = &session.defs.item(farm.crop).name;
        effects.push(format!(
            "Sow ({:.0}s), grows {crop} ({:.0}s), yields {} on reap ({:.0}s)",
            farm.sow_seconds, farm.grow_seconds, farm.harvest_count, farm.reap_seconds
        ));
    }
    if effects.is_empty() {
        effects.push("Nothing beyond shelter".to_owned());
    }
    let requires = def
        .unlocked_by
        .filter(|&t| !session.world.is_unlocked(Some(t)))
        .map(|t| format!("Requires {}", session.defs.tech(t).name));
    BuildingDetail {
        name: def.display_name(session.world.nation).to_owned(),
        cost,
        footprint: format!("{w}x{h}"),
        effects,
        requires,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fn_sim::world::Command;
    use fn_sim::{Nation, Terrain};

    fn session(nation: Nation) -> Session {
        Session::new(7, nation, 64, 64)
    }

    /// Some tile a 1x1 building can go on: unoccupied and, since #226, with room on the ground
    /// below it for its door.
    fn free_tile(session: &Session) -> Pos {
        let bed = session.defs.building_id("bed").unwrap();
        session
            .world
            .map
            .iter()
            .find(|(p, t)| {
                t.terrain == Terrain::Land
                    && t.resource.is_none()
                    && !session.world.units.iter().any(|u| u.pos == *p)
                    && session.world.check_build(*p, bed, &session.defs).is_ok()
            })
            .map(|(p, _)| p)
            .expect("a land tile")
    }

    fn row<'a>(rows: &'a [PickerRow], session: &Session, id: &str) -> &'a PickerRow {
        let id = session.defs.building_id(id).unwrap();
        rows.iter().find(|r| r.id == id).unwrap()
    }

    #[test]
    fn the_picker_lists_each_building_the_nation_can_have_by_its_name_with_cost_and_work() {
        for nation in Nation::ALL {
            let session = session(nation);
            let rows = picker_rows(&session);
            let mine: Vec<&BuildingDef> = session
                .defs
                .buildings
                .iter()
                .filter(|d| available_to_nation(&session, d))
                .collect();
            assert_eq!(rows.len(), mine.len());
            assert!(rows.len() < session.defs.buildings.len(), "{nation:?} has hidden buildings");
            for (row, def) in rows.iter().zip(mine) {
                assert_eq!(row.name, def.display_name(nation));
                assert_eq!(row.work, format!("{:.0}s work", def.work));
            }
            assert_eq!(row(&rows, &session, "bed").cost, "5 Wood");
        }
        let fire = session(Nation::Fire);
        assert_eq!(row(&picker_rows(&fire), &fire, "study_hall").name, "Academy");
        let air = session(Nation::Air);
        assert_eq!(row(&picker_rows(&air), &air, "study_hall").name, "Meditation Pavilion");
    }

    #[test]
    fn a_stone_cost_shows_the_stand_in_for_a_nation_that_can_make_it() {
        // Only Fire can ever raise a brick kiln, so only Fire's picker names brick as a stand-in
        // for stone (issue #371); the shared Workshop costs stone for everyone.
        let fire = session(Nation::Fire);
        assert_eq!(
            row(&picker_rows(&fire), &fire, "workshop").cost,
            "8 Wood, 4 Stone (or brick), 2x2"
        );
        let earth = session(Nation::Earth);
        assert_eq!(row(&picker_rows(&earth), &earth, "workshop").cost, "8 Wood, 4 Stone, 2x2");
    }

    #[test]
    fn buildings_a_nation_has_not_unlocked_are_locked_with_the_tech_named() {
        for nation in Nation::ALL {
            let session = session(nation);
            let rows = picker_rows(&session);
            let mine = session.defs.buildings.iter().filter(|d| available_to_nation(&session, d));
            for (row, def) in rows.iter().zip(mine) {
                assert_eq!(row.locked(), !session.world.is_unlocked(def.unlocked_by), "{}", def.id);
                if let Some(t) = def.unlocked_by.filter(|_| row.locked()) {
                    assert_eq!(
                        row.requires,
                        Some(format!("Requires {}", session.defs.tech(t).name))
                    );
                }
            }
        }
        // Fire starts knowing fire making; Earth doesn't.
        let fire = session(Nation::Fire);
        assert!(!row(&picker_rows(&fire), &fire, "campfire").locked());
        let earth = session(Nation::Earth);
        let rows = picker_rows(&earth);
        assert_eq!(
            row(&rows, &earth, "campfire").requires.as_deref(),
            Some("Requires Fire Making")
        );
        assert!(!row(&rows, &earth, "bed").locked());
    }

    #[test]
    fn buildings_behind_another_nation_s_tech_are_hidden() {
        let ids =
            |s: &Session| -> Vec<BuildingId> { picker_rows(s).iter().map(|r| r.id).collect() };
        let earth = session(Nation::Earth);
        let fire = session(Nation::Fire);
        let wall = earth.defs.building_id("earth_wall").unwrap();
        let earthen_walls = earth.defs.tech(earth.defs.building(wall).unlocked_by.unwrap());
        assert_eq!(earthen_walls.nation, Some(Nation::Earth));
        // Listed (and locked) for Earth, absent for Fire.
        assert!(row(&picker_rows(&earth), &earth, "earth_wall").locked());
        assert!(!ids(&fire).contains(&wall));
        // Earth in turn never sees Air's orchard.
        let orchard = earth.defs.building_id("orchard").unwrap();
        assert!(!ids(&earth).contains(&orchard));
        assert!(ids(&session(Nation::Air)).contains(&orchard));
        // Shared buildings stay visible to everyone.
        let bed = earth.defs.building_id("bed").unwrap();
        for nation in Nation::ALL {
            assert!(ids(&session(nation)).contains(&bed));
        }
    }

    /// The shared Workshop (`carpentry`, issue #369) is in every nation's picker, the way the
    /// Bed is: tools are everyone's fitting-out good, not just Fire's.
    #[test]
    fn the_workshop_is_listed_for_every_nation() {
        let workshop = session(Nation::Fire).defs.building_id("workshop").unwrap();
        for nation in Nation::ALL {
            let s = session(nation);
            let ids: Vec<BuildingId> = picker_rows(&s).iter().map(|r| r.id).collect();
            assert!(ids.contains(&workshop), "{nation:?} should see the Workshop");
        }
    }

    #[test]
    fn a_locked_building_cannot_be_the_choice() {
        let earth = session(Nation::Earth);
        let rows = picker_rows(&earth);
        let campfire = earth.defs.building_id("campfire").unwrap();
        let first = rows.iter().find(|r| !r.locked()).unwrap().id;
        assert_eq!(valid_choice(&rows, None), Some(first));
        assert_eq!(valid_choice(&rows, Some(campfire)), Some(first));
        let bed = earth.defs.building_id("bed").unwrap();
        assert_eq!(valid_choice(&rows, Some(bed)), Some(bed));
    }

    #[test]
    fn cycling_skips_locked_buildings_and_wraps() {
        let earth = session(Nation::Earth);
        let rows = picker_rows(&earth);
        let open: Vec<BuildingId> = rows.iter().filter(|r| !r.locked()).map(|r| r.id).collect();
        assert!(open.len() >= 2);
        let mut at = None;
        let mut seen = Vec::new();
        for _ in 0..open.len() {
            at = next_choice(&rows, at);
            seen.push(at.unwrap());
        }
        assert_eq!(seen, open);
        assert_eq!(next_choice(&rows, at), Some(open[0]));
    }

    #[test]
    fn an_unresearched_designation_names_the_tech() {
        let mut earth = session(Nation::Earth);
        let campfire = earth.defs.building_id("campfire").unwrap();
        let pos = free_tile(&earth);
        let err = earth.apply(Command::DesignateBuild { pos, building: campfire }).unwrap_err();
        assert!(matches!(err, CommandError::Build(BuildError::NotResearched(_))));
        assert_eq!(describe_error(&earth, err), "requires Fire Making");
        let off = CommandError::Build(BuildError::CannotBuild {
            at: Pos::new(-1, 0),
            why: fn_sim::world::Blocker::OffMap,
        });
        assert_eq!(describe_error(&earth, off), "off the map");
    }

    #[test]
    fn a_site_s_report_shows_materials_and_work() {
        let mut earth = session(Nation::Earth);
        let bed = earth.defs.building_id("bed").unwrap();
        let pos = free_tile(&earth);
        assert_eq!(site_report(&earth, pos), None);
        earth.apply(Command::DesignateBuild { pos, building: bed }).unwrap();
        let report = site_report(&earth, pos).expect("a site");
        assert_eq!(report.name, "Bed");
        assert_eq!(report.materials, "Materials: 0/5 Wood");
        assert_eq!(report.work, "Work: 0/20s");
        assert_eq!(built_name(&earth, pos), None);
    }

    /// The Build modal's detail (#285): the nation's name, the cost against stores, the
    /// footprint, what the building does, and the tech it still needs when locked.
    #[test]
    fn the_detail_shows_cost_against_stores_footprint_effects_and_the_tech_it_needs() {
        let earth = session(Nation::Earth);
        let bed = earth.defs.building_id("bed").unwrap();
        let detail = building_detail(&earth, bed);
        assert_eq!(detail.name, "Bed");
        assert_eq!(detail.cost, vec!["5 Wood (0 in stores)".to_owned()]);
        assert_eq!(detail.footprint, "1x1");
        assert!(detail.effects.iter().any(|e| e.starts_with("Bed:")), "{:?}", detail.effects);
        assert_eq!(detail.requires, None);

        let campfire = earth.defs.building_id("campfire").unwrap();
        let locked = building_detail(&earth, campfire);
        assert_eq!(locked.requires.as_deref(), Some("Requires Fire Making"));
        assert!(
            locked.effects.iter().any(|e| e.starts_with("Keeps a fire:")),
            "{:?}",
            locked.effects
        );

        let rail = earth.defs.building_id("rail").unwrap();
        let rail_detail = building_detail(&earth, rail);
        assert!(rail_detail.effects.iter().any(|e| e.starts_with("Rail:")), "{:?}", rail_detail);

        let hall = earth.defs.building_id("study_hall").unwrap();
        let hall_detail = building_detail(&earth, hall);
        assert!(
            hall_detail.effects.iter().any(|e| e.starts_with("Study hall:")),
            "{:?}",
            hall_detail
        );

        let air = session(Nation::Air);
        let orchard = air.defs.building_id("orchard").unwrap();
        let orchard_detail = building_detail(&air, orchard);
        assert!(
            orchard_detail.effects.iter().any(|e| e.starts_with("Tends up to")),
            "{:?}",
            orchard_detail
        );

        let wall = earth.defs.building_id("earth_wall").unwrap();
        assert_eq!(
            building_detail(&earth, wall).effects,
            vec!["Nothing beyond shelter".to_owned()]
        );

        // A capacity beyond the seats (#226) gets its own line; a hall's equal one doesn't.
        assert!(!hall_detail.effects.iter().any(|e| e.starts_with("Room for")), "{hall_detail:?}");
        let mut roomy = session(Nation::Earth);
        roomy.defs.buildings[wall.0 as usize].capacity = 3;
        assert!(
            building_detail(&roomy, wall)
                .effects
                .contains(&"Room for 3 inside, through the door".to_owned()),
            "{:?}",
            building_detail(&roomy, wall)
        );

        // The endeavor's gate has no cost of its own (#236): its stages do.
        let gate = earth.defs.building_id("harmony_gate").unwrap();
        assert_eq!(building_detail(&earth, gate).cost, vec!["free".to_owned()]);
    }

    #[test]
    fn the_cursor_anchors_odd_sizes_centred_and_even_ones_top_left_of_the_middle() {
        let c = Pos::new(10, 10);
        assert_eq!(anchor_for(c, (1, 1)), c);
        assert_eq!(anchor_for(c, (2, 2)), c);
        assert_eq!(anchor_for(c, (3, 3)), Pos::new(9, 9));
        assert_eq!(anchor_for(c, (4, 3)), Pos::new(9, 9));
    }

    /// Earth with its bed made a 2x2 building, and a clear 3x3 patch of land at the returned
    /// tile.
    fn earth_with_2x2_hall() -> (Session, BuildingId, Pos) {
        let mut s = session(Nation::Earth);
        let bed = s.defs.building_id("bed").unwrap();
        let def = &mut s.defs.buildings[bed.0 as usize];
        def.footprint = (2, 2);
        def.bed = None;
        let at = Pos::new(30, 30);
        for dy in 0..3 {
            for dx in 0..3 {
                let t = s.world.map.get_mut(Pos::new(at.x + dx, at.y + dy)).unwrap();
                t.terrain = Terrain::Land;
                t.resource = None;
            }
        }
        (s, bed, at)
    }

    #[test]
    fn a_placement_marks_each_footprint_tile_and_the_first_reason() {
        let (mut s, hall, at) = earth_with_2x2_hall();
        let free = placement(&s, at, hall);
        assert_eq!(free.footprint, Footprint::new(at, 2, 2));
        assert!(free.ok() && free.tiles.iter().all(|(_, b)| b.is_none()));
        assert_eq!(free.tiles.len(), 4);

        let corner = Pos::new(at.x + 1, at.y + 1);
        s.apply(Command::DesignateStockpile { pos: corner }).unwrap();
        let blocked = placement(&s, at, hall);
        assert_eq!(blocked.tiles[3], (corner, Some(Blocker::Stockpile)));
        assert!(blocked.tiles[..3].iter().all(|(_, b)| b.is_none()));
        let err = CommandError::Build(blocked.refused.expect("refused"));
        assert_eq!(describe_error(&s, err), "on a stockpile");

        // A locked building is refused for its tech first, wherever it goes.
        let campfire = s.defs.building_id("campfire").unwrap();
        let locked = placement(&s, Pos::new(at.x + 5, at.y), campfire);
        assert!(matches!(locked.refused, Some(BuildError::NotResearched(_))));
    }

    /// Earth's study hall (2x2, capacity 2 since seats defaults the door, #226) and a clear 3x3
    /// patch at the returned tile, which also clears the ring row below it where its door
    /// (`Footprint::entrance`) lands.
    fn earth_with_clear_hall_site() -> (Session, BuildingId, Pos) {
        let mut s = session(Nation::Earth);
        let hall = s.defs.building_id("study_hall").unwrap();
        let at = Pos::new(40, 40);
        for dy in 0..3 {
            for dx in 0..3 {
                let t = s.world.map.get_mut(Pos::new(at.x + dx, at.y + dy)).unwrap();
                t.terrain = Terrain::Land;
                t.resource = None;
            }
        }
        (s, hall, at)
    }

    /// #230: the ghost names a capacity-above-0 building's own door and whether it's free, and
    /// marks nothing for one that can't be entered at all.
    #[test]
    fn a_placement_names_its_own_door_and_whether_it_is_free() {
        let (mut s, hall, at) = earth_with_clear_hall_site();
        let door = Footprint::new(at, 2, 2).entrance();
        let free = placement(&s, at, hall);
        assert_eq!(free.entrance, Some((door, true)));

        // A resource on the door tile makes it unwalkable, so the ghost would mark it blocked,
        // and the whole placement is refused for it.
        let oak = s.defs.resource_id("oak_tree").unwrap();
        s.world.map.get_mut(door).unwrap().resource = Some(oak);
        let blocked = placement(&s, at, hall);
        assert_eq!(blocked.entrance, Some((door, false)));
        assert_eq!(
            blocked.refused,
            Some(BuildError::CannotBuild { at: door, why: Blocker::Entrance })
        );

        // A bed has no capacity, so no door to mark.
        let bed = s.defs.building_id("bed").unwrap();
        assert_eq!(placement(&s, at, bed).entrance, None);
    }

    /// #230: a footprint that would cover another building's door is refused for that tile, in
    /// red like any other blocked tile, and the Cursor line names whose door it would block.
    #[test]
    fn a_placement_marks_a_tile_that_would_cover_another_buildings_door() {
        let (mut s, hall, at) = earth_with_clear_hall_site();
        s.apply(Command::DesignateBuild { pos: at, building: hall }).unwrap();
        let door = Footprint::new(at, 2, 2).entrance();

        let bed = s.defs.building_id("bed").unwrap();
        let on_door = placement(&s, door, bed);
        assert_eq!(on_door.tiles, vec![(door, Some(Blocker::Entrance))]);
        let err = CommandError::Build(on_door.refused.expect("refused"));
        assert_eq!(
            describe_error(&s, err),
            format!("would block {}'s door", s.defs.building(hall).display_name(Nation::Earth))
        );
    }

    #[test]
    fn ghosts_follow_the_cursor_or_the_drag_and_judge_each_stamp_on_its_own() {
        let (mut s, hall, at) = earth_with_2x2_hall();
        let hover = ghosts(&s, None, at, hall);
        assert_eq!(hover, vec![placement(&s, at, hall)]);

        // A drag shows one ghost per stamp the drag would place, at the same anchors
        // `tools::build_stamps` would (one row to the right, inside the cleared patch).
        let to = Pos::new(at.x + 2, at.y + 1);
        let drag = ghosts(&s, Some(at), to, hall);
        let anchors: Vec<Pos> = drag.iter().map(|g| g.footprint.anchor).collect();
        assert_eq!(anchors, crate::tools::build_stamps(at, to, (2, 2)));
        assert_eq!(anchors, vec![at]);

        // A blocker on one stamp doesn't change another's: each is judged against the world as
        // it stands, not against the other stamps in the drag.
        let corner = Pos::new(at.x + 1, at.y + 1);
        s.apply(Command::DesignateStockpile { pos: corner }).unwrap();
        let hover = ghosts(&s, None, at, hall);
        assert_eq!(
            hover[0].refused,
            Some(BuildError::CannotBuild { at: corner, why: Blocker::Stockpile })
        );
    }

    #[test]
    fn a_refusal_names_what_is_in_the_way() {
        let (mut s, hall, at) = earth_with_2x2_hall();
        let oak = s.defs.resource_id("oak_tree").unwrap();
        let tree = Pos::new(at.x + 1, at.y);
        s.world.map.get_mut(tree).unwrap().resource = Some(oak);
        let why = |s: &Session| {
            let e = s.world.check_build(at, hall, &s.defs).unwrap_err();
            describe_error(s, CommandError::Build(e))
        };
        assert_eq!(why(&s), "an oak tree is in the way");
        s.world.map.get_mut(tree).unwrap().resource = None;
        s.world.map.get_mut(tree).unwrap().terrain = Terrain::Ford;
        assert_eq!(why(&s), "can't build on a ford");
    }

    #[test]
    fn the_picker_shows_the_size_only_for_buildings_bigger_than_one_tile() {
        let (s, hall, _) = earth_with_2x2_hall();
        let rows = picker_rows(&s);
        let row = |id| rows.iter().find(|r| r.id == id).unwrap().cost.clone();
        assert_eq!(row(hall), "5 Wood, 2x2");
        // #214 turns real footprints on, so a fresh nation sees the size suffix on every
        // bigger-than-1x1 building (the study hall from the start) and on no 1x1 one.
        let fresh = session(Nation::Earth);
        for r in picker_rows(&fresh) {
            let big = fresh.defs.building(r.id).footprint != (1, 1);
            assert_eq!(r.cost.contains('x'), big, "{}", fresh.defs.building(r.id).id);
        }
    }

    #[test]
    fn every_tile_of_a_footprint_reports_the_same_site_and_building() {
        let (mut s, hall, at) = earth_with_2x2_hall();
        s.apply(Command::DesignateBuild { pos: at, building: hall }).unwrap();
        let foot = Footprint::new(at, 2, 2);
        for p in foot.tiles() {
            assert_eq!(site_report(&s, p), site_report(&s, at), "{p:?}");
        }
        assert!(site_report(&s, at).is_some());
        // Finished: baked onto every tile.
        s.apply(Command::CancelBuild { pos: at }).unwrap();
        for p in foot.tiles() {
            let part = [(p.x - at.x) as u8, (p.y - at.y) as u8];
            let b = fn_sim::building::Building {
                part,
                size: [2, 2],
                ..fn_sim::building::Building::new(hall, false, false)
            };
            s.world.map.get_mut(p).unwrap().building = Some(b);
        }
        for p in foot.tiles() {
            assert_eq!(built_name(&s, p).as_deref(), Some("Bed"), "{p:?}");
        }
    }

    /// A finished study hall (capacity 2, #226) anchored at a clear 3x3 patch, baked onto the map
    /// directly as `every_tile_of_a_footprint_reports_the_same_site_and_building` does, so the
    /// test needs no real construction.
    fn earth_with_a_study_hall() -> (Session, BuildingId, Pos) {
        let mut s = session(Nation::Earth);
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
        let door = foot.entrance();
        s.world.units.retain(|u| !foot.contains(u.pos) && u.pos != door);
        for p in foot.tiles() {
            let part = [(p.x - at.x) as u8, (p.y - at.y) as u8];
            let b = fn_sim::building::Building {
                part,
                size: [2, 2],
                ..fn_sim::building::Building::new(hall, passable, false)
            };
            s.world.map.get_mut(p).unwrap().building = Some(b);
        }
        (s, hall, at)
    }

    /// `occupant_rows` (#226, #229): empty with nobody inside, then each occupant in id order,
    /// reported the same from every tile of the hall's footprint. Nothing claims a study seat
    /// without #227's jobs yet, so an occupant put inside directly (`World::enter_building`)
    /// reads "idle"; #227 will make "studying" reachable.
    #[test]
    fn occupant_rows_lists_who_is_inside_in_id_order_with_a_short_task() {
        let (mut s, _, at) = earth_with_a_study_hall();
        assert_eq!(occupant_rows(&s, at), Vec::new(), "nobody inside yet");

        let door = Footprint::new(at, 2, 2).entrance();
        s.world.units[0].pos = door;
        s.world.units[1].pos = door;
        let names = [s.world.units[0].name.clone(), s.world.units[1].name.clone()];
        let ids = [s.world.units[0].id, s.world.units[1].id];
        assert!(s.world.enter_building(0, at, &s.defs));
        assert!(s.world.enter_building(1, at, &s.defs));

        let rows = occupant_rows(&s, at);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, ids[0]);
        assert_eq!(rows[0].task, "idle");
        assert_eq!(rows[0].text(), format!("{}, idle", names[0]));
        assert_eq!(rows[1].text(), format!("{}, idle", names[1]));

        // Every footprint tile names the same occupants (#212).
        for p in Footprint::new(at, 2, 2).tiles() {
            assert_eq!(occupant_rows(&s, p), rows, "{p:?}");
        }
        // Off the building, or past its door: nothing.
        assert_eq!(occupant_rows(&s, door), Vec::new());
    }
}

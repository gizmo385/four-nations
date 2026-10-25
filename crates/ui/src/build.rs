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
use fn_sim::world::{Blocker, BuildError, CommandError, DesignateError};
use fn_sim::{Pos, Terrain};

use crate::session::Session;

/// One line of the building picker.
#[derive(Clone, PartialEq, Debug)]
pub struct PickerRow {
    pub id: BuildingId,
    /// The nation's name for it (`BuildingDef::display_name`).
    pub name: String,
    /// "5 Wood", "10 Wood, 2 Stone" or "free".
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

/// What a building costs, as "5 Wood" (comma-separated for several items), followed by its
/// size if it covers more than one tile ("10 Wood, 2x2").
fn cost_text(session: &Session, def: &BuildingDef) -> String {
    let mut parts: Vec<String> = def
        .cost
        .iter()
        .map(|c| format!("{} {}", c.count, session.defs.item(c.item).name))
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
    /// Every footprint tile, row-major, with what blocks it (`World::build_blocker`).
    pub tiles: Vec<(Pos, Option<Blocker>)>,
    /// Why designating it would be refused, if it would: the tech it still needs, else the
    /// first blocked tile (`World::check_build`).
    pub refused: Option<BuildError>,
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
    let (w, h) = session.defs.building(building).footprint;
    let footprint = Footprint::new(anchor, w, h);
    let tiles = footprint.tiles().map(|p| (p, world.build_blocker(p))).collect();
    let refused = world.check_build(anchor, building, &session.defs).err();
    Placement { footprint, tiles, refused }
}

/// The ghosts the Build tool shows: with no corner set, the one placement under the cursor;
/// while dragging from `corner`, one per stamp `tools::build_stamps` would place. Each is judged
/// against the world as it is, not against the stamps before it.
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

/// Why the first of `ghosts` that would be refused is refused, as the status line says it:
/// "can't build here: an oak tree is in the way". `None` when every one can be placed.
pub fn ghost_refusal(session: &Session, ghosts: &[Placement]) -> Option<String> {
    let e = ghosts.iter().find_map(|g| g.refused)?;
    Some(format!("can't build here: {}", describe_error(session, CommandError::Build(e))))
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
    }
}

/// Whether the current nation could ever build `def`: not if the tech that unlocks it belongs
/// to another nation's branch (`TechDef::nation`), which the nation can never research.
fn available_to_nation(session: &Session, def: &BuildingDef) -> bool {
    def.unlocked_by
        .and_then(|t| session.defs.tech(t).nation)
        .is_none_or(|n| n == session.world.nation)
}

/// Every building the current nation can build or may yet unlock, in file order, by its name for
/// them, with cost and work, and the tech it still needs if it's locked. Buildings behind
/// another nation's tech are left out (issue #137).
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

#[cfg(test)]
mod tests {
    use super::*;
    use fn_sim::world::Command;
    use fn_sim::{Nation, Terrain};

    fn session(nation: Nation) -> Session {
        Session::new(7, nation, 64, 64)
    }

    /// Some tile a building can go on.
    fn free_tile(session: &Session) -> Pos {
        session
            .world
            .map
            .iter()
            .find(|(p, t)| {
                t.terrain == Terrain::Land
                    && t.resource.is_none()
                    && !session.world.units.iter().any(|u| u.pos == *p)
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

    #[test]
    fn ghosts_follow_the_cursor_or_the_drag_and_name_the_first_refusal() {
        let (mut s, hall, at) = earth_with_2x2_hall();
        let hover = ghosts(&s, None, at, hall);
        assert_eq!(hover, vec![placement(&s, at, hall)]);
        assert_eq!(ghost_refusal(&s, &hover), None);

        // A drag shows one ghost per stamp the drag would place, at the same anchors.
        let to = Pos::new(at.x + 3, at.y + 1);
        let drag = ghosts(&s, Some(at), to, hall);
        let anchors: Vec<Pos> = drag.iter().map(|g| g.footprint.anchor).collect();
        assert_eq!(anchors, crate::tools::build_stamps(at, to, (2, 2)));
        assert_eq!(anchors.len(), 2);

        s.apply(Command::DesignateStockpile { pos: Pos::new(at.x + 1, at.y) }).unwrap();
        let hover = ghosts(&s, None, at, hall);
        assert_eq!(ghost_refusal(&s, &hover).as_deref(), Some("can't build here: on a stockpile"));
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
        let fresh = session(Nation::Earth);
        assert!(picker_rows(&fresh).iter().all(|r| !r.cost.contains('x')));
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
}

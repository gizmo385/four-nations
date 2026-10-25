//! The build tool's data (issue #111): which buildings the picker lists and which are locked,
//! the building chosen to place, how a rejected command reads, and what a construction site or a
//! finished building says in the tooltip. Pure functions over `Session`, so the tests cover them;
//! the frontends draw the picker and `tools::apply_rect` turns a drag into `DesignateBuild`.
//!
//! The Cancel tool withdraws a build designation too: on a tile with a site it issues
//! `CancelBuild`, on a stockpile tile `CancelStockpile`, elsewhere `CancelHarvest`.

use fn_sim::Pos;
use fn_sim::building::{BuildingDef, BuildingId};
use fn_sim::world::{BuildError, CommandError, DesignateError};

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

/// What a building costs, as "5 Wood" (comma-separated for several items).
fn cost_text(session: &Session, def: &BuildingDef) -> String {
    if def.cost.is_empty() {
        return "free".into();
    }
    let parts: Vec<String> = def
        .cost
        .iter()
        .map(|c| format!("{} {}", c.count, session.defs.item(c.item).name))
        .collect();
    parts.join(", ")
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

/// The pending build on `pos`, if any, with its materials delivered / needed and work progress.
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

/// The name of the finished building on `pos`, if any.
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
}

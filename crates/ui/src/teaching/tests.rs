//! The LOST TEACHING box's content (#743), from a great ruin searched the real way: designated,
//! dug, its teaching scroll carried to a study hall and read there.

use fn_sim::Nation;
use fn_sim::Pos;
use fn_sim::defs::Terrain;
use fn_sim::geom::Footprint;
use fn_sim::world::Command;

use super::*;

/// Stands a study hall on open land near the settlers, turns an open tile near it into great
/// ruins, designates the search and runs the world until the scroll is read and its draft is
/// pending. Returns where the ruins and the hall are.
pub(crate) fn read_a_teaching_scroll(s: &mut Session) -> (Pos, Pos) {
    let home = s.world.units[0].pos;
    let open = |s: &Session, p: Pos| {
        s.world.map.get(p).is_some_and(|t| {
            t.terrain == Terrain::Land && t.building.is_none() && t.resource.is_none()
        })
    };
    let foot = (2..20)
        .flat_map(|r: i32| (-r..=r).flat_map(move |d| [(d, -r), (d, r), (-r, d), (r, d)]))
        .map(|(dx, dy)| Footprint::new(Pos::new(home.x + dx, home.y + dy), 2, 2))
        .find(|f| f.tiles().chain(f.ring()).all(|p| open(s, p)))
        .expect("room for a study hall");
    let hall = foot.anchor;
    s.world.units.retain(|u| !foot.contains(u.pos));
    let id = s.defs.building_id("study_hall").unwrap();
    s.world.put_building(foot, id, &s.defs);
    let ruins = s.defs.resource_id("great_ruins").unwrap();
    let at = (3..12)
        .flat_map(|r| [Pos::new(hall.x - r, hall.y), Pos::new(hall.x, hall.y - r)])
        .find(|&p| open(s, p))
        .expect("open land near the hall");
    s.world.map[at].resource = Some(ruins);
    s.world.map_changed();
    s.apply(Command::DesignateHarvest { pos: at }).unwrap();
    for _ in 0..60 {
        if !s.world.teaching_offers().is_empty() {
            return (at, hall);
        }
        s.tick_n(500);
    }
    panic!(
        "the teaching scroll was never read: tick {}, ground {:?}, jobs {:?}",
        s.world.tick,
        s.world.ground_items().collect::<Vec<_>>(),
        s.world.jobs(),
    );
}

fn session() -> Session {
    crate::new_session(7, Nation::Air)
}

#[test]
fn the_box_opens_on_decide_later_then_one_keep_row_per_teaching() {
    let mut s = session();
    assert_eq!(draft(&s), None, "nothing pending");
    read_a_teaching_scroll(&mut s);
    let d = draft(&s).expect("a pending offer");
    assert_eq!(d.title, "LOST TEACHING");
    assert_eq!(d.lines[0], "A teaching scroll was read at the Meditation Pavilion.");
    assert_eq!(d.lines[1], "Keep one teaching. The other is lost for this game.");
    assert_eq!(d.keeps.len(), 2);
    let choices = d.choices();
    assert_eq!(choices[0], Choice::Later, "the safe answer comes first");
    for (row, choice) in d.keeps.iter().zip(&choices[1..]) {
        assert_eq!(*choice, Choice::Keep(row.tech));
        assert!(row.label.starts_with("Keep "), "{row:?}");
        assert_eq!(row.cost, "250 pts + scroll");
        assert!(row.unlock.starts_with("Unlocks "), "{row:?}");
    }
}

#[test]
fn keeping_one_loses_the_other_and_closes_the_offer() {
    let mut s = session();
    read_a_teaching_scroll(&mut s);
    let d = draft(&s).unwrap();
    let (keep, lose) = (d.keeps[0].tech, d.keeps[1].tech);
    assert_eq!(take(&mut s, Choice::Later), Ok(String::new()));
    assert!(draft(&s).is_some(), "Decide later leaves the offer waiting");
    let name = s.defs.tech(keep).name.clone();
    assert_eq!(take(&mut s, Choice::Keep(keep)), Ok(format!("Kept {name}")));
    assert_eq!(draft(&s), None);
    assert!(s.world.kept_teachings().any(|t| t == keep));
    assert!(s.world.lost_teachings().any(|t| t == lose));
    assert!(take(&mut s, Choice::Keep(lose)).unwrap_err().starts_with("Can't keep"));
}

#[test]
fn the_reason_line_is_the_sims_caveat() {
    let s = session();
    let defs = &s.defs;
    let guru = defs.tech_id("guru_teachings").unwrap();
    // Air starts with no master, and Guru Teachings only changes what a master pays.
    let caveat = s.world.teaching_caveat(guru, defs);
    let meditation = defs.tech_id("meditation").unwrap();
    let want = if s.world.research().is_known(meditation) {
        "No master yet.".to_owned()
    } else {
        "Needs: Meditation".to_owned()
    };
    assert_eq!(caveat.map(|c| describe_caveat(defs, &c)), Some(want));
    let pai_sho = defs.tech_id("pai_sho").unwrap();
    let caveat = s.world.teaching_caveat(pai_sho, defs).unwrap();
    assert_eq!(describe_caveat(defs, &caveat), "Needs: Known on the road");
}

#[test]
fn a_great_ruins_search_reads_as_two_scrolls_and_a_teaching_scroll() {
    let s = session();
    let ruins = s.defs.resource_id("great_ruins").unwrap();
    let h = s.defs.harvest(ruins).unwrap();
    assert_eq!(yield_phrase(&s.defs, h, h.count), "2 ancient scrolls and a teaching scroll");
}

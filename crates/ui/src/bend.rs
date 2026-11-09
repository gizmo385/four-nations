//! The Bend tool's banner as a view model (#586): what a player reads on the active tool banner
//! (`BEND Hearthflame ... B: next ability`) and in the side panel's one Bend row
//! (`Hearthflame  20 benders`), in one place, so the TUI draws from it and `screen {"bend"}`
//! reports it, and the two can't drift apart.
//!
//! What it covers is exactly what pressing `B` shows, no more. The banner names one ability at a
//! time; `B` moves on to the next the settlement can use (`tools::usable_abilities`: its nation's,
//! with their tech known, in data order), wrapping round, and costs nothing. So the rows here are
//! those abilities in `B`'s own order, each as the banner and the Tool row would draw it when
//! `B` lands on it. An ability the settlement can't use yet is never on the banner, so it isn't
//! here either (the Research screen's "Unlocks:" line is where a player reads which tech gives
//! it). Nothing about targets, best uses or whether a tile is bendable: the banner says none of
//! that.
//!
//! It reads the same whichever tool is active: `b` from any tool lands on the ability this names
//! as current (`tools::cycle_ability` only moves the choice when `B` is pressed *within* the Bend
//! tool), the way `build::picker_rows` reads the same whether or not the Build tool is open.

use crate::session::Session;
use crate::text::caster_label;
use crate::tools::{bend_ability, usable_abilities};

/// One ability as the Bend tool shows it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BendRow {
    /// The ability's display name, as the banner badge shows it ("Hearthflame"), and what
    /// `act {designate, tool: "bend", ability}` takes.
    pub name: String,
    /// The Tool row's count: "20 benders", or for a rank-gated ability "2 adepts" / "no master"
    /// (`text::caster_label`).
    pub casters: String,
    /// The one the banner names now, which a plain `b` then `Enter` would cast.
    pub current: bool,
}

/// The Bend tool banner's content.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct BendView {
    /// Every ability `B` cycles through, in `B`'s order; the current one is flagged. Empty only
    /// if the nation has no ability defined at all (the data tests guard against that).
    pub rows: Vec<BendRow>,
}

impl BendView {
    /// The row the banner names now.
    pub fn current(&self) -> Option<&BendRow> {
        self.rows.iter().find(|r| r.current)
    }

    /// Whether the banner offers `B: next ability`: there is more than one to move on to.
    pub fn offers_next(&self) -> bool {
        self.rows.len() > 1
    }
}

/// The Bend tool banner's content for `session`, as of now.
pub fn view(session: &Session) -> BendView {
    let defs = &session.defs;
    let Some(current) = bend_ability(session) else { return BendView::default() };
    let mut ids = usable_abilities(session);
    // Nothing usable at all: the banner still names the nation's signature ability.
    if !ids.contains(&current) {
        ids.insert(0, current);
    }
    let rows = ids
        .into_iter()
        .map(|id| {
            let def = defs.ability(id);
            let casters = caster_label(def.rank, session.world.caster_count(id, defs));
            BendRow { name: def.name.clone(), casters, current: id == current }
        })
        .collect();
    BendView { rows }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use fn_sim::world::Command;

    use super::*;
    use crate::tools::{Tool, cycle_ability};

    #[test]
    fn a_new_game_shows_each_nations_signature_ability_alone() {
        for nation in Nation::ALL {
            let s = Session::new(1, nation, 48, 32);
            let v = view(&s);
            assert_eq!(v.rows.len(), 1, "{nation:?}: {v:?}");
            assert!(!v.offers_next(), "{nation:?}");
            let row = v.current().unwrap();
            let signature = crate::tools::ability_for_nation(&s.defs, nation).unwrap();
            assert_eq!(row.name, s.defs.ability(signature).name);
            assert!(row.casters.ends_with("bender") || row.casters.ends_with("benders"));
        }
    }

    /// Locked forms are absent; research the tech and the form is there, in `B`'s order, and
    /// `B` walks the current flag along the rows exactly as the banner's badge would.
    #[test]
    fn a_locked_form_is_absent_until_researched_and_b_walks_the_rows() {
        let mut s = Session::new(3, Nation::Fire, 48, 32);
        let before = view(&s);
        assert_eq!(before.rows.len(), 1);
        assert!(before.rows.iter().all(|r| r.name != "Inner Fire" && r.name != "Weld"));

        let dragon = s.defs.tech_id("dancing_dragon").unwrap();
        s.apply(Command::SetResearch { tech: dragon }).unwrap();
        for _ in 0..200_000 {
            if s.world.research().is_known(dragon) {
                break;
            }
            s.tick();
        }
        assert!(s.world.research().is_known(dragon), "researched");

        let v = view(&s);
        let names: Vec<&str> = v.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Hearthflame", "Inner Fire", "Weld"]);
        assert!(v.offers_next());
        assert_eq!(v.current().unwrap().name, "Hearthflame");

        // B inside the Bend tool moves the flag; the rows themselves stay put.
        cycle_ability(&mut s, Tool::Bend, Tool::Bend);
        let v = view(&s);
        assert_eq!(v.current().unwrap().name, "Inner Fire");
        assert_eq!(v.rows.iter().filter(|r| r.current).count(), 1);
        assert_eq!(v.rows.len(), 3);
        // Picking the tool from another leaves it alone, so reading from any tool agrees.
        cycle_ability(&mut s, Tool::Select, Tool::Bend);
        assert_eq!(view(&s).current().unwrap().name, "Inner Fire");
    }
}

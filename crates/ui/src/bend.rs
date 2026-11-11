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
//!
//! The abilities screen (`I`, #719) is wider than the banner: [`abilities`] lists the ones that
//! can't be used yet too, with what each needs, which `B` never visits.

use fn_sim::Terrain;
use fn_sim::bending::AbilityId;
use fn_sim::world::BendError;

use crate::session::Session;
use crate::text::{article, caster_label};
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

/// The words for a terrain in an effect line: land and coast are both "land", the rest are named
/// for what they are.
fn terrain_word(t: Terrain) -> String {
    match t {
        Terrain::Land | Terrain::Coast => "land".to_owned(),
        other => format!("{other:?}").to_lowercase(),
    }
}

/// `words` as a list ("river, lake or canal"), each once.
fn word_list(words: impl IntoIterator<Item = String>) -> String {
    let mut seen: Vec<String> = Vec::new();
    for w in words {
        if !seen.contains(&w) {
            seen.push(w);
        }
    }
    match seen.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

/// What a bend's rules say beyond its description, a line each (docs/design/horizons-phase-2.md,
/// "`canals`: Carve Canal", Hover): `Turns land beside fresh water into a canal`, `Irrigates
/// plots within 2 tiles` and `Permanent`. Only a bend that cuts from a source has any
/// (`AbilityDef::cut`); every other ability's description says it all, so its lines are empty.
pub fn effect_lines(session: &Session, id: AbilityId) -> Vec<String> {
    let defs = &session.defs;
    let Some(cut) = defs.ability(id).cut() else { return Vec::new() };
    let source = if cut.source.iter().all(|t| t.is_fresh()) {
        "fresh water".to_owned()
    } else {
        word_list(cut.source.iter().map(|&t| terrain_word(t)))
    };
    let to = terrain_word(cut.to);
    let mut lines = vec![format!(
        "Turns {} beside {source} into {} {to}",
        word_list(cut.from.iter().map(|&t| terrain_word(t))),
        article(&to),
    )];
    // `[canal]` is what the terrain Carve Canal makes does for the fields beside it.
    if cut.to == Terrain::Canal {
        lines.push(format!("Irrigates plots within {} tiles", defs.canal.irrigation_radius));
    }
    if cut.permanent {
        lines.push("Permanent".to_owned());
    }
    lines
}

/// One ability on the abilities screen (#719): what it does and whether it can be used now.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AbilityRow {
    pub id: AbilityId,
    /// The display name ("Hearthflame").
    pub name: String,
    /// What it does, from the data.
    pub what: String,
    /// What its rules add to that ([`effect_lines`]); empty for most.
    pub effects: Vec<String>,
    /// The count beside it ("20 benders", "no master"): `text::caster_label`. Empty for one
    /// still locked behind a tech, which nobody can bend yet.
    pub casters: String,
    /// Why it can't be used now, if it can't: the tech it still needs ("Needs Dancing Dragon"),
    /// or, for one gated on a rank nobody here holds, the title it needs ("Needs an Earthbending
    /// Master"). `None` when it can be used.
    pub unusable: Option<String>,
    /// The one the Bend tool channels now (`tools::bend_ability`).
    pub current: bool,
}

impl AbilityRow {
    /// Whether `Enter` on the screen can pick it for the Bend tool.
    pub fn usable(&self) -> bool {
        self.unusable.is_none()
    }
}

/// Every ability the abilities screen lists, the ones that can be used now first (in `B`'s
/// order), then the ones that can't, dim, each in data order: the settlement's own nation's, and
/// any other nation's that someone living here can cast (a newcomer, the Avatar). Another
/// nation's ability nobody here could ever cast is left out.
pub fn abilities(session: &Session) -> Vec<AbilityRow> {
    let defs = &session.defs;
    let current = bend_ability(session);
    let mut usable = Vec::new();
    let mut locked = Vec::new();
    for i in 0..defs.abilities.len() as u16 {
        let id = AbilityId(i);
        let def = defs.ability(id);
        let casters = session.world.caster_count(id, defs);
        let mut casters_label = caster_label(def.rank, casters);
        let unusable = match session.world.check_ability(id, defs) {
            Err(BendError::Locked { tech }) => {
                casters_label.clear();
                Some(format!("Needs {}", defs.tech(tech).name))
            }
            Err(_) => continue,
            Ok(()) => def.rank.filter(|_| casters == 0).map(|rank| {
                let title = defs.mastery.title(def.nation, rank);
                format!("Needs {} {title}", article(title))
            }),
        };
        let row = AbilityRow {
            id,
            name: def.name.clone(),
            what: def.description.clone(),
            effects: effect_lines(session, id),
            casters: casters_label,
            current: Some(id) == current,
            unusable,
        };
        if row.usable() {
            usable.push(row);
        } else {
            locked.push(row);
        }
    }
    usable.extend(locked);
    usable
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

    /// The abilities screen's list: a new game has the nation's signature ability to use and the
    /// rest locked behind their tech, each saying which; researching the tech moves the ability
    /// up into the usable ones.
    #[test]
    fn the_abilities_list_has_the_usable_ones_first_and_names_what_a_locked_one_needs() {
        for nation in Nation::ALL {
            let s = Session::new(1, nation, 48, 32);
            let rows = abilities(&s);
            assert!(rows.len() > 1, "{nation:?}: more than the signature");
            assert!(rows[0].usable() && rows[0].current, "{nation:?}: {:?}", rows[0]);
            assert!(rows.iter().all(|r| s.defs.ability(r.id).nation == nation));
            let first_locked = rows.iter().position(|r| !r.usable()).unwrap();
            assert!(rows[first_locked..].iter().all(|r| !r.usable()), "{nation:?}: grouped");
            for r in &rows {
                assert!(!r.what.is_empty());
                assert_eq!(r.casters.is_empty(), r.unusable.is_some(), "{r:?}");
                if let Some(why) = &r.unusable {
                    assert!(why.starts_with("Needs "), "{why}");
                }
            }
            assert_eq!(rows.iter().filter(|r| r.current).count(), 1);
        }
        let s = Session::new(3, Nation::Fire, 48, 32);
        let rows = abilities(&s);
        let weld = rows.iter().find(|r| r.name == "Weld").unwrap();
        assert!(!weld.usable());
        assert!(weld.unusable.as_deref().unwrap().starts_with("Needs "));
    }

    /// Every ability the Bend tool cycles through is a usable row, in `B`'s order.
    #[test]
    fn the_usable_rows_are_exactly_what_b_cycles_through() {
        let mut s = Session::new(3, Nation::Fire, 48, 32);
        let dragon = s.defs.tech_id("dancing_dragon").unwrap();
        s.apply(Command::SetResearch { tech: dragon }).unwrap();
        for _ in 0..200_000 {
            if s.world.research().is_known(dragon) {
                break;
            }
            s.tick();
        }
        let rows = abilities(&s);
        let usable: Vec<&str> =
            rows.iter().filter(|r| r.usable()).map(|r| r.name.as_str()).collect();
        let banner: Vec<String> = view(&s).rows.into_iter().map(|r| r.name).collect();
        assert_eq!(usable, banner);
    }
}

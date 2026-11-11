//! Switching a building's recipe (`R`, #712, docs/design/horizons-phase-3.md, "`trade_wares`"):
//! the Workshop makes tools until a tech unlocks wares, and each Workshop is set on its own with
//! `Command::SetRecipe`. Which recipes a building has, and whether one may be set, are the
//! sim's; this picks the next one and words the outcome.

use fn_sim::Pos;
use fn_sim::world::Command;

use crate::session::Session;

/// Sets the building under `at` to its next recipe, wrapping from the last alt recipe back to the
/// default. A recipe the sim refuses (a tech has not unlocked it) is skipped, and if it refuses
/// every other one the first refusal is the answer ("Can't switch recipe: needs Trade Wares").
/// The outcome as a notice either way.
pub fn switch(session: &mut Session, at: Pos) -> Result<String, String> {
    let nothing = || Err("Can't switch recipe: nothing to switch here".to_owned());
    let (world, defs) = (&session.world, &session.defs);
    let (Some(foot), Some(b)) = (world.footprint_at(at), world.building_at(at)) else {
        return nothing();
    };
    let def = defs.building(b);
    if def.alt_recipes.is_empty() || def.recipe.is_none() {
        return nothing();
    }
    let anchor = foot.anchor;
    // Choice 0 is the default recipe; the alt recipes follow.
    let current = world.recipe_choice_at(anchor, defs).map_or(0, |i| usize::from(i) + 1);
    let count = def.alt_recipes.len() + 1;
    let name = def.display_name(world.nation).to_owned();
    let mut first_refusal = None;
    for step in 1..count {
        let next = (current + step) % count;
        let recipe = next.checked_sub(1).map(|i| i as u8);
        match session.apply(Command::SetRecipe { pos: anchor, recipe }) {
            Ok(()) => {
                let made = crate::inspect::recipe_lines(&session.world, &session.defs, anchor)
                    .into_iter()
                    .next()
                    .map_or_else(String::new, |l| l.replace("Making: ", ""));
                return Ok(format!("{name} now makes {made}"));
            }
            Err(e) => {
                first_refusal.get_or_insert(e);
            }
        }
    }
    let why = first_refusal.map_or_else(|| "nothing to switch to".to_owned(), |e| e.to_string());
    Err(format!("Can't switch recipe: {why}"))
}

#[cfg(test)]
mod tests {
    use fn_sim::geom::Footprint;
    use fn_sim::{Nation, Pos};

    use super::*;

    /// Puts a building down on the free 2x2 spot nearest the first settler.
    fn put(s: &mut Session, id: &str) -> Pos {
        let id = s.defs.building_id(id).unwrap();
        let home = s.world.units[0].pos;
        let bed = s.defs.building_id("bed").unwrap();
        let at = s
            .world
            .map
            .iter()
            .map(|(p, _)| p)
            .filter(|&p| {
                [(0, 0), (1, 0), (0, 1), (1, 1)].iter().all(|&(dx, dy)| {
                    let q = Pos::new(p.x + dx, p.y + dy);
                    q != home && s.world.check_build(q, bed, &s.defs).is_ok()
                })
            })
            .min_by_key(|p| ((p.x - home.x).pow(2) + (p.y - home.y).pow(2), p.x, p.y))
            .expect("room near home");
        s.world.put_building(Footprint::new(at, 2, 2), id, &s.defs);
        at
    }

    #[test]
    fn a_locked_recipe_is_refused_by_name_and_an_unlocked_one_switches_back_and_forth() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let at = put(&mut s, "workshop");
        assert_eq!(
            switch(&mut s, at),
            Err("Can't switch recipe: needs Trade Wares".to_owned()),
            "the tech is unknown"
        );
        let id = s.defs.building_id("workshop").unwrap();
        s.defs.buildings[id.0 as usize].alt_recipes[0].unlocked_by = None;
        // Any tile of the footprint will do.
        let tile = Pos::new(at.x + 1, at.y + 1);
        assert_eq!(switch(&mut s, tile), Ok("Workshop now makes Wares".to_owned()));
        assert_eq!(s.world.recipe_choice_at(at, &s.defs), Some(0));
        assert_eq!(switch(&mut s, at), Ok("Workshop now makes Tools".to_owned()));
        assert_eq!(s.world.recipe_choice_at(at, &s.defs), None);
    }

    #[test]
    fn there_is_nothing_to_switch_off_a_workshop() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let want = Err("Can't switch recipe: nothing to switch here".to_owned());
        let home = s.world.units[0].pos;
        assert_eq!(switch(&mut s, home), want);
        let smelter = put(&mut s, "smelter");
        assert_eq!(switch(&mut s, smelter), want);
    }
}

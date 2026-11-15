//! Stamping a plan on the map (docs/design/factory-endgame.md, "The pipeline planner", point 4,
//! #1029): `Enter` in the planner turns the plan into a ghost block of machines, laid out in the
//! chain's order so each step stands against the next, with as many of each as the plan needs.
//! The arrows move it, `R` turns it (columns become rows), and `Enter` turns every piece that
//! fits into an ordinary build order (`Command::DesignateBuild`); settlers haul and build it as
//! they would anything else. `Esc` goes back to the plan.
//!
//! A step whose machine must stand somewhere of its own (a mine at the rock face) or whose tech
//! isn't known yet is left out of the block, and the summary says so.

use fn_sim::building::BuildingId;
use fn_sim::geom::{Footprint, Pos};
use fn_sim::jobs::ItemId;
use fn_sim::world::{Blocker, Command};

use crate::build::{Placement, placement_at};
use crate::planner::{PlanEdit, Travel, plan, standing_rule};
use crate::session::Session;

/// The stamp being placed: the plan it came from, and whether it's turned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StampEdit {
    pub plan: PlanEdit,
    pub turned: bool,
}

/// One machine of the block, where it would go.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub building: BuildingId,
    /// Which plan step it is.
    pub step: usize,
    pub placement: Placement,
}

/// The block under the cursor, and what placing it would take.
#[derive(Clone, Debug, PartialEq)]
pub struct StampView {
    /// "Furniture".
    pub goal: String,
    pub pieces: Vec<Piece>,
    /// Links between two steps in the block that should touch, and how many do as laid out.
    pub links: usize,
    pub touching: usize,
    /// Pieces that can't go where they are now.
    pub blocked: usize,
    /// Tiles under the block with a tree, bush or rock to clear first.
    pub to_clear: usize,
    /// Powered pieces outside every running yard: a crafter will have to work them.
    pub outside_yard: usize,
    /// What the whole block costs, and whether stores hold it all.
    pub costs: Vec<(ItemId, u32)>,
    pub have_all: bool,
    /// Steps left out of the block, with why: "Deep Mine (at the rock face)".
    pub left_out: Vec<String>,
}

/// Each block step's machine and count, in chain order, and the steps left out.
fn block_steps(
    session: &Session,
    edit: &StampEdit,
) -> (Vec<(usize, BuildingId, u32)>, Vec<String>) {
    let p = plan(session, edit.plan.goal, edit.plan.per_min);
    let mut steps = Vec::new();
    let mut left = Vec::new();
    for (i, s) in p.steps.iter().enumerate() {
        let def = session.defs.building(s.building);
        if let Some(rule) = standing_rule(def) {
            left.push(format!("{} ({rule})", s.machine));
        } else if let Some(tech) = &s.locked {
            left.push(format!("{} (needs {tech})", s.machine));
        } else {
            steps.push((i, s.building, s.needed));
        }
    }
    (steps, left)
}

/// The block's offsets from its top-left: one column a step, left to right in chain order, each
/// step's machines stacked down it; turned, one row a step, top to bottom, stacked across.
pub fn layout(session: &Session, edit: &StampEdit) -> Vec<(usize, BuildingId, Pos)> {
    let (steps, _) = block_steps(session, edit);
    let mut out = Vec::new();
    let mut along = 0;
    for (step, building, n) in steps {
        let (w, h) = session.defs.building(building).footprint;
        let (w, h) = (i32::from(w), i32::from(h));
        let (major, minor) = if edit.turned { (h, w) } else { (w, h) };
        for k in 0..n as i32 {
            let off =
                if edit.turned { Pos::new(k * minor, along) } else { Pos::new(along, k * minor) };
            out.push((step, building, off));
        }
        along += major;
    }
    out
}

/// The block with its top-left on `at`.
pub fn view(session: &Session, edit: &StampEdit, at: Pos) -> StampView {
    let (world, defs) = (&session.world, &session.defs);
    let p = plan(session, edit.plan.goal, edit.plan.per_min);
    let (_, left_out) = block_steps(session, edit);
    let pieces: Vec<Piece> = layout(session, edit)
        .into_iter()
        .map(|(step, building, off)| Piece {
            building,
            step,
            placement: placement_at(session, Pos::new(at.x + off.x, at.y + off.y), building),
        })
        .collect();
    let feet = |step: usize| -> Vec<Footprint> {
        pieces.iter().filter(|q| q.step == step).map(|q| q.placement.footprint).collect()
    };
    let (mut links, mut touching) = (0, 0);
    for (i, s) in p.steps.iter().enumerate() {
        for input in &s.inputs {
            let (Some(j), Travel::Touching) = (input.from, &input.travel) else { continue };
            let (a, b) = (feet(j), feet(i));
            if a.is_empty() || b.is_empty() {
                continue;
            }
            links += 1;
            if a.iter().any(|fa| b.iter().any(|fb| fa.gap(*fb) == 1)) {
                touching += 1;
            }
        }
    }
    let blocked = pieces.iter().filter(|q| !q.placement.ok()).count();
    let to_clear = pieces
        .iter()
        .flat_map(|q| q.placement.tiles.iter())
        .filter(|(_, b)| matches!(b, Some(Blocker::Resource)))
        .count();
    let yards = world.yards(defs);
    let outside_yard = pieces
        .iter()
        .filter(|q| defs.building(q.building).uses_power)
        .filter(|q| !yards.iter().any(|&(f, r)| f.gap(q.placement.footprint) <= r as i32))
        .count();
    let mut costs: Vec<(ItemId, u32)> = Vec::new();
    for q in &pieces {
        for c in &defs.building(q.building).cost {
            match costs.iter_mut().find(|(i, _)| *i == c.item) {
                Some((_, n)) => *n += c.count,
                None => costs.push((c.item, c.count)),
            }
        }
    }
    let have_all = costs.iter().all(|&(i, n)| world.stored_total(i) >= n);
    StampView {
        goal: p.goal,
        pieces,
        links,
        touching,
        blocked,
        to_clear,
        outside_yard,
        costs,
        have_all,
        left_out,
    }
}

/// The one-line summary for the status bar: "Furniture plan: 6 machines, 4 of 4 links
/// touching. Fits here. 2 tiles to clear. Costs 52 stone, 30 wood (have all of it)."
pub fn summary(session: &Session, v: &StampView) -> String {
    let defs = &session.defs;
    let mut parts = vec![format!(
        "{} plan: {} machines, {} of {} links touching.",
        v.goal,
        v.pieces.len(),
        v.touching,
        v.links
    )];
    parts.push(if v.blocked == 0 {
        "Fits here.".to_owned()
    } else {
        format!("{} blocked here.", v.blocked)
    });
    if v.to_clear > 0 {
        parts.push(format!("{} tiles to clear (E).", v.to_clear));
    }
    if v.outside_yard > 0 {
        parts.push(format!("{} outside any yard: crafters will work them.", v.outside_yard));
    }
    let costs: Vec<String> =
        v.costs.iter().map(|&(i, n)| format!("{n} {}", defs.item(i).name.to_lowercase())).collect();
    if !costs.is_empty() {
        let have = if v.have_all { "have all of it" } else { "short of some" };
        parts.push(format!("Costs {} ({have}).", costs.join(", ")));
    }
    if !v.left_out.is_empty() {
        parts.push(format!("Place yourself: {}.", v.left_out.join(", ")));
    }
    parts.join(" ")
}

/// `Enter`: every piece that fits becomes a build order. Returns what to tell the player.
pub fn place(session: &mut Session, edit: &StampEdit, at: Pos) -> String {
    let v = view(session, edit, at);
    if v.pieces.is_empty() {
        return "Nothing in this plan to stamp: place its machines yourself".to_owned();
    }
    let mut placed = 0;
    for q in v.pieces.iter().filter(|q| q.placement.ok()) {
        let cmd =
            Command::DesignateBuild { pos: q.placement.footprint.anchor, building: q.building };
        if session.apply(cmd).is_ok() {
            placed += 1;
        }
    }
    let skipped = v.pieces.len() - placed;
    match (placed, skipped) {
        (0, _) => "Nothing placed: every machine is blocked here".to_owned(),
        (n, 0) => format!("Stamped the {} plan: {n} build orders", v.goal.to_lowercase()),
        (n, k) => format!(
            "Stamped the {} plan: {n} build orders, {k} blocked and left out",
            v.goal.to_lowercase()
        ),
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;

    fn republic(nation: Nation) -> Session {
        let mut s = Session::new(7, nation, 64, 64);
        let defs = s.defs.clone();
        s.world.start_republic(&defs);
        s
    }

    fn fittings(s: &Session) -> StampEdit {
        StampEdit { plan: PlanEdit::new(s.defs.item_id("fittings").unwrap()), turned: false }
    }

    #[test]
    fn the_block_stands_each_step_against_the_next() {
        let s = republic(Nation::Earth);
        let edit = fittings(&s);
        let at = s.world.units[0].pos;
        let v = view(&s, &edit, Pos::new(at.x + 4, at.y + 4));
        assert!(!v.pieces.is_empty(), "{v:?}");
        assert_eq!(v.touching, v.links, "every touching link touches as laid out: {v:?}");
        let turned = view(&s, &StampEdit { turned: true, ..edit }, Pos::new(at.x + 4, at.y + 4));
        assert_eq!(turned.touching, turned.links);
        assert_eq!(turned.pieces.len(), v.pieces.len());
        assert!(summary(&s, &v).starts_with("Fittings plan:"), "{}", summary(&s, &v));
    }

    #[test]
    fn enter_turns_the_block_into_build_orders() {
        let mut s = republic(Nation::Earth);
        let edit = fittings(&s);
        // Find a spot where the whole block fits.
        let spots: Vec<Pos> = s.world.map.iter().map(|(p, _)| p).collect();
        let at = spots
            .into_iter()
            .find(|&p| {
                let v = view(&s, &edit, p);
                !v.pieces.is_empty() && v.blocked == 0
            })
            .expect("somewhere it fits");
        let n = view(&s, &edit, at).pieces.len();
        let before = s.world.build_sites().count();
        let said = place(&mut s, &edit, at);
        assert_eq!(s.world.build_sites().count(), before + n, "{said}");
        assert!(said.starts_with("Stamped the fittings plan"), "{said}");
    }

    #[test]
    fn a_mine_at_the_rock_face_is_left_out_and_named() {
        let s = republic(Nation::Earth);
        let edit =
            StampEdit { plan: PlanEdit::new(s.defs.item_id("gold_ore").unwrap()), turned: false };
        let v = view(&s, &edit, Pos::new(10, 10));
        assert!(v.pieces.is_empty());
        assert!(v.left_out.iter().any(|l| l.contains("rock face")), "{:?}", v.left_out);
    }

    /// Enter in the planner starts stamping; R turns the block; Esc goes back to the plan;
    /// Enter again places it.
    #[test]
    fn the_planner_s_enter_stamps_and_the_stamp_s_keys_work() {
        use crate::app::App;
        use crate::keys::Action;
        use crate::state::Screen;
        let s = republic(Nation::Earth);
        let edit = fittings(&s);
        let spots: Vec<Pos> = s.world.map.iter().map(|(p, _)| p).collect();
        let at = spots
            .into_iter()
            .find(|&p| {
                let v = view(&s, &edit, p);
                !v.pieces.is_empty() && v.blocked == 0
            })
            .expect("somewhere it fits");
        let mut app = App::new(s, (120, 40));
        app.ui.screen = Some(Screen::Planner);
        app.ui.planner = Some(edit.plan);
        app.act(Action::Confirm);
        assert_eq!(app.ui.screen, None);
        assert!(app.ui.stamp.is_some());
        assert!(app.ui.status.starts_with("Fittings plan:"), "{}", app.ui.status);
        app.act(Action::NextRecipe);
        assert!(app.ui.stamp.unwrap().turned);
        app.act(Action::Cancel);
        assert_eq!(app.ui.screen, Some(Screen::Planner), "back to the plan");
        app.act(Action::Confirm);
        app.ui.cursor = at;
        let before = app.session.world.build_sites().count();
        app.act(Action::Confirm);
        assert!(app.ui.stamp.is_none());
        assert!(app.session.world.build_sites().count() > before, "{}", app.ui.notice);
    }
}

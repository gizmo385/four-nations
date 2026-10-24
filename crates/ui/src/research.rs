//! Research (#23) as the panels show it: the side panel's progress line and the tech-tree
//! screen's view. Reads `World::research` and formats it; the rule for which techs a nation can
//! see is the sim's (`Defs::techs_for`). Choosing a tech issues `Command::SetResearch`; `fn_sim`
//! does the rest (`docs/design/research.md`).

use fn_sim::Defs;
use fn_sim::research::{MILLIS_PER_POINT, Research, TechDef, TechId};

use crate::session::Session;
use crate::view::{humanize, multiplier};

/// The side panel's Research section: the tech the pool is currently paying for, its progress,
/// and the knowledge rate.
#[derive(Clone, PartialEq, Debug)]
pub struct ResearchStatus {
    /// The tech currently being paid for (the queue's front), or a "no target" message.
    pub label: String,
    /// Progress toward `label`, in `[0, 1]`. Zero when nothing is queued.
    pub frac: f32,
    /// "18/25 pts" while queued, "42 pts banked" otherwise.
    pub pool: String,
    /// "3.0/min".
    pub rate: String,
    /// Further techs queued behind `label` (the rest of the chosen tech's prereq chain).
    pub more_queued: usize,
}

/// Knowledge millis as a fractional point count.
fn points(millis: u64) -> f64 {
    millis as f64 / MILLIS_PER_POINT as f64
}

pub fn research_status(research: &Research, defs: &Defs) -> ResearchStatus {
    let rate = format!("{:.1}/min", defs.research.passive_knowledge_per_minute);
    let pool = points(research.pool_millis());
    let Some(&front) = research.queue().first() else {
        return ResearchStatus {
            label: "No target set".to_owned(),
            frac: 0.0,
            pool: format!("{pool:.0} pts banked"),
            rate,
            more_queued: 0,
        };
    };
    let cost = f64::from(defs.tech(front).cost);
    ResearchStatus {
        label: defs.tech(front).name.clone(),
        frac: (pool / cost).clamp(0.0, 1.0) as f32,
        pool: format!("{pool:.0}/{cost:.0} pts"),
        rate,
        more_queued: research.queue().len() - 1,
    }
}

/// How a tech reads in the screen: never both known and queued, so one colour tells you which.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TechState {
    Known,
    /// In the current queue: either the chosen target or one of its unknown prereqs.
    Queued,
    Available,
    /// At least one prereq isn't known yet. Still selectable: `Research::set_target` queues the
    /// whole chain.
    Locked,
}

impl TechState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Known => "Known",
            Self::Queued => "Queued",
            Self::Available => "Available",
            Self::Locked => "Needs prereqs",
        }
    }
}

pub fn tech_state(research: &Research, id: TechId, def: &TechDef) -> TechState {
    if research.is_known(id) {
        TechState::Known
    } else if research.queue().contains(&id) {
        TechState::Queued
    } else if def.prereqs.iter().all(|p| research.is_known(*p)) {
        TechState::Available
    } else {
        TechState::Locked
    }
}

/// Each tech's depth: 0 for one with no prereqs, otherwise one past its deepest prereq. Prereqs
/// always have a smaller `TechId` (`research.rs`'s file-order invariant), so a single ascending
/// pass is enough.
pub fn tech_depths(techs: &[TechDef]) -> Vec<usize> {
    let mut depths = vec![0usize; techs.len()];
    for (i, t) in techs.iter().enumerate() {
        depths[i] = t.prereqs.iter().map(|p| depths[p.0 as usize] + 1).max().unwrap_or(0);
    }
    depths
}

/// "Study x1.25, Air Scooter" (a modifier and an ability this tech unlocks), or "-" for neither.
/// Buildings and jobs aren't listed here (out of scope; `docs/design/research.md` "Unlocks").
pub fn unlock_summary(defs: &Defs, id: TechId, def: &TechDef) -> String {
    let mut parts: Vec<String> = def
        .unlocks
        .modifiers
        .iter()
        .map(|(n, m)| format!("{} {}", humanize(n), multiplier(m)))
        .collect();
    parts.extend(
        defs.abilities.iter().filter(|a| a.unlocked_by == Some(id)).map(|a| a.name.clone()),
    );
    if parts.is_empty() { "-".to_owned() } else { parts.join(", ") }
}

#[derive(Clone, PartialEq, Debug)]
pub struct TechCard {
    pub id: TechId,
    pub name: String,
    pub cost: u32,
    pub state: TechState,
    pub unlock: String,
}

/// Columns by depth, each holding the techs at that depth: shared techs plus the nation's own
/// branch, in `TechId` order within a column. Other nations' branches are never built.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ScreenView {
    pub columns: Vec<Vec<TechCard>>,
}

pub fn screen_view(session: &Session) -> ScreenView {
    let defs = &session.defs;
    let research = session.world.research();
    let depths = tech_depths(&defs.techs);
    let mut columns: Vec<Vec<TechCard>> = Vec::new();
    for (id, def) in defs.techs_for(session.world.nation) {
        let depth = depths[id.0 as usize];
        if columns.len() <= depth {
            columns.resize(depth + 1, Vec::new());
        }
        columns[depth].push(TechCard {
            id,
            name: def.name.clone(),
            cost: def.cost,
            state: tech_state(research, id, def),
            unlock: unlock_summary(defs, id, def),
        });
    }
    ScreenView { columns }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use fn_sim::research::Unlocks;
    use fn_sim::world::Command;

    use super::*;

    fn def(nation: Option<Nation>, cost: u32, prereqs: &[TechId]) -> TechDef {
        TechDef {
            id: "x".into(),
            name: "X".into(),
            description: String::new(),
            nation,
            cost,
            prereqs: prereqs.to_vec(),
            unlocks: Unlocks::default(),
        }
    }

    #[test]
    fn depths_follow_the_deepest_prereq() {
        let techs = vec![
            def(None, 1, &[]),                     // 0: depth 0
            def(None, 1, &[]),                     // 1: depth 0
            def(None, 1, &[TechId(0)]),            // 2: depth 1
            def(None, 1, &[TechId(1), TechId(2)]), // 3: depth 2
        ];
        assert_eq!(tech_depths(&techs), [0, 0, 1, 2]);
    }

    #[test]
    fn depths_are_empty_for_no_techs() {
        assert_eq!(tech_depths(&[]), Vec::<usize>::new());
    }

    #[test]
    fn tech_state_reflects_known_queued_and_locked() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let research = s.world.research();
        let defs = &s.defs;
        let fm = defs.tech_id("fire_making").unwrap();
        assert_eq!(tech_state(research, fm, defs.tech(fm)), TechState::Known);
        let shelter = defs.tech_id("shelter").unwrap();
        assert_eq!(tech_state(research, shelter, defs.tech(shelter)), TechState::Available);
        let masonry = defs.tech_id("masonry").unwrap();
        assert_eq!(tech_state(research, masonry, defs.tech(masonry)), TechState::Locked);
    }

    #[test]
    fn set_research_marks_target_and_its_prereqs_as_queued() {
        let mut s = Session::new(7, Nation::Fire, 48, 32);
        let masonry = s.defs.tech_id("masonry").unwrap();
        s.apply(Command::SetResearch { tech: masonry }).unwrap();
        let research = s.world.research();
        assert_eq!(tech_state(research, masonry, s.defs.tech(masonry)), TechState::Queued);
        let carpentry = s.defs.tech_id("carpentry").unwrap();
        assert_eq!(tech_state(research, carpentry, s.defs.tech(carpentry)), TechState::Queued);
    }

    #[test]
    fn unlock_summary_names_modifiers_and_is_a_dash_otherwise() {
        let defs = Defs::bundled();
        let writing = defs.tech_id("writing").unwrap();
        let summary = unlock_summary(&defs, writing, defs.tech(writing));
        assert_eq!(summary, "Study x1.25");
        let fire_making = defs.tech_id("fire_making").unwrap();
        assert_eq!(unlock_summary(&defs, fire_making, defs.tech(fire_making)), "-");
    }

    #[test]
    fn screen_view_hides_other_nations_branches() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let view = screen_view(&s);
        let names: Vec<_> = view.columns.iter().flatten().map(|c| c.name.clone()).collect();
        // A Water-only tech never shows for a Fire player.
        let healing = s.defs.tech(s.defs.tech_id("healing").unwrap());
        assert!(!names.contains(&healing.name), "{names:?}");
        // The shared tier and Fire's own branch both show.
        assert!(names.contains(&"Fire Making".to_owned()), "{names:?}");
        let cards: usize = view.columns.iter().map(Vec::len).sum();
        assert_eq!(cards, s.defs.techs_for(Nation::Fire).count());
    }

    #[test]
    fn research_status_without_a_target_banks_knowledge() {
        let mut s = Session::new(7, Nation::Fire, 48, 32);
        s.tick_n(1000);
        let status = research_status(s.world.research(), &s.defs);
        assert_eq!(status.label, "No target set");
        assert_eq!(status.frac, 0.0);
        assert!(status.pool.ends_with("pts banked"), "{}", status.pool);
    }

    #[test]
    fn research_status_with_a_target_shows_progress_toward_the_front() {
        let mut s = Session::new(7, Nation::Fire, 48, 32);
        let shelter = s.defs.tech_id("shelter").unwrap();
        s.apply(Command::SetResearch { tech: shelter }).unwrap();
        let status = research_status(s.world.research(), &s.defs);
        assert_eq!(status.label, "Shelter");
        assert_eq!(status.more_queued, 0);
        assert!(status.frac >= 0.0 && status.frac <= 1.0);
    }
}

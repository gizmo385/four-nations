//! Research (#23) as the panels show it: the side panel's progress line and the tech-tree
//! screen's view. Reads `World::research` and formats it; the rule for which techs a nation can
//! see is the sim's (`Defs::techs_for`). Choosing a tech issues `Command::SetResearch`; `fn_sim`
//! does the rest (`docs/design/research.md`).

use fn_sim::research::{Feature, MILLIS_PER_POINT, Research, TechDef, TechId};
use fn_sim::{Defs, Nation, World};

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
    /// "3.0/min" with nobody studying, else "3.0 base + 12.0 study" (#338): the flat passive
    /// trickle alone read as the whole rate, hiding study's much bigger contribution (a library
    /// full of scholars can be several times it).
    pub rate: String,
    /// Further techs queued behind `label` (the rest of the chosen tech's prereq chain).
    pub more_queued: usize,
}

/// Knowledge millis as a fractional point count.
fn points(millis: u64) -> f64 {
    millis as f64 / MILLIS_PER_POINT as f64
}

/// The settlement's current knowledge rate, split by source (#338): the flat passive trickle,
/// and every seated scholar's `study_knowledge_per_minute`, scaled by the nation's effective
/// `study` modifier the same way `run_study` adds it tick by tick
/// (`World::study_seats_filled`). A unit still walking to a seat contributes nothing yet.
fn effective_rate(world: &World, defs: &Defs) -> (f64, f64) {
    let base = defs.research.passive_knowledge_per_minute;
    let seats = world.study_seats_filled() as f64;
    let study =
        seats * defs.research.study_knowledge_per_minute * f64::from(world.modifiers().study);
    (base, study)
}

/// "3.0/min" with no study contribution, else "3.0 base + 12.0 study" (#338's literal example):
/// naming the source is the point, so a player isn't left doing the addition from a flat number
/// that doesn't match how long techs actually take.
fn rate_text(base: f64, study: f64) -> String {
    if study > 0.0 {
        format!("{base:.1} base + {study:.1} study")
    } else {
        format!("{base:.1}/min")
    }
}

pub fn research_status(world: &World, defs: &Defs) -> ResearchStatus {
    let research = world.research();
    let (base, study) = effective_rate(world, defs);
    let rate = rate_text(base, study);
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

/// "Study x1.25, Workshop, Academy +2 seats, Air Scooter" (every modifier, building, upgrade,
/// job and ability `id` unlocks), in that order. A tech that grants none of those outright still has a reason to
/// research (#338): naming the tech it is a prereq for ("Leads to Writing"), so the
/// column never reads as a dead end while it's actually a stepping stone. Only a tech with
/// neither -- truly nothing downstream wires up to yet, a content gap, not a display bug -- gets
/// a plain "-" (`cartography` once did; #402 wired up its Map Room. `herbalism` once did too,
/// for every nation but Water; #390's Herb Garden wired that up as well. No bundled tech reads
/// this way today, which is the point: the dash is a prompt to close a real gap, not a steady
/// state, so a unit test builds a `TechDef` of its own to exercise it).
pub fn unlock_summary(defs: &Defs, nation: Nation, id: TechId, def: &TechDef) -> String {
    let mut parts: Vec<String> = def
        .unlocks
        .modifiers
        .iter()
        .map(|(n, m)| format!("{} {}", humanize(n), multiplier(m)))
        .collect();
    parts.extend(
        def.unlocks
            .buildings
            .iter()
            .filter_map(|b| defs.building_id(b))
            .map(|b| defs.building(b).display_name(nation).to_owned()),
    );
    parts.extend(def.unlocks.upgrades.iter().filter_map(|u| {
        let b = defs.building(defs.building_id(&u.building)?);
        Some(format!("{} +{} seats", b.display_name(nation), u.seats))
    }));
    parts.extend(
        def.unlocks
            .jobs
            .iter()
            .filter_map(|r| defs.resource_id(r))
            .map(|r| defs.resource(r).name.clone()),
    );
    parts.extend(
        defs.abilities.iter().filter(|a| a.unlocked_by == Some(id)).map(|a| a.name.clone()),
    );
    parts.extend(def.unlocks.features.iter().map(|f| feature_name(*f).to_owned()));
    if !parts.is_empty() {
        return parts.join(", ");
    }
    // Only a dependent `nation` could ever see (shared, or its own branch): naming another
    // nation's tech would leak its branch onto a screen that otherwise never shows it
    // (`screen_view`, `Defs::techs_for`).
    let dependents: Vec<&str> = defs
        .techs
        .iter()
        .filter(|t| t.prereqs.contains(&id) && t.open_to(nation))
        .map(|t| t.name.as_str())
        .collect();
    if dependents.is_empty() {
        "-".to_owned()
    } else {
        format!("Leads to {}", dependents.join(", "))
    }
}

/// A feature (`Unlocks::features`) as the Research screen names it.
pub fn feature_name(feature: Feature) -> &'static str {
    match feature {
        Feature::Almanac => "Almanac",
        Feature::StandingSow => "Standing sow order",
    }
}

/// Names of `def`'s prereqs not yet known, in the order the data lists them. Empty once every
/// prereq is known, which is every state but `Locked` (`tech_state`'s own definition of
/// `Available`). What a Locked card's own row names (#338), instead of leaving the state word
/// alone to hide which tier-1 tech unlocks it.
pub fn missing_prereqs(research: &Research, defs: &Defs, def: &TechDef) -> Vec<String> {
    def.prereqs
        .iter()
        .filter(|p| !research.is_known(**p))
        .map(|p| defs.tech(*p).name.clone())
        .collect()
}

#[derive(Clone, PartialEq, Debug)]
pub struct TechCard {
    pub id: TechId,
    pub name: String,
    pub cost: u32,
    pub state: TechState,
    /// `unlock_summary`, or empty while `state` is `Locked` (#338): a Locked card's own row
    /// never shows what it would eventually unlock (`needs` below, not this, is what it shows),
    /// so this stays empty rather than claim information nowhere on screen actually offers --
    /// `screen {"research"}`'s own parity with the rendered frame (docs/design/agent-play.md,
    /// "Observation containment").
    pub unlock: String,
    /// "Masonry, Carpentry" while `state` is `Locked`; empty otherwise (`missing_prereqs`).
    pub needs: String,
    /// The current research target (the last tech of the queue): the one tech the player chose,
    /// as against the prereqs queued ahead of it (#659).
    pub target: bool,
}

/// Columns by depth, each holding the techs at that depth: shared techs plus the nation's own
/// branch, in `TechId` order within a column. Other nations' branches are never built.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ScreenView {
    pub columns: Vec<Vec<TechCard>>,
}

pub fn screen_view(session: &Session) -> ScreenView {
    let defs = &session.defs;
    let nation = session.world.nation;
    let research = session.world.research();
    let depths = tech_depths(&defs.techs);
    let target = research.queue().last().copied();
    let mut columns: Vec<Vec<TechCard>> = Vec::new();
    for (id, def) in defs.techs_for(nation) {
        let depth = depths[id.0 as usize];
        if columns.len() <= depth {
            columns.resize(depth + 1, Vec::new());
        }
        let state = tech_state(research, id, def);
        columns[depth].push(TechCard {
            id,
            name: def.name.clone(),
            cost: def.cost,
            state,
            // Locked never shows its own `unlock_summary`, on the card or anywhere else
            // (`TechCard::unlock`'s own doc): nothing reachable would ever display it.
            unlock: if state == TechState::Locked {
                String::new()
            } else {
                unlock_summary(defs, nation, id, def)
            },
            needs: missing_prereqs(research, defs, def).join(", "),
            target: target == Some(id),
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
            nations: Vec::new(),
            cost,
            prereqs: prereqs.to_vec(),
            unlocks: Unlocks::default(),
            core: true,
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
    fn unlock_summary_names_modifiers_buildings_and_jobs() {
        let defs = Defs::bundled();
        let writing = defs.tech_id("writing").unwrap();
        let summary = unlock_summary(&defs, Nation::Fire, writing, defs.tech(writing));
        // Its upgrade reads with the nation's own name for the hall.
        let hall = defs.building(defs.building_id("study_hall").unwrap());
        assert_eq!(summary, format!("Study x1.25, {} +2 seats", hall.display_name(Nation::Fire)));
        // Carpentry both multiplies construction and unlocks the Workshop (#369): a player
        // couldn't tell the building was there from the modifier alone (the issue's evidence).
        let carpentry = defs.tech_id("carpentry").unwrap();
        assert_eq!(
            unlock_summary(&defs, Nation::Fire, carpentry, defs.tech(carpentry)),
            "Construction x1.1, Workshop"
        );
        let calendar = defs.tech_id("calendar").unwrap();
        assert_eq!(
            unlock_summary(&defs, Nation::Earth, calendar, defs.tech(calendar)),
            "Almanac, Standing sow order"
        );
        let fishing = defs.tech_id("fishing").unwrap();
        assert_eq!(
            unlock_summary(&defs, Nation::Water, fishing, defs.tech(fishing)),
            "Fishing Spot"
        );
    }

    #[test]
    fn a_tech_with_no_direct_unlock_names_what_it_is_a_prereq_for() {
        let defs = Defs::bundled();
        // Pottery grants nothing on its own, but Writing (and Fire's Brickmaking) need it (the
        // issue's evidence: the unlock column used to read "-" with no way to tell it mattered
        // at all).
        let pottery = defs.tech_id("pottery").unwrap();
        assert_eq!(
            unlock_summary(&defs, Nation::Fire, pottery, defs.tech(pottery)),
            "Leads to Writing, Brickmaking"
        );
    }

    #[test]
    fn cartography_no_longer_reads_as_a_dead_end() {
        // #402 gave cartography its Map Room, closing the gap this test used to document.
        let defs = Defs::bundled();
        let cartography = defs.tech_id("cartography").unwrap();
        assert_eq!(
            unlock_summary(&defs, Nation::Fire, cartography, defs.tech(cartography)),
            "Map Room"
        );
    }

    #[test]
    fn herbalism_no_longer_reads_as_a_dead_end() {
        // #390 gave herbalism its Herb Garden, closing the gap the test below used to document
        // for herbalism specifically (every nation now sees it unlock something directly, not
        // just Water's own Healing branch).
        let defs = Defs::bundled();
        let herbalism = defs.tech_id("herbalism").unwrap();
        assert_eq!(
            unlock_summary(&defs, Nation::Fire, herbalism, defs.tech(herbalism)),
            "Herb Garden"
        );
    }

    #[test]
    fn a_tech_that_truly_unlocks_nothing_yet_is_a_dash() {
        // No bundled tech has this shape today (every one either unlocks something directly or
        // is a prereq for something that does); a `TechDef` built here, under an id no real
        // tech names as a prereq, exercises the dash branch on its own rather than leaning on
        // content staying sparse.
        let defs = Defs::bundled();
        let id = TechId(defs.techs.len() as u16 + 1);
        let nothing = def(None, 1, &[]);
        assert_eq!(unlock_summary(&defs, Nation::Fire, id, &nothing), "-");
    }

    #[test]
    fn a_locked_cards_needs_names_its_unmet_prereqs() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let defs = &s.defs;
        let research = s.world.research();
        let masonry = defs.tech_id("masonry").unwrap();
        assert_eq!(tech_state(research, masonry, defs.tech(masonry)), TechState::Locked);
        // Fire starts knowing only `fire_making`, so both of masonry's prereqs are still missing
        // (the issue's evidence: a Locked card alone never said which techs would unlock it).
        assert_eq!(
            missing_prereqs(research, defs, defs.tech(masonry)),
            ["Stoneworking", "Carpentry"]
        );
        let fire_making = defs.tech_id("fire_making").unwrap();
        assert!(missing_prereqs(research, defs, defs.tech(fire_making)).is_empty());
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

    /// A Locked card's `unlock` is empty: nothing reachable ever shows it (the grid names what
    /// it still needs instead), so `screen {"research"}` must not carry it either (#338,
    /// docs/design/agent-play.md "Observation containment").
    #[test]
    fn a_locked_cards_unlock_is_empty_but_an_available_cards_is_not() {
        let s = Session::new(7, Nation::Earth, 48, 32);
        let view = screen_view(&s);
        let masonry = s.defs.tech_id("masonry").unwrap();
        let card = |id| view.columns.iter().flatten().find(|c| c.id == id).unwrap();
        let masonry_card = card(masonry);
        assert_eq!(masonry_card.state, TechState::Locked);
        assert_eq!(masonry_card.unlock, "");
        assert_eq!(masonry_card.needs, "Carpentry");
        let earth_shaping = s.defs.tech_id("earth_shaping").unwrap();
        let shaping_card = card(earth_shaping);
        assert_eq!(shaping_card.state, TechState::Available);
        assert_eq!(shaping_card.unlock, "Earthworks x1.2, Earth Tent");
        assert_eq!(shaping_card.needs, "");
    }

    #[test]
    fn research_status_without_a_target_banks_knowledge() {
        let mut s = Session::new(7, Nation::Fire, 48, 32);
        s.tick_n(1000);
        let status = research_status(&s.world, &s.defs);
        assert_eq!(status.label, "No target set");
        assert_eq!(status.frac, 0.0);
        assert!(status.pool.ends_with("pts banked"), "{}", status.pool);
        assert_eq!(status.rate, "3.0/min", "nobody is studying yet");
    }

    #[test]
    fn research_status_with_a_target_shows_progress_toward_the_front() {
        let mut s = Session::new(7, Nation::Fire, 48, 32);
        let shelter = s.defs.tech_id("shelter").unwrap();
        s.apply(Command::SetResearch { tech: shelter }).unwrap();
        let status = research_status(&s.world, &s.defs);
        assert_eq!(status.label, "Shelter");
        assert_eq!(status.more_queued, 0);
        assert!(status.frac >= 0.0 && status.frac <= 1.0);
    }

    #[test]
    fn rate_text_names_study_once_it_contributes() {
        // `World::study_seats_filled`'s own tests (`world/study/tests.rs`) cover the sim side of
        // this; what belongs here is the formatting (#338's literal example).
        assert_eq!(rate_text(3.0, 0.0), "3.0/min");
        assert_eq!(rate_text(3.0, 12.0), "3.0 base + 12.0 study");
    }
}

//! Research (#23) as the panels show it: the side panel's progress line and the tech-tree
//! screen's view. Reads `World::research` and formats it; the rule for which techs a nation can
//! see is the sim's (`Defs::techs_for`). Choosing a tech issues `Command::SetResearch`; `fn_sim`
//! does the rest (`docs/design/research.md`).

use fn_sim::building::ArchiveDef;
use fn_sim::research::{Facts, Feature, MILLIS_PER_POINT, Research, TechDef, TechId};
use fn_sim::world::Command;
use fn_sim::{Defs, Nation, World};

use crate::session::Session;
use crate::view::{humanize, multiplier};

/// The side panel's Research section: the tech the pool is currently paying for, its progress,
/// and the knowledge rate.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
    /// With no target and an Archive standing, what the scholars are doing instead: "Copying
    /// scrolls: 90/150", or "Copying paused: 12 in stores" at the Archive's stock limit (#712).
    /// `None` otherwise.
    pub copying: Option<String>,
    /// While the front tech's item bill is short (docs/design/horizons-phase-4.md, "Mockups"):
    /// "Metal 5/8 on the way" when stores hold what it still owes, else "Waiting for 3 Metal".
    /// `None` otherwise.
    pub bill: Option<String>,
}

/// What an Archive's scribing is doing (`World::scribing_on`): nothing while a target is set,
/// paused at the stock limit, else the points toward the next copy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Scribing {
    /// A research target is set: knowledge goes to the tech.
    Off,
    /// The stock limit is reached: this many copies are in stores or waiting for a haul.
    Paused { in_stores: u32 },
    /// Whole knowledge points toward the next copy, out of the `per` points one costs.
    Copying { points: u32, per: u32 },
}

/// The first finished Archive's def in row-major order, the one scribing runs at (`None` with
/// none standing).
pub fn archive_def(world: &World, defs: &Defs) -> Option<ArchiveDef> {
    world.buildings().find_map(|b| defs.building(b.id).archive)
}

/// What scribing at `archive` is doing now.
pub fn scribing(world: &World, archive: &ArchiveDef) -> Scribing {
    if !world.research().queue().is_empty() {
        return Scribing::Off;
    }
    let held = world.copies_held(archive.copy_item);
    if held >= archive.stock_limit {
        return Scribing::Paused { in_stores: held };
    }
    let points = (world.research().scribe_millis() / MILLIS_PER_POINT) as u32;
    Scribing::Copying { points: points.min(archive.copy_points), per: archive.copy_points }
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
            copying: archive_def(world, defs).map(|a| match scribing(world, &a) {
                Scribing::Paused { in_stores } => format!("Copying paused: {in_stores} in stores"),
                Scribing::Copying { points, per } => format!("Copying scrolls: {points}/{per}"),
                Scribing::Off => String::new(),
            }),
            bill: None,
        };
    };
    let cost = f64::from(defs.tech(front).cost);
    ResearchStatus {
        label: defs.tech(front).name.clone(),
        frac: (pool / cost).clamp(0.0, 1.0) as f32,
        pool: format!("{pool:.0}/{cost:.0} pts"),
        rate,
        more_queued: research.queue().len() - 1,
        copying: None,
        bill: bill_status(world, defs, front),
    }
}

/// The side panel's line under the Research row while `tech`'s bill is short: "Metal 5/8 on the
/// way" when stores hold something of every short line, else "Waiting for 3 Metal" for the first
/// line stores hold none of.
fn bill_status(world: &World, defs: &Defs, tech: TechId) -> Option<String> {
    let bill = world.research().bill(tech, defs);
    if bill.iter().all(|&(_, got, need)| got >= need) {
        return None;
    }
    let lacking = bill.iter().find(|&&(item, got, need)| {
        got < need && !defs.item(item).is_readable() && world.stored_total(item) == 0
    });
    Some(match lacking {
        Some(&(item, got, need)) => format!("Waiting for {} {}", need - got, defs.item(item).name),
        None => format!("{} on the way", bill_text(world.research(), defs, tech)),
    })
}

/// `X` on the Research screen: empties the research queue so the scholars scribe
/// (`Command::ClearResearch`, #709). The outcome as a status line either way; the sim refuses it
/// without an Archive, and clearing an empty queue changes nothing.
pub fn clear_target(session: &mut Session) -> Result<String, String> {
    match session.apply(Command::ClearResearch) {
        Ok(()) => Ok("Research target cleared".to_owned()),
        Err(e) => Err(format!("Can't clear the target: {e}")),
    }
}

/// How a tech reads in the screen: never both known and queued, so one colour tells you which.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum TechState {
    Known,
    /// In the current queue: either the chosen target or one of its unknown prereqs.
    Queued,
    Available,
    /// At least one prereq isn't known yet, or a `requires` condition isn't met. Still
    /// selectable while only prereqs are missing: `Research::set_target` queues the whole chain.
    /// A condition it refuses (`ResearchError::NotYet`) until it holds.
    Locked,
    /// A school option whose mate is learned (docs/design/horizons-phase-4.md): it can never be
    /// researched in this game.
    Closed,
}

impl TechState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Known => "Known",
            Self::Queued => "Queued",
            Self::Available => "Available",
            Self::Locked => "Needs prereqs",
            Self::Closed => "Closed",
        }
    }
}

pub fn tech_state(
    research: &Research,
    facts: &Facts<'_>,
    defs: &Defs,
    id: TechId,
    def: &TechDef,
) -> TechState {
    if research.is_known(id) {
        TechState::Known
    } else if research.closed_by(id, defs).is_some() {
        TechState::Closed
    } else if research.queue().contains(&id) {
        TechState::Queued
    } else if def.prereqs.iter().all(|p| research.is_known(*p))
        && def.requires.unmet(facts, defs).is_empty()
    {
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
        if let (Some(item), Some(target)) = (&u.item, &u.stands_for) {
            // "Ice pays for stone" (#726).
            let name = |id: &str| defs.item_id(id).map(|i| defs.item(i).name.clone());
            return Some(format!("{} pays for {}", name(item)?, name(target)?.to_lowercase()));
        }
        let b = defs.building(defs.building_id(&u.building)?);
        if let Some(id) = &u.recipe {
            let alt = b.alt_recipes.iter().find(|a| a.id == *id)?;
            return Some(format!(
                "{} makes {}",
                b.display_name(nation),
                defs.item(alt.recipe.output.item).name
            ));
        }
        let plural = || format!("{}s", b.display_name(nation));
        if u.free == Some(true) {
            // "Waystations cost only work" (#727).
            return Some(format!("{} cost only work", plural()));
        }
        if let Some(a) = u.aura {
            // "Campfire heals 2x and restores chi 1.5x nearby" (#724).
            let mut what = Vec::new();
            if let Some(h) = a.heal.filter(|&h| h > 1.0) {
                what.push(format!("heals {h}x"));
            }
            if let Some(c) = a.chi_regen.filter(|&c| c > 1.0) {
                what.push(format!("restores chi {c}x"));
            }
            if let Some(s) = a.study.filter(|&s| s > 1.0) {
                what.push(format!("speeds study {s}x"));
            }
            return Some(format!("{} {} nearby", b.display_name(nation), what.join(" and ")));
        }
        if u.fuel_free == Some(true) {
            return Some(format!("{} needs no fuel", b.display_name(nation)));
        }
        if let Some(a) = u.automate {
            return Some(format!(
                "Machines within {} tiles of a {} run with no crafter",
                a.radius,
                b.display_name(nation)
            ));
        }
        if let Some(boost) = u.boost {
            // "Hot Spring Mill powers x1.5" (#722).
            return Some(format!("{} powers x{boost}", b.display_name(nation)));
        }
        if let Some(r) = u.shelter {
            return Some(format!("{} keep crews within {r} tiles warm", plural()));
        }
        Some(format!("{} +{} seats", b.display_name(nation), u.seats?))
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
        Feature::PackUp => "Pack up (buildings come down whole, twice as fast)",
        Feature::ShoreFishing => "Fish from the shore",
        Feature::ShelteredRings => "Sheltered rings",
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

/// What `def` still needs besides prereqs (`TechDef::requires`), as the Locked line words it:
/// "3 scrolls read", "Known on the road". A condition that is met drops off, the way a known
/// prereq does (`missing_prereqs`).
pub fn unmet_conditions(facts: &Facts<'_>, defs: &Defs, def: &TechDef) -> Vec<String> {
    def.requires.unmet(facts, defs).iter().map(ToString::to_string).collect()
}

/// Everything a Locked card still needs, in the order it is listed: missing prereqs first
/// (`missing_prereqs`), then unmet conditions (`unmet_conditions`).
pub fn missing_needs(
    research: &Research,
    facts: &Facts<'_>,
    defs: &Defs,
    def: &TechDef,
) -> Vec<String> {
    let mut needs = missing_prereqs(research, defs, def);
    needs.extend(unmet_conditions(facts, defs, def));
    needs
}

#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
    /// "Masonry, Carpentry" while `state` is `Locked`, with unmet conditions after the prereqs
    /// ("Writing, 3 scrolls read"); empty otherwise (`missing_needs`).
    pub needs: String,
    /// The current research target (the last tech of the queue): the one tech the player chose,
    /// as against the prereqs queued ahead of it (#659).
    pub target: bool,
    /// `Closed: you chose Metal City` for a school option whose mate is learned
    /// (`closed_reason`); empty otherwise.
    pub closed: String,
    /// The items its bill asks for besides points, "8 Metal" (`cost_items_text`); empty for a
    /// tech with none.
    pub items: String,
    /// Delivered against needed for each bill line while the bill is started or the tech is
    /// queued, "Metal 5/8" (`bill_text`); empty otherwise.
    pub bill: String,
    /// Where it sits in its school's fork, for a school option; `None` for every other tech.
    pub school: Option<SchoolSlot>,
}

/// A school option's place in its fork (docs/design/horizons-phase-4.md, "Mockups"): the
/// school's question heads the fork, and the second option follows an "or".
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SchoolSlot {
    /// "Which city?"
    pub question: String,
    /// The first of the school's options in the column.
    pub first: bool,
    /// The other option's name.
    pub other: String,
}

/// "8 Metal" or "40 Stone, 3 Ancient Scroll": a tech's `cost_items`, as build costs read.
pub fn cost_items_text(defs: &Defs, def: &TechDef) -> String {
    def.cost_items
        .iter()
        .map(|s| format!("{} {}", s.count, defs.item(s.item).name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// "1000 pts + 8 Metal", or "250 pts" for a tech with no items.
pub fn cost_text(cost: u32, items: &str) -> String {
    if items.is_empty() { format!("{cost} pts") } else { format!("{cost} pts + {items}") }
}

/// "Metal 5/8" for each line of `tech`'s bill, while it is queued or something is delivered;
/// empty otherwise, and for a tech with no items.
pub fn bill_text(research: &Research, defs: &Defs, tech: TechId) -> String {
    let bill = research.bill(tech, defs);
    let started = bill.iter().any(|&(_, got, _)| got > 0);
    if bill.is_empty() || !(started || research.queue().contains(&tech)) {
        return String::new();
    }
    bill.iter()
        .map(|&(item, got, need)| format!("{} {got}/{need}", defs.item(item).name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Why `tech` can't be researched any more, as the player reads it: `Closed: you chose Metal
/// City` once the other option of its school is learned (docs/design/horizons-phase-4.md).
/// `None` for every other tech.
pub fn closed_reason(research: &Research, defs: &Defs, tech: TechId) -> Option<String> {
    research
        .closed_by(tech, defs)
        .map(|chosen| format!("Closed: you chose {}", defs.tech(chosen).name))
}

/// Columns by depth, each holding the techs at that depth: shared techs plus the nation's own
/// branch, in `TechId` order within a column. Other nations' branches are never built.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ScreenView {
    pub columns: Vec<Vec<TechCard>>,
}

pub fn screen_view(session: &Session) -> ScreenView {
    let defs = &session.defs;
    let nation = session.world.nation;
    let research = session.world.research();
    let facts = session.world.facts();
    let depths = tech_depths(&defs.techs);
    let target = research.queue().last().copied();
    let mut columns: Vec<Vec<TechCard>> = Vec::new();
    for (id, def) in defs.techs_for(nation) {
        let depth = depths[id.0 as usize];
        if columns.len() <= depth {
            columns.resize(depth + 1, Vec::new());
        }
        let state = tech_state(research, &facts, defs, id, def);
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
            needs: missing_needs(research, &facts, defs, def).join(", "),
            target: target == Some(id),
            closed: closed_reason(research, defs, id).unwrap_or_default(),
            items: cost_items_text(defs, def),
            bill: bill_text(research, defs, id),
            school: def.school.map(|school| {
                let mate = defs.school_mates(id).next();
                SchoolSlot {
                    question: defs.school(school).question.clone(),
                    first: mate.is_none_or(|m| id < m),
                    other: mate.map(|m| defs.tech(m).name.clone()).unwrap_or_default(),
                }
            }),
        });
    }
    ScreenView { columns }
}

#[cfg(test)]
mod tests {
    use fn_sim::research::{Requires, Unlocks};
    use fn_sim::world::Command;
    use fn_sim::{Nation, Pos};

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
            requires: Requires::default(),
            school: None,
            cost_items: Vec::new(),
            teaching: None,
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
        let facts = s.world.facts();
        let defs = &s.defs;
        let fm = defs.tech_id("fire_making").unwrap();
        assert_eq!(tech_state(research, &facts, defs, fm, defs.tech(fm)), TechState::Known);
        let shelter = defs.tech_id("shelter").unwrap();
        assert_eq!(
            tech_state(research, &facts, defs, shelter, defs.tech(shelter)),
            TechState::Available
        );
        let masonry = defs.tech_id("masonry").unwrap();
        assert_eq!(
            tech_state(research, &facts, defs, masonry, defs.tech(masonry)),
            TechState::Locked
        );
    }

    #[test]
    fn set_research_marks_target_and_its_prereqs_as_queued() {
        let mut s = Session::new(7, Nation::Fire, 48, 32);
        let masonry = s.defs.tech_id("masonry").unwrap();
        s.apply(Command::SetResearch { tech: masonry }).unwrap();
        let research = s.world.research();
        let facts = s.world.facts();
        assert_eq!(
            tech_state(research, &facts, &s.defs, masonry, s.defs.tech(masonry)),
            TechState::Queued
        );
        let carpentry = s.defs.tech_id("carpentry").unwrap();
        assert_eq!(
            tech_state(research, &facts, &s.defs, carpentry, s.defs.tech(carpentry)),
            TechState::Queued
        );
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
        // A recipe upgrade names what the building makes (#708).
        let wares = defs.tech_id("trade_wares").unwrap();
        assert_eq!(
            unlock_summary(&defs, Nation::Fire, wares, defs.tech(wares)),
            "Workshop makes Wares"
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
        // Pottery grants nothing on its own, but Writing (Fire's Brickmaking, Tea Ceremony) need
        // it (the issue's evidence: the unlock column used to read "-" with no way to tell it
        // mattered at all).
        let pottery = defs.tech_id("pottery").unwrap();
        assert_eq!(
            unlock_summary(&defs, Nation::Fire, pottery, defs.tech(pottery)),
            "Leads to Writing, Brickmaking, Tea Ceremony"
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
        let facts = s.world.facts();
        let masonry = defs.tech_id("masonry").unwrap();
        assert_eq!(
            tech_state(research, &facts, defs, masonry, defs.tech(masonry)),
            TechState::Locked
        );
        // Fire starts knowing only `fire_making`, so both of masonry's prereqs are still missing
        // (the issue's evidence: a Locked card alone never said which techs would unlock it).
        assert_eq!(
            missing_prereqs(research, defs, defs.tech(masonry)),
            ["Stoneworking", "Carpentry"]
        );
        let fire_making = defs.tech_id("fire_making").unwrap();
        assert!(missing_prereqs(research, defs, defs.tech(fire_making)).is_empty());
    }

    /// The Locked line lists unmet conditions after the prereqs, and a met one drops off.
    #[test]
    fn a_locked_cards_needs_lists_unmet_conditions_after_prereqs() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let defs = &s.defs;
        let research = s.world.research();
        let writing = defs.tech_id("writing").unwrap();
        let mut gated = def(None, 100, &[writing]);
        gated.requires =
            Requires { scrolls_read: Some(3), chapter: Some("known_on_the_road".to_owned()) };
        let id = TechId(defs.techs.len() as u16);
        let needs = |facts: &Facts<'_>| missing_needs(research, facts, defs, &gated);
        let state = |facts: &Facts<'_>| tech_state(research, facts, defs, id, &gated);

        let none = Facts::default();
        assert_eq!(needs(&none), ["Writing", "3 scrolls read", "Known on the road"]);
        assert_eq!(state(&none), TechState::Locked);
        let two = Facts { scrolls_read: 2, ..Facts::default() };
        assert_eq!(needs(&two), ["Writing", "3 scrolls read", "Known on the road"]);
        let three = Facts { scrolls_read: 3, ..Facts::default() };
        assert_eq!(needs(&three), ["Writing", "Known on the road"]);
        let road =
            Facts { scrolls_read: 3, chapters_met: vec!["known_on_the_road"], ..Facts::default() };
        assert_eq!(needs(&road), ["Writing"]);

        // With no prereq in the way, only a condition keeps a tech Locked, and then nothing does.
        let mut open = def(None, 100, &[]);
        open.requires = gated.requires.clone();
        let state = |facts: &Facts<'_>| tech_state(research, facts, defs, id, &open);
        assert_eq!(unmet_conditions(&three, defs, &open), ["Known on the road"]);
        assert_eq!(state(&three), TechState::Locked, "the chapter is still unmet");
        assert_eq!(state(&road), TechState::Available);
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

    /// An Archive and no target: the panel's row reads the scribing counter (#712); a target, or
    /// no Archive, reads as before.
    #[test]
    fn an_archive_with_no_target_shows_the_copying_counter_instead() {
        let mut s = Session::new(7, Nation::Fire, 64, 64);
        assert_eq!(research_status(&s.world, &s.defs).copying, None, "no Archive");
        let id = s.defs.building_id("archive").unwrap();
        let home = s.world.units[0].pos;
        let at = Pos::new(home.x + 3, home.y + 3);
        s.world.put_building(fn_sim::geom::Footprint::new(at, 2, 2), id, &s.defs);
        s.tick_n(6000);
        let copying = research_status(&s.world, &s.defs).copying.expect("an Archive stands");
        assert!(copying.starts_with("Copying scrolls: "), "{copying}");
        assert!(copying.ends_with("/150"), "{copying}");
        let target = s.defs.tech_id("shelter").unwrap();
        s.apply(Command::SetResearch { tech: target }).unwrap();
        assert_eq!(research_status(&s.world, &s.defs).copying, None, "a target is set");
    }

    #[test]
    fn clearing_the_target_needs_an_archive() {
        let mut s = Session::new(7, Nation::Fire, 64, 64);
        let target = s.defs.tech_id("shelter").unwrap();
        s.apply(Command::SetResearch { tech: target }).unwrap();
        assert_eq!(
            clear_target(&mut s),
            Err("Can't clear the target: needs an Archive".to_owned())
        );
        assert!(!s.world.research().queue().is_empty());
        let id = s.defs.building_id("archive").unwrap();
        let home = s.world.units[0].pos;
        let at = Pos::new(home.x + 3, home.y + 3);
        s.world.put_building(fn_sim::geom::Footprint::new(at, 2, 2), id, &s.defs);
        assert_eq!(clear_target(&mut s), Ok("Research target cleared".to_owned()));
        assert!(s.world.research().queue().is_empty());
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

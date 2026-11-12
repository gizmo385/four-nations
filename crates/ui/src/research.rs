//! Research (#23) as the panels show it: the side panel's progress line and the tech-tree
//! screen's view. Reads `World::research` and formats it; the rule for which techs a nation can
//! see is the sim's (`Defs::techs_for`). Choosing a tech issues `Command::SetResearch`; `fn_sim`
//! does the rest (`docs/design/research.md`).

use fn_sim::building::ArchiveDef;
use fn_sim::research::{Condition, Facts, Feature, MILLIS_PER_POINT, Research, TechDef, TechId};
use fn_sim::world::{Command, ExchangeEntry, ExchangeKind, ExchangeNeed, ExchangeStanding};
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
    /// While the front tech has its points and its bill and waits on a condition that fell
    /// (docs/design/horizons-phase-6.md, "Conditions"): "Waiting: 3 Fire residents (2 live
    /// here)". `None` otherwise.
    pub waiting: Option<String>,
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
    // Fox spirits (#739) study like seated scholars.
    let seats = (world.study_seats_filled() as u32 + world.foxes_studying(defs)) as f64;
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
            waiting: None,
        };
    };
    // A borrowed tech costs more than its own `cost` (docs/design/horizons-phase-6.md).
    let cost = f64::from(defs.tech(front).cost_for(world.nation, defs.research.borrow_cost));
    ResearchStatus {
        label: defs.tech(front).name.clone(),
        frac: (pool / cost).clamp(0.0, 1.0) as f32,
        pool: format!("{pool:.0}/{cost:.0} pts"),
        rate,
        more_queued: research.queue().len() - 1,
        copying: None,
        bill: bill_status(world, defs, front),
        waiting: waiting_condition(world, defs)
            .map(|c| format!("Waiting: {}", live_text(&c, world))),
    }
}

/// The condition the front tech of the queue is waiting on: its points and its bill are in, and
/// a `requires` condition fell (`Research::waiting_on`, #750). `None` otherwise.
pub fn waiting_condition(world: &World, defs: &Defs) -> Option<Condition> {
    let research_cost = world.knobs(defs).research_cost;
    world.research().waiting_on(&world.facts(), defs, research_cost, world.nation)
}

/// `condition` as the screens word it, with the count it is measured against: "3 Fire residents
/// (2 live here)", "24 people (21 here)", "an Earth or Fire master (1 here)". The sim's own
/// `Display` words a resident count already and leaves the others bare (they are also the queueing
/// refusal, "needs 24 people").
pub fn live_text(condition: &Condition, world: &World) -> String {
    live_text_for(condition, &world.facts())
}

/// `condition` as a card says it, in a column too narrow for the live count that the detail strip
/// has: "3 Earth residents" (the column header counts them), "24 people (21)".
fn card_text(condition: &Condition, facts: &Facts<'_>) -> String {
    match condition {
        Condition::Residents { nation, need, .. } => {
            format!(
                "{need} {} {}",
                nation.title(),
                if *need == 1 { "resident" } else { "residents" }
            )
        }
        Condition::Population(_) => format!("{condition} ({})", facts.population),
        Condition::Masters { count, of } => masters_text(*count, of),
        other => other.to_string(),
    }
}

fn live_text_for(condition: &Condition, facts: &Facts<'_>) -> String {
    match condition {
        Condition::Population(_) => format!("{condition} ({} here)", facts.population),
        Condition::Masters { count, of } => {
            let have: u32 = if of.is_empty() {
                facts.masters.iter().map(|&m| u32::from(m)).sum()
            } else {
                of.iter().map(|n| u32::from(facts.masters[n.index()])).sum()
            };
            format!("{} ({have} here)", masters_text(*count, of))
        }
        other => other.to_string(),
    }
}

/// "a master", "an Earth or Fire master", "2 Fire, Air or Water masters": the sim's wording with
/// the article that suits the nation named first.
fn masters_text(count: u32, of: &[Nation]) -> String {
    let names: Vec<&str> = of.iter().map(|n| n.title()).collect();
    let who = match names.split_last() {
        None => String::new(),
        Some((last, [])) => format!("{last} "),
        Some((last, rest)) => format!("{} or {last} ", rest.join(", ")),
    };
    match count {
        1 if who.starts_with(['A', 'E', 'I', 'O', 'U']) => format!("an {who}master"),
        1 => format!("a {who}master"),
        n => format!("{n} {who}masters"),
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
    /// The front of the queue, with its points and its bill in and a `requires` condition that
    /// fell (docs/design/horizons-phase-6.md, "Conditions"): it finishes as soon as the
    /// condition holds again.
    Waiting,
    Available,
    /// At least one prereq isn't known yet, or a `requires` condition isn't met. Still
    /// selectable while only prereqs are missing: `Research::set_target` queues the whole chain.
    /// A condition it refuses (`ResearchError::NotYet`) until it holds.
    Locked,
    /// A school option whose mate is learned (docs/design/horizons-phase-4.md): it can never be
    /// researched in this game.
    Closed,
    /// A teaching lost to a draft (#743): gone for the rest of the game.
    Lost,
}

impl TechState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Known => "Known",
            Self::Queued => "Queued",
            Self::Waiting => "Waiting",
            Self::Available => "Available",
            Self::Locked => "Needs prereqs",
            Self::Closed => "Closed",
            Self::Lost => "Lost",
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
    let (mut parts, rest) = unlock_parts(defs, nation, id, def);
    parts.extend(rest);
    if !parts.is_empty() {
        return parts.join(", ");
    }
    leads_to(defs, nation, id)
}

/// `unlock_summary`'s two halves: the modifiers ("Heating x1.2"), then everything else the tech
/// switches on (buildings, upgrades, jobs, abilities, features), each as its own list.
pub fn unlock_parts(
    defs: &Defs,
    nation: Nation,
    id: TechId,
    def: &TechDef,
) -> (Vec<String>, Vec<String>) {
    let modifiers: Vec<String> = def
        .unlocks
        .modifiers
        .iter()
        .map(|(n, m)| format!("{} {}", humanize(n), multiplier(m)))
        .collect();
    let mut parts: Vec<String> = Vec::new();
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
        if let Some(n) = u.spirit_seats {
            // "A fox spirit studies in each study hall" (#739).
            let who = if n == 1 {
                "A fox spirit studies".to_owned()
            } else {
                format!("{n} fox spirits study")
            };
            return Some(format!("{who} in each {}", b.display_name(nation).to_lowercase()));
        }
        if let Some(n) = u.vials_per_year {
            let s = if n == 1 { "" } else { "s" };
            return Some(format!(
                "{} fills {n} vial{s} of spirit water each spring",
                b.display_name(nation)
            ));
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
    (modifiers, parts)
}

/// What a tech with nothing to unlock says: the techs it opens ("Leads to Writing"), or "-".
fn leads_to(defs: &Defs, nation: Nation, id: TechId) -> String {
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
        Feature::CelestialDays => "Celestial days",
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
    /// What it is in the TEACHINGS column (#743); `None` for every card of the tiers.
    pub teaching: Option<TeachingRow>,
    /// Set on the From newcomers page: where the tech comes from and what it asks of the
    /// settlement. `None` for every card of the tree.
    pub newcomer: Option<Newcomer>,
}

/// What a card of the From newcomers page adds (docs/design/horizons-phase-6.md, "Mockups"): a
/// borrowed tech of another nation, or a fusion of two.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Newcomer {
    /// The nation a borrowed tech is learned from; `None` for a fusion.
    pub from: Option<Nation>,
    /// The two nations a fusion joins; `None` for a borrowed tech.
    pub fusion: Option<[Nation; 2]>,
    /// A borrowed tech's modifiers, which only residents of its own nation get: "Heating x1.2 for
    /// Fire residents only". Empty for a fusion and for a tech with none.
    pub modifiers: String,
    /// Every condition on the tech with the live count it is measured against, met or not
    /// (`condition_lines`).
    pub conditions: Vec<(String, bool)>,
    /// The prereqs a borrower is spared, by name: the bending of the tech's own nation.
    pub waived: Vec<String>,
}

/// The two nations of a fusion as a card says them: "Earth+Water".
pub fn pair_short(pair: [Nation; 2]) -> String {
    format!("{}+{}", pair[0].title(), pair[1].title())
}

/// A card of the TEACHINGS column (#743, docs/design/horizons-phase-5.md, "Mockups"), after the
/// last tier: the draft's own row, the teachings kept and lost, and how many are left to find.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum TeachingRow {
    /// `! Choose a teaching`: drafts are waiting (`waiting` of them), and `Enter` reopens the
    /// LOST TEACHING box. Its card's `id` is the first teaching on offer, never researched from
    /// here.
    Choose { waiting: usize },
    /// A kept teaching, with the study hall its scroll is shelved at until it is learned.
    Kept { hall: Option<String> },
    /// A teaching lost to a draft: `x`, "Lost to the ruins".
    Lost,
    /// `? 2 more to find` (owner #734: shown only once a teaching scroll has been read, a
    /// count and never names). Its card's `id` is meaningless.
    More(usize),
}

impl TechCard {
    /// Whether the card stands for a tech: every card but the TEACHINGS column's Choose and
    /// More rows.
    pub fn is_tech(&self) -> bool {
        !matches!(self.teaching, Some(TeachingRow::Choose { .. } | TeachingRow::More(_)))
    }
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
    /// Which of `columns` is the TEACHINGS column (#743), always the last; `None` before the
    /// first teaching scroll is read.
    pub teachings: Option<usize>,
    /// The column headers when they are not "TIER n": the From newcomers page's nations and
    /// "FUSIONS". Empty on the tree.
    pub titles: Vec<String>,
}

/// The pages of the Research screen. Once the Harmony Gate stands, `Tab` switches between the
/// settlement's own tree and what the newcomers can teach it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResearchPage {
    #[default]
    Tree,
    Newcomers,
}

impl ResearchPage {
    pub fn other(self) -> Self {
        match self {
            Self::Tree => Self::Newcomers,
            Self::Newcomers => Self::Tree,
        }
    }
}

/// Whether the Research screen has a second page: the gate stands, so newcomers have something
/// to teach (`World::exchange`).
pub fn has_newcomers(session: &Session) -> bool {
    session.world.united()
}

/// The cards of `page`.
pub fn screen_page(session: &Session, page: ResearchPage) -> ScreenView {
    match page {
        ResearchPage::Tree => screen_view(session),
        ResearchPage::Newcomers => newcomers_view(session),
    }
}

pub fn screen_view(session: &Session) -> ScreenView {
    let defs = &session.defs;
    let facts = session.world.facts();
    let depths = tech_depths(&defs.techs);
    let mut columns: Vec<Vec<TechCard>> = Vec::new();
    for (id, def) in session.world.tree_techs(defs) {
        // A kept teaching is drawn in the TEACHINGS column, not at its depth.
        if def.teaching.is_some() {
            continue;
        }
        // A fusion has its own page (`newcomers_view`).
        if def.is_fusion() {
            continue;
        }
        let depth = depths[id.0 as usize];
        if columns.len() <= depth {
            columns.resize(depth + 1, Vec::new());
        }
        columns[depth].push(tech_card(session, &facts, id));
    }
    let teachings = teachings_column(session, &facts);
    let teachings_at = (!teachings.is_empty()).then(|| {
        columns.push(teachings);
        columns.len() - 1
    });
    ScreenView { columns, teachings: teachings_at, titles: Vec::new() }
}

/// The card for `id`, in a tier or the TEACHINGS column.
fn tech_card(session: &Session, facts: &fn_sim::research::Facts<'_>, id: TechId) -> TechCard {
    let defs = &session.defs;
    let nation = session.world.nation;
    let research = session.world.research();
    let target = research.queue().last().copied();
    let def = defs.tech(id);
    let mut state = tech_state(research, facts, defs, id, def);
    if state == TechState::Queued
        && research.queue().first() == Some(&id)
        && waiting_condition(&session.world, defs).is_some()
    {
        state = TechState::Waiting;
    }
    TechCard {
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
        needs: missing_needs(research, facts, defs, def).join(", "),
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
        teaching: None,
        newcomer: None,
    }
}

/// The TEACHINGS column (#743): `! Choose a teaching` while drafts wait, every kept teaching,
/// every lost one, then `? N more to find` once a scroll has been read (owner #734). Empty
/// before the first teaching scroll is read.
fn teachings_column(session: &Session, facts: &fn_sim::research::Facts<'_>) -> Vec<TechCard> {
    let (world, defs) = (&session.world, &session.defs);
    let pseudo = |id: TechId, name: String, row: TeachingRow| TechCard {
        id,
        name,
        cost: 0,
        state: TechState::Available,
        unlock: String::new(),
        needs: String::new(),
        target: false,
        closed: String::new(),
        items: String::new(),
        bill: String::new(),
        school: None,
        teaching: Some(row),
        newcomer: None,
    };
    let mut out = Vec::new();
    let offers = world.teaching_offers();
    if let Some(first) = offers.first().and_then(|o| o.techs.first()) {
        let row = TeachingRow::Choose { waiting: offers.len() };
        out.push(pseudo(*first, "Choose a teaching".to_owned(), row));
    }
    for id in world.kept_teachings() {
        let mut card = tech_card(session, facts, id);
        let hall = world.shelved_at(id).map(|p| crate::teaching::hall_name(session, p));
        card.teaching = Some(TeachingRow::Kept { hall });
        out.push(card);
    }
    for id in world.lost_teachings() {
        let mut card = tech_card(session, facts, id);
        card.state = TechState::Lost;
        card.unlock = String::new();
        card.needs = String::new();
        card.target = false;
        card.bill = String::new();
        card.teaching = Some(TeachingRow::Lost);
        out.push(card);
    }
    if let Some(n) = world.teachings_to_find(defs).filter(|&n| n > 0) {
        out.push(pseudo(TechId(0), format!("{n} more to find"), TeachingRow::More(n)));
    }
    out
}

/// The From newcomers page (docs/design/horizons-phase-6.md, "Mockups"): a column for each other
/// nation, headed by its name and how many of its people live here, holding the techs it can
/// teach, then a FUSIONS column. Built from `World::exchange`, so it is empty until the gate
/// stands. The cards are the tree's, with `TechCard::newcomer` set.
pub fn newcomers_view(session: &Session) -> ScreenView {
    let (world, defs) = (&session.world, &session.defs);
    let exchange = world.exchange(defs);
    let facts = world.facts();
    let mut view = ScreenView::default();
    for nation in Nation::ALL.into_iter().filter(|&n| n != world.nation) {
        let cards: Vec<TechCard> = exchange
            .borrowed_from(nation)
            .map(|entry| newcomer_card(session, &facts, entry))
            .collect();
        if cards.is_empty() {
            continue;
        }
        let here = facts.residents[nation.index()];
        view.titles.push(format!("{}: {here} here", defs.nation(nation).name.to_uppercase()));
        view.columns.push(cards);
    }
    let fusions: Vec<TechCard> =
        exchange.fusions().map(|entry| newcomer_card(session, &facts, entry)).collect();
    if !fusions.is_empty() {
        view.titles.push("FUSIONS".to_owned());
        view.columns.push(fusions);
    }
    view
}

/// One row of the exchange as a card.
fn newcomer_card(session: &Session, facts: &Facts<'_>, entry: &ExchangeEntry) -> TechCard {
    let (world, defs) = (&session.world, &session.defs);
    let (research, home) = (world.research(), world.nation);
    let def = defs.tech(entry.tech);
    let mut state = match entry.standing {
        ExchangeStanding::Known => TechState::Known,
        ExchangeStanding::Queued => TechState::Queued,
        ExchangeStanding::Available => TechState::Available,
        ExchangeStanding::Locked(_) => TechState::Locked,
    };
    if state == TechState::Queued
        && research.queue().first() == Some(&entry.tech)
        && waiting_condition(world, defs).is_some()
    {
        state = TechState::Waiting;
    }
    let needs = match &entry.standing {
        ExchangeStanding::Locked(needs) => needs
            .iter()
            .map(|need| match need {
                ExchangeNeed::Tech(id) => defs.tech(*id).name.clone(),
                ExchangeNeed::Condition(c) => card_text(c, facts),
            })
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    };
    let (modifiers, rest) = unlock_parts(defs, home, entry.tech, def);
    let from = match entry.kind {
        ExchangeKind::Borrowed { from } => Some(from),
        ExchangeKind::Fusion { .. } => None,
    };
    let fusion = match entry.kind {
        ExchangeKind::Fusion { pair } => Some(pair),
        ExchangeKind::Borrowed { .. } => None,
    };
    // A borrowed tech's modifiers reach only residents of its own nation, so the card leaves
    // them to the detail strip, unless they are all it has.
    let unlock = if state == TechState::Locked {
        String::new()
    } else if from.is_some() && !rest.is_empty() {
        rest.join(", ")
    } else {
        unlock_summary(defs, home, entry.tech, def)
    };
    let modifiers = match from {
        Some(nation) if !modifiers.is_empty() => {
            format!("{} for {} residents only", modifiers.join(", "), nation.title())
        }
        _ => String::new(),
    };
    let waived = match from {
        Some(_) => {
            let kept = defs.borrow_prereqs(entry.tech);
            def.prereqs
                .iter()
                .filter(|p| !kept.contains(p))
                .map(|&p| defs.tech(p).name.clone())
                .collect()
        }
        None => Vec::new(),
    };
    TechCard {
        id: entry.tech,
        name: def.name.clone(),
        cost: entry.cost,
        state,
        unlock,
        needs,
        target: research.queue().last() == Some(&entry.tech),
        closed: String::new(),
        items: cost_items_text(defs, def),
        bill: bill_text(research, defs, entry.tech),
        school: None,
        teaching: None,
        newcomer: Some(Newcomer {
            from,
            fusion,
            modifiers,
            conditions: condition_lines(def, home, facts, defs),
            waived,
        }),
    }
}

/// Every condition on `def` for a settlement of `home`, with the count it is measured against and
/// whether it holds: the `requires` keys in `Requires::unmet`'s order, then a borrowed tech's
/// implicit residents of its own nation. "3 Air residents (3 live here)", "an Earth or Fire master
/// (5 here)", "24 people (21 here)".
pub fn condition_lines(
    def: &TechDef,
    home: Nation,
    facts: &Facts<'_>,
    defs: &Defs,
) -> Vec<(String, bool)> {
    let r = &def.requires;
    let mut all = Vec::new();
    all.extend(r.scrolls_read.map(Condition::ScrollsRead));
    all.extend(r.chapter.as_deref().map(|id| {
        let name = defs.endeavor.chapters.iter().find(|c| c.id == id).map_or(id, |c| &c.name);
        Condition::Chapter(name.to_owned())
    }));
    all.extend(r.population.map(Condition::Population));
    all.extend(
        r.masters.map(|count| Condition::Masters {
            count,
            of: r.masters_of.clone().unwrap_or_default(),
        }),
    );
    for (&nation, &need) in &r.residents {
        let have = u32::from(facts.residents[nation.index()]);
        all.push(Condition::Residents { nation, need, have });
    }
    if let Some(nation) = def.borrowed_from(home) {
        let need = defs.research.borrow_residents;
        let asked = r.residents.get(&nation).is_some_and(|&n| n >= need);
        if !asked {
            let have = u32::from(facts.residents[nation.index()]);
            all.push(Condition::Residents { nation, need, have });
        }
    }
    let unmet = def.unmet(home, facts, defs);
    all.into_iter().map(|c| (live_text_for(&c, facts), !unmet.contains(&c))).collect()
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
            fusion: None,
            teachable: false,
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
        gated.requires = Requires {
            scrolls_read: Some(3),
            chapter: Some("known_on_the_road".to_owned()),
            ..Requires::default()
        };
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
        assert_eq!(cards, s.world.tree_techs(&s.defs).count());
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

    #[test]
    fn conditions_read_with_the_count_they_are_measured_against() {
        let facts = Facts { population: 21, masters: [0, 0, 1, 4], ..Facts::default() };
        let pop = Condition::Population(24);
        assert_eq!(live_text_for(&pop, &facts), "24 people (21 here)");
        assert_eq!(card_text(&pop, &facts), "24 people (21)");
        let one = Condition::Masters { count: 1, of: vec![Nation::Earth, Nation::Fire] };
        assert_eq!(live_text_for(&one, &facts), "an Earth or Fire master (5 here)");
        let water = Condition::Masters { count: 1, of: vec![Nation::Water, Nation::Air] };
        assert_eq!(live_text_for(&water, &facts), "a Water or Air master (0 here)");
        let any = Condition::Masters { count: 2, of: Vec::new() };
        assert_eq!(live_text_for(&any, &facts), "2 masters (5 here)");
        let fire = Condition::Residents { nation: Nation::Fire, need: 3, have: 2 };
        assert_eq!(live_text_for(&fire, &facts), "3 Fire residents (2 live here)");
        assert_eq!(card_text(&fire, &facts), "3 Fire residents");
    }

    #[test]
    fn fusions_are_not_on_the_tree_and_the_waiting_row_needs_a_fallen_condition() {
        let mut s = Session::new(7, Nation::Water, 64, 64);
        assert!(s.world.complete_endeavor(&s.defs));
        let tree = screen_view(&s);
        assert!(tree.columns.iter().flatten().all(|c| !s.defs.tech(c.id).is_fusion()));
        let page = newcomers_view(&s);
        assert!(page.columns.iter().flatten().any(|c| s.defs.tech(c.id).is_fusion()));
        assert_eq!(page.titles.last().map(String::as_str), Some("FUSIONS"));
        assert!(page.titles[0].ends_with(" here"), "{:?}", page.titles);
        // Nothing queued: no waiting row.
        assert_eq!(research_status(&s.world, &s.defs).waiting, None);
    }
}

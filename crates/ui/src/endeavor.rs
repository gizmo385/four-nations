//! The end-game endeavor as the panels show it (#235, `docs/design/endgame.md`, "Showing
//! progress"): the Endeavor screen's view (`G`), the side panel's section, and the alert lines
//! for its events. Reads `World::endeavor_signs` and `World::endeavor` and words them; which signs
//! there are, and when the endeavor is Ready, are the sim's.

use fn_sim::bending::Rank;
use fn_sim::endeavor::{ChapterDef, SignDef, SignGroup, SignKind, Tally};
use fn_sim::world::{
    AvatarLeaveReason, EndeavorEvent, EndeavorEventKind, EndeavorState, SignProgress,
};
use fn_sim::{Defs, Pos, World};

use crate::session::Session;
use crate::text::compass_distance;
use crate::tools::Tool;
use crate::view::population_by_nation;

/// What the toast and the log say once the endeavor is Ready.
pub const READY_TEXT: &str = "Word of your settlement has reached the Avatar.";

/// The side panel's line once the endeavor is Ready (#643): short enough to fit beside the count,
/// and the row's own `[G]` already names the screen where the site is chosen.
pub const READY_LINE: &str = "Ready";

/// The Endeavor screen (`G`).
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ScreenView {
    pub name: String,
    pub description: String,
    pub met: usize,
    pub total: usize,
    /// "7 of 10 signs".
    pub count: String,
    /// The share of signs met, in `[0, 1]`: how much of the gate is drawn in colour.
    pub frac: f32,
    /// Under the gate: where the endeavor stands ("Stage: not begun").
    pub stage: String,
    /// Under the stage, what can be done with the gate now (#643): "Enter: choose a site for the
    /// gate" once Ready, "X: withdraw the site" until work on it starts (`crate::gate::action`).
    pub action: Option<String>,
    /// The current chapter (#439), shown at the top: its signs until it's met, then its name
    /// alone. `None` when the data has no chapter.
    pub chapter: Option<ChapterView>,
    /// The signs under their headings, in data order. Empty in the United City, where the signs
    /// are all behind it (`united`).
    pub groups: Vec<GroupView>,
    /// The United City (#442, docs/design/endgame.md "The transformation"): once the gate stands,
    /// the screen shows this in place of the chapter and the signs.
    pub united: Option<UnitedView>,
}

/// The Endeavor screen's United City view (#442): who lives there and the next population
/// milestone.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct UnitedView {
    /// The living settlers (the Avatar and beasts never count).
    pub population: usize,
    /// Each nation with at least one settler, as (name, count) (`view::population_by_nation`).
    pub by_nation: Vec<(String, usize)>,
    /// The next milestone's population, or `None` when the data has no more.
    pub next: Option<u32>,
    /// How far `population` is toward `next`, in `[0, 1]`: from the milestone before it (or
    /// from nothing before the first).
    pub frac: f32,
}

/// The Endeavor screen's CHAPTER section (#439, docs/design/pacing.md section 3).
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ChapterView {
    /// "Known on the road".
    pub name: String,
    pub met: bool,
    /// "2 of 3"; empty once met.
    pub count: String,
    /// Its signs while it isn't met, in data order; empty once met.
    pub signs: Vec<SignRow>,
    /// What meeting it brings ("Traders come once the road knows you"), while it isn't met.
    pub hint: Option<String>,
}

/// One heading of the screen and its signs.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct GroupView {
    pub heading: String,
    pub signs: Vec<SignRow>,
}

/// One sign's row: a check if met, the name, a hint if not, `value/target` and a bar.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SignRow {
    pub id: String,
    pub name: String,
    pub met: bool,
    /// What would help, shown only while the sign isn't met.
    pub hint: Option<String>,
    /// "14/16".
    pub value: String,
    /// In `[0, 1]`.
    pub frac: f32,
    /// The masters sign only, shown only while it isn't met: who has already mastered and the
    /// closest non-masters ("Master: Lu Ten. Closest: Jee 9 of 10 bends, 2 of 3 forms",
    /// `docs/design/endgame.md`, "Showing mastery", #347). `None` for every other sign.
    pub closest: Option<String>,
    /// The scrolls sign only, shown only while it isn't met: the scroll sources already marked
    /// or charted, nearest first (`found_scrolls`; #380, #403). Empty for every other sign, and
    /// for this one until a surface scroll is marked or a ruin is uncovered.
    pub found: Vec<FoundScroll>,
}

/// One marked surface scroll or uncovered ruin, for the scrolls sign's own row
/// (`found_scrolls`): where it is and why it isn't read yet.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FoundScroll {
    /// "Ancient Scroll", or "Lost Ruins (2)" when one harvest yields more than one.
    pub name: String,
    /// "34 tiles north-west" (`crate::text::compass_distance`), from the settlement's anchor.
    pub distance: String,
    /// "marked", "not marked" (an uncovered ruin only: a surface scroll is listed only while
    /// marked) or "no settler can reach it" (`World::job_unreachable`, #337's flag, reused
    /// rather than a new one).
    pub status: String,
}

/// The side panel's Endeavor section.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct EndeavorStatus {
    pub name: String,
    /// "7/10".
    pub count: String,
    pub frac: f32,
    /// The line under the bar: what the screen is for, or that the endeavor is Ready.
    pub line: String,
    /// Up to three things still missing, in data order ("2 more people", "80 more stone").
    pub next: Vec<String>,
}

/// How many missing things the side panel names.
pub const NEXT_SHOWN: usize = 3;

/// The heading a group's signs are listed under. Stores and harvests share one: both are about
/// what the settlement has gathered.
pub fn heading(group: SignGroup) -> &'static str {
    match group {
        SignGroup::People => "People",
        SignGroup::Buildings => "Buildings",
        SignGroup::Knowledge => "Knowledge",
        SignGroup::Stores | SignGroup::Harvests => "Stores and harvests",
        SignGroup::Bending => "Bending",
    }
}

fn frac(p: &SignProgress) -> f32 {
    if p.target == 0 { 0.0 } else { (p.value as f32 / p.target as f32).clamp(0.0, 1.0) }
}

/// Every sign with its definition, in data order.
fn signs<'d>(world: &World, defs: &'d Defs) -> Vec<(&'d SignDef, SignProgress<'d>)> {
    defs.endeavor.signs.iter().zip(world.endeavor_signs(defs)).collect()
}

/// The chapter the screen and the side panel show (#439): the first one not met yet, else the
/// last one met, with its signs' progress (empty once met). `None` without chapters in the data.
fn chapter<'d>(
    world: &World,
    defs: &'d Defs,
) -> Option<(&'d ChapterDef, Vec<(&'d SignDef, SignProgress<'d>)>)> {
    match world.current_chapter(defs) {
        Some(c) => Some((c, c.signs.iter().zip(world.chapter_signs(c, defs)).collect())),
        None => defs.endeavor.chapters.last().map(|c| (c, Vec::new())),
    }
}

/// Names of the settlement's own techs it doesn't know yet, in data order.
fn missing_techs<'d>(world: &World, defs: &'d Defs) -> Vec<&'d str> {
    world
        .tree_techs(defs)
        .filter(|(id, t)| t.nation == Some(world.nation) && !world.research().is_known(*id))
        .map(|(_, t)| t.name.as_str())
        .collect()
}

/// Names of the reachable building kinds this nation hasn't raised yet, in data order
/// (`World::missing_building_kinds`, #354).
fn missing_building_names<'d>(world: &World, defs: &'d Defs) -> Vec<&'d str> {
    world
        .missing_building_kinds(defs)
        .into_iter()
        .map(|id| defs.building(id).display_name(world.nation))
        .collect()
}

/// How many of the closest non-masters the masters sign's own line names.
const MASTERS_SHOWN: usize = 3;

/// Whether the data has a masters sign at all: `EndeavorDef::master_threshold`'s `Option`-ness
/// doesn't depend on which nation it's asked for, only on whether `data/endeavor.toml` has one,
/// so one check at the settlement's own nation answers it for every unit.
fn has_masters_sign(world: &World, defs: &Defs) -> bool {
    defs.endeavor.master_threshold(world.nation).is_some()
}

/// Non-master units with their own bends and forms against their own nation's Master
/// thresholds (`fn_sim::bending::MasteryDef::master_bends_for`/`master_forms`, `World::rank_of`),
/// closest to mastery first by bends (a tie keeps each unit's order in `World::units`). Guests
/// (the Avatar) never count. Empty without a masters sign in the data, or once every counted
/// unit has already mastered.
fn closest_non_masters(world: &World, defs: &Defs) -> Vec<(String, u32, u32, u32, u32)> {
    if !has_masters_sign(world, defs) {
        return Vec::new();
    }
    let mut rest: Vec<(String, u32, u32, u32, u32)> = world
        .units
        .iter()
        .filter(|u| u.is_settler())
        .filter_map(|u| {
            let progress = world.rank_of(u, defs);
            (progress.rank != Rank::Master).then(|| {
                let nation = world.nation_of(u);
                (
                    u.name.clone(),
                    progress.bends,
                    defs.mastery.master_bends_for(nation),
                    progress.forms,
                    defs.mastery.master_forms,
                )
            })
        })
        .collect();
    rest.sort_by_key(|&(_, bends, ..)| std::cmp::Reverse(bends));
    rest
}

/// Names of units that have already reached `Rank::Master` (`World::rank_of`), alphabetical.
/// Empty without a masters sign in the data.
fn current_masters(world: &World, defs: &Defs) -> Vec<String> {
    if !has_masters_sign(world, defs) {
        return Vec::new();
    }
    let mut masters: Vec<String> = world
        .units
        .iter()
        .filter(|u| u.is_settler())
        .filter(|u| world.rank_of(u, defs).rank == Rank::Master)
        .map(|u| u.name.clone())
        .collect();
    masters.sort();
    masters
}

/// The masters sign's own line: who has already mastered and the closest non-masters, forms
/// included (#361, docs/design/bending.md "Seeing it": "its 'Closest' line shows forms too").
/// `None` with nobody to name (no masters sign, or no units at all).
fn masters_line(world: &World, defs: &Defs) -> Option<String> {
    masters_text(&current_masters(world, defs), &closest_non_masters(world, defs))
}

/// Pure text assembly for `masters_line`, split out so it can be tested against literal data
/// without simulating real bends: `masters` (already alphabetical) and `closest` (already
/// closest-first, as `closest_non_masters` returns them: name, bends, bend target, forms, form
/// target).
fn masters_text(masters: &[String], closest: &[(String, u32, u32, u32, u32)]) -> Option<String> {
    let master_part = (!masters.is_empty()).then(|| {
        let label = if masters.len() == 1 { "Master" } else { "Masters" };
        format!("{label}: {}.", masters.join(", "))
    });
    let closest_part = (!closest.is_empty()).then(|| {
        // In words ("9 of 10 bends, 2 of 3 forms"), names split by a semicolon since each
        // entry has a comma of its own (#663). The screen wraps the line.
        let names: Vec<String> = closest
            .iter()
            .take(MASTERS_SHOWN)
            .map(|(name, bends, bend_t, forms, form_t)| {
                format!("{name} {}", crate::text::progress_words(*bends, *bend_t, *forms, *form_t))
            })
            .collect();
        format!("Closest: {}", names.join("; "))
    });
    match (master_part, closest_part) {
        (Some(m), Some(c)) => Some(format!("{m} {c}")),
        (Some(m), None) => Some(m),
        (None, Some(c)) => Some(c),
        (None, None) => None,
    }
}

/// How many sources the scrolls sign's row names: enough for every seed the supply survey
/// measured (`docs/design/cartography.md`, up to 6 on a small map), without letting a very lucky
/// one turn the row into a wall of text.
const FOUND_SHOWN: usize = 6;

/// Whether the scrolls sign's full list is open (owner decision, 2026-11-09, second part): the
/// sign's `hint_until` tech (`cartography`) is known and a Map Room has finished a chart
/// (`World::has_charted`, saved state that outlives the room and the ruins). Until then the list
/// holds only what a Glide uncovered (`found_scrolls`).
fn scrolls_unlocked(sign: &SignDef, world: &World) -> bool {
    sign.hint_until.as_ref().is_none_or(|u| world.research().is_known(u.tech))
        && world.has_charted()
}

/// The scroll sources the player has already acted on or charted, nearest the settlement's anchor
/// first (`World::settlement_anchor`). Owner decisions, 2026-11-09 (`docs/design/cartography.md`
/// "What the player sees"): finding scrolls stays the player's job, and the list opens only once
/// the settlement has charted (`scrolls_unlocked`).
///
/// Once open, two kinds and nothing else:
/// - a buried resource that something has uncovered (a lost ruin, by a Map Room's chart, #402,
///   or Air's Glide, #197), marked or not;
/// - an ordinary surface scroll, but only while it is marked.
///
/// Before it opens, only a ruin a Glide uncovered: one on the map that no finished chart
/// recorded (`World::charted_at`), since the player already found it. A marked surface scroll is
/// not listed yet.
///
/// An unmarked surface scroll is never listed, however near, and neither is a ruin still under
/// the ground. Both yield the same `ancient_scroll` item (`data/jobs.toml`), so one item filter
/// catches them without naming either resource by id, the same trick
/// `fn_sim::autoplay::recover_scrolls` plays; the resource's `buried` flag tells them apart.
///
/// The list is worked out from the map and the open jobs on every call and remembers nothing, so
/// it follows them: a scroll leaves it once it is recovered (its map tile is gone, and an item
/// being carried or stored is not a source), and a surface scroll leaves it if its mark is
/// removed. An uncovered ruin stays listed, as "not marked", after its mark is removed.
///
/// For each: its name (with a count when a harvest yields more than one), how far and which way
/// from the settlement, and whether it's marked, not marked (an uncovered ruin only), or not
/// reachable by any living settler (`World::job_unreachable`, #337's flag). Empty without the
/// `ancient_scroll` item in the data at all.
fn found_scrolls(world: &World, defs: &Defs, sign: &SignDef) -> Vec<FoundScroll> {
    let Some(scroll_item) = defs.item_id("ancient_scroll") else { return Vec::new() };
    let unlocked = scrolls_unlocked(sign, world);
    let anchor = world.settlement_anchor();
    let marked: Vec<Pos> = world.harvest_designations().collect();
    let mut found: Vec<Pos> = world
        .map
        .iter()
        .filter(|(p, t)| {
            let Some(r) = t.resource else { return false };
            if !defs.harvest(r).is_some_and(|h| h.item == scroll_item) {
                return false;
            }
            if defs.resource(r).buried {
                unlocked || !world.charted_at(*p)
            } else {
                unlocked && marked.contains(p)
            }
        })
        .map(|(p, _)| p)
        .collect();
    found.sort_by_key(|&p| anchor.chebyshev(p));
    found
        .into_iter()
        .take(FOUND_SHOWN)
        .map(|pos| {
            // `found`'s own filter already guarantees a resource and a harvest here.
            let resource = world.map.get(pos).and_then(|t| t.resource).expect("filtered above");
            let h = defs.harvest(resource).expect("filtered above");
            let name = if !h.also.is_empty() {
                format!("{} ({})", defs.resource(resource).name, scrolls_and_also(defs, h))
            } else if h.count > 1 {
                format!("{} ({})", defs.resource(resource).name, h.count)
            } else {
                defs.resource(resource).name.clone()
            };
            let status = if world.job_unreachable(pos) == Some(true) {
                "no settler can reach it"
            } else if marked.contains(&pos) {
                "marked"
            } else {
                "not marked"
            };
            FoundScroll { name, distance: compass_distance(anchor, pos), status: status.to_owned() }
        })
        .collect()
}

/// A great ruin's count on the scrolls sign's list (#736): "2 scrolls and a teaching". Each
/// `also` stack is named by its item without a trailing "Scroll" ("Teaching Scroll" reads
/// "teaching"), with "a" for one.
fn scrolls_and_also(defs: &Defs, h: &fn_sim::jobs::HarvestDef) -> String {
    let mut out = match h.count {
        1 => "1 scroll".to_owned(),
        n => format!("{n} scrolls"),
    };
    for extra in &h.also {
        let name = defs.item(extra.item).name.to_lowercase();
        let name = name.strip_suffix(" scroll").unwrap_or(&name).to_owned();
        match extra.count {
            1 => out.push_str(&format!(" and a {name}")),
            n => out.push_str(&format!(" and {n} {name}s")),
        }
    }
    out
}

/// The hint for an unmet sign: the branch names the techs left once only one or two are, the
/// buildings sign names the missing kinds the same way ("Not yet raised: Earth Wall, Rail"), the
/// masters sign says how to start a bend (#382: the data's own hint, "Each finished bend counts
/// for whoever channels it", explained credit but never how to begin one, which playtest agents
/// read as bending being a rare, passive event), and every other sign uses its data hint.
fn hint(sign: &SignDef, world: &World, defs: &Defs) -> Option<String> {
    match &sign.kind {
        SignKind::BranchComplete => branch_hint(&missing_techs(world, defs), sign.hint.as_deref()),
        SignKind::EveryBuilding { .. } => {
            building_hint(&missing_building_names(world, defs), sign.hint.as_deref())
        }
        SignKind::Masters => Some(masters_hint()),
        _ => match &sign.hint_until {
            // The scrolls sign points at cartography until it is researched, then at the Map
            // Room until the first chart is done (#403).
            Some(until) if !world.research().is_known(until.tech) => Some(until.text.clone()),
            Some(_) if !world.has_charted() && sign.hint_uncharted.is_some() => {
                sign.hint_uncharted.clone()
            }
            _ => sign.hint.clone(),
        },
    }
}

/// How to start a bend, naming the Bend tool's real key (`Tool::Bend::key`, the same source the
/// key help reads, so this can't drift from it, and never names an F-key, which WSL terminals
/// never deliver): "Designate a bend (B) on a tile a bender can work - credit to whoever
/// channels it" (#382).
fn masters_hint() -> String {
    format!(
        "Designate a bend ({}) on a tile a bender can work - credit to whoever channels it",
        Tool::Bend.key()
    )
}

/// The branch sign's hint, given the techs `left`: their names when there are one or two, else
/// the data's `hint`.
fn branch_hint(left: &[&str], hint: Option<&str>) -> Option<String> {
    if (1..=2).contains(&left.len()) {
        Some(format!("{} left", left.join(", ")))
    } else {
        hint.map(str::to_owned)
    }
}

/// The buildings sign's hint, given the kinds `left` unbuilt: their names when there are one or
/// two, else the data's `hint` ("Raise something new").
fn building_hint(left: &[&str], hint: Option<&str>) -> Option<String> {
    if (1..=2).contains(&left.len()) {
        Some(format!("Not yet raised: {}", left.join(", ")))
    } else {
        hint.map(str::to_owned)
    }
}

/// What a sign still needs, as a count and the data's own short noun ("2 more beds", "150 more
/// food", `SignDef::short`, #416, docs/design/side-panel-v2.md "Zone 1"): the side panel's
/// compact Gate rows and its `next` list both use this instead of a sentence. The fuller wording
/// (which techs or masters are closest) stays on the Endeavor screen's own hint (`hint`,
/// `masters_line`).
pub fn missing(sign: &SignDef, p: &SignProgress) -> String {
    format!("{} more {}", p.target.saturating_sub(p.value), sign.short)
}

/// The side panel's second Gate row (#663): what the gate still needs ("Needs 4 more people",
/// from `next`, `missing`'s phrases), or, once nothing is missing, the endeavor's own `line`
/// (signs met, Ready, or the stage).
pub fn gate_detail(next: &[String], line: &str) -> String {
    match next.first() {
        Some(first) => format!("Needs {first}"),
        None => line.to_owned(),
    }
}

/// A sign's label: its data name, except that the branch sign names the nation's branch, and the
/// masters sign composes its own threshold and form count ("Bending masters (10 bends in 3
/// forms)", docs/design/bending.md "Seeing it", #361) so the label can't drift from the
/// settlement's own numbers (`EndeavorDef::master_threshold`, #344; `MasteryDef::master_forms`,
/// #360).
fn sign_name(sign: &SignDef, world: &World, defs: &Defs) -> String {
    match &sign.kind {
        SignKind::BranchComplete => format!("{} branch", defs.nation(world.nation).name),
        SignKind::Masters => {
            let threshold = defs
                .endeavor
                .master_threshold(world.nation)
                .expect("this sign is the masters sign");
            let forms = defs.mastery.master_forms;
            format!("{} ({threshold} bends in {forms} forms)", sign.name)
        }
        _ => sign.name.clone(),
    }
}

/// Where the endeavor stands, for under the gate.
fn stage(world: &World, defs: &Defs) -> String {
    match world.endeavor() {
        EndeavorState::Dormant => "Stage: not begun".to_owned(),
        EndeavorState::Ready { .. } => "Ready: the Avatar has heard".to_owned(),
        EndeavorState::Building(_) => match world.gate_stage(defs) {
            Some((n, s)) => {
                format!("Stage {} of {}: {}", n + 1, defs.endeavor.stages.len(), s.name)
            }
            None => "Building".to_owned(),
        },
        EndeavorState::Complete { .. } => format!("{} stands", defs.endeavor.name),
    }
}

/// One sign's row on the screen, gate or chapter alike.
fn sign_row(sign: &SignDef, p: &SignProgress, world: &World, defs: &Defs) -> SignRow {
    let closest = (!p.met && matches!(sign.kind, SignKind::Masters))
        .then(|| masters_line(world, defs))
        .flatten();
    let found = if !p.met && matches!(sign.kind, SignKind::Tally(Tally::ScrollsRead)) {
        found_scrolls(world, defs, sign)
    } else {
        Vec::new()
    };
    SignRow {
        id: sign.id.clone(),
        name: sign_name(sign, world, defs),
        met: p.met,
        hint: if p.met { None } else { hint(sign, world, defs) },
        value: format!("{}/{}", p.value, p.target),
        frac: frac(p),
        closest,
        found,
    }
}

/// The CHAPTER section (#439).
fn chapter_view(world: &World, defs: &Defs) -> Option<ChapterView> {
    let (c, signs) = chapter(world, defs)?;
    let met = world.chapter_met(&c.id).is_some();
    let done = signs.iter().filter(|(_, p)| p.met).count();
    Some(ChapterView {
        name: c.name.clone(),
        met,
        count: if met { String::new() } else { format!("{done} of {}", signs.len()) },
        signs: signs.iter().map(|(s, p)| sign_row(s, p, world, defs)).collect(),
        hint: (!met).then(|| c.hint.clone()),
    })
}

/// The United City's view (#442), once the gate stands.
fn united_view(world: &World, defs: &Defs) -> Option<UnitedView> {
    if !world.united() {
        return None;
    }
    let population = world.settlers();
    let next = world.next_milestone(defs);
    // The bar runs from the milestone before `next` (the last one that fired, or none).
    let from = next.map_or(0, |n| defs.endeavor.milestones.previous(n));
    let frac = next.map_or(1.0, |n| {
        let span = n.saturating_sub(from).max(1) as f32;
        (population.saturating_sub(from as usize) as f32 / span).clamp(0.0, 1.0)
    });
    Some(UnitedView { population, by_nation: population_by_nation(world, defs), next, frac })
}

pub fn screen_view(session: &Session) -> ScreenView {
    let (world, defs) = (&session.world, &session.defs);
    if let Some(united) = united_view(world, defs) {
        return ScreenView {
            name: "The United City".to_owned(),
            description: String::new(),
            met: 0,
            total: 0,
            count: format!("{} people", united.population),
            frac: united.frac,
            stage: stage(world, defs),
            action: None,
            chapter: None,
            groups: Vec::new(),
            united: Some(united),
        };
    }
    let signs = signs(world, defs);
    let met = signs.iter().filter(|(_, p)| p.met).count();
    let total = signs.len();
    let mut groups: Vec<GroupView> = Vec::new();
    for (sign, p) in &signs {
        let row = sign_row(sign, p, world, defs);
        let heading = heading(sign.group);
        match groups.last_mut() {
            Some(g) if g.heading == heading => g.signs.push(row),
            _ => groups.push(GroupView { heading: heading.to_owned(), signs: vec![row] }),
        }
    }
    ScreenView {
        name: defs.endeavor.name.clone(),
        description: defs.endeavor.description.clone(),
        met,
        total,
        count: format!("{met} of {total} signs"),
        frac: share(met, total),
        stage: stage(world, defs),
        action: crate::gate::action(session).map(|a| a.hint().to_owned()),
        chapter: chapter_view(world, defs),
        groups,
        united: None,
    }
}

fn share(met: usize, total: usize) -> f32 {
    if total == 0 { 0.0 } else { met as f32 / total as f32 }
}

/// The side panel's section, or `None` when the data has no endeavor (the section hides).
pub fn endeavor_status(session: &Session) -> Option<EndeavorStatus> {
    let (world, defs) = (&session.world, &session.defs);
    let signs = signs(world, defs);
    if signs.is_empty() {
        return None;
    }
    let met = signs.iter().filter(|(_, p)| p.met).count();
    // Every sign has been met once the endeavor is past Dormant, so nothing is missing.
    let ready = *world.endeavor() != EndeavorState::Dormant;
    // A missing chapter sign comes before the gate's own (#439): it's the nearer goal.
    let chapter_signs = chapter(world, defs).map(|(_, s)| s).unwrap_or_default();
    let next = if ready {
        Vec::new()
    } else {
        chapter_signs
            .iter()
            .chain(&signs)
            .filter(|(_, p)| !p.met)
            .take(NEXT_SHOWN)
            .map(|(s, p)| missing(s, p))
            .collect()
    };
    Some(EndeavorStatus {
        name: defs.endeavor.name.clone(),
        count: format!("{met} of {} signs", signs.len()),
        frac: share(met, signs.len()),
        line: match world.endeavor() {
            EndeavorState::Dormant => "Signs met - G: open".to_owned(),
            EndeavorState::Ready { .. } => READY_LINE.to_owned(),
            EndeavorState::Building(_) | EndeavorState::Complete { .. } => stage(world, defs),
        },
        next,
    })
}

/// The alert line for an endeavor event: "Sign met: Population", the Ready message, or "Stage
/// done: Foundation".
pub fn describe_event(defs: &Defs, ev: &EndeavorEvent) -> String {
    match &ev.kind {
        EndeavorEventKind::SignMet(id) => {
            let name = defs.endeavor.signs.iter().find(|s| s.id == *id).map_or(id, |s| &s.name);
            format!("Sign met: {name}")
        }
        EndeavorEventKind::ChapterMet(id) => {
            let name = defs.endeavor.chapters.iter().find(|c| c.id == *id).map_or(id, |c| &c.name);
            format!("Chapter met: {name}")
        }
        EndeavorEventKind::Ready => READY_TEXT.to_owned(),
        EndeavorEventKind::StageDone(id) => {
            let name = defs.endeavor.stage(id).map_or(id.as_str(), |(_, s)| &s.name);
            format!("Stage done: {name}")
        }
        EndeavorEventKind::TaskDone(id) => {
            let task = defs.endeavor.stages.iter().flat_map(|s| &s.tasks).find(|t| t.id == *id);
            format!("Task done: {}", task.map_or(id.as_str(), |t| &t.name))
        }
        // The Avatar as a guest (#237); his own screen and side-panel glyphs are #242's.
        EndeavorEventKind::AvatarArrived => {
            let name = defs.endeavor.avatar.as_ref().map_or("The Avatar", |a| a.name.as_str());
            format!("{name} has arrived on Appa.")
        }
        EndeavorEventKind::AvatarLeft(reason) => {
            let name = defs.endeavor.avatar.as_ref().map_or("The Avatar", |a| a.name.as_str());
            let why = match reason {
                AvatarLeaveReason::Hungry => "to find food",
                AvatarLeaveReason::Tired => "to rest",
            };
            format!("{name} has gone {why}. He'll be back.")
        }
        EndeavorEventKind::AvatarReturned => {
            let name = defs.endeavor.avatar.as_ref().map_or("The Avatar", |a| a.name.as_str());
            format!("{name} is back.")
        }
        // The Harmony Rite, completion and the Avatar's farewell (#238); the rite stage and the
        // Avatar's own glyphs on screen are #242's.
        EndeavorEventKind::RitePaused => "The rite pauses: not enough of the settlement is \
            attending."
            .to_owned(),
        EndeavorEventKind::RiteBroken => "The rite is broken by grief.".to_owned(),
        EndeavorEventKind::Complete => format!("{} is complete.", defs.endeavor.name),
        // A United City population milestone (#442): the data's own words.
        EndeavorEventKind::PopulationMilestone(n) => defs
            .endeavor
            .milestones
            .toast(*n)
            .unwrap_or_else(|| format!("The United City passes {n} souls.")),
        EndeavorEventKind::AvatarFarewell => {
            let name = defs.endeavor.avatar.as_ref().map_or("The Avatar", |a| a.name.as_str());
            format!("{name} gives his farewell and flies off on Appa.")
        }
        // A settler's rank-up toast (#347, #360, #361): fires once per rank,
        // `World::note_rank_up`, naming this nation's own title for the rank ("Jee is now a
        // Firebender.", "Lu Ten has become a Firebending Master.", docs/design/bending.md
        // "Seeing it") rather than a generic "adept bender"/"bending master".
        EndeavorEventKind::RankUp { name, rank, title, .. } => {
            let article = crate::text::article(title);
            match rank {
                Rank::Adept => format!("{name} is now {article} {title}."),
                Rank::Master => format!("{name} has become {article} {title}."),
                Rank::Novice => unreachable!("nobody ranks up to Novice"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;

    fn session(nation: Nation) -> Session {
        Session::new(7, nation, 64, 64)
    }

    /// The bundled data with "Known on the road" made easy (#439): one person, any kind of
    /// building, and a second population sign in place of food, so a fresh game can meet it.
    fn easy_road(s: &mut Session) {
        let src = include_str!("../../../data/endeavor.toml")
            .replacen("target = 10\n", "target = 1\n", 1)
            .replacen(
                "kind = \"every_building\"\ntarget = 4",
                "kind = \"every_building\"\ntarget = 1",
                1,
            )
            .replacen(
                "kind = \"stored\"\nfood = true\ntarget = 60",
                "kind = \"population\"\ntarget = 1",
                1,
            );
        s.defs = s.defs.clone().with_endeavor(&src).unwrap();
    }

    #[test]
    fn the_chapter_shows_its_signs_until_met_then_only_its_name() {
        let mut s = session(Nation::Earth);
        let v = screen_view(&s).chapter.expect("the bundled data has a chapter");
        assert_eq!(v.name, "Known on the road");
        assert!(!v.met);
        assert_eq!(v.count, "0 of 3");
        let ids: Vec<&str> = v.signs.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["chapter_population", "chapter_kinds", "chapter_food"]);
        assert_eq!(v.hint.as_deref(), Some("Traders come once the road knows you"));
        // The gate's own count never includes the chapter's signs.
        assert_eq!(screen_view(&s).total, s.defs.endeavor.signs.len());

        easy_road(&mut s);
        let bed = s.defs.building_id("bed").unwrap();
        let at = s.world.settlement_anchor();
        s.world.put_building(fn_sim::geom::Footprint::single(at), bed, &s.defs);
        for _ in 0..10 {
            s.tick();
        }
        assert!(s.world.chapter_met("known_on_the_road").is_some());
        let v = screen_view(&s).chapter.unwrap();
        assert!(v.met);
        assert!(v.signs.is_empty() && v.hint.is_none() && v.count.is_empty());
        let ev = s
            .world
            .endeavor_events()
            .find(|e| matches!(e.kind, EndeavorEventKind::ChapterMet(_)))
            .unwrap()
            .clone();
        assert_eq!(describe_event(&s.defs, &ev), "Chapter met: Known on the road");
    }

    #[test]
    fn the_side_panel_names_a_missing_chapter_sign_first() {
        let s = session(Nation::Earth);
        let status = endeavor_status(&s).unwrap();
        // A new game: 4 more people for the chapter (6 of 10), before the gate's 10 (6 of 16).
        assert_eq!(status.next[0], format!("{} more people", 10 - s.world.units.len()));
        assert_eq!(status.next.len(), NEXT_SHOWN);
    }

    #[test]
    fn the_screen_lists_every_sign_under_its_heading_in_data_order() {
        let s = session(Nation::Earth);
        let v = screen_view(&s);
        assert_eq!(v.name, "The Harmony Gate");
        assert_eq!(v.total, s.defs.endeavor.signs.len());
        let ids: Vec<&str> =
            v.groups.iter().flat_map(|g| &g.signs).map(|r| r.id.as_str()).collect();
        let data: Vec<&str> = s.defs.endeavor.signs.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, data);
        let headings: Vec<&str> = v.groups.iter().map(|g| g.heading.as_str()).collect();
        assert_eq!(
            headings,
            ["People", "Buildings", "Knowledge", "Stores and harvests", "Bending"]
        );
        assert_eq!(v.count, format!("{} of {} signs", v.met, v.total));
        assert_eq!(v.stage, "Stage: not begun");
        let branch = v.groups[2].signs[0].name.clone();
        assert_eq!(branch, "Earth Kingdom branch");
    }

    #[test]
    fn a_new_game_shows_its_population_and_hints_on_unmet_signs() {
        let s = session(Nation::Earth);
        let v = screen_view(&s);
        let pop = &v.groups[0].signs[0];
        assert_eq!(pop.value, format!("{}/16", s.world.units.len()));
        assert!(!pop.met);
        assert_eq!(pop.hint.as_deref(), Some("Spare beds and food draw newcomers"));
        assert!((pop.frac - s.world.units.len() as f32 / 16.0).abs() < 1e-6);
    }

    #[test]
    fn the_branch_names_the_last_techs() {
        let data = Some("Research every tech");
        assert_eq!(branch_hint(&["Metalbending"], data), Some("Metalbending left".into()));
        assert_eq!(branch_hint(&["A", "B"], data), Some("A, B left".into()));
        assert_eq!(branch_hint(&["A", "B", "C"], data), Some("Research every tech".into()));
        assert_eq!(branch_hint(&[], None), None);
        // A new Earth game knows none of its branch yet.
        let s = session(Nation::Earth);
        let left = missing_techs(&s.world, &s.defs);
        // Hidden teachings (#738 on) aren't on the tree, so they aren't left either.
        let branch = s
            .defs
            .techs
            .iter()
            .filter(|t| t.nation == Some(Nation::Earth) && t.teaching.is_none())
            .count();
        assert_eq!(left.len(), branch);
    }

    #[test]
    fn the_buildings_sign_names_the_last_missing_kinds() {
        let data = Some("Raise something new");
        assert_eq!(
            building_hint(&["Earth Wall", "Rail"], data),
            Some("Not yet raised: Earth Wall, Rail".into())
        );
        assert_eq!(building_hint(&["Earth Wall"], data), Some("Not yet raised: Earth Wall".into()));
        assert_eq!(building_hint(&["A", "B", "C"], data), Some("Raise something new".into()));
        assert_eq!(building_hint(&[], None), None);
        // A new Earth game hasn't raised any of its 12 reachable kinds yet (4 shared, the Map
        // Room, #402, and the Herb Garden, #390, earth_wall, rail, the shared Workshop, #369,
        // the Mine Shaft and Deep Mine, #373, and the Hot Spring Mill, #448).
        let s = session(Nation::Earth);
        let left = missing_building_names(&s.world, &s.defs);
        assert_eq!(left.len(), 12);
    }

    #[test]
    fn a_new_games_buildings_sign_is_renamed_and_falls_back_to_the_data_hint() {
        let s = session(Nation::Earth);
        let v = screen_view(&s);
        let row = &v.groups[1].signs[0];
        assert_eq!(row.name, "One of every building");
        // Too many kinds are missing to name them all, so the data's own hint shows instead.
        assert_eq!(row.hint.as_deref(), Some("Raise something new"));
    }

    /// #663: the Gate's second row says what is needed in words, and falls back to the
    /// endeavor's own line once nothing is missing.
    #[test]
    fn the_gate_detail_says_what_is_needed() {
        let s = session(Nation::Fire);
        let status = endeavor_status(&s).unwrap();
        // The chapter's population (10) comes first while it isn't met (#439).
        let people = 10 - s.world.units.len();
        assert_eq!(gate_detail(&status.next, &status.line), format!("Needs {people} more people"));
        assert_eq!(gate_detail(&[], READY_LINE), "Ready");
        assert_eq!(status.count, format!("0 of {} signs", s.defs.endeavor.signs.len()));
    }

    #[test]
    fn the_side_panel_names_the_next_three_missing_things() {
        let s = session(Nation::Fire);
        let status = endeavor_status(&s).unwrap();
        assert_eq!(status.name, "The Harmony Gate");
        assert_eq!(status.next.len(), NEXT_SHOWN);
        // The chapter's signs first (#439): 10 people, 4 kinds, 60 food.
        let people = 10 - s.world.units.len();
        assert_eq!(status.next[0], format!("{people} more people"));
        assert_eq!(status.next[1], "4 more kinds");
        assert_eq!(status.next[2], "60 more food");
        assert_eq!(status.line, "Signs met - G: open");
    }

    /// `missing` (#416): a count and the data's own short noun, the same for every kind -- the
    /// fuller wording (which techs or masters are closest) is the Endeavor screen's own hint
    /// (`hint`, `masters_line`), not this compact phrase.
    #[test]
    fn missing_is_a_count_and_the_signs_own_short_noun() {
        let s = session(Nation::Earth);
        let (w, d) = (&s.world, &s.defs);
        let text = |id: &str| {
            let (sign, p) = signs(w, d).into_iter().find(|(s, _)| s.id == id).unwrap();
            missing(sign, &p)
        };
        assert_eq!(text("food"), "150 more food");
        assert_eq!(text("stone"), "200 more stone");
        assert_eq!(text("ore"), "40 more ore");
        assert_eq!(text("masters"), "3 more masters");
        assert_eq!(text("scrolls"), "3 more scrolls");
    }

    /// `closest_non_masters` and `current_masters` on a fresh game: nobody has bent yet, so
    /// everyone is a non-master at 0/10 bends, 0/3 forms (Fire's own thresholds), and nobody is
    /// a master.
    #[test]
    fn mastery_helpers_read_a_fresh_games_units() {
        let s = session(Nation::Fire);
        let (w, d) = (&s.world, &s.defs);
        assert!(current_masters(w, d).is_empty());
        let closest = closest_non_masters(w, d);
        assert_eq!(closest.len(), w.units.len());
        assert!(
            closest.iter().all(|&(_, bends, bend_t, forms, form_t)| (bends, bend_t, forms, form_t)
                == (0, 10, 0, 3))
        );
        let mut names: Vec<&str> = closest.iter().map(|(n, ..)| n.as_str()).collect();
        names.sort();
        let mut unit_names: Vec<&str> = w.units.iter().map(|u| u.name.as_str()).collect();
        unit_names.sort();
        assert_eq!(names, unit_names);
    }

    /// `masters_text`'s own assembly, from literal data (#347, #361): no need to simulate real
    /// bends to cover every shape of the line.
    #[test]
    fn masters_text_covers_every_shape_of_the_line() {
        let jee = ("Jee".to_owned(), 9, 10, 2, 3);
        let shyu = ("Shyu".to_owned(), 8, 10, 3, 3);
        let ikem = ("Ikem".to_owned(), 8, 10, 1, 3);
        let extra = ("Zei".to_owned(), 1, 10, 1, 3);
        assert_eq!(masters_text(&[], &[]), None);
        assert_eq!(
            masters_text(&[], &[jee.clone(), shyu.clone(), ikem.clone(), extra.clone()]),
            Some(
                "Closest: Jee 9 of 10 bends, 2 of 3 forms; Shyu 8 of 10 bends, forms done; \
                 Ikem 8 of 10 bends, 1 of 3 forms"
                    .to_owned()
            ),
            "only the closest three show"
        );
        assert_eq!(
            masters_text(&["Lu Ten".to_owned()], &[jee.clone(), shyu.clone(), ikem.clone()]),
            Some(
                "Master: Lu Ten. Closest: Jee 9 of 10 bends, 2 of 3 forms; \
                 Shyu 8 of 10 bends, forms done; Ikem 8 of 10 bends, 1 of 3 forms"
                    .to_owned()
            )
        );
        assert_eq!(
            masters_text(&["Jee".to_owned(), "Lu Ten".to_owned()], &[]),
            Some("Masters: Jee, Lu Ten.".to_owned()),
            "more than one master is plural, and no closest line without a non-master"
        );
    }

    #[test]
    fn events_read_as_alert_lines() {
        let defs = Defs::bundled();
        let met = EndeavorEvent { tick: 10, kind: EndeavorEventKind::SignMet("beds".into()) };
        assert_eq!(describe_event(&defs, &met), "Sign met: Everyone has a bed");
        let ready = EndeavorEvent { tick: 10, kind: EndeavorEventKind::Ready };
        assert_eq!(describe_event(&defs, &ready), READY_TEXT);
        let stage =
            EndeavorEvent { tick: 10, kind: EndeavorEventKind::StageDone("pillars".into()) };
        assert_eq!(describe_event(&defs, &stage), "Stage done: The Four Pillars");
        let paused = EndeavorEvent { tick: 10, kind: EndeavorEventKind::RitePaused };
        assert!(describe_event(&defs, &paused).contains("pause"));
        let broken = EndeavorEvent { tick: 10, kind: EndeavorEventKind::RiteBroken };
        assert_eq!(describe_event(&defs, &broken), "The rite is broken by grief.");
        let complete = EndeavorEvent { tick: 10, kind: EndeavorEventKind::Complete };
        assert_eq!(describe_event(&defs, &complete), "The Harmony Gate is complete.");
        for (n, want) in [(24, "passes 24 souls"), (32, "passes 32 souls"), (56, "passes 56 souls")]
        {
            let milestone =
                EndeavorEvent { tick: 10, kind: EndeavorEventKind::PopulationMilestone(n) };
            assert!(describe_event(&defs, &milestone).contains(want), "{n}");
        }
        let farewell = EndeavorEvent { tick: 10, kind: EndeavorEventKind::AvatarFarewell };
        assert_eq!(
            describe_event(&defs, &farewell),
            "Aang gives his farewell and flies off on Appa."
        );
        let adept = EndeavorEvent {
            tick: 10,
            kind: EndeavorEventKind::RankUp {
                unit: fn_sim::world::UnitId(0),
                name: "Jee".into(),
                rank: Rank::Adept,
                title: "Firebender".into(),
            },
        };
        assert_eq!(describe_event(&defs, &adept), "Jee is now a Firebender.");
        let mastered = EndeavorEvent {
            tick: 10,
            kind: EndeavorEventKind::RankUp {
                unit: fn_sim::world::UnitId(0),
                name: "Lu Ten".into(),
                rank: Rank::Master,
                title: "Firebending Master".into(),
            },
        };
        assert_eq!(describe_event(&defs, &mastered), "Lu Ten has become a Firebending Master.");
        let earthbender = EndeavorEvent {
            tick: 10,
            kind: EndeavorEventKind::RankUp {
                unit: fn_sim::world::UnitId(0),
                name: "Haru".into(),
                rank: Rank::Adept,
                title: "Earthbender".into(),
            },
        };
        assert_eq!(
            describe_event(&defs, &earthbender),
            "Haru is now an Earthbender.",
            "an Earthbender, not a: article picks by the title's own first letter"
        );
    }

    /// The masters sign's row composes its label from the settlement's own nation threshold
    /// (`EndeavorDef::master_threshold`, #344) and the shared form count
    /// (`MasteryDef::master_forms`, #360, #361), so the data's bare name never drifts from
    /// either: Fire's 10, Earth's and Water's 8, Air's 16, all in 3 forms.
    #[test]
    fn the_masters_sign_names_its_own_nations_threshold() {
        let thresholds =
            [(Nation::Fire, 10), (Nation::Earth, 8), (Nation::Water, 8), (Nation::Air, 16)];
        for (nation, bends) in thresholds {
            let s = session(nation);
            let v = screen_view(&s);
            let row = v.groups.iter().flat_map(|g| &g.signs).find(|r| r.id == "masters").unwrap();
            assert_eq!(row.name, format!("Bending masters ({bends} bends in 3 forms)"));
        }
    }

    /// Under the masters sign, while it isn't met, the screen names the closest non-masters
    /// (`docs/design/endgame.md`, "Showing mastery", #347); once it is met, the row carries
    /// neither a hint nor a closest line, as every other sign already does.
    #[test]
    fn the_masters_sign_names_the_closest_non_masters_while_unmet() {
        let s = session(Nation::Fire);
        let v = screen_view(&s);
        let row = v.groups.iter().flat_map(|g| &g.signs).find(|r| r.id == "masters").unwrap();
        assert!(!row.met);
        assert_eq!(row.closest, masters_line(&s.world, &s.defs));
        assert!(row.closest.as_deref().unwrap().starts_with("Closest: "));
        // Every other sign carries no closest line.
        for other in v.groups.iter().flat_map(|g| &g.signs).filter(|r| r.id != "masters") {
            assert_eq!(other.closest, None);
        }
    }

    /// The masters sign's own hint says how to start a bend, naming the Bend tool's real key,
    /// instead of only explaining credit (#382: playtest agents read the old hint, "Each
    /// finished bend counts for whoever channels it", as bending being a rare, passive event).
    #[test]
    fn the_masters_sign_s_hint_says_how_to_start_a_bend() {
        let s = session(Nation::Fire);
        let v = screen_view(&s);
        let row = v.groups.iter().flat_map(|g| &g.signs).find(|r| r.id == "masters").unwrap();
        let hint = row.hint.as_deref().unwrap();
        // Names the Bend tool's real key (`Tool::key`, the key help's own source), never an
        // F-key: WSL terminals never deliver one.
        let expected = format!(
            "Designate a bend ({}) on a tile a bender can work - credit to whoever channels it",
            Tool::Bend.key()
        );
        assert_eq!(hint, expected);
        assert_eq!(Tool::Bend.key(), "B");
    }

    /// Where `scroll_session` puts its lost ruin, and the other places the tests use.
    const RUIN: Pos = Pos { x: 10, y: 1 };
    const FAR_RUIN: Pos = Pos { x: 11, y: 2 };
    const NEAR_SCROLL: Pos = Pos { x: 7, y: 2 };
    const ISLAND_SCROLL: Pos = Pos { x: 1, y: 1 };

    /// A session on a small map with two surface scrolls, a lost ruin and one settler: X, a
    /// surface scroll on an island nobody can reach; Y, one on the settlement's own landmass;
    /// R, a lost ruin already uncovered (a resource on the map: `World::with_map` buries it, as
    /// it does every buried kind, so it is put back below). Distances from the settler at
    /// (5, 1): X 4 west, Y 2 south-east, R 5 east. No Map Room has charted anything, so the
    /// ruin counts as a Glide find.
    fn scroll_session() -> Session {
        use fn_sim::defs::BiomeId;
        use fn_sim::{Defs, Grid, Terrain, Tile, World};

        use crate::clock::Clock;

        let defs = Defs::bundled();
        let scroll = defs.resource_id("ancient_scroll").unwrap();
        let ruins = defs.resource_id("lost_ruins").unwrap();
        let rows = ["~~~~~~~~~~~~~", "~X~.......R.~", "~~~....Y....~", "~~~~~~~~~~~~~"];
        let map = Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
            let c = rows[p.y as usize].as_bytes()[p.x as usize];
            Tile {
                elevation: 100,
                temperature: 128,
                moisture: 128,
                terrain: if c == b'~' { Terrain::Water } else { Terrain::Land },
                biome: BiomeId(0),
                resource: match c {
                    b'X' | b'Y' => Some(scroll),
                    b'R' => Some(ruins),
                    _ => None,
                },
                building: None,
            }
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(5, 1)], &defs);
        let mut s = Session { world, defs, clock: Clock::default(), bend_choice: None };
        s.world.map[RUIN].resource = Some(ruins);
        s.world.ensure_regions();
        s
    }

    /// Researches `cartography` (made cheap and prerequisite-free for the test) before anything
    /// is marked, so no settler has a job to wander off to.
    fn learn_cartography(s: &mut Session) {
        use fn_sim::world::Command;
        let cart = s.defs.tech_id("cartography").unwrap();
        s.defs.techs[cart.0 as usize].cost = 1;
        s.defs.techs[cart.0 as usize].prereqs.clear();
        s.apply(Command::SetResearch { tech: cart }).unwrap();
        for _ in 0..20_000 {
            if s.world.research().is_known(cart) {
                break;
            }
            s.tick();
        }
        assert!(s.world.research().is_known(cart), "researched");
    }

    /// Records one finished chart that uncovered `found` (a Map Room's chart event), the way a
    /// real one does, by writing it into a save and loading that back: the event is the saved
    /// state `World::has_charted` reads, and nothing outside `fn_sim` can push one.
    fn finish_first_chart(s: &mut Session, found: Pos) {
        use fn_sim::World;
        let text = s.world.save(&s.defs).unwrap();
        let (header, world) = text.rsplit_once('\n').unwrap();
        // First in the event log (#605), on tick 0, so the log's ticks still never go down.
        let event = format!(
            "events:[Chart((tick:0,room:(x:3,y:1),kind:Found(pos:(x:{},y:{}),reachable:true))),",
            found.x, found.y
        );
        assert!(world.contains("events:["), "the save writes its event log");
        let world = world.replacen("events:[", &event, 1);
        s.world = World::load(&format!("{header}\n{world}"), &s.defs).unwrap();
        s.world.ensure_regions();
        assert!(s.world.has_charted() && s.world.charted_at(found));
    }

    fn scrolls_row(s: &Session) -> SignRow {
        let v = screen_view(s);
        v.groups.into_iter().flat_map(|g| g.signs).find(|r| r.id == "scrolls").unwrap()
    }

    fn found(s: &Session) -> Vec<FoundScroll> {
        let sign = s.defs.endeavor.signs.iter().find(|x| x.id == "scrolls").unwrap();
        found_scrolls(&s.world, &s.defs, sign)
    }

    const CART_HINT: &str = "Bring scrolls to a study hall. Cartography charts ruins.";
    const ROOM_HINT: &str = "Bring scrolls to a study hall. A Map Room charts ruins.";
    const PLAIN_HINT: &str = "Bring scrolls to a study hall";

    /// An uncovered great ruin (#736) is listed like a lost one, with "2 scrolls and a teaching"
    /// for its count.
    #[test]
    fn a_great_ruin_lists_its_scrolls_and_a_teaching() {
        let mut s = scroll_session();
        s.world.map[RUIN].resource = s.defs.resource_id("great_ruins");
        let list = found(&s);
        assert_eq!(list.len(), 1, "{list:?}");
        assert_eq!(list[0].name, "Great Ruins (2 scrolls and a teaching)");
        assert_eq!(list[0].distance, "5 tiles east");
    }

    /// Before cartography is known and a chart is done, the list is closed: a marked surface
    /// scroll is not on it, and only a Glide's ruin is (one no chart recorded), since the
    /// player already found it. Unmarked surface scrolls are never on it.
    #[test]
    fn before_the_first_chart_the_scrolls_list_holds_glide_finds_and_nothing_else() {
        use fn_sim::world::Command;

        let mut s = scroll_session();
        let list = found(&s);
        assert_eq!(list.len(), 1, "{list:?}");
        assert_eq!(list[0].name, "Lost Ruins (2)");
        assert_eq!(list[0].distance, "5 tiles east");
        assert_eq!(list[0].status, "not marked");
        assert_eq!(scrolls_row(&s).hint.as_deref(), Some(CART_HINT));

        // Marking a surface scroll does not list it yet.
        s.apply(Command::DesignateHarvest { pos: NEAR_SCROLL }).unwrap();
        s.apply(Command::DesignateHarvest { pos: ISLAND_SCROLL }).unwrap();
        assert_eq!(found(&s).len(), 1, "{:?}", found(&s));
        // The Glide's ruin can be marked, and says so.
        s.apply(Command::DesignateHarvest { pos: RUIN }).unwrap();
        assert_eq!(found(&s)[0].status, "marked");
    }

    /// With nothing found at all, the sign is its name, count and hint: no list, no number of
    /// scrolls left to find, no direction. A ruin still buried is not on it.
    #[test]
    fn with_nothing_found_the_scrolls_sign_shows_only_its_hint() {
        let mut s = scroll_session();
        s.world.map[RUIN].resource = None;
        let row = scrolls_row(&s);
        assert!(row.found.is_empty(), "{:?}", row.found);
        assert_eq!(row.hint.as_deref(), Some(CART_HINT));
        assert_eq!(row.value, "0/3");
        // `World::with_map` buried the ruin; it is still underground.
        assert_eq!(s.world.buried_count(), 1);
    }

    /// Cartography known but nothing charted yet: still closed, and the hint points at the Map
    /// Room instead.
    #[test]
    fn once_cartography_is_known_but_nothing_is_charted_the_hint_names_the_map_room() {
        use fn_sim::world::Command;

        let mut s = scroll_session();
        learn_cartography(&mut s);
        s.apply(Command::DesignateHarvest { pos: NEAR_SCROLL }).unwrap();
        let row = scrolls_row(&s);
        assert_eq!(row.hint.as_deref(), Some(ROOM_HINT));
        assert_eq!(row.found.len(), 1, "the Glide's ruin only: {:?}", row.found);
        assert_eq!(row.found[0].name, "Lost Ruins (2)");
    }

    /// A finished chart without the tech does not open the list either (it cannot happen in
    /// play, since the Map Room needs the tech, but the gate names both).
    #[test]
    fn a_chart_alone_without_cartography_does_not_open_the_list() {
        use fn_sim::world::Command;

        let mut s = scroll_session();
        finish_first_chart(&mut s, RUIN);
        s.apply(Command::DesignateHarvest { pos: NEAR_SCROLL }).unwrap();
        assert!(found(&s).is_empty(), "{:?}", found(&s));
    }

    /// After cartography and a first chart: the list opens. Charted ruins (marked or not) and
    /// marked surface scrolls are on it, nearest first. Unmarked surface scrolls never are, and
    /// the plain hint closes the sign.
    #[test]
    fn after_the_first_chart_the_list_has_charted_ruins_and_marked_scrolls_only() {
        use fn_sim::world::Command;

        let mut s = scroll_session();
        learn_cartography(&mut s);
        finish_first_chart(&mut s, RUIN);
        let list = found(&s);
        assert_eq!(list.len(), 1, "both surface scrolls are unmarked: {list:?}");
        assert_eq!(list[0].name, "Lost Ruins (2)");
        assert_eq!(list[0].status, "not marked");
        assert_eq!(scrolls_row(&s).hint.as_deref(), Some(PLAIN_HINT));

        s.apply(Command::DesignateHarvest { pos: NEAR_SCROLL }).unwrap();
        let list = found(&s);
        assert_eq!(list.len(), 2, "{list:?}");
        // Nearest first from the settlement's anchor, which the settler's walk to research
        // moved a little: the nearer scroll leads, the ruin follows.
        assert_eq!(list[0].name, "Ancient Scroll");
        assert!(list[0].distance.ends_with("south-east"), "{list:?}");
        assert_eq!(list[0].status, "marked");
        assert_eq!(list[1].name, "Lost Ruins (2)");

        s.apply(Command::DesignateHarvest { pos: ISLAND_SCROLL }).unwrap();
        let list = found(&s);
        assert_eq!(list.len(), 3, "{list:?}");
        let island = list.iter().find(|f| f.status == "no settler can reach it").unwrap();
        assert_eq!(island.name, "Ancient Scroll");
        assert!(island.distance.ends_with("west"), "{island:?}");

        s.apply(Command::DesignateHarvest { pos: RUIN }).unwrap();
        let list = found(&s);
        let ruin = list.iter().find(|f| f.name == "Lost Ruins (2)").unwrap();
        assert_eq!(ruin.status, "marked");

        // The Endeavor screen's own row carries the same list while the sign isn't met, and no
        // other sign carries one.
        assert_eq!(scrolls_row(&s).found, found(&s));
        let v = screen_view(&s);
        for other in v.groups.iter().flat_map(|g| &g.signs).filter(|r| r.id != "scrolls") {
            assert!(other.found.is_empty(), "{other:?}");
        }
    }

    /// The gate is saved state: it survives a save and load, and stays open with the charted
    /// ruin gone from the map (recovered) and no Map Room standing anywhere.
    #[test]
    fn the_open_gate_survives_a_save_and_a_recovered_ruin() {
        use fn_sim::World;
        use fn_sim::world::Command;

        let mut s = scroll_session();
        learn_cartography(&mut s);
        finish_first_chart(&mut s, RUIN);
        s.world.map[RUIN].resource = None;
        let text = s.world.save(&s.defs).unwrap();
        s.world = World::load(&text, &s.defs).unwrap();
        s.world.ensure_regions();
        assert!(found(&s).is_empty());
        s.apply(Command::DesignateHarvest { pos: NEAR_SCROLL }).unwrap();
        assert_eq!(found(&s).len(), 1, "a marked surface scroll is listed: the gate is open");
    }

    /// A ruin the Glide uncovered after the gate opened is listed beside the charted ones.
    #[test]
    fn a_glide_find_is_listed_beside_charted_ruins_once_the_gate_is_open() {
        let mut s = scroll_session();
        learn_cartography(&mut s);
        finish_first_chart(&mut s, RUIN);
        s.world.map[FAR_RUIN].resource = s.defs.resource_id("lost_ruins");
        let list = found(&s);
        assert_eq!(list.len(), 2, "{list:?}");
    }

    /// A mark taken off a surface scroll takes it off the list; an uncovered ruin stays, as
    /// "not marked". A scroll that has been recovered is gone from the map and so from the list.
    #[test]
    fn a_surface_scroll_leaves_the_list_when_unmarked_or_recovered() {
        use fn_sim::world::Command;

        let mut s = scroll_session();
        learn_cartography(&mut s);
        finish_first_chart(&mut s, RUIN);
        s.apply(Command::DesignateHarvest { pos: NEAR_SCROLL }).unwrap();
        s.apply(Command::DesignateHarvest { pos: RUIN }).unwrap();
        assert_eq!(found(&s).len(), 2);

        s.apply(Command::CancelHarvest { pos: NEAR_SCROLL }).unwrap();
        s.apply(Command::CancelHarvest { pos: RUIN }).unwrap();
        let list = found(&s);
        assert_eq!(list.len(), 1, "{list:?}");
        assert_eq!(list[0].name, "Lost Ruins (2)");
        assert_eq!(list[0].status, "not marked");

        // Marked again and then recovered: the tile loses its resource, so nothing lists it,
        // and the scroll in a settler's hands or a stockpile is not a source.
        s.apply(Command::DesignateHarvest { pos: NEAR_SCROLL }).unwrap();
        assert_eq!(found(&s).len(), 2);
        s.world.map[NEAR_SCROLL].resource = None;
        assert_eq!(found(&s).len(), 1);
    }
}

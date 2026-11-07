//! The end-game endeavor as the panels show it (#235, `docs/design/endgame.md`, "Showing
//! progress"): the Endeavor screen's view (`G`), the side panel's section, and the alert lines
//! for its events. Reads `World::endeavor_signs` and `World::endeavor` and words them; which signs
//! there are, and when the endeavor is Ready, are the sim's.

use fn_sim::bending::Rank;
use fn_sim::endeavor::{SignDef, SignGroup, SignKind, Tally};
use fn_sim::world::{
    AvatarLeaveReason, EndeavorEvent, EndeavorEventKind, EndeavorState, SignProgress,
};
use fn_sim::{Defs, Pos, World};

use crate::session::Session;
use crate::text::compass_distance;
use crate::tools::Tool;

/// What the screen and the side panel say once the endeavor is Ready (the toast says it too).
pub const READY_TEXT: &str = "Word of your settlement has reached the Avatar.";

/// The Endeavor screen (`G`).
#[derive(Clone, PartialEq, Debug)]
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
    /// The signs under their headings, in data order.
    pub groups: Vec<GroupView>,
}

/// One heading of the screen and its signs.
#[derive(Clone, PartialEq, Debug)]
pub struct GroupView {
    pub heading: String,
    pub signs: Vec<SignRow>,
}

/// One sign's row: a check if met, the name, a hint if not, `value/target` and a bar.
#[derive(Clone, PartialEq, Debug)]
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
    /// closest non-masters ("Master: Lu Ten. Closest: Jee 9/10, Shyu 8/10, Ikem 8/10",
    /// `docs/design/endgame.md`, "Showing mastery", #347). `None` for every other sign.
    pub closest: Option<String>,
    /// The scrolls sign only, shown only while it isn't met: every scroll-yielding resource
    /// still sitting on the map, nearest first (#380, "let a player find out where to look").
    /// Empty for every other sign.
    pub found: Vec<FoundScroll>,
}

/// One scroll-yielding resource still on the map, for the scrolls sign's own row
/// (`found_scrolls`): where it is and why it isn't read yet.
#[derive(Clone, PartialEq, Debug)]
pub struct FoundScroll {
    /// "Ancient Scroll", or "Lost Ruins (2)" when one harvest yields more than one.
    pub name: String,
    /// "34 tiles north-west" (`crate::text::compass_distance`), from the settlement's anchor.
    pub distance: String,
    /// "marked", "not marked" or "no settler can reach it" (`World::job_unreachable`, #337's
    /// flag, reused rather than a new one).
    pub status: String,
}

/// The side panel's Endeavor section.
#[derive(Clone, PartialEq, Debug)]
pub struct EndeavorStatus {
    pub name: String,
    /// "7/10".
    pub count: String,
    pub frac: f32,
    /// The line under the bar: what the screen is for, or that the endeavor is Ready.
    pub line: String,
    /// Up to three things still missing, in data order ("2 more people", "80 stone").
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

/// Names of the settlement's own techs it doesn't know yet, in data order.
fn missing_techs<'d>(world: &World, defs: &'d Defs) -> Vec<&'d str> {
    defs.techs_for(world.nation)
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

/// Non-master units with their own bends and nation's mastery threshold
/// (`fn_sim::endeavor::EndeavorDef::master_threshold`), closest to mastery first (a tie keeps
/// each unit's order in `World::units`). Guests (the Avatar) never count. Empty without a
/// masters sign in the data, or once every counted unit has already mastered.
fn closest_non_masters(world: &World, defs: &Defs) -> Vec<(String, u32, u32)> {
    let mut rest: Vec<(String, u32, u32)> = world
        .units
        .iter()
        .filter(|u| !u.is_avatar())
        .filter_map(|u| {
            let threshold = defs.endeavor.master_threshold(world.nation_of(u))?;
            (u.bends() < threshold).then(|| (u.name.clone(), u.bends(), threshold))
        })
        .collect();
    rest.sort_by_key(|&(_, bends, _)| std::cmp::Reverse(bends));
    rest
}

/// Names of units that have already reached their own nation's mastery threshold, alphabetical.
fn current_masters(world: &World, defs: &Defs) -> Vec<String> {
    let mut masters: Vec<String> = world
        .units
        .iter()
        .filter(|u| !u.is_avatar())
        .filter_map(|u| {
            let threshold = defs.endeavor.master_threshold(world.nation_of(u))?;
            (u.bends() >= threshold).then(|| u.name.clone())
        })
        .collect();
    masters.sort();
    masters
}

/// The masters sign's own line: who has already mastered and the closest non-masters ("Master:
/// Lu Ten. Closest: Jee 9/10, Shyu 8/10, Ikem 8/10"). `None` with nobody to name (no masters
/// sign, or no units at all).
fn masters_line(world: &World, defs: &Defs) -> Option<String> {
    masters_text(&current_masters(world, defs), &closest_non_masters(world, defs))
}

/// Pure text assembly for `masters_line`, split out so it can be tested against literal data
/// without simulating real bends: `masters` (already alphabetical) and `closest` (already
/// closest-first, as `closest_non_masters` returns them).
fn masters_text(masters: &[String], closest: &[(String, u32, u32)]) -> Option<String> {
    let master_part = (!masters.is_empty()).then(|| {
        let label = if masters.len() == 1 { "Master" } else { "Masters" };
        format!("{label}: {}.", masters.join(", "))
    });
    let closest_part = (!closest.is_empty()).then(|| {
        let names: Vec<String> = closest
            .iter()
            .take(MASTERS_SHOWN)
            .map(|(name, bends, threshold)| format!("{name} {bends}/{threshold}"))
            .collect();
        format!("Closest: {}", names.join(", "))
    });
    match (master_part, closest_part) {
        (Some(m), Some(c)) => Some(format!("{m} {c}")),
        (Some(m), None) => Some(m),
        (None, Some(c)) => Some(c),
        (None, None) => None,
    }
}

/// How many found-but-unresolved scrolls the scrolls sign's row names: enough for every seed the
/// supply survey measured (`docs/design/cartography.md`, up to 6 on a small map), without letting
/// a very lucky one turn the row into a wall of text.
const FOUND_SHOWN: usize = 6;

/// Every scroll-yielding resource still sitting on the map, nearest the settlement's anchor
/// first (`World::settlement_anchor`): `ancient_scroll` itself (always visible) and `lost_ruins`
/// once something has uncovered one (Air's Glide, #197, or a Map Room's Chart, #402) -- both
/// yield the same `ancient_scroll` item (`data/jobs.toml`), so one item filter catches both
/// without naming either resource by id, the same trick `fn_sim::autoplay::recover_scrolls`
/// plays. For each: its name (with a count when a harvest yields more than one), how far and
/// which way from the settlement, and whether it's marked, not yet marked, or not reachable by
/// any living settler (`World::job_unreachable`, #337's flag) -- the "why isn't one fetched" a
/// 2026-11-05 playtest never got an answer to (#380). Empty without the `ancient_scroll` item in
/// the data at all.
fn found_scrolls(world: &World, defs: &Defs) -> Vec<FoundScroll> {
    let Some(scroll_item) = defs.item_id("ancient_scroll") else { return Vec::new() };
    let anchor = world.settlement_anchor();
    let mut found: Vec<Pos> = world
        .map
        .iter()
        .filter(|(_, t)| {
            t.resource.and_then(|r| defs.harvest(r)).is_some_and(|h| h.item == scroll_item)
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
            let name = if h.count > 1 {
                format!("{} ({})", defs.resource(resource).name, h.count)
            } else {
                defs.resource(resource).name.clone()
            };
            let status = if world.job_unreachable(pos) == Some(true) {
                "no settler can reach it"
            } else if world.harvest_designations().any(|p| p == pos) {
                "marked"
            } else {
                "not marked"
            };
            FoundScroll { name, distance: compass_distance(anchor, pos), status: status.to_owned() }
        })
        .collect()
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
        _ => sign.hint.clone(),
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

/// What a sign still needs, as a count and the data's own short noun ("2 beds", "150 food",
/// `SignDef::short`, #416, docs/design/side-panel-v2.md "Zone 1"): the side panel's compact Gate
/// row and its `next` list both use this instead of a sentence, so the row ("Gate 8/9 · 3
/// scrolls [G]") never needs to wrap. The fuller wording (which techs or masters are closest)
/// stays on the Endeavor screen's own hint (`hint`, `masters_line`).
pub fn missing(sign: &SignDef, p: &SignProgress) -> String {
    format!("{} {}", p.target.saturating_sub(p.value), sign.short)
}

/// A sign's label: its data name, except that the branch sign names the nation's branch, and the
/// masters sign composes its own threshold ("Bending masters (10 bends each)") so the label
/// can't drift from the settlement's own number (`EndeavorDef::master_threshold`, #344).
fn sign_name(sign: &SignDef, world: &World, defs: &Defs) -> String {
    match &sign.kind {
        SignKind::BranchComplete => format!("{} branch", defs.nation(world.nation).name),
        SignKind::Masters => {
            let threshold = defs
                .endeavor
                .master_threshold(world.nation)
                .expect("this sign is the masters sign");
            format!("{} ({threshold} bends each)", sign.name)
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

pub fn screen_view(session: &Session) -> ScreenView {
    let (world, defs) = (&session.world, &session.defs);
    let signs = signs(world, defs);
    let met = signs.iter().filter(|(_, p)| p.met).count();
    let total = signs.len();
    let mut groups: Vec<GroupView> = Vec::new();
    for (sign, p) in &signs {
        let closest = (!p.met && matches!(sign.kind, SignKind::Masters))
            .then(|| masters_line(world, defs))
            .flatten();
        let found = if !p.met && matches!(sign.kind, SignKind::Tally(Tally::ScrollsRead)) {
            found_scrolls(world, defs)
        } else {
            Vec::new()
        };
        let row = SignRow {
            id: sign.id.clone(),
            name: sign_name(sign, world, defs),
            met: p.met,
            hint: if p.met { None } else { hint(sign, world, defs) },
            value: format!("{}/{}", p.value, p.target),
            frac: frac(p),
            closest,
            found,
        };
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
        groups,
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
    let next = if ready {
        Vec::new()
    } else {
        signs.iter().filter(|(_, p)| !p.met).take(NEXT_SHOWN).map(|(s, p)| missing(s, p)).collect()
    };
    Some(EndeavorStatus {
        name: defs.endeavor.name.clone(),
        count: format!("{met}/{}", signs.len()),
        frac: share(met, signs.len()),
        line: match world.endeavor() {
            EndeavorState::Dormant => "Signs met - G: open".to_owned(),
            EndeavorState::Ready { .. } => READY_TEXT.to_owned(),
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
        EndeavorEventKind::Ready => READY_TEXT.to_owned(),
        EndeavorEventKind::StageDone(id) => {
            let name = defs.endeavor.stage(id).map_or(id.as_str(), |(_, s)| &s.name);
            format!("Stage done: {name}")
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
        EndeavorEventKind::AvatarFarewell => {
            let name = defs.endeavor.avatar.as_ref().map_or("The Avatar", |a| a.name.as_str());
            format!("{name} gives his farewell and flies off on Appa.")
        }
        // A settler's rank-up toast (#347, #360): fires once per rank, `World::note_rank_up`.
        EndeavorEventKind::RankUp { name, rank, .. } => match rank {
            Rank::Adept => format!("{name} is now an adept bender"),
            Rank::Master => format!("{name} is now a bending master"),
            Rank::Novice => unreachable!("nobody ranks up to Novice"),
        },
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;

    fn session(nation: Nation) -> Session {
        Session::new(7, nation, 64, 64)
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
        let branch = s.defs.techs.iter().filter(|t| t.nation == Some(Nation::Earth)).count();
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
        // A new Earth game hasn't raised any of its 11 reachable kinds yet (4 shared, the Map
        // Room, #402, and the Herb Garden, #390, earth_wall, rail, the shared Workshop, #369,
        // and the Mine Shaft and Deep Mine, #373).
        let s = session(Nation::Earth);
        let left = missing_building_names(&s.world, &s.defs);
        assert_eq!(left.len(), 11);
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

    #[test]
    fn the_side_panel_names_the_next_three_missing_things() {
        let s = session(Nation::Fire);
        let status = endeavor_status(&s).unwrap();
        assert_eq!(status.name, "The Harmony Gate");
        assert_eq!(status.next.len(), NEXT_SHOWN);
        let people = 16 - s.world.units.len();
        assert_eq!(status.next[0], format!("{people} people"));
        assert_eq!(status.next[1], format!("{} beds", s.world.units.len()));
        assert_eq!(status.next[2], "11 kinds");
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
        assert_eq!(text("food"), "150 food");
        assert_eq!(text("stone"), "200 stone");
        assert_eq!(text("ore"), "40 ore");
        assert_eq!(text("masters"), "3 masters");
        assert_eq!(text("scrolls"), "3 scrolls");
    }

    /// `closest_non_masters` and `current_masters` on a fresh game: nobody has bent yet, so
    /// everyone is a non-master at 0/10 (Fire's own threshold), and nobody is a master.
    #[test]
    fn mastery_helpers_read_a_fresh_games_units() {
        let s = session(Nation::Fire);
        let (w, d) = (&s.world, &s.defs);
        assert!(current_masters(w, d).is_empty());
        let closest = closest_non_masters(w, d);
        assert_eq!(closest.len(), w.units.len());
        assert!(closest.iter().all(|(_, bends, threshold)| (*bends, *threshold) == (0, 10)));
        let mut names: Vec<&str> = closest.iter().map(|(n, ..)| n.as_str()).collect();
        names.sort();
        let mut unit_names: Vec<&str> = w.units.iter().map(|u| u.name.as_str()).collect();
        unit_names.sort();
        assert_eq!(names, unit_names);
    }

    /// `masters_text`'s own assembly, from literal data (#347): no need to simulate real bends
    /// to cover every shape of the line.
    #[test]
    fn masters_text_covers_every_shape_of_the_line() {
        let jee = ("Jee".to_owned(), 9, 10);
        let shyu = ("Shyu".to_owned(), 8, 10);
        let ikem = ("Ikem".to_owned(), 8, 10);
        let extra = ("Zei".to_owned(), 1, 10);
        assert_eq!(masters_text(&[], &[]), None);
        assert_eq!(
            masters_text(&[], &[jee.clone(), shyu.clone(), ikem.clone(), extra.clone()]),
            Some("Closest: Jee 9/10, Shyu 8/10, Ikem 8/10".to_owned()),
            "only the closest three show"
        );
        assert_eq!(
            masters_text(&["Lu Ten".to_owned()], &[jee.clone(), shyu.clone(), ikem.clone()]),
            Some("Master: Lu Ten. Closest: Jee 9/10, Shyu 8/10, Ikem 8/10".to_owned())
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
            },
        };
        assert_eq!(describe_event(&defs, &adept), "Jee is now an adept bender");
        let mastered = EndeavorEvent {
            tick: 10,
            kind: EndeavorEventKind::RankUp {
                unit: fn_sim::world::UnitId(0),
                name: "Jee".into(),
                rank: Rank::Master,
            },
        };
        assert_eq!(describe_event(&defs, &mastered), "Jee is now a bending master");
    }

    /// The masters sign's row composes its label from the settlement's own nation threshold
    /// (`EndeavorDef::master_threshold`, #344), so the data's bare name never drifts from it:
    /// Fire's 10, Earth's and Water's 8, Air's 16.
    #[test]
    fn the_masters_sign_names_its_own_nations_threshold() {
        let thresholds =
            [(Nation::Fire, 10), (Nation::Earth, 8), (Nation::Water, 8), (Nation::Air, 16)];
        for (nation, bends) in thresholds {
            let s = session(nation);
            let v = screen_view(&s);
            let row = v.groups.iter().flat_map(|g| &g.signs).find(|r| r.id == "masters").unwrap();
            assert_eq!(row.name, format!("Bending masters ({bends} bends each)"));
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

    /// The scrolls sign's own found-but-unresolved list (#380, "let a player find out where to
    /// look"): nearest first, naming whether each is marked, not yet marked, or stranded where
    /// no settler can reach it -- the "why isn't one fetched" a 2026-11-05 playtest never got.
    #[test]
    fn the_scrolls_sign_lists_found_scrolls_nearest_first_with_their_status() {
        use fn_sim::defs::BiomeId;
        use fn_sim::{Defs, Grid, Nation, Terrain, Tile, World};

        use crate::clock::Clock;

        let defs = Defs::bundled();
        let scroll = defs.resource_id("ancient_scroll").unwrap();
        // X: an island scroll, surrounded by water on every side, nobody can ever reach it.
        // Y: an ordinary scroll on the settlement's own landmass, 5 tiles closer.
        let rows = ["~~~~~~~~~~~", "~X~.......~", "~~~....Y..~", "~~~~~~~~~~~"];
        let map = Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
            let c = rows[p.y as usize].as_bytes()[p.x as usize];
            Tile {
                elevation: 100,
                temperature: 128,
                moisture: 128,
                terrain: if c == b'~' { Terrain::Water } else { Terrain::Land },
                biome: BiomeId(0),
                resource: (c == b'X' || c == b'Y').then_some(scroll),
                building: None,
            }
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(5, 1)], &defs);
        let mut s = Session { world, defs, clock: Clock::default(), bend_choice: None };
        s.world.ensure_regions();

        let found = found_scrolls(&s.world, &s.defs);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].name, "Ancient Scroll");
        assert_eq!(found[0].distance, "2 tiles south-east");
        assert_eq!(found[0].status, "not marked", "reachable and not yet marked");
        assert_eq!(found[1].distance, "4 tiles west");
        assert_eq!(found[1].status, "no settler can reach it");

        // Marking the reachable one updates its own status to "marked"; the island's doesn't
        // change, since marking it would never do anything.
        s.apply(fn_sim::world::Command::DesignateHarvest { pos: Pos::new(7, 2) }).unwrap();
        let found = found_scrolls(&s.world, &s.defs);
        assert_eq!(found[0].status, "marked");
        assert_eq!(found[1].status, "no settler can reach it");

        // The Endeavor screen's own row carries the same list while the sign isn't met.
        let v = screen_view(&s);
        let row = v.groups.iter().flat_map(|g| &g.signs).find(|r| r.id == "scrolls").unwrap();
        assert_eq!(row.found, found);
        // Every other sign carries no found list at all.
        for other in v.groups.iter().flat_map(|g| &g.signs).filter(|r| r.id != "scrolls") {
            assert!(other.found.is_empty(), "{other:?}");
        }
    }
}

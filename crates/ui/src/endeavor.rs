//! The end-game endeavor as the panels show it (#235, `docs/design/endgame.md`, "Showing
//! progress"): the Endeavor screen's view (`G`), the side panel's section, and the alert lines
//! for its events. Reads `World::endeavor_signs` and `World::endeavor` and words them; which signs
//! there are, and when the endeavor is Ready, are the sim's.

use fn_sim::endeavor::{SignDef, SignGroup, SignKind, Tally};
use fn_sim::world::{
    AvatarLeaveReason, EndeavorEvent, EndeavorEventKind, EndeavorState, SignProgress,
};
use fn_sim::{Defs, World};

use crate::session::Session;

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

/// The hint for an unmet sign: the branch names the techs left once only one or two are, the
/// buildings sign names the missing kinds the same way ("Not yet raised: Earth Wall, Rail"),
/// and every other sign uses its data hint.
fn hint(sign: &SignDef, world: &World, defs: &Defs) -> Option<String> {
    match &sign.kind {
        SignKind::BranchComplete => branch_hint(&missing_techs(world, defs), sign.hint.as_deref()),
        SignKind::EveryBuilding { .. } => {
            building_hint(&missing_building_names(world, defs), sign.hint.as_deref())
        }
        _ => sign.hint.clone(),
    }
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

/// What the branch sign still needs: the tech's name when one is left, else how many.
fn branch_missing(left: &[&str]) -> String {
    match left {
        [one] => (*one).to_owned(),
        _ => count(left.len() as u32, "more tech", "more techs"),
    }
}

/// "1 more person" or "3 more people": `n` and the right noun.
fn count(n: u32, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// What a sign still needs, in a few words: "2 more people", "Metalbending", "80 stone".
pub fn missing(sign: &SignDef, p: &SignProgress, world: &World, defs: &Defs) -> String {
    let n = p.target.saturating_sub(p.value);
    let items = |items: &[fn_sim::jobs::ItemId]| match items {
        [one] => defs.item(*one).name.to_lowercase(),
        many if many.iter().all(|&i| defs.food_of(i).is_some()) => "food".to_owned(),
        // A sign whose extra items are all stand-ins for the first (brick for stone, issue
        // #371) is still named for that one item: "200 stone", not the sign's own name.
        [primary, rest @ ..] if rest.iter().all(|&i| defs.stands_for(i) == Some(*primary)) => {
            defs.item(*primary).name.to_lowercase()
        }
        _ => sign.name.to_lowercase(),
    };
    match &sign.kind {
        SignKind::Population => count(n, "more person", "more people"),
        SignKind::BedsForAll => count(n, "more bed", "more beds"),
        SignKind::EveryBuilding { .. } => {
            count(n, "more kind of building", "more kinds of building")
        }
        SignKind::BranchComplete => branch_missing(&missing_techs(world, defs)),
        SignKind::Stored { items: list } | SignKind::Tally(Tally::Harvested(list)) => {
            format!("{n} {}", items(list))
        }
        SignKind::Tally(Tally::ScrollsRead) => count(n, "more scroll read", "more scrolls read"),
        SignKind::Masters { .. } => {
            let label = count(n, "more master", "more masters");
            match closest_non_masters(world, defs).first() {
                Some((name, bends, threshold)) => format!("{label} ({name} {bends}/{threshold})"),
                None => label,
            }
        }
    }
}

/// A sign's label: its data name, except that the branch sign names the nation's branch.
fn sign_name(sign: &SignDef, world: &World, defs: &Defs) -> String {
    if sign.kind == SignKind::BranchComplete {
        format!("{} branch", defs.nation(world.nation).name)
    } else {
        sign.name.clone()
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
        let closest = (!p.met && matches!(sign.kind, SignKind::Masters { .. }))
            .then(|| masters_line(world, defs))
            .flatten();
        let row = SignRow {
            id: sign.id.clone(),
            name: sign_name(sign, world, defs),
            met: p.met,
            hint: if p.met { None } else { hint(sign, world, defs) },
            value: format!("{}/{}", p.value, p.target),
            frac: frac(p),
            closest,
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
        signs
            .iter()
            .filter(|(_, p)| !p.met)
            .take(NEXT_SHOWN)
            .map(|(s, p)| missing(s, p, world, defs))
            .collect()
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
        // The masters sign's own toast (#347): fires once per unit, `World::note_mastery`.
        EndeavorEventKind::Mastered { name, .. } => format!("{name} is now a bending master"),
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
        assert_eq!(branch_missing(&["Metalbending"]), "Metalbending");
        assert_eq!(branch_missing(&["A", "B"]), "2 more techs");
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
        // A new Earth game hasn't raised any of its 9 reachable kinds yet (4 shared, earth_wall,
        // rail, the shared Workshop, #369, and the Mine Shaft and Deep Mine, #373).
        let s = session(Nation::Earth);
        let left = missing_building_names(&s.world, &s.defs);
        assert_eq!(left.len(), 9);
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
        assert_eq!(status.next[0], format!("{people} more people"));
        assert_eq!(status.next[1], format!("{} more beds", s.world.units.len()));
        assert_eq!(status.next[2], "9 more kinds of building");
        assert_eq!(status.line, "Signs met - G: open");
    }

    #[test]
    fn missing_things_are_worded_by_kind() {
        let s = session(Nation::Earth);
        let (w, d) = (&s.world, &s.defs);
        let text = |id: &str| {
            let (sign, p) = signs(w, d).into_iter().find(|(s, _)| s.id == id).unwrap();
            missing(sign, &p, w, d)
        };
        assert_eq!(text("food"), "150 food");
        assert_eq!(text("stone"), "200 stone");
        assert_eq!(text("ore"), "40 gold ore");
        // A fresh game: nobody has bent yet, so the closest is whoever is first, at 0/25
        // (`EndeavorDef::master_threshold` reads the bundled sign).
        let closest = &w.units[0].name;
        assert_eq!(text("masters"), format!("3 more masters ({closest} 0/25)"));
        assert_eq!(text("scrolls"), "3 more scrolls read");
    }

    /// `closest_non_masters` and `current_masters` on a fresh game: nobody has bent yet, so
    /// everyone is a non-master at 0/25, and nobody is a master.
    #[test]
    fn mastery_helpers_read_a_fresh_games_units() {
        let s = session(Nation::Fire);
        let (w, d) = (&s.world, &s.defs);
        assert!(current_masters(w, d).is_empty());
        let closest = closest_non_masters(w, d);
        assert_eq!(closest.len(), w.units.len());
        assert!(closest.iter().all(|(_, bends, threshold)| (*bends, *threshold) == (0, 25)));
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
        let mastered = EndeavorEvent {
            tick: 10,
            kind: EndeavorEventKind::Mastered {
                unit: fn_sim::world::UnitId(0),
                name: "Jee".into(),
            },
        };
        assert_eq!(describe_event(&defs, &mastered), "Jee is now a bending master");
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
}

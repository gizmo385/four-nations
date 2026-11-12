//! The Airship screen (docs/design/horizons-phase-6.md, "`airships`" and "Mockups", #763):
//! opened with `Enter` on an Airship Dock in Select mode. Sail to a nation, pick up to the dock's
//! `deals` of its caravan deals and how many times each, and `Enter` asks for the confirm box
//! (SEND THE AIRSHIP) before `Command::PlanVoyage` goes to the sim. Once a voyage is planned the
//! screen reads it back (loading, fuel, crew, away) and `X` cancels it with `Command::
//! CancelVoyage`.
//!
//! The picks are the frontend's own (`UiState::airship`): which nation and deal the pointer is
//! on and how many times each deal is taken. What can be sent, what a voyage carries and brings
//! back, and every refusal are the sim's (`World::voyage_status`, `VoyageError`); this only
//! words them and checks the cheap ones up front so a refused `+` doesn't need a round trip.

use fn_sim::building::{AirshipDef, MAX_VOYAGE_DEALS};
use fn_sim::trade::CaravanDef;
use fn_sim::world::{Command, TICKS_PER_SECOND, VoyageError, VoyageStatus};
use fn_sim::{Defs, Nation, Pos, World};

use crate::session::Session;
use crate::trade::{and_list, deal_words, goods_name, in_stores};

/// Which column of the screen the arrows move in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Column {
    /// SAIL TO: the nation.
    #[default]
    Nation,
    /// DEALS: the deal the `+` and `-` keys change.
    Deals,
}

/// What the player has picked on the open screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picks {
    /// The anchor of the dock the screen is for.
    pub dock: Pos,
    pub column: Column,
    /// An index into [`caravans`].
    pub nation: usize,
    /// An index into the nation's deals.
    pub deal: usize,
    /// How many times each of the nation's deals is taken, in the caravan's order.
    pub times: Vec<u8>,
    /// Whether the SEND THE AIRSHIP box is up.
    pub confirm: bool,
}

/// One deal of the nation being sailed to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DealRow {
    /// "25 wood for 3 gold ore".
    pub text: String,
    /// "2/2": how many times it is taken, of how many it allows.
    pub times: String,
    /// "1640 wood": what the stores hold of what it takes.
    pub stores: String,
    /// Whether it is taken at least once.
    pub picked: bool,
}

/// The screen's contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AirshipView {
    /// "Airship Dock at (61, 40)".
    pub head: String,
    /// "Docked", "Loading for Earth Kingdom: 32/50", "Away: back in 9 min".
    pub status: String,
    /// "Fuel 20/20 Wood", empty for a dock with no bunker.
    pub fuel: String,
    /// "Crew: 2 residents".
    pub crew: String,
    /// Each nation that can be sailed to, as "Earth Kingdom" or "Water Tribes (kin)".
    pub nations: Vec<String>,
    pub nation_at: usize,
    /// The nation's deals, or while a voyage is planned the ones it takes.
    pub deals: Vec<DealRow>,
    pub deal_at: usize,
    pub column: Column,
    /// Whether the picks can change: no voyage is planned or away.
    pub editable: bool,
    /// "Out: 50 wood", or "Out: nothing" with nothing picked.
    pub out: String,
    /// "50/60": the items out, of what the hold takes.
    pub load: String,
    /// "Back in about 15 min with 6 gold ore", or what to do first.
    pub back: String,
}

/// The airships' caravans, in file order: what a dock can sail to (the United City's scaled set
/// once the gate stands, `World::united`).
pub fn caravans(session: &Session) -> &[CaravanDef] {
    session.defs.trade.caravans_for(session.world.united())
}

/// The anchor of the finished Airship Dock covering `pos`, if one does.
pub fn dock_at(session: &Session, pos: Pos) -> Option<Pos> {
    let (world, defs) = (&session.world, &session.defs);
    let id = world.building_at(pos)?;
    defs.building(id).airship?;
    world.footprint_at(pos).map(|f| f.anchor)
}

/// The dock's airship, from the data.
fn def_of(session: &Session, dock: Pos) -> Option<AirshipDef> {
    let id = session.world.building_at(dock)?;
    session.defs.building(id).airship
}

/// The picks a freshly opened screen starts with: the dock under `pos`, on the first nation that
/// isn't the settlement's own, if there is one. `None` off a dock.
pub fn open(session: &Session, pos: Pos) -> Option<Picks> {
    let dock = dock_at(session, pos)?;
    let list = caravans(session);
    let nation =
        list.iter().position(|c| c.nation != session.world.nation).unwrap_or(0).min(list.len());
    let deals = list.get(nation).map_or(0, |c| c.deals.len());
    Some(Picks {
        dock,
        column: Column::Nation,
        nation,
        deal: 0,
        times: vec![0; deals],
        confirm: false,
    })
}

/// `picks` with the nation and deal pointers brought back inside the lists, and `times` the
/// right length: a save loaded or the gate standing since can change what is there.
pub fn settle(session: &Session, picks: &mut Picks) {
    let list = caravans(session);
    picks.nation = picks.nation.min(list.len().saturating_sub(1));
    let deals = list.get(picks.nation).map_or(0, |c| c.deals.len());
    picks.times.resize(deals, 0);
    picks.deal = picks.deal.min(deals.saturating_sub(1));
}

/// The nation's name as the list says it: "Earth Kingdom", "Water Tribes (kin)".
fn nation_name(session: &Session, nation: Nation) -> String {
    let name = &session.defs.nation(nation).name;
    if nation == session.world.nation { format!("{name} (kin)") } else { name.clone() }
}

/// "9 min": whole minutes, rounded up, at least 1.
fn minutes(ticks: u64) -> u64 {
    ticks.div_ceil(u64::from(TICKS_PER_SECOND) * 60).max(1)
}

/// What the dock is doing, for its screen and its cursor panel: "Docked", "Loading for Earth
/// Kingdom: 32/50", "Needs fuel: 12/20 Wood", "Boarding: 1/2 crew" or "Away: back in 9 min".
/// `None` off an Airship Dock.
pub fn status_text(world: &World, defs: &Defs, dock: Pos) -> Option<String> {
    let status = world.voyage_status(dock, defs)?;
    let to = |nation: Nation| defs.nation(nation).name.clone();
    let plan_nation = world.voyage_plan(dock).map(|p| to(p.nation)).unwrap_or_default();
    Some(match status {
        VoyageStatus::Docked => "Docked".to_owned(),
        VoyageStatus::Loading { held, out } => {
            format!("Loading for {plan_nation}: {held}/{out}")
        }
        VoyageStatus::NeedsFuel { have, need } => {
            let fuel = world
                .bunker_at(dock, defs)
                .map_or_else(String::new, |(_, f)| format!(" {}", f.names(defs)));
            format!("Needs fuel: {have}/{need}{fuel}")
        }
        VoyageStatus::Boarding { aboard, crew } => format!("Boarding: {aboard}/{crew} crew"),
        VoyageStatus::Away { ticks } => format!("Away: back in {} min", minutes(ticks)),
    })
}

/// "Fuel 20/20 Wood", or empty with no bunker.
fn fuel_text(session: &Session, dock: Pos) -> String {
    let (world, defs) = (&session.world, &session.defs);
    world.bunker_at(dock, defs).map_or_else(String::new, |(have, fuel)| {
        let item = have.item.map_or_else(|| fuel.names(defs), |i| defs.item(i).name.clone());
        format!("Fuel {}/{} {item}", have.count, fuel.bunker)
    })
}

/// "Crew: 2 residents".
fn crew_text(def: AirshipDef) -> String {
    format!("Crew: {} resident{}", def.crew, if def.crew == 1 { "" } else { "s" })
}

/// What a set of deals taken some times takes and brings: the merged give and get lists in
/// words, and how many items go out.
struct Cargo {
    out: Vec<(String, u32)>,
    back: Vec<(String, u32)>,
    items: u32,
}

/// Adds `n` of `name` to `list`, merging with an entry of the same name.
fn add(list: &mut Vec<(String, u32)>, name: String, n: u32) {
    match list.iter_mut().find(|(k, _)| *k == name) {
        Some((_, have)) => *have += n,
        None => list.push((name, n)),
    }
}

/// `picked`: each deal of `caravan` by index with its times.
fn cargo(session: &Session, caravan: &CaravanDef, picked: &[(usize, u8)]) -> Cargo {
    let defs = &session.defs;
    let mut cargo = Cargo { out: Vec::new(), back: Vec::new(), items: 0 };
    for &(i, times) in picked.iter().filter(|&&(_, t)| t > 0) {
        let Some(deal) = caravan.deals.get(i) else { continue };
        let times = u32::from(times);
        for g in &deal.give {
            add(&mut cargo.out, goods_name(defs, g), g.count * times);
            cargo.items += g.count * times;
        }
        for &(item, n) in &deal.get {
            add(&mut cargo.back, defs.item(item).name.to_lowercase(), n * times);
        }
        if deal.knowledge > 0 {
            add(&mut cargo.back, "knowledge".to_owned(), deal.knowledge * times);
        }
    }
    cargo
}

/// "50 wood and 10 food".
fn counted(list: &[(String, u32)]) -> String {
    and_list(&list.iter().map(|(name, n)| format!("{n} {name}")).collect::<Vec<_>>())
}

/// The screen's contents for `picks`.
pub fn view(session: &Session, picks: &Picks) -> AirshipView {
    let (world, defs) = (&session.world, &session.defs);
    let dock = picks.dock;
    let def = def_of(session, dock);
    let status = status_text(world, defs, dock).unwrap_or_default();
    let list = caravans(session);
    let nations: Vec<String> = list.iter().map(|c| nation_name(session, c.nation)).collect();
    let away = world.voyage_away(dock);
    let plan = world.voyage_plan(dock);
    let editable = plan.is_none() && away.is_none();
    let head = format!("Airship Dock at ({}, {})", dock.x, dock.y);
    let crew = def.map_or_else(String::new, crew_text);
    let fuel = fuel_text(session, dock);
    let hold = def.map_or(0, |d| d.hold);

    // A planned voyage reads back its own nation and deals; an airship that is away, only where
    // it went and what it brings.
    let shown_nation = match (plan, away) {
        (Some(p), _) => Some(p.nation),
        (None, Some(a)) => Some(a.nation),
        (None, None) => list.get(picks.nation).map(|c| c.nation),
    };
    let nation_at =
        shown_nation.and_then(|n| list.iter().position(|c| c.nation == n)).unwrap_or(picks.nation);
    let caravan = list.get(nation_at);
    let row = |deal: &fn_sim::trade::DealDef, times: u8| {
        let (give, get) = deal_words(defs, deal);
        let stores: Vec<String> = deal
            .give
            .iter()
            .map(|g| format!("{} {}", in_stores(world, g), goods_name(defs, g)))
            .collect();
        DealRow {
            text: format!("{give} for {get}"),
            times: format!("{times}/{}", deal.times),
            stores: and_list(&stores),
            picked: times > 0,
        }
    };
    let (deals, picked): (Vec<DealRow>, Vec<(usize, u8)>) = match (caravan, plan) {
        (Some(c), Some(p)) => (
            p.deals
                .iter()
                .filter_map(|d| Some(row(c.deals.get(usize::from(d.deal))?, d.times)))
                .collect(),
            p.deals.iter().map(|d| (usize::from(d.deal), d.times)).collect(),
        ),
        (Some(_), None) if away.is_some() => (Vec::new(), Vec::new()),
        (Some(c), None) => (
            c.deals
                .iter()
                .enumerate()
                .map(|(i, d)| row(d, picks.times.get(i).copied().unwrap_or(0)))
                .collect(),
            picks.times.iter().copied().enumerate().collect(),
        ),
        (None, _) => (Vec::new(), Vec::new()),
    };
    let cargo = caravan.map(|c| cargo(session, c, &picked));
    let (out, load, back) = match (&cargo, away) {
        (_, Some(a)) => {
            let back: Vec<(String, u32)> = {
                let mut list = Vec::new();
                for s in &a.back {
                    add(&mut list, defs.item(s.item).name.to_lowercase(), s.count);
                }
                if a.knowledge > 0 {
                    add(&mut list, "knowledge".to_owned(), a.knowledge);
                }
                list
            };
            let text = if back.is_empty() {
                format!("Back in {} min", minutes(a.lands.saturating_sub(world.tick)))
            } else {
                format!(
                    "Back in {} min with {}",
                    minutes(a.lands.saturating_sub(world.tick)),
                    counted(&back)
                )
            };
            ("Out: gone".to_owned(), String::new(), text)
        }
        (Some(c), None) if c.items > 0 => {
            let away_min = def.map_or(0, |d| minutes(d.away_ticks));
            (
                format!("Out: {}", counted(&c.out)),
                format!("{}/{hold}", c.items),
                format!("Back in about {away_min} min with {}", counted(&c.back)),
            )
        }
        _ => {
            ("Out: nothing".to_owned(), format!("0/{hold}"), "Pick a deal to sail with".to_owned())
        }
    };
    AirshipView {
        head,
        status,
        fuel,
        crew,
        nations,
        nation_at,
        deal_at: picks.deal.min(deals.len().saturating_sub(1)),
        deals,
        column: picks.column,
        editable,
        out,
        load,
        back,
    }
}

/// An arrow key: Left and Right swap the column the pointer is in, Up and Down move it within.
/// Choosing another nation clears the picks, which belong to the nation's own deals.
pub fn step(session: &Session, picks: &mut Picks, dx: i32, dy: i32) {
    if dx != 0 {
        picks.column = if dx < 0 { Column::Nation } else { Column::Deals };
    }
    let list = caravans(session);
    match picks.column {
        Column::Nation if dy != 0 && !list.is_empty() => {
            let to = (picks.nation as i64 + i64::from(dy.signum())).clamp(0, list.len() as i64 - 1)
                as usize;
            if to != picks.nation {
                picks.nation = to;
                picks.deal = 0;
                picks.times = vec![0; list[to].deals.len()];
            }
        }
        Column::Deals if dy != 0 => {
            let n = list.get(picks.nation).map_or(0, |c| c.deals.len());
            if n > 0 {
                picks.deal =
                    (picks.deal as i64 + i64::from(dy.signum())).clamp(0, n as i64 - 1) as usize;
            }
        }
        _ => {}
    }
}

/// `+` or `-` (`delta` is 1 or -1) on the deal under the pointer: one more or one less time,
/// within what the deal allows, the dock's `deals` and its hold. The reason, in the sim's words,
/// when it can't.
pub fn adjust(session: &Session, picks: &mut Picks, delta: i32) -> Result<(), String> {
    settle(session, picks);
    let status = session.world.voyage_status(picks.dock, &session.defs);
    match status {
        Some(VoyageStatus::Docked) => {}
        Some(VoyageStatus::Away { .. }) => return Err(VoyageError::Away.to_string()),
        _ => return Err(VoyageError::Planned.to_string()),
    }
    let (Some(caravan), Some(def)) =
        (caravans(session).get(picks.nation), def_of(session, picks.dock))
    else {
        return Err(VoyageError::NoCaravan.to_string());
    };
    let Some(deal) = caravan.deals.get(picks.deal) else {
        return Err(VoyageError::NoSuchDeal.to_string());
    };
    let now = picks.times[picks.deal];
    let to = (i32::from(now) + delta).clamp(0, deal.times.min(u32::from(u8::MAX)) as i32) as u8;
    if to > now {
        if now == 0 && picks.times.iter().filter(|&&t| t > 0).count() >= usize::from(def.deals) {
            return Err(VoyageError::TooManyDeals { most: def.deals }.to_string());
        }
        let per: u32 = deal.give.iter().map(|g| g.count).sum();
        let out: u32 = picked_items(caravan, &picks.times) + per;
        if out > def.hold {
            return Err(VoyageError::TooBig { out, hold: def.hold }.to_string());
        }
    }
    picks.times[picks.deal] = to;
    Ok(())
}

/// How many items the picked deals send out.
fn picked_items(caravan: &CaravanDef, times: &[u8]) -> u32 {
    caravan
        .deals
        .iter()
        .zip(times)
        .map(|(d, &t)| d.give.iter().map(|g| g.count).sum::<u32>() * u32::from(t))
        .sum()
}

/// The deals picked, as `Command::PlanVoyage` takes them: `(deal, times)` for each picked, the
/// rest `(0, 0)`.
fn picked_deals(picks: &Picks) -> [(u8, u8); MAX_VOYAGE_DEALS] {
    let mut out = [(0, 0); MAX_VOYAGE_DEALS];
    let picked = picks.times.iter().enumerate().filter(|&(_, &t)| t > 0);
    for (slot, (i, &t)) in out.iter_mut().zip(picked) {
        *slot = (i as u8, t);
    }
    out
}

/// `Enter`: asks for the confirm box, unless nothing is picked or a voyage is already planned
/// or away. The reason when it can't.
pub fn ask(session: &Session, picks: &mut Picks) -> Result<(), String> {
    settle(session, picks);
    match session.world.voyage_status(picks.dock, &session.defs) {
        Some(VoyageStatus::Docked) => {}
        Some(VoyageStatus::Away { .. }) => return Err(VoyageError::Away.to_string()),
        _ => return Err(VoyageError::Planned.to_string()),
    }
    if picks.times.iter().all(|&t| t == 0) {
        return Err(format!("Can't send it: {}", VoyageError::NoDeals));
    }
    picks.confirm = true;
    Ok(())
}

/// The title of the confirm box.
pub const CONFIRM_TITLE: &str = "SEND THE AIRSHIP";

/// How wide the confirm box is, in text cells, at most (the border and its padding come to four
/// more).
pub const CONFIRM_WIDTH: usize = 68;

/// "two" for 2, the digits from 10 up.
fn number_word(n: u32) -> String {
    const WORDS: [&str; 10] =
        ["no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"];
    WORDS.get(n as usize).map_or_else(|| n.to_string(), |w| (*w).to_owned())
}

/// The confirm box's lines: the question, a blank row, then what follows from it.
pub fn confirm_lines(session: &Session, picks: &Picks) -> Vec<String> {
    let defs = &session.defs;
    let caravan = caravans(session).get(picks.nation);
    let to = caravan.map_or_else(String::new, |c| defs.nation(c.nation).name.clone());
    let picked: Vec<(usize, u8)> = picks.times.iter().copied().enumerate().collect();
    let c = caravan.map(|c| cargo(session, c, &picked));
    let (out, back) =
        c.map_or((String::new(), String::new()), |c| (counted(&c.out), counted(&c.back)));
    let def = def_of(session, picks.dock);
    let crew = def.map_or(0, |d| u32::from(d.crew));
    let away = def.map_or(0, |d| minutes(d.away_ticks));
    vec![
        format!("Load {out} and sail to the {to}?"),
        String::new(),
        format!(
            "Haulers bring the goods to the dock. Once it is loaded, {} resident{} board and {} \
             away for about {away} minutes. It comes back with {back}. Until it sails you can \
             cancel, and the goods stay at the dock.",
            number_word(crew),
            if crew == 1 { "" } else { "s" },
            if crew == 1 { "is" } else { "are" },
        ),
    ]
}

/// What the box's two answers are called.
pub fn confirm_buttons() -> (&'static str, &'static str) {
    ("Load and sail [Enter]", "Back [Esc]")
}

/// `Enter` in the confirm box: plans the voyage. The outcome as a status line either way.
pub fn send(session: &mut Session, picks: &mut Picks) -> Result<String, String> {
    picks.confirm = false;
    let Some(caravan) = caravans(session).get(picks.nation) else {
        return Err(format!("Can't send it: {}", VoyageError::NoCaravan));
    };
    let nation = caravan.nation;
    let deals = picked_deals(picks);
    let to = session.defs.nation(nation).name.clone();
    match session.apply(Command::PlanVoyage { dock: picks.dock, nation, deals }) {
        Ok(()) => Ok(format!("Loading the airship for the {to}")),
        Err(e) => Err(format!("Can't send it: {e}")),
    }
}

/// `X`: cancels the voyage being loaded. The outcome as a status line either way.
pub fn cancel(session: &mut Session, dock: Pos) -> Result<String, String> {
    match session.apply(Command::CancelVoyage { dock }) {
        Ok(()) => Ok("Voyage cancelled: the goods wait at the dock".to_owned()),
        Err(e) => Err(format!("Can't cancel: {e}")),
    }
}

#[cfg(test)]
mod tests;

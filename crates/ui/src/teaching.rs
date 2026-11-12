//! The LOST TEACHING box (#743, docs/design/horizons-phase-5.md, "Mockups"): the teaching draft
//! as a #655-style centred box. Its first row, "Decide later", is the safe answer, where the
//! cursor starts and what `Esc` picks; then one `Keep …` row per teaching on offer, with its
//! unlock line and the sim's reason line under it ([`fn_sim::world::TeachingCaveat`]). `Keep`
//! asks no second confirm: the box already says the rest is lost.
//!
//! It opens by itself and pauses the clock, once per offer (owner #733), whenever nothing else is
//! open (`App::settle`). With several offers pending it shows the oldest, titled `LOST TEACHING
//! 1 OF 2`. "Decide later" closes it; the offer waits, the alert `A teaching waits for your
//! choice [T]` stays open, and the Research screen's TEACHINGS column has a `! Choose a
//! teaching` row that reopens it.

use fn_sim::Defs;
use fn_sim::Pos;
use fn_sim::jobs::{HarvestDef, Stack};
use fn_sim::research::TechId;
use fn_sim::world::{Command, TeachingCaveat};

use crate::Session;
use crate::research::{cost_text, unlock_summary};

/// What a row of the box does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Choice {
    /// Close the box; the offer waits.
    Later,
    /// Keep this teaching and lose the rest of its offer.
    Keep(TechId),
}

/// One `Keep …` row: the teaching, its cost, and the two lines under it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct KeepRow {
    pub tech: TechId,
    /// "Keep Guru Teachings".
    pub label: String,
    /// "250 pts + scroll".
    pub cost: String,
    /// "Unlocks Master chi x0.75".
    pub unlock: String,
    /// The sim's reason it might not pay yet ("Needs: Known on the road", "No master yet."),
    /// if any.
    pub reason: Option<String>,
}

/// The box for the oldest pending offer.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Draft {
    /// "LOST TEACHING", or "LOST TEACHING 1 OF 2" with several offers pending.
    pub title: String,
    /// Where the scroll was read, and what keeping means.
    pub lines: Vec<String>,
    pub keeps: Vec<KeepRow>,
}

impl Draft {
    /// The rows top to bottom: "Decide later", then each teaching.
    pub fn choices(&self) -> Vec<Choice> {
        let mut out = vec![Choice::Later];
        out.extend(self.keeps.iter().map(|k| Choice::Keep(k.tech)));
        out
    }

    /// How many rows the cursor walks: "Decide later" and one per teaching.
    pub fn rows(&self) -> usize {
        1 + self.keeps.len()
    }
}

/// The study hall's own name for the building anchored at `hall` ("Meditation Pavilion"), or
/// "study hall" once it is gone.
pub fn hall_name(session: &Session, hall: Pos) -> String {
    let (world, defs) = (&session.world, &session.defs);
    world
        .building_at(hall)
        .map(|id| defs.building(id).display_name(world.nation).to_owned())
        .unwrap_or_else(|| "study hall".to_owned())
}

/// The reason line for a caveat: "Needs: Meditation, Known on the road" or "No master yet.".
pub fn describe_caveat(defs: &Defs, caveat: &TeachingCaveat) -> String {
    match caveat {
        TeachingCaveat::Needs { prereqs, conditions } => {
            let mut needs: Vec<String> =
                prereqs.iter().map(|&p| defs.tech(p).name.clone()).collect();
            needs.extend(conditions.iter().map(ToString::to_string));
            format!("Needs: {}", needs.join(", "))
        }
        TeachingCaveat::NoMaster => "No master yet.".to_owned(),
    }
}

/// A teaching's cost as the box and the detail strip read it: "250 pts + scroll".
pub fn teaching_cost(defs: &Defs, tech: TechId) -> String {
    let def = defs.tech(tech);
    let items = crate::research::cost_items_text(defs, def);
    let items = if items.is_empty() { "scroll".to_owned() } else { format!("{items}, scroll") };
    cost_text(def.cost, &items)
}

/// The box for the oldest pending offer, or `None` with none pending.
pub fn draft(session: &Session) -> Option<Draft> {
    let (world, defs) = (&session.world, &session.defs);
    let offers = world.teaching_offers();
    let offer = offers.first()?;
    let title = if offers.len() > 1 {
        format!("LOST TEACHING 1 OF {}", offers.len())
    } else {
        "LOST TEACHING".to_owned()
    };
    let read = format!("A teaching scroll was read at the {}.", hall_name(session, offer.hall));
    let keep = if offer.techs.len() > 1 {
        "Keep one teaching. The other is lost for this game.".to_owned()
    } else {
        "Keep it now, or decide later.".to_owned()
    };
    let keeps = offer
        .techs
        .iter()
        .map(|&tech| {
            let def = defs.tech(tech);
            KeepRow {
                tech,
                label: format!("Keep {}", def.name),
                cost: teaching_cost(defs, tech),
                unlock: format!("Unlocks {}", unlock_summary(defs, world.nation, tech, def)),
                reason: world.teaching_caveat(tech, defs).map(|c| describe_caveat(defs, &c)),
            }
        })
        .collect();
    Some(Draft { title, lines: vec![read, keep], keeps })
}

/// Takes `choice`: keeps a teaching (`Command::KeepTeaching`), or does nothing for "Decide
/// later". Returns the notice for the status line.
pub fn take(session: &mut Session, choice: Choice) -> Result<String, String> {
    match choice {
        Choice::Later => Ok(String::new()),
        Choice::Keep(tech) => {
            let name = session.defs.tech(tech).name.clone();
            session
                .apply(Command::KeepTeaching { tech })
                .map(|()| format!("Kept {name}"))
                .map_err(|e| format!("Can't keep {name}: {e}"))
        }
    }
}

/// "a teaching scroll", "2 ancient scrolls": a stack as a phrase.
fn stack_phrase(defs: &Defs, stack: &Stack) -> String {
    let name = defs.item(stack.item).name.to_lowercase();
    match stack.count {
        1 if name.starts_with(['a', 'e', 'i', 'o', 'u']) => format!("an {name}"),
        1 => format!("a {name}"),
        n => format!("{n} {name}s"),
    }
}

/// What a harvest with extra drops yields, as a phrase: "2 ancient scrolls and a teaching
/// scroll" (a great ruin's search, #736). `count` is the main yield.
pub fn yield_phrase(defs: &Defs, h: &HarvestDef, count: u32) -> String {
    let mut parts = vec![stack_phrase(defs, &Stack { item: h.item, count })];
    parts.extend(h.also.iter().map(|s| stack_phrase(defs, s)));
    match parts.len() {
        1 => parts.remove(0),
        _ => {
            let last = parts.pop().unwrap_or_default();
            format!("{} and {last}", parts.join(", "))
        }
    }
}

#[cfg(test)]
mod tests;

//! The event log (#605): everything the world reports as having happened, in one ordered list.
//!
//! Bends, tech completions, deaths, endeavor events, arrivals, season events, plants picked to
//! death, harm and finished charts used to be nine separate append-only vectors, and every
//! reader (the alert log, the agent surface's referee, the balance harness) kept a cursor into
//! each. Now they are one `Vec<Event>` in the order they were logged, so its ticks never go
//! down, and a reader keeps one cursor. A new kind of event is a new `Event` variant, and every
//! exhaustive `match` over it (`fn_ui::alerts` has one) fails to compile until it has an arm.
//!
//! The typed accessors (`bend_events()`, `deaths()`, ...) are filters over the log, oldest
//! first. The log is never trimmed; capping it is out of scope for #605.
//!
//! # Saves (version 4)
//!
//! The body writes the log as `events`. Saves up to version 3 kept the nine vectors (one of them,
//! the tech completions, inside `Research` as `log`, and the endeavor's inside `endeavor_log`).
//! `World` reads them into deserialize-only fields named for them, and `upgrade_from_v3` merges
//! them into the log by tick. Events on the same tick are ordered by [`Event::order`], the order
//! `fn_ui::Alerts` used to read the nine vectors in, and each vector keeps its own order, so a
//! loaded save's alert log reads exactly as it did before.

use serde::{Deserialize, Serialize};

use crate::research::TechCompleted;

use super::*;

/// One thing the world logged. Every variant carries the tick it happened on ([`Event::tick`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// A bend finished or failed.
    Bend(BendEvent),
    /// A tech finished. Starting techs are not logged.
    Tech(TechCompleted),
    /// A unit died.
    Death(Death),
    /// An endeavor sign was met, a stage finished, the Avatar came or went (`endeavor.rs`).
    Endeavor(EndeavorEvent),
    /// A newcomer joined the settlement (`population.rs`, #232).
    Arrival(Arrival),
    /// A season changed, the winter warning, or frost (`season.rs`, #295).
    Season(SeasonEvent),
    /// A wild plant was picked to death (`strain.rs`, #296).
    Strain(StrainEvent),
    /// A unit lay down to sleep rough in the cold, or was hurt (`harm.rs`, #387; `injury.rs`).
    Harm(HarmEvent),
    /// A Map Room finished a chart (`chart.rs`, #402).
    Chart(ChartEvent),
    /// A caravan arrived, was skipped, closed a deal or a request, or left (`trade.rs`, #437).
    Caravan(CaravanEvent),
}

impl Event {
    /// The world tick it happened on.
    pub fn tick(&self) -> u64 {
        match self {
            Self::Bend(e) => e.tick,
            Self::Tech(e) => e.tick,
            Self::Death(e) => e.tick,
            Self::Endeavor(e) => e.tick,
            Self::Arrival(e) => e.tick,
            Self::Season(e) => e.tick,
            Self::Strain(e) => e.tick,
            Self::Harm(e) => e.tick,
            Self::Chart(e) => e.tick,
            Self::Caravan(e) => e.tick,
        }
    }

    /// Where an event sorts among others on the same tick, lowest first: the order the alert
    /// log read the nine pre-#605 vectors in (bends, techs, deaths, endeavor, arrivals, seasons,
    /// strain), then harm and charts, which it didn't read. `upgrade_from_v3` merges old saves
    /// by `(tick, order)`, and `fn_ui::Alerts` sorts each batch of new lines the same way, so
    /// same-tick lines keep the order they always had, whatever order the sim logged them in.
    pub fn order(&self) -> u8 {
        match self {
            Self::Bend(_) => 0,
            Self::Tech(_) => 1,
            Self::Death(_) => 2,
            Self::Endeavor(_) => 3,
            Self::Arrival(_) => 4,
            Self::Season(_) => 5,
            Self::Strain(_) => 6,
            Self::Harm(_) => 7,
            Self::Chart(_) => 8,
            Self::Caravan(_) => 9,
        }
    }
}

macro_rules! event_from {
    ($($variant:ident($ty:ty)),* $(,)?) => {
        $(impl From<$ty> for Event {
            fn from(e: $ty) -> Self {
                Self::$variant(e)
            }
        })*
    };
}

event_from!(
    Bend(BendEvent),
    Tech(TechCompleted),
    Death(Death),
    Endeavor(EndeavorEvent),
    Arrival(Arrival),
    Season(SeasonEvent),
    Strain(StrainEvent),
    Harm(HarmEvent),
    Chart(ChartEvent),
    Caravan(CaravanEvent),
);

/// A typed filter over the log: `$name()` yields every `Event::$variant`, oldest first.
macro_rules! event_filters {
    ($($(#[$doc:meta])* $name:ident => $variant:ident($ty:ty);)*) => {
        impl World {
            $(
                $(#[$doc])*
                pub fn $name(&self) -> impl DoubleEndedIterator<Item = &$ty> + Clone + '_ {
                    self.events.iter().filter_map(|e| match e {
                        Event::$variant(e) => Some(e),
                        _ => None,
                    })
                }
            )*
        }
    };
}

event_filters! {
    /// Every bend that has finished or failed, oldest first.
    bend_events => Bend(BendEvent);
    /// Every tech completion so far, oldest first. Starting techs are not logged.
    tech_completions => Tech(TechCompleted);
    /// Units that have died, oldest first.
    deaths => Death(Death);
    /// Every endeavor event so far, oldest first.
    endeavor_events => Endeavor(EndeavorEvent);
    /// Every newcomer who has joined so far, oldest first.
    arrivals => Arrival(Arrival);
    /// Season changes, the winter warning and frost, oldest first.
    season_events => Season(SeasonEvent);
    /// Wild plants killed by over-picking so far, oldest first.
    strain_events => Strain(StrainEvent);
    /// Every harm event so far, oldest first.
    harm_events => Harm(HarmEvent);
    /// Every finished chart so far, oldest first.
    chart_events => Chart(ChartEvent);
    /// Every caravan event so far, oldest first.
    caravan_events => Caravan(CaravanEvent);
}

impl World {
    /// Everything the world has logged, oldest first. Ticks never go down; events on the same
    /// tick are in the order they happened, not in [`Event::order`].
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The events logged on the current tick, in the order they happened: a cheap way to look
    /// at what one `tick` did without a cursor.
    pub fn events_this_tick(&self) -> &[Event] {
        let start = self.events.partition_point(|e| e.tick() < self.tick);
        &self.events[start..]
    }

    /// Appends `event` to the log. It must have happened on the current tick.
    pub(super) fn log_event(&mut self, event: impl Into<Event>) {
        let event = event.into();
        debug_assert_eq!(event.tick(), self.tick, "events are logged as they happen");
        self.events.push(event);
    }

    /// Puts `event` into the log at its tick, after every event already on that tick, for tests
    /// that need something to have happened in the past.
    #[cfg(test)]
    pub(crate) fn insert_event_for_test(&mut self, event: impl Into<Event>) {
        let event = event.into();
        let at = self.events.partition_point(|e| e.tick() <= event.tick());
        self.events.insert(at, event);
    }

    /// Whether any of a pre-v4 save's event vectors is still waiting for `upgrade_from_v3`.
    pub(super) fn has_v3_logs(&self) -> bool {
        !(self.v3_bend_events.is_empty()
            && self.v3_deaths.is_empty()
            && self.v3_arrivals.is_empty()
            && self.v3_season_events.is_empty()
            && self.v3_strain_events.is_empty()
            && self.v3_harm_events.is_empty()
            && self.v3_chart_events.is_empty())
            || self.research.has_v3_log()
            || self.endeavor_log.has_v3_events()
    }

    /// Merges a save's pre-v4 event vectors into the log (`save.rs`, version 4): by tick, then
    /// by [`Event::order`], each vector keeping its own order. The merged events go after any
    /// the body already had (none, in a real v1 to v3 save).
    pub(crate) fn upgrade_from_v3(&mut self) {
        fn drain<T: Into<Event>>(v: &mut Vec<T>) -> impl Iterator<Item = Event> + '_ {
            v.drain(..).map(Into::into)
        }
        let mut merged: Vec<Event> = Vec::new();
        merged.extend(drain(&mut self.v3_bend_events));
        merged.extend(self.research.take_v3_log().into_iter().map(Event::from));
        merged.extend(drain(&mut self.v3_deaths));
        merged.extend(self.endeavor_log.take_v3_events().into_iter().map(Event::from));
        merged.extend(drain(&mut self.v3_arrivals));
        merged.extend(drain(&mut self.v3_season_events));
        merged.extend(drain(&mut self.v3_strain_events));
        merged.extend(drain(&mut self.v3_harm_events));
        merged.extend(drain(&mut self.v3_chart_events));
        // Stable, and the vectors went in by `order`, so this is (tick, order, index).
        merged.sort_by_key(|e| (e.tick(), e.order()));
        self.events.extend(merged);
    }
}

#[cfg(test)]
mod tests;

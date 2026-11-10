//! The timeline's markers: the turns where something worth finding again happened -- a death,
//! an endeavor milestone, the Avatar coming or going, something learned, a newcomer -- so the
//! page can mark them on the scrub bar.
//!
//! The log holds only keys and turns, not what they led to, so the only way to know where the
//! deaths are is to replay the whole run. [`Scan`] does that with a watcher of its own, a turn
//! per [`Scan::step`], reading the world's event log (`fn_sim::world::Event`) as it goes. The page
//! runs it in a Web Worker on a second instance of the same wasm, so it never slows the replay
//! being watched; markers appear on the bar as the scan reaches them.

use fn_sim::Defs;
use fn_sim::world::{EndeavorEventKind, Event};
use fn_tui::watch::Watcher;
use fn_ui::alerts::{describe_arrival, describe_death};
use fn_ui::endeavor::describe_event;

/// What a marker is about. The page colours and shapes each kind differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MarkerKind {
    /// A unit died.
    Death,
    /// The endeavor moved on: a sign met, the endeavor Ready, a stage done, the gate complete.
    Milestone,
    /// The Avatar arrived, left, came back or said farewell, or the Harmony Rite paused or broke.
    Avatar,
    /// A tech finished, or a bender reached a new rank.
    Learned,
    /// A newcomer joined.
    Arrival,
}

impl MarkerKind {
    /// The word the page reads it by.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Death => "death",
            Self::Milestone => "milestone",
            Self::Avatar => "avatar",
            Self::Learned => "learned",
            Self::Arrival => "arrival",
        }
    }
}

/// One event on the timeline: what happened, and the turn whose end shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub turn: u32,
    pub kind: MarkerKind,
    /// The alert log's own line for it ("Mei died of starvation", "Sign met: Population").
    pub text: String,
}

/// The marker one world event makes, if it is one worth marking. Bends, seasons, picked-out
/// plants, harm and charts are left off: they are routine or frequent enough to bury the rest.
pub fn marker_for(defs: &Defs, ev: &Event) -> Option<(MarkerKind, String)> {
    Some(match ev {
        Event::Death(d) => (MarkerKind::Death, describe_death(d)),
        Event::Endeavor(e) => {
            let kind = match e.kind {
                EndeavorEventKind::SignMet(_)
                | EndeavorEventKind::Ready
                | EndeavorEventKind::StageDone(_)
                | EndeavorEventKind::Complete => MarkerKind::Milestone,
                EndeavorEventKind::AvatarArrived
                | EndeavorEventKind::AvatarLeft(_)
                | EndeavorEventKind::AvatarReturned
                | EndeavorEventKind::AvatarFarewell
                | EndeavorEventKind::RitePaused
                | EndeavorEventKind::RiteBroken => MarkerKind::Avatar,
                EndeavorEventKind::RankUp { .. } => MarkerKind::Learned,
            };
            (kind, describe_event(defs, e))
        }
        Event::Tech(t) => (MarkerKind::Learned, format!("Learned {}", defs.tech(t.tech).name)),
        Event::Arrival(a) => (MarkerKind::Arrival, describe_arrival(a)),
        Event::Bend(_) | Event::Season(_) | Event::Strain(_) | Event::Harm(_) | Event::Chart(_) => {
            return None;
        }
    })
}

/// A replay of a whole log, run only to collect its [`Marker`]s.
pub struct Scan {
    watcher: Watcher,
    /// How many of the world's events have been read.
    seen: usize,
    markers: Vec<Marker>,
}

impl Scan {
    /// A scan of the whole log `text`, at its start. Refuses what [`crate::Replay::new`] refuses.
    pub fn new(text: &str) -> Result<Self, String> {
        let watcher = Watcher::from_text(text)?;
        let mut scan = Self { watcher, seen: 0, markers: Vec::new() };
        scan.collect();
        Ok(scan)
    }

    /// Replays one more turn and collects what it brought. `true` while there's more to scan.
    pub fn step(&mut self) -> bool {
        if self.done() {
            return false;
        }
        self.watcher.fast_forward_to_turn(self.watcher.turn() + 1);
        self.collect();
        !self.done()
    }

    /// The whole log has been scanned.
    pub fn done(&self) -> bool {
        self.watcher.caught_up()
    }

    /// How far the scan has got.
    pub fn turn(&self) -> u32 {
        self.watcher.turn()
    }

    /// Every marker found so far, in the order things happened.
    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }

    fn collect(&mut self) {
        let session = self.watcher.session();
        let events = session.world.events();
        let turn = self.watcher.turn();
        for ev in &events[self.seen..] {
            if let Some((kind, text)) = marker_for(&session.defs, ev) {
                self.markers.push(Marker { turn, kind, text });
            }
        }
        self.seen = events.len();
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::needs::DeathCause;
    use fn_sim::world::{Death, EndeavorEvent, UnitId};

    use super::*;

    #[test]
    fn deaths_and_milestones_are_marked_and_routine_events_are_not() {
        let defs = Defs::bundled();
        let death = Event::Death(Death {
            id: UnitId(1),
            name: "Mei".into(),
            tick: 5,
            cause: DeathCause::Starvation,
            weakened_by: None,
        });
        assert_eq!(
            marker_for(&defs, &death),
            Some((MarkerKind::Death, "Mei died of starvation".into()))
        );
        let sign = &defs.endeavor.signs[0];
        let met = Event::Endeavor(EndeavorEvent {
            tick: 9,
            kind: EndeavorEventKind::SignMet(sign.id.clone()),
        });
        assert_eq!(
            marker_for(&defs, &met),
            Some((MarkerKind::Milestone, format!("Sign met: {}", sign.name)))
        );
        let broken =
            Event::Endeavor(EndeavorEvent { tick: 9, kind: EndeavorEventKind::RiteBroken });
        assert_eq!(marker_for(&defs, &broken).map(|m| m.0), Some(MarkerKind::Avatar));
    }
}

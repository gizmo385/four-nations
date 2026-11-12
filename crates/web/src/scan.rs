//! The timeline's markers: the turns where something worth finding again happened -- a death,
//! an endeavor milestone, the Avatar coming or going, something learned, a newcomer, a new
//! revision of the agent's script -- so the page can mark them on the scrub bar.
//!
//! The log holds only keys and turns, not what they led to, so the only way to know where the
//! deaths are is to replay the whole run. [`Scan`] does that with a watcher of its own, a turn
//! per [`Scan::step`], reading the world's event log (`fn_sim::world::Event`) as it goes. The page
//! runs it in a Web Worker on a second instance of the same wasm, so it never slows the replay
//! being watched; markers appear on the bar as the scan reaches them.

use std::collections::{HashMap, VecDeque};

use fn_sim::Defs;
use fn_sim::world::{EndeavorEventKind, Event};
use fn_tui::surface::LogLine;
use fn_tui::watch::{self, Watcher};
use fn_ui::alerts::{describe_arrival, describe_death};
use fn_ui::code::line_diff;
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
    /// The agent saved a new revision of a script, or ran new ad hoc code (a log line, not a world
    /// event: see [`script_changes`]).
    Script,
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
            Self::Script => "script",
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
/// plants, harm, charts and caravans are left off: they are routine or frequent enough to bury the rest.
pub fn marker_for(defs: &Defs, ev: &Event) -> Option<(MarkerKind, String)> {
    Some(match ev {
        Event::Death(d) => (MarkerKind::Death, describe_death(d)),
        Event::Endeavor(e) => {
            let kind = match e.kind {
                EndeavorEventKind::SignMet(_)
                | EndeavorEventKind::ChapterMet(_)
                | EndeavorEventKind::Ready
                | EndeavorEventKind::StageDone(_)
                | EndeavorEventKind::PopulationMilestone(_)
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
        // Caravans (#437) have no alert line yet, so no marker either.
        Event::Bend(_)
        | Event::Season(_)
        | Event::Strain(_)
        | Event::Harm(_)
        | Event::Chart(_)
        | Event::Caravan(_)
        | Event::Beast(_)
        | Event::Find(_)
        | Event::Teaching(_) => {
            return None;
        }
    })
}

/// The script changes in `lines`, as markers: each `Script` line that brings a new revision (a
/// saved script's new `version`, or ad hoc code not run before under its tag; the same rule as
/// the studio's code pane, `fn_tui::studio`), with its line diff against the name's previous
/// revision. Each is marked at the end of the turn after it, the first turn whose end has it
/// (clamped to the log's last turn, for one saved after the last turn).
pub fn script_changes(lines: &[LogLine]) -> Vec<Marker> {
    let total = lines.iter().filter(|l| matches!(l, LogLine::Turn { .. })).count() as u32;
    let mut turns = 0;
    // Per name: the versions and ad hoc codes seen, and the latest revision's code.
    let mut seen: HashMap<&str, (Vec<u32>, Vec<&str>, &str)> = HashMap::new();
    let mut markers = Vec::new();
    for line in lines {
        match line {
            LogLine::Turn { .. } => turns += 1,
            LogLine::Script { name, version, code, .. } => {
                let (versions, codes, last) = seen.entry(name).or_default();
                let first = versions.is_empty() && codes.is_empty();
                let is_new = if *version > 0 {
                    !versions.contains(version)
                } else {
                    !codes.contains(&code.as_str())
                };
                if !is_new {
                    continue;
                }
                let change = if first {
                    let n = code.lines().count();
                    format!("new, {n} line{}", if n == 1 { "" } else { "s" })
                } else {
                    let (_, added, deleted) = line_diff(last, code);
                    format!("+{added} \u{2212}{deleted}")
                };
                let text = if *version > 0 {
                    versions.push(*version);
                    format!("Saved {name} v{version} ({change})")
                } else {
                    codes.push(code);
                    format!("Ran new {name} code ({change})")
                };
                *last = code;
                let turn = (turns + 1).min(total);
                markers.push(Marker { turn, kind: MarkerKind::Script, text });
            }
            _ => {}
        }
    }
    markers
}

/// A replay of a whole log, run only to collect its [`Marker`]s.
pub struct Scan {
    watcher: Watcher,
    /// How many of the world's events have been read.
    seen: usize,
    /// [`script_changes`]' markers the scan hasn't reached yet, in turn order: they come from the
    /// log, not the world, but join `markers` as the scan passes their turn, so it stays in the
    /// order things happened.
    scripts: VecDeque<Marker>,
    markers: Vec<Marker>,
}

impl Scan {
    /// A scan of the whole log `text`, at its start. Refuses what [`crate::Replay::new`] refuses.
    pub fn new(text: &str) -> Result<Self, String> {
        let (header, lines) = watch::parse(&format!("{text}\n"))?;
        let scripts = script_changes(&lines).into();
        let mut watcher = Watcher::new(&header)?;
        watcher.feed(lines);
        let mut scan = Self { watcher, seen: 0, scripts, markers: Vec::new() };
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
        while self.scripts.front().is_some_and(|m| m.turn <= turn) {
            self.markers.extend(self.scripts.pop_front());
        }
        for ev in &events[self.seen..] {
            if let Some((kind, text)) = marker_for(&session.defs, ev) {
                self.markers.push(Marker { turn, kind, text });
            }
        }
        self.seen = events.len();
        if self.done() {
            // Anything left (a log that stops short of its own turn count) goes at the end.
            self.markers.extend(self.scripts.drain(..));
        }
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::needs::DeathCause;
    use fn_sim::world::{Death, EndeavorEvent, UnitId};

    use super::*;

    fn script(name: &str, version: u32, code: &str, turns: u32) -> LogLine {
        LogLine::Script {
            name: name.into(),
            version,
            code: code.into(),
            turns,
            wake_on: Vec::new(),
        }
    }

    fn turn(index: u32) -> LogLine {
        LogLine::Turn { index, tick: 0 }
    }

    #[test]
    fn each_new_script_revision_is_marked_once_at_the_turn_after_it() {
        let lines = [
            script("governor", 6, "a();", 0),
            script("governor", 6, "a();", 30), // running a saved version again: nothing new
            turn(0),
            script("scout", 0, "look();", 5),
            turn(1),
            script("governor", 7, "a();\nb();", 0),
            script("scout", 0, "look();", 5), // the same ad hoc code again
            turn(2),
            script("governor", 8, "b();", 0), // saved after the last turn
        ];
        let got: Vec<_> = script_changes(&lines).into_iter().map(|m| (m.turn, m.text)).collect();
        assert_eq!(
            got,
            [
                (1, "Saved governor v6 (new, 1 line)".to_owned()),
                (2, "Ran new scout code (new, 1 line)".to_owned()),
                (3, "Saved governor v7 (+1 \u{2212}0)".to_owned()),
                (3, "Saved governor v8 (+0 \u{2212}1)".to_owned()),
            ]
        );
    }

    #[test]
    fn deaths_and_milestones_are_marked_and_routine_events_are_not() {
        let defs = Defs::bundled();
        let death = Event::Death(Death {
            id: UnitId(1),
            name: "Mei".into(),
            tick: 5,
            cause: DeathCause::Starvation,
            weakened_by: None,
            pos: None,
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

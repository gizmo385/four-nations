//! The alert log: bend results (#72), tech completions (#23) and deaths as one stream of lines.
//!
//! Every line is kept in a bounded history (the terminal UI's message log). The newest ones also
//! show as short-lived *toasts* in the side panel's Alerts section (ui.md: "a section with
//! nothing to show hides itself"). A toast ages by the `dt` the caller passes to
//! [`Alerts::update`], in real seconds, so this crate never reads a clock.

use fn_sim::needs::DeathCause;
use fn_sim::world::{BendEvent, BendOutcome};

use crate::session::Session;

/// How many lines of history to keep; the oldest drop first.
pub const MAX_HISTORY: usize = 1000;

/// What an alert is about. Decides how long its toast stays and how many show at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A bend that finished or failed.
    Bend,
    /// A finished tech.
    Tech,
    /// A unit that died.
    Death,
}

impl Kind {
    /// How long the toast stays, in real seconds. A kind with no toast is history only: the
    /// Bevy client shows no death alerts yet.
    pub fn toast_seconds(self) -> f32 {
        match self {
            Self::Bend => 6.0,
            Self::Tech => 8.0,
            Self::Death => 0.0,
        }
    }

    /// At most this many toasts of the kind show at once; the oldest drops first.
    pub fn max_toasts(self) -> usize {
        match self {
            Self::Bend => 4,
            Self::Tech => 3,
            Self::Death => 0,
        }
    }
}

/// One line of the log.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The world tick it happened on.
    pub tick: u64,
    pub kind: Kind,
    pub text: String,
    /// Seconds its toast has left; at or below zero it is history only.
    left: f32,
}

/// The alert log. Feed it every frame with [`Alerts::update`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Alerts {
    /// Oldest first.
    log: Vec<Entry>,
    /// How many of `World::bend_events`, `Research::completions` and `World::deaths` have
    /// already become a line.
    seen: (usize, usize, usize),
    /// The tick of the last update, to notice a new world.
    last_tick: u64,
}

impl Alerts {
    /// Turns what the world logged since the last call into lines, then ages every toast by `dt`
    /// seconds.
    pub fn update(&mut self, session: &Session, dt: f32) {
        let world = &session.world;
        let events = world.bend_events();
        let completions = world.research().completions();
        let deaths = &world.deaths;
        let counts = (events.len(), completions.len(), deaths.len());
        if world.tick < self.last_tick
            || counts.0 < self.seen.0
            || counts.1 < self.seen.1
            || counts.2 < self.seen.2
        {
            // A new world (regenerate, nation switch, a loaded game): the old log is gone.
            *self = Self::default();
        }
        self.last_tick = world.tick;

        let mut fresh = Vec::new();
        for ev in &events[self.seen.0..] {
            fresh.push((ev.tick, Kind::Bend, describe_bend_event(session, ev)));
        }
        for c in &completions[self.seen.1..] {
            fresh.push((c.tick, Kind::Tech, format!("Learned {}", session.defs.tech(c.tech).name)));
        }
        for d in &deaths[self.seen.2..] {
            let how = match d.cause {
                DeathCause::Starvation => "starvation",
                DeathCause::Dehydration => "dehydration",
                DeathCause::Exhaustion => "exhaustion",
            };
            fresh.push((d.tick, Kind::Death, format!("{} died of {how}", d.name)));
        }
        self.seen = counts;
        fresh.sort_by_key(|(tick, ..)| *tick);
        self.log.extend(fresh.into_iter().map(|(tick, kind, text)| Entry {
            tick,
            kind,
            text,
            left: kind.toast_seconds(),
        }));
        for e in &mut self.log {
            e.left -= dt;
        }
        if self.log.len() > MAX_HISTORY {
            self.log.drain(0..self.log.len() - MAX_HISTORY);
        }
    }

    /// Every line since the world began, oldest first (bounded by [`MAX_HISTORY`]).
    pub fn history(&self) -> &[Entry] {
        &self.log
    }

    /// The toasts still showing, oldest first: those with time left, at most `Kind::max_toasts`
    /// of each kind (the newest).
    pub fn toasts(&self) -> Vec<&Entry> {
        let live = |e: &&Entry| e.left > 0.0;
        let mut keep = vec![false; self.log.len()];
        for kind in [Kind::Bend, Kind::Tech, Kind::Death] {
            let of_kind: Vec<usize> = (0..self.log.len())
                .filter(|&i| self.log[i].kind == kind && live(&&self.log[i]))
                .collect();
            let skip = of_kind.len().saturating_sub(kind.max_toasts());
            for &i in &of_kind[skip..] {
                keep[i] = true;
            }
        }
        self.log.iter().zip(keep).filter_map(|(e, k)| k.then_some(e)).collect()
    }

    /// The toasts' text, oldest first, for the side panel.
    pub fn toast_lines(&self) -> Vec<String> {
        self.toasts().into_iter().map(|e| e.text.clone()).collect()
    }
}

/// "Haru used Raise Earth at (5, 3)" or "Haru's Raise Earth failed: needs dry ground beside it".
pub fn describe_bend_event(session: &Session, ev: &BendEvent) -> String {
    let def = session.defs.ability(ev.ability);
    let who = session.world.unit(ev.unit).map_or("Someone", |u| u.name.as_str());
    match ev.outcome {
        BendOutcome::Done => format!("{who} used {} at ({}, {})", def.name, ev.pos.x, ev.pos.y),
        BendOutcome::Failed(why) => format!("{who}'s {} failed: {why}", def.name),
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::bending::AbilityId;
    use fn_sim::world::Command;
    use fn_sim::{Nation, Pos};

    use super::*;
    use crate::tools::ability_for_nation;

    fn run_until(s: &mut Session, mut done: impl FnMut(&Session) -> bool) {
        for _ in 0..20_000 {
            if done(s) {
                return;
            }
            s.tick();
        }
        panic!("never happened");
    }

    #[test]
    fn describes_a_done_and_a_failed_bend() {
        let s = Session::new(1, Nation::Earth, 48, 32);
        let ability = ability_for_nation(&s.defs, Nation::Earth).unwrap();
        let unit = s.world.units[0].id;
        let pos = Pos::new(5, 3);
        let done =
            BendEvent { tick: 0, unit, ability, pos, outcome: BendOutcome::Done, moved: None };
        let line = describe_bend_event(&s, &done);
        assert!(line.contains(&s.world.units[0].name), "{line}");
        assert!(line.contains("(5, 3)"), "{line}");

        let failed =
            BendEvent { outcome: BendOutcome::Failed("needs dry ground beside it"), ..done };
        let line = describe_bend_event(&s, &failed);
        assert!(line.contains("failed: needs dry ground beside it"), "{line}");
    }

    #[test]
    fn a_designated_bend_eventually_becomes_an_alert() {
        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let ability: AbilityId = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        let pos = s.world.units[0].pos;
        s.apply(Command::Bend { ability, pos }).unwrap();
        run_until(&mut s, |s| !s.world.bend_events().is_empty());
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.016);
        let lines = alerts.toast_lines();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains(&s.defs.ability(ability).name));
        // Updating again doesn't repeat it.
        alerts.update(&s, 0.016);
        assert_eq!(alerts.history().len(), 1);
    }

    #[test]
    fn a_completed_tech_becomes_a_toast() {
        let mut s = Session::new(7, Nation::Fire, 48, 32);
        let shelter = s.defs.tech_id("shelter").unwrap();
        s.apply(Command::SetResearch { tech: shelter }).unwrap();
        run_until(&mut s, |s| !s.world.research().completions().is_empty());
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.0);
        assert_eq!(alerts.toast_lines(), ["Learned Shelter"]);
        assert_eq!(alerts.history()[0].kind, Kind::Tech);
    }

    #[test]
    fn toasts_age_out_by_the_dt_the_caller_passes_but_the_history_stays() {
        let mut s = Session::new(7, Nation::Fire, 48, 32);
        let shelter = s.defs.tech_id("shelter").unwrap();
        s.apply(Command::SetResearch { tech: shelter }).unwrap();
        run_until(&mut s, |s| !s.world.research().completions().is_empty());
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.0);
        alerts.update(&s, Kind::Tech.toast_seconds() - 0.5);
        assert_eq!(alerts.toasts().len(), 1, "still showing just before its time is up");
        alerts.update(&s, 0.6);
        assert!(alerts.toasts().is_empty());
        assert_eq!(alerts.history().len(), 1);
    }

    #[test]
    fn only_the_newest_few_of_a_kind_show() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let mut alerts = Alerts::default();
        for i in 0..6 {
            alerts.log.push(Entry {
                tick: i,
                kind: Kind::Bend,
                text: i.to_string(),
                left: Kind::Bend.toast_seconds(),
            });
        }
        alerts.update(&s, 0.5);
        let lines = alerts.toast_lines();
        assert_eq!(lines.len(), Kind::Bend.max_toasts());
        assert_eq!(lines.last().unwrap(), "5", "kept the newest");
        assert_eq!(alerts.history().len(), 6, "capping a toast doesn't drop the history");
        alerts.update(&s, 10.0);
        assert!(alerts.toasts().is_empty(), "everything has expired");
    }

    #[test]
    fn toasts_of_different_kinds_come_out_in_the_order_they_happened() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let mut alerts = Alerts::default();
        for (tick, kind) in [(1, Kind::Tech), (2, Kind::Bend), (3, Kind::Tech)] {
            alerts.log.push(Entry { tick, kind, text: tick.to_string(), left: 5.0 });
        }
        alerts.update(&s, 0.0);
        assert_eq!(alerts.toast_lines(), ["1", "2", "3"]);
    }

    #[test]
    fn a_death_is_logged_but_not_toasted() {
        let mut s = Session::new(7, Nation::Earth, 48, 32);
        let mut alerts = Alerts::default();
        // One settler is starving and nearly dead.
        s.world.units[1].needs.hunger = 0.0;
        s.world.units[1].needs.health = 0.01;
        run_until(&mut s, |s| !s.world.deaths.is_empty());
        alerts.update(&s, 0.0);
        let dead = &s.world.deaths[0];
        let death = alerts.history().iter().find(|e| e.kind == Kind::Death).expect("logged");
        assert!(death.text.starts_with(&format!("{} died of ", dead.name)), "{}", death.text);
        assert_eq!(death.tick, dead.tick);
        assert!(alerts.toasts().iter().all(|e| e.kind != Kind::Death));
    }

    #[test]
    fn a_new_world_clears_the_log() {
        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let ability = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        let pos = s.world.units[0].pos;
        s.apply(Command::Bend { ability, pos }).unwrap();
        run_until(&mut s, |s| !s.world.bend_events().is_empty());
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.0);
        assert_eq!(alerts.history().len(), 1);
        alerts.update(&Session::new(2, Nation::Fire, 48, 32), 0.0);
        assert!(alerts.history().is_empty());
    }
}

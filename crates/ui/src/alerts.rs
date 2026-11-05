//! The alert log: bend results (#72), tech completions (#23), deaths, the end-game endeavor's
//! events (#235) and newcomers joining the settlement (#232) as one stream of lines.
//!
//! Every line is kept in a bounded history (the terminal UI's message log). The newest ones also
//! show as short-lived *toasts* in the side panel's Alerts section (ui.md: "a section with
//! nothing to show hides itself"). A toast ages by the `dt` the caller passes to
//! [`Alerts::update`], in real seconds, so this crate never reads a clock.

use fn_sim::needs::DeathCause;
use fn_sim::world::{Arrival, BendEvent, BendOutcome, SeasonEvent, SeasonEventKind, StrainEvent};

use crate::endeavor;
use crate::session::Session;
use crate::view::food_in_stores;

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
    /// An endeavor sign met for the first time, or the endeavor Ready (#235).
    Endeavor,
    /// A newcomer joined the settlement (#232).
    Arrival,
    /// The winter warning, a season change worth announcing (winter or spring), the first
    /// frost, or (Hard) a wild plant picked to death (#295, #296, #299).
    Season,
    /// An agent's act, while `fn_tui --watch` (#319) is replaying a run log: not something the
    /// world logged, so [`Alerts::push_agent`] adds it directly instead of coming from
    /// [`Alerts::update`]'s own counts.
    Agent,
}

impl Kind {
    /// How long the toast stays, in real seconds. A kind with no toast is history only.
    pub fn toast_seconds(self) -> f32 {
        match self {
            Self::Bend => 6.0,
            Self::Tech => 8.0,
            Self::Death => 0.0,
            Self::Endeavor => 10.0,
            Self::Arrival => 8.0,
            Self::Season => 10.0,
            // The agent strip (#319) already names the current action; the log (`L`) is where
            // this is meant to be read back, not a toast on top of the map.
            Self::Agent => 0.0,
        }
    }

    /// At most this many toasts of the kind show at once; the oldest drops first.
    pub fn max_toasts(self) -> usize {
        match self {
            Self::Bend => 4,
            Self::Tech => 3,
            Self::Death => 0,
            Self::Endeavor => 3,
            Self::Arrival => 3,
            Self::Season => 3,
            Self::Agent => 0,
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
    /// How many of `World::bend_events`, `Research::completions`, `World::deaths`,
    /// `World::endeavor_events`, `World::arrivals`, `World::season_events` and
    /// `World::strain_events` have already become a line.
    seen: (usize, usize, usize, usize, usize, usize, usize),
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
        let endeavor = world.endeavor_events();
        let arrivals = world.arrivals();
        let seasons = world.season_events();
        let strains = world.strain_events();
        let counts = (
            events.len(),
            completions.len(),
            deaths.len(),
            endeavor.len(),
            arrivals.len(),
            seasons.len(),
            strains.len(),
        );
        if world.tick < self.last_tick
            || counts.0 < self.seen.0
            || counts.1 < self.seen.1
            || counts.2 < self.seen.2
            || counts.3 < self.seen.3
            || counts.4 < self.seen.4
            || counts.5 < self.seen.5
            || counts.6 < self.seen.6
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
        for ev in &endeavor[self.seen.3..] {
            fresh.push((ev.tick, Kind::Endeavor, endeavor::describe_event(&session.defs, ev)));
        }
        for a in &arrivals[self.seen.4..] {
            fresh.push((a.tick, Kind::Arrival, describe_arrival(a)));
        }
        for ev in &seasons[self.seen.5..] {
            if let Some(text) = describe_season_event(session, ev) {
                fresh.push((ev.tick, Kind::Season, text));
            }
        }
        for ev in &strains[self.seen.6..] {
            fresh.push((ev.tick, Kind::Season, describe_strain_event(&session.defs, ev)));
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

    /// Adds an `Agent` line directly, for `fn_tui --watch` (#319), which has no world events of
    /// its own to turn into one: every act a run log replays gets a line here too
    /// (docs/design/agent-play.md, "Watching": "The message log gains an Agent entry for every
    /// act"). Appends in call order, so the caller is the one keeping `tick` non-decreasing.
    pub fn push_agent(&mut self, tick: u64, text: String) {
        self.log.push(Entry { tick, kind: Kind::Agent, text, left: Kind::Agent.toast_seconds() });
        if self.log.len() > MAX_HISTORY {
            self.log.drain(0..self.log.len() - MAX_HISTORY);
        }
    }

    /// The toasts still showing, oldest first: those with time left, at most `Kind::max_toasts`
    /// of each kind (the newest).
    pub fn toasts(&self) -> Vec<&Entry> {
        let live = |e: &&Entry| e.left > 0.0;
        let mut keep = vec![false; self.log.len()];
        for kind in [
            Kind::Bend,
            Kind::Tech,
            Kind::Death,
            Kind::Endeavor,
            Kind::Arrival,
            Kind::Season,
            Kind::Agent,
        ] {
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

/// "Pema has joined the settlement" (#232).
pub fn describe_arrival(a: &Arrival) -> String {
    format!("{} has joined the settlement", a.name)
}

/// "Haru used Raise Earth at (5, 3) - bend 9 of 10" (the bender's own count toward mastery,
/// `docs/design/endgame.md`, "Showing mastery", #347; the suffix drops once the data has no
/// masters sign), or "Haru's Raise Earth failed: needs dry ground beside it" (a failed bend
/// never counts, so it never carries one).
pub fn describe_bend_event(session: &Session, ev: &BendEvent) -> String {
    let def = session.defs.ability(ev.ability);
    let unit = session.world.unit(ev.unit);
    let who = unit.map_or("Someone", |u| u.name.as_str());
    match ev.outcome {
        BendOutcome::Done => {
            let line = format!("{who} used {} at ({}, {})", def.name, ev.pos.x, ev.pos.y);
            let threshold = unit
                .and_then(|u| session.defs.endeavor.master_threshold(session.world.nation_of(u)));
            match threshold {
                Some(t) => format!("{line} - bend {} of {t}", unit.map_or(0, |u| u.bends())),
                None => line,
            }
        }
        BendOutcome::Failed(why) => format!("{who}'s {} failed: {why}", def.name),
    }
}

/// A season event's line (`docs/design/difficulty.md` "TUI"): the winter warning (with current
/// food and population), entering winter, or spring returning. `None` for a change the doc
/// doesn't call out (every season but winter and the one right after it, generically "spring",
/// whichever a difficulty preset's own data calls the season before `winter_index`).
fn describe_season_event(session: &Session, ev: &SeasonEvent) -> Option<String> {
    let defs = &session.defs;
    Some(match ev.kind {
        SeasonEventKind::WinterWarning => {
            let minutes = defs.seasons.calendar.winter_warning_minutes.round() as u32;
            let food = food_in_stores(session);
            let people = session.world.units.len();
            format!("Winter in {minutes} minutes: {food} food for {people} people.")
        }
        SeasonEventKind::Changed { season } => {
            let n = defs.seasons.seasons.len();
            if defs.seasons.seasons[season].winter {
                "Winter. Wild plants rest, and the cold tires people away from fires.".to_owned()
            } else if n > 0 && season == (defs.seasons.winter_index() + 1) % n {
                "Spring: the land wakes.".to_owned()
            } else {
                return None;
            }
        }
        SeasonEventKind::Frost { count } => format!("First frost: {count} wild plants died back."),
    })
}

/// "A fruit tree at (90, 41) was picked to death" (Hard, `strain_kills`, #296): the resource's
/// name, lower case, since the tile's own picked-to-death tree reads better as a thing than a
/// proper noun.
fn describe_strain_event(defs: &fn_sim::Defs, ev: &StrainEvent) -> String {
    let name = defs.resource(ev.resource).name.to_lowercase();
    format!("A {name} at ({}, {}) was picked to death.", ev.pos.x, ev.pos.y)
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
    fn describes_an_arrival() {
        let a = Arrival { tick: 0, unit: fn_sim::world::UnitId(6), name: "Pema".into() };
        assert_eq!(describe_arrival(&a), "Pema has joined the settlement");
    }

    #[test]
    fn an_arrival_becomes_a_toast() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let mut alerts = Alerts::default();
        alerts.log.push(Entry {
            tick: 0,
            kind: Kind::Arrival,
            text: describe_arrival(&Arrival {
                tick: 0,
                unit: fn_sim::world::UnitId(6),
                name: "Pema".into(),
            }),
            left: Kind::Arrival.toast_seconds(),
        });
        alerts.update(&s, 0.0);
        assert_eq!(alerts.toast_lines(), ["Pema has joined the settlement"]);
        alerts.update(&s, Kind::Arrival.toast_seconds() + 1.0);
        assert!(alerts.toasts().is_empty(), "the toast should have aged out");
        assert_eq!(alerts.history().len(), 1, "but the history keeps it");
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
        // The bundled masters sign's threshold (#347); a fresh unit hasn't bent yet.
        assert!(line.ends_with(" - bend 0 of 25"), "{line}");

        let failed =
            BendEvent { outcome: BendOutcome::Failed("needs dry ground beside it"), ..done };
        let line = describe_bend_event(&s, &failed);
        assert!(line.contains("failed: needs dry ground beside it"), "{line}");
        assert!(!line.contains(" - bend "), "a failed bend never counts: {line}");
    }

    /// Without a masters sign in the data, a done bend's line carries no credit suffix at all
    /// (`EndeavorDef::master_threshold` is `None`).
    #[test]
    fn a_done_bend_carries_no_suffix_without_a_masters_sign() {
        let mut s = Session::new(1, Nation::Earth, 48, 32);
        s.defs = s.defs.clone().with_endeavor("[endeavor]\nname = \"Gate\"\n").unwrap();
        let ability = ability_for_nation(&s.defs, Nation::Earth).unwrap();
        let unit = s.world.units[0].id;
        let done = BendEvent {
            tick: 0,
            unit,
            ability,
            pos: Pos::new(5, 3),
            outcome: BendOutcome::Done,
            moved: None,
        };
        let line = describe_bend_event(&s, &done);
        assert!(!line.contains(" - bend "), "{line}");
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
    fn endeavor_events_become_toasts() {
        let mut s = Session::new(7, Nation::Fire, 48, 32);
        // One sign a new game meets at once, so the first check (tick 10) logs it and Ready.
        let signs = "[endeavor]\nname = \"Gate\"\n[[sign]]\nid = \"p\"\ngroup = \"people\"\n\
                     name = \"People\"\nkind = \"population\"\ntarget = 1\n";
        s.defs = s.defs.clone().with_endeavor(signs).unwrap();
        run_until(&mut s, |s| s.world.endeavor_events().len() == 2);
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.0);
        assert_eq!(alerts.toast_lines(), ["Sign met: People", crate::endeavor::READY_TEXT]);
        assert!(alerts.history().iter().all(|e| e.kind == Kind::Endeavor));
        alerts.update(&s, 0.0);
        assert_eq!(alerts.history().len(), 2, "not repeated");
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

    /// The winter warning names the minutes, the food and the population;
    /// entering winter and the season right after it (generically "spring") each have their own
    /// line; every other season change is silent.
    #[test]
    fn describes_the_winter_warning_winter_and_spring_but_nothing_else() {
        let s = Session::new(7, Nation::Earth, 48, 32);
        let warning = SeasonEvent { tick: 0, kind: SeasonEventKind::WinterWarning };
        let line = describe_season_event(&s, &warning).unwrap();
        assert!(line.starts_with("Winter in 3 minutes: "), "{line}");
        assert!(line.ends_with(&format!("for {} people.", s.world.units.len())), "{line}");

        let winter_idx = s.defs.seasons.winter_index();
        let winter = SeasonEvent { tick: 0, kind: SeasonEventKind::Changed { season: winter_idx } };
        assert_eq!(
            describe_season_event(&s, &winter).as_deref(),
            Some("Winter. Wild plants rest, and the cold tires people away from fires.")
        );

        let n = s.defs.seasons.seasons.len();
        let spring_idx = (winter_idx + 1) % n;
        let spring = SeasonEvent { tick: 0, kind: SeasonEventKind::Changed { season: spring_idx } };
        assert_eq!(describe_season_event(&s, &spring).as_deref(), Some("Spring: the land wakes."));

        for season in 0..n {
            if season != winter_idx && season != spring_idx {
                let ev = SeasonEvent { tick: 0, kind: SeasonEventKind::Changed { season } };
                assert_eq!(describe_season_event(&s, &ev), None, "season {season} stays quiet");
            }
        }

        let frost = SeasonEvent { tick: 0, kind: SeasonEventKind::Frost { count: 23 } };
        assert_eq!(
            describe_season_event(&s, &frost).as_deref(),
            Some("First frost: 23 wild plants died back.")
        );
    }

    /// A plant picked to death names what it was, lower case, and where.
    #[test]
    fn describes_a_plant_picked_to_death() {
        let defs = fn_sim::Defs::bundled();
        let resource = defs.resource_id("fruit_tree").unwrap();
        let ev = StrainEvent { tick: 0, pos: Pos::new(90, 41), resource };
        assert_eq!(
            describe_strain_event(&defs, &ev),
            "A fruit tree at (90, 41) was picked to death."
        );
    }
}

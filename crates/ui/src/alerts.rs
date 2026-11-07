//! The alert log: bend results (#72), tech completions (#23), deaths, the end-game endeavor's
//! events (#235), newcomers joining the settlement (#232), a stockpile that can't take any more
//! of something (#337) and a one-time nudge to try bending (#382) as one stream of lines.
//!
//! Every line is kept in a bounded history (the terminal UI's message log). The newest ones also
//! show as short-lived *toasts* in the side panel's Alerts section (ui.md: "a section with
//! nothing to show hides itself"). A toast ages by the `dt` the caller passes to
//! [`Alerts::update`], in real seconds, so this crate never reads a clock.
//!
//! A **routine success never toasts** (#417, docs/design/side-panel-v2.md "Zone 2"): a finished
//! bend is logged (`L`) but never shown as a toast (a failed one still is -- that's the one
//! worth an eye). **Repeats coalesce**: while two or more toasts of the same [`Kind`] and
//! subject ([`Entry::toast`]'s own grouping key, e.g. the ability a failed bend named) are both
//! still live, [`Alerts::toasts`] shows one line with a count instead of one row each. A toast's
//! own text never carries map coordinates (the log entry, [`Alerts::history`], still does).

use std::collections::BTreeSet;

use fn_sim::Defs;
use fn_sim::jobs::ItemId;
use fn_sim::needs::DeathCause;
use fn_sim::world::{
    Arrival, BendEvent, BendOutcome, SeasonEvent, SeasonEventKind, StrainEvent, TICKS_PER_SECOND,
};

use crate::endeavor;
use crate::session::Session;
use crate::tools::{Tool, usable_abilities};
use crate::view::{food_in_stores, stuck_items};

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
    /// Something lying on the ground can't be stored because every stockpile able to take it is
    /// already full (#337; the playtest evidence that made this the issue's highest-value fix:
    /// a single filled stockpile left every chopped tree's wood on the ground with nothing to
    /// say why).
    Stockpile,
    /// The winter warning, a season change worth announcing (winter or spring), the first
    /// frost, or (Hard) a wild plant picked to death (#295, #296, #299).
    Season,
    /// An agent's act, while `fn_tui --watch` (#319) is replaying a run log: not something the
    /// world logged, so [`Alerts::push_agent`] adds it directly instead of coming from
    /// [`Alerts::update`]'s own counts.
    Agent,
    /// The one-time nudge to designate a bend (#382): fires once, when the settlement has gone
    /// [`BEND_TIP_AFTER_TICKS`] with chi to spare and nobody has ever designated one. A gentle
    /// onboarding hint, not something wrong, so it never repeats and never stops `end_turn`
    /// (`Entry::worth_stopping`).
    BendTip,
}

impl Kind {
    /// How long the toast stays, in real seconds, for a kind that toasts at all. A finished
    /// (non-failed) `Bend` never reaches this (#417: a routine success is log-only), so this is
    /// really "how long a *failed* bend's toast stays" for that one kind.
    pub fn toast_seconds(self) -> f32 {
        match self {
            Self::Bend => 6.0,
            Self::Tech => 8.0,
            Self::Death => 0.0,
            Self::Endeavor => 10.0,
            Self::Arrival => 8.0,
            Self::Season => 10.0,
            Self::Stockpile => 10.0,
            // The agent strip (#319) already names the current action; the log (`L`) is where
            // this is meant to be read back, not a toast on top of the map.
            Self::Agent => 0.0,
            Self::BendTip => 10.0,
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
            Self::Stockpile => 3,
            Self::Agent => 0,
            // Fires at most once a game, ever (`Alerts::update`), so there's never more than one
            // to show at a time.
            Self::BendTip => 1,
        }
    }
}

/// What a toast shows for one entry, before [`Alerts::toasts`] folds repeats together (#417):
/// `subject` is the grouping key (and the plural line's own text, "Hearthflame failed ×3"),
/// `text` is what a single, uncoalesced occurrence shows -- the richer line, but still with no
/// map coordinates (those stay in [`Entry::text`], the log's own copy).
#[derive(Clone, Debug, PartialEq)]
struct ToastSpec {
    subject: String,
    text: String,
}

/// One line of the log.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The world tick it happened on.
    pub tick: u64,
    pub kind: Kind,
    /// The log's own copy: the full detail, map coordinates included where the describer names
    /// a place. Never shown as a toast directly; see [`Entry::toast`].
    pub text: String,
    /// Seconds its toast has left; at or below zero it is history only. Always zero for an entry
    /// that never toasts (`toast` is `None`): a routine success (#417, a finished `Bend`), a
    /// `Death` or an `Agent` line.
    left: f32,
    /// `Some` for a kind and an outcome that toasts at all; `None` for a routine success (#417),
    /// a `Death` or an `Agent` line, which are log-only.
    toast: Option<ToastSpec>,
    /// A `Bend` that failed rather than finished. Ignored for every other kind: only `Bend`
    /// alerts need the distinction, since a finished bend is routine (#377) but a failed one
    /// still says something worth an agent's attention.
    failed: bool,
}

impl Entry {
    /// Worth an agent stopping `Host::end_turn` (crates/tui/src/surface.rs) for: every kind but
    /// `Bend`, `Agent` and `BendTip`, since those are either routine (a settlement that bends a
    /// lot finishes one most turns, #377), for `Agent`, never produced by [`Alerts::update`] in
    /// the first place (only [`Alerts::push_agent`], for `--watch` replay), or, for `BendTip`,
    /// a gentle onboarding nudge rather than something gone wrong: it fires at most once a game
    /// (#382) and a racer or player who misses it loses nothing it can't recover from later, so
    /// stopping every `end_turn` call made before it's finally noticed would cost far more than
    /// it saves. A `Bend` that *failed* is the one exception: still worth stopping for.
    /// `Stockpile` always stops too (#337): a filled stockpile left a 2026-11-05 playtest
    /// concluding wood was unobtainable, so this is worth an agent's attention every time it
    /// fires (once per item going stuck, `Alerts::update`, not every tick it stays that way).
    pub fn worth_stopping(&self) -> bool {
        match self.kind {
            Kind::Death | Kind::Arrival | Kind::Endeavor | Kind::Tech | Kind::Season => true,
            Kind::Stockpile => true,
            Kind::Bend => self.failed,
            Kind::Agent => false,
            Kind::BendTip => false,
        }
    }
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
    /// Items `stuck_items` found with nowhere to go, as of the last call (#337): not one of
    /// `World`'s own monotonic event logs (it's a standing condition, not something that
    /// happens once), so a fresh line is pushed only on the ones newly in this set, not every
    /// call while it stays stuck.
    full_items: BTreeSet<ItemId>,
    /// Whether the one-time bend tip (#382) has already fired, or is moot because a bend has
    /// already been designated: either way, [`Alerts::update`] never checks for it again.
    bend_tip_shown: bool,
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
            let failed = matches!(ev.outcome, BendOutcome::Failed(_));
            let text = describe_bend_event(session, ev);
            // A finished bend is routine (#377) and, since #417, log-only: with a settlement
            // that bends a lot, toasting every one is exactly the noise this issue exists to
            // cut. A failed one still says something worth a toast, coalesced by the ability
            // that failed (not who was channelling it) so several benders failing the same way
            // reads as one line with a count.
            let toast = failed.then(|| {
                let ability = &session.defs.ability(ev.ability).name;
                ToastSpec { subject: format!("{ability} failed"), text: text.clone() }
            });
            fresh.push((ev.tick, Kind::Bend, text, failed, toast));
        }
        for c in &completions[self.seen.1..] {
            let text = format!("Learned {}", session.defs.tech(c.tech).name);
            let toast = Some(ToastSpec { subject: text.clone(), text: text.clone() });
            fresh.push((c.tick, Kind::Tech, text, false, toast));
        }
        for d in &deaths[self.seen.2..] {
            let how = match d.cause {
                DeathCause::Starvation => "starvation",
                DeathCause::Dehydration => "dehydration",
                DeathCause::Exhaustion => "exhaustion",
                // #387, #388, #389; the full wording (and `weakened_by`) is #393's job.
                DeathCause::Exposure => "exposure",
                DeathCause::Injury => "injuries",
                DeathCause::Sickness => "sickness",
            };
            let text = format!("{} died of {how}", d.name);
            fresh.push((d.tick, Kind::Death, text, false, None));
        }
        for ev in &endeavor[self.seen.3..] {
            let text = endeavor::describe_event(&session.defs, ev);
            let toast = Some(ToastSpec { subject: text.clone(), text: text.clone() });
            fresh.push((ev.tick, Kind::Endeavor, text, false, toast));
        }
        for a in &arrivals[self.seen.4..] {
            let text = describe_arrival(a);
            let toast = Some(ToastSpec { subject: text.clone(), text: text.clone() });
            fresh.push((a.tick, Kind::Arrival, text, false, toast));
        }
        for ev in &seasons[self.seen.5..] {
            if let Some(text) = describe_season_event(session, ev) {
                let toast = Some(ToastSpec { subject: text.clone(), text: text.clone() });
                fresh.push((ev.tick, Kind::Season, text, false, toast));
            }
        }
        for ev in &strains[self.seen.6..] {
            let text = describe_strain_event(&session.defs, ev);
            let toast = Some(strain_toast(&session.defs, ev));
            fresh.push((ev.tick, Kind::Season, text, false, toast));
        }
        // Not one of `World`'s own monotonic event logs above (#337: a standing condition, not
        // something that happens once), so the line is for whichever items are newly stuck since
        // the last call, not every item still stuck.
        let stuck = stuck_items(session);
        for &(item, count) in &stuck {
            if !self.full_items.contains(&item) {
                let text = describe_stockpile_full(&session.defs, item, count);
                let toast = Some(ToastSpec { subject: text.clone(), text: text.clone() });
                fresh.push((world.tick, Kind::Stockpile, text, false, toast));
            }
        }
        self.full_items = stuck.into_iter().map(|(item, _)| item).collect();
        // The one-time bend tip (#382): also not one of `World`'s own monotonic logs, so this
        // checks live state instead of a count. Moot once a bend has ever been designated
        // (`events` -- finished or failed -- or a job still pending), whether or not it ever
        // fires; either way `bend_tip_shown` stops the check for the rest of the game.
        if !self.bend_tip_shown {
            let ever_designated = !events.is_empty() || world.bend_designations().next().is_some();
            if ever_designated {
                self.bend_tip_shown = true;
            } else if world.tick >= BEND_TIP_AFTER_TICKS && chi_to_spare(session) {
                let text = bend_tip_text();
                let toast = Some(ToastSpec { subject: text.clone(), text: text.clone() });
                fresh.push((world.tick, Kind::BendTip, text, false, toast));
                self.bend_tip_shown = true;
            }
        }
        self.seen = counts;
        fresh.sort_by_key(|(tick, ..)| *tick);
        self.log.extend(fresh.into_iter().map(|(tick, kind, text, failed, toast)| {
            let left = if toast.is_some() { kind.toast_seconds() } else { 0.0 };
            Entry { tick, kind, text, left, toast, failed }
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
        self.log.push(Entry {
            tick,
            kind: Kind::Agent,
            text,
            left: 0.0,
            toast: None,
            failed: false,
        });
        if self.log.len() > MAX_HISTORY {
            self.log.drain(0..self.log.len() - MAX_HISTORY);
        }
    }

    /// The toasts still showing, oldest first: those with time left, folded by kind and subject
    /// (#417, [`ToastSpec`]) into one [`Toast`] each with a count, at most `Kind::max_toasts`
    /// groups of each kind (the newest).
    pub fn toasts(&self) -> Vec<Toast> {
        struct Group {
            kind: Kind,
            subject: String,
            text: String,
            count: usize,
            tick: u64,
        }
        let mut groups: Vec<Group> = Vec::new();
        for e in &self.log {
            if e.left <= 0.0 {
                continue;
            }
            let Some(toast) = &e.toast else { continue };
            match groups.iter_mut().find(|g| g.kind == e.kind && g.subject == toast.subject) {
                Some(g) => {
                    g.count += 1;
                    g.text = toast.text.clone();
                    g.tick = e.tick;
                }
                None => groups.push(Group {
                    kind: e.kind,
                    subject: toast.subject.clone(),
                    text: toast.text.clone(),
                    count: 1,
                    tick: e.tick,
                }),
            }
        }
        let mut keep = vec![true; groups.len()];
        for kind in [
            Kind::Bend,
            Kind::Tech,
            Kind::Death,
            Kind::Endeavor,
            Kind::Arrival,
            Kind::Season,
            Kind::Stockpile,
            Kind::Agent,
            Kind::BendTip,
        ] {
            let mut of_kind: Vec<usize> =
                (0..groups.len()).filter(|&i| groups[i].kind == kind).collect();
            of_kind.sort_by_key(|&i| groups[i].tick);
            let drop = of_kind.len().saturating_sub(kind.max_toasts());
            for &i in &of_kind[..drop] {
                keep[i] = false;
            }
        }
        let mut out: Vec<Group> =
            groups.into_iter().zip(keep).filter_map(|(g, k)| k.then_some(g)).collect();
        out.sort_by_key(|g| g.tick);
        out.into_iter()
            .map(|g| Toast {
                kind: g.kind,
                // A repeat coalesces into the subject plus a count (#417: "Hearthflame failed
                // ×3"); a lone occurrence keeps its own, richer text instead.
                text: if g.count > 1 { format!("{} ×{}", g.subject, g.count) } else { g.text },
            })
            .collect()
    }

    /// The toasts' text, oldest first, for the side panel.
    pub fn toast_lines(&self) -> Vec<String> {
        self.toasts().into_iter().map(|t| t.text).collect()
    }
}

/// One toast [`Alerts::toasts`] shows: already folded by kind and subject, so its text is
/// exactly what the side panel's Alerts section puts on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub kind: Kind,
    pub text: String,
}

/// "Pema has joined the settlement" (#232).
pub fn describe_arrival(a: &Arrival) -> String {
    format!("{} has joined the settlement", a.name)
}

/// "Haru used Raise Earth at (5, 3) - bend 9 of 10" (the bender's own count toward mastery,
/// `docs/design/endgame.md`, "Showing mastery", #347; the suffix drops once the data has no
/// masters sign), carrying ", a new form" when this was the first bend of this ability this
/// unit ever finished (`Unit::bends_of`, docs/design/bending.md "Seeing it", #361: "Jee welded
/// the kiln: bend 9 of 10, a new form"), or "Haru's Raise Earth failed: needs dry ground beside
/// it" (a failed bend never counts, so it never carries either).
pub fn describe_bend_event(session: &Session, ev: &BendEvent) -> String {
    let def = session.defs.ability(ev.ability);
    let unit = session.world.unit(ev.unit);
    let who = unit.map_or("Someone", |u| u.name.as_str());
    match ev.outcome {
        BendOutcome::Done => {
            let line = format!("{who} used {} at ({}, {})", def.name, ev.pos.x, ev.pos.y);
            let threshold = unit
                .and_then(|u| session.defs.endeavor.master_threshold(session.world.nation_of(u)));
            let new_form = unit.is_some_and(|u| u.bends_of(ev.ability) == 1);
            let line = match threshold {
                Some(t) => format!("{line} - bend {} of {t}", unit.map_or(0, |u| u.bends())),
                None => line,
            };
            if new_form { format!("{line}, a new form") } else { line }
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

/// A strain event's toast (#417): the log's own line (`describe_strain_event`) names the place,
/// but the toast drops it, and coalesces by the resource (the subject), so several of the same
/// kind dying back in a stretch reads as one line with a count instead of one row per tree.
fn strain_toast(defs: &fn_sim::Defs, ev: &StrainEvent) -> ToastSpec {
    let name = defs.resource(ev.resource).name.to_lowercase();
    ToastSpec {
        subject: format!("{name} picked to death"),
        text: format!("A {name} was picked to death."),
    }
}

/// "42 wood can't be stored: every stockpile is full" (#337): the lower-case name since the
/// count in front reads better as a quantity than a label, and the count itself so the player
/// sees there's really something piling up, not just a flag.
fn describe_stockpile_full(defs: &Defs, item: ItemId, count: u32) -> String {
    let name = defs.item(item).name.to_lowercase();
    format!("{count} {name} can't be stored: every stockpile is full.")
}

/// How long the settlement may go with chi to spare and no bend ever designated before the
/// one-time tip fires (#382): 10 in-game minutes, long enough to clear the opening minutes
/// (founding, assigning idle settlers) but short enough to catch a player before they've sunk
/// real time not knowing a bend is something to designate -- the gap a 2026-11-05 playtest
/// surfaced lasted hundreds of turns, since every unit starts at full chi (`World::new`) and so
/// would otherwise trip this on turn one.
const BEND_TIP_AFTER_TICKS: u64 = 10 * 60 * TICKS_PER_SECOND as u64;

/// Whether anyone in the settlement, other than the Avatar (a guest, never the player's to
/// bend), has chi enough right now to channel at least one ability the settlement can already
/// use (`crate::tools::usable_abilities`): "chi to spare" tied to what a bend actually costs,
/// not an arbitrary fraction of the meter. `false` with nothing usable at all, which shouldn't
/// happen for a settlement's own nation (every nation's data defines a signature ability) but
/// costs nothing to guard.
fn chi_to_spare(session: &Session) -> bool {
    let min_cost = usable_abilities(session)
        .iter()
        .map(|&id| session.defs.ability(id).chi)
        .fold(f32::INFINITY, f32::min);
    min_cost.is_finite() && session.world.units.iter().any(|u| !u.is_avatar() && u.chi >= min_cost)
}

/// The one-time bend tip's own line (#382), naming the Bend tool's real key
/// (`Tool::Bend::key`, the key help's own source, so this can't drift from it and never names
/// an F-key, which WSL terminals never deliver): "Chi to spare, no bend designated yet - press B
/// on a tile a bender can work."
fn bend_tip_text() -> String {
    format!(
        "Chi to spare, no bend designated yet - press {} on a tile a bender can work.",
        Tool::Bend.key()
    )
}

#[cfg(test)]
mod tests {
    use fn_sim::bending::AbilityId;
    use fn_sim::world::Command;
    use fn_sim::{Nation, Pos, Terrain};

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
        let text = describe_arrival(&Arrival {
            tick: 0,
            unit: fn_sim::world::UnitId(6),
            name: "Pema".into(),
        });
        alerts.log.push(Entry {
            tick: 0,
            kind: Kind::Arrival,
            text: text.clone(),
            left: Kind::Arrival.toast_seconds(),
            toast: Some(ToastSpec { subject: text.clone(), text }),
            failed: false,
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
        // Earth's own threshold from the bundled masters sign's `bends_per_nation` (#347); a
        // fresh unit hasn't bent yet.
        assert!(line.ends_with(" - bend 0 of 8"), "{line}");

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

    /// A unit's first-ever finished bend of an ability carries ", a new form"
    /// (`Unit::bends_of`, docs/design/bending.md "Seeing it", #361); its second, of the same
    /// ability but a fresh tile (so Hearthflame's own restoke rule, #359, never enters into
    /// it), doesn't, since the form was already counted. One starting unit, so there's no doubt
    /// which of them claims each designation.
    #[test]
    fn a_units_first_bend_of_an_ability_notes_a_new_form() {
        use fn_sim::defs::BiomeId;
        use fn_sim::{Grid, Terrain, Tile, World};

        let defs = Defs::bundled();
        let map = Grid::from_fn(6, 3, |_| Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: Terrain::Land,
            biome: BiomeId(0),
            resource: None,
            building: None,
        });
        let world = World::with_map(1, map, Nation::Fire, &[Pos::new(0, 1)], &defs);
        let mut s =
            Session { world, defs, clock: crate::clock::Clock::default(), bend_choice: None };
        let ability = ability_for_nation(&s.defs, Nation::Fire).unwrap();

        s.apply(Command::Bend { ability, pos: Pos::new(1, 1) }).unwrap();
        run_until(&mut s, |s| !s.world.bend_events().is_empty());
        let first = describe_bend_event(&s, &s.world.bend_events()[0]);
        assert!(first.ends_with(", a new form"), "{first}");

        s.apply(Command::Bend { ability, pos: Pos::new(4, 1) }).unwrap();
        run_until(&mut s, |s| s.world.bend_events().len() > 1);
        let second = describe_bend_event(&s, &s.world.bend_events()[1]);
        assert!(!second.contains("new form"), "{second}");
    }

    /// A finished bend lands in the log (`alerts.history()`), but never as a toast (#417: a
    /// routine success is noise, exactly what made three rows of "X used Hearthflame" push out
    /// anything that mattered).
    #[test]
    fn a_designated_bend_eventually_becomes_a_log_line_but_not_a_toast() {
        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let ability: AbilityId = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        let pos = s.world.units[0].pos;
        s.apply(Command::Bend { ability, pos }).unwrap();
        run_until(&mut s, |s| !s.world.bend_events().is_empty());
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.016);
        assert!(alerts.toast_lines().is_empty(), "{:?}", alerts.toast_lines());
        assert_eq!(alerts.history().len(), 1);
        assert!(alerts.history()[0].text.contains(&s.defs.ability(ability).name));
        // Updating again doesn't repeat it.
        alerts.update(&s, 0.016);
        assert_eq!(alerts.history().len(), 1);
    }

    /// A finished bend is routine and never worth stopping `end_turn` for (#377); a failed one
    /// still is, so it needs the same attention a death or an arrival gets.
    #[test]
    fn a_finished_bend_isnt_worth_stopping_but_a_failed_one_is() {
        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let ability: AbilityId = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        let pos = s.world.units[0].pos;
        s.apply(Command::Bend { ability, pos }).unwrap();
        run_until(&mut s, |s| !s.world.bend_events().is_empty());
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.0);
        let done = alerts.history().last().expect("the finished bend logged a line");
        assert_eq!(done.kind, Kind::Bend);
        assert!(!done.text.contains("failed"), "{}", done.text);
        assert!(!done.worth_stopping(), "a finished bend is routine: {}", done.text);

        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let pos = s.world.units[0].pos;
        s.apply(Command::Bend { ability, pos }).unwrap();
        // The ground turns to rock before the channel finishes, so it fails instead.
        s.world.map[pos].terrain = Terrain::Mountain;
        s.world.map_changed();
        run_until(&mut s, |s| !s.world.bend_events().is_empty());
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.0);
        let failed = alerts.history().last().expect("the failed bend logged a line");
        assert_eq!(failed.kind, Kind::Bend);
        assert!(failed.text.contains("failed"), "{}", failed.text);
        assert!(failed.worth_stopping(), "a failed bend still says something: {}", failed.text);
    }

    /// Every other kind `Alerts::update` ever produces -- a tech, a death, an endeavor sign (or
    /// #347's "is now a bending master", which comes through as `Kind::Endeavor` too), an
    /// arrival, a season event -- is worth stopping `end_turn` for; only `Agent` (never produced
    /// here, only by [`Alerts::push_agent`] for `--watch` replay) isn't.
    #[test]
    fn every_kind_but_a_finished_bend_agent_and_the_bend_tip_is_worth_stopping() {
        let worth_stopping = |kind: Kind, failed: bool| {
            Entry { tick: 0, kind, text: String::new(), left: 0.0, toast: None, failed }
                .worth_stopping()
        };
        assert!(worth_stopping(Kind::Death, false));
        assert!(worth_stopping(Kind::Arrival, false));
        assert!(worth_stopping(Kind::Endeavor, false));
        assert!(worth_stopping(Kind::Tech, false));
        assert!(worth_stopping(Kind::Season, false));
        assert!(worth_stopping(Kind::Bend, true), "a failed bend");
        assert!(!worth_stopping(Kind::Bend, false), "a finished bend is routine");
        assert!(!worth_stopping(Kind::Agent, false));
        // Gentle, not urgent (#382): an agent that misses it loses nothing it can't recover
        // from by noticing the hint later, so it shouldn't cost every `end_turn` call made
        // before that.
        assert!(!worth_stopping(Kind::BendTip, false));
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
                     name = \"People\"\nshort = \"people\"\nkind = \"population\"\ntarget = 1\n";
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
            let text = i.to_string();
            alerts.log.push(Entry {
                tick: i,
                kind: Kind::Bend,
                text: text.clone(),
                left: Kind::Bend.toast_seconds(),
                // Each a distinct subject, so this tests the per-kind cap, not coalescing.
                toast: Some(ToastSpec { subject: text.clone(), text }),
                failed: false,
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
            let text = tick.to_string();
            alerts.log.push(Entry {
                tick,
                kind,
                text: text.clone(),
                left: 5.0,
                toast: Some(ToastSpec { subject: text.clone(), text }),
                failed: false,
            });
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

    /// A stockpile tile already holding one item strands another dropped afterward (#337, the
    /// 2026-11-05 playtest evidence: a filled stockpile left every chopped tree's wood on the
    /// ground with nothing to say why). The first `update` after it goes stuck gets a line
    /// naming the item and how much is stuck, worth stopping `end_turn` for; staying stuck
    /// doesn't repeat it.
    #[test]
    fn a_full_stockpile_becomes_a_one_shot_alert_worth_stopping_for() {
        use fn_sim::defs::BiomeId;
        use fn_sim::world::Command;
        use fn_sim::{Grid, Tile, World};
        let defs = fn_sim::Defs::bundled();
        let stone = defs.resource_id("stone").unwrap();
        let oak = defs.resource_id("oak_tree").unwrap();
        let (stone_item, wood_item) =
            (defs.item_id("stone").unwrap(), defs.item_id("wood").unwrap());
        let map = Grid::from_fn(10, 3, |p| Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: if p == Pos::new(3, 1) { Terrain::Mountain } else { Terrain::Land },
            biome: BiomeId(0),
            resource: match p {
                q if q == Pos::new(3, 1) => Some(stone),
                q if q == Pos::new(7, 1) => Some(oak),
                _ => None,
            },
            building: None,
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(0, 1)], &defs);
        let mut s =
            Session { world, defs, clock: crate::clock::Clock::default(), bend_choice: None };
        s.apply(Command::DesignateStockpile { pos: Pos::new(1, 1) }).unwrap();
        s.apply(Command::DesignateHarvest { pos: Pos::new(3, 1) }).unwrap();
        run_until(&mut s, |s| s.world.stored_total(stone_item) > 0);
        s.apply(Command::DesignateHarvest { pos: Pos::new(7, 1) }).unwrap();
        run_until(&mut s, |s| s.world.ground_items().any(|(_, st)| st.item == wood_item));

        let mut alerts = Alerts::default();
        alerts.update(&s, 0.0);
        assert_eq!(alerts.history().len(), 1, "{:?}", alerts.history());
        let entry = &alerts.history()[0];
        assert_eq!(entry.kind, Kind::Stockpile);
        assert!(entry.text.contains("wood"), "{}", entry.text);
        assert!(entry.text.contains("full"), "{}", entry.text);
        assert!(entry.worth_stopping(), "worth an agent's attention (#337)");

        // Staying stuck for another stretch of ticks doesn't add a second line.
        for _ in 0..200 {
            s.tick();
            alerts.update(&s, 0.0);
        }
        assert_eq!(alerts.history().len(), 1);
    }

    /// Every unit starts at full chi (`World::new`), so there's always "chi to spare" from
    /// turn one; only the elapsed time gates the bend tip (#382).
    #[test]
    fn chi_to_spare_is_true_from_the_start() {
        let s = Session::new(7, Nation::Earth, 48, 32);
        assert!(chi_to_spare(&s));
    }

    /// The bend tip fires once, naming the real Bend key, after chi has gone spare for
    /// `BEND_TIP_AFTER_TICKS` with no bend ever designated (#382: the playtest evidence that
    /// a settlement can run for hundreds of turns with nobody ever trying). It never repeats,
    /// and it never stops `end_turn`.
    #[test]
    fn the_bend_tip_fires_once_after_a_while_with_nothing_designated() {
        let mut s = Session::new(7, Nation::Earth, 48, 32);
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.0);
        assert!(alerts.history().is_empty(), "too soon: {:?}", alerts.history());

        s.world.tick = BEND_TIP_AFTER_TICKS;
        alerts.update(&s, 0.0);
        assert_eq!(alerts.history().len(), 1, "{:?}", alerts.history());
        let entry = &alerts.history()[0];
        assert_eq!(entry.kind, Kind::BendTip);
        assert_eq!(entry.text, bend_tip_text());
        assert!(entry.text.contains(&format!("press {}", Tool::Bend.key())), "{}", entry.text);
        assert!(!entry.worth_stopping(), "a gentle nudge, not something gone wrong");

        // Doesn't repeat, however much more time passes.
        s.world.tick += BEND_TIP_AFTER_TICKS;
        alerts.update(&s, 0.0);
        assert_eq!(alerts.history().len(), 1);
    }

    /// Once a bend is designated, however briefly, the tip never fires at all: the player has
    /// already found the Bend tool, even if that one designation is cancelled before it
    /// finishes. Fire's Hearthflame needs only an empty Land or Coast tile, the easiest target
    /// to find on any map.
    #[test]
    fn the_bend_tip_never_fires_once_a_bend_is_designated() {
        let mut s = Session::new(7, Nation::Fire, 48, 32);
        let ability = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        let pos = (0..32)
            .flat_map(|y| (0..48).map(move |x| Pos::new(x, y)))
            .find(|&p| {
                let t = &s.world.map[p];
                matches!(t.terrain, Terrain::Land | Terrain::Coast)
                    && t.resource.is_none()
                    && t.building.is_none()
            })
            .expect("some open tile exists on a 48x32 map");
        s.apply(Command::Bend { ability, pos }).unwrap();

        let mut alerts = Alerts::default();
        s.world.tick = BEND_TIP_AFTER_TICKS;
        alerts.update(&s, 0.0);
        assert!(alerts.history().is_empty(), "{:?}", alerts.history());

        s.apply(Command::CancelBend { pos }).unwrap();
        s.world.tick += BEND_TIP_AFTER_TICKS;
        alerts.update(&s, 0.0);
        assert!(
            alerts.history().is_empty(),
            "designating once is enough, even cancelled: {:?}",
            alerts.history()
        );
    }

    /// Three toasts of the same kind and subject, still live at once, fold into one line with a
    /// count (#417) instead of three separate rows -- the panel's own spare height then goes to
    /// something that actually needs it, not three near-identical lines.
    #[test]
    fn repeats_of_the_same_kind_and_subject_coalesce_with_a_count() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let mut alerts = Alerts::default();
        for tick in 0..3 {
            alerts.log.push(Entry {
                tick,
                kind: Kind::Bend,
                text: format!("Someone's Hearthflame failed at tick {tick}"),
                left: Kind::Bend.toast_seconds(),
                toast: Some(ToastSpec {
                    subject: "Hearthflame failed".to_owned(),
                    text: "Someone's Hearthflame failed: no fuel nearby".to_owned(),
                }),
                failed: true,
            });
        }
        alerts.update(&s, 0.0);
        assert_eq!(alerts.toast_lines(), ["Hearthflame failed ×3"]);
        // Every occurrence still gets its own line in the log.
        assert_eq!(alerts.history().len(), 3);
    }

    /// A single occurrence of a kind and subject -- no repeat yet -- keeps its own, richer text
    /// instead of the terse "subject ×1" a count would otherwise force.
    #[test]
    fn a_single_toast_keeps_its_own_text_instead_of_a_count_of_one() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let mut alerts = Alerts::default();
        alerts.log.push(Entry {
            tick: 0,
            kind: Kind::Bend,
            text: "Haru's Hearthflame failed: no fuel nearby".to_owned(),
            left: Kind::Bend.toast_seconds(),
            toast: Some(ToastSpec {
                subject: "Hearthflame failed".to_owned(),
                text: "Haru's Hearthflame failed: no fuel nearby".to_owned(),
            }),
            failed: true,
        });
        alerts.update(&s, 0.0);
        assert_eq!(alerts.toast_lines(), ["Haru's Hearthflame failed: no fuel nearby"]);
    }

    /// Different subjects of the same kind never coalesce: three different abilities failing
    /// stay three separate toasts, up to `Kind::max_toasts`.
    #[test]
    fn different_subjects_of_the_same_kind_dont_coalesce() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let mut alerts = Alerts::default();
        for (tick, ability) in [(0, "Hearthflame"), (1, "Inner Fire")] {
            let text = format!("{ability} failed");
            alerts.log.push(Entry {
                tick,
                kind: Kind::Bend,
                text: text.clone(),
                left: Kind::Bend.toast_seconds(),
                toast: Some(ToastSpec { subject: text.clone(), text }),
                failed: true,
            });
        }
        alerts.update(&s, 0.0);
        assert_eq!(alerts.toast_lines(), ["Hearthflame failed", "Inner Fire failed"]);
    }

    /// A strain event's toast drops the map coordinates the log entry keeps (#417): the log
    /// still names the place, so going there is still one key away (`L`), but the toast reads
    /// as a fact about the settlement, not a spot on the map.
    #[test]
    fn a_strain_toast_drops_the_coordinates_the_log_keeps() {
        let defs = fn_sim::Defs::bundled();
        let resource = defs.resource_id("fruit_tree").unwrap();
        let ev = StrainEvent { tick: 0, pos: Pos::new(90, 41), resource };
        let log_text = describe_strain_event(&defs, &ev);
        assert!(log_text.contains("(90, 41)"), "{log_text}");
        let toast = strain_toast(&defs, &ev);
        assert!(!toast.text.contains('('), "{}", toast.text);
        assert!(!toast.subject.contains('('), "{}", toast.subject);
        assert_eq!(toast.text, "A fruit tree was picked to death.");
    }

    /// Two different resources picked to death in the same stretch stay two toasts (different
    /// subjects); the same resource repeating would coalesce the same way a failed ability does.
    #[test]
    fn two_different_resources_picked_to_death_dont_coalesce() {
        let defs = fn_sim::Defs::bundled();
        let oak = defs.resource_id("oak_tree").unwrap();
        let fruit = defs.resource_id("fruit_tree").unwrap();
        let oak_toast =
            strain_toast(&defs, &StrainEvent { tick: 0, pos: Pos::new(1, 1), resource: oak });
        let fruit_toast =
            strain_toast(&defs, &StrainEvent { tick: 0, pos: Pos::new(2, 2), resource: fruit });
        assert_ne!(oak_toast.subject, fruit_toast.subject);
    }
}

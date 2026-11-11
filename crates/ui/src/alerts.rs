//! The alert log: bend results (#72), tech completions (#23), deaths, the end-game endeavor's
//! events (#235), newcomers joining the settlement (#232), finished Map Room charts (#605), a
//! stockpile that can't take any more of something (#337) and a one-time nudge to try bending
//! (#382) as one stream of lines.
//!
//! Two conditions are **standing alerts**, open for as long as they hold rather than timed
//! toasts (#656): stockpiles that can't take something, and research points banked with no target
//! set. The log gets one line per cause when each is raised; [`Alerts::open`] is what the side
//! panel lists, most urgent first, with every full stockpile grouped into one entry.
//!
//! The world's own events come from its one event log (`World::events`, #605), read with a
//! single cursor; harm events reach it too, but have no line until #393 words them.
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

use fn_sim::Pos;
use fn_sim::jobs::ItemId;
use fn_sim::needs::DeathCause;
use fn_sim::research::MILLIS_PER_POINT;
use fn_sim::world::{
    Arrival, BendEvent, BendOutcome, CaravanEvent, CaravanEventKind, ChartEvent, ChartEventKind,
    Death, Event, SeasonEvent, SeasonEventKind, StrainEvent, TICKS_PER_SECOND, UnitId,
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
    /// A Map Room finished a chart (#402, #605): what it found, or that nothing is left.
    Chart,
    /// An agent's act, while `fn_tui --watch` (#319) is replaying a run log: not something the
    /// world logged, so [`Alerts::push_agent`] adds it directly instead of coming from
    /// [`Alerts::update`]'s own counts.
    Agent,
    /// The one-time nudge to designate a bend (#382): fires once, when the settlement has gone
    /// [`BEND_TIP_AFTER_TICKS`] with chi to spare and nobody has ever designated one. A gentle
    /// onboarding hint, not something wrong, so it never repeats and never stops `end_turn`
    /// (`Entry::worth_stopping`).
    BendTip,
    /// Research points are banked and no target is set (#656): a standing condition like a full
    /// stockpile, open until a target is set. The log gets a line when it is raised; the panel
    /// shows it for as long as it holds ([`Alerts::open`]), never as a timed toast.
    Research,
    /// Traders (#437, #438): a caravan made camp, found nowhere to, is about to leave or left,
    /// and (log only) a deal or request done.
    Trade,
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
            Self::Chart => 8.0,
            // The agent strip (#319) already names the current action; the log (`L`) is where
            // this is meant to be read back, not a toast on top of the map.
            Self::Agent => 0.0,
            Self::BendTip => 10.0,
            // A standing alert: open while the condition holds, not a toast that ages.
            Self::Research => 0.0,
            Self::Trade => 8.0,
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
            Self::Chart => 3,
            Self::Agent => 0,
            // Fires at most once a game, ever (`Alerts::update`), so there's never more than one
            // to show at a time.
            Self::BendTip => 1,
            Self::Research => 0,
            Self::Trade => 2,
        }
    }
}

/// What a log line is about, for the message log's filter (#661): the five groups a player
/// reads the log by. Set where the entry is made ([`Kind::category`]), never worked out by the
/// screen drawing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    /// Something that needs an eye: a death, the winter warning and the season turning, a full
    /// stockpile, a plant picked to death, a failed bend, an endeavor sign met.
    Alert,
    /// A settler: someone joined the settlement.
    People,
    /// Something learned: a finished tech, a finished chart.
    Knowledge,
    /// A bend that finished.
    Bending,
    /// Work and the people at it: an agent's act in a watched run, the bend tip.
    Work,
}

impl Category {
    /// Every category, in the order the filter row lists them (keys `1` to `5`).
    pub const ALL: [Category; 5] =
        [Self::Alert, Self::People, Self::Knowledge, Self::Bending, Self::Work];

    /// The word the filter row shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Alert => "Alerts",
            Self::People => "People",
            Self::Knowledge => "Knowledge",
            Self::Bending => "Bending",
            Self::Work => "Work",
        }
    }

    /// The category the digit `key` (`1` to `5`) picks.
    pub fn from_key(key: char) -> Option<Category> {
        let n = key.to_digit(10)? as usize;
        Self::ALL.get(n.checked_sub(1)?).copied()
    }

    /// The digit that picks this category.
    pub fn key(self) -> char {
        let at = Self::ALL.iter().position(|&c| c == self).unwrap_or(0);
        char::from_digit(at as u32 + 1, 10).unwrap_or('1')
    }
}

/// How soon an open alert wants an eye (#656): the panel lists them most urgent first. The
/// order is the declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    /// A settler's life: anyone starving, freezing or dying.
    Life,
    /// The stores and what feeds them: a full stockpile, the winter warning, a plant picked to
    /// death.
    Stores,
    /// Capacity going unused: no research target, a bend that failed, an endeavor sign.
    Idle,
    /// News, not a problem (a learned tech, a newcomer, a chart, the bend tip): shown under the
    /// alerts while its toast lasts, and the first thing folded away.
    Notice,
}

impl Kind {
    /// How urgent an open alert of this kind is ([`Urgency`]); anything the log doesn't file
    /// under [`Category::Alert`] is a [`Urgency::Notice`].
    pub fn urgency(self, failed: bool) -> Urgency {
        match self.category(failed) {
            Category::Alert => match self {
                Self::Death => Urgency::Life,
                Self::Stockpile | Self::Season => Urgency::Stores,
                _ => Urgency::Idle,
            },
            _ => Urgency::Notice,
        }
    }
}

impl Kind {
    /// The log category of a line of this kind. `failed` only matters for a `Bend`: a finished
    /// one is routine bending, a failed one is an alert.
    pub fn category(self, failed: bool) -> Category {
        match self {
            Self::Bend if failed => Category::Alert,
            Self::Bend => Category::Bending,
            Self::Tech | Self::Chart => Category::Knowledge,
            Self::Death | Self::Endeavor | Self::Season | Self::Stockpile | Self::Research => {
                Category::Alert
            }
            Self::Arrival | Self::Trade => Category::People,
            Self::Agent | Self::BendTip => Category::Work,
        }
    }
}

/// What lets a run of the same event close together fold into one log line (#661): the same
/// variant and the same name. Set where the entry is made; only the message log's view reads it
/// (`crate::msglog`), so the history, and the run log an agent reads, stay one line per event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fold {
    /// A finished bend of the named ability.
    Bend(String),
    /// A newcomer joined.
    Arrival,
    /// A wild plant of the named resource picked to death.
    Strain(String),
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
    /// Which line it is: counts up from 1 as lines are made, so it names the line for as long as
    /// the line is in the history, whatever drops off the front (#661: the log's cursor and its
    /// open folds hold ids, not positions).
    pub id: u64,
    /// The world tick it happened on.
    pub tick: u64,
    pub kind: Kind,
    /// What the message log's filter sorts it under ([`Kind::category`]).
    pub category: Category,
    /// The place it is about, if it names one: `Enter` in the message log puts the map cursor
    /// there.
    pub place: Option<Pos>,
    /// The settler it is about, if it names one and they are in the settlement.
    pub unit: Option<UnitId>,
    /// What it folds with when a run of it comes close together (the log's view only).
    pub fold: Option<Fold>,
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
    /// An entry with nothing set but its defaults, for a test to fill in the fields it cares about.
    #[cfg(test)]
    pub(crate) fn blank() -> Self {
        Self {
            id: 0,
            tick: 0,
            kind: Kind::Bend,
            category: Category::Bending,
            place: None,
            unit: None,
            fold: None,
            text: String::new(),
            left: 0.0,
            toast: None,
            failed: false,
        }
    }

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
            // A finished chart is progress the player set going, like a finished bend: worth a
            // toast, not worth cutting an agent's turn short.
            Kind::Chart => false,
            Kind::Agent => false,
            Kind::BendTip => false,
            // Idle capacity, not an emergency: stopping every turn until it is set would cost
            // far more than it tells (the glance's own row and the panel's alert say it too).
            Kind::Research => false,
            // Traders come and go once a season on a fixed schedule: worth a toast, and the
            // Traders screen is where they are dealt with, not a reason to cut a turn short.
            Kind::Trade => false,
        }
    }
}

/// The alert log. Feed it every frame with [`Alerts::update`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Alerts {
    /// Oldest first.
    log: Vec<Entry>,
    /// The id the next line gets (`Entry::id`).
    next_id: u64,
    /// How many of `World::events` have already been read (#605: one cursor for every kind).
    seen: usize,
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
    /// The items stuck with nowhere to go right now, as (lower-case name, count), and the tick
    /// the latest of them went stuck (#656). A standing condition: the panel's one grouped
    /// "Stockpiles are full" alert is open exactly while this is not empty.
    stores_full: Vec<(String, u32)>,
    stores_since: u64,
    /// Whole research points banked with no target set, and the tick that began (#656); `None`
    /// while a target is set or nothing is banked. Open while it is `Some`.
    research_idle: Option<(u64, u32)>,
    /// The arrival tick of the caravan already warned about leaving (#438), so the warning is
    /// one line a visit.
    trade_warned: Option<u64>,
}

impl Alerts {
    /// Turns what the world logged since the last call into lines, then ages every toast by `dt`
    /// seconds.
    pub fn update(&mut self, session: &Session, dt: f32) {
        let world = &session.world;
        let events = world.events();
        if world.tick < self.last_tick || events.len() < self.seen {
            // A new world (regenerate, nation switch, a loaded game): the old log is gone.
            *self = Self::default();
        }
        self.last_tick = world.tick;

        let mut fresh = fresh_lines(session, &events[self.seen..]);
        self.seen = events.len();
        // Not one of `World`'s own monotonic event logs above (#337: a standing condition, not
        // something that happens once), so the line is for whichever items are newly stuck since
        // the last call, not every item still stuck.
        let stuck: Vec<(ItemId, String, u32)> = stuck_items(session)
            .into_iter()
            .map(|(item, count)| (item, session.defs.item(item).name.to_lowercase(), count))
            .collect();
        self.note_stuck(world.tick, &stuck, &mut fresh);
        // No research target with points banked (#656): also a standing condition, so a line
        // is logged when it is raised, not every call while it stays so.
        let banked = (world.research().pool_millis() / MILLIS_PER_POINT) as u32;
        if world.research().queue().is_empty() && banked > 0 {
            if self.research_idle.is_none() {
                let text = describe_research_idle(banked);
                fresh.push(((world.tick, RESEARCH_ORDER), Line::new(Kind::Research, text, None)));
            }
            let since = self.research_idle.map_or(world.tick, |(since, _)| since);
            self.research_idle = Some((since, banked));
        } else {
            self.research_idle = None;
        }
        // A caravan about to leave (#438): its own line, once a visit.
        if let Some(v) = world.caravan()
            && self.trade_warned != Some(v.arrived)
            && v.leaves.saturating_sub(world.tick) <= TRADE_WARNING_TICKS
        {
            self.trade_warned = Some(v.arrived);
            let text = format!("{} leave in two minutes", caravan_name(session, v.nation));
            let line = Line::new(Kind::Trade, text.clone(), toast_of(&text)).at(v.camp);
            fresh.push(((world.tick, TRADE_ORDER), line));
        }
        // The one-time bend tip (#382): also not one of `World`'s own monotonic logs, so this
        // checks live state instead of a count. Moot once a bend has ever been designated
        // (`events` -- finished or failed -- or a job still pending), whether or not it ever
        // fires; either way `bend_tip_shown` stops the check for the rest of the game.
        if !self.bend_tip_shown {
            let ever_designated =
                world.bend_events().next().is_some() || world.bend_designations().next().is_some();
            if ever_designated {
                self.bend_tip_shown = true;
            } else if world.tick >= BEND_TIP_AFTER_TICKS && chi_to_spare(session) {
                let text = bend_tip_text();
                let toast = Some(ToastSpec { subject: text.clone(), text: text.clone() });
                fresh.push(((world.tick, BEND_TIP_ORDER), Line::new(Kind::BendTip, text, toast)));
                self.bend_tip_shown = true;
            }
        }
        // Stable: lines with the same key keep the order the world logged them in. Same-tick
        // lines therefore read bends, techs, deaths, endeavor, arrivals, seasons, strain, charts,
        // then a full stockpile and the bend tip: the order the alert log read the world's nine
        // separate lists in before #605, whatever order the sim logged them in.
        fresh.sort_by_key(|(key, ..)| *key);
        for ((tick, _), line) in fresh {
            self.next_id += 1;
            let left = if line.toast.is_some() { line.kind.toast_seconds() } else { 0.0 };
            self.log.push(Entry {
                id: self.next_id,
                tick,
                kind: line.kind,
                category: line.kind.category(line.failed),
                place: line.place,
                unit: line.unit,
                fold: line.fold,
                text: line.text,
                left,
                toast: line.toast,
                failed: line.failed,
            });
        }
        for e in &mut self.log {
            e.left -= dt;
        }
        if self.log.len() > MAX_HISTORY {
            self.log.drain(0..self.log.len() - MAX_HISTORY);
        }
    }

    /// Takes the items stuck with nowhere to go as of `tick` (item, lower-case name, count):
    /// one log line for each that is newly stuck, and the standing set the panel's single
    /// grouped alert is made from (#656).
    fn note_stuck(&mut self, tick: u64, stuck: &[(ItemId, String, u32)], fresh: &mut Vec<Fresh>) {
        for (item, name, count) in stuck {
            if !self.full_items.contains(item) {
                // One log line per item that goes stuck; the panel shows them as one open alert
                // (`Self::open`), so none of these is a toast of its own.
                let text = describe_stockpile_full(name, *count);
                fresh.push(((tick, STOCKPILE_ORDER), Line::new(Kind::Stockpile, text, None)));
                self.stores_since = tick;
            }
        }
        self.stores_full = stuck.iter().map(|(_, name, count)| (name.clone(), *count)).collect();
        self.full_items = stuck.iter().map(|(item, ..)| *item).collect();
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
        self.next_id += 1;
        self.log.push(Entry {
            id: self.next_id,
            tick,
            kind: Kind::Agent,
            category: Kind::Agent.category(false),
            place: None,
            unit: None,
            fold: None,
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
            category: Category,
            subject: String,
            text: String,
            count: usize,
            tick: u64,
            failed: bool,
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
                    g.failed = e.failed;
                }
                None => groups.push(Group {
                    kind: e.kind,
                    category: e.category,
                    subject: toast.subject.clone(),
                    text: toast.text.clone(),
                    count: 1,
                    tick: e.tick,
                    failed: e.failed,
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
            Kind::Chart,
            Kind::Agent,
            Kind::BendTip,
            Kind::Trade,
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
                category: g.category,
                urgency: g.kind.urgency(g.failed),
                since: g.tick,
                // A repeat coalesces into the subject plus a count (#417: "Hearthflame failed
                // ×3"); a lone occurrence keeps its own, richer text instead.
                text: if g.count > 1 { format!("{} ×{}", g.subject, g.count) } else { g.text },
            })
            .collect()
    }

    /// Everything the panel's Alerts section lists, most urgent first ([`Urgency`]), the newest
    /// first within one (#656): the standing alerts (the stockpiles that are full, grouped into
    /// one entry, and a research target that is not set), which stay open while their condition
    /// holds, then the live toasts, which age out. Notices (news, not problems) come last.
    pub fn open(&self) -> Vec<Toast> {
        let mut open: Vec<Toast> = self.toasts();
        if !self.stores_full.is_empty() {
            open.push(Toast {
                kind: Kind::Stockpile,
                category: Category::Alert,
                urgency: Urgency::Stores,
                since: self.stores_since,
                text: describe_stores_full(&self.stores_full),
            });
        }
        if let Some((since, points)) = self.research_idle {
            open.push(Toast {
                kind: Kind::Research,
                category: Category::Alert,
                urgency: Urgency::Idle,
                since,
                text: describe_research_idle(points),
            });
        }
        // Stable, and `toasts` is oldest first, so reversing first puts the newest first.
        open.reverse();
        open.sort_by_key(|t| (t.urgency, std::cmp::Reverse(t.since)));
        open
    }

    /// The open alerts: [`Alerts::open`] without its notices. They are the same lines the log's
    /// Alerts filter shows (#661), so a player who sees one in the side panel finds it there:
    /// the log has the line each was raised with (a grouped one has a line per cause).
    pub fn open_alerts(&self) -> Vec<Toast> {
        self.open().into_iter().filter(|t| t.category == Category::Alert).collect()
    }

    /// The toasts' text, oldest first.
    pub fn toast_lines(&self) -> Vec<String> {
        self.toasts().into_iter().map(|t| t.text).collect()
    }
}

/// One toast [`Alerts::toasts`] shows: already folded by kind and subject, so its text is
/// exactly what the side panel's Alerts section puts on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub kind: Kind,
    /// The message log's category of the lines it stands for (`Entry::category`).
    pub category: Category,
    /// How soon it wants an eye ([`Urgency`]); the panel lists the most urgent first.
    pub urgency: Urgency,
    /// The tick it was raised (or last repeated), so the newest sorts first within an urgency.
    pub since: u64,
    pub text: String,
}

/// One line before it is an [`Entry`]: what an event (or a standing condition) becomes.
struct Line {
    kind: Kind,
    text: String,
    /// A `Bend` that failed rather than finished.
    failed: bool,
    toast: Option<ToastSpec>,
    place: Option<Pos>,
    unit: Option<UnitId>,
    fold: Option<Fold>,
}

impl Line {
    /// A line that names no place or settler and folds with nothing.
    fn new(kind: Kind, text: String, toast: Option<ToastSpec>) -> Self {
        Self { kind, text, failed: false, toast, place: None, unit: None, fold: None }
    }

    fn at(mut self, place: Pos) -> Self {
        self.place = Some(place);
        self
    }

    fn of(mut self, unit: UnitId) -> Self {
        self.unit = Some(unit);
        self
    }

    fn folding(mut self, fold: Fold) -> Self {
        self.fold = Some(fold);
        self
    }
}

/// A line not yet in the log, with its sort key first: the tick, then `Event::order` for the
/// world's own events, or `STOCKPILE_ORDER` and `BEND_TIP_ORDER` for the standing conditions,
/// which sort after every one of those.
type Fresh = ((u64, u8), Line);

/// The lines `events` (newly logged, oldest first) become, sorted by key. The sort is stable,
/// so lines with the same key keep the order the world logged them in.
fn fresh_lines(session: &Session, events: &[Event]) -> Vec<Fresh> {
    let mut fresh: Vec<Fresh> = events
        .iter()
        .filter_map(|ev| Some(((ev.tick(), ev.order()), line_for(session, ev)?)))
        .collect();
    fresh.sort_by_key(|(key, ..)| *key);
    fresh
}

/// Where a full stockpile's line sorts among same-tick lines: after every world event
/// (`Event::order` runs 0 to 10).
const STOCKPILE_ORDER: u8 = 11;
/// Where the bend tip's line sorts among same-tick lines: last.
const BEND_TIP_ORDER: u8 = 12;
/// Where the idle-research line sorts among same-tick lines: after the bend tip.
const RESEARCH_ORDER: u8 = 13;
/// Where a caravan's leaving warning sorts among same-tick lines: last.
const TRADE_ORDER: u8 = 14;
/// How long before a caravan leaves its warning comes (#438): two minutes.
const TRADE_WARNING_TICKS: u64 = 2 * 60 * TICKS_PER_SECOND as u64;

/// The line one logged event becomes: its kind, the log's text, whether it is a failed bend, and
/// its toast (`None` for a log-only line). `None` for an event with no line (yet).
///
/// The match is exhaustive on purpose (#605): a new `Event` variant doesn't compile until it has
/// an arm here, so no kind of event can be silently left out of the alert log.
fn line_for(session: &Session, ev: &Event) -> Option<Line> {
    let defs = &session.defs;
    let toast = |text: &String| Some(ToastSpec { subject: text.clone(), text: text.clone() });
    Some(match ev {
        Event::Bend(ev) => {
            let failed = matches!(ev.outcome, BendOutcome::Failed(_));
            let text = describe_bend_event(session, ev);
            // A finished bend is routine (#377) and, since #417, log-only: with a settlement
            // that bends a lot, toasting every one is exactly the noise this issue exists to
            // cut. A failed one still says something worth a toast, coalesced by the ability
            // that failed (not who was channelling it) so several benders failing the same way
            // reads as one line with a count.
            let ability = defs.ability(ev.ability).name.clone();
            let toast = failed
                .then(|| ToastSpec { subject: format!("{ability} failed"), text: text.clone() });
            let line = Line { failed, ..Line::new(Kind::Bend, text, toast) }.at(ev.pos).of(ev.unit);
            // Only the routine ones fold: a failure says something of its own each time.
            if failed { line } else { line.folding(Fold::Bend(ability)) }
        }
        Event::Tech(c) => {
            let text = format!("Learned {}", defs.tech(c.tech).name);
            Line::new(Kind::Tech, text.clone(), toast(&text))
        }
        Event::Death(d) => {
            let line = Line::new(Kind::Death, describe_death(d), None);
            // The settler is gone, so the line has no unit to follow; the tile it died on (#775)
            // is what `Enter` jumps to. A death from before the event carried one has none.
            match d.pos {
                Some(pos) => line.at(pos),
                None => line,
            }
        }
        Event::Endeavor(ev) => {
            let text = endeavor::describe_event(defs, ev);
            Line::new(Kind::Endeavor, text.clone(), toast(&text))
        }
        Event::Arrival(a) => {
            let text = describe_arrival(a);
            Line::new(Kind::Arrival, text.clone(), toast(&text)).of(a.unit).folding(Fold::Arrival)
        }
        Event::Season(ev) => {
            let text = describe_season_event(session, ev)?;
            Line::new(Kind::Season, text.clone(), toast(&text))
        }
        Event::Strain(ev) => {
            let text = describe_strain_event(defs, ev);
            let name = defs.resource(ev.resource).name.to_lowercase();
            Line::new(Kind::Season, text, Some(strain_toast(defs, ev)))
                .at(ev.pos)
                .folding(Fold::Strain(name))
        }
        // Harm reaches the alert log here, but has no line yet: its wording is #393's.
        Event::Harm(_) => return None,
        // Caravans (#437, #438): coming and going toast; a deal or request done is log only.
        Event::Caravan(ev) => caravan_line(session, ev),
        // A beast going back to the wild (#693): the line is the event's own wording.
        Event::Beast(ev) => {
            let text = ev.describe();
            Line::new(Kind::Arrival, text.clone(), toast(&text)).at(ev.home)
        }
        Event::Chart(ev) => {
            let (text, toast) = describe_chart_event(session, ev);
            let line = Line::new(Kind::Chart, text, Some(toast));
            match ev.kind {
                ChartEventKind::Found { pos, .. } => line.at(pos),
                ChartEventKind::NoneLeft => line.at(ev.room),
            }
        }
    })
}

/// A toast that coalesces by its own text.
fn toast_of(text: &str) -> Option<ToastSpec> {
    Some(ToastSpec { subject: text.to_owned(), text: text.to_owned() })
}

/// "Fire Nation traders", from the data; "Traders" for a nation with none.
fn caravan_name(session: &Session, nation: fn_sim::Nation) -> String {
    let caravans = &session.defs.trade.caravans;
    caravans.iter().find(|c| c.nation == nation).map_or("Traders".to_owned(), |c| c.name.clone())
}

/// A caravan's line (#438, docs/design/text-voice.md): "Fire Nation traders made camp", "...
/// found nowhere to camp", "... left", and in the log only "Deal done with the Fire Nation
/// traders: 2 metal" and "Request met: Food for the families at Fire Fountain City".
fn caravan_line(session: &Session, ev: &CaravanEvent) -> Line {
    let defs = &session.defs;
    let caravan = |n| defs.trade.caravans.iter().find(|c| c.nation == n);
    match &ev.kind {
        CaravanEventKind::Arrived { nation, camp } => {
            let text = format!("{} made camp", caravan_name(session, *nation));
            Line::new(Kind::Trade, text.clone(), toast_of(&text)).at(*camp)
        }
        CaravanEventKind::Skipped { nation } => {
            let text = format!("{} found nowhere to camp", caravan_name(session, *nation));
            Line::new(Kind::Trade, text.clone(), toast_of(&text))
        }
        CaravanEventKind::Left { nation } => {
            let text = format!("{} left", caravan_name(session, *nation));
            Line::new(Kind::Trade, text.clone(), toast_of(&text))
        }
        CaravanEventKind::DealDone { nation, deal } => {
            let name = caravan_name(session, *nation);
            let got = caravan(*nation).and_then(|c| c.deals.get(usize::from(*deal))).map(|d| {
                let mut parts: Vec<String> = d
                    .get
                    .iter()
                    .map(|&(i, n)| format!("{n} {}", defs.item(i).name.to_lowercase()))
                    .collect();
                if d.knowledge > 0 {
                    parts.push(format!("{} knowledge", d.knowledge));
                }
                parts.join(", ")
            });
            let text = match got {
                Some(got) if !got.is_empty() => format!("Deal done with the {name}: {got}"),
                _ => format!("Deal done with the {name}"),
            };
            Line::new(Kind::Trade, text, None)
        }
        CaravanEventKind::RequestDone { nation, request } => {
            let req = caravan(*nation).and_then(|c| c.requests.get(usize::from(*request)));
            let text = match req {
                Some(r) => format!("Request met: {}", r.name),
                None => "Request met".to_owned(),
            };
            Line::new(Kind::Trade, text, None)
        }
    }
}

/// A finished chart (#402, #605): "Map Room charted Lost Ruins at (10, 1)", with ": out of
/// reach" when no settler could walk there when it was found, or "Map Room has nothing left to
/// chart". The toast drops the place, as every toast does (#417), and coalesces by its own text.
/// The resource is read off the map, since the event doesn't carry it; a chart read back from a
/// save after its find was used up reads "Map Room finished a chart at (10, 1)".
fn describe_chart_event(session: &Session, ev: &ChartEvent) -> (String, ToastSpec) {
    let (world, defs) = (&session.world, &session.defs);
    let room = world
        .building_at(ev.room)
        .map(|id| defs.building(id))
        .or_else(|| defs.buildings.iter().find(|b| b.chart.is_some()))
        .map_or("Map Room", |b| b.name.as_str());
    let (text, short) = match ev.kind {
        ChartEventKind::Found { pos, reachable } => {
            let reach = if reachable { "" } else { ": out of reach" };
            let short = match world.map.get(pos).and_then(|t| t.resource) {
                Some(r) => format!("{room} charted {}", defs.resource(r).name),
                None => format!("{room} finished a chart"),
            };
            (format!("{short} at ({}, {}){reach}", pos.x, pos.y), format!("{short}{reach}"))
        }
        ChartEventKind::NoneLeft => {
            let text = format!("{room} has nothing left to chart");
            (text.clone(), text)
        }
    };
    (text, ToastSpec { subject: short.clone(), text: short })
}

/// The alert line for a death: "Mei died of starvation". The web replayer's timeline markers
/// (#595) use it too, so a marker's tooltip reads like the log.
pub fn describe_death(d: &Death) -> String {
    let how = match d.cause {
        DeathCause::Starvation => "starvation",
        DeathCause::Dehydration => "dehydration",
        DeathCause::Exhaustion => "exhaustion",
        // #387, #388, #389; the full wording (and `weakened_by`) is #393's job.
        DeathCause::Exposure => "exposure",
        DeathCause::Injury => "injuries",
        DeathCause::Sickness => "sickness",
    };
    format!("{} died of {how}", d.name)
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
            // A beast has no mastery to count toward (#697).
            let threshold = unit
                .filter(|u| !u.is_beast())
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
            let people = session.world.population();
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
fn describe_stockpile_full(name: &str, count: u32) -> String {
    format!("{count} {name} can't be stored: every stockpile is full.")
}

/// The panel's one entry for every stockpile that can't take something (#656): the stuck items
/// and how much of each, in one sentence. "Stockpiles are full. 442 stone, 17 fish and 15 fruit
/// have nowhere to go."
fn describe_stores_full(items: &[(String, u32)]) -> String {
    let parts: Vec<String> = items.iter().map(|(name, n)| format!("{n} {name}")).collect();
    let verb = if matches!(items, [(_, 1)]) { "has" } else { "have" };
    format!("Stockpiles are full. {} {verb} nowhere to go.", join_and(&parts))
}

/// "a", "a and b", "a, b and c".
fn join_and(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// "No research target is set. 84 points are waiting." (#656)
fn describe_research_idle(points: u32) -> String {
    let (noun, verb) = if points == 1 { ("point", "is") } else { ("points", "are") };
    format!("No research target is set. {points} {noun} {verb} waiting.")
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
    min_cost.is_finite() && session.world.units.iter().any(|u| u.is_settler() && u.chi >= min_cost)
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
    use fn_sim::{Defs, Nation, Pos, Terrain};

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
            ..Entry::blank()
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
        run_until(&mut s, |s| s.world.bend_events().next().is_some());
        let first = describe_bend_event(&s, s.world.bend_events().next().unwrap());
        assert!(first.ends_with(", a new form"), "{first}");

        s.apply(Command::Bend { ability, pos: Pos::new(4, 1) }).unwrap();
        run_until(&mut s, |s| s.world.bend_events().count() > 1);
        let second = describe_bend_event(&s, s.world.bend_events().nth(1).unwrap());
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
        run_until(&mut s, |s| s.world.bend_events().next().is_some());
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.016);
        assert!(alerts.toast_lines().is_empty(), "{:?}", alerts.toast_lines());
        assert_eq!(alerts.history().len(), 1);
        assert!(alerts.history()[0].text.contains(&s.defs.ability(ability).name));
        // Updating again doesn't repeat it.
        alerts.update(&s, 0.016);
        assert_eq!(alerts.history().len(), 1);
    }

    /// Every entry has a category, a place and a settler where its event names them, and what it
    /// folds with, all set where the entry is made (#661): a finished bend is Bending, folds by
    /// its ability and goes to where it was done; the entries get ids that count up.
    #[test]
    fn an_entry_carries_its_category_place_settler_and_fold() {
        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let ability: AbilityId = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        let (unit, pos) = (s.world.units[0].id, s.world.units[0].pos);
        s.apply(Command::Bend { ability, pos }).unwrap();
        run_until(&mut s, |s| s.world.bend_events().next().is_some());
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.0);
        let done = &alerts.history()[0];
        assert_eq!(done.category, Category::Bending);
        assert_eq!((done.place, done.unit), (Some(pos), Some(unit)));
        assert_eq!(done.fold, Some(Fold::Bend(s.defs.ability(ability).name.clone())));
        assert_eq!(done.id, 1);
        alerts.push_agent(5, "x".to_owned());
        let agent = &alerts.history()[1];
        assert_eq!((agent.category, agent.id, agent.fold.is_none()), (Category::Work, 2, true));
    }

    /// The kinds sort under the categories the filter row names: a failed bend is an alert, a
    /// finished one is bending.
    #[test]
    fn every_kind_has_a_category() {
        assert_eq!(Kind::Bend.category(true), Category::Alert);
        assert_eq!(Kind::Bend.category(false), Category::Bending);
        assert_eq!(Kind::Death.category(false), Category::Alert);
        assert_eq!(Kind::Season.category(false), Category::Alert);
        assert_eq!(Kind::Stockpile.category(false), Category::Alert);
        assert_eq!(Kind::Endeavor.category(false), Category::Alert);
        assert_eq!(Kind::Arrival.category(false), Category::People);
        assert_eq!(Kind::Tech.category(false), Category::Knowledge);
        assert_eq!(Kind::Chart.category(false), Category::Knowledge);
        assert_eq!(Kind::Agent.category(false), Category::Work);
        assert_eq!(Kind::BendTip.category(false), Category::Work);
    }

    /// A finished bend is routine and never worth stopping `end_turn` for (#377); a failed one
    /// still is, so it needs the same attention a death or an arrival gets.
    #[test]
    fn a_finished_bend_isnt_worth_stopping_but_a_failed_one_is() {
        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let ability: AbilityId = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        let pos = s.world.units[0].pos;
        s.apply(Command::Bend { ability, pos }).unwrap();
        run_until(&mut s, |s| s.world.bend_events().next().is_some());
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
        run_until(&mut s, |s| s.world.bend_events().next().is_some());
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
            Entry {
                tick: 0,
                kind,
                text: String::new(),
                left: 0.0,
                toast: None,
                failed,
                ..Entry::blank()
            }
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
        run_until(&mut s, |s| s.world.tech_completions().next().is_some());
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
        run_until(&mut s, |s| s.world.endeavor_events().count() == 2);
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
        run_until(&mut s, |s| s.world.tech_completions().next().is_some());
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
                ..Entry::blank()
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
                ..Entry::blank()
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
        run_until(&mut s, |s| s.world.deaths().next().is_some());
        alerts.update(&s, 0.0);
        let dead = &s.world.deaths().next().unwrap();
        let death = alerts.history().iter().find(|e| e.kind == Kind::Death).expect("logged");
        assert!(death.text.starts_with(&format!("{} died of ", dead.name)), "{}", death.text);
        assert_eq!(death.tick, dead.tick);
        assert!(alerts.toasts().iter().all(|e| e.kind != Kind::Death));
    }

    /// A death line names the tile the settler died on (#775), so `Enter` on it in the message
    /// log goes there; one read back from before the event carried a tile names none.
    #[test]
    fn a_death_line_names_where_the_settler_died() {
        let mut s = Session::new(7, Nation::Earth, 48, 32);
        s.world.units[1].needs.hunger = 0.0;
        s.world.units[1].needs.health = 0.01;
        run_until(&mut s, |s| s.world.deaths().next().is_some());
        let dead = s.world.deaths().next().unwrap().clone();
        let at = dead.pos.expect("the death records its tile");
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.0);
        let death = alerts.history().iter().find(|e| e.kind == Kind::Death).expect("logged");
        assert_eq!((death.place, death.unit), (Some(at), None));
        assert_eq!(death.text, describe_death(&dead), "the text is unchanged");

        let old = Death { pos: None, ..dead };
        let line = line_for(&s, &Event::Death(old)).unwrap();
        assert_eq!(line.place, None);
    }

    #[test]
    fn a_new_world_clears_the_log() {
        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let ability = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        let pos = s.world.units[0].pos;
        s.apply(Command::Bend { ability, pos }).unwrap();
        run_until(&mut s, |s| s.world.bend_events().next().is_some());
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
        let stockpile = |a: &Alerts| {
            a.history().iter().filter(|e| e.kind == Kind::Stockpile).cloned().collect::<Vec<_>>()
        };
        assert_eq!(stockpile(&alerts).len(), 1, "{:?}", alerts.history());
        let entry = &stockpile(&alerts)[0];
        assert_eq!(entry.kind, Kind::Stockpile);
        let open: Vec<_> =
            alerts.open().into_iter().filter(|t| t.kind == Kind::Stockpile).collect();
        assert_eq!(open.len(), 1, "one grouped entry open: {open:?}");
        assert!(open[0].text.starts_with("Stockpiles are full. "), "{}", open[0].text);
        assert!(entry.text.contains("wood"), "{}", entry.text);
        assert!(entry.text.contains("full"), "{}", entry.text);
        assert!(entry.worth_stopping(), "worth an agent's attention (#337)");

        // Staying stuck for another stretch of ticks doesn't add a second line.
        for _ in 0..200 {
            s.tick();
            alerts.update(&s, 0.0);
        }
        assert_eq!(stockpile(&alerts).len(), 1);
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
                ..Entry::blank()
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
            ..Entry::blank()
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
                ..Entry::blank()
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

    /// A session over `fixtures/v3_events.ron` (#605): a version 3 save with something in each
    /// of the nine old event lists, several of them on the same tick (600).
    fn v3_fixture_session() -> Session {
        let defs = Defs::bundled();
        let text = include_str!("../../sim/tests/fixtures/v3_events.ron");
        let world = fn_sim::World::load(text, &defs).unwrap();
        Session { world, defs, clock: crate::clock::Clock::default(), bend_choice: None }
    }

    /// The alert log a version 3 save gives reads exactly as it did before the nine event lists
    /// became one (#605): these lines were printed by the build before the change, from the same
    /// fixture. Same-tick lines keep the old order (bends, techs, deaths, endeavor, arrivals,
    /// seasons, strain). The chart lines are new, and sort after those on their tick.
    #[test]
    fn an_old_saves_alerts_read_as_they_did_before_the_one_event_log() {
        let mut s = v3_fixture_session();
        let mut a = Alerts::default();
        a.update(&s, 0.0);
        s.tick();
        a.update(&s, 0.0);
        let lines: Vec<String> = a
            .history()
            .iter()
            .filter(|e| e.kind != Kind::Chart)
            .map(|e| format!("{} {:?} {}", e.tick, e.kind, e.text))
            .collect();
        let before = [
            "60 Season Winter in 3 minutes: 0 food for 3 people.",
            "120 Bend Lan used Raise Earth at (1, 5) - bend 0 of 8",
            "300 Tech Learned Pottery",
            "450 Endeavor Sign met: Everyone has a bed",
            "600 Bend Lan's Raise Earth failed: the ground there is gone",
            "600 Bend Lan used Raise Earth at (3, 5) - bend 0 of 8",
            "600 Tech Learned Carpentry",
            "600 Death Kiluk died of starvation",
            "600 Endeavor Sign met: Population",
            "600 Arrival Pema has joined the settlement",
            "600 Season Winter. Wild plants rest, and the cold tires people away from fires.",
            "600 Season A stone outcrop at (9, 6) was picked to death.",
            "840 Bend Lan used Raise Earth at (4, 5) - bend 0 of 8",
        ];
        assert_eq!(lines, before);
        let all: Vec<(u64, Kind)> = a.history().iter().map(|e| (e.tick, e.kind)).collect();
        let at = |tick, kind| all.iter().position(|&e| e == (tick, kind)).unwrap();
        assert_eq!(at(600, Kind::Chart), at(600, Kind::Season) + 2, "after the strain line");
        assert_eq!(at(720, Kind::Chart), at(840, Kind::Bend) - 1);
    }

    /// Same-tick events become lines in the old fixed order (`Event::order`), whatever order the
    /// sim logged them in: a death is logged before the bends of that tick (needs run before
    /// units act), but its line still comes after them.
    #[test]
    fn same_tick_lines_follow_the_old_order_not_the_log_order() {
        use fn_sim::research::TechCompleted;
        use fn_sim::world::{Arrival, Death};

        let s = v3_fixture_session();
        let defs = &s.defs;
        let unit = s.world.units[0].id;
        let tech = defs.tech_id("pottery").unwrap();
        let ability = ability_for_nation(defs, Nation::Earth).unwrap();
        let tick = 5;
        let logged = [
            Event::Chart(ChartEvent { tick, room: Pos::new(6, 6), kind: ChartEventKind::NoneLeft }),
            Event::Death(Death {
                id: unit,
                name: "Kiluk".into(),
                tick,
                cause: DeathCause::Exhaustion,
                weakened_by: None,
                pos: None,
            }),
            Event::Arrival(Arrival { tick, unit, name: "Pema".into() }),
            Event::Bend(BendEvent {
                tick,
                unit,
                ability,
                pos: Pos::new(1, 1),
                outcome: BendOutcome::Done,
                moved: None,
            }),
            Event::Tech(TechCompleted { tech, tick }),
        ];
        let kinds: Vec<Kind> = fresh_lines(&s, &logged).into_iter().map(|l| l.1.kind).collect();
        assert_eq!(kinds, [Kind::Bend, Kind::Tech, Kind::Death, Kind::Arrival, Kind::Chart]);
    }

    /// Every kind of event reaches the alert log (#605). `line_for`'s exhaustive match is what
    /// makes a new `Event` variant fail to compile without an arm; this checks what each arm
    /// gives. Harm has no line until #393 words it.
    #[test]
    fn every_kind_of_event_reaches_the_alert_log() {
        let s = v3_fixture_session();
        let w = &s.world;
        let lines: Vec<Option<Kind>> =
            w.events().iter().map(|e| line_for(&s, e).map(|l| l.kind)).collect();
        assert_eq!(lines.len(), w.events().len());
        for (ev, line) in w.events().iter().zip(&lines) {
            let want = match ev {
                Event::Bend(_) => Some(Kind::Bend),
                Event::Tech(_) => Some(Kind::Tech),
                Event::Death(_) => Some(Kind::Death),
                Event::Endeavor(_) => Some(Kind::Endeavor),
                Event::Arrival(_) => Some(Kind::Arrival),
                Event::Season(_) | Event::Strain(_) => Some(Kind::Season),
                Event::Harm(_) => None,
                Event::Caravan(_) => Some(Kind::Trade),
                Event::Chart(_) => Some(Kind::Chart),
                Event::Beast(_) => Some(Kind::Arrival),
            };
            assert_eq!(*line, want, "{ev:?}");
        }
        assert!(w.harm_events().next().is_some() && w.chart_events().next().is_some());
    }

    /// Caravans (#438): made camp and left toast, two minutes before leaving warns once, and a
    /// deal done goes to the log only. None of them stops an agent's turn.
    #[test]
    fn a_caravan_makes_camp_warns_before_it_leaves_and_leaves() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let src = include_str!("../../../data/trade.toml")
            .replacen("opens = \"known_on_the_road\"", "", 1)
            .replacen("offset_minutes = 4.0", "offset_minutes = 0.1", 1);
        s.defs = s.defs.clone().with_trade(&src).unwrap();
        let mut alerts = Alerts::default();
        let texts = |s: &mut Session, alerts: &mut Alerts, ticks: u64| {
            for _ in 0..ticks {
                s.tick();
                alerts.update(s, 0.0);
            }
            alerts.history().iter().map(|e| (e.kind, e.text.clone())).collect::<Vec<_>>()
        };
        let log = texts(&mut s, &mut alerts, 100);
        let camp = log.iter().find(|(_, t)| t == "Fire Nation traders made camp");
        assert_eq!(camp.map(|(k, _)| *k), Some(Kind::Trade), "{log:?}");
        assert!(alerts.toasts().iter().any(|t| t.text == "Fire Nation traders made camp"));
        // Stay 9 minutes: the warning comes at 7, once.
        let log = texts(&mut s, &mut alerts, 7 * 600);
        let warned = |log: &[(Kind, String)]| {
            log.iter().filter(|(_, t)| t == "Fire Nation traders leave in two minutes").count()
        };
        assert_eq!(warned(&log), 1, "{log:?}");
        let log = texts(&mut s, &mut alerts, 2 * 600 + 10);
        assert_eq!(warned(&log), 1, "still one");
        assert!(log.iter().any(|(_, t)| t == "Fire Nation traders left"), "{log:?}");
        assert!(alerts.history().iter().all(|e| e.kind != Kind::Trade || !e.worth_stopping()));
    }

    #[test]
    fn a_deal_done_is_a_log_line_naming_what_came() {
        let s = Session::new(7, Nation::Earth, 64, 64);
        let fire = s.defs.trade.caravans.iter().position(|c| c.nation == Nation::Fire).unwrap();
        let deal = s.defs.trade.caravans[fire].deals.iter().position(|d| d.knowledge == 0);
        let ev = CaravanEvent {
            tick: 0,
            kind: CaravanEventKind::DealDone { nation: Nation::Fire, deal: deal.unwrap() as u8 },
        };
        let line = caravan_line(&s, &ev);
        assert!(line.text.starts_with("Deal done with the Fire Nation traders: "), "{}", line.text);
        assert!(line.toast.is_none(), "log only");
        let ev = CaravanEvent {
            tick: 0,
            kind: CaravanEventKind::RequestDone { nation: Nation::Fire, request: 0 },
        };
        assert_eq!(
            caravan_line(&s, &ev).text,
            "Request met: Food for the families at Fire Fountain City"
        );
    }

    /// A finished chart's line (#605): the building's and the resource's names from the data,
    /// the place in the log's own copy only, and ": out of reach" when no settler could walk
    /// there. A room with nothing left says so. None of them stops an agent's turn.
    #[test]
    fn a_finished_chart_gets_a_line() {
        let mut s = Session::new(1, Nation::Earth, 48, 32);
        let ruins = s.defs.resource_id("lost_ruins").unwrap();
        let (found, room) = (Pos::new(10, 1), Pos::new(6, 6));
        s.world.map[found].resource = Some(ruins);
        let line = |s: &Session, kind| {
            let line = line_for(s, &Event::Chart(ChartEvent { tick: 0, room, kind }))
                .expect("a chart has a line");
            assert_eq!(line.kind, Kind::Chart);
            (line.text, line.toast.expect("a chart toasts").text)
        };
        let reachable = ChartEventKind::Found { pos: found, reachable: true };
        assert_eq!(
            line(&s, reachable),
            ("Map Room charted Lost Ruins at (10, 1)".into(), "Map Room charted Lost Ruins".into())
        );
        let unreachable = ChartEventKind::Found { pos: found, reachable: false };
        assert_eq!(
            line(&s, unreachable),
            (
                "Map Room charted Lost Ruins at (10, 1): out of reach".into(),
                "Map Room charted Lost Ruins: out of reach".into()
            )
        );
        assert_eq!(
            line(&s, ChartEventKind::NoneLeft),
            (
                "Map Room has nothing left to chart".into(),
                "Map Room has nothing left to chart".into()
            )
        );
        // A find that has since been used up still names the place.
        s.world.map[found].resource = None;
        assert_eq!(line(&s, reachable).0, "Map Room finished a chart at (10, 1)");
        let entry = Entry {
            tick: 0,
            kind: Kind::Chart,
            text: String::new(),
            left: 0.0,
            toast: None,
            failed: false,
            ..Entry::blank()
        };
        assert!(!entry.worth_stopping());
    }

    /// Three items stuck at once (#656): the message log keeps a line for each, but the panel
    /// has one open alert that lists them all, and it stays one entry as the counts change.
    #[test]
    fn full_stockpiles_are_one_open_alert_but_one_log_line_per_item() {
        let s = Session::new(7, Nation::Earth, 48, 32);
        let stuck: Vec<(ItemId, String, u32)> = [("stone", 442), ("fish", 17), ("fruit", 15)]
            .into_iter()
            .map(|(id, n)| (s.defs.item_id(id).unwrap(), id.to_owned(), n))
            .collect();
        let mut alerts = Alerts::default();
        let mut fresh = Vec::new();
        alerts.note_stuck(100, &stuck, &mut fresh);
        assert_eq!(fresh.len(), 3, "one line per item for the log");

        let open = alerts.open();
        assert_eq!(open.len(), 1, "{open:?}");
        assert_eq!(
            open[0].text,
            "Stockpiles are full. 442 stone, 17 fish and 15 fruit have nowhere to go."
        );
        assert_eq!((open[0].kind, open[0].category), (Kind::Stockpile, Category::Alert));

        // The same items on the next call add no lines, and the counts are the live ones.
        let more: Vec<_> = stuck.iter().map(|(i, n, c)| (*i, n.clone(), c + 1)).collect();
        let mut fresh = Vec::new();
        alerts.note_stuck(101, &more, &mut fresh);
        assert!(fresh.is_empty());
        assert!(alerts.open()[0].text.contains("443 stone, 18 fish and 16 fruit"));

        // Nothing stuck any more: the alert closes.
        alerts.note_stuck(102, &[], &mut fresh);
        assert!(alerts.open().is_empty());
    }

    #[test]
    fn the_grouped_stockpile_alert_reads_for_one_item_and_for_two() {
        let one = [("wood".to_owned(), 42)];
        assert_eq!(describe_stores_full(&one), "Stockpiles are full. 42 wood have nowhere to go.");
        let single = [("wood".to_owned(), 1)];
        assert_eq!(describe_stores_full(&single), "Stockpiles are full. 1 wood has nowhere to go.");
        let two = [("wood".to_owned(), 4), ("fish".to_owned(), 2)];
        assert_eq!(
            describe_stores_full(&two),
            "Stockpiles are full. 4 wood and 2 fish have nowhere to go."
        );
    }

    /// Research points banked with no target is an open alert; setting a target closes it
    /// (#656). The log gets one line when it is raised, not one a frame.
    #[test]
    fn banked_research_with_no_target_is_an_alert_until_a_target_is_set() {
        let mut s = Session::new(7, Nation::Earth, 48, 32);
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.0);
        assert!(alerts.open().is_empty(), "nothing banked yet");
        run_until(&mut s, |s| s.world.research().pool_millis() >= MILLIS_PER_POINT);
        alerts.update(&s, 0.0);
        let open = alerts.open_alerts();
        assert_eq!(open.len(), 1, "{open:?}");
        assert_eq!(open[0].kind, Kind::Research);
        assert!(open[0].text.starts_with("No research target is set. "), "{}", open[0].text);
        assert!(open[0].text.ends_with(" waiting."), "{}", open[0].text);
        let lines = |a: &Alerts| a.history().iter().filter(|e| e.kind == Kind::Research).count();
        assert_eq!(lines(&alerts), 1);
        assert_eq!(alerts.history()[0].category, Category::Alert);
        assert!(!alerts.history()[0].worth_stopping());
        for _ in 0..50 {
            s.tick();
            alerts.update(&s, 0.0);
        }
        assert_eq!(lines(&alerts), 1, "staying idle doesn't log again");

        let shelter = s.defs.tech_id("shelter").unwrap();
        s.apply(Command::SetResearch { tech: shelter }).unwrap();
        alerts.update(&s, 0.0);
        assert!(alerts.open().is_empty(), "a target is set: {:?}", alerts.open());
        assert_eq!(lines(&alerts), 1, "the log keeps what it said");
    }

    #[test]
    fn open_alerts_come_most_urgent_first_then_newest_first_with_notices_last() {
        let s = Session::new(7, Nation::Earth, 48, 32);
        let mut alerts = Alerts::default();
        alerts.update(&s, 0.0);
        let toast = |kind: Kind, text: &str, tick: u64| Entry {
            tick,
            kind,
            category: kind.category(true),
            text: text.to_owned(),
            left: 5.0,
            toast: Some(ToastSpec { subject: text.to_owned(), text: text.to_owned() }),
            failed: true,
            ..Entry::blank()
        };
        alerts.log.push(toast(Kind::Tech, "Learned Shelter", 50));
        alerts.log.push(toast(Kind::Bend, "Haru's Raise Earth failed", 10));
        alerts.log.push(toast(Kind::Bend, "Mei's Raise Earth failed", 20));
        alerts.research_idle = Some((30, 84));
        alerts.stores_full = vec![("stone".to_owned(), 5)];
        alerts.stores_since = 5;
        let order: Vec<Kind> = alerts.open().iter().map(|t| t.kind).collect();
        assert_eq!(
            order,
            [Kind::Stockpile, Kind::Research, Kind::Bend, Kind::Bend, Kind::Tech],
            "stores, then idle capacity (newest first), then the news"
        );
        let texts: Vec<String> = alerts.open().into_iter().map(|t| t.text).collect();
        assert_eq!(texts[2], "Mei's Raise Earth failed");
        assert_eq!(alerts.open_alerts().len(), 4, "the learned tech is a notice, not an alert");
    }
}

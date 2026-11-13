//! Corner notices (#945): the rare, important events that stay on screen until the player
//! dismisses them, and the contextual actions a notice (or an ordinary toast) offers.
//!
//! This is view state and frontend-agnostic: [`Notices`] lives in [`crate::alerts::Alerts`],
//! which makes them from the same lines it logs, and a frontend draws [`Notices::shown`] in a
//! corner of the map with a button for each [`NoticeAction`] and a close button. Nothing here
//! is saved and nothing in the sim knows about it.
//!
//! A frontend that draws notices turns them on ([`Notices::set_on`]). Until then none are made,
//! so the terminal, which has no corner to put them in, keeps today's fading toasts for the same
//! events; when they are on, an event that is pinned as a notice no longer toasts as well.

use fn_sim::Pos;
use fn_sim::world::UnitId;

use crate::alerts::Kind;

/// The most notices drawn at once, newest first; the rest wait behind them as a "+N more" row.
pub const MAX_NOTICES: usize = 4;
/// The most notices kept; a newer one beyond this pushes the oldest off (it is still in the
/// message log).
pub const MAX_KEPT: usize = 12;

/// Something a notice or a toast can do for the player, as a button: it takes them to what the
/// event is about. [`crate::app::App::do_notice_action`] carries it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum NoticeAction {
    /// Centres the map on a caravan's camp.
    JumpToCaravan(Pos),
    /// Centres the map on a place.
    JumpTo(Pos),
    /// Selects a settler (and centres the map on them).
    SelectSettler(UnitId),
    /// Opens the research screen.
    OpenResearch,
    /// Opens the Traders screen.
    OpenTraders,
}

impl NoticeAction {
    /// The button's words.
    pub fn label(self) -> &'static str {
        match self {
            Self::JumpToCaravan(_) => "Jump to caravan",
            Self::JumpTo(_) => "Jump there",
            Self::SelectSettler(_) => "Show settler",
            Self::OpenResearch => "Open research",
            Self::OpenTraders => "Trade [M]",
        }
    }
}

/// Which notices stand for one thing, so a newer one replaces an older (a caravan's leaving
/// warning replaces its arrival) or folds into it (more deaths).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// The caravan camped now: it arrives, then is about to leave, then is gone.
    Caravan,
    /// A settler died.
    Death,
}

/// One notice on screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    /// Names it for as long as it is up: what a close button hands back to [`Notices::dismiss`].
    pub id: u64,
    pub kind: Kind,
    /// The world tick it was raised (or last repeated).
    pub tick: u64,
    /// Its heading ("Fire Nation traders", "Research done: Ice Construction"): no map
    /// coordinates, as a toast's.
    pub title: String,
    /// What it says under the heading, a line each.
    pub body: Vec<String>,
    /// Its buttons, in order.
    pub actions: Vec<NoticeAction>,
    group: Option<Group>,
    /// How many like it have folded into it (1 for a lone one).
    count: u32,
}

/// The notices up now. Off until a frontend that draws them turns it on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notices {
    on: bool,
    /// Oldest first.
    items: Vec<Notice>,
    next_id: u64,
}

impl Notice {
    /// The heading and the body on one line, for a log or a test.
    pub fn text(&self) -> String {
        std::iter::once(self.title.as_str())
            .chain(self.body.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl Notices {
    /// Whether notices are being made (a frontend draws them).
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Turns notices on or off. Turning them off drops the ones up.
    pub fn set_on(&mut self, on: bool) {
        self.on = on;
        if !on {
            self.items.clear();
        }
    }

    /// Raises a notice. One of the same `group` already up is replaced, except that deaths fold
    /// into one ("Katara died (and 2 more)") whose buttons are the latest's. Over [`MAX_KEPT`]
    /// the oldest goes.
    pub fn push(
        &mut self,
        (kind, tick): (Kind, u64),
        (title, body): (String, Vec<String>),
        actions: Vec<NoticeAction>,
        group: Option<Group>,
    ) {
        let mut count = 1;
        if let Some(g) = group
            && let Some(at) = self.items.iter().position(|n| n.group == Some(g))
        {
            let old = self.items.remove(at);
            if g == Group::Death {
                count += old.count;
            }
        }
        self.next_id += 1;
        let title = match count {
            1 => title,
            n => format!("{title} (and {} more)", n - 1),
        };
        self.items.push(Notice {
            id: self.next_id,
            kind,
            tick,
            title,
            body,
            actions,
            group,
            count,
        });
        if self.items.len() > MAX_KEPT {
            self.items.remove(0);
        }
    }

    /// Takes down the notices of `group` (the caravan has left: there is nothing to jump to).
    pub fn retire(&mut self, group: Group) {
        self.items.retain(|n| n.group != Some(group));
    }

    /// The notices to draw, newest first (the top of the stack): at most [`MAX_NOTICES`].
    pub fn shown(&self) -> impl Iterator<Item = &Notice> {
        self.items.iter().rev().take(MAX_NOTICES)
    }

    /// How many more are up than [`Self::shown`] draws.
    pub fn hidden(&self) -> usize {
        self.items.len().saturating_sub(MAX_NOTICES)
    }

    /// Takes every notice down.
    pub fn dismiss_all(&mut self) {
        self.items.clear();
    }

    /// Whether any is up.
    pub fn any(&self) -> bool {
        !self.items.is_empty()
    }

    /// Takes down the notice `id`; whether there was one.
    pub fn dismiss(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|n| n.id != id);
        self.items.len() != before
    }

    /// Takes down the newest notice (the key); whether there was one.
    pub fn dismiss_newest(&mut self) -> bool {
        self.items.pop().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> Notices {
        let mut n = Notices::default();
        n.set_on(true);
        n
    }

    fn face(title: &str) -> (String, Vec<String>) {
        (title.to_owned(), Vec::new())
    }

    fn titles(n: &Notices) -> Vec<&str> {
        n.shown().map(|n| n.title.as_str()).collect()
    }

    #[test]
    fn they_stack_newest_first_and_the_rest_wait_behind() {
        let mut n = on();
        for i in 0..MAX_NOTICES + 2 {
            n.push((Kind::Tech, i as u64), face(&format!("Learned {i}")), vec![], None);
        }
        let shown = titles(&n);
        assert_eq!(shown.len(), MAX_NOTICES);
        assert_eq!(shown[0], format!("Learned {}", MAX_NOTICES + 1), "newest on top");
        assert_eq!(shown[MAX_NOTICES - 1], "Learned 2");
        assert_eq!(n.hidden(), 2, "two more behind them");
        n.dismiss_newest();
        assert_eq!(n.hidden(), 1, "dismissing one brings one up");
        n.dismiss_all();
        assert!(!n.any() && n.hidden() == 0);
    }

    #[test]
    fn the_oldest_goes_beyond_what_is_kept() {
        let mut n = on();
        for i in 0..MAX_KEPT + 3 {
            n.push((Kind::Tech, i as u64), face(&format!("T{i}")), vec![], None);
        }
        assert_eq!(n.hidden(), MAX_KEPT - MAX_NOTICES);
    }

    #[test]
    fn dismissing_takes_one_down_by_id_or_the_newest() {
        let mut n = on();
        for (tick, title) in [(1, "a"), (2, "b"), (3, "c")] {
            n.push((Kind::Tech, tick), face(title), vec![], None);
        }
        let b = n.shown().nth(1).unwrap().id;
        assert!(n.dismiss(b));
        assert!(!n.dismiss(b), "already gone");
        assert!(n.dismiss_newest());
        assert_eq!(titles(&n), ["a"]);
        assert!(n.dismiss_newest() && !n.dismiss_newest());
    }

    #[test]
    fn a_group_replaces_its_earlier_notice_and_deaths_fold() {
        let mut n = on();
        n.push((Kind::Trade, 1), face("camp"), vec![], Some(Group::Caravan));
        n.push(
            (Kind::Trade, 2),
            ("traders".into(), vec!["leave soon".into()]),
            vec![],
            Some(Group::Caravan),
        );
        assert_eq!(n.shown().map(Notice::text).collect::<Vec<_>>(), ["traders leave soon"]);
        n.retire(Group::Caravan);
        assert!(!n.any());
        for (tick, title) in [(3, "Mei died"), (4, "Sokka died"), (5, "Katara died")] {
            n.push((Kind::Death, tick), face(title), vec![], Some(Group::Death));
        }
        assert_eq!(titles(&n), ["Katara died (and 2 more)"]);
    }

    #[test]
    fn turning_them_off_drops_them() {
        let mut n = on();
        n.push((Kind::Tech, 1), face("a"), vec![], None);
        n.set_on(false);
        assert!(!n.any() && !n.is_on());
    }
}

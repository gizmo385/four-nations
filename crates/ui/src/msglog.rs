//! The message log as a player reads it (#661): newest first, a run of the same event folded
//! into one line, one category at a time.
//!
//! Everything here is a *view* of [`Alerts::history`](crate::alerts::Alerts::history): the
//! history itself stays one entry per event, oldest first, and so does the run log an agent
//! reads. [`lines`] turns it into the lines the screen shows, in the order it shows them;
//! [`step`] moves a cursor over them; the frontend draws them and acts on `Enter`.
//!
//! **Folding.** A finished bend, a newcomer, or a wild plant picked to death folds with the
//! earlier lines of its own [`Fold`] when it comes within [`FOLD_GAP_SECS`] of the last of them,
//! within [`FOLD_SPAN_SECS`] of the first, and within [`FOLD_RADIUS`] tiles of the first when
//! both name a place. Other lines may sit between the members: the fold goes where its newest
//! member is. A fold of one is just that line.

use fn_sim::Pos;
use fn_sim::world::{TICKS_PER_SECOND, UnitId};

use crate::alerts::{Category, Entry, Fold, Kind};

/// Two lines fold only if they are at most this many game seconds apart (the run's last line to
/// the next).
pub const FOLD_GAP_SECS: u64 = 60;
/// A fold never reaches over more than this many game seconds, first line to last.
pub const FOLD_SPAN_SECS: u64 = 300;
/// Lines that each name a place fold only if it is within this many tiles (either way) of the
/// run's first.
pub const FOLD_RADIUS: i32 = 12;
/// How many of the most recent runs a line looks through for one to join.
const SEARCH: usize = 32;

/// Which line of the log the cursor is on. Held as the line's id, not its row, so a new line
/// arriving at the top does not move it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineKey {
    /// The entry's id ([`Entry::id`]); for a fold, its first line's.
    pub id: u64,
    /// Whether this is one of an open fold's own lines rather than the fold or a plain line.
    pub sub: bool,
}

/// One line of the log as shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub key: LineKey,
    /// When it happened: the newest of a fold's lines.
    pub tick: u64,
    pub kind: Kind,
    pub category: Category,
    /// What it says: a fold's own sentence, else the entry's text.
    pub text: String,
    /// How many entries it stands for: 1 for a plain line (and for each line of an open fold).
    pub count: usize,
    /// Whether it is a fold that is open: its lines follow it.
    pub open: bool,
    /// The place `Enter` goes to; for a fold, the middle of its lines' places.
    pub place: Option<Pos>,
    /// The settler `Enter` selects when the line has no place.
    pub unit: Option<UnitId>,
}

impl Line {
    /// Whether this line folds several entries (and so opens and closes).
    pub fn is_fold(&self) -> bool {
        self.count > 1 && !self.key.sub
    }
}

/// The lines of the log, newest first: the history under `filter` (`None`: all of it), with runs
/// folded, and the folds whose first entry's id is in `open` followed by their own lines, newest
/// first, as `sub` lines.
pub fn lines(history: &[Entry], filter: Option<Category>, open: &[u64]) -> Vec<Line> {
    let gap = FOLD_GAP_SECS * u64::from(TICKS_PER_SECOND);
    let span = FOLD_SPAN_SECS * u64::from(TICKS_PER_SECOND);
    let mut runs: Vec<Vec<usize>> = Vec::new();
    for (i, e) in history.iter().enumerate() {
        if filter.is_some_and(|c| e.category != c) {
            continue;
        }
        let joins = e.fold.as_ref().and_then(|_| {
            // Only the most recent runs are looked through.
            let from = runs.len().saturating_sub(SEARCH);
            (from..runs.len()).rev().find(|&r| joins(history, &runs[r], e, gap, span))
        });
        match joins {
            Some(r) => runs[r].push(i),
            None => runs.push(vec![i]),
        }
    }
    // Newest first, by each run's newest member.
    runs.sort_by_key(|run| std::cmp::Reverse(run[run.len() - 1]));
    let mut out = Vec::new();
    for run in &runs {
        let first = &history[run[0]];
        let last = &history[run[run.len() - 1]];
        if run.len() == 1 {
            out.push(plain(last, false));
            continue;
        }
        let open = open.contains(&first.id);
        out.push(Line {
            key: LineKey { id: first.id, sub: false },
            tick: last.tick,
            kind: last.kind,
            category: last.category,
            text: folded_text(history, run),
            count: run.len(),
            open,
            place: middle(history, run),
            unit: last.unit,
        });
        if open {
            out.extend(run.iter().rev().map(|&i| plain(&history[i], true)));
        }
    }
    out
}

fn plain(e: &Entry, sub: bool) -> Line {
    Line {
        key: LineKey { id: e.id, sub },
        tick: e.tick,
        kind: e.kind,
        category: e.category,
        text: e.text.clone(),
        count: 1,
        open: false,
        place: e.place,
        unit: e.unit,
    }
}

/// Whether `e` folds into `run` (indices into `history`, oldest first).
fn joins(history: &[Entry], run: &[usize], e: &Entry, gap: u64, span: u64) -> bool {
    let first = &history[run[0]];
    let last = &history[run[run.len() - 1]];
    if first.fold != e.fold {
        return false;
    }
    if e.tick.saturating_sub(last.tick) > gap || e.tick.saturating_sub(first.tick) > span {
        return false;
    }
    match (first.place, e.place) {
        (Some(a), Some(b)) => (a.x - b.x).abs().max((a.y - b.y).abs()) <= FOLD_RADIUS,
        _ => true,
    }
}

/// The middle of the places the run's lines name, rounded: roughly where.
fn middle(history: &[Entry], run: &[usize]) -> Option<Pos> {
    let places: Vec<Pos> = run.iter().filter_map(|&i| history[i].place).collect();
    if places.is_empty() {
        return None;
    }
    let n = places.len() as i32;
    let (x, y) = places.iter().fold((0, 0), |(x, y), p| (x + p.x, y + p.y));
    Some(Pos::new((x + n / 2) / n, (y + n / 2) / n))
}

/// A fold's sentence: what happened, how many times, by how many settlers, and near where.
fn folded_text(history: &[Entry], run: &[usize]) -> String {
    let n = run.len();
    let near = middle(history, run).map_or(String::new(), |p| format!(", near ({}, {})", p.x, p.y));
    let first = &history[run[0]];
    match first.fold.as_ref() {
        Some(Fold::Bend(ability)) => {
            let mut who: Vec<UnitId> = run.iter().filter_map(|&i| history[i].unit).collect();
            who.sort_by_key(|u| u.0);
            who.dedup();
            let benders = match who.len() {
                0 | 1 => "1 bender".to_owned(),
                k => format!("{k} benders"),
            };
            format!("{ability} used {n} times by {benders}{near}")
        }
        Some(Fold::Arrival) => format!("{n} settlers have joined the settlement"),
        Some(Fold::Strain(name)) => {
            let mut chars = name.chars();
            let name: String =
                chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect());
            format!("{name} picked to death {n} times{near}")
        }
        None => first.text.clone(),
    }
}

/// Where `at` is in `lines`: its index, or the first line if it is not there (a line that
/// dropped off the history, or is hidden by the filter).
pub fn index_of(lines: &[Line], at: Option<LineKey>) -> usize {
    at.and_then(|k| lines.iter().position(|l| l.key == k)).unwrap_or(0)
}

/// The key of the line `dy` lines (its sign times `by`) below `at`, stopping at the ends. `None`
/// for an empty log.
pub fn step(lines: &[Line], at: Option<LineKey>, dy: i64) -> Option<LineKey> {
    if lines.is_empty() {
        return None;
    }
    let to = (index_of(lines, at) as i64 + dy).clamp(0, lines.len() as i64 - 1) as usize;
    Some(lines[to].key)
}

/// The log's filter after the digit `key` (`1` to `5`, `0` for all): the same category again
/// goes back to all.
pub fn filter_after(filter: Option<Category>, key: char) -> Option<Option<Category>> {
    if key == '0' {
        return Some(None);
    }
    let picked = Category::from_key(key)?;
    Some(if filter == Some(picked) { None } else { Some(picked) })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TPS: u64 = TICKS_PER_SECOND as u64;

    fn entry(id: u64, secs: u64, kind: Kind, text: &str) -> Entry {
        let mut e = Entry::blank();
        e.id = id;
        e.tick = secs * TPS;
        e.kind = kind;
        e.category = kind.category(false);
        e.text = text.to_owned();
        e
    }

    fn bend(id: u64, secs: u64, unit: u32, x: i32, y: i32) -> Entry {
        let mut e =
            entry(id, secs, Kind::Bend, &format!("Unit {unit} used Raise Earth at ({x}, {y})"));
        e.place = Some(Pos::new(x, y));
        e.unit = Some(UnitId(unit));
        e.fold = Some(Fold::Bend("Raise Earth".to_owned()));
        e
    }

    fn mixed() -> Vec<Entry> {
        let mut h = vec![entry(1, 0, Kind::Season, "Winter in 3 minutes: 10 food for 7 people.")];
        for (i, unit) in [1u32, 2, 3, 4, 5, 6, 1].into_iter().enumerate() {
            h.push(bend(2 + i as u64, 10 + 5 * i as u64, unit, 50 + i as i32, 65));
        }
        let mut learned = entry(9, 50, Kind::Tech, "Learned Pottery");
        learned.category = Category::Knowledge;
        h.push(learned);
        let mut arrival = entry(10, 52, Kind::Arrival, "An has joined the settlement");
        arrival.unit = Some(UnitId(9));
        h.push(arrival);
        h
    }

    #[test]
    fn the_newest_line_is_first() {
        let l = lines(&mixed(), None, &[]);
        assert_eq!(l[0].text, "An has joined the settlement");
        assert_eq!(l[1].text, "Learned Pottery");
        assert_eq!(l.last().unwrap().kind, Kind::Season);
    }

    /// Seven bends within a minute are one line: how many, by how many settlers, roughly where.
    #[test]
    fn seven_bends_in_a_minute_are_one_line() {
        let l = lines(&mixed(), None, &[]);
        let folds: Vec<&Line> = l.iter().filter(|l| l.is_fold()).collect();
        assert_eq!(folds.len(), 1, "{l:#?}");
        assert_eq!(folds[0].count, 7);
        assert_eq!(folds[0].text, "Raise Earth used 7 times by 6 benders, near (53, 65)");
        assert_eq!(folds[0].place, Some(Pos::new(53, 65)));
        // The season line, the fold, the tech, the arrival.
        assert_eq!(l.len(), 4);
    }

    #[test]
    fn right_opens_a_fold_into_its_lines_and_left_folds_it_again() {
        let h = mixed();
        let fold_id = lines(&h, None, &[])[2].key.id;
        let open = lines(&h, None, &[fold_id]);
        assert_eq!(open.len(), 4 + 7);
        assert!(open[2].open);
        // Newest first, indented as sub lines.
        assert!(open[3].key.sub && open[3].text.contains("Unit 1 used"));
        assert!(open[3..10].iter().all(|l| l.key.sub && l.count == 1));
        assert_eq!(lines(&h, None, &[]).len(), 4);
    }

    #[test]
    fn bends_far_apart_in_time_or_place_do_not_fold() {
        let mut h = vec![bend(1, 0, 1, 10, 10), bend(2, 5, 2, 11, 10)];
        // Too far away, though close in time.
        h.push(bend(3, 8, 3, 60, 60));
        // Too long after the last of the first run.
        h.push(bend(4, 100, 1, 10, 10));
        let l = lines(&h, None, &[]);
        assert_eq!(l.len(), 3, "{l:#?}");
        assert_eq!(l.iter().filter(|l| l.is_fold()).count(), 1);
    }

    #[test]
    fn a_run_goes_where_its_newest_member_is_with_other_lines_between() {
        let h = vec![
            bend(1, 0, 1, 10, 10),
            entry(2, 5, Kind::Tech, "Learned X"),
            bend(3, 10, 2, 10, 10),
        ];
        let l = lines(&h, None, &[]);
        assert_eq!(l.len(), 2);
        assert!(l[0].is_fold(), "the fold sits where its newest member is");
        assert_eq!(l[1].text, "Learned X");
    }

    #[test]
    fn a_filter_shows_one_category_and_folds_within_it() {
        let h = mixed();
        let alerts = lines(&h, Some(Category::Alert), &[]);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].kind, Kind::Season);
        let bending = lines(&h, Some(Category::Bending), &[]);
        assert_eq!(bending.len(), 1);
        assert_eq!(bending[0].count, 7);
        let people = lines(&h, Some(Category::People), &[]);
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].unit, Some(UnitId(9)));
        assert!(lines(&h, Some(Category::Work), &[]).is_empty());
        assert_eq!(lines(&h, None, &[]).len(), 4);
    }

    #[test]
    fn a_digit_picks_a_category_and_the_same_digit_or_zero_shows_all() {
        assert_eq!(filter_after(None, '1'), Some(Some(Category::Alert)));
        assert_eq!(filter_after(None, '5'), Some(Some(Category::Work)));
        assert_eq!(filter_after(Some(Category::Alert), '1'), Some(None));
        assert_eq!(filter_after(Some(Category::Alert), '3'), Some(Some(Category::Knowledge)));
        assert_eq!(filter_after(Some(Category::Alert), '0'), Some(None));
        assert_eq!(filter_after(None, '6'), None);
        assert_eq!(filter_after(None, 'x'), None);
        for c in Category::ALL {
            assert_eq!(Category::from_key(c.key()), Some(c));
        }
    }

    #[test]
    fn the_cursor_steps_by_id_and_stops_at_the_ends() {
        let l = lines(&mixed(), None, &[]);
        let top = step(&l, None, 0).unwrap();
        assert_eq!(top, l[0].key);
        assert_eq!(step(&l, Some(top), -1), Some(top));
        let down = step(&l, Some(top), 1).unwrap();
        assert_eq!(down, l[1].key);
        assert_eq!(step(&l, Some(down), 100), Some(l[3].key));
        assert_eq!(step(&[], None, 1), None);
        // A key that is gone reads as the top.
        assert_eq!(index_of(&l, Some(LineKey { id: 999, sub: false })), 0);
    }

    #[test]
    fn newcomers_close_together_fold_with_no_place() {
        let mut h = Vec::new();
        for i in 0..3u32 {
            let mut e = entry(i as u64 + 1, 10 * i as u64, Kind::Arrival, "x joined");
            e.fold = Some(Fold::Arrival);
            e.unit = Some(UnitId(i));
            h.push(e);
        }
        let l = lines(&h, None, &[]);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].text, "3 settlers have joined the settlement");
        assert_eq!(l[0].place, None);
        assert_eq!(l[0].unit, Some(UnitId(2)));
    }
}

//! The one way the game tells the time (#654, owner decision 2026-11-10): the season, the year
//! and the time left in the season ("Spring Y2 · 15:00 left"), on the panel, the Nation header,
//! the game menu, the saves list and the message log alike. Never minutes since the start, so a
//! log line can be set against the panel's own clock.
//!
//! Seasons are a world setting (`Knobs::seasons`). With them off there is no season to name, so
//! the same shape runs on days instead ("Day 3 · 2:10 left"); a day is [`DAY_SECS`] of game time,
//! a presentation unit only (the sim has no day).
//!
//! Run logs and scorecards keep their own tick-based times (`fn_runfmt`); this is player-facing
//! text only.

use fn_sim::Defs;
use fn_sim::world::TICKS_PER_SECOND;

use crate::session::Session;

/// How long a "day" lasts in game seconds, with seasons off (five minutes: three to a season of
/// the bundled data). Presentation only.
pub const DAY_SECS: u64 = 300;

/// A point in game time, ready to be worded.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct GameTime {
    /// Which stretch it falls in: a season of a year, or a day.
    pub period: Period,
    /// In-game seconds left in that stretch.
    pub left_secs: f32,
}

/// The stretch of game time a [`GameTime`] falls in. Two points share a log heading exactly when
/// their periods are equal.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Period {
    Season { name: String, year: u64 },
    Day(u64),
}

/// The time at `tick`: by season and year when `seasons` is on, else by day.
pub fn game_time(defs: &Defs, tick: u64, seasons: bool) -> GameTime {
    if seasons {
        let at = defs.seasons.at(tick);
        let name = defs.seasons.seasons.get(at.season).map_or_else(String::new, |s| s.name.clone());
        return GameTime {
            period: Period::Season { name, year: at.year },
            left_secs: at.left_secs,
        };
    }
    let per = DAY_SECS * u64::from(TICKS_PER_SECOND);
    GameTime {
        period: Period::Day(tick / per + 1),
        left_secs: (per - tick % per) as f32 / TICKS_PER_SECOND as f32,
    }
}

/// The session's time now.
pub fn session_time(session: &Session) -> GameTime {
    let seasons = session.world.knobs(&session.defs).seasons;
    game_time(&session.defs, session.world.tick, seasons)
}

/// Seconds as "4:10 left" (minutes with no leading zero, seconds zero-padded):
/// `docs/design/difficulty.md` ("TUI"), the Season section's heading.
pub fn left_text(secs: f32) -> String {
    let secs = secs.max(0.0).round() as u64;
    format!("{}:{:02} left", secs / 60, secs % 60)
}

impl GameTime {
    /// "Spring Y2" or "Day 3".
    pub fn label(&self) -> String {
        match &self.period {
            Period::Season { name, year } => format!("{name} Y{year}"),
            Period::Day(n) => format!("Day {n}"),
        }
    }

    /// "15:00 left": the time left in the stretch, also a log line's stamp.
    pub fn left(&self) -> String {
        left_text(self.left_secs)
    }

    /// The whole clock: "Spring Y2 · 15:00 left".
    pub fn text(&self) -> String {
        self.text_with("·")
    }

    /// The whole clock with `dot` between the two halves ("Spring Y2 - 15:00 left"): the one
    /// place the clock's words are put together, so a frontend with an ASCII mode picks its own
    /// separator and nothing else.
    pub fn text_with(&self, dot: &str) -> String {
        format!("{} {dot} {}", self.label(), self.left())
    }

    /// A log heading's words, upper case: "WINTER, YEAR 1" or "DAY 3".
    pub fn heading(&self) -> String {
        match &self.period {
            Period::Season { name, year } => format!("{}, YEAR {year}", name.to_uppercase()),
            Period::Day(n) => format!("DAY {n}"),
        }
    }
}

/// The clock text for the session: every screen's one clock ("Spring Y2 · 15:00 left").
pub fn clock_text(session: &Session) -> String {
    session_time(session).text()
}

/// The clock text at `tick` against `defs` alone, for a save slot: by season when `seasons` is
/// on, else by day, as the game's panel read when it was saved (#766). The halves are joined with
/// a plain `-`, since a slot's description is plain ASCII for every frontend.
pub fn clock_text_at(defs: &Defs, tick: u64, seasons: bool) -> String {
    game_time(defs, tick, seasons).text_with("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TPS: u64 = TICKS_PER_SECOND as u64;

    #[test]
    fn a_fresh_game_is_at_the_start_of_spring_year_one() {
        let defs = Defs::bundled();
        let t = game_time(&defs, 0, true);
        assert_eq!(t.text(), "Spring Y1 · 15:00 left");
        assert_eq!(t.heading(), "SPRING, YEAR 1");
        assert_eq!(t.text_with("-"), "Spring Y1 - 15:00 left");
    }

    #[test]
    fn the_time_left_counts_down_and_rolls_into_the_next_season_and_year() {
        let defs = Defs::bundled();
        let per = defs.seasons.season_ticks();
        assert_eq!(game_time(&defs, 10 * TPS, true).left(), "14:50 left");
        assert_eq!(game_time(&defs, per, true).text(), "Summer Y1 · 15:00 left");
        assert_eq!(game_time(&defs, 4 * per, true).text(), "Spring Y2 · 15:00 left");
        assert_eq!(game_time(&defs, 3 * per + 1, true).heading(), "WINTER, YEAR 1");
    }

    #[test]
    fn with_seasons_off_the_same_shape_runs_on_days() {
        let defs = Defs::bundled();
        assert_eq!(game_time(&defs, 0, false).text(), "Day 1 · 5:00 left");
        assert_eq!(game_time(&defs, (DAY_SECS + 10) * TPS, false).text(), "Day 2 · 4:50 left");
        assert_eq!(game_time(&defs, 0, false).heading(), "DAY 1");
    }

    #[test]
    fn left_reads_minutes_and_zero_padded_seconds() {
        assert_eq!(left_text(250.0), "4:10 left");
        assert_eq!(left_text(0.0), "0:00 left");
        assert_eq!(left_text(900.0), "15:00 left");
    }
}

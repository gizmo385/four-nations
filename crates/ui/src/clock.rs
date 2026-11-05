//! The session clock: pause, speed, and how many ticks a stretch of real time is worth.
//!
//! `fn_sim` never sees real time. This only decides *how many* `World::tick`s run between inputs.
//! It reads no clock: the caller passes elapsed seconds in, so it is as testable as the sim.
//! Speeds are pacing, not balance, so they are constants here rather than data.

use fn_sim::world::TICKS_PER_SECOND;

/// The speeds, as multiples of real time. The first is the default.
pub const SPEEDS: [u32; 4] = [1, 2, 3, 5];

/// With `--fast` (a host setting, never an agent capability, #321): the normal ladder plus x10,
/// x20 and x50, for a session someone wants to run much faster than any real player would sit
/// through (docs/design/agent-play.md, "Speed"). Agents already run as fast as the CPU allows
/// between turns (`fn_tui::surface::Host`), so this is for a human watching or playing solo.
pub const FAST_SPEEDS: [u32; 7] = [1, 2, 3, 5, 10, 20, 50];

/// The most ticks one `advance` call returns at the normal ladder's top speed ([`SPEEDS`]'s
/// last). Any debt beyond that is dropped, so a slow machine (or a frame that took a second)
/// runs the game slower instead of spiralling into ever longer frames. [`max_ticks_per_frame`]
/// scales this up for faster speeds.
pub const MAX_TICKS_PER_FRAME: u32 = 20;

/// The most ticks one `advance` call may release at `speed`, scaled so a fast speed (`--fast`'s
/// x10/x20/x50, #321, or a watcher's own pace, #319) really runs that many times over on a
/// machine that keeps up, while a machine that doesn't still drops debt rather than spiralling
/// into an ever-longer frame: [`MAX_TICKS_PER_FRAME`] at [`SPEEDS`]'s top speed (5) and
/// proportionally more above it, unchanged at or below it.
pub fn max_ticks_per_frame(speed: u32) -> u32 {
    let top = SPEEDS[SPEEDS.len() - 1];
    MAX_TICKS_PER_FRAME * speed.div_ceil(top).max(1)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clock {
    /// While paused, `advance` returns 0 and only `step` runs a tick.
    pub paused: bool,
    /// Index into the ladder ([`SPEEDS`] or, with `fast`, [`FAST_SPEEDS`]).
    speed: usize,
    /// The fraction of a tick owed to the sim, in `[0, 1)`.
    owed: f64,
    /// `--fast` (#321): [`FAST_SPEEDS`] instead of [`SPEEDS`]. A host or session setting, read
    /// nowhere in `fn_sim` and set by nothing an agent can reach.
    fast: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self { paused: false, speed: 0, owed: 0.0, fast: false }
    }
}

impl Clock {
    /// `--fast`'s ladder instead of the normal one (#321). The pause and the speed index stay as
    /// they were, so turning this on or off never itself changes the running speed's rank.
    pub fn with_fast(mut self, fast: bool) -> Self {
        self.fast = fast;
        self
    }

    /// Whether this clock uses the fast ladder ([`FAST_SPEEDS`]).
    pub fn is_fast(&self) -> bool {
        self.fast
    }

    fn ladder(&self) -> &'static [u32] {
        if self.fast { &FAST_SPEEDS } else { &SPEEDS }
    }

    /// The current speed as a multiple of real time: from [`SPEEDS`], or (`--fast`) from
    /// [`FAST_SPEEDS`].
    pub fn speed(&self) -> u32 {
        self.ladder()[self.speed]
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// The next speed up; stays at the fastest.
    pub fn faster(&mut self) {
        self.speed = (self.speed + 1).min(self.ladder().len() - 1);
    }

    /// The next speed down; stays at the slowest.
    pub fn slower(&mut self) {
        self.speed = self.speed.saturating_sub(1);
    }

    /// Ticks to run for `real_secs` seconds of wall time: 0 while paused, else what is owed plus
    /// `real_secs * speed * TICKS_PER_SECOND`, rounded down to whole ticks and capped at
    /// [`max_ticks_per_frame`] of the current speed. The fraction left over is carried to the
    /// next call.
    pub fn advance(&mut self, real_secs: f64) -> u32 {
        if self.paused {
            return 0;
        }
        let owed =
            self.owed + real_secs.max(0.0) * f64::from(self.speed()) * f64::from(TICKS_PER_SECOND);
        let whole = owed.floor();
        let cap = f64::from(max_ticks_per_frame(self.speed()));
        if whole >= cap {
            self.owed = 0.0;
            return cap as u32;
        }
        self.owed = owed - whole;
        whole as u32
    }

    /// While paused, exactly one tick (the single-step key); otherwise 0, since time is already
    /// running.
    pub fn step(&mut self) -> u32 {
        u32::from(self.paused)
    }

    /// For a frontend whose own scheduler already fires once per tick period: the ticks to run
    /// for that one firing. 0 while paused, else the speed, so at the default speed it is one
    /// tick a firing.
    pub fn per_fixed_step(&self) -> u32 {
        if self.paused { 0 } else { self.speed() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TPS: f64 = TICKS_PER_SECOND as f64;

    #[test]
    fn starts_running_at_the_first_speed() {
        let c = Clock::default();
        assert!(!c.paused);
        assert_eq!(c.speed(), 1);
    }

    #[test]
    fn one_second_at_speed_one_is_one_seconds_worth_of_ticks() {
        let mut c = Clock::default();
        assert_eq!(c.advance(1.0), TICKS_PER_SECOND);
    }

    #[test]
    fn paused_runs_no_ticks_and_keeps_the_debt_it_had() {
        let mut c = Clock::default();
        assert_eq!(c.advance(0.25 / TPS), 0, "a quarter tick is owed");
        c.toggle_pause();
        assert_eq!(c.advance(5.0), 0);
        c.toggle_pause();
        assert_eq!(c.advance(0.75 / TPS), 1, "the quarter owed before the pause is still there");
    }

    #[test]
    fn step_runs_one_tick_only_while_paused() {
        let mut c = Clock::default();
        assert_eq!(c.step(), 0);
        c.toggle_pause();
        assert_eq!(c.step(), 1);
        assert_eq!(c.step(), 1);
    }

    #[test]
    fn speed_multiplies_the_ticks_and_stops_at_both_ends() {
        let mut c = Clock::default();
        c.slower();
        assert_eq!(c.speed(), 1, "already the slowest");
        for want in [2, 3, 5, 5] {
            c.faster();
            assert_eq!(c.speed(), want);
        }
        assert_eq!(c.advance(1.0 / TPS), 5, "one tick period at x5");
        for want in [3, 2, 1, 1] {
            c.slower();
            assert_eq!(c.speed(), want);
        }
    }

    #[test]
    fn a_frame_is_capped_and_the_debt_beyond_the_cap_is_dropped() {
        let mut c = Clock::default();
        assert_eq!(c.advance(60.0), MAX_TICKS_PER_FRAME);
        // Nothing carried over: the next short frame is worth its own ticks only.
        assert_eq!(c.advance(0.0), 0);
        assert_eq!(c.advance(2.0 / TPS), 2);
    }

    /// Without `--fast`, the ladder is still just `[1, 2, 3, 5]` (#321's acceptance criteria).
    #[test]
    fn fast_adds_x10_x20_and_x50_above_the_normal_ladder() {
        let mut plain = Clock::default();
        for _ in 0..10 {
            plain.faster();
        }
        assert_eq!(plain.speed(), 5, "unchanged without --fast");

        let mut fast = Clock::default().with_fast(true);
        assert!(fast.is_fast());
        let mut seen = vec![fast.speed()];
        for _ in 0..6 {
            fast.faster();
            seen.push(fast.speed());
        }
        assert_eq!(seen, vec![1, 2, 3, 5, 10, 20, 50]);
        for _ in 0..3 {
            fast.faster();
        }
        assert_eq!(fast.speed(), 50, "stays at the fastest");
    }

    /// `with_fast` alone doesn't change the clock's running speed, only which ladder its index
    /// reads from.
    #[test]
    fn turning_fast_on_or_off_keeps_the_same_rank_in_the_ladder() {
        let mut c = Clock::default();
        c.faster();
        c.faster();
        assert_eq!(c.speed(), 3, "third rung of the plain ladder");
        let c = c.with_fast(true);
        assert_eq!(c.speed(), 3, "same rank, the fast ladder's third rung");
    }

    /// At a fast speed, a machine that keeps up (one frame's worth of real time at 30 fps) gets
    /// the full multiple instead of being capped away (#321's acceptance criteria).
    #[test]
    fn a_machine_that_keeps_up_gets_the_full_fast_multiple() {
        let mut c = Clock::default().with_fast(true);
        for _ in 0..6 {
            c.faster();
        }
        assert_eq!(c.speed(), 50);
        // 1/30 s at x50 (500 ticks/s) owes 16.67 ticks: comfortably under the scaled cap.
        assert_eq!(c.advance(1.0 / 30.0), 16);
    }

    /// A machine that falls behind still drops debt rather than spiralling, even at a fast speed
    /// (#321's acceptance criteria): `max_ticks_per_frame` caps it well short of the ticks a
    /// whole slow second would otherwise owe.
    #[test]
    fn a_slow_machine_drops_debt_rather_than_spiralling_at_a_fast_speed() {
        let mut c = Clock::default().with_fast(true);
        for _ in 0..6 {
            c.faster();
        }
        assert_eq!(max_ticks_per_frame(50), 200);
        assert_eq!(c.advance(1.0), 200, "500 ticks were owed; only the scaled cap runs");
        assert_eq!(c.advance(0.0), 0, "the dropped debt isn't carried");
    }

    /// The cap is unchanged at and below the normal ladder's top speed, so every existing
    /// (non-fast) behaviour is untouched.
    #[test]
    fn the_cap_is_unchanged_at_normal_speeds() {
        for speed in SPEEDS {
            assert_eq!(max_ticks_per_frame(speed), MAX_TICKS_PER_FRAME, "{speed}");
        }
        assert_eq!(max_ticks_per_frame(10), MAX_TICKS_PER_FRAME * 2);
        assert_eq!(max_ticks_per_frame(20), MAX_TICKS_PER_FRAME * 4);
        assert_eq!(max_ticks_per_frame(50), MAX_TICKS_PER_FRAME * 10);
    }

    #[test]
    fn the_fraction_of_a_tick_is_carried_between_frames() {
        // 0.125 s is 1.25 ticks at 10 ticks a second, and both are exact in binary, so the
        // carried quarter-tick shows up as a second tick on the fourth frame.
        const { assert!(TICKS_PER_SECOND == 10) };
        let mut c = Clock::default();
        let frames: Vec<u32> = (0..4).map(|_| c.advance(0.125)).collect();
        assert_eq!(frames, [1, 1, 1, 2]);
        assert_eq!(c.owed, 0.0);
    }

    #[test]
    fn negative_elapsed_time_is_ignored() {
        let mut c = Clock::default();
        assert_eq!(c.advance(-3.0), 0);
        assert_eq!(c.advance(1.0 / TPS), 1);
    }

    #[test]
    fn a_fixed_step_is_one_tick_at_the_default_speed_and_none_while_paused() {
        let mut c = Clock::default();
        assert_eq!(c.per_fixed_step(), 1);
        c.faster();
        assert_eq!(c.per_fixed_step(), 2);
        c.toggle_pause();
        assert_eq!(c.per_fixed_step(), 0);
    }
}

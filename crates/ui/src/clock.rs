//! The session clock: pause, speed, and how many ticks a stretch of real time is worth.
//!
//! `fn_sim` never sees real time. This only decides *how many* `World::tick`s run between inputs.
//! It reads no clock: the caller passes elapsed seconds in, so it is as testable as the sim.
//! Speeds are pacing, not balance, so they are constants here rather than data.

use fn_sim::world::TICKS_PER_SECOND;

/// The speeds, as multiples of real time. The first is the default.
pub const SPEEDS: [u32; 4] = [1, 2, 3, 5];

/// The most ticks one `advance` call returns. Any debt beyond that is dropped, so a slow machine
/// (or a frame that took a second) runs the game slower instead of spiralling into ever longer
/// frames.
pub const MAX_TICKS_PER_FRAME: u32 = 20;

#[derive(Clone, Debug, PartialEq)]
pub struct Clock {
    /// While paused, `advance` returns 0 and only `step` runs a tick.
    pub paused: bool,
    /// Index into [`SPEEDS`].
    speed: usize,
    /// The fraction of a tick owed to the sim, in `[0, 1)`.
    owed: f64,
}

impl Default for Clock {
    fn default() -> Self {
        Self { paused: false, speed: 0, owed: 0.0 }
    }
}

impl Clock {
    /// The current speed as a multiple of real time: 1, 2, 3 or 5.
    pub fn speed(&self) -> u32 {
        SPEEDS[self.speed]
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// The next speed up; stays at the fastest.
    pub fn faster(&mut self) {
        self.speed = (self.speed + 1).min(SPEEDS.len() - 1);
    }

    /// The next speed down; stays at the slowest.
    pub fn slower(&mut self) {
        self.speed = self.speed.saturating_sub(1);
    }

    /// Ticks to run for `real_secs` seconds of wall time: 0 while paused, else what is owed plus
    /// `real_secs * speed * TICKS_PER_SECOND`, rounded down to whole ticks and capped at
    /// [`MAX_TICKS_PER_FRAME`]. The fraction left over is carried to the next call.
    pub fn advance(&mut self, real_secs: f64) -> u32 {
        if self.paused {
            return 0;
        }
        let owed =
            self.owed + real_secs.max(0.0) * f64::from(self.speed()) * f64::from(TICKS_PER_SECOND);
        let whole = owed.floor();
        if whole >= f64::from(MAX_TICKS_PER_FRAME) {
            self.owed = 0.0;
            return MAX_TICKS_PER_FRAME;
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

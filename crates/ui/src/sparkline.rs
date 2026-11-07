//! The agent studio's acts-per-turn sparkline (docs/design/agent-studio.md, "The sparkline and
//! the strip"): a plain scaling function over a window of turn counts, no drawing. A frontend
//! maps each level to a glyph (`▁▂▃▄▅▆▇█`, or `_.:-=+*#` with `--ascii`) and dims level 0.
//!
//! Levels, not glyphs, live here: the ramp is presentation (ASCII vs Unicode), while how `n`
//! acts in a turn scales against the window's own peak is a rule every frontend should apply the
//! same way, and is worth testing on its own.

/// How many non-zero levels the ramp has (`▂`..`█`, or `.`..`#`): one each, picked by
/// [`levels`].
pub const MAX_LEVEL: u8 = 7;

/// One level per turn in `acts`, 0..=[`MAX_LEVEL`] (docs/design/agent-studio.md, "Scale"): 0 for
/// a turn with no acts at all (a frontend dims this one rather than giving it its own shade), and
/// otherwise `ceil(n / max(peak, floor) * MAX_LEVEL)`, clamped to `[1, MAX_LEVEL]` -- so with
/// `floor` above the window's own peak (a quiet window), even the busiest turn in it doesn't
/// fill the bar.
pub fn levels(acts: &[u16], floor: u16) -> Vec<u8> {
    let peak = acts.iter().copied().max().unwrap_or(0);
    let scale_floor = peak.max(floor).max(1);
    acts.iter()
        .map(|&n| {
            if n == 0 {
                0
            } else {
                let frac = f64::from(n) / f64::from(scale_floor);
                (frac * f64::from(MAX_LEVEL)).ceil().clamp(1.0, f64::from(MAX_LEVEL)) as u8
            }
        })
        .collect()
}

/// The window's own peak (the largest single turn's acts), for the strip's own `… N` after the
/// bars. `0` for an empty window.
pub fn peak(acts: &[u16]) -> u16 {
    acts.iter().copied().max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_window_has_no_levels_and_no_peak() {
        assert_eq!(levels(&[], 4), Vec::<u8>::new());
        assert_eq!(peak(&[]), 0);
    }

    #[test]
    fn a_turn_with_no_acts_is_level_zero() {
        assert_eq!(levels(&[0, 0], 4), vec![0, 0]);
    }

    #[test]
    fn the_busiest_turn_in_the_window_fills_the_bar() {
        // peak 7, above the floor: 7 / 7 == 1.0 -> the top level.
        assert_eq!(levels(&[0, 2, 7], 4), vec![0, 2, MAX_LEVEL]);
    }

    /// With the window's peak below `floor`, even the busiest turn doesn't fill the bar: the
    /// design's own "a quiet turn with one act doesn't fill the bar".
    #[test]
    fn a_quiet_window_is_scaled_against_the_floor_not_its_own_peak() {
        // peak 1, floor 4: scale_floor is 4, so n=1 -> ceil(1/4 * 7) = ceil(1.75) = 2, not 7.
        assert_eq!(levels(&[1, 1], 4), vec![2, 2]);
        assert_eq!(peak(&[1, 1]), 1);
    }

    #[test]
    fn levels_scale_proportionally_between_zero_and_the_peak() {
        // peak 7 (above the floor of 4), scale_floor is 7.
        assert_eq!(levels(&[1, 3, 7], 4), vec![1, 3, 7]);
    }

    #[test]
    fn a_higher_floor_flattens_every_bar_further() {
        let low_floor = levels(&[1, 2, 3], 1);
        let high_floor = levels(&[1, 2, 3], 20);
        for (low, high) in low_floor.iter().zip(high_floor.iter()) {
            assert!(high <= low, "{low_floor:?} vs {high_floor:?}");
        }
    }
}

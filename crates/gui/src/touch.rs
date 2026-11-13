//! Touch on the map (#977, docs/design/touch-pwa.md §3): what a finger's press means, beyond what
//! egui already makes of it. A tap is a click and a one-finger drag is a mouse drag, so the
//! map's mouse rules ([`crate::mapview`], `fn_ui::pointer`) cover those. This adds the rest:
//!
//! | Gesture | Means |
//! |---|---|
//! | Two fingers down | pan and pinch-zoom ([`Frame::multi`]); a rectangle being drawn is dropped ([`Frame::cancel`]) |
//! | A press that rests [`Gestures::LONG_PRESS`] s within [`Gestures::SLOP`] pt | the tile's tooltip until the finger lifts ([`Frame::long_press`]) |
//! | The release after either | swallowed for that frame and the next ([`Frame::swallow_release`]): no click, no one-tile apply |
//!
//! Once a press has had two fingers it stays a two-finger press until every finger lifts, so the
//! finger left behind after a pinch doesn't start a rectangle or pan by itself.
//!
//! Pure: the caller hands in what egui saw this frame (touching or not, the primary position,
//! the number of fingers, the frame time), so a test steps it with scripted touches.

use eframe::egui::Pos2;

/// One press of the finger, from the first touch down to the last one up.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Press {
    /// Where the finger went down: the first position seen while it is down.
    start: Option<Pos2>,
    at: f64,
    moved: bool,
    multi: bool,
    long: bool,
}

/// What this frame's touches mean for the map.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    /// The press has (or had) two fingers: they pan and zoom, and the one-finger rules are off.
    pub multi: bool,
    /// The second finger came down this frame: drop a rectangle in progress.
    pub cancel: bool,
    /// A long press is held here: show this point's tooltip.
    pub long_press: Option<Pos2>,
    /// The fingers lifted this frame after a long press or a two-finger press: whatever click
    /// the release makes is not a click.
    pub swallow_release: bool,
}

/// The touch state between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gestures {
    press: Option<Press>,
    /// The frame after a swallowed release swallows too: egui can report the click a frame
    /// after the finger lifts.
    swallow_next: bool,
}

impl Gestures {
    /// How long a still finger rests before it is a long press, in seconds.
    pub const LONG_PRESS: f64 = 0.5;
    /// How far a long press may wander and still be one, in points.
    pub const SLOP: f32 = 10.0;

    /// This frame: `touching` is whether any finger is down, `pos` the primary finger's
    /// position, `fingers` how many are down, `now` the frame time in seconds. A mouse is never
    /// `touching`, so with one everything here stays off.
    pub fn update(&mut self, touching: bool, pos: Option<Pos2>, fingers: usize, now: f64) -> Frame {
        if !touching {
            let lifted = self.press.take().is_some_and(|p| p.long || p.multi);
            let swallow = lifted || std::mem::take(&mut self.swallow_next);
            self.swallow_next = lifted;
            return Frame { swallow_release: swallow, ..Frame::default() };
        }
        self.swallow_next = false;
        let press = self.press.get_or_insert(Press {
            start: None,
            at: now,
            moved: false,
            multi: false,
            long: false,
        });
        let mut cancel = false;
        if fingers >= 2 && !press.multi {
            press.multi = true;
            press.long = false;
            cancel = true;
        }
        if let Some(p) = pos {
            let start = *press.start.get_or_insert(p);
            if p.distance(start) > Self::SLOP {
                press.moved = true;
            }
        }
        if !press.moved && !press.multi && now - press.at >= Self::LONG_PRESS {
            press.long = true;
        }
        Frame {
            multi: press.multi,
            cancel,
            long_press: if press.long { pos.or(press.start) } else { None },
            swallow_release: false,
        }
    }

    /// Whether the press under way is a long press (so a drag past the threshold isn't one).
    pub fn long_pressing(&self) -> bool {
        self.press.is_some_and(|p| p.long)
    }
}

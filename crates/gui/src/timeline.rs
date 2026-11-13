//! The replay's controls and timeline (#998), what the canvas page's footer did: play and pause,
//! the pace, a step, the next turn, the pane buttons, and a scrubber with the turns worth finding
//! again marked on it.
//!
//! The markers come from [`Scan`] (`fn_tui::replay::scan`), a second replay of the whole log
//! that reads the world's events as it goes. The page ran it in a Web Worker; here it runs in
//! slices of each frame ([`SCAN_BUDGET`]), so it never stalls the replay being watched, and the
//! markers appear as it reaches them. A shaded stretch of the bar shows how far it has got.
//!
//! Every control is the key it stands for (the watcher's own), or a seek: [`Timeline::show`]
//! returns [`Action`]s for [`crate::replay::ReplayApp`] to carry out, so nothing here touches the
//! replay.

use web_time::Instant;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, Sense, Stroke, Ui, pos2, vec2,
};
use fn_tui::replay::Replay;
use fn_tui::replay::scan::{Marker, MarkerKind, Scan};

/// How much of each frame the scan may take, in seconds.
pub const SCAN_BUDGET: f64 = 0.004;

/// The bar's height, the markers' row above it included, in points.
pub const BAR_HEIGHT: f32 = 26.0;

/// Every kind, in the order the legend lists them, with its label and colour (the canvas page's).
pub const KINDS: [(MarkerKind, &str, Color32); 6] = [
    (MarkerKind::Death, "Deaths", Color32::from_rgb(0xe0, 0x6c, 0x6c)),
    (MarkerKind::Milestone, "Milestones", Color32::from_rgb(0xe5, 0xc0, 0x7b)),
    (MarkerKind::Avatar, "Avatar", Color32::from_rgb(0xc9, 0x9b, 0xe6)),
    (MarkerKind::Learned, "Learned", Color32::from_rgb(0x61, 0xaf, 0xef)),
    (MarkerKind::Arrival, "Arrivals", Color32::from_rgb(0x98, 0xc3, 0x79)),
    (MarkerKind::Script, "Script changes", Color32::from_rgb(0x56, 0xb6, 0xc2)),
];

const ACCENT: Color32 = Color32::from_rgb(0x6f, 0xb3, 0xd2);
const TRACK: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);

/// The colour `kind` is drawn in.
pub fn color(kind: MarkerKind) -> Color32 {
    KINDS.iter().find(|k| k.0 == kind).map_or(Color32::GRAY, |k| k.2)
}

/// What a click on the controls asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// The watcher key it stands for (`Space`, `+`, `-`, `.`, `]`, `0`-`3`).
    Key(char),
    /// Go to the end of this turn.
    Seek(u32),
}

/// The timeline's own state: the scan and which kinds are hidden.
pub struct Timeline {
    scan: Option<Scan>,
    /// Why the scan could not run, if it could not.
    error: Option<String>,
    hidden: Vec<MarkerKind>,
    /// The turn the scrubber is being dragged to, until it is let go.
    dragging: Option<u32>,
}

impl Timeline {
    /// The timeline of the log `text`, its scan at the start.
    pub fn new(text: &str) -> Self {
        match Scan::new(text) {
            Ok(scan) => Self { scan: Some(scan), error: None, hidden: Vec::new(), dragging: None },
            Err(e) => Self { scan: None, error: Some(e), hidden: Vec::new(), dragging: None },
        }
    }

    /// Scans for up to `budget` seconds. `true` while there is more to scan.
    pub fn scan_for(&mut self, budget: f64) -> bool {
        let Some(scan) = &mut self.scan else { return false };
        let started = Instant::now();
        while scan.step() {
            if started.elapsed().as_secs_f64() >= budget {
                return true;
            }
        }
        false
    }

    /// The whole log has been scanned (or the scan could not run).
    pub fn scanned(&self) -> bool {
        self.scan.as_ref().is_none_or(Scan::done)
    }

    /// Every marker found so far.
    pub fn markers(&self) -> &[Marker] {
        self.scan.as_ref().map_or(&[], Scan::markers)
    }

    /// Hides `kind`'s markers, or shows them again.
    pub fn toggle(&mut self, kind: MarkerKind) {
        match self.hidden.iter().position(|&k| k == kind) {
            Some(i) => {
                self.hidden.remove(i);
            }
            None => self.hidden.push(kind),
        }
    }

    pub fn hidden(&self, kind: MarkerKind) -> bool {
        self.hidden.contains(&kind)
    }

    /// The ticks on the bar: one a kind a turn, shown kinds only, in the order found.
    pub fn ticks(&self) -> Vec<(u32, MarkerKind)> {
        let mut out: Vec<(u32, MarkerKind)> = Vec::new();
        for m in self.markers() {
            let tick = (m.turn, m.kind);
            if !self.hidden(m.kind) && !out.contains(&tick) {
                out.push(tick);
            }
        }
        out
    }

    /// The shown markers at `turn`.
    pub fn events_at(&self, turn: u32) -> Vec<&Marker> {
        self.markers().iter().filter(|m| m.turn == turn && !self.hidden(m.kind)).collect()
    }

    /// The controls, two rows: the buttons, the pace, the turn and the legend, then the bar.
    pub fn show(&mut self, ui: &mut Ui, replay: &Replay) -> Vec<Action> {
        let mut actions = Vec::new();
        ui.horizontal(|ui| {
            let play = if replay.paused() { "Play replay" } else { "Pause replay" };
            if ui.button(play).on_hover_text("Space").clicked() {
                actions.push(Action::Key(' '));
            }
            if ui.button("−").on_hover_text("-: slower").clicked() {
                actions.push(Action::Key('-'));
            }
            ui.monospace(format!("×{}", replay.pace()));
            if ui.button("+").on_hover_text("+: faster").clicked() {
                actions.push(Action::Key('+'));
            }
            if ui.button("Step").on_hover_text(".: the next key").clicked() {
                actions.push(Action::Key('.'));
            }
            if ui.button("Next turn").on_hover_text("]").clicked() {
                actions.push(Action::Key(']'));
            }
            ui.separator();
            for (key, label) in [('1', "Code"), ('2', "Game"), ('3', "Feed"), ('0', "Studio")] {
                if ui.button(label).on_hover_text(key.to_string()).clicked() {
                    actions.push(Action::Key(key));
                }
            }
            ui.separator();
            let shown = self.dragging.unwrap_or(replay.turn());
            ui.monospace(format!("turn {shown} / {}", replay.total_turns()));
            if replay.seeking() {
                ui.weak("seeking…");
            }
            ui.separator();
            self.show_legend(ui, replay.total_turns());
        });
        if let Some(turn) = self.show_bar(ui, replay) {
            actions.push(Action::Seek(turn));
        }
        actions
    }

    /// One button a kind found (its colour, its label and count; a click hides or shows it),
    /// then what the scan is doing.
    fn show_legend(&mut self, ui: &mut Ui, total: u32) {
        for (kind, label, hue) in KINDS {
            let count = self.markers().iter().filter(|m| m.kind == kind).count();
            if count == 0 {
                continue;
            }
            let hidden = self.hidden(kind);
            let text = egui::RichText::new(format!("● {label} {count}")).color(if hidden {
                Color32::from_gray(110)
            } else {
                hue
            });
            let tip = if hidden { "show" } else { "hide" };
            if ui.selectable_label(!hidden, text).on_hover_text(tip).clicked() {
                self.toggle(kind);
            }
        }
        if let Some(e) = &self.error {
            ui.weak("events unavailable").on_hover_text(e.as_str());
        } else if let Some(scan) = self.scan.as_ref().filter(|s| !s.done()) {
            ui.weak(format!("finding events… {} / {total}", scan.turn()));
        } else if self.markers().is_empty() {
            ui.weak("no events to mark");
        }
    }

    /// The scrubber: the track, how far the replay (and the scan) has got, the ticks above it
    /// and the thumb. A press or a drag picks a turn, and letting go seeks there. Hovering a tick
    /// lists what happened at its turn.
    fn show_bar(&mut self, ui: &mut Ui, replay: &Replay) -> Option<u32> {
        let total = replay.total_turns().max(1);
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), BAR_HEIGHT), Sense::click_and_drag());
        let label = format!("timeline, turn {} of {}", replay.turn(), replay.total_turns());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, &label));
        let track = Rect::from_min_max(
            pos2(rect.left() + 6.0, rect.bottom() - 9.0),
            pos2(rect.right() - 6.0, rect.bottom() - 5.0),
        );
        let x_of =
            |turn: u32| track.left() + track.width() * (turn.min(total) as f32 / total as f32);
        let turn_at = |x: f32| {
            let f = ((x - track.left()) / track.width().max(1.0)).clamp(0.0, 1.0);
            (f * total as f32).round() as u32
        };
        let painter = ui.painter_at(rect);
        painter.rect_filled(track, CornerRadius::same(2), TRACK);
        if let Some(scan) = self.scan.as_ref().filter(|s| !s.done()) {
            let scanned = Rect::from_min_max(
                pos2(track.left(), rect.top() + 2.0),
                pos2(x_of(scan.turn()), track.top() - 3.0),
            );
            painter.rect_filled(scanned, 0.0, Color32::from_white_alpha(10));
        }
        let shown = self.dragging.unwrap_or(replay.turn());
        let done = Rect::from_min_max(track.min, pos2(x_of(shown), track.max.y));
        painter.rect_filled(done, CornerRadius::same(2), ACCENT);

        // The ticks: kinds at one turn stack upwards, so none hides another.
        let mut hovered_tick = None;
        let pointer = response.hover_pos();
        let mut stacked: Vec<(u32, u8)> = Vec::new();
        for (turn, kind) in self.ticks() {
            let level = match stacked.iter_mut().find(|(t, _)| *t == turn) {
                Some((_, n)) => {
                    *n += 1;
                    *n
                }
                None => {
                    stacked.push((turn, 0));
                    0
                }
            };
            let x = x_of(turn);
            let top = track.top() - 5.0 - f32::from(level) * 4.0;
            let tick = Rect::from_min_max(pos2(x - 1.5, top - 4.0), pos2(x + 1.5, top));
            painter.rect_filled(tick, 0.0, color(kind));
            if pointer.is_some_and(|p| (p.x - x).abs() <= 3.0 && p.y < track.top()) {
                hovered_tick = Some(turn);
            }
        }
        painter.circle_filled(pos2(x_of(shown), track.center().y), 6.0, Color32::from_gray(208));
        painter.circle_stroke(pos2(x_of(shown), track.center().y), 6.0, Stroke::new(1.0, TRACK));

        if let Some(turn) = hovered_tick {
            let lines: Vec<String> = self.events_at(turn).iter().map(|m| m.text.clone()).collect();
            response.clone().on_hover_ui_at_pointer(|ui| {
                ui.strong(format!("turn {turn}"));
                for line in lines {
                    ui.label(line);
                }
            });
        } else if let Some(p) = pointer {
            let turn = turn_at(p.x);
            ui.painter().text(
                pos2(p.x, rect.top()),
                Align2::CENTER_TOP,
                format!("{turn}"),
                FontId::monospace(11.0),
                Color32::from_gray(160),
            );
        }

        if let Some(p) = response.interact_pointer_pos()
            && (response.is_pointer_button_down_on() || response.dragged())
        {
            self.dragging = Some(turn_at(p.x));
        }
        let released = response.drag_stopped() || response.clicked();
        if released {
            let turn = response.interact_pointer_pos().map(|p| turn_at(p.x)).or(self.dragging);
            self.dragging = None;
            return turn;
        }
        None
    }
}

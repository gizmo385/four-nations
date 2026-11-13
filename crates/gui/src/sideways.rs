//! "Turn your phone sideways" (#975, docs/design/touch-pwa.md §1): iOS ignores the manifest's
//! landscape lock, so when the page is taller than it is wide the game shows this screen instead
//! (mock-up board 3) and a game in progress pauses until the phone turns back.
//!
//! Only on the web: a desktop window may be any shape it likes ([`crate::app::GuiApp`] decides).

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Ui, vec2};
use fn_tui::color::Palette;
use fn_tui::shell::{Mode, Shell};

use crate::color::to_egui;

/// Whether this screen paused the game, between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sideways {
    /// The clock was running when the screen came up and this paused it, so turning back
    /// starts it again. A game the player paused stays paused.
    paused_here: bool,
    /// The screen was up last frame.
    shown: bool,
}

impl Sideways {
    /// This frame: `upright` is whether the window is taller than it is wide. Pauses a running
    /// game as the screen comes up and resumes it as the screen goes, if this paused it. Returns
    /// whether the screen shows.
    pub fn update(&mut self, upright: bool, shell: &mut Shell) -> bool {
        let clock = match &mut shell.mode {
            Mode::Game(app) => Some(&mut app.session.clock),
            Mode::Title(_) | Mode::Embark(_) => None,
        };
        match (upright, self.shown) {
            (true, false) => {
                if let Some(clock) = clock
                    && !clock.paused
                {
                    clock.paused = true;
                    self.paused_here = true;
                }
            }
            (false, true) => {
                if std::mem::take(&mut self.paused_here)
                    && let Some(clock) = clock
                {
                    clock.paused = false;
                }
            }
            _ => {}
        }
        self.shown = upright;
        upright
    }
}

/// The screen itself, over the whole of `ui`: the phone turning, and what to do. `in_game` adds
/// the line that the settlement is paused.
pub fn show(ui: &mut Ui, palette: &Palette, in_game: bool) {
    let c = |rgb| to_egui(palette.token(rgb), Color32::GRAY);
    let t = &palette.theme;
    let rect = ui.max_rect();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, c(t.panel));
    // The art is 32 blocks wide: as big as fits in four fifths of the width, whole points.
    let block = ((rect.width() * 0.8) / ART_WIDTH as f32).floor().clamp(2.0, 8.0);
    let art = vec2(ART_WIDTH as f32, ART_HEIGHT as f32) * block;
    let mid = rect.center();
    let top = Pos2::new(mid.x - art.x / 2.0, mid.y - art.y - block * 4.0);
    paint_art(&painter, top, block, c(t.text), c(t.accent));
    let mut y = top.y + art.y + block * 6.0;
    let title = painter.text(
        Pos2::new(mid.x, y),
        Align2::CENTER_TOP,
        "Turn your phone sideways",
        FontId::proportional(22.0),
        c(t.accent_text),
    );
    y = title.bottom() + 10.0;
    if in_game {
        painter.text(
            Pos2::new(mid.x, y),
            Align2::CENTER_TOP,
            "Your settlement is paused until you turn back",
            FontId::proportional(15.0),
            c(t.dim),
        );
    }
    // For a screen reader and the tests, the words as one label.
    let words = if in_game {
        "Turn your phone sideways. Your settlement is paused until you turn back."
    } else {
        "Turn your phone sideways."
    };
    let r = ui.allocate_rect(rect, egui::Sense::hover());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, words));
}

const ART_WIDTH: usize = 32;
const ART_HEIGHT: usize = 14;

/// An upright phone, an arrow and the same phone sideways, in blocks: `o` the outline, `a` the
/// arrow in the accent.
const ART: [&str; ART_HEIGHT] = [
    "oooooooo                        ",
    "o      o                        ",
    "o      o                        ",
    "o      o        a               ",
    "o      o        aa              ",
    "o      o  aaaaaaaaa  ooooooooooo",
    "o      o        aa   o         o",
    "o      o        a    o        oo",
    "o      o             o        oo",
    "o      o             o         o",
    "o      o             ooooooooooo",
    "o  oo  o                        ",
    "o      o                        ",
    "oooooooo                        ",
];

fn paint_art(painter: &Painter, top: Pos2, block: f32, line: Color32, arrow: Color32) {
    for (y, row) in ART.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let colour = match ch {
                'o' => line,
                'a' => arrow,
                _ => continue,
            };
            let at = top + vec2(x as f32, y as f32) * block;
            painter.rect_filled(Rect::from_min_size(at, vec2(block, block)), 0.0, colour);
        }
    }
}

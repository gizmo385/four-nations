//! The confirm box (#655): every question that can lose progress (a new map, a switch of nation,
//! quit, a load, a quickload, a save over a slot) is asked in a centred box, in the Seal frame the
//! menu uses, and not in the hint bar (`crate::modal`).
//!
//! The box says what happens, says how long ago the game was last saved (in game time, from
//! [`UiState::saved_at`](crate::state::UiState::saved_at): `fn_ui` never reads the wall clock) or
//! that it never was, and lists its answers as rows. The cursor starts on the first, which is
//! always the safe one, and `Esc` picks it too; a bare `y` does nothing (`App::prompt_key`).
//! Where saving first makes sense (a new map, a switch, quit) the second row is "Save, then …",
//! which writes a new slot (`App::save_new_slot`) and only then goes on.

use fn_ui::Session;
use fn_ui::view::format_ago;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::Palette;
use crate::modal;
use crate::state::{Prompt, UiState};
use crate::widgets::{Marks, Painter, Row, Tone};

/// How wide the box would like to be; it shrinks to fit a small terminal (`modal::modal_area`).
const WIDTH: u16 = 56;

/// What a row of the box does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The safe answer: nothing changes. Always the first row, and what `Esc` picks.
    Keep,
    /// Saves to a new slot, then does what was asked (and stays open if the save fails).
    SaveThen,
    /// Does what was asked.
    Go,
}

/// What the box for one [`Prompt`] says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    pub title: &'static str,
    /// The question, in one line.
    pub question: String,
    /// What follows from it: when the game was last saved and what that costs.
    pub lines: Vec<String>,
    /// The answers, top to bottom; the first is [`Answer::Keep`].
    pub answers: Vec<(Answer, String)>,
}

/// The box for `prompt`. `can_save` is whether there is anywhere to save to (a save directory),
/// which decides whether the "Save, then …" row is offered; `ticks_since_save` is the game time
/// since the last save, or `None` if the game was never saved.
pub fn describe(
    prompt: &Prompt,
    session: &Session,
    can_save: bool,
    ticks_since_save: Option<u64>,
) -> Ask {
    let lost = match ticks_since_save {
        Some(0) => vec!["This game was last saved just now.".to_owned()],
        Some(ticks) => vec![
            format!("This game was last saved {}.", format_ago(ticks)),
            "Anything since then is lost.".to_owned(),
        ],
        None => {
            vec!["This game has not been saved.".to_owned(), "Everything in it is lost.".to_owned()]
        }
    };
    let keep = (Answer::Keep, "Keep playing".to_owned());
    // Quit, new map and switch nation: save first, or go without.
    let leave = |then: &str, without: &str| {
        let mut answers = vec![keep.clone()];
        if can_save {
            answers.push((Answer::SaveThen, then.to_owned()));
        }
        answers.push((Answer::Go, without.to_owned()));
        answers
    };
    match prompt {
        Prompt::Quit => Ask {
            title: "QUIT",
            question: "Quit the game?".to_owned(),
            lines: lost,
            answers: leave("Save, then quit", "Quit without saving"),
        },
        Prompt::NewMap => Ask {
            title: "NEW MAP",
            question: format!("Start a new map (seed {})?", session.world.seed.wrapping_add(1)),
            lines: lost,
            answers: leave("Save, then start over", "Start over without saving"),
        },
        Prompt::Nation(n) => Ask {
            title: "SWITCH NATION",
            question: format!("Start over as the {}?", session.defs.nation(*n).name),
            lines: lost,
            answers: leave("Save, then start over", "Start over without saving"),
        },
        Prompt::QuickLoad => Ask {
            title: "LOAD",
            question: "Load the quicksave in place of this game?".to_owned(),
            lines: lost,
            answers: vec![keep, (Answer::Go, "Load it, losing this game".to_owned())],
        },
        Prompt::LoadSave(name) => Ask {
            title: "LOAD",
            question: format!("Load {name} in place of this game?"),
            lines: lost,
            answers: vec![keep, (Answer::Go, "Load it, losing this game".to_owned())],
        },
        Prompt::SaveOver(name) => Ask {
            title: "SAVE",
            question: format!("Save over the {name} slot?"),
            lines: vec!["What is in the slot now is replaced.".to_owned()],
            answers: vec![
                (Answer::Keep, "Keep the old save".to_owned()),
                (Answer::Go, "Save over it".to_owned()),
            ],
        },
    }
}

/// The box for the prompt `ui` has open, if any.
pub fn current(session: &Session, ui: &UiState) -> Option<Ask> {
    let prompt = ui.prompt.as_ref()?;
    let since = ui.saved_at.map(|at| session.world.tick.saturating_sub(at));
    Some(describe(prompt, session, ui.ask_can_save, since))
}

/// The box's target size: the question, a blank, what follows, a blank, the answers.
fn size(ask: &Ask) -> (u16, u16) {
    let rows = 1 + 1 + ask.lines.len() + 1 + ask.answers.len();
    (WIDTH, rows as u16 + 2)
}

/// Draws the box over the map area `outer`, the answer at `at` reversed.
pub fn draw(ask: &Ask, at: usize, palette: &Palette, outer: Rect, buf: &mut Buffer) {
    let marks = Marks::new(palette.ascii);
    let title = format!(" {0} {1} {0} ", marks.seal, ask.title);
    let inner = modal::draw(buf, outer, size(ask), palette, &title, "");
    let mut p = Painter::new(buf, inner, palette);
    p.row(&Row::text(ask.question.clone(), Tone::Strong));
    p.blank();
    for line in &ask.lines {
        p.row(&Row::text(line.clone(), Tone::Text));
    }
    p.blank();
    let mut reversed = None;
    for (i, (_, label)) in ask.answers.iter().enumerate() {
        let chosen = i == at;
        if chosen {
            reversed = Some(inner.y + p.row);
        }
        let pointer = if chosen { marks.pointer } else { " " };
        let tone = if chosen { Tone::Title } else { Tone::Text };
        p.row(&Row::text(format!("{pointer} {label}"), tone));
    }
    if let Some(y) = reversed {
        for x in inner.x..inner.right() {
            buf[(x, y)].set_style(Style::new().add_modifier(Modifier::REVERSED));
        }
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;

    fn session() -> Session {
        Session::new(7, Nation::Earth, 64, 64)
    }

    fn every_prompt() -> Vec<Prompt> {
        vec![
            Prompt::Quit,
            Prompt::NewMap,
            Prompt::Nation(Nation::Water),
            Prompt::QuickLoad,
            Prompt::LoadSave("save-1".to_owned()),
            Prompt::SaveOver("save-1".to_owned()),
        ]
    }

    #[test]
    fn the_first_answer_is_always_the_safe_one() {
        for prompt in every_prompt() {
            for can_save in [false, true] {
                let ask = describe(&prompt, &session(), can_save, Some(7200));
                assert_eq!(ask.answers[0].0, Answer::Keep, "{prompt:?}");
                assert_eq!(ask.answers.iter().filter(|(a, _)| *a == Answer::Keep).count(), 1);
            }
        }
    }

    #[test]
    fn only_a_new_map_a_switch_and_quit_offer_to_save_first() {
        for prompt in every_prompt() {
            let ask = describe(&prompt, &session(), true, None);
            let offers = ask.answers.iter().any(|(a, _)| *a == Answer::SaveThen);
            let leaves = matches!(prompt, Prompt::Quit | Prompt::NewMap | Prompt::Nation(_));
            assert_eq!(offers, leaves, "{prompt:?}");
            // With nowhere to save to, there is no such row.
            let ask = describe(&prompt, &session(), false, None);
            assert!(ask.answers.iter().all(|(a, _)| *a != Answer::SaveThen), "{prompt:?}");
        }
    }

    #[test]
    fn the_box_says_when_the_game_was_last_saved_or_that_it_never_was() {
        let minute = 600;
        let ask = describe(&Prompt::Quit, &session(), true, Some(12 * minute));
        assert_eq!(ask.lines[0], "This game was last saved 12 minutes ago.");
        assert_eq!(ask.lines[1], "Anything since then is lost.");
        let ask = describe(&Prompt::Quit, &session(), true, None);
        assert_eq!(ask.lines[0], "This game has not been saved.");
    }

    #[test]
    fn the_switch_box_names_the_nation() {
        let ask = describe(&Prompt::Nation(Nation::Water), &session(), true, None);
        assert_eq!(ask.question, "Start over as the Water Tribes?");
    }
}

//! The saves screen (`F6`, #276): every slot in the save directory, newest first, with its
//! `Slot::describe` text; an unloadable slot's reason shows dimmed. `Enter` asks to load the
//! highlighted slot through `App::load`; `S` (Shift+`s`: plain `s` already moves the highlight)
//! asks to save over it; `N` writes a new one (`fn_ui::saves::new_name`), with no prompt since
//! nothing is lost.
//!
//! Listing touches disk, so unlike the other screens the slots are read once, when the screen
//! opens or a save changes them (`App::refresh_saves`), and cached on `UiState`; [`draw`] here
//! is then a pure function of that cache like every other screen.

use fn_sim::Defs;
use fn_ui::saves::Slot;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::color::Palette;
use crate::page;
use crate::widgets::{Marks, Painter, Row, Tone, window_start};

/// The cursor after a step of `dy` rows (its sign only), stopping at the list's edges.
pub fn step(slots: &[Slot], at: usize, dy: i32) -> usize {
    if slots.is_empty() {
        return 0;
    }
    let last = slots.len() - 1;
    (at as i64 + i64::from(dy.signum())).clamp(0, last as i64) as usize
}

/// One slot's row: its name, then what `Slot::describe` says. The whole row dims when the slot
/// can't be loaded.
fn slot_row(slot: &Slot, defs: &Defs, chosen: bool, marks: &Marks) -> Row {
    let tone = if slot.loadable() { Tone::Text } else { Tone::Dim };
    let name_tone = if slot.loadable() { Tone::Strong } else { Tone::Dim };
    let pointer = if chosen { marks.pointer } else { " " };
    Row::spans(vec![
        (format!("{pointer} "), Tone::Title),
        (format!("{}  ", slot.name), name_tone),
        (slot.describe(defs), tone),
    ])
}

/// Draws the screen's contents (the list, or a note that there is none yet) into `inner` (a full
/// screen's body, or the title screen's Load page). The slot under `at` wears the selection
/// (`page::highlight`), as the research screen's chosen card does.
pub fn draw(
    slots: &[Slot],
    at: usize,
    defs: &Defs,
    palette: &Palette,
    inner: Rect,
    buf: &mut Buffer,
) -> Option<String> {
    let marks = Marks::new(palette.ascii);
    let mut p = Painter::new(buf, inner, palette);
    if slots.is_empty() {
        p.row(&Row::text("No saves yet: press N to write one.", Tone::Dim));
        return None;
    }
    let at = at.min(slots.len() - 1);
    let shown = usize::from(inner.height);
    let first = window_start(at, shown);
    let mut chosen_at = None;
    for (i, slot) in slots.iter().enumerate().skip(first).take(shown) {
        let chosen = i == at;
        if chosen {
            chosen_at = Some(inner.y + p.row);
        }
        p.row(&slot_row(slot, defs, chosen, &marks));
    }
    if let Some(y) = chosen_at {
        page::highlight(buf, Rect::new(inner.x, y, inner.width, 1), palette);
    }
    fn_ui::text::window_label("slots", first, shown, slots.len())
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use fn_sim::save::SaveInfo;
    use fn_ui::saves::SlotState;

    use super::*;

    fn ready(seed: u64, tick: u64) -> SlotState {
        SlotState::Ready(SaveInfo {
            magic: "four-nations-save".into(),
            version: 1,
            data_hash: 0,
            seed,
            nation: Nation::Earth,
            tick,
        })
    }

    fn slot(name: &str, state: SlotState) -> Slot {
        Slot { name: name.into(), state, modified: None }
    }

    #[test]
    fn stepping_stops_at_the_edges_and_an_empty_list_stays_put() {
        let slots = vec![slot("a", ready(1, 0)), slot("b", ready(1, 10)), slot("c", ready(1, 20))];
        assert_eq!(step(&slots, 0, -1), 0);
        assert_eq!(step(&slots, 0, 1), 1);
        assert_eq!(step(&slots, 2, 1), 2);
        // Shift (tiles > 1) is never passed through: callers pass the sign only.
        assert_eq!(step(&slots, 0, 5), 1);
        assert_eq!(step(&[], 0, 1), 0);
    }

    #[test]
    fn the_window_slides_only_once_the_cursor_passes_its_edge() {
        assert_eq!((0..3).map(|at| window_start(at, 3)).collect::<Vec<_>>(), [0, 0, 0]);
        assert_eq!((3..5).map(|at| window_start(at, 3)).collect::<Vec<_>>(), [1, 2]);
    }

    #[test]
    fn a_row_names_the_slot_and_its_description_and_marks_the_cursor() {
        let defs = Defs::bundled();
        let marks = Marks::new(false);
        let row = slot_row(&slot("save-1", ready(3, 40)), &defs, false, &marks);
        assert_eq!(
            row.plain(),
            "  save-1  Earth Kingdom, seed 3, Spring Y1 - 14:56 left (tick 40)"
        );
        let chosen = slot_row(&slot("save-1", ready(3, 40)), &defs, true, &marks);
        assert!(chosen.plain().starts_with('▸'), "{}", chosen.plain());
    }

    #[test]
    fn an_unloadable_slot_dims_the_whole_row() {
        let defs = Defs::bundled();
        let marks = Marks::new(false);
        let row = slot_row(
            &slot("junk", SlotState::Unloadable("not a save".into())),
            &defs,
            false,
            &marks,
        );
        assert_eq!(row.left[1].1, Tone::Dim);
        assert_eq!(row.left[2].1, Tone::Dim);
        assert!(row.plain().contains("can't load: not a save"));
    }
}

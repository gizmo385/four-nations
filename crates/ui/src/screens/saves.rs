//! The saves screen (`F6`, #276): every slot in the save directory, newest first, with its
//! `Slot::describe` text; an unloadable slot's reason shows dimmed. `Enter` asks to load the
//! highlighted slot through `App::load`; `S` (Shift+`s`: plain `s` already moves the highlight)
//! asks to save over it; `N` types a name for a new one (#704): `Enter` saves it, with the next
//! free `save-N` (`crate::saves::new_name`) when nothing was typed, and `Esc` goes back. A name
//! `crate::saves::check_name` refuses is refused with the typing kept, and one already taken asks
//! first, as Shift+`S` does.
//!
//! Listing touches disk, so unlike the other screens the slots are read once, when the screen
//! opens or a save changes them (`App::refresh_saves`), and cached on `UiState`; `fn_tui::saves::draw` here
//! is then a pure function of that cache like every other screen.

use crate::rows::{Marks, Row, Tone};
use crate::saves::Slot;
use fn_sim::Defs;

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
pub fn slot_row(slot: &Slot, defs: &Defs, chosen: bool, marks: &Marks) -> Row {
    let tone = if slot.loadable() { Tone::Text } else { Tone::Dim };
    let name_tone = if slot.loadable() { Tone::Strong } else { Tone::Dim };
    let pointer = if chosen { marks.pointer } else { " " };
    Row::spans(vec![
        (format!("{pointer} "), Tone::Title),
        (format!("{}  ", slot.name), name_tone),
        (slot.describe(defs), tone),
    ])
}

#[cfg(test)]
mod tests {
    use crate::saves::SlotState;
    use fn_sim::Nation;
    use fn_sim::save::SaveInfo;

    use super::*;
    use crate::rows::window_start;

    fn ready(seed: u64, tick: u64) -> SlotState {
        SlotState::Ready(SaveInfo {
            magic: "four-nations-save".into(),
            version: 1,
            data_hash: 0,
            seed,
            nation: Nation::Earth,
            tick,
            seasons: Some(true),
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

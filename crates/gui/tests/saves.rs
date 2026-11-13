//! The Saves screen (#885) against a store in a temp directory: the list is `fn_ui::saves`'s,
//! the name is a text field whose edits are the keys the terminal types, and saving and loading
//! a named slot works through clicks. Nothing here needs a GPU.

use std::path::PathBuf;

use eframe::egui::{self, Event, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::frame::{BoxKind, PageKind, Surface};
use fn_gui::screens::saves::{NAME_FIELD, click_keys, edit_keys, name_problem};
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_ui::saves::{self, DiskStore, Store};

const SIZE: (u16, u16) = (120, 40);

/// A fresh save directory, removed when the test is done.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("fn-gui-saves-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        TempDir(dir)
    }

    fn store(&self) -> Store {
        DiskStore::shared(&self.0)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A game of seed 7 that has run `ticks`, with `store` as its saves, after `script`.
fn game(store: &Store, ticks: u32, script: &str) -> Shell {
    let mut session = fn_tui::new_session(7, Nation::Earth);
    session.tick_n(ticks);
    let mut shell = Shell::new_game(session, SIZE, Some(store.clone()));
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

fn harness(app: &mut GuiApp) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 960.0))
        // Each queued event is a frame of its own, so short ones: two clicks are a double-click
        // only if less than 0.3 s apart.
        .with_step_dt(0.02)
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(2);
    h
}

fn state(app: &GuiApp) -> &fn_ui::app::App {
    match &app.shell().mode {
        Mode::Game(game) => game,
        Mode::Title(_) | Mode::Embark(_) => panic!("not in a game"),
    }
}

fn names(store: &Store) -> Vec<String> {
    saves::list(store.as_ref()).unwrap().into_iter().map(|s| s.name).collect()
}

/// Two quick presses at `at`: a double-click for egui.
fn double_click(h: &mut Harness<'_>, at: egui::Pos2) {
    h.event(Event::PointerMoved(at));
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
    }
    h.run_steps(4);
}

/// Writes `names` as slots of a game at `ticks`, and returns a shell on the saves screen.
fn with_slots(store: &Store, names: &[&str], ticks: u32) -> Shell {
    let mut shell = game(store, ticks, "");
    let Mode::Game(app) = &mut shell.mode else { panic!("a game") };
    for name in names {
        saves::save(store.as_ref(), name, &app.session).unwrap();
    }
    shell.play(&fn_ui::script::parse("<f6>").unwrap());
    shell
}

#[test]
fn f6_opens_a_native_page_with_the_slots_as_rows() {
    let dir = TempDir::new("list");
    let store = dir.store();
    let mut app = GuiApp::with_shell(with_slots(&store, &["alpha", "beta"], 0));
    let h = harness(&mut app);
    h.get_by_label("◆ SAVES ◆");
    assert_eq!(h.query_all_by_label_contains("SETTLEMENT").count(), 0, "the side panel is hidden");
    // Each row is the terminal's: its name, then `Slot::describe`'s text.
    for name in ["alpha", "beta"] {
        h.get_by_label_contains(&format!("{name}  Earth Kingdom, seed 7"));
    }
    h.get_by_label("New save [N]");
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::Page(PageKind::Saves)));
}

#[test]
fn an_empty_list_says_how_to_write_the_first_save() {
    let dir = TempDir::new("empty");
    let mut app = GuiApp::with_shell(game(&dir.store(), 0, "<f6>"));
    let h = harness(&mut app);
    h.get_by_label("No saves yet: press N to write one.");
}

/// The acceptance test: name a slot in the text field and save it, then, in a game that has
/// moved on, load it back from the list.
#[test]
fn a_named_slot_is_saved_from_the_field_and_loaded_from_the_list() {
    let dir = TempDir::new("roundtrip");
    let store = dir.store();
    let mut app = GuiApp::with_shell(game(&store, 0, "<f6>"));

    // New save: the field opens with the keyboard, and what is typed in it is the name.
    let mut h = harness(&mut app);
    h.get_by_label("New save [N]").click();
    h.run_steps(3);
    h.get_by_label(NAME_FIELD).type_text("my-slot");
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.save_name.as_deref(), Some("my-slot"), "typed once, not twice");

    let mut h = harness(&mut app);
    h.get_by_label("Save [Enter]").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(names(&store), ["my-slot"]);
    assert!(state(&app).ui.save_name.is_none());
    assert!(state(&app).ui.status.starts_with("Saved to"), "{}", state(&app).ui.status);

    // A game that has moved on, on the same store.
    let mut app = GuiApp::with_shell(game(&store, 30, "<f6>"));
    assert_eq!(state(&app).session.world.tick, 30);
    // What the slot holds: the first game, a few ticks in (the window's clock runs on).
    let saved_tick =
        saves::load(store.as_ref(), "my-slot", &state(&app).session.defs).unwrap().tick;
    assert!(saved_tick < 30, "{saved_tick}");
    let mut h = harness(&mut app);
    // A double-click on the row asks first (the box is over the page); the box loads it.
    let row = h.get_by_label_contains("my-slot  Earth Kingdom").rect().center();
    double_click(&mut h, row);
    h.get_by_label_contains("Load my-slot in place of this game?");
    h.get_by_label("Load it, losing this game").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(app.frame().modal, None);
    // The load's own line names the tick it loaded; the clock has run a frame or two since.
    assert!(
        state(&app).ui.status.starts_with(&format!("Loaded my-slot (tick {saved_tick})")),
        "{}",
        state(&app).ui.status
    );
    assert!(state(&app).session.world.tick < 30, "the slot's world is back");
}

/// Enter with nothing typed takes the suggestion, and a name that is taken asks before it
/// replaces the slot: the shell's own rules, through the same keys.
#[test]
fn the_field_takes_the_suggestion_and_asks_before_replacing() {
    let dir = TempDir::new("suggest");
    let store = dir.store();
    let mut app = GuiApp::with_shell(game(&store, 0, "<f6>"));
    let mut h = harness(&mut app);
    h.get_by_label("New save [N]").click();
    h.run_steps(3);
    h.get_by_label("Save [Enter]").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(names(&store), ["save-1"]);

    // The name again: it exists, so the box asks.
    let mut h = harness(&mut app);
    h.get_by_label("New save [N]").click();
    h.run_steps(3);
    h.get_by_label(NAME_FIELD).type_text("save-1");
    h.run_steps(3);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    h.get_by_label("Save over the save-1 slot?");
    drop(h);
    assert_eq!(app.frame().modal, Some(BoxKind::Ask));
}

/// The keyboard's own typing (a `Text` event per character, a `Backspace` key) reaches the name
/// once, not once through the shell and again through the field.
#[test]
fn the_keyboard_types_into_the_field_once() {
    let dir = TempDir::new("keyboard");
    let mut app = GuiApp::with_shell(game(&dir.store(), 0, "<f6>"));
    let mut h = harness(&mut app);
    h.get_by_label("New save [N]").click();
    h.run_steps(3);
    for c in ["a", "b", "c"] {
        h.event(Event::Text(c.into()));
        h.run_steps(2);
    }
    h.key_press(egui::Key::Backspace);
    h.run_steps(2);
    h.event(Event::Text("Z".into()));
    h.run_steps(2);
    drop(h);
    assert_eq!(state(&app).ui.save_name.as_deref(), Some("abZ"));
}

/// With the field unfocused (a click elsewhere), the keyboard still types the name, as it does
/// at the terminal.
#[test]
fn an_unfocused_field_still_takes_the_shells_typing() {
    let dir = TempDir::new("unfocused");
    let mut app = GuiApp::with_shell(game(&dir.store(), 0, "<f6> n"));
    let mut h = harness(&mut app);
    // A click on the empty page takes the keyboard off the field.
    let at = egui::pos2(800.0, 700.0);
    h.hover_at(at);
    h.run_steps(1);
    h.drag_at(at);
    h.run_steps(1);
    h.drop_at(at);
    h.run_steps(2);
    for c in ["a", "b"] {
        h.event(Event::Text(c.into()));
        h.run_steps(2);
    }
    drop(h);
    assert_eq!(state(&app).ui.save_name.as_deref(), Some("ab"));
}

#[test]
fn a_name_that_cannot_be_a_slot_is_flagged_and_refused() {
    let dir = TempDir::new("refused");
    let store = dir.store();
    let mut app = GuiApp::with_shell(game(&store, 0, "<f6>"));
    let mut h = harness(&mut app);
    h.get_by_label("New save [N]").click();
    h.run_steps(3);
    h.get_by_label(NAME_FIELD).type_text("no way!");
    h.run_steps(3);
    h.get_by_label_contains("letters, digits, - and _");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    drop(h);
    assert!(names(&store).is_empty());
    assert_eq!(state(&app).ui.save_name.as_deref(), Some("no way!"), "the typing is kept");
}

/// The Cancel button leaves the field with nothing written.
#[test]
fn cancel_leaves_the_field_and_writes_nothing() {
    let dir = TempDir::new("cancel");
    let store = dir.store();
    let mut app = GuiApp::with_shell(game(&store, 0, "<f6>"));
    let mut h = harness(&mut app);
    h.get_by_label("New save [N]").click();
    h.run_steps(3);
    h.get_by_label(NAME_FIELD).type_text("x");
    h.run_steps(2);
    h.get_by_label("Cancel [Esc]").click();
    h.run_steps(3);
    assert!(h.query_by_label(NAME_FIELD).is_none());
    drop(h);
    assert!(names(&store).is_empty());
    assert!(state(&app).ui.save_name.is_none());
    assert_eq!(state(&app).ui.screen, Some(fn_ui::state::Screen::Saves), "still on the screen");
}

/// A click on a row moves the highlight there, and Save over asks about that slot.
#[test]
fn clicking_a_row_moves_the_highlight_and_save_over_asks() {
    let dir = TempDir::new("over");
    let store = dir.store();
    let mut app = GuiApp::with_shell(with_slots(&store, &["old", "new"], 0));
    let mut h = harness(&mut app);
    let list = saves::list(store.as_ref()).unwrap();
    // The highlight starts on the first row; click the other.
    let other = &list[1].name;
    h.get_by_label_contains(&format!("{other}  Earth Kingdom")).click();
    h.run_steps(3);
    h.get_by_label("Save over [Shift+S]").click();
    h.run_steps(3);
    h.get_by_label(&format!("Save over the {other} slot?"));
}

#[test]
fn clicks_are_arrow_presses_and_edits_are_the_keys_that_make_them() {
    let codes = |keys: Vec<KeyEvent>| keys.into_iter().map(|k| k.code).collect::<Vec<_>>();
    assert_eq!(codes(click_keys(1, 4, false)), [KeyCode::Down; 3]);
    assert_eq!(codes(click_keys(3, 2, true)), [KeyCode::Up, KeyCode::Enter]);
    assert!(click_keys(2, 2, false).is_empty());

    assert_eq!(codes(edit_keys("", "ab")), [KeyCode::Char('a'), KeyCode::Char('b')]);
    assert_eq!(codes(edit_keys("abc", "ab")), [KeyCode::Backspace]);
    // A change in the middle: back to what is shared, then the rest again.
    assert_eq!(
        codes(edit_keys("abc", "aXc")),
        [KeyCode::Backspace, KeyCode::Backspace, KeyCode::Char('X'), KeyCode::Char('c')]
    );
    let big = edit_keys("", "S");
    assert_eq!(big[0].modifiers, KeyModifiers::SHIFT);
    assert!(edit_keys("same", "same").is_empty());

    assert_eq!(name_problem(""), None, "nothing typed takes the suggestion");
    assert_eq!(name_problem("save_1-b"), None);
    assert!(name_problem("no way").is_some());
}

/// On touch (#976), New save writes a slot named for the nation and the time at once, and a
/// second one gets a number; nothing is typed and no name field is left open.
#[test]
fn on_touch_new_save_writes_an_auto_named_slot() {
    let tmp = TempDir::new("auto");
    let store = tmp.store();
    let mut app = GuiApp::with_shell(game(&store, 40, "<f6>"));
    app.frame_mut().controls.force_touch = true;
    let want = saves::auto_name(&[], &state(&app).session);
    let mut h = harness(&mut app);
    h.get_by_label("New save [N]").click();
    h.run_steps(3);
    h.get_by_label("New save [N]").click();
    h.run_steps(3);
    drop(h);
    let mut got = names(&store);
    got.sort();
    assert_eq!(got, vec![want.clone(), format!("{want}-2")]);
    assert!(state(&app).ui.save_name.is_none(), "no name field left open");
}

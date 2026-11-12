//! The key help and its legend (#888): the words are `fn_ui`'s, `Tab` (or the other tab's
//! button) switches tabs, and the page closes as it does in the terminal. Nothing here needs a
//! GPU.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::frame::{PageKind, Surface};
use fn_gui::page;
use fn_gui::screens::help::split;
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_ui::input::help_rows;
use fn_ui::legend::LegendGroup;
use fn_ui::state::{HelpTab, Screen};

const SIZE: (u16, u16) = (120, 40);

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

fn harness(app: &mut GuiApp) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 960.0))
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

/// `?` opens a native page on the Keys tab: every group's heading and every key's wording, the
/// tab strip, and no side panel.
#[test]
fn the_help_is_a_page_of_the_keys_by_group() {
    let mut app = GuiApp::with_shell(game("?"));
    let rows = help_rows();
    let h = harness(&mut app);
    assert_eq!(h.query_all_by_label_contains("SETTLEMENT").count(), 0, "the side panel is hidden");
    h.get_by_label("◆ KEY HELP ◆");
    h.get_by_label("Keys");
    h.get_by_label("Legend");
    h.get_by_label("Tab switches");
    for group in fn_ui::input::Group::ALL {
        h.get_by_label(&group.title().to_uppercase());
    }
    // Every key's row is on the page (the wrapping is egui's, the words are not).
    for row in &rows {
        let label = format!("{}  {}", row.keys, row.what);
        assert!(h.query_all_by_label(&label).count() >= 1, "no row for {label}");
    }
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::Page(PageKind::Help)));
}

/// The other tab's button is `Tab`: the Legend lists the glyphs of the map in view under their
/// groups, and Keys brings the keys back.
#[test]
fn the_legend_tab_lists_the_map_glyphs_and_tab_switches() {
    let mut app = GuiApp::with_shell(game("?"));
    let mut h = harness(&mut app);
    h.get_by_label("Legend").click();
    h.run_steps(3);
    h.get_by_label("PEOPLE");
    h.get_by_label("TERRAIN");
    h.get_by_label("☺ Settler");
    assert_eq!(h.query_all_by_label("CURSOR AND VIEW").count(), 0, "the keys are not shown");
    drop(h);
    assert_eq!(state(&app).ui.help_tab, HelpTab::Legend);

    // The same as the key: Tab on the keyboard from the Legend tab goes back.
    let mut h = harness(&mut app);
    h.key_press(egui::Key::Tab);
    h.run_steps(3);
    h.get_by_label("CURSOR AND VIEW");
    drop(h);
    assert_eq!(state(&app).ui.help_tab, HelpTab::Keys);
}

/// Every group the map in view has is a heading of the legend; the open tab's button does
/// nothing when clicked.
#[test]
fn the_legend_has_a_heading_per_group_in_view_and_the_open_tab_is_inert() {
    let mut app = GuiApp::with_shell(game("? <tab>"));
    let mut h = harness(&mut app);
    for group in [LegendGroup::People, LegendGroup::Terrain, LegendGroup::Marks] {
        h.get_by_label(&group.title().to_uppercase());
    }
    h.get_by_label("Legend").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.help_tab, HelpTab::Legend);
}

/// The close button is `Esc`.
#[test]
fn the_close_button_is_esc() {
    let mut app = GuiApp::with_shell(game("?"));
    let mut h = harness(&mut app);
    h.get_by_label(page::CLOSE).click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.screen, None);
    assert_ne!(state(&app).ui.screen, Some(Screen::Help));
}

/// The two columns split the groups where the taller is shortest.
#[test]
fn the_columns_split_where_the_taller_is_shortest() {
    assert_eq!(split(&[]), 0);
    assert_eq!(split(&[5]), 0);
    assert_eq!(split(&[4, 4, 4, 4]), 2);
    assert_eq!(split(&[10, 3, 3, 3]), 1);
}

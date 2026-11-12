//! The school fork on the research screen (#729, docs/design/horizons-phase-4.md "Mockups"): the
//! fork headed by the school's question, the CHOOSE A SCHOOL box `Enter` opens on an open option,
//! the Closed state once the mate is learned, and the bill's progress on the card, the strip,
//! the side panel and the hall.

use fn_sim::Nation;
use fn_sim::World;
use fn_sim::research::TechId;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::state::Screen;
use fn_tui::{keys, new_session};
use fn_ui::research::{TechState, screen_view};

const SIZE: (u16, u16) = (120, 40);

fn app() -> App {
    App::new(new_session(7, Nation::Earth), SIZE)
}

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn text(app: &mut App, ascii: bool) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, ascii);
    to_text(&render_app(app, &p, SIZE))
}

fn tech(app: &App, id: &str) -> TechId {
    app.session.defs.tech_id(id).unwrap_or_else(|| panic!("no tech {id}"))
}

/// Learns `ids` by way of a save: the sim has no command that grants a tech, and a save's research
/// is its known list in `TechId` order.
fn learn(app: &mut App, ids: &[&str]) {
    let defs = &app.session.defs;
    let text = app.session.world.save(defs).expect("saves");
    let known: Vec<usize> = ids.iter().map(|id| usize::from(tech(app, id).0)).collect();
    let start = text.find("known:[").expect("a known list") + "known:[".len();
    let end = start + text[start..].find(']').unwrap();
    let list: Vec<String> = text[start..end]
        .split(',')
        .enumerate()
        .map(|(i, v)| if known.contains(&i) { "true".to_owned() } else { v.to_owned() })
        .collect();
    let text = format!("{}{}{}", &text[..start], list.join(","), &text[end..]);
    app.session.world = World::load(&text, defs).expect("loads");
    for id in ids {
        assert!(app.session.world.research().is_known(tech(app, id)), "{id}");
    }
}

/// Moves the cursor onto the card for `id`.
fn go_to(app: &mut App, id: &str) {
    let want = tech(app, id);
    let view = screen_view(&app.session);
    let (c, r) = view
        .columns
        .iter()
        .enumerate()
        .find_map(|(c, col)| col.iter().position(|t| t.id == want).map(|r| (c, r)))
        .unwrap_or_else(|| panic!("{id} is not in this nation's tree"));
    let (c0, r0) = app.ui.tech;
    let mut script = String::new();
    if r0 > 0 {
        script += &format!("<up*{r0}> ");
    }
    if c > c0 {
        script += &format!("<right*{}> ", c - c0);
    } else if c < c0 {
        script += &format!("<left*{}> ", c0 - c);
    }
    if r > 0 {
        script += &format!("<down*{r}> ");
    }
    run(app, &script);
    assert_eq!(app.ui.tech, (c, r), "{id}");
}

fn card_state(app: &App, id: &str) -> TechState {
    let want = tech(app, id);
    let view = screen_view(&app.session);
    view.columns.iter().flatten().find(|c| c.id == want).expect("in the tree").state
}

#[test]
fn the_fork_is_headed_by_the_schools_question_and_its_options_are_split_by_an_or() {
    let mut a = app();
    run(&mut a, "t");
    go_to(&mut a, "metal_city");
    let frame = text(&mut a, false);
    assert!(frame.contains("WHICH CITY?"), "{frame}");
    assert!(frame.contains(" or "), "{frame}");
    assert!(frame.contains("+ 8 Metal"), "{frame}");
    assert!(frame.contains("School: Which city? Choosing one closes the other"), "{frame}");
    let ascii = text(&mut a, true);
    assert!(ascii.contains("+- WHICH CITY? ---"), "{ascii}");
}

#[test]
fn enter_on_a_school_option_asks_first_and_esc_backs_out() {
    let mut a = app();
    run(&mut a, "t");
    go_to(&mut a, "metal_city");
    run(&mut a, "<enter>");
    assert_eq!(a.ui.school_confirm, Some(tech(&a, "metal_city")));
    assert!(a.session.world.research().queue().is_empty(), "nothing is chosen yet");
    let frame = text(&mut a, false);
    assert!(frame.contains("CHOOSE A SCHOOL"), "{frame}");
    assert!(frame.contains("Which city? Metal City or Agrarian Ring."), "{frame}");
    assert!(frame.contains("is closed for the rest of this game."), "{frame}");
    assert!(frame.contains("In stores: 0 Metal"), "{frame}");
    assert!(frame.contains("Enter choose · Esc back"), "the hints: {frame}");
    // The arrows wait while it is open.
    run(&mut a, "<left>");
    assert_eq!(a.ui.school_confirm, Some(tech(&a, "metal_city")));
    run(&mut a, "<esc>");
    assert_eq!(a.ui.school_confirm, None);
    assert_eq!(a.ui.screen, Some(Screen::Research), "Esc closes only the box");
    assert!(a.session.world.research().queue().is_empty());
}

#[test]
fn enter_twice_chooses_the_school_and_the_bill_shows_on_the_card_and_the_strip() {
    let mut a = app();
    run(&mut a, "t");
    go_to(&mut a, "metal_city");
    run(&mut a, "<enter> <enter>");
    assert_eq!(a.ui.school_confirm, None);
    assert_eq!(a.session.world.research().queue().last(), Some(&tech(&a, "metal_city")));
    let frame = text(&mut a, false);
    assert!(frame.contains("Metal 0/8"), "{frame}");
    assert!(
        frame.contains("Bill: Metal 0/8, hauled to a Scroll Library. Agrarian Ring stays open"),
        "{frame}"
    );
    // A queued option is chosen already: Enter on it again doesn't ask.
    run(&mut a, "<enter>");
    assert_eq!(a.ui.school_confirm, None);
}

#[test]
fn the_mates_option_reads_closed_once_one_is_learned() {
    let mut a = app();
    learn(&mut a, &["metal_city"]);
    run(&mut a, "t");
    assert_eq!(card_state(&a, "agrarian_ring"), TechState::Closed);
    go_to(&mut a, "agrarian_ring");
    let frame = text(&mut a, false);
    assert!(frame.contains("× Agrarian Ring"), "{frame}");
    assert!(frame.contains("You chose Metal City"), "{frame}");
    assert!(frame.contains("Closed: this settlement chose Metal City."), "{frame}");
    run(&mut a, "<enter>");
    assert_eq!(a.ui.school_confirm, None, "a closed option doesn't ask");
    assert!(a.session.world.research().queue().is_empty(), "and can't be chosen");
}

#[test]
fn the_side_panel_says_where_the_bill_is() {
    let mut a = app();
    learn(&mut a, &["metalbending"]);
    run(&mut a, "t");
    go_to(&mut a, "metal_city");
    run(&mut a, "<enter> <enter> <esc>");
    assert_eq!(a.session.world.research().queue().first(), Some(&tech(&a, "metal_city")));
    let frame = text(&mut a, false);
    assert!(frame.contains("Waiting for 8 Metal"), "{frame}");
}

#[test]
fn the_hall_that_takes_the_bill_says_which_tech_it_is_toward() {
    let mut a = app();
    learn(&mut a, &["metalbending"]);
    let metal_city = tech(&a, "metal_city");
    a.session.apply(fn_sim::world::Command::SetResearch { tech: metal_city }).unwrap();
    let defs = a.session.defs.clone();
    let hall = defs.building_id("study_hall").unwrap();
    let (w, h) = defs.building(hall).footprint;
    let p = a.session.world.units[0].pos;
    let at = fn_sim::geom::Pos::new(p.x + 3, p.y + 3);
    a.session.world.put_building(fn_sim::geom::Footprint::new(at, w, h), hall, &defs);
    let lines = fn_ui::inspect::tile_lines(&a.session, fn_ui::Tool::Select, at).unwrap();
    let details: Vec<String> = lines
        .into_iter()
        .filter_map(|l| match l {
            fn_ui::inspect::Line::Detail(d) => Some(d),
            _ => None,
        })
        .collect();
    assert!(details.iter().any(|d| d == "Toward Metal City: Metal 0/8"), "{details:?}");
}

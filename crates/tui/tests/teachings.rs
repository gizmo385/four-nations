//! The lost teachings in the terminal (#743, docs/design/horizons-phase-5.md, "Mockups"): the
//! LOST TEACHING box that opens by itself and pauses (owner #733), "Decide later" and the alert
//! it leaves, the TEACHINGS column with its `! Choose a teaching` row, kept and lost cards and
//! the hidden count (owner #734), the detail strip's cost, and the great ruin's inspect line.

use fn_sim::Nation;
use fn_sim::Pos;
use fn_sim::defs::Terrain;
use fn_sim::geom::Footprint;
use fn_sim::world::Command;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::state::{Modal, Screen};
use fn_tui::{keys, new_session};
use fn_ui::research::{TeachingRow, screen_view};

const SIZE: (u16, u16) = (120, 40);

fn app() -> App {
    App::new(new_session(7, Nation::Air), SIZE)
}

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn text(app: &mut App) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, true);
    to_text(&render_app(app, &p, SIZE))
}

fn open(app: &App, p: Pos) -> bool {
    app.session
        .world
        .map
        .get(p)
        .is_some_and(|t| t.terrain == Terrain::Land && t.building.is_none() && t.resource.is_none())
}

/// Stands a study hall on open land near the settlers and turns `ruins` open tiles near it into
/// great ruins, each designated for a search. Returns the ruins.
fn great_ruins(app: &mut App, ruins: usize) -> Vec<Pos> {
    let home = app.session.world.units[0].pos;
    let foot = (2..20)
        .flat_map(|r: i32| (-r..=r).flat_map(move |d| [(d, -r), (d, r), (-r, d), (r, d)]))
        .map(|(dx, dy)| Footprint::new(Pos::new(home.x + dx, home.y + dy), 2, 2))
        .find(|f| f.tiles().chain(f.ring()).all(|p| open(app, p)))
        .expect("room for a study hall");
    let hall = foot.anchor;
    app.session.world.units.retain(|u| !foot.contains(u.pos));
    let id = app.session.defs.building_id("study_hall").unwrap();
    app.session.world.put_building(foot, id, &app.session.defs);
    let great = app.session.defs.resource_id("great_ruins").unwrap();
    let spots: Vec<Pos> = (3..14)
        .flat_map(|r| [Pos::new(hall.x - r, hall.y), Pos::new(hall.x, hall.y - r)])
        .filter(|&p| open(app, p))
        .take(ruins)
        .collect();
    assert_eq!(spots.len(), ruins, "open land near the hall");
    for &at in &spots {
        app.session.world.map[at].resource = Some(great);
    }
    app.session.world.map_changed();
    spots
}

/// Designates the search of each of `ruins` and runs ticks until `offers` drafts are pending.
fn search(app: &mut App, ruins: &[Pos], offers: usize) {
    for &pos in ruins {
        app.session.apply(Command::DesignateHarvest { pos }).unwrap();
    }
    for _ in 0..120 {
        if app.session.world.teaching_offers().len() >= offers {
            return;
        }
        app.run_ticks(500);
    }
    panic!("only {} drafts", app.session.world.teaching_offers().len());
}

fn one_draft() -> App {
    let mut a = app();
    let ruins = great_ruins(&mut a, 1);
    search(&mut a, &ruins, 1);
    a
}

#[test]
fn the_great_ruin_reads_two_scrolls_and_a_teaching_scroll() {
    let mut a = app();
    let ruins = great_ruins(&mut a, 1);
    let lines = fn_ui::inspect::tooltip_lines(&a.session, fn_ui::Tool::Select, ruins[0]).unwrap();
    let text = format!("{lines:?}");
    assert!(text.contains("Great Ruins"), "{text}");
    assert!(text.contains("Search: 2 ancient scrolls and a teaching scroll - 40s"), "{text}");
}

#[test]
fn the_box_opens_by_itself_paused_on_decide_later() {
    let mut a = app();
    let ruins = great_ruins(&mut a, 1);
    let was_paused = a.session.clock.paused;
    search(&mut a, &ruins, 1);
    assert_eq!(a.ui.modal, Some(Modal::Teaching));
    assert!(a.session.clock.paused, "the draft pauses the clock");
    let log: Vec<&str> = a.ui.alerts.history().iter().map(|e| e.text.as_str()).collect();
    assert!(log.contains(&"Searched the great ruins: 2 ancient scrolls and a teaching scroll"));
    assert!(log.contains(&"A teaching scroll was read. Choose a teaching [T]"), "{log:?}");
    let frame = text(&mut a);
    assert!(frame.contains("* LOST TEACHING *"), "{frame}");
    assert!(frame.contains("A teaching scroll was read at the Meditation Pavilion."), "{frame}");
    assert!(frame.contains("Keep one teaching. The other is lost for this game."), "{frame}");
    assert!(frame.contains("> Decide later"), "{frame}");
    assert_eq!(frame.matches("  Keep ").count(), 2, "{frame}");
    assert_eq!(frame.matches("250 pts + scroll").count(), 2, "{frame}");
    assert!(frame.contains("Unlocks "), "{frame}");
    assert!(frame.contains("TEACHING") && frame.contains("Esc decide later"), "{frame}");
    // Esc decides later: the box closes, the pause is what it was, and the offer waits.
    run(&mut a, "<esc>");
    assert_eq!(a.ui.modal, None);
    assert_eq!(a.session.clock.paused, was_paused);
    assert_eq!(a.session.world.teaching_offers().len(), 1);
    let open: Vec<String> = a.ui.alerts.open().into_iter().map(|t| t.text).collect();
    assert!(open.contains(&"A teaching waits for your choice [T]".to_owned()), "{open:?}");
    // Once per offer: more ticks don't bring it back.
    a.run_ticks(50);
    assert_eq!(a.ui.modal, None, "the box opens once per offer");
}

#[test]
fn the_teachings_column_reopens_the_box_and_keeping_loses_the_other() {
    let mut a = one_draft();
    run(&mut a, "<esc> t");
    assert_eq!(a.ui.screen, Some(Screen::Research));
    let view = screen_view(&a.session);
    let col = view.teachings.expect("a TEACHINGS column after a scroll is read");
    assert_eq!(col, view.columns.len() - 1, "after the last tier");
    assert!(matches!(view.columns[col][0].teaching, Some(TeachingRow::Choose { waiting: 1 })));
    // Walk to the column's first row and press Enter: the box comes back.
    let (c0, r0) = a.ui.tech;
    let mut keys = String::new();
    if r0 > 0 {
        keys += &format!("<up*{r0}> ");
    }
    keys += &format!("<right*{}>", col - c0);
    run(&mut a, &keys);
    let frame = text(&mut a);
    assert!(frame.contains("TEACHINGS"), "{frame}");
    assert!(frame.contains("! Choose a teaching"), "{frame}");
    run(&mut a, "<enter>");
    assert_eq!(a.ui.modal, Some(Modal::Teaching));
    assert_eq!(a.ui.screen, None);
    // Down to the first Keep row, Enter: kept, no second question, the box closes.
    let offer = a.session.world.teaching_offers()[0].clone();
    run(&mut a, "<down> <enter>");
    assert_eq!(a.ui.modal, None);
    assert!(a.session.world.kept_teachings().any(|t| t == offer.techs[0]));
    assert!(a.session.world.lost_teachings().any(|t| t == offer.techs[1]));
    // The column now holds the kept card, the lost one, and the count still to find.
    run(&mut a, "t");
    let view = screen_view(&a.session);
    let column = &view.columns[view.teachings.unwrap()];
    assert!(column.iter().any(|c| c.id == offer.techs[0]
        && matches!(&c.teaching, Some(TeachingRow::Kept { hall: Some(h) }) if h == "Meditation Pavilion")));
    assert!(column.iter().any(|c| c.id == offer.techs[1] && c.teaching == Some(TeachingRow::Lost)));
    assert!(matches!(column.last().unwrap().teaching, Some(TeachingRow::More(_))));
    let kept = a.session.defs.tech(offer.techs[0]).name.clone();
    let lost = a.session.defs.tech(offer.techs[1]).name.clone();
    // The cursor on the kept card: its detail strip reads the scroll as part of the cost.
    let row = column.iter().position(|c| c.id == offer.techs[0]).unwrap();
    let (c0, r0) = a.ui.tech;
    let mut keys = String::new();
    if r0 > 0 {
        keys += &format!("<up*{r0}> ");
    }
    keys += &format!("<right*{}> ", view.teachings.unwrap() - c0);
    if row > 0 {
        keys += &format!("<down*{row}>");
    }
    run(&mut a, &keys);
    let frame = text(&mut a);
    assert!(frame.contains(&kept), "{frame}");
    assert!(frame.contains("x ") && frame.contains(&lost), "{frame}");
    assert!(frame.contains("Lost to the ruins"), "{frame}");
    assert!(frame.contains("more to find"), "{frame}");
    assert!(frame.contains("250 pts + scroll"), "{frame}");
    assert!(
        frame.contains("Uses up the teaching scroll read at the Meditation Pavilion"),
        "{frame}"
    );
}

#[test]
fn several_drafts_are_numbered_and_follow_one_another() {
    let mut a = app();
    let ruins = great_ruins(&mut a, 2);
    search(&mut a, &ruins, 2);
    if a.ui.modal != Some(Modal::Teaching) {
        a.open_teaching();
    }
    let frame = text(&mut a);
    assert!(frame.contains("LOST TEACHING 1 OF 2"), "{frame}");
    run(&mut a, "<down> <enter>");
    assert_eq!(a.ui.modal, Some(Modal::Teaching), "the next draft follows at once");
    let frame = text(&mut a);
    assert!(frame.contains("* LOST TEACHING *"), "{frame}");
    assert!(frame.contains("> Decide later"), "the cursor is back on the safe row");
}

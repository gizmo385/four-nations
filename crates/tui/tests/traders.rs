//! The Traders screen (`M`, #438): the caravan camped now, its deals and request, taking and
//! cancelling, when the next is due, and its camp on the map. `earth-year-two` has met "Known on
//! the road", so Fire's traders camp four minutes in.

use fn_sim::Nation;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::shell::{Mode, Shell};
use fn_tui::state::Screen;
use fn_tui::{keys, new_session};

const SIZE: (u16, u16) = (120, 40);
/// Four minutes in plus a little: Fire's traders have made camp.
const CAMPED: &str = "<tick 2500>";

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn text(app: &mut App) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    to_text(&render_app(app, &p, SIZE))
}

fn year_two() -> App {
    let args = fn_tui::cli::parse(
        ["--scenario", "earth-year-two", "--frame"].into_iter().map(str::to_owned),
    )
    .unwrap();
    let shell = Shell::for_args(&args, SIZE, None, 0).unwrap();
    let Mode::Game(game) = shell.mode else { panic!("a scenario starts a game") };
    let mut game = *game;
    game.resize(SIZE.0, SIZE.1);
    game
}

#[test]
fn m_opens_the_screen_and_says_the_road_is_closed_in_a_new_game() {
    let mut a = App::new(new_session(7, Nation::Earth), SIZE);
    run(&mut a, "m");
    assert_eq!(a.ui.screen, Some(Screen::Traders));
    let frame = text(&mut a);
    assert!(frame.contains("TRADERS"), "{frame}");
    assert!(frame.contains("The road isn't open yet."), "{frame}");
    assert!(frame.lines().last().unwrap().contains("Esc or M close"), "{frame}");
    run(&mut a, "m");
    assert_eq!(a.ui.screen, None);
    run(&mut a, "m <esc>");
    assert_eq!(a.ui.screen, None);
}

#[test]
fn before_the_caravan_comes_the_screen_says_who_is_due() {
    let mut a = year_two();
    run(&mut a, "m");
    let frame = text(&mut a);
    assert!(frame.contains("Fire Nation traders are due in 4:00."), "{frame}");
}

#[test]
fn a_camped_caravan_lists_deals_and_its_request_and_enter_takes_one() {
    let mut a = year_two();
    run(&mut a, &format!("{CAMPED} m"));
    let frame = text(&mut a);
    for want in ["Fire Nation traders", " left", "DEALS", "REQUEST", "0 of 2 taken", "not taken"] {
        assert!(frame.contains(want), "{want:?} in\n{frame}");
    }
    assert!(frame.contains("Food for the families at Fire Fountain City"), "{frame}");
    assert!(frame.contains(" in stores"), "{frame}");
    assert!(!frame.contains("UNDER WAY"), "nothing taken yet\n{frame}");

    run(&mut a, "<enter>");
    let frame = text(&mut a);
    assert!(frame.contains("1 of 2 taken"), "{frame}");
    assert!(frame.contains("UNDER WAY"), "{frame}");
    assert!(frame.contains(" delivered"), "{frame}");
    assert!(a.ui.status.starts_with("Taken: "), "{}", a.ui.status);
    assert_eq!(a.ui.screen, Some(Screen::Traders), "the screen stays open");

    run(&mut a, "x");
    let frame = text(&mut a);
    assert!(!frame.contains("UNDER WAY"), "{frame}");
    assert!(a.ui.status.starts_with("Cancelled: "), "{}", a.ui.status);
}

#[test]
fn the_pointer_reaches_the_request_and_it_is_taken_once() {
    let mut a = year_two();
    run(&mut a, &format!("{CAMPED} m <down*9> <enter> <enter>"));
    let frame = text(&mut a);
    let request =
        frame.lines().find(|l| l.contains("Food for the families")).expect("the request row");
    assert!(request.contains("▸") && request.contains("taken"), "{request}");
    assert!(!request.contains("not taken"), "{request}");
    assert!(a.ui.status.contains("already taken in full"), "{}", a.ui.status);
}

#[test]
fn the_camp_draws_as_tents_and_the_traders_on_the_map() {
    let mut a = year_two();
    run(&mut a, CAMPED);
    let camp = a.session.world.caravan().expect("camped").camp;
    let frame = text(&mut a);
    let row = |dy: i32| -> String {
        let (x0, y0) = (a.ui.view.x, a.ui.view.y);
        let line = frame.lines().nth((camp.y + dy - y0) as usize).unwrap();
        line.chars().skip((camp.x - x0) as usize).take(2).collect()
    };
    assert_eq!(row(0), "△△", "{frame}");
    assert_eq!(row(1), "☺△", "{frame}");
    let logged = a.ui.alerts.history().iter().any(|e| e.text == "Fire Nation traders made camp");
    assert!(logged, "the alert log has it");
}

#[test]
fn the_key_help_lists_m() {
    let mut a = App::new(new_session(7, Nation::Earth), SIZE);
    run(&mut a, "?");
    let frame = text(&mut a);
    let line = frame.lines().find(|l| l.contains("Traders")).expect("a Traders row");
    assert!(line.contains("M "), "{line}");
}

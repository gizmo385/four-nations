//! The Traders screen (#886): the words are the terminal's, a click is the keys it stands for,
//! and a double-click is `Enter`, which takes the deal. `earth-year-two` has met "Known on the
//! road", so Fire's traders camp four minutes in. Nothing here needs a GPU.

use eframe::egui::{self, Event, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use fn_gui::app::GuiApp;
use fn_gui::frame::{PageKind, Surface};
use fn_gui::page;
use fn_gui::screens::traders::{blocks, click_keys, rows};
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_tui::widgets::Marks;
use fn_ui::keyev::KeyCode;
use fn_ui::state::Screen;
use fn_ui::trade::{CampView, OfferRow, TradersView, view};

const SIZE: (u16, u16) = (120, 40);
/// Four minutes in plus a little: Fire's traders have made camp.
const CAMPED: &str = "<tick 2500>";

fn year_two(script: &str) -> Shell {
    let args = fn_tui::cli::parse(
        ["--scenario", "earth-year-two", "--frame"].into_iter().map(str::to_owned),
    )
    .unwrap();
    let mut shell = Shell::for_args(&args, SIZE, None, 0).unwrap();
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

fn new_game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
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

fn state(shell: &Shell) -> &fn_ui::app::App {
    match &shell.mode {
        Mode::Game(game) => game,
        Mode::Title(_) | Mode::Embark(_) => panic!("not in a game"),
    }
}

fn camp(shell: &Shell) -> CampView {
    match view(&state(shell).session) {
        TradersView::Camp(c) => c,
        TradersView::Away(text) => panic!("no caravan: {text}"),
    }
}

/// What an offer's words are: a deal's give and get, the request's name.
fn words(o: &OfferRow) -> String {
    if o.request { o.name.clone() } else { format!("{} for {}", o.give, o.get) }
}

/// The offer's button: one node holding its rows' words.
fn offer<'a>(h: &'a Harness<'_>, o: &OfferRow) -> egui_kittest::Node<'a> {
    let want = words(o);
    h.query_all_by_role(egui::accesskit::Role::Button)
        .find(|n| n.accesskit_node().label().unwrap_or_default().contains(&want))
        .unwrap_or_else(|| panic!("no button for {want}"))
}

/// Two quick presses at `centre`: a double-click for egui.
fn double_click(h: &mut Harness<'_>, centre: egui::Pos2) {
    h.event(Event::PointerMoved(centre));
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(Event::PointerButton {
                pos: centre,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
    }
    h.step();
}

/// What the keyboard arrives at: the pointer, the screen and the commands (not when they ran:
/// the window's clock runs on a little between frames).
fn snapshot(shell: &Shell) -> (usize, Option<Screen>, String) {
    let app = state(shell);
    let log = app.session.world.command_log();
    let commands = format!("{:?}", log.iter().map(|r| &r.command).collect::<Vec<_>>());
    (app.ui.trade_cursor, app.ui.screen, commands)
}

/// `M` opens a native page: the caravan's name and how long it stays, DEALS and the REQUEST,
/// each offer a button with its words and what the stores hold.
#[test]
fn a_camped_caravan_is_a_page_of_deals_and_a_request() {
    let shell = year_two(&format!("{CAMPED} m"));
    let c = camp(&shell);
    let mut app = GuiApp::with_shell(shell);
    let h = harness(&mut app);
    assert_eq!(h.query_all_by_label_contains("SETTLEMENT").count(), 0, "the side panel");
    h.get_by_label("◆ TRADERS ◆");
    h.get_by_label_contains(&c.name);
    h.get_by_label("DEALS");
    h.get_by_label("REQUEST");
    for o in &c.offers {
        let label = offer(&h, o).accesskit_node().label().unwrap_or_default();
        assert!(label.contains(&o.stores), "{label}");
        assert!(label.contains(&o.taken), "{label}");
    }
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::Page(PageKind::Traders)));
    assert!(app.frame().hint_text.starts_with("TRADERS"), "{}", app.frame().hint_text);
}

/// With no caravan the page says so, as the terminal's does.
#[test]
fn with_no_caravan_the_page_says_when_the_next_is_due() {
    let mut app = GuiApp::with_shell(year_two("m"));
    let h = harness(&mut app);
    h.get_by_label_contains("Fire Nation traders are due in 4:00.");
    drop(h);
    let mut app = GuiApp::with_shell(new_game("m"));
    let h = harness(&mut app);
    h.get_by_label_contains("The road isn't open yet.");
}

/// A click moves the pointer: the very keys the arrows are, onto the request too, and back up.
#[test]
fn a_click_moves_the_pointer_as_the_arrow_keys_do() {
    let c = camp(&year_two(CAMPED));
    let last = c.offers.len() - 1;
    for (from, to) in [(0, 1), (0, last), (last, 0)] {
        let down = if from == 0 { String::new() } else { format!("<down*{from}>") };
        let mut app = GuiApp::with_shell(year_two(&format!("{CAMPED} m {down}")));
        let mut h = harness(&mut app);
        offer(&h, &c.offers[to]).click();
        h.run_steps(3);
        drop(h);
        assert_eq!(state(app.shell()).ui.trade_cursor, to);
        let by_hand = if to == 0 {
            year_two(&format!("{CAMPED} m {down} <up*{from}>"))
        } else {
            year_two(&format!("{CAMPED} m <down*{to}>"))
        };
        assert_eq!(snapshot(app.shell()), snapshot(&by_hand), "{from} to {to}");
    }
}

/// A double-click on a deal is `Enter` there: the deal is taken once, the screen stays open, and
/// the commands and the status are those of the keys.
#[test]
fn a_double_click_takes_the_deal_as_enter_does() {
    let c = camp(&year_two(CAMPED));
    let mut app = GuiApp::with_shell(year_two(&format!("{CAMPED} m")));
    let mut h = harness(&mut app);
    let centre = offer(&h, &c.offers[1]).rect().center();
    double_click(&mut h, centre);
    h.run_steps(3);
    h.get_by_label("UNDER WAY");
    drop(h);
    let taken = state(app.shell());
    assert!(taken.ui.status.starts_with("Taken: "), "{}", taken.ui.status);
    assert_eq!(taken.ui.screen, Some(Screen::Traders));
    let by_hand = year_two(&format!("{CAMPED} m <down> <enter>"));
    assert_eq!(snapshot(app.shell()), snapshot(&by_hand));
    assert!(snapshot(app.shell()).2.contains("TakeDeal"));
}

/// The request is taken once: a second double-click is refused as the keys' second `Enter`.
#[test]
fn the_request_is_taken_once_by_double_click_too() {
    let c = camp(&year_two(CAMPED));
    let request = c.offers.iter().find(|o| o.request).unwrap();
    let mut app = GuiApp::with_shell(year_two(&format!("{CAMPED} m")));
    let mut h = harness(&mut app);
    for _ in 0..2 {
        let centre = offer(&h, request).rect().center();
        double_click(&mut h, centre);
        // Past egui's double-click window, or the next pair of presses is a triple-click.
        h.run_steps(20);
    }
    drop(h);
    assert!(state(app.shell()).ui.status.contains("already taken in full"));
    let by_hand = year_two(&format!("{CAMPED} m <down*{}> <enter> <enter>", request.slot));
    assert_eq!(snapshot(app.shell()), snapshot(&by_hand));
}

/// `X` cancels what is owed on the pointer's offer, from the keyboard on the page.
#[test]
fn x_cancels_on_the_page() {
    let mut app = GuiApp::with_shell(year_two(&format!("{CAMPED} m <enter>")));
    let mut h = harness(&mut app);
    h.get_by_label("UNDER WAY");
    h.event(Event::Text("x".to_owned()));
    h.step();
    h.run_steps(3);
    assert!(h.query_by_label("UNDER WAY").is_none());
    drop(h);
    assert!(state(app.shell()).ui.status.starts_with("Cancelled: "));
}

/// The close button is `Esc`.
#[test]
fn the_close_button_is_esc() {
    let mut app = GuiApp::with_shell(year_two(&format!("{CAMPED} m")));
    let mut h = harness(&mut app);
    h.get_by_label(page::CLOSE).click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(app.shell()).ui.screen, None);
    assert_eq!(snapshot(app.shell()), snapshot(&year_two(&format!("{CAMPED} m <esc>"))));
}

/// The offers sit where the screen's own rows put them: a deal is two rows, the request three.
#[test]
fn the_blocks_are_where_the_rows_put_the_offers() {
    let shell = year_two(&format!("{CAMPED} m"));
    let trade = view(&state(&shell).session);
    let c = camp(&shell);
    let marks = Marks::new(false);
    let found = blocks(&trade, 80, &marks);
    assert_eq!(found.len(), c.offers.len());
    for (&(first, len, slot), o) in found.iter().zip(&c.offers) {
        let (shown, _) = rows(&trade, slot, 80, &marks);
        let text: Vec<String> = shown[first..first + len].iter().map(|r| r.plain()).collect();
        assert!(text[0].contains(&words(o).chars().take(20).collect::<String>()), "{text:?}");
        assert!(text[1].contains(&o.stores), "{text:?}");
        assert_eq!(len, if o.request { 3 } else { 2 });
    }
}

#[test]
fn the_keys_of_a_click_are_the_arrows_then_enter() {
    assert!(click_keys(2, 2, false).is_empty());
    let down: Vec<_> = click_keys(0, 2, true).iter().map(|k| k.code).collect();
    assert_eq!(down, [KeyCode::Down, KeyCode::Down, KeyCode::Enter]);
    let up: Vec<_> = click_keys(3, 2, false).iter().map(|k| k.code).collect();
    assert_eq!(up, [KeyCode::Up]);
}

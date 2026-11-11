//! The title screen's Replays item (#841): it shows only with runs to replay, and the round trip
//! title → Replays → a run → the browser → the title leaves the title screen as it was. Nothing
//! here opens a terminal: the shell takes the key script, and the browser's headless path
//! (`Browser::play`, which `fn_tui --runs DIR --frame` uses) stands in for the nested loop
//! `crate::run` opens on the real terminal.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::SystemTime;

use fn_sim::Nation;
use fn_tui::browser::{Browser, Scoring};
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_shell, to_text};
use fn_tui::keys;
use fn_tui::shell::{Mode, Shell};
use fn_tui::surface::{Host, LogLine};
use fn_tui::title::{MenuItem, TitleScreen};

const SIZE: (u16, u16) = (120, 40);
const GOOD: &str = "20261109-000000-earth-7-normal";
const OTHER_DATA: &str = "20261108-000000-earth-7-normal";

fn fresh_dir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fn_tui_replays_test_{}_{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn game_log() -> String {
    let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap().with_turn_seconds(20);
    h.end_turn(1);
    let end = LogLine::End { tick: 400, reason: "agent finished".to_owned() };
    format!("{}{}\n", h.log().to_jsonl(&h.header()), serde_json::to_string(&end).unwrap())
}

/// A runs folder with one good run and one whose log was made against other game data.
fn runs_folder() -> PathBuf {
    let dir = fresh_dir();
    let log = game_log();
    let at = log.find("\"data_digest\":").expect("a digest") + "\"data_digest\":".len();
    let end = at + log[at..].find(|c: char| !c.is_ascii_digit()).expect("digits end");
    let other = format!("{}1{}", &log[..at], &log[end..]);
    for (name, text) in [(GOOD, &log), (OTHER_DATA, &other)] {
        fs::create_dir_all(dir.join(name)).unwrap();
        fs::write(dir.join(name).join("run.jsonl"), text).unwrap();
    }
    dir
}

fn frame(shell: &mut Shell) -> String {
    let p = Palette::new(shell.defs(), shell.nation(), ColorDepth::TrueColor, false);
    to_text(&render_shell(shell, &p, SIZE))
}

fn title(shell: &Shell) -> TitleScreen {
    match &shell.mode {
        Mode::Title(t) => (**t).clone(),
        other => panic!("expected the title screen, got {other:?}"),
    }
}

fn browser(dir: &Path) -> Browser {
    Browser::open(dir, SystemTime::now(), Scoring::Inline, ColorDepth::Never, false)
}

#[test]
fn replays_is_hidden_without_a_runs_folder_or_with_an_empty_one() {
    let none = Shell::new_title(SIZE, None, 1);
    assert!(!title(&none).items().contains(&MenuItem::Replays));
    let mut empty = Shell::new_title(SIZE, None, 1).with_runs(Some(fresh_dir()));
    assert!(!title(&empty).items().contains(&MenuItem::Replays));
    assert!(!frame(&mut empty).contains("Replays"));
    let missing = Shell::new_title(SIZE, None, 1).with_runs(Some(fresh_dir().join("nope")));
    assert!(!title(&missing).items().contains(&MenuItem::Replays));
}

#[test]
fn title_to_replays_to_a_run_and_back_leaves_the_title_screen_unchanged() {
    let dir = runs_folder();
    let mut shell = Shell::new_title(SIZE, None, 1).with_runs(Some(dir.clone()));
    let text = frame(&mut shell);
    let (load, replays, quit) =
        (text.find("Load").unwrap(), text.find("Replays").unwrap(), text.find("Quit").unwrap());
    assert!(load < replays && replays < quit, "Replays sits between Load and Quit:\n{text}");
    assert_eq!(shell.take_replays(), None, "nothing chosen yet");

    shell.play(&keys::parse("<down> <down> <enter>").unwrap());
    let before = title(&shell);
    let before_text = frame(&mut shell);
    assert_eq!(before.items()[before.cursor], MenuItem::Replays);
    assert_eq!(shell.take_replays(), Some(dir.clone()), "Enter on Replays asks for the browser");
    assert_eq!(shell.take_replays(), None, "once");

    // What the loop does with it: the browser on that folder, a run opened, `q` back to the
    // list, `q` out.
    let mut b = browser(&dir);
    assert_eq!(b.selected_name(), Some(GOOD));
    b.play(&keys::parse("<enter>").unwrap());
    assert!(b.is_watching(), "Enter opens the run in the watcher");
    b.play(&keys::parse("q").unwrap());
    assert!(!b.is_watching() && !b.quit(), "q in the watcher comes back to the browser");
    b.play(&keys::parse("q").unwrap());
    assert!(b.quit(), "q in the browser leaves it");

    assert_eq!(title(&shell), before, "the title screen is as it was, cursor on Replays");
    assert_eq!(frame(&mut shell), before_text);
}

#[test]
fn a_run_made_against_other_data_shows_a_notice_and_does_not_open() {
    let dir = runs_folder();
    let mut b = browser(&dir);
    b.play(&keys::parse("<down>").unwrap());
    assert_eq!(b.selected_name(), Some(OTHER_DATA));
    b.play(&keys::parse("<enter>").unwrap());
    assert!(!b.is_watching());
    let f = to_text(&b.render(SIZE));
    assert!(f.contains("different data"), "the browser says why:\n{f}");
}

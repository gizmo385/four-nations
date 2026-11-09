//! The web replayer shows what the terminal watcher shows (#595's acceptance criteria): an
//! animated replay, and any seek (forwards, backwards past a snapshot, to the start), end on the
//! same frame as `fn_tui --watch LOG --frame --at TURN` (`main.rs::print_watch_frame`'s steps).

use fn_sim::Nation;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::to_text;
use fn_tui::surface::Host;
use fn_tui::watch::{self, Watcher};
use fn_web::{Replay, SNAPSHOT_EVERY, START_PACE};

const SIZE: (u16, u16) = (160, 45);

/// A log of `turns` turns, with a designation in the first and a cursor walk every few.
fn log(turns: u32) -> String {
    let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap();
    h.press_keys(
        "h <right*3> <down*2> <enter> <right*4> <down*3> <enter>",
        Some("harvest".into()),
        None,
    )
    .unwrap();
    for t in 0..turns {
        if t % 7 == 3 {
            h.press_keys("<left*2> <up>", None, None).unwrap();
        }
        h.end_turn(1);
    }
    h.log().to_jsonl(&h.header())
}

/// `fn_tui --watch LOG --frame --at turn` at [`START_PACE`], as text.
fn reference(text: &str, turn: u32) -> String {
    let (header, lines) = watch::parse(text).unwrap();
    let mut w = Watcher::new(&header).unwrap();
    while w.pace() < START_PACE {
        w.faster();
    }
    w.feed(lines);
    w.fast_forward_to_turn(turn);
    w.set_frame_size(SIZE.0, SIZE.1);
    let palette = Palette::new(w.defs(), w.nation(), ColorDepth::TrueColor, false);
    to_text(&w.render(&palette, SIZE))
}

fn frame(r: &mut Replay) -> String {
    to_text(&r.render(SIZE.0, SIZE.1))
}

#[test]
fn an_animated_replay_ends_on_the_terminal_watchers_last_frame() {
    let text = log(6);
    let mut r = Replay::new(&text).unwrap();
    assert_eq!(r.total_turns(), 6);
    for _ in 0..200_000 {
        if r.at_end() {
            break;
        }
        r.advance(1.0 / 60.0);
    }
    assert!(r.at_end());
    // Let any attention ghost fade, as it has by the time a still frame is printed.
    r.advance(5.0);
    assert_eq!(frame(&mut r), reference(&text, u32::MAX));
}

#[test]
fn seeks_forwards_backwards_and_to_the_start_match_the_terminal_watcher() {
    let turns = SNAPSHOT_EVERY * 2 + 5;
    let text = log(turns);
    let mut r = Replay::new(&text).unwrap();

    // Forwards past two snapshot turns, then backwards to between them, then to exactly a
    // snapshot's own turn, then the start, then the end.
    for target in [turns - 2, SNAPSHOT_EVERY + 3, SNAPSHOT_EVERY, 0, turns] {
        r.seek_now(target);
        assert_eq!(r.turn(), target, "seek to {target}");
        assert_eq!(frame(&mut r), reference(&text, target), "frame after a seek to {target}");
    }
}

#[test]
fn a_seek_steps_one_turn_at_a_time_and_play_resumes_after_it() {
    let text = log(10);
    let mut r = Replay::new(&text).unwrap();
    r.seek(4);
    let mut steps = 0;
    while r.seek_step() {
        steps += 1;
    }
    assert_eq!((r.turn(), steps), (4, 3), "four turns, the last step reporting done");
    assert!(!r.seeking());
    for _ in 0..200_000 {
        if r.at_end() {
            break;
        }
        r.advance(0.05);
    }
    assert_eq!(r.turn(), 10);
}

#[test]
fn a_log_made_against_other_data_names_its_build() {
    let text = log(1);
    let (mut header, _) = watch::parse(&text).unwrap();
    header.data_digest = header.data_digest.wrapping_add(1);
    header.build = Some("abc1234".into());
    let first = serde_json::to_string(&fn_tui::surface::LogLine::Header(header)).unwrap();
    let rest = text.split_once('\n').unwrap().1;
    let err = match Replay::new(&format!("{first}\n{rest}")) {
        Ok(_) => panic!("a mismatched digest should be refused"),
        Err(e) => e,
    };
    assert!(err.contains("different data") && err.contains("abc1234"), "{err}");
}

#[test]
fn the_c_abi_opens_renders_and_reports_errors() {
    use fn_web::abi::*;

    let text = log(2);
    let ptr = fw_input(text.len());
    // SAFETY: `fw_input` just returned a buffer of exactly this many bytes.
    unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), ptr, text.len()) };
    assert_eq!(fw_open(), 1);
    assert_eq!(fw_total_turns(), 2);
    assert_eq!(fw_pace(), START_PACE);
    fw_seek(2);
    while fw_seek_step() == 1 {}
    assert_eq!(fw_turn(), 2);
    assert!(!fw_render(SIZE.0, SIZE.1).is_null());

    let bad = b"not a log";
    let ptr = fw_input(bad.len());
    // SAFETY: as above.
    unsafe { std::ptr::copy_nonoverlapping(bad.as_ptr(), ptr, bad.len()) };
    assert_eq!(fw_open(), 0);
    // SAFETY: the text buffer is valid until the next call.
    let message = unsafe { std::slice::from_raw_parts(fw_text_ptr(), fw_text_len()) };
    assert!(!message.is_empty());
}

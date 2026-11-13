//! The page's C ABI (crates/web/src/abi.rs) over `fn_tui::replay::Replay`. The replay itself is
//! tested in fn_tui (`crates/tui/tests/replayer.rs`).

use fn_sim::Nation;
use fn_tui::surface::Host;
use fn_web::START_PACE;

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

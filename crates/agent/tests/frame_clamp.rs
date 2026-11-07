//! `frame`'s own size clamp (#334): a `w`/`h` below the renderer's minimum (80x24) still renders
//! the whole game, at that minimum, and the tool result says so through `clamped` rather than
//! staying silent about the size it actually used.

use fn_agent::runlog::Writer;
use fn_agent::tools::Tools;
use fn_tui::surface::Host;
use serde_json::json;

const SEED: u64 = 7;

fn host() -> Host {
    let nation = fn_tui::cli::parse_nation("earth").unwrap();
    Host::new(SEED, nation, None, None, None, false).unwrap()
}

/// A request below the renderer's minimum is clamped up to it, and the result names the size
/// actually used instead of rendering `draw::too_small_text`'s one-line notice.
#[test]
fn a_too_small_frame_request_is_clamped_and_says_so() {
    let mut tools = Tools::new(host(), Writer::new(Vec::new())).unwrap();
    let result = tools.call("frame", json!({"w": 80, "h": 2})).unwrap();
    let frame = result["frame"].as_str().unwrap();
    assert!(!frame.contains("Terminal too small"), "{frame}");
    assert_eq!(result["clamped"], json!({"w": 80, "h": 24}));
}

/// A request already at or above the minimum is untouched, and `clamped` is `null`.
#[test]
fn a_big_enough_frame_request_is_not_clamped() {
    let mut tools = Tools::new(host(), Writer::new(Vec::new())).unwrap();
    let result = tools.call("frame", json!({"w": 120, "h": 40})).unwrap();
    assert_eq!(result["clamped"], serde_json::Value::Null);
}

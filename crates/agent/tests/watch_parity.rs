//! End-to-end: `fn_agent serve` over in-memory stdio, through a short session (an `act`, a read,
//! and an `end_turn`), writing a real log file; `fn_tui --watch LOG --frame --at N` then renders
//! the same frame the `Host` that served it would (#324's acceptance criteria). This is the
//! whole point of unifying the schema: a log `fn_agent` writes has to be one `fn_tui --watch`
//! can actually read.

use std::io::BufReader;

use fn_agent::runlog::Writer;
use fn_agent::server;
use fn_agent::tools::Tools;
use fn_tui::surface::{ActionStep, Host};
use fn_tui::watch;

const SEED: u64 = 7;
const TURN_SECONDS: u32 = 10;
const SIZE: (u16, u16) = (100, 40);

/// A fresh `Host` for the reference frame, and the script a session should run through it: a
/// `cursor` act, then an `end_turn`, matching what the MCP script below asks for.
fn reference_frame() -> String {
    let nation = fn_tui::cli::parse_nation("fire").unwrap();
    let mut h =
        Host::new(SEED, nation, None, None, None, false).unwrap().with_turn_seconds(TURN_SECONDS);
    let steps: Vec<ActionStep> =
        serde_json::from_value(serde_json::json!([{"step": "cursor", "x": 5, "y": 5}])).unwrap();
    h.act(&steps, Some("scout the east".into()));
    h.end_turn(1);
    h.frame(SIZE.0, SIZE.1)
}

#[test]
fn watching_a_log_fn_agent_served_shows_the_same_frame_the_host_that_served_it_had() {
    let want = reference_frame();

    let path = std::env::temp_dir()
        .join(format!("fn_agent_watch_parity_test_{}.jsonl", std::process::id()));
    {
        let nation = fn_tui::cli::parse_nation("fire").unwrap();
        let host = Host::new(SEED, nation, None, None, None, false)
            .unwrap()
            .with_turn_seconds(TURN_SECONDS);
        let writer = Writer::create(&path).unwrap();
        let mut tools = Tools::new(host, writer).unwrap();

        // The MCP session: an act, a read (look, which changes nothing but is logged for a
        // watcher's attention ghosts), then an end_turn -- all over in-memory stdio, the same
        // shape `fn_agent serve`'s real stdin and stdout are.
        let script = [
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"act","arguments":{"steps":[{"step":"cursor","x":5,"y":5}],"note":"scout the east"}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"look","arguments":{"x":0,"y":0,"w":4,"h":4}}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"end_turn","arguments":{"turns":1}}}"#,
        ]
        .join("\n");
        let mut out = Vec::new();
        server::serve(BufReader::new(script.as_bytes()), &mut out, &mut tools);
    }

    // `fn_tui --watch LOG --frame --at 1`: read the finished log, replay it into a fresh
    // `Watcher`, and render the frame at the end of turn 1 (`main::print_watch_frame`'s own
    // path).
    let (header, lines) = watch::read_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    let mut watcher = watch::Watcher::new(&header).unwrap();
    watcher.feed(lines);
    watcher.fast_forward_to_turn(1);

    let palette = fn_tui::color::Palette::new(
        watcher.defs(),
        watcher.nation(),
        fn_tui::color::ColorDepth::TrueColor,
        true,
    );
    let got = fn_tui::frame::to_text(&watcher.render(&palette, SIZE));

    // The watcher's bottom row is the agent strip, not the key hints (by design); compare
    // everything above it.
    let trim_strip =
        |s: &str| s.lines().take(usize::from(SIZE.1) - 1).collect::<Vec<_>>().join("\n");
    assert_eq!(trim_strip(&got), trim_strip(&want));
}

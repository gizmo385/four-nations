//! End-to-end: two `fn_agent serve` sessions against the same log, the second resuming where
//! the first left off (#341) -- the same sequence `fn_agent serve --resume LOG` runs (read the
//! log, `Host::resume`, drop the `end` line and keep appending, `Tools::resume`), proven against
//! a single unbroken session doing the same two rounds in one go.

use std::io::BufReader;

use fn_agent::runlog::Writer;
use fn_agent::server;
use fn_agent::tools::Tools;
use fn_tui::surface::{ActionStep, Host, LogLine};
use fn_tui::watch;

const SEED: u64 = 7;
const TURN_SECONDS: u32 = 10;
const SIZE: (u16, u16) = (100, 40);

/// One `act` (a `cursor` step, noted "scout the east") and one `end_turn`, as an MCP script.
fn round_script() -> String {
    [
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"act","arguments":{"steps":[{"step":"cursor","x":5,"y":5}],"note":"scout the east"}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"end_turn","arguments":{"turns":1}}}"#,
    ]
    .join("\n")
}

fn fresh_host() -> Host {
    let nation = fn_tui::cli::parse_nation("fire").unwrap();
    Host::new(SEED, nation, None, None, None, false).unwrap().with_turn_seconds(TURN_SECONDS)
}

/// `fn_agent serve --resume PATH`'s own steps, over the public API a test can reach (the private
/// `serve_resume` in `main.rs` does exactly this): read `path`, rebuild and validate the `Host`,
/// then wrap it all in a resumed `Tools` that appends to the same file (an `end` line left by the
/// last session stays where it is, #511).
fn resume_tools(path: &std::path::Path) -> Tools<std::io::BufWriter<std::fs::File>> {
    let (header, lines) = watch::read_file(path).unwrap();
    let host = Host::resume(&header, &lines).unwrap();
    let writer = Writer::append(path).unwrap();
    Tools::resume(host, writer, &lines)
}

#[test]
fn a_resumed_session_continues_the_same_log_as_one_unbroken_session() {
    // The reference: one session, never interrupted, running both rounds back to back.
    let mut want = fresh_host();
    let cursor: Vec<ActionStep> =
        serde_json::from_value(serde_json::json!([{"step": "cursor", "x": 5, "y": 5}])).unwrap();
    for _ in 0..2 {
        want.act(&cursor, Some("scout the east".into()));
        want.end_turn(1);
    }
    let want_frame = want.frame(SIZE.0, SIZE.1);

    let path =
        std::env::temp_dir().join(format!("fn_agent_resume_test_{}.jsonl", std::process::id()));

    // Session one: serve, run one round, then stop (the end marker a closed session writes).
    {
        let writer = Writer::create(&path).unwrap();
        let mut tools = Tools::new(fresh_host(), writer).unwrap();
        let mut out = Vec::new();
        let reason = server::serve(BufReader::new(round_script().as_bytes()), &mut out, &mut tools);
        tools.end(&reason).unwrap();
    }

    // The log in between: ends with `end`, the same as any other finished session's log.
    let (_, lines_after_first) = watch::read_file(&path).unwrap();
    assert!(
        matches!(lines_after_first.last(), Some(LogLine::End { .. })),
        "the first session should have left an end marker"
    );

    // Session two: `--resume PATH`, picking the same log back up.
    {
        let mut tools = resume_tools(&path);
        let mut out = Vec::new();
        let reason = server::serve(BufReader::new(round_script().as_bytes()), &mut out, &mut tools);
        tools.end(&reason).unwrap();
    }

    let (header, lines) = watch::read_file(&path).unwrap();
    std::fs::remove_file(&path).ok();

    // The log is append-only (#511): the first session's `end` stays mid-log, and the second
    // session's is the last line.
    let end_count = lines.iter().filter(|l| matches!(l, LogLine::End { .. })).count();
    assert_eq!(end_count, 2, "{lines:?}");
    assert!(matches!(lines.last(), Some(LogLine::End { .. })));

    // `fn_tui --watch` and `fn_agent keys` read it as one continuous run: replaying the whole
    // thing reaches the same frame the unbroken reference session does.
    let mut watcher = watch::Watcher::new(&header).unwrap();
    watcher.feed(lines);
    watcher.fast_forward_to_turn(u32::MAX);
    assert!(watcher.finished(), "the watcher should reach the final end marker");

    let palette = fn_tui::color::Palette::new(
        watcher.defs(),
        watcher.nation(),
        fn_tui::color::ColorDepth::TrueColor,
        true,
    );
    let got = fn_tui::frame::to_text(&watcher.render(&palette, SIZE));
    let trim_strip =
        |s: &str| s.lines().take(usize::from(SIZE.1) - 1).collect::<Vec<_>>().join("\n");
    assert_eq!(trim_strip(&got), trim_strip(&want_frame));
}

/// `fn_agent keys LOG` reads a resumed log the same as any other: the printed script replays
/// both rounds (session one's and session two's) into a fresh game for the same frame, with no
/// gap where the dropped `end` line used to be.
#[test]
fn keys_reads_a_resumed_log_as_one_continuous_run() {
    let path = std::env::temp_dir()
        .join(format!("fn_agent_resume_keys_test_{}.jsonl", std::process::id()));
    {
        let writer = Writer::create(&path).unwrap();
        let mut tools = Tools::new(fresh_host(), writer).unwrap();
        let mut out = Vec::new();
        let reason = server::serve(BufReader::new(round_script().as_bytes()), &mut out, &mut tools);
        tools.end(&reason).unwrap();
    }
    {
        let mut tools = resume_tools(&path);
        let mut out = Vec::new();
        let reason = server::serve(BufReader::new(round_script().as_bytes()), &mut out, &mut tools);
        tools.end(&reason).unwrap();
    }

    let (header, lines) = watch::read_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    let script = watch::to_keys_script(&header, &lines);

    let nation = fn_tui::cli::parse_nation("fire").unwrap();
    let args = fn_tui::cli::Args {
        seed: Some(SEED),
        nation: Some(nation),
        frame: false,
        size: fn_tui::cli::DEFAULT_SIZE,
        ticks: 0,
        keys: Vec::new(),
        ascii: false,
        ansi: false,
        color: None,
        embark: false,
        start: None,
        map_size: None,
        rough: false,
        difficulty: None,
        fast: false,
        scenario: None,
        watch: None,
        pace: None,
        at: None,
        record: None,
        fps: None,
    };
    let mut replay =
        fn_tui::shell::Shell::for_args(&args, fn_tui::cli::DEFAULT_SIZE, None, 0).unwrap();
    replay.play(&fn_tui::keys::parse(&script).unwrap());

    let mut want = fresh_host();
    let cursor: Vec<ActionStep> =
        serde_json::from_value(serde_json::json!([{"step": "cursor", "x": 5, "y": 5}])).unwrap();
    for _ in 0..2 {
        want.act(&cursor, Some("scout the east".into()));
        want.end_turn(1);
    }

    let palette = fn_tui::color::Palette::new(
        replay.defs(),
        nation,
        fn_tui::color::ColorDepth::TrueColor,
        true,
    );
    let got = fn_tui::frame::to_text(&fn_tui::frame::render_shell(&mut replay, &palette, SIZE));
    assert_eq!(got, want.frame(SIZE.0, SIZE.1));
}

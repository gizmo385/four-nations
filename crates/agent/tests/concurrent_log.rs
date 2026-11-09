//! #587: a run log is written by two processes during a playtest (`fn_agent`'s `Writer`, and the
//! `say` lines `xtask agent-run --playtest` appends) while a third reads it (`fn_tui --watch`).
//! Every line must arrive whole, none may be lost, and the reader's offset must never end up
//! inside a line. The "processes" here are threads on separate file handles, which is what the
//! kernel sees too.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use fn_agent::runlog::Writer;
use fn_tui::surface::LogLine;
use fn_tui::watch::LogTail;

const PLANS: usize = 300;
const SAYS: usize = 300;

fn plan_line(i: usize) -> LogLine {
    // Alternates a few hundred bytes (what a real `plan` line is) with well over a `BufWriter`'s
    // 8 KiB buffer, whose text and newline used to go out as two writes.
    let size = if i.is_multiple_of(7) { 20_000 } else { 500 + i % 300 };
    LogLine::Note { text: format!("plan {i} {}", "x".repeat(size)) }
}

fn say_text(i: usize) -> String {
    format!("remark {i} {}", "w ".repeat(10 + i % 40))
}

#[test]
fn a_writer_an_appender_and_a_tail_never_tear_or_lose_a_line() {
    let dir = std::env::temp_dir().join(format!("fn_agent_concurrent_log_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("run.jsonl");

    // The log exists with a first line, as it does when the appender starts.
    let mut writer = Writer::create(&path).unwrap();
    writer.write_line(&LogLine::End { tick: 0, reason: "start".into() }).unwrap();

    let writing_done = Arc::new(AtomicBool::new(false));
    let appending_done = Arc::new(AtomicBool::new(false));

    let appender = {
        let path = path.clone();
        let done = appending_done.clone();
        std::thread::spawn(move || {
            for i in 0..SAYS {
                let line = serde_json::json!({"type": "say", "text": say_text(i)});
                let mut f = OpenOptions::new().append(true).open(&path).unwrap();
                f.write_all(format!("{line}\n").as_bytes()).unwrap();
                std::thread::sleep(Duration::from_micros(300));
            }
            done.store(true, Ordering::SeqCst);
        })
    };
    let writer_thread = {
        let done = writing_done.clone();
        std::thread::spawn(move || {
            for i in 0..PLANS {
                writer.write_line(&plan_line(i)).unwrap();
                std::thread::sleep(Duration::from_micros(200));
            }
            done.store(true, Ordering::SeqCst);
        })
    };

    let mut tail = LogTail::open(&path).unwrap();
    let mut lines = Vec::new();
    loop {
        let finished = writing_done.load(Ordering::SeqCst) && appending_done.load(Ordering::SeqCst);
        lines.extend(tail.poll_lines().unwrap());
        if finished {
            // One more read after both stopped, so nothing written last is missed.
            lines.extend(tail.poll_lines().unwrap());
            break;
        }
    }
    appender.join().unwrap();
    writer_thread.join().unwrap();

    let mut notes = 0;
    let mut says = Vec::new();
    for l in &lines {
        match serde_json::from_str::<LogLine>(&l.text) {
            Ok(LogLine::Note { .. }) => notes += 1,
            Ok(LogLine::Say { text }) => says.push(text),
            Ok(LogLine::End { .. }) => {}
            Ok(other) => panic!("unexpected {other:?}"),
            Err(e) => {
                panic!("line {} (byte {}) does not parse: {e}: {:.80}", l.number, l.offset, l.text)
            }
        }
    }
    assert_eq!(notes, PLANS, "every line the writer wrote arrived");
    let want: Vec<String> = (0..SAYS).map(say_text).collect();
    assert_eq!(says, want, "every remark arrived, in order");

    // And the file on disk is the same lines.
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), lines.len());
    let _ = std::fs::remove_dir_all(&dir);
}

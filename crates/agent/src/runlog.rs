//! The run log writer: flushes each line as it's written, so a watcher tailing the file sees it
//! as it happens (docs/design/agent-play.md, "Racing"). The schema itself is
//! `fn_tui::surface::{RunLogHeader, LogLine}` -- the one shape `fn_tui --watch` reads, so the two
//! can't drift apart (#324) -- and this module is only the buffered-file plumbing `fn_agent`
//! needs that `fn_tui` has no reason to depend on `std::fs` for.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use fn_tui::surface::{LogLine, RunLogHeader};
use serde::Serialize;

/// Writes a run log's header, then one [`LogLine`] a line, flushing after each so a watcher
/// tailing the file (`fn_tui::watch::LogTail`) sees it as soon as it lands.
pub struct Writer<W: Write> {
    out: W,
}

impl<W: Write> Writer<W> {
    pub fn new(out: W) -> Self {
        Self { out }
    }

    /// The log's first line: what the run was made against (`Host::header`).
    pub fn write_header(&mut self, header: &RunLogHeader) -> io::Result<()> {
        self.write_json(&LogLine::Header(header.clone()))
    }

    /// One more [`LogLine`] (`Keys`, `Look`, `Inspect` or `Turn`).
    pub fn write_line(&mut self, line: &LogLine) -> io::Result<()> {
        self.write_json(line)
    }

    /// The line a session ends with (#327): `tick` is the world's own tick when the session
    /// ended, `reason` says why (stdin closed, or an error). A later `--resume` appends after it
    /// (#511), so it is the *run's* end only while it is the log's last line.
    pub fn write_end(&mut self, tick: u64, reason: &str) -> io::Result<()> {
        self.write_json(&LogLine::End { tick, reason: reason.to_owned() })
    }

    fn write_json(&mut self, value: &impl Serialize) -> io::Result<()> {
        let text = serde_json::to_string(value).expect("a run log line always serializes");
        writeln!(self.out, "{text}")?;
        self.out.flush()
    }

    /// The underlying writer, for a test to read back what's been written so far.
    pub fn get_ref(&self) -> &W {
        &self.out
    }
}

impl Writer<BufWriter<File>> {
    /// Creates (or truncates) `path` and wraps it for buffered, line-at-a-time writes.
    pub fn create(path: &Path) -> io::Result<Self> {
        Ok(Self::new(BufWriter::new(File::create(path)?)))
    }

    /// Opens `path` for appending, writing nothing new to it. A run log is append-only (#511):
    /// `fn_agent serve --resume` keeps adding to the log it resumes (a watcher tailing the file
    /// holds a byte offset into it, so nothing already written may move), and the signal
    /// handler (#329) uses it to add one last `End` line to a log a live session is already
    /// writing, without going through that session's own `Writer` (which the signal arrived in
    /// the middle of a blocking stdin read, not holding onto).
    pub fn append(path: &Path) -> io::Result<Self> {
        Ok(Self::new(BufWriter::new(std::fs::OpenOptions::new().append(true).open(path)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> RunLogHeader {
        RunLogHeader {
            seed: 7,
            nation: fn_tui::cli::parse_nation("earth").unwrap(),
            turn_seconds: 20,
            ticks_per_turn: 200,
            map_size: None,
            difficulty: None,
            site: None,
            rough: false,
            data_digest: 42,
            crate_version: "0.1.0".into(),
            prelude: None,
            build: None,
        }
    }

    #[test]
    fn writing_flushes_the_header_then_every_line_as_json_lines() {
        let mut buf = Vec::new();
        {
            let mut w = Writer::new(&mut buf);
            w.write_header(&header()).unwrap();
            w.write_line(&LogLine::Turn { index: 1, tick: 200 }).unwrap();
        }
        let text = String::from_utf8(buf).unwrap();
        let lines: Vec<LogLine> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], LogLine::Header(header()));
        assert_eq!(lines[1], LogLine::Turn { index: 1, tick: 200 });
    }

    /// `append` adds to whatever's already on disk rather than truncating it (#329): the
    /// signal handler's own way of adding one `end` line to a log a live session has already
    /// written to, without disturbing anything that's there.
    #[test]
    fn append_adds_to_what_is_already_on_disk() {
        let path = std::env::temp_dir()
            .join(format!("fn_agent_runlog_append_test_{}.jsonl", std::process::id()));
        {
            let mut w = Writer::create(&path).unwrap();
            w.write_header(&header()).unwrap();
            w.write_line(&LogLine::Turn { index: 1, tick: 200 }).unwrap();
        }
        {
            let mut w = Writer::append(&path).unwrap();
            w.write_end(200, "stopped by SIGTERM").unwrap();
        }

        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();
        let lines: Vec<LogLine> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(
            lines,
            vec![
                LogLine::Header(header()),
                LogLine::Turn { index: 1, tick: 200 },
                LogLine::End { tick: 200, reason: "stopped by SIGTERM".to_owned() },
            ]
        );
    }
}

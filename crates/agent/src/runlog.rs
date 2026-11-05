//! The run log: JSON Lines, one object per event, flushed as it's written so a watcher could
//! follow it live (docs/design/agent-play.md, "Racing"). [`LogEvent`] is the one shape both
//! [`Writer`] (while `serve` runs) and [`read`] (`fn_agent keys`, after it's done) use, so the
//! two can't drift apart.
//!
//! A header names what the run was made against (the seed, the nation, how many ticks one turn
//! is, and `Host::data_digest`, a hash of the bundled data's id tables): a log only replays
//! against the same code and data, the same rule `fn_sim::save` follows for a save file.

use std::fs::File;
use std::io::{self, BufRead, BufWriter, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One line of the log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogEvent {
    /// The first line of every log: what `serve` was started with, and what it was started
    /// against.
    Header {
        seed: u64,
        nation: String,
        turn_seconds: u32,
        /// How many ticks one `end_turn` turn runs (`Host::ticks_per_turn`): what turns `fn_agent
        /// keys` without having to know `TICKS_PER_SECOND` itself.
        ticks_per_turn: u32,
        data_digest: u64,
        crate_version: String,
    },
    /// One `act` tool call: its label, the agent's note, the keys it actually pressed (already
    /// `--keys` tokens), and the reason if a step failed.
    Act { label: Option<String>, note: Option<String>, keys: Vec<String>, error: Option<String> },
    /// A read-only tool call (`briefing`, `observe`, `look`, `inspect`, `screen`, `frame`):
    /// which one and its arguments, for a watcher's attention ghosts later (#319). The result
    /// isn't logged: it's free to recompute, and keeping the log small matters more.
    Read { tool: String, args: Value },
    /// One `end_turn` call: how many turns it actually ran (it can stop early, #315) and the
    /// tick it ended on.
    Turn { turns: u32, tick: u64 },
}

/// Writes [`LogEvent`]s as JSON Lines, flushing after each one.
pub struct Writer<W: Write> {
    out: W,
}

impl<W: Write> Writer<W> {
    pub fn new(out: W) -> Self {
        Self { out }
    }

    pub fn write(&mut self, event: &LogEvent) -> io::Result<()> {
        let line = serde_json::to_string(event).expect("LogEvent always serializes");
        writeln!(self.out, "{line}")?;
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
}

/// Reads every [`LogEvent`] of a log written by [`Writer`] (`fn_agent keys`'s own use), in
/// order. A blank line is skipped; anything else that doesn't parse is an error naming the
/// line.
pub fn read(r: impl BufRead) -> Result<Vec<LogEvent>, String> {
    let mut events = Vec::new();
    for (i, line) in r.lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let event: LogEvent =
            serde_json::from_str(&line).map_err(|e| format!("line {}: {e}", i + 1))?;
        events.push(event);
    }
    Ok(events)
}

/// `fn_agent keys`: the run as a `--keys` script that reproduces its final frame with
/// `fn_tui ... --frame --keys` (docs/design/agent-play.md, "Racing"). `look` and `inspect`
/// leave nothing to replay, the same as a live [`fn_tui::surface::RunLog`]'s own
/// `to_keys_script`; a `turn` becomes `<tick N>` for `turns` times the header's own
/// `ticks_per_turn`, collapsing however many turns one `end_turn` call ran into a single tick
/// count (running them one after another or all at once ends on the same tick either way).
pub fn to_keys_script(events: &[LogEvent]) -> Result<String, String> {
    let mut ticks_per_turn = None;
    let mut tokens = Vec::new();
    for event in events {
        match event {
            LogEvent::Header { ticks_per_turn: t, .. } => ticks_per_turn = Some(*t),
            LogEvent::Act { keys, .. } => tokens.extend(keys.iter().cloned()),
            LogEvent::Turn { turns, .. } => {
                let t = ticks_per_turn
                    .ok_or_else(|| "a turn before the log's own header".to_owned())?;
                tokens.push(format!("<tick {}>", u64::from(t) * u64::from(*turns)));
            }
            LogEvent::Read { .. } => {}
        }
    }
    Ok(tokens.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_then_reading_gives_back_the_same_events() {
        let mut buf = Vec::new();
        {
            let mut w = Writer::new(&mut buf);
            w.write(&LogEvent::Header {
                seed: 7,
                nation: "earth".into(),
                turn_seconds: 20,
                ticks_per_turn: 200,
                data_digest: 42,
                crate_version: "0.1.0".into(),
            })
            .unwrap();
            w.write(&LogEvent::Act {
                label: Some("cursor (1,2)".into()),
                note: None,
                keys: vec!["<right>".into()],
                error: None,
            })
            .unwrap();
            w.write(&LogEvent::Turn { turns: 2, tick: 400 }).unwrap();
        }
        let events = read(buf.as_slice()).unwrap();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], LogEvent::Header { seed: 7, .. }));
        assert!(matches!(events[2], LogEvent::Turn { turns: 2, tick: 400 }));
    }

    #[test]
    fn to_keys_script_joins_act_keys_and_turns_a_turn_into_a_tick_count() {
        let events = vec![
            LogEvent::Header {
                seed: 7,
                nation: "earth".into(),
                turn_seconds: 20,
                ticks_per_turn: 200,
                data_digest: 1,
                crate_version: "0.1.0".into(),
            },
            LogEvent::Act {
                label: None,
                note: None,
                keys: vec!["h".into(), "<right>".into(), "<enter>".into()],
                error: None,
            },
            LogEvent::Turn { turns: 3, tick: 600 },
            LogEvent::Read { tool: "look".into(), args: serde_json::json!({"x": 0}) },
        ];
        let script = to_keys_script(&events).unwrap();
        assert_eq!(script, "h <right> <enter> <tick 600>");
    }

    #[test]
    fn a_turn_before_any_header_is_an_error() {
        let events = vec![LogEvent::Turn { turns: 1, tick: 200 }];
        assert!(to_keys_script(&events).is_err());
    }
}

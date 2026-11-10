//! `agent-run --playtest` (#576): a run played to learn about the game rather than to score, whose
//! model says what it is thinking as it goes.
//!
//! Three parts, none of which touch a campaign or an ordinary `--scripts` run:
//!
//! - **The prompts** ([`first_prompt`], [`handoff_prompt`]): the script prompts' framing (a saved
//!   governor, the plan, notes), with the aim changed to learning how the game plays, and a short
//!   remark asked for before significant actions. The scored prompts are not edited, so they stay
//!   byte for byte what they were (a test pins that, in `agent_run.rs`).
//! - **The stream** ([`run_claude_stream`]): `claude -p --output-format stream-json --verbose`,
//!   read line by line as it arrives. Each plain-text block the model writes is appended to
//!   `run.jsonl` as a `say` line at once, so a watcher sees it live, and a crash keeps what was
//!   already written. The final `result` event is written to `chunk-N.json` the moment it arrives,
//!   in the shape `--output-format json` gives, so cost reading (`read_claude_result`,
//!   `settle_chunk`) is the code it always was, and a run with no result event is an unknown cost
//!   exactly as before. Thinking blocks are skipped: they come through empty, with an opaque
//!   signature, and the signature is never stored.
//! - **`game-problems.md`** ([`write_game_problems`]): the `game problem:` lines of the notes and
//!   the remarks, each once, with the turn it was first said.
//!
//! The reason for streaming rather than reading the transcript at the end of each chunk: the owner
//! wants to watch the remarks as the agent plays, and a transcript only exists once the session is
//! over. The cost is a small race: the remark and the `call` line that follows it are written by
//! two processes, so on a very busy machine the remark can land a line after its call. The feed
//! shows it a line late, never lost.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead as _, BufReader, Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use super::{
    ChunkRun, LogLine, Opts, append_log, otel_attrs, read_claude_result, read_log_lines, sandbox,
    say, script_style_passage,
};
use crate::Result;

/// How many words a remark may run to, as the prompt asks. Short, to keep narration cheap: it is
/// output the model pays for on every significant action, and a long one is a sign it is
/// performing rather than noting.
pub(super) const REMARK_WORDS: u32 = 30;

/// A remark longer than this many characters is cut when it is logged: the prompt asks for
/// [`REMARK_WORDS`] words, so this only bounds a model that ignores it.
const MAX_REMARK_CHARS: usize = 1500;

// -------------------------------------------------------------------------------------------
// Prompts
// -------------------------------------------------------------------------------------------

/// What the playtest adds to a script prompt: the aim, and the two things asked of the model's
/// words (a short remark, and `game problem:` lines).
fn narration_passage() -> String {
    format!(
        "Say what you're thinking as you go, in plain text between your tool calls. Before a \
         significant action (saving or changing the governor, changing the plan, running \
         something for the first time) and whenever something surprises you, write a remark of \
         at most {REMARK_WORDS} words: what you expect, what surprised you, or what is confusing. \
         Skip routine calls. Your remarks are shown live to the people who make the game and \
         kept with the run, so be honest and specific. When something seems wrong with the game \
         itself (a rule or number that contradicts the briefing or what you saw, a screen or \
         tool that misleads you, a result that cannot be right), put it on a line of its own \
         starting \"game problem:\" in a remark, saying what you saw and what you expected. \
         Keep \"SDK gap:\" for what the scripting tools cannot do, and \"game problem:\" for the \
         game."
    )
}

/// `--playtest`'s first prompt: `default_script_prompt` with the aim changed. The people who
/// make the game want to learn how it plays, so the briefing's scoring is not the goal (it is
/// still how the model learns what the game values), and the model is asked to notice and say.
pub(super) fn first_prompt(total: u32, calls: u32) -> String {
    format!(
        "You're playtesting The Four Nations, a colony game, by writing and running scripts \
         through the four-nations tools, rather than acting turn by turn. The people who make \
         the game want to learn how it plays: what is clear, what is confusing, what surprises \
         you, what seems wrong. That matters more than your score, so don't play for a high \
         one: play sensibly, keep everyone alive, and notice things. Call briefing first, then \
         observe, and read sdk_reference. Run the whole settlement from one saved script named \
         \"governor\": a single loop of duties (water, food, beds, stockpile, research, what to \
         build next) that reads what to aim for from plan(). Keep your intent in the plan tool \
         (goal, order, targets, build queue) and change it there: when something needs doing, \
         edit the plan and let the governor do it. Revise the governor with save_script rather \
         than replacing it or starting another one, and only when it can't do something at all. \
         Start it small, run it with run_script for just 3-5 turns to see what it does, then \
         for a season, and once that goes well, for a year. Use inline run_script code only to \
         look (it is read-only: observe, look, cells and the like), never to act. {style} Keep \
         everyone alive and grow the settlement, toward about {total} turns in all.\n\n\
         {narration}\n\nAfter every run_script call, use notes to record what you learned so it \
         survives into the next stretch, and a line starting \"SDK gap:\" for anything you \
         wished the tools could do. This session is one chunk of that: after roughly {calls} \
         tool calls, stop and summarise how it went so the next session can carry on.",
        style = script_style_passage(),
        narration = narration_passage(),
    )
}

/// A resumed playtest chunk's prompt: `scripts_handoff_prompt` with the same aim and narration.
pub(super) fn handoff_prompt(total: u32, done_turns: u32, calls: u32, listing: &str) -> String {
    format!(
        "You're continuing the same playtest of the same settlement from where you left off \
         (turn {done_turns} of about {total} so far), not starting a new one. Here is your own \
         plan, script library and lab notes:\n\n{listing}\n\nCarry on the same way. The aim is \
         still to learn how the game plays, not to score. One saved script, \"governor\", runs \
         the whole settlement from plan(): change what you want with the plan tool, and revise \
         the governor with save_script (don't replace it or start another) only when it can't \
         do something at all. Use inline run_script code only to look; it is read-only. \
         {style}\n\n{narration}\n\nRecord what you learn in notes (and any SDK gap), and let \
         time pass with run_script. After roughly {calls} more tool calls, stop and summarise \
         how it went.",
        style = script_style_passage(),
        narration = narration_passage(),
    )
}

// -------------------------------------------------------------------------------------------
// The stream
// -------------------------------------------------------------------------------------------

/// What one line of `claude`'s stream carries that this cares about.
#[derive(Debug, PartialEq)]
enum Event {
    /// Plain text the model wrote, one entry per text block. `id` names the message it came from,
    /// so a block repeated across events is only taken once.
    Said { id: String, text: String },
    /// A thinking block's summary (`--thinking-display summarized`), one per non-empty block.
    Thought { id: String, text: String },
    /// The final `result` event, verbatim: the same object `--output-format json` prints.
    Result(String),
}

/// Reads one line of `claude -p --output-format stream-json --verbose`. A line that doesn't parse
/// (the last one of a stream cut short, say) is nothing, never an error.
///
/// Only the model's own top-level blocks count: `text` blocks are what it said, and `thinking`
/// blocks with text are summaries of its thinking (an empty one, the default display, is
/// skipped, and the signature is never kept). `tool_use` blocks are skipped, and an event from a
/// sub-agent (`parent_tool_use_id` set) is not the player speaking.
fn parse_event(line: &str) -> Vec<Event> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { return Vec::new() };
    match v["type"].as_str() {
        Some("result") => vec![Event::Result(line.trim().to_owned())],
        Some("assistant") if v["parent_tool_use_id"].is_null() => {
            let id = v["message"]["id"].as_str().unwrap_or_default().to_owned();
            v["message"]["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|b| {
                    let (text, thought) = match b["type"].as_str()? {
                        "text" => (b["text"].as_str()?, false),
                        "thinking" => (b["thinking"].as_str()?, true),
                        _ => return None,
                    };
                    let text = text.trim();
                    if text.is_empty() {
                        return None;
                    }
                    let (id, text) = (id.clone(), text.to_owned());
                    Some(if thought {
                        Event::Thought { id, text }
                    } else {
                        Event::Said { id, text }
                    })
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// `text` as the `say` line it is logged as, cut to [`MAX_REMARK_CHARS`].
fn say_line(text: &str) -> String {
    commentary_line("say", text, MAX_REMARK_CHARS)
}

/// A thinking summary longer than this many characters is cut when it is logged.
const MAX_THOUGHT_CHARS: usize = 4000;

/// `text` as the `think` line it is logged as, cut to [`MAX_THOUGHT_CHARS`].
fn think_line(text: &str) -> String {
    commentary_line("think", text, MAX_THOUGHT_CHARS)
}

/// A `{"type":kind,"text":...}` line, the text cut to `max` characters.
fn commentary_line(kind: &str, text: &str, max: usize) -> String {
    let text = if text.chars().count() > max {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}…")
    } else {
        text.to_owned()
    };
    // Written by hand so `type` comes first, as every line `serde` writes for the log has it
    // (readers of a log's lines look at the start of the line).
    let quoted = serde_json::to_string(&text).expect("a string always serializes");
    format!("{{\"type\":\"{kind}\",\"text\":{quoted}}}\n")
}

/// Appends remarks to `run.jsonl` as they arrive, without ever starting the log or cutting into
/// a line `fn_agent` is writing.
///
/// `fn_agent serve` creates the log (truncating any file already there) when it starts, which can
/// be after the model's first words arrive, and a line of its own may be mid-write: so a remark
/// waits until the log exists and ends on a line break, and is held in memory until then. Every
/// write is one `write_all` of whole lines to a file opened for append, which the kernel places
/// at the end whatever else is appending.
///
/// That only holds if `fn_agent` appends too (#587): it used to write through a plain
/// `File::create` handle, at its own offset, so it overwrote the remarks appended behind its back
/// (25 of a session's 26 were lost) and left a watcher's read position inside a longer line.
/// `Writer::create` now reopens the log in append mode, and writes each line with one `write_all`,
/// so the two processes' lines interleave whole.
struct Sink {
    log: PathBuf,
    pending: Vec<String>,
}

impl Sink {
    fn new(log: &Path) -> Self {
        Self { log: log.to_path_buf(), pending: Vec::new() }
    }

    fn remark(&mut self, text: &str) {
        self.pending.push(say_line(text));
        self.flush();
    }

    fn thought(&mut self, text: &str) {
        self.pending.push(think_line(text));
        self.flush();
    }

    /// Whether the log is there and ends on a complete line.
    fn ready(&self) -> bool {
        let Ok(mut f) = File::open(&self.log) else { return false };
        let Ok(len) = f.metadata().map(|m| m.len()) else { return false };
        if len == 0 || f.seek(SeekFrom::Start(len - 1)).is_err() {
            return false;
        }
        let mut last = [0u8; 1];
        f.read_exact(&mut last).is_ok() && last[0] == b'\n'
    }

    /// Writes what is waiting, if the log is ready for it. True when nothing is left waiting.
    fn flush(&mut self) -> bool {
        if self.pending.is_empty() {
            return true;
        }
        if !self.ready() {
            return false;
        }
        let text = self.pending.concat();
        let wrote = OpenOptions::new()
            .append(true)
            .open(&self.log)
            .and_then(|mut f| f.write_all(text.as_bytes()).and_then(|()| f.flush()));
        if wrote.is_ok() {
            self.pending.clear();
        }
        wrote.is_ok()
    }

    /// The chunk is over: one last try. Whatever still can't be written (the session never
    /// called a tool, so there is no log) is returned, to be reported rather than lost quietly.
    fn finish(mut self) -> Vec<String> {
        // `fn_agent serve` may still be writing its last lines as the session ends (an `end`
        // line after stdin closes): a few short tries let a remark wait out a line in progress
        // rather than be reported as lost, and it still lands in order with what is there.
        for _ in 0..20 {
            if self.flush() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        self.pending
    }
}

/// What reading the stream found, kept apart from the process so it can be tested.
struct Reader {
    sink: Sink,
    seen: HashSet<(String, String)>,
    /// The `result` event, once it arrives.
    result: Option<String>,
    remarks: usize,
    thoughts: usize,
}

impl Reader {
    fn new(log: &Path) -> Self {
        Self { sink: Sink::new(log), seen: HashSet::new(), result: None, remarks: 0, thoughts: 0 }
    }

    fn line(&mut self, line: &str) {
        for event in parse_event(line) {
            match event {
                Event::Said { id, text } => {
                    // A message with no id can't be told from a repeat of another, so it counts.
                    if id.is_empty() || self.seen.insert((id, text.clone())) {
                        self.remarks += 1;
                        self.sink.remark(&text);
                    }
                }
                Event::Thought { id, text } => {
                    if id.is_empty() || self.seen.insert((id, text.clone())) {
                        self.thoughts += 1;
                        self.sink.thought(&text);
                    }
                }
                Event::Result(raw) => self.result = Some(raw),
            }
        }
        // A remark held back for a log that wasn't there yet goes as soon as the log is.
        self.sink.flush();
    }
}

/// One `claude -p ... --output-format stream-json --verbose` call, the playtest's counterpart of
/// `run_claude_p` (same arguments, same stderr to `agent.log`, same [`ChunkRun`] out): stdout is
/// read as it arrives, remarks go into the log at once, and the result is written to `out_json`
/// as soon as it is seen. A stream that ends with no result event leaves `out_json` unwritten,
/// which is an unknown cost.
pub(super) fn run_claude_stream(
    opts: &Opts,
    prompt: &str,
    call_args: &[String],
    mode: &str,
    chunk: u32,
    dir: &Path,
    out_json: &Path,
) -> Result<ChunkRun> {
    let stderr_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("agent.log"))
        .map_err(|e| e.to_string())?;
    let attrs = otel_attrs(opts, dir, mode, Some(chunk));
    let session = sandbox::new_session_id();
    let spawned = sandbox::claude_command(&opts.claude_bin, dir, &session)?
        .arg("-p")
        .arg(prompt)
        .args(call_args)
        .args(["--output-format", "stream-json", "--verbose"])
        // Summaries of the model's thinking (#576): `-p` defaults to an empty display.
        .args(["--thinking-display", "summarized"])
        .env("OTEL_RESOURCE_ATTRIBUTES", attrs)
        .stdout(Stdio::piped())
        .stderr(stderr_file)
        .spawn();
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => {
            return Ok(ChunkRun {
                failure: Some(format!("claude could not be run: {e}")),
                result: None,
            });
        }
    };

    let mut reader = Reader::new(&dir.join("run.jsonl"));
    let mut read_failure = None;
    if let Some(stdout) = child.stdout.take() {
        let mut stdout = BufReader::new(stdout);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match stdout.read_until(b'\n', &mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    reader.line(&String::from_utf8_lossy(&buf));
                    // The result is on disk before anything can go wrong after it.
                    if let Some(raw) = reader.result.take()
                        && let Err(e) = fs::write(out_json, format!("{raw}\n"))
                    {
                        read_failure = Some(format!("writing {}: {e}", out_json.display()));
                    }
                }
                Err(e) => {
                    read_failure = Some(format!("reading claude's output failed: {e}"));
                    break;
                }
            }
        }
    }
    let status = child.wait();
    sandbox::keep_transcript(dir, &session, &format!("chunk-{chunk}"));
    let said = reader.remarks;
    let thought = reader.thoughts;
    let unwritten = reader.sink.finish();
    if !unwritten.is_empty() {
        say(&format!(
            "chunk {chunk}: {} remark(s) couldn't be put in the log (it was never started); \
             they are in agent.log",
            unwritten.len()
        ));
        append_log(
            &dir.join("agent.log"),
            &format!("\n--- unlogged remarks ---\n{}", unwritten.concat()),
        )?;
    }
    say(&format!("chunk {chunk}: {said} remark(s) and {thought} thinking summary(ies) logged"));

    let failure = match status {
        Ok(s) if s.success() => read_failure,
        Ok(s) => Some(format!("claude ended with {s}")),
        Err(e) => Some(format!("claude could not be run: {e}")),
    };
    // A stub or a crash leaves no result event: nothing was written, which reads as no result.
    Ok(ChunkRun { failure, result: read_claude_result(out_json) })
}

// -------------------------------------------------------------------------------------------
// game-problems.md
// -------------------------------------------------------------------------------------------

/// One distinct problem the model flagged.
#[derive(Debug, PartialEq)]
struct Problem {
    /// The words after `game problem:`.
    text: String,
    /// The game turn it was first said at (0 before the first turn).
    turn: u32,
    /// Where it was first said.
    source: &'static str,
    /// The turns it was said again at, in order.
    again: Vec<u32>,
}

/// The `game problem:` lines in `text`, with the prefix and any list bullet taken off.
fn problem_lines(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim().trim_start_matches(['-', '*']).trim_start();
            let head = line.get(..13)?;
            head.eq_ignore_ascii_case("game problem:").then(|| line[13..].trim().to_owned())
        })
        .filter(|rest| !rest.is_empty())
        .collect()
}

/// Two mentions of one problem differ in case, spacing and a closing full stop at most.
fn same_problem_key(text: &str) -> String {
    let squashed = text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    squashed.trim_end_matches(['.', '!']).to_owned()
}

/// The distinct `game problem:` lines of a log's notes and remarks, in the order first said. A
/// carried note appears in the log again in a later session; it counts once, with the repeat
/// listed beside it, so the count is of problems and not of mentions.
fn collect(lines: &[LogLine]) -> (Vec<Problem>, usize) {
    let mut turn = 0;
    let mut out: Vec<Problem> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    let mut mentions = 0;
    for line in lines {
        let (text, source) = match line {
            LogLine::Turn { index, .. } => {
                turn = turn.max(*index);
                continue;
            }
            LogLine::Note { text } => (text, "note"),
            LogLine::Say { text } => (text, "remark"),
            _ => continue,
        };
        for found in problem_lines(text) {
            mentions += 1;
            let key = same_problem_key(&found);
            match keys.iter().position(|k| *k == key) {
                Some(at) => {
                    if out[at].turn != turn && !out[at].again.contains(&turn) {
                        out[at].again.push(turn);
                    }
                }
                None => {
                    keys.push(key);
                    out.push(Problem { text: found, turn, source, again: Vec::new() });
                }
            }
        }
    }
    (out, mentions)
}

fn render(problems: &[Problem], mentions: usize) -> String {
    let mut out = String::from("# Game problems\n\n");
    if problems.is_empty() {
        out.push_str("None flagged this run.\n");
        return out;
    }
    out.push_str(&format!(
        "{} distinct problem(s) from {mentions} mention(s), in the order first said. These are \
         what the model said seemed wrong with the game: its claims, not checked bugs. A \
         problem said again is listed once, with the turns it came back at.\n\n",
        problems.len()
    ));
    for p in problems {
        let again = if p.again.is_empty() {
            String::new()
        } else {
            let turns: Vec<String> = p.again.iter().map(u32::to_string).collect();
            format!(" (said again at turn {})", turns.join(", "))
        };
        out.push_str(&format!("- turn {} ({}): {}{again}\n", p.turn, p.source, p.text));
    }
    out
}

/// `dir/game-problems.md` (#576): what the model flagged as wrong with the game, from the notes
/// and the remarks of `dir/run.jsonl`, each distinct problem once with the turn it was first
/// said. Written after every chunk, so an interrupted run has it too.
pub(super) fn write_game_problems(dir: &Path) -> Result {
    let lines = read_log_lines(&dir.join("run.jsonl")).unwrap_or_default();
    let (problems, mentions) = collect(&lines);
    fs::write(dir.join("game-problems.md"), render(&problems, mentions))
        .map_err(|e| format!("writing game-problems.md: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assistant(blocks: serde_json::Value) -> String {
        serde_json::json!({
            "type": "assistant",
            "message": {"id": "msg_1", "role": "assistant", "content": blocks},
            "parent_tool_use_id": null,
            "session_id": "s",
        })
        .to_string()
    }

    fn said(text: &str) -> Event {
        Event::Said { id: "msg_1".into(), text: text.into() }
    }

    #[test]
    fn text_blocks_are_remarks_and_thinking_and_tool_calls_are_not() {
        let line = assistant(serde_json::json!([
            {"type": "thinking", "thinking": "", "signature": "OPAQUE-SIGNATURE"},
            {"type": "text", "text": "  I expect water nearby.  "},
            {"type": "tool_use", "id": "t1", "name": "mcp__four-nations__observe", "input": {}},
            {"type": "text", "text": "   "},
        ]));
        assert_eq!(parse_event(&line), vec![said("I expect water nearby.")]);
    }

    #[test]
    fn a_summarized_thinking_block_is_a_thought_and_its_signature_is_dropped() {
        let line = assistant(serde_json::json!([
            {"type": "thinking", "thinking": " The well is close. ", "signature": "OPAQUE"},
            {"type": "text", "text": "Building the well."},
        ]));
        let events = parse_event(&line);
        assert_eq!(
            events,
            vec![
                Event::Thought { id: "msg_1".into(), text: "The well is close.".into() },
                said("Building the well."),
            ]
        );
        assert_eq!(
            think_line("The well is close."),
            "{\"type\":\"think\",\"text\":\"The well is close.\"}\n"
        );
    }

    #[test]
    fn a_signature_never_reaches_a_remark_or_its_log_line() {
        let line = assistant(serde_json::json!([
            {"type": "thinking", "thinking": "", "signature": "OPAQUE-SIGNATURE"},
            {"type": "text", "text": "hello"},
        ]));
        for e in parse_event(&line) {
            if let Event::Said { text, .. } = e {
                assert!(!say_line(&text).contains("OPAQUE"));
            }
        }
    }

    #[test]
    fn the_result_event_is_taken_verbatim_and_other_events_are_not() {
        let result =
            r#"{"type":"result","subtype":"success","total_cost_usd":0.42,"result":"done"}"#;
        assert_eq!(parse_event(result), vec![Event::Result(result.to_owned())]);
        for other in [
            r#"{"type":"system","subtype":"init"}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"x"}]}}"#,
            "not json at all",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"cut"#,
            "",
        ] {
            assert!(parse_event(other).is_empty(), "{other}");
        }
    }

    #[test]
    fn a_sub_agents_text_is_not_the_players() {
        let line = serde_json::json!({
            "type": "assistant",
            "message": {"id": "m", "content": [{"type": "text", "text": "sub-agent talk"}]},
            "parent_tool_use_id": "toolu_1",
        })
        .to_string();
        assert!(parse_event(&line).is_empty());
    }

    #[test]
    fn long_remarks_are_cut_and_the_line_is_one_json_line() {
        let long = "word ".repeat(1000);
        let line = say_line(&long);
        assert!(line.ends_with("\n") && line.matches('\n').count() == 1);
        let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(v["type"], "say");
        assert!(v["text"].as_str().unwrap().chars().count() <= MAX_REMARK_CHARS + 1);
        // Multi-line remarks stay one line of the log.
        let multi = say_line("one\ntwo \"quoted\"");
        assert_eq!(multi.matches('\n').count(), 1);
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fn_playtest_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The remark waits for a log that `fn_agent` has not started yet, never starts it, and does
    /// not cut into a half-written line.
    #[test]
    fn a_remark_waits_for_the_log_and_for_a_whole_line() {
        let dir = tmp("sink");
        let log = dir.join("run.jsonl");
        let mut r = Reader::new(&log);
        r.line(&assistant(serde_json::json!([{"type": "text", "text": "early words"}])));
        assert!(!log.exists(), "the sink must not create the log");

        fs::write(&log, "{\"type\":\"header\"}\n{\"type\":\"turn\",").unwrap();
        r.line("");
        assert_eq!(fs::read_to_string(&log).unwrap().matches("say").count(), 0, "mid-line");

        fs::write(&log, "{\"type\":\"header\"}\n{\"type\":\"turn\",\"index\":1,\"tick\":2}\n")
            .unwrap();
        r.line("");
        let text = fs::read_to_string(&log).unwrap();
        assert!(text.ends_with("{\"type\":\"say\",\"text\":\"early words\"}\n"), "{text}");
        assert!(r.sink.finish().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    /// A block repeated across events (the same message id and text) is one remark; the same text
    /// in a later message is another.
    #[test]
    fn a_repeated_block_is_one_remark_and_a_later_one_is_another() {
        let dir = tmp("dedupe");
        let log = dir.join("run.jsonl");
        fs::write(&log, "{\"type\":\"header\"}\n").unwrap();
        let mut r = Reader::new(&log);
        let one = assistant(serde_json::json!([{"type": "text", "text": "same"}]));
        r.line(&one);
        r.line(&one);
        let two = one.replace("msg_1", "msg_2");
        r.line(&two);
        assert_eq!(r.remarks, 2);
        // One block per event under one message id (#587): different text under the same id is
        // not a repeat.
        r.line(&assistant(serde_json::json!([{"type": "text", "text": "other"}])));
        assert_eq!(r.remarks, 3);
        assert_eq!(fs::read_to_string(&log).unwrap().matches("\"say\"").count(), 3);
        let _ = fs::remove_dir_all(&dir);
    }

    /// A real stream (#587), cut down to the fields read here: `claude -p --output-format
    /// stream-json --verbose` sends one content block per `assistant` event (each event with its
    /// own message id here), `thinking` and `tool_use` blocks between the text. Every text block
    /// becomes one `say` line, in order, and the result event is the last thing seen. Blocks
    /// sharing an id are `a_repeated_block_is_one_remark_and_a_later_one_is_another`'s.
    #[test]
    fn every_text_block_of_a_real_stream_is_one_say_line_in_order() {
        const STREAM: &str = include_str!("playtest_stream.jsonl");
        let dir = tmp("real_stream");
        let log = dir.join("run.jsonl");
        fs::write(&log, "{\"type\":\"header\"}\n").unwrap();
        let mut r = Reader::new(&log);
        let mut want = Vec::new();
        for line in STREAM.lines() {
            for e in parse_event(line) {
                if let Event::Said { text, .. } = e {
                    want.push(text);
                }
            }
            r.line(line);
            // What `fn_agent` adds as the model works: calls between the remarks.
            if line.contains("tool_use") {
                let mut f = OpenOptions::new().append(true).open(&log).unwrap();
                f.write_all(b"{\"type\":\"call\",\"tool\":\"x\",\"summary\":\"\"}\n").unwrap();
            }
        }
        assert!(want.len() >= 10, "the fixture has the real session's remarks: {}", want.len());
        assert_eq!(r.remarks, want.len());
        assert!(r.result.as_deref().is_some_and(|r| r.contains("total_cost_usd")));
        let said: Vec<String> = fs::read_to_string(&log)
            .unwrap()
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter(|v| v["type"] == "say")
            .map(|v| v["text"].as_str().unwrap().to_owned())
            .collect();
        // The long closing report is cut to the logged length; the rest are verbatim.
        let want: Vec<String> = want
            .iter()
            .map(|t| {
                let v: serde_json::Value = serde_json::from_str(say_line(t).trim()).unwrap();
                v["text"].as_str().unwrap().to_owned()
            })
            .collect();
        assert_eq!(said, want);
        assert!(r.sink.finish().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remarks_that_never_found_a_log_are_handed_back() {
        let dir = tmp("nolog");
        let mut r = Reader::new(&dir.join("run.jsonl"));
        r.line(&assistant(serde_json::json!([{"type": "text", "text": "lost?"}])));
        let left = r.sink.finish();
        assert_eq!(left.len(), 1);
        assert!(left[0].contains("lost?"));
        let _ = fs::remove_dir_all(&dir);
    }

    fn note(t: &str) -> LogLine {
        LogLine::Note { text: t.into() }
    }
    fn says(t: &str) -> LogLine {
        LogLine::Say { text: t.into() }
    }
    fn turns(from: u32, to: u32) -> Vec<LogLine> {
        (from..=to).map(|index| LogLine::Turn { index, tick: u64::from(index) }).collect()
    }

    #[test]
    fn problems_come_from_notes_and_remarks_once_each_with_their_turn() {
        let mut lines = vec![says("Starting. game problem: not at line start")];
        lines.extend(turns(1, 3));
        lines.push(says("Odd result.\nGame Problem: The well gives no water."));
        lines.extend(turns(4, 6));
        lines.push(note("SDK gap: x\n- game problem: the well gives no water\nother"));
        lines.extend(turns(7, 9));
        // The same note carried into a later session appears in the log again.
        lines.push(note("game problem: THE WELL gives   no water!"));
        lines.push(says("* game problem: bed count in the briefing is 4, I see 3"));
        let (problems, mentions) = collect(&lines);
        assert_eq!(mentions, 4);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert_eq!(
            problems[0],
            Problem {
                text: "The well gives no water.".into(),
                turn: 3,
                source: "remark",
                again: vec![6, 9],
            }
        );
        assert_eq!((problems[1].turn, problems[1].source), (9, "remark"));
        assert!(problems[1].text.starts_with("bed count"));
    }

    #[test]
    fn the_file_says_how_many_and_none_when_there_are_none() {
        let dir = tmp("md");
        let log = dir.join("run.jsonl");
        fs::write(&log, "{\"type\":\"turn\",\"index\":1,\"tick\":2}\n").unwrap();
        write_game_problems(&dir).unwrap();
        let none = fs::read_to_string(dir.join("game-problems.md")).unwrap();
        assert!(none.contains("None flagged this run."), "{none}");

        let text = format!(
            "{}{}\n{}\n",
            fs::read_to_string(&log).unwrap(),
            serde_json::json!({"type": "say", "text": "game problem: A"}),
            serde_json::json!({"type": "note", "text": "game problem: a"}),
        );
        fs::write(&log, text).unwrap();
        write_game_problems(&dir).unwrap();
        let some = fs::read_to_string(dir.join("game-problems.md")).unwrap();
        assert!(some.contains("1 distinct problem(s) from 2 mention(s)"), "{some}");
        assert!(some.contains("- turn 1 (remark): A"), "{some}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_playtest_prompt_asks_for_short_remarks_and_game_problems_and_says_the_aim() {
        let p = first_prompt(40, 30);
        assert!(p.contains("playtesting"), "{p}");
        assert!(p.contains("don't play for a high one"), "{p}");
        assert!(p.contains(&format!("at most {REMARK_WORDS} words")), "{p}");
        assert!(p.contains("\"game problem:\"") && p.contains("\"SDK gap:\""), "{p}");
        assert!(p.contains("about 40 turns") && p.contains("roughly 30 tool calls"), "{p}");
        assert!(p.contains("\"governor\"") && p.contains("plan"), "{p}");
        let h = handoff_prompt(40, 12, 30, "LISTING");
        assert!(h.contains("LISTING") && h.contains("turn 12 of about 40"), "{h}");
        assert!(h.contains(&format!("at most {REMARK_WORDS} words")), "{h}");
        assert!(h.contains("\"game problem:\""), "{h}");
    }
}

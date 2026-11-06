//! MCP over stdio: JSON-RPC 2.0, hand-rolled (no SDK), one JSON object per line in both
//! directions (docs/design/agent-play.md, "Architecture"). [`serve`] reads from anything
//! [`BufRead`] and writes to anything [`Write`], so a test can drive it over an in-memory pipe
//! the same shape `fn_agent serve`'s real stdin and stdout are.
//!
//! Three methods: `initialize`, `tools/list` and `tools/call`
//! ([`crate::tools::Tools::call`]). A request with no `id` is a notification (`initialize`'s own
//! client sends `notifications/initialized` after it): it gets no response, whatever its method,
//! the same as the spec says for any JSON-RPC notification. The loop reads until EOF and returns
//! then, never blocking on a TTY and never panicking on a line it can't parse.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use crate::tools::{self, Tools};

const PARSE_ERROR: i64 = -32700;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Reads one JSON-RPC message per line from `input` until EOF, dispatches it against `tools`,
/// and writes one response per line to `output` for every request that had an `id` (a
/// notification gets none). Returns once `input` is exhausted, with the reason why (#327):
/// "stdin closed" on plain EOF, or the I/O error that stopped the loop early -- what the caller
/// writes as the run log's own `end` line (`Tools::end`).
pub fn serve<W: std::io::Write>(
    input: impl BufRead,
    mut output: impl Write,
    tools: &mut Tools<W>,
) -> String {
    for line in input.lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => return format!("stdin read error: {e}"),
        };
        if line.trim().is_empty() {
            continue;
        }
        let parsed: Result<Value, _> = serde_json::from_str(&line);
        let request = match parsed {
            Ok(v) => v,
            Err(_) => {
                write_error(&mut output, Value::Null, PARSE_ERROR, "invalid JSON");
                continue;
            }
        };
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str).unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id else {
            // A notification: run it for effect (there is none we act on yet) and move on.
            continue;
        };
        match dispatch(method, params, tools) {
            Ok(result) => write_response(&mut output, id, result),
            Err((code, message)) => write_error(&mut output, id, code, &message),
        }
    }
    "stdin closed".to_owned()
}

fn dispatch<W: std::io::Write>(
    method: &str,
    params: Value,
    tools: &mut Tools<W>,
) -> Result<Value, (i64, String)> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "fn_agent", "version": env!("CARGO_PKG_VERSION")},
        })),
        "tools/list" => Ok(json!({"tools": tools::list()})),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| (INVALID_PARAMS, "tools/call needs a name".to_owned()))?;
            let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            match tools.call(name, arguments) {
                Ok(result) => Ok(json!({
                    "content": [{"type": "text", "text": result.to_string()}],
                })),
                Err(reason) => Ok(json!({
                    "content": [{"type": "text", "text": reason}],
                    "isError": true,
                })),
            }
        }
        other => Err((METHOD_NOT_FOUND, format!("no such method {other:?}"))),
    }
}

fn write_response(output: &mut impl Write, id: Value, result: Value) {
    write_line(output, &json!({"jsonrpc": "2.0", "id": id, "result": result}));
}

fn write_error(output: &mut impl Write, id: Value, code: i64, message: &str) {
    write_line(
        output,
        &json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
    );
}

fn write_line(output: &mut impl Write, value: &Value) {
    let _ = writeln!(output, "{value}");
    let _ = output.flush();
}

#[cfg(test)]
mod tests {
    use fn_tui::surface::Host;

    use super::*;
    use crate::runlog::Writer;

    fn tools() -> Tools<Vec<u8>> {
        let host =
            Host::new(7, fn_tui::cli::parse_nation("earth").unwrap(), None, None, None, false)
                .unwrap();
        Tools::new(host, Writer::new(Vec::new())).unwrap()
    }

    fn call(input: &str) -> Vec<Value> {
        let mut out = Vec::new();
        serve(input.as_bytes(), &mut out, &mut tools());
        String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    #[test]
    fn initialize_tools_list_and_a_tool_call_in_one_session() {
        let script = [
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"observe","arguments":{}}}"#,
        ]
        .join("\n");
        let responses = call(&script);
        // The notification got no response: three requests, three responses.
        assert_eq!(responses.len(), 3);

        assert_eq!(responses[0]["id"], 1);
        assert_eq!(responses[0]["result"]["serverInfo"]["name"], "fn_agent");

        assert_eq!(responses[1]["id"], 2);
        let tool_list = responses[1]["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tool_list.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            ["briefing", "observe", "look", "inspect", "screen", "frame", "act", "end_turn"]
        );

        assert_eq!(responses[2]["id"], 3);
        let text = responses[2]["result"]["content"][0]["text"].as_str().unwrap();
        let observe: Value = serde_json::from_str(text).unwrap();
        assert_eq!(observe["tool"], "Select");
        assert_eq!(observe["people"]["total"], 6);
    }

    #[test]
    fn an_unknown_method_is_a_json_rpc_error_and_an_unknown_tool_is_a_tool_error() {
        let responses = call(
            &[
                r#"{"jsonrpc":"2.0","id":1,"method":"nonsense"}"#,
                r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"nonsense"}}"#,
            ]
            .join("\n"),
        );
        assert_eq!(responses[0]["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(responses[1]["result"]["isError"], true);
    }

    /// `serve`'s own return value (#327): "stdin closed" on a plain, clean EOF -- what
    /// `fn_agent serve` writes as the run log's `end` line once stdin closes.
    #[test]
    fn serve_returns_stdin_closed_on_a_clean_eof() {
        let mut out = Vec::new();
        let reason = serve(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#.as_bytes(),
            &mut out,
            &mut tools(),
        );
        assert_eq!(reason, "stdin closed");
    }

    /// A line `BufRead` can't even read (not valid UTF-8) stops the loop early with that error
    /// as the reason, instead of "stdin closed" claiming a clean end it wasn't.
    #[test]
    fn serve_returns_the_read_error_when_a_line_cannot_be_read() {
        let mut out = Vec::new();
        let bad: &[u8] = &[0xff, 0xfe, b'\n'];
        let reason = serve(bad, &mut out, &mut tools());
        assert_ne!(reason, "stdin closed", "{reason}");
        assert!(reason.contains("stdin read error"), "{reason}");
    }

    #[test]
    fn invalid_json_gets_a_parse_error_and_the_session_keeps_going() {
        let responses = call(
            &["not json at all", r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#].join("\n"),
        );
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["error"]["code"], PARSE_ERROR);
        assert_eq!(responses[1]["id"], 1);
    }

    #[test]
    fn act_and_end_turn_drive_the_game_through_the_same_session() {
        let mut t = tools();
        let mut out = Vec::new();
        let script = [
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"act","arguments":{"steps":[{"step":"cursor","x":5,"y":5}],"note":"scout the east"}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"end_turn","arguments":{"turns":1}}}"#,
        ]
        .join("\n");
        serve(script.as_bytes(), &mut out, &mut t);
        let responses: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let act_text = responses[0]["result"]["content"][0]["text"].as_str().unwrap();
        let act: Value = serde_json::from_str(act_text).unwrap();
        assert!(act["error"].is_null(), "{act}");
        assert_eq!(act["observe"]["cursor"]["x"], 5);
        assert!(act["keys"].as_array().unwrap().contains(&json!("<up>")));

        let turn_text = responses[1]["result"]["content"][0]["text"].as_str().unwrap();
        let turn: Value = serde_json::from_str(turn_text).unwrap();
        assert_eq!(turn["turn"], 1);

        // The log holds the header (written at `Tools::new`), the act's keys and label, and the
        // turn, in order -- the one schema `fn_tui::surface::{RunLogHeader, LogLine}` defines
        // (#324), which is also what `Host::log` itself would have recorded for the same calls.
        let log_lines: Vec<Value> = String::from_utf8(t.log().get_ref().clone())
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(log_lines.len(), 3, "{log_lines:?}");
        assert_eq!(log_lines[0]["type"], "header");
        assert_eq!(log_lines[0]["seed"], 7);
        assert_eq!(log_lines[0]["nation"], "earth");
        assert_eq!(log_lines[1]["type"], "keys");
        assert_eq!(log_lines[1]["label"], "cursor (5,5)");
        assert_eq!(log_lines[1]["note"], "scout the east");
        assert!(log_lines[1]["keys"].as_str().unwrap().contains("<up>"));
        assert_eq!(log_lines[2]["type"], "turn");
        assert_eq!(log_lines[2]["index"], 1);
    }

    /// `look` and `inspect` are logged too (a watcher's attention ghosts, #319), as `LogLine`'s
    /// own `look`/`inspect` lines; every other read (`observe`, `screen`, `frame`) changes
    /// nothing and logs nothing, the same as `Host::log` itself only tracks those two.
    #[test]
    fn look_and_inspect_are_logged_but_other_reads_are_not() {
        let mut t = tools();
        t.call("look", json!({"x": 0, "y": 0, "w": 4, "h": 4})).unwrap();
        t.call("inspect", json!({"x": 1, "y": 1})).unwrap();
        t.call("observe", json!({})).unwrap();
        t.call("screen", json!({"name": "help"})).unwrap();
        let log_lines: Vec<Value> = String::from_utf8(t.log().get_ref().clone())
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        // The header, then exactly the look and the inspect: observe and screen add nothing.
        assert_eq!(log_lines.len(), 3, "{log_lines:?}");
        assert_eq!(log_lines[1]["type"], "look");
        assert_eq!(log_lines[1]["w"], 4);
        assert_eq!(log_lines[2]["type"], "inspect");
        assert_eq!(log_lines[2]["x"], 1);
    }

    /// `briefing` is in-game knowledge only: the seed, the nation, the turn length and the key
    /// help, nothing from `data/races.toml` (#318, deliberately out of scope here).
    #[test]
    fn briefing_names_the_seed_the_nation_the_turn_length_and_the_key_help() {
        let mut t = tools();
        let result = t.call("briefing", json!({})).unwrap();
        assert_eq!(result["seed"], 7);
        assert_eq!(result["nation"], "earth");
        assert_eq!(result["turn_seconds"], 20);
        assert!(
            result["help"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["what"].as_str().unwrap().starts_with("Move the cursor"))
        );
    }

    /// The key help in `briefing` names the Bend tool and what it's for, so a sandboxed racer
    /// with no access to `docs/` or `data/` can still learn, from in-game knowledge alone, that
    /// bends are something it designates rather than something that just happens (#382, #313).
    #[test]
    fn briefings_key_help_names_the_bend_tool_and_what_its_for() {
        let mut t = tools();
        let result = t.call("briefing", json!({})).unwrap();
        let bend = result["help"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["what"].as_str().unwrap().starts_with("Bend tool"))
            .expect("the key help lists the Bend tool");
        assert_eq!(bend["keys"], "B");
        assert!(
            bend["what"].as_str().unwrap().contains("ability"),
            "says what the tool is for: {bend}"
        );
    }
}

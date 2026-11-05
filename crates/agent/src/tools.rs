//! The MCP tools `fn_agent serve` exposes (docs/design/agent-play.md, "Observation", "Action"):
//! `briefing`, `observe`, `look`, `inspect`, `screen`, `frame`, `act` and `end_turn`, each with a
//! JSON Schema for `tools/list` and a handler that reads or presses keys through a
//! [`fn_tui::surface::Host`] and never anything else.
//!
//! The note on #317 (and #313's "What racers may know"): **this module's tool list is the whole
//! surface a sandboxed racer should have.** A harness that wants an agent limited to the game's
//! own knowledge has to grant only these tools (no shell, no file reads of `docs/` or `data/`,
//! no web) -- that is a setting of whatever runs the agent (an MCP client's tool allow-list, for
//! instance), and nothing in this crate or in a test can enforce it. See the crate's own doc
//! comment for the harness recipe.

use fn_tui::surface::{ActionStep, Host};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::runlog::{LogEvent, Writer};

/// Every tool's name, description and input schema, in the order `tools/list` returns them.
pub fn list() -> Vec<Value> {
    let empty = json!({"type": "object", "properties": {}});
    let coord = json!({
        "type": "object",
        "properties": {"x": {"type": "integer"}, "y": {"type": "integer"}},
        "required": ["x", "y"],
    });
    vec![
        json!({
            "name": "briefing",
            "description": "The nation, seed, turn length and the key help: in-game knowledge \
                             only, no rules text from docs/ or data/.",
            "inputSchema": empty,
        }),
        json!({
            "name": "observe",
            "description": "Turn, clock, season, tool, notice, people, research, stores, \
                             endeavor, the cursor's own tile and alerts since the last turn. \
                             Free: changes nothing and takes no game time.",
            "inputSchema": empty,
        }),
        json!({
            "name": "look",
            "description": "A map region (things, ground and marks, with a legend), at most \
                             64x40.",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer"}, "y": {"type": "integer"},
                    "w": {"type": "integer"}, "h": {"type": "integer"},
                },
                "required": ["x", "y", "w", "h"],
            }),
        }),
        json!({
            "name": "inspect",
            "description": "That tile's Cursor-section lines, as if the cursor sat there.",
            "inputSchema": coord,
        }),
        json!({
            "name": "screen",
            "description": "nation, research, endeavor, log or help, as data.",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "enum": ["nation", "research", "endeavor", "log", "help"],
                    },
                },
                "required": ["name"],
            }),
        }),
        json!({
            "name": "frame",
            "description": "The rendered frame as ASCII text: the parity escape hatch.",
            "inputSchema": json!({
                "type": "object",
                "properties": {"w": {"type": "integer"}, "h": {"type": "integer"}},
            }),
        }),
        json!({
            "name": "act",
            "description": "Runs named steps in order, each lowered to the keys a player would \
                             press: designate {tool: harvest|cancel|stockpile|bend, from, to, \
                             standing?}, build {building, from, to}, select {x,y} or \
                             {name}, research {tech}, cursor {x,y}, keys {script}. A step that \
                             can't be planned (an unknown tech, a building the Build modal \
                             doesn't offer, a tile off the map, ...) presses nothing and the \
                             steps after it don't run.",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "steps": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "step": {
                                    "type": "string",
                                    "enum": [
                                        "designate", "build", "select", "research", "cursor",
                                        "keys",
                                    ],
                                },
                                "tool": {
                                    "type": "string",
                                    "enum": ["harvest", "cancel", "stockpile", "bend"],
                                },
                                "building": {"type": "string"},
                                "from": coord,
                                "to": coord,
                                "standing": {"type": "boolean"},
                                "x": {"type": "integer"},
                                "y": {"type": "integer"},
                                "name": {"type": "string"},
                                "tech": {"type": "string"},
                                "script": {"type": "string"},
                            },
                            "required": ["step"],
                        },
                    },
                    "note": {
                        "type": "string",
                        "description": "A one-line reason, at most 80 characters, shown to \
                                        watchers and logged; never affects the game.",
                    },
                },
                "required": ["steps"],
            }),
        }),
        json!({
            "name": "end_turn",
            "description": "Runs turns of turn_seconds game seconds each (the host owns the \
                             clock); stops early at the end of the first turn with a new alert \
                             or a first-shown need flag.",
            "inputSchema": json!({
                "type": "object",
                "properties": {"turns": {"type": "integer", "default": 1}},
            }),
        }),
    ]
}

/// Deserializes `arguments` into `T`, naming the tool in any error so a bad call says which
/// field it got wrong.
fn args<T: for<'de> Deserialize<'de>>(tool: &str, arguments: Value) -> Result<T, String> {
    serde_json::from_value(arguments).map_err(|e| format!("{tool}: {e}"))
}

fn ok<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("every tool result serializes")
}

#[derive(Deserialize)]
struct LookArgs {
    x: i32,
    y: i32,
    w: u16,
    h: u16,
}

#[derive(Deserialize)]
struct InspectArgs {
    x: i32,
    y: i32,
}

#[derive(Deserialize)]
struct ScreenArgs {
    name: String,
}

#[derive(Deserialize, Default)]
struct FrameArgs {
    w: Option<u16>,
    h: Option<u16>,
}

#[derive(Deserialize)]
struct ActArgs {
    steps: Vec<ActionStep>,
    note: Option<String>,
}

#[derive(Deserialize, Default)]
struct EndTurnArgs {
    turns: Option<u32>,
}

/// Wraps a [`Host`] and the run log [`Writer`]: the one place `tools/call` dispatches to, so
/// every tool call is logged the same way it's served.
pub struct Tools<W: std::io::Write> {
    host: Host,
    log: Writer<W>,
}

impl<W: std::io::Write> Tools<W> {
    pub fn new(host: Host, log: Writer<W>) -> Self {
        Self { host, log }
    }

    /// The run log written so far, for a test to read back (`Writer::get_ref`).
    pub fn log(&self) -> &Writer<W> {
        &self.log
    }

    /// Runs `name` with `arguments` (as `tools/call`'s own params shape), returning the tool's
    /// JSON result, or the reason it couldn't run at all (a bad argument, an unknown tool, or a
    /// log write that failed). A step `act` couldn't plan is not one of these: it's
    /// `Host::act`'s own `error` field in the result, so the steps before it still show as
    /// having run.
    pub fn call(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        match name {
            "briefing" => Ok(self.briefing()),
            "observe" => {
                self.read("observe", &arguments)?;
                Ok(ok(&self.host.observe()))
            }
            "look" => {
                let a: LookArgs = args(name, arguments.clone())?;
                self.read("look", &arguments)?;
                Ok(ok(&self.host.look(a.x, a.y, a.w, a.h)))
            }
            "inspect" => {
                let a: InspectArgs = args(name, arguments.clone())?;
                self.read("inspect", &arguments)?;
                Ok(ok(&self.host.inspect(a.x, a.y)))
            }
            "screen" => {
                let a: ScreenArgs = args(name, arguments.clone())?;
                self.read("screen", &arguments)?;
                self.host.screen(&a.name).map(|v| ok(&v))
            }
            "frame" => {
                let a: FrameArgs = args(name, arguments.clone()).unwrap_or_default();
                self.read("frame", &arguments)?;
                let (w, h) = (
                    a.w.unwrap_or(fn_tui::cli::DEFAULT_SIZE.0),
                    a.h.unwrap_or(fn_tui::cli::DEFAULT_SIZE.1),
                );
                Ok(json!({ "frame": self.host.frame(w, h) }))
            }
            "act" => {
                let a: ActArgs = args(name, arguments)?;
                let note = a.note.map(|n| n.chars().take(80).collect::<String>());
                let result = self.host.act(&a.steps, note.clone());
                self.log
                    .write(&LogEvent::Act {
                        label: result.label.clone(),
                        note,
                        keys: result.keys.clone(),
                        error: result
                            .error
                            .as_ref()
                            .map(|e| format!("step {}: {}", e.step, e.reason)),
                    })
                    .map_err(|e| e.to_string())?;
                Ok(ok(&result))
            }
            "end_turn" => {
                let a: EndTurnArgs = args(name, arguments).unwrap_or_default();
                let turns = a.turns.unwrap_or(1).max(1);
                let before = self.host.observe().turn;
                let obs = self.host.end_turn(turns);
                let ran = obs.turn.saturating_sub(before);
                self.log
                    .write(&LogEvent::Turn { turns: ran, tick: self.host.tick() })
                    .map_err(|e| e.to_string())?;
                Ok(ok(&obs))
            }
            other => Err(format!("no such tool {other:?}; call tools/list")),
        }
    }

    /// Everything `briefing` returns except the goal text, the limit and calls left, which
    /// belong to a race preset (`data/races.toml`, #318, deliberately not built here: the
    /// orchestrator's own note on #313 and the batch this shipped in say build nothing
    /// race-specific yet). A sandboxed racer still has no better way to learn its own seed,
    /// nation or turn length than this tool.
    fn briefing(&self) -> Value {
        let log = self.host.log();
        let help: Vec<Value> = fn_tui::input::help_rows()
            .into_iter()
            .map(|r| json!({"group": r.group.title(), "keys": r.keys, "what": r.what}))
            .collect();
        json!({
            "seed": log.seed,
            "nation": log.nation.as_str(),
            "turn_seconds": self.host.turn_seconds(),
            "help": help,
        })
    }

    fn read(&mut self, tool: &str, args: &Value) -> Result<(), String> {
        self.log
            .write(&LogEvent::Read { tool: tool.to_owned(), args: args.clone() })
            .map_err(|e| e.to_string())
    }

    /// Writes the log's header (`fn_agent serve`'s own first write): what the run was made
    /// against, so a later `fn_agent keys` or a verify (#318) can tell.
    pub fn write_header(&mut self, seed: u64, nation: &str) -> Result<(), String> {
        self.log
            .write(&LogEvent::Header {
                seed,
                nation: nation.to_owned(),
                turn_seconds: self.host.turn_seconds(),
                ticks_per_turn: self.host.ticks_per_turn(),
                data_digest: self.host.data_digest(),
                crate_version: env!("CARGO_PKG_VERSION").to_owned(),
            })
            .map_err(|e| e.to_string())
    }
}

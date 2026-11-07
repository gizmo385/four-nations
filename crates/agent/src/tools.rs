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
//!
//! `run_script` ([`Tools::with_scripts`], `--scripts`) is the one addition
//! docs/design/agent-scripting.md makes: it still only reaches `Host` through
//! [`crate::script::run`], which is itself built from the same `act`/`keys`/`end_turn` this
//! module's other tools call, so granting it adds no capability a sandboxed racer wouldn't
//! already have.

use fn_tui::surface::{ActionStep, Host, LogLine};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::runlog::Writer;
use crate::script::{self, Library, Limits as ScriptLimits};

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
                             64x40. A legend character always means the same thing for the \
                             whole session, even across different regions, so there's no need \
                             to re-check it once learned.",
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
            "description": "nation, research, endeavor, log, help or build, as data. `build` \
                             is the Build modal's own list (name, size, cost, locked or \
                             unlocked), the same text the modal shows, without opening it.",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "enum": ["nation", "research", "endeavor", "log", "help", "build"],
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
                             press: designate {tool: harvest|cancel|clear|stockpile|bend|sow, \
                             from, to, standing?, teardown?, ability?}, build {building, from, \
                             to}, select {x,y} or {name}, research {tech}, cursor {x,y}, keys \
                             {script}. `standing` (harvest only) re-queues the designation when \
                             the resource ripens again. `teardown` (cancel only) also withdraws \
                             a build site or a stockpile tile; without it a cancel withdraws \
                             only job designations (a harvest, a bend, a sow or a clear mark) \
                             and leaves those alone. `ability` (bend only) is the ability's own \
                             name as the Bend tool's own banner shows it (e.g. \"Bore Tunnel\"); \
                             it's lowered to however many presses of the bend key move it there \
                             from whichever ability is active now, so a repeated designate with \
                             the same ability never presses a key at all. A step that can't be \
                             planned (an unknown tech, a building the Build modal doesn't \
                             offer, a tile off the map, standing or teardown on the wrong \
                             tool, an ability that's unknown, locked or not for the bend tool, \
                             ...) presses nothing and the steps after it don't run. The \
                             result's `notices` carries each designate or build step's own \
                             'N done, M skipped: reason' text (cancel's own breaks the done \
                             count out by kind instead, and a sow names a plot that's already \
                             sown or marked instead of the misleading 'not a farm plot'), so a \
                             mostly-skipped rectangle (an impassable tile, say) is obvious \
                             right away instead of only in a later `observe`.",
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
                                    "enum": [
                                        "harvest", "cancel", "clear", "stockpile", "bend", "sow",
                                    ],
                                },
                                "building": {"type": "string"},
                                "from": coord,
                                "to": coord,
                                "standing": {"type": "boolean"},
                                "teardown": {"type": "boolean"},
                                "ability": {"type": "string"},
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
                             clock); stops early at the end of the first turn with an alert \
                             worth stopping for -- a death, an arrival, an endeavor event \
                             (including a settler becoming a bending master), a finished tech, \
                             a season event, or a bend that failed; a bend that merely finished \
                             is routine and never stops it on its own -- or a need flag that \
                             wasn't already up when this call started and hasn't shown yet in \
                             this same call (a flag that flickers on and off only counts once). \
                             The result's `stopped` names why it stopped early, or is null when \
                             every requested turn ran. Every alert is still in the result's own \
                             `alerts` list either way.",
            "inputSchema": json!({
                "type": "object",
                "properties": {"turns": {"type": "integer", "default": 1}},
            }),
        }),
    ]
}

/// `run_script`'s own entry, added to [`Tools::list`] only once [`Tools::with_scripts`] has
/// turned it on (`fn_agent serve --scripts`, docs/design/agent-scripting.md's own "`run_script`"):
/// not in [`list`] itself, since a sandboxed racer with no `--scripts` flag should never even
/// see it offered.
fn run_script_tool() -> Value {
    json!({
        "name": "run_script",
        "description": "Runs code (embedded Rhai: no import, no clock, no RNG) against the \
                         same Host this server's other tools read and act through -- observe, \
                         look, inspect, screen, act(steps, note), keys(script, note), \
                         wait(turns) (at least 1, Host::end_turn(1) per turn), events() \
                         (#{turn, kind, text} since this run started) and wake(msg). Give \
                         either `name` (a `save_script`d script's own latest version) or \
                         `code` (an ad hoc, unsaved one), not both or neither. `memory` is a \
                         map that persists across run_script calls in this session (#{...}, \
                         read and written as a normal variable); a write over data/agent.toml's \
                         own memory_bytes fails the whole run with a clear error and keeps the \
                         memory from before. Stops and returns once the script returns \
                         (`done`, with its own value), errors (`error`, with message and \
                         line:col), calls `wake(msg)` (`asked`), a `wake_on` event lands \
                         during a `wait` (`event`), uses up `turns` (`budget`), or runs too \
                         many operations with no `wait` (`runaway`). The result is the wake \
                         report: reason, why, script name and version (if run by name), the \
                         turns actually run, the clock span, a since_start diff (people, store \
                         deltas, event counts, deduplicated texts), act counts with the last \
                         failure, the script's own log tail, the resulting memory, and the \
                         final `observe`.",
        "inputSchema": json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Runs this saved script's own latest version instead of \
                                    inline code; give name or code, not both.",
                },
                "code": {"type": "string"},
                "turns": {"type": "integer"},
                "note": {
                    "type": "string",
                    "description": "An ad hoc (`code`) run's own tag for every act/keys step \
                                    this run makes in the log, e.g. \"[keep-fed]\"; defaults \
                                    to \"script\" with none given. Ignored for a `name` run, \
                                    which is always tagged by its own name and version.",
                },
                "wake_on": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["death", "arrival", "tech", "endeavor", "season", "stockpile",
                                  "bend_failed", "need"],
                    },
                    "description": "Which events a `wait` stops the whole script for \
                                    (`\"event\"`); defaults to data/agent.toml's own \
                                    `default_wake` (death, need, endeavor). Every other kind \
                                    still shows in `events()` for the script to handle itself.",
                },
            },
        }),
    })
}

/// `save_script`'s own entry (#479, docs/design/agent-scripting.md's own "`save_script`"):
/// stores `code` under `name` in this session's own library, bumping its version.
fn save_script_tool() -> Value {
    json!({
        "name": "save_script",
        "description": "Stores code under name in this session's own script library, bumping \
                         its version (1 for a new name, one more otherwise, even if code is \
                         unchanged). run_script {name} then runs this, and scripts {} lists \
                         it. Logged right away (#480), so a script that's saved but never run \
                         in this session is still restored by --resume.",
        "inputSchema": json!({
            "type": "object",
            "properties": {"name": {"type": "string"}, "code": {"type": "string"}},
            "required": ["name", "code"],
        }),
    })
}

/// `sdk_reference`'s own entry (#480, docs/design/agent-scripting.md's own "The SDK"): the
/// plumbing's own signatures and the porcelain layer's own `///` doc comments, both read back
/// from the registered functions and the compiled `prelude.rhai` rather than kept by hand, so
/// this can't say anything the code doesn't actually do.
fn sdk_reference_tool() -> Value {
    json!({
        "name": "sdk_reference",
        "description": "The run_script SDK, in two layers: plumbing (observe, look, cells, \
                         inspect, screen, act, keys, wait, events, wake, log -- the same calls \
                         every run_script uses) and porcelain (prelude.rhai's own find, \
                         designate_near, build_near, wait_until, stalled, store, idle and need, \
                         loaded before every script). Each plumbing entry is #{signature, doc}; \
                         each porcelain entry is #{name, params, doc}, doc straight from the \
                         function's own /// comment.",
        "inputSchema": json!({"type": "object", "properties": {}}),
    })
}

/// `scripts`'s own entry (#479): every saved script this session knows, with its version and
/// its last `run_script` call's own outcome and turns run.
fn scripts_tool() -> Value {
    json!({
        "name": "scripts",
        "description": "Every script save_script has stored this session: name, version, and \
                         its last run_script call's own outcome (null, reason \"done\"/\
                         \"error\"/... until it's been run at least once) and turns run.",
        "inputSchema": json!({"type": "object", "properties": {}}),
    })
}

/// `notes`'s own entry (#479): the agent's own lab notes, read or appended one line at a time.
fn notes_tool() -> Value {
    json!({
        "name": "notes",
        "description": "Reads this session's own lab notes (one line per past finding -- a \
                         script's own root cause, a fix, an SDK gap), or appends one more with \
                         append. Always returns the full list, oldest first.",
        "inputSchema": json!({
            "type": "object",
            "properties": {
                "append": {"type": "string", "description": "One more line to add."},
            },
        }),
    })
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

#[derive(Deserialize, Default)]
struct RunScriptArgs {
    name: Option<String>,
    code: Option<String>,
    turns: Option<u32>,
    note: Option<String>,
    wake_on: Option<Vec<fn_tui::surface::EventKind>>,
}

#[derive(Deserialize)]
struct SaveScriptArgs {
    name: String,
    code: String,
}

#[derive(Deserialize, Default)]
struct NotesArgs {
    append: Option<String>,
}

/// Wraps a [`Host`] and the run log [`Writer`]: the one place `tools/call` dispatches to.
///
/// Logging doesn't duplicate [`Host`]'s own bookkeeping: `Host` already keeps a `RunLog` of
/// every `Keys`, `Look`, `Inspect` and `Turn` entry a call makes (`press_keys`, `act`, `look`,
/// `inspect`, `end_turn`), in the one schema `fn_tui::surface::LogLine` defines (#324). After
/// every call, [`Self::flush_new_lines`] writes whatever of `host.log().lines()` wasn't on disk
/// yet, so the file is exactly what a live `fn_tui --watch` or a later `fn_agent keys` reads
/// back -- the same lines [`Host`]'s own tests already check.
pub struct Tools<W: std::io::Write> {
    host: Host,
    log: Writer<W>,
    /// How many of `host.log().lines()` are already flushed to `log`.
    written: usize,
    /// `Some` once [`Self::with_scripts`] has turned `run_script` on (`fn_agent serve
    /// --scripts`); `None` keeps this session's tool list exactly what it was before #477, the
    /// sandboxed racer's whole surface (the crate's own doc comment).
    scripts: Option<ScriptLimits>,
    /// `save_script`'s own library, `run_script {name}`'s own source (#479): empty for a fresh
    /// session, rebuilt from the log by [`Self::resume`] otherwise.
    library: Library,
    /// `run_script`'s own `memory`, persisted across calls in this session (#479): always a
    /// JSON object, `{}` for a fresh session.
    memory: Value,
    /// `notes`'s own lab notes, oldest first (#479): empty for a fresh session.
    notes: Vec<String>,
}

impl<W: std::io::Write> Tools<W> {
    /// Wraps `host` and `log`, writing the header at once (`Host::header`): what the run was
    /// made against, so a later `fn_tui --watch` or `fn_agent keys` can tell.
    pub fn new(host: Host, mut log: Writer<W>) -> Result<Self, String> {
        log.write_header(&host.header()).map_err(|e| e.to_string())?;
        Ok(Self {
            host,
            log,
            written: 0,
            scripts: None,
            library: Library::new(),
            memory: json!({}),
            notes: Vec::new(),
        })
    }

    /// Wraps a [`Host`] built with [`fn_tui::surface::Host::resume`] and a `log` that's already
    /// had its header and every replayed line written (`fn_agent serve --resume`, #341):
    /// `written` starts at `host.log().lines().len()`, not 0, so [`Self::call`]'s first flush
    /// appends only what's new instead of writing the replayed lines a second time. `lines` are
    /// that same log's own lines (`fn_tui::watch::read_file`, minus any `End`), replayed by
    /// [`rebuild_session`] into this session's own script library, `memory` and lab notes
    /// (#479, docs/design/agent-scripting.md, "Memory across sessions") -- the same state a
    /// live session reached, not a fresh one.
    pub fn resume(host: Host, log: Writer<W>, lines: &[LogLine]) -> Self {
        let written = host.log().lines().len();
        let (library, memory, notes) = rebuild_session(lines);
        Self { host, log, written, scripts: None, library, memory, notes }
    }

    /// Turns `run_script` on for this session (`fn_agent serve --scripts`), with `limits` as
    /// `data/agent.toml`'s `[scripts]` table.
    pub fn with_scripts(mut self, limits: ScriptLimits) -> Self {
        self.scripts = Some(limits);
        self
    }

    /// This session's own tool list: [`list`], plus `run_script`, `save_script`, `scripts`,
    /// `notes` and `sdk_reference` once [`Self::with_scripts`] has turned them on.
    pub fn list(&self) -> Vec<Value> {
        let mut tools = list();
        if self.scripts.is_some() {
            tools.push(run_script_tool());
            tools.push(save_script_tool());
            tools.push(scripts_tool());
            tools.push(notes_tool());
            tools.push(sdk_reference_tool());
        }
        tools
    }

    /// The run log written so far, for a test to read back (`Writer::get_ref`).
    pub fn log(&self) -> &Writer<W> {
        &self.log
    }

    /// Writes the log's `end` line (#327): the world's own tick right now, and `reason` (stdin
    /// closed, or an error reading it). The last thing `fn_agent serve` writes before it exits,
    /// so `fn_tui --watch` can tell a finished run from one that's merely caught up with a live
    /// agent still thinking.
    pub fn end(&mut self, reason: &str) -> Result<(), String> {
        self.log.write_end(self.host.tick(), reason).map_err(|e| e.to_string())
    }

    /// Runs `name` with `arguments` (as `tools/call`'s own params shape), returning the tool's
    /// JSON result, or the reason it couldn't run at all (a bad argument, an unknown tool, or a
    /// log write that failed). A step `act` couldn't plan is not one of these: it's
    /// `Host::act`'s own `error` field in the result, so the steps before it still show as
    /// having run.
    pub fn call(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        let result = self.dispatch(name, arguments)?;
        self.flush_new_lines()?;
        Ok(result)
    }

    fn dispatch(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        match name {
            "briefing" => Ok(self.briefing()),
            "observe" => Ok(ok(&self.host.observe())),
            "look" => {
                let a: LookArgs = args(name, arguments)?;
                Ok(ok(&self.host.look(a.x, a.y, a.w, a.h)))
            }
            "inspect" => {
                let a: InspectArgs = args(name, arguments)?;
                Ok(ok(&self.host.inspect(a.x, a.y)))
            }
            "screen" => {
                let a: ScreenArgs = args(name, arguments)?;
                self.host.screen(&a.name).map(|v| ok(&v))
            }
            "frame" => {
                let a: FrameArgs = args(name, arguments).unwrap_or_default();
                let (w, h) = (
                    a.w.unwrap_or(fn_tui::cli::DEFAULT_SIZE.0),
                    a.h.unwrap_or(fn_tui::cli::DEFAULT_SIZE.1),
                );
                Ok(json!({ "frame": self.host.frame(w, h) }))
            }
            "act" => {
                let a: ActArgs = args(name, arguments)?;
                Ok(ok(&self.host.act(&a.steps, a.note)))
            }
            "end_turn" => {
                let a: EndTurnArgs = args(name, arguments).unwrap_or_default();
                let turns = a.turns.unwrap_or(1).max(1);
                Ok(ok(&self.host.end_turn(turns)))
            }
            "run_script" => self.run_script(arguments),
            "save_script" => {
                self.require_scripts(name)?;
                let a: SaveScriptArgs = args(name, arguments)?;
                let version = self.library.save(&a.name, &a.code);
                // Logged right away (#480, this module's own doc comment on `save_script_tool`
                // and `rebuild_session`): `turns: 0` is this line's own "never run" marker --
                // `resolve_turns` always clamps a real `run_script` call's own `turns` to at
                // least 1, so a save-only line can never be mistaken for one.
                self.log
                    .write_line(&LogLine::Script {
                        name: a.name.clone(),
                        version,
                        code: a.code.clone(),
                        turns: 0,
                        wake_on: Vec::new(),
                    })
                    .map_err(|e| e.to_string())?;
                Ok(json!({"name": a.name, "version": version}))
            }
            "scripts" => {
                self.require_scripts(name)?;
                Ok(self.library.list())
            }
            "sdk_reference" => {
                self.require_scripts(name)?;
                let plumbing = script::bindings::plumbing_reference(&mut self.host);
                let porcelain = script::prelude::reference();
                Ok(json!({"plumbing": plumbing, "porcelain": porcelain}))
            }
            "notes" => {
                self.require_scripts(name)?;
                let a: NotesArgs = args(name, arguments).unwrap_or_default();
                if let Some(line) = a.append.filter(|l| !l.is_empty()) {
                    self.log
                        .write_line(&LogLine::Note { text: line.clone() })
                        .map_err(|e| e.to_string())?;
                    self.notes.push(line);
                }
                Ok(json!({"notes": self.notes}))
            }
            other => Err(format!("no such tool {other:?}; call tools/list")),
        }
    }

    /// `limits` once [`Self::with_scripts`] has turned the script tools on for this session, or
    /// an ordinary tool error naming `tool` otherwise -- the guard every one of `run_script`,
    /// `save_script`, `scripts` and `notes` shares.
    fn require_scripts(&self, tool: &str) -> Result<&ScriptLimits, String> {
        self.scripts
            .as_ref()
            .ok_or_else(|| format!("{tool}: this session wasn't started with --scripts"))
    }

    /// `run_script`'s own dispatch (#477, #479): resolves `arguments` into either a saved
    /// script's own name and code (`run_script {name}`, from [`Self::library`]) or an ad hoc
    /// one (`run_script {code}`, `version` 0), writes that run's own `Script` log line, runs it
    /// (`script::run`), then writes the `Wake` line and updates [`Self::memory`] and (for a
    /// named run) [`Self::library`]'s own last outcome -- in that order, so a watcher or a
    /// resumed session reads `Script`, then the run's own `Keys`/`Turn` lines, then `Wake`,
    /// exactly as they happened (docs/design/agent-scripting.md, "Determinism, replay and
    /// watching").
    fn run_script(&mut self, arguments: Value) -> Result<Value, String> {
        let limits = self.require_scripts("run_script")?.clone();
        let a: RunScriptArgs = args("run_script", arguments)?;
        let (tag, code, version) = match (a.name, a.code) {
            (Some(n), None) => {
                let (code, version) = self
                    .library
                    .get(&n)
                    .map(|(c, v)| (c.to_owned(), v))
                    .ok_or_else(|| format!("run_script: no such saved script {n:?}"))?;
                (n, code, version)
            }
            (None, Some(code)) => {
                let tag = a.note.filter(|n| !n.is_empty()).unwrap_or_else(|| "script".to_owned());
                (tag, code, 0)
            }
            _ => {
                return Err(
                    "run_script needs either name or code, but not both or neither".to_owned()
                );
            }
        };
        let wake_on = a.wake_on.unwrap_or_else(|| limits.default_wake.clone());
        let turns = limits.resolve_turns(a.turns);

        self.log
            .write_line(&LogLine::Script {
                name: tag.clone(),
                version,
                code: code.clone(),
                turns,
                wake_on: wake_on.clone(),
            })
            .map_err(|e| e.to_string())?;

        let request = script::Request {
            code: &code,
            turns: Some(turns),
            note: Some(&tag),
            version,
            wake_on: &wake_on,
            memory: &self.memory,
        };
        let outcome = script::run(&mut self.host, request, &limits);

        // The script's own `Keys`/`Turn` lines land between `Script` and `Wake`, not after:
        // flushing here (rather than waiting for `Self::call`'s own flush) keeps the log in the
        // order the run actually happened.
        self.flush_new_lines()?;

        self.memory = outcome.memory.clone();
        if version > 0 {
            self.library.record_run(&tag, outcome.reason, outcome.turns_run);
        }

        self.log
            .write_line(&LogLine::Wake {
                reason: outcome.reason.to_owned(),
                why: outcome.why.clone(),
                turn: outcome.observe.turn,
                memory: outcome.memory.clone(),
            })
            .map_err(|e| e.to_string())?;

        Ok(ok(&outcome))
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

    /// Writes whatever of `host.log().lines()` isn't on disk yet (`Self::written`'s own count),
    /// flushing each as it goes (`Writer::write_line`).
    fn flush_new_lines(&mut self) -> Result<(), String> {
        let lines = self.host.log().lines();
        for line in &lines[self.written..] {
            self.log.write_line(line).map_err(|e| e.to_string())?;
        }
        self.written = lines.len();
        Ok(())
    }
}

/// Rebuilds the script library, `memory` and lab notes a resumed session should start from, by
/// replaying `lines`' own `Script`, `Turn`, `Wake` and `Note` lines in order (#479, #480,
/// `Tools::resume`, docs/design/agent-scripting.md, "Memory across sessions"):
/// - Any `Script` line with `version >= 1` restores its own `name` and `code` into the library
///   (`Library::restore`) -- whether it came from `save_script` (`turns == 0`, this dispatch's
///   own "never run" marker, `Self::dispatch`'s own `"save_script"` arm) or from `run_script
///   {name}` (`turns >= 1`: `ScriptLimits::resolve_turns` always clamps to at least 1, so the
///   two can never be confused). Only the latter also opens a pending run to close with the
///   next `Wake` line -- a save-only line never gets one, since no `Wake` line ever follows it.
///   `version == 0` is an ad hoc `run_script {code}` call (never `save_script`d, so nothing to
///   restore into the library either) and opens no pending run.
/// - Every `Turn` line while a run is pending counts toward that run's own turns run -- exactly
///   what `fn_agent::script::Outcome::turns_run` counted live, since every turn a script runs
///   is logged the same way (`fn_tui::surface::LogLine::Turn`).
/// - The `Wake` line that follows closes whatever run is pending (`Library::record_run` with
///   its own `reason` and the turns counted since the `Script` line), and its own `memory` is
///   kept as the session's current one regardless -- `memory` is global, not per-script, so
///   only the very last `Wake` line's value matters.
/// - Every `Note` line's own `text`, oldest first.
fn rebuild_session(lines: &[LogLine]) -> (Library, Value, Vec<String>) {
    let mut library = Library::new();
    let mut memory = json!({});
    let mut notes = Vec::new();
    let mut pending: Option<String> = None;
    let mut turns_run: u32 = 0;
    for line in lines {
        match line {
            LogLine::Script { name, version, code, turns, .. } => {
                if *version >= 1 {
                    library.restore(name, *version, code);
                }
                pending = if *version >= 1 && *turns >= 1 { Some(name.clone()) } else { None };
                turns_run = 0;
            }
            LogLine::Turn { .. } => turns_run += 1,
            LogLine::Wake { reason, memory: m, .. } => {
                if let Some(name) = pending.take() {
                    library.record_run(&name, reason, turns_run);
                }
                memory = m.clone();
                turns_run = 0;
            }
            LogLine::Note { text } => notes.push(text.clone()),
            LogLine::Header(_)
            | LogLine::Keys { .. }
            | LogLine::Look { .. }
            | LogLine::Inspect { .. }
            | LogLine::End { .. } => {}
        }
    }
    (library, memory, notes)
}

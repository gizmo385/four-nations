//! The plumbing layer (docs/design/agent-scripting.md, "The SDK", "Plumbing: 1:1 with `Host`"):
//! [`register`] adds `observe`, `look`, `cells`, `inspect`, `screen`, `can_build`, `act`, `keys`,
//! `wait`, `events`, `wake` and `log` to an [`Engine`], each exactly the [`Host`] call its name
//! says (`events` and `wake` are from #478, `cells` from #480, `can_build` from #509), with its
//! result serialized to a Rhai map through `rhai::serde` -- nothing a plumbing call does reaches
//! anything but `Host`'s own public methods, so a script can't do anything a human's keyboard or
//! an MCP agent couldn't.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fn_tui::surface::{ActResult, ActionStep, Event, EventKind, Host, PressResult};
use rhai::{Dynamic, Engine, EvalAltResult, Position};
use serde::{Deserialize, Serialize};

/// What [`register`]'s `wait` or `wake` raises through `EvalAltResult::ErrorTerminated` to stop
/// a script run early, read back by `crate::script::runner::classify_error` -- never a plain
/// [`Err`] string, because [`rhai::EvalAltResult::is_catchable`] is `false` for `ErrorTerminated`
/// (docs/design/agent-scripting.md, "`run_script`": "The wake can't be caught by the script's
/// own `try`/`catch`"), unlike every ordinary error a script's own code can raise.
#[derive(Serialize, Deserialize)]
pub(super) enum Signal {
    /// The script's own turn budget (`run_script`'s `turns`) ran out mid-`wait`.
    Budget,
    /// The script called `wake(msg)` directly.
    Wake { msg: String },
    /// A `wake_on` event landed during a `wait`.
    Event { kind: EventKind, text: String, turn: u32 },
    /// The script did more work than the budget allows since its last `wait` (or its start):
    /// `spent` units against the `limit` (#564, [`Context::check_work`]).
    Runaway { spent: u64, limit: u64 },
}

fn terminate(signal: Signal) -> Box<EvalAltResult> {
    let dynamic = rhai::serde::to_dynamic(signal).expect("Signal always serializes to a Dynamic");
    Box::new(EvalAltResult::ErrorTerminated(dynamic, Position::NONE))
}

/// `token` read back as a [`Signal`], or `None` for any other `Dynamic` an
/// `EvalAltResult::ErrorTerminated` might someday carry (there is no other source today).
pub(super) fn read_signal(token: &Dynamic) -> Option<Signal> {
    rhai::serde::from_dynamic(token).ok()
}

/// Shared, interior-mutable state every registered function closes over: one built fresh for
/// each `run_script` call ([`crate::script::runner::run`]) and dropped with the `Engine` that
/// used it.
///
/// `host` is a raw pointer rather than a borrow because `Engine::register_fn` requires its
/// closures to be `'static` (Rhai's own `SendSync` bound), which rules out capturing a `&mut
/// Host` borrowed from the caller's stack frame directly. This is sound here because:
/// - every closure that holds a clone of this `Rc` lives inside the one `Engine`
///   [`crate::script::runner::run`] builds, uses and drops, all within the lifetime of the
///   `&mut Host` borrow this pointer was built from;
/// - that `Engine` (and every clone of this `Rc` it holds) is dropped before `run` touches
///   `host` again, so the pointer and the borrow are never live at once;
/// - `fn_agent serve` is single-threaded and handles one `tools/call` at a time, so no two
///   `run_script` calls -- and so no two `Context`s -- ever exist concurrently.
pub(super) struct Context {
    host: *mut Host,
    /// This run's own plain name (the caller's `note`, or `"script"` with none given --
    /// `run_script {name}`'s own saved name once #479 is in play, or `run_script {code}`'s own
    /// `note`): [`Self::tag`] and `Outcome::script`'s own value, never version-suffixed.
    tag: String,
    /// The saved script's own version being run (`run_script {name}`, #479), or `0` for an ad
    /// hoc `run_script {code}` call that was never `save_script`d. [`Self::log_tag`] suffixes
    /// `self.tag` with `" vN"` whenever this is nonzero (docs/design/agent-scripting.md,
    /// "Determinism, replay and watching": "its note is `[keep-fed v3]`").
    version: u32,
    log: RefCell<Vec<String>>,
    max_turns: u32,
    turns_used: Cell<u32>,
    /// `Host::event_mark()` at the moment this run began, so `events()` can always answer
    /// "since the script started" (#478).
    events_start: usize,
    /// Which [`EventKind`]s a `wait` stops the whole script for (`run_script`'s own `wake_on`).
    wake_on: Vec<EventKind>,
    acts_ok: Cell<u32>,
    acts_failed: Cell<u32>,
    last_failure: RefCell<Option<String>>,
    /// Every failed `act`/`keys` of this run with the turn it was made on (turns into the run),
    /// up to [`FAILURE_CAP`]: what `test_governor`'s trace counts by message and time (#570).
    failures: RefCell<Vec<(u32, String)>>,
    /// The session's `plan` map as of this run's start (#524): `plan()` returns a fresh copy of
    /// it every call, so a script can't change it (only the `plan` tool does, between runs).
    plan: serde_json::Value,
    /// Whether this run is a probe (#527, decision #522): an inline `run_script {code}`, which
    /// looks but doesn't act. `act`, `keys` and `wait` then fail with [`PROBE_REFUSAL`] before
    /// they touch the host, so no key is pressed and no turn passes; every porcelain helper that
    /// acts or waits does so through one of those three and so fails the same way.
    probe: bool,
    /// The most operations (plus native-call [`cost`]s) the script may spend between two `wait`s
    /// (#564): `Limits::max_operations`. `u64::MAX` until [`Self::set_work_limit`].
    work_limit: Cell<u64>,
    /// Rhai's own operation count at the last `Engine::on_progress` call, which is cumulative
    /// over the whole run: [`Self::check_work`] writes it, [`Self::reset_work`] reads it.
    ops_seen: Cell<u64>,
    /// `ops_seen` as of the last `wait` (or `0` at the start): what the budget counts from.
    ops_base: Cell<u64>,
    /// The cost of the native calls (`cells`, `act`, ...) made since the last `wait`, in
    /// operations (the [`cost`] module): Rhai counts a call as one operation however much work
    /// the host does for it.
    native_cost: Cell<u64>,
}

/// How many failed acts one run keeps for the trace (#570); the tally in `acts.failed` goes on
/// counting past it. A governor refused 20 times a turn for 540 turns is about half of this.
const FAILURE_CAP: usize = 20_000;

/// What a native call charges the operation budget (#564), in the budget's own unit: one Rhai
/// operation, taken as about a microsecond of a script doing real work. Rhai counts a call to
/// `cells` or `act` as one operation however long the host takes, so a loop of them would run
/// for minutes inside the limit. These are flat, or a function of the arguments, never of a
/// clock, so the budget is the same on every machine.
///
/// Measured on the reference governor (`crates/agent/tests/reference_governor.rs`) in a release
/// build over a 540-turn game: `observe` and `act` took 3.5 to 4.5 ms each once the colony had
/// grown (60 us when it had died out), `cells` took 0.7 ms for its biggest, 39x39 window and
/// about 0.5 us a cell, `can_build` 10 to 15 us, `screen` 10 to 30 us. `observe`, `act` and `keys`
/// are charged half of the late-game figure, so a pass of the reference governor (up to 40 acts
/// and 9 observes) uses about a sixth of the limit and a loop of nothing but `observe` runs about
/// 500 times before it stops.
mod cost {
    /// `observe` (and `act` and `keys`, which return a fresh observation's worth of work).
    pub(super) const HEAVY: u64 = 2000;
    /// `can_build`, `inspect`, `screen`, `events` and `plan`.
    pub(super) const LIGHT: u64 = 40;

    /// `cells` and `look`: a base and half an operation a cell of the window the host really
    /// returns (it caps one at `MAX_LOOK`).
    pub(super) fn window(w: u16, h: u16) -> u64 {
        let (max_w, max_h) = fn_tui::surface::MAX_LOOK;
        20 + u64::from(w.min(max_w)) * u64::from(h.min(max_h)) / 2
    }
}

/// The error a probe's `act`, `keys` or `wait` fails with (docs/design/agent-hillclimb.md,
/// "Probes look, they don't act").
pub(crate) const PROBE_REFUSAL: &str = "probes can't act: put this in the governor, or in the plan";

/// What `sdk_reference` and `briefing` tell the model about probes (inline `run_script {code}`).
pub(crate) const PROBE_NOTE: &str = "Inline run_script code is a read-only probe. It may call \
    observe, look, inspect, cells, can_build, screen, events, plan, log, find, store, idle and \
    need, and read memory. act, keys, wait, designate_near, build_near and wait_until fail with \
    \"probes can't act: put this in the governor, or in the plan\": no key is pressed and no \
    turn passes. Put every action in a script you save_script and run by name.";

impl Context {
    pub(super) fn new(
        host: &mut Host,
        tag: String,
        version: u32,
        max_turns: u32,
        wake_on: Vec<EventKind>,
        plan: serde_json::Value,
        probe: bool,
    ) -> Rc<Self> {
        let events_start = host.event_mark();
        Rc::new(Self {
            host: std::ptr::from_mut(host),
            tag,
            version,
            log: RefCell::new(Vec::new()),
            max_turns,
            turns_used: Cell::new(0),
            events_start,
            wake_on,
            acts_ok: Cell::new(0),
            acts_failed: Cell::new(0),
            last_failure: RefCell::new(None),
            failures: RefCell::new(Vec::new()),
            plan,
            probe,
            work_limit: Cell::new(u64::MAX),
            ops_seen: Cell::new(0),
            ops_base: Cell::new(0),
            native_cost: Cell::new(0),
        })
    }

    /// Sets the budget between two `wait`s (`Limits::max_operations`, #564).
    pub(super) fn set_work_limit(&self, limit: u64) {
        self.work_limit.set(limit);
    }

    /// `Engine::on_progress`'s callback (#564): `ops` is Rhai's cumulative count. The budget is
    /// what the script spent since its last `wait`: the operations Rhai counted since then plus
    /// the cost of the native calls. Past the limit, ends the script with [`Signal::Runaway`]
    /// (Rhai puts the token in an `ErrorTerminated` at the statement it was in).
    pub(super) fn check_work(&self, ops: u64) -> Option<Dynamic> {
        self.ops_seen.set(ops);
        let spent = self.work_spent();
        let limit = self.work_limit.get();
        (spent > limit).then(|| {
            rhai::serde::to_dynamic(Signal::Runaway { spent, limit })
                .expect("Signal always serializes to a Dynamic")
        })
    }

    /// The operations and native-call costs since the last `wait`.
    fn work_spent(&self) -> u64 {
        (self.ops_seen.get() - self.ops_base.get()) + self.native_cost.get()
    }

    /// Charges a native call's [`cost`] to the budget; [`Self::check_work`] sees it at the
    /// script's next operation, which a loop of native calls reaches at once.
    fn charge(&self, cost: u64) {
        self.native_cost.set(self.native_cost.get().saturating_add(cost));
    }

    /// A `wait` returned to the script: the budget counts from here (#564).
    fn reset_work(&self) {
        self.ops_base.set(self.ops_seen.get());
        self.native_cost.set(0);
    }

    /// # Safety
    /// See this struct's own doc comment: sound exactly because every caller of this method is
    /// a closure [`register`] built, alive only for the `Engine`'s one call. Clippy's own
    /// `mut_from_ref` lint flags this shape on principle (an aliased `&self` handing out `&mut
    /// Host`), which is exactly the invariant that comment explains.
    #[allow(clippy::mut_from_ref)]
    fn host(&self) -> &mut Host {
        unsafe { &mut *self.host }
    }

    pub(super) fn push_log(&self, line: impl Into<String>) {
        self.log.borrow_mut().push(line.into());
    }

    pub(super) fn log_lines(&self) -> Vec<String> {
        self.log.borrow().clone()
    }

    pub(super) fn turns_used(&self) -> u32 {
        self.turns_used.get()
    }

    /// `Err(PROBE_REFUSAL)` when this run is a probe, checked first thing by every call that acts
    /// or waits.
    fn refuse_if_probe(&self) -> Result<(), Box<EvalAltResult>> {
        if self.probe { Err(err_to_rhai(PROBE_REFUSAL.to_owned())) } else { Ok(()) }
    }

    fn bump_turns(&self) {
        self.turns_used.set(self.turns_used.get() + 1);
    }

    /// [`Self::tag`] suffixed `" vN"` when [`Self::version`] is nonzero: a saved script's own
    /// run is tagged by name and version, an ad hoc one by name alone.
    fn log_tag(&self) -> String {
        match self.version {
            0 => self.tag.clone(),
            v => format!("{} v{v}", self.tag),
        }
    }

    /// `note` tagged with [`Self::log_tag`] (docs/design/agent-scripting.md, "Determinism,
    /// replay and watching": "Its label is the step and its note is `[keep-fed v3]` plus the
    /// script's own note").
    fn tag_note(&self, note: Option<&str>) -> String {
        let tag = self.log_tag();
        match note {
            Some(n) if !n.is_empty() => format!("[{tag}] {n}"),
            _ => format!("[{tag}]"),
        }
    }

    /// `self.tag`, for [`crate::script::runner::run`]'s own wake report (`Outcome::script`):
    /// always the plain name, never version-suffixed (`Outcome::version` carries that).
    pub(super) fn tag(&self) -> &str {
        &self.tag
    }

    /// `self.version`, for [`crate::script::runner::run`]'s own wake report (`Outcome::version`).
    pub(super) fn version(&self) -> u32 {
        self.version
    }

    /// Every event since [`Self::events_start`], for the `events()` binding and
    /// [`crate::script::runner::run`]'s own `since_start` diff.
    pub(super) fn events_since_start(&self) -> Vec<Event> {
        self.host().events_since(self.events_start)
    }

    pub(super) fn acts_ok(&self) -> u32 {
        self.acts_ok.get()
    }

    pub(super) fn acts_failed(&self) -> u32 {
        self.acts_failed.get()
    }

    pub(super) fn last_failure(&self) -> Option<String> {
        self.last_failure.borrow().clone()
    }

    /// Every failure [`Self::record`] kept, oldest first, as (turns into the run, message).
    pub(super) fn failures(&self) -> Vec<(u32, String)> {
        self.failures.borrow().clone()
    }

    /// Tallies one `act`/`keys` step toward the eventual wake report's own `acts` counts:
    /// `Some(msg)` counts as a failure and becomes the new `last_failure`; `None` counts as ok.
    fn record(&self, failure: Option<String>) {
        match failure {
            Some(msg) => {
                self.acts_failed.set(self.acts_failed.get() + 1);
                let mut failures = self.failures.borrow_mut();
                if failures.len() < FAILURE_CAP {
                    failures.push((self.turns_used.get(), msg.clone()));
                }
                drop(failures);
                *self.last_failure.borrow_mut() = Some(msg);
            }
            None => self.acts_ok.set(self.acts_ok.get() + 1),
        }
    }
}

/// Registers every plumbing function on `engine`, each closing over a clone of `ctx`.
pub(super) fn register(engine: &mut Engine, ctx: &Rc<Context>) {
    let c = ctx.clone();
    engine.register_fn("observe", move || -> Result<Dynamic, Box<EvalAltResult>> {
        c.charge(cost::HEAVY);
        rhai::serde::to_dynamic(c.host().observe())
    });

    let c = ctx.clone();
    engine.register_fn(
        "look",
        move |x: i64, y: i64, w: i64, h: i64| -> Result<Dynamic, Box<EvalAltResult>> {
            c.charge(cost::window(clamp_u16(w), clamp_u16(h)));
            let view = c.host().look(x as i32, y as i32, clamp_u16(w), clamp_u16(h));
            rhai::serde::to_dynamic(view)
        },
    );

    let c = ctx.clone();
    engine.register_fn("inspect", move |x: i64, y: i64| -> Result<Dynamic, Box<EvalAltResult>> {
        c.charge(cost::LIGHT);
        rhai::serde::to_dynamic(c.host().inspect(x as i32, y as i32))
    });

    let c = ctx.clone();
    engine.register_fn(
        "cells",
        move |x: i64, y: i64, w: i64, h: i64| -> Result<Dynamic, Box<EvalAltResult>> {
            c.charge(cost::window(clamp_u16(w), clamp_u16(h)));
            let view = c.host().cells(x as i32, y as i32, clamp_u16(w), clamp_u16(h));
            rhai::serde::to_dynamic(view)
        },
    );

    let c = ctx.clone();
    engine.register_fn("screen", move |name: &str| -> Result<Dynamic, Box<EvalAltResult>> {
        c.charge(cost::LIGHT);
        let view = c.host().screen(name).map_err(err_to_rhai)?;
        rhai::serde::to_dynamic(view)
    });

    let c = ctx.clone();
    engine.register_fn(
        "can_build",
        move |building: &str, x: i64, y: i64| -> Result<Dynamic, Box<EvalAltResult>> {
            c.charge(cost::LIGHT);
            let view = c.host().can_build(building, x as i32, y as i32);
            rhai::serde::to_dynamic(view)
        },
    );

    let c = ctx.clone();
    engine.register_fn("act", move |steps: Dynamic| do_act(&c, steps, None));
    let c = ctx.clone();
    engine.register_fn("act", move |steps: Dynamic, note: &str| do_act(&c, steps, Some(note)));

    let c = ctx.clone();
    engine.register_fn("keys", move |script: &str| do_keys(&c, script, None));
    let c = ctx.clone();
    engine.register_fn("keys", move |script: &str, note: &str| do_keys(&c, script, Some(note)));

    let c = ctx.clone();
    engine.register_fn("wait", move |turns: i64| -> Result<Dynamic, Box<EvalAltResult>> {
        do_wait(&c, turns)
    });

    let c = ctx.clone();
    engine.register_fn("events", move || -> Result<Dynamic, Box<EvalAltResult>> {
        c.charge(cost::LIGHT);
        rhai::serde::to_dynamic(c.events_since_start())
    });

    engine.register_fn("wake", |msg: &str| -> Result<(), Box<EvalAltResult>> {
        Err(terminate(Signal::Wake { msg: msg.to_owned() }))
    });

    let c = ctx.clone();
    engine.register_fn("plan", move || -> Result<Dynamic, Box<EvalAltResult>> {
        // Always a map, even for a session that never set a plan.
        c.charge(cost::LIGHT);
        if c.plan.is_object() {
            rhai::serde::to_dynamic(&c.plan)
        } else {
            Ok(Dynamic::from_map(rhai::Map::new()))
        }
    });

    let c = ctx.clone();
    engine.register_fn("log", move |msg: Dynamic| {
        c.push_log(log_text(&msg));
    });
}

/// `sdk_reference`'s own plumbing half (#480, `crate::tools`): one `#{signature, doc}` per
/// function [`register`] adds, in the order `Engine::gen_fn_signatures` lists them --
/// `host`'s own state is untouched (a throwaway [`Context`] and [`Engine`] are built, registered
/// against and dropped, with nothing ever actually called) -- so the signatures themselves can
/// never drift from the real plumbing, even though [`plumbing_docs`]'s own prose next to each
/// one is this module's own hand-kept text (a native `register_fn` carries no doc comment the
/// way a script-defined function's `///` can, unlike [`crate::script::prelude::reference`]'s).
/// `-> Type` and every parameter's own `: Type` are dropped from `gen_fn_signatures`' own
/// signature text (`simplify_signature`): with no named parameters registered, they're raw
/// Rust type names (`Box<EvalAltResult>` and the rest), not worth a script author's reading --
/// [`plumbing_docs`]'s own prose says what each parameter actually means instead.
pub(crate) fn plumbing_reference(host: &mut Host) -> Vec<serde_json::Value> {
    let ctx = Context::new(
        host,
        "sdk_reference".to_owned(),
        0,
        0,
        Vec::new(),
        serde_json::Value::Null,
        false,
    );
    let mut engine = Engine::new();
    register(&mut engine, &ctx);
    let docs: std::collections::BTreeMap<&str, &str> = plumbing_docs().iter().copied().collect();
    engine
        .gen_fn_signatures(false)
        .into_iter()
        .map(|sig| {
            let signature = simplify_signature(&sig);
            let name = signature.split('(').next().unwrap_or(&signature);
            let doc = docs.get(name).copied().unwrap_or("");
            serde_json::json!({"signature": signature, "doc": doc})
        })
        .collect()
}

/// `"act(_: Dynamic, _: string) -> Result<...>"` to `"act(_, _)"`: drops `gen_fn_signatures`'
/// own `-> Type` suffix and every parameter's `: Type` in turn, keeping just the name and how
/// many parameters it takes -- `plumbing_docs`'s own prose is the readable half.
fn simplify_signature(sig: &str) -> String {
    let head = sig.split(" -> ").next().unwrap_or(sig);
    let Some(open) = head.find('(') else { return head.to_owned() };
    let name = &head[..open];
    let inner = head[open + 1..].trim_end_matches(')');
    let params: Vec<&str> = if inner.trim().is_empty() {
        Vec::new()
    } else {
        inner.split(',').map(|p| p.split(':').next().unwrap_or(p).trim()).collect()
    };
    format!("{name}({})", params.join(", "))
}

/// One line per plumbing function [`register`] adds, keyed by its own Rhai name: kept by hand
/// (see [`plumbing_reference`]'s own doc comment for why), and checked against `register`'s
/// actual names by `every_registered_function_has_a_plumbing_doc_line` below, so the two can't
/// quietly drift apart -- add a line here in the same commit that adds a new `register_fn` call.
fn plumbing_docs() -> &'static [(&'static str, &'static str)] {
    &[
        (
            "observe",
            "observe() -> turn, clock, season, tool, notice, people, research, stores, \
             endeavor, the cursor's own tile and alerts since the last turn. Free: changes \
             nothing and takes no game time.",
        ),
        (
            "look",
            "look(x, y, w, h) -> #{x, y, w, h, things, ground, legend, marks}: a map region, \
             at most 64x40, as the same text --ascii draws there.",
        ),
        (
            "cells",
            "cells(x, y, w, h) -> [#{x, y, thing, ground, marks}], one entry per on-map cell: \
             the same thing and ground description look()'s own legend would decode its glyph \
             to, without needing the legend.",
        ),
        (
            "inspect",
            "inspect(x, y) -> that tile's Cursor-section lines, as if the cursor sat there.",
        ),
        (
            "screen",
            "screen(name) -> nation, research, endeavor, log, help or build, as data: the \
             same text that screen shows, without opening it.",
        ),
        (
            "can_build",
            "can_build(building, x, y) -> #{ok, reason}: whether building could be designated \
             with its anchor (top-left tile) at (x, y), the same placement check a build \
             step's own confirm uses. Read-only: presses no keys and changes nothing. Unknown \
             or locked buildings give ok: false with a reason.",
        ),
        (
            "act",
            "act(steps) or act(steps, note) -> #{ok, keys, label, notices, error, observe}: \
             runs named steps (designate, build, select, research, cursor, keys) in order, \
             each lowered to the keys a player would press. ok is false when a step couldn't \
             be planned (error is then #{step, reason}, else ()) or a designate/build step \
             got \"0 done\" (see notices): the test the run report's acts.failed counts. ok \
             says the keys were pressed, not what they did: check observe afterwards, say \
             observe.research.label after a research step. Refused in a probe (inline run_script code): \
             \"probes can't act\".",
        ),
        (
            "keys",
            "keys(script) or keys(script, note) -> #{pressed, refused}: a --keys token script, \
             pressed directly. Refused in a probe (inline run_script code).",
        ),
        (
            "wait",
            "wait(turns) -> the last turn's own observation: runs at least 1 turn \
             (end_turn(1), repeated), stopping the whole script early if a wake_on event lands. Refused in a probe \
             (inline run_script code).",
        ),
        ("events", "events() -> [#{turn, kind, text}], every event since this run started."),
        (
            "wake",
            "wake(msg) -> never returns normally: ends the whole script right there with \
             reason \"asked\" and msg.",
        ),
        (
            "plan",
            "plan() -> #{...}: the plan map the plan tool holds (the model's intent as data: \
             goal, order, research, build, targets), as it stood when this run started. \
             Read-only: changing the returned map changes nothing, and `{}` when no plan is \
             set. Edit it with the plan tool between runs; a long-running governor that wants \
             a fresh plan returns or wakes so it's re-run.",
        ),
        (
            "log",
            "log(value) -> (); the script's own log line, kept in the wake report's log tail. \
             Takes any value: a string as written, anything else (a number, a map, an array, \
             ()) as its printed form.",
        ),
    ]
}

fn do_act(
    ctx: &Context,
    steps: Dynamic,
    note: Option<&str>,
) -> Result<Dynamic, Box<EvalAltResult>> {
    ctx.refuse_if_probe()?;
    ctx.charge(cost::HEAVY);
    let steps: Vec<ActionStep> = rhai::serde::from_dynamic(&steps)?;
    let note = Some(ctx.tag_note(note));
    let result = ctx.host().act(&steps, note);
    let failure = act_failure(&result);
    ctx.record(failure.clone());
    let mut out = rhai::serde::to_dynamic(result)?;
    // `ok` is the test `acts.failed` counts, so a script and its run report agree on which acts
    // failed. It isn't a field of `ActResult`: the `act` tool's own result stays as it is.
    if let Some(mut map) = out.write_lock::<rhai::Map>() {
        map.insert("ok".into(), Dynamic::from(failure.is_none()));
    }
    Ok(out)
}

/// What `log` writes for `value`: a string as it is, anything else as the form a script's own
/// string interpolation prints (`()` as `()`), so `log(r)` of a map or a number never errors.
fn log_text(value: &Dynamic) -> String {
    if value.is_unit() { "()".to_owned() } else { value.to_string() }
}

/// `result`'s own failure text, if any, for [`Context::record`]: the step that couldn't even be
/// planned (`ActResult::error`), or -- short of that -- a `designate`/`build` step whose own
/// notice says nothing actually got done (`"0 done, ..."`, the same text a player's Tool section
/// shows, `fn_ui::tools`'s own "N done, M skipped" shape). A partial skip (something still got
/// done) stays `None`: still worth counting a success, the way a mostly-filled rectangle is for
/// a player watching the same notice.
fn act_failure(result: &ActResult) -> Option<String> {
    if let Some(err) = &result.error {
        let label = result.label.as_deref().unwrap_or("act");
        return Some(format!("{label}: step {} failed: {}", err.step, err.reason));
    }
    result.notices.iter().find(|n| n.notice.contains("0 done,")).map(|n| n.notice.clone())
}

fn do_keys(ctx: &Context, script: &str, note: Option<&str>) -> Result<Dynamic, Box<EvalAltResult>> {
    ctx.refuse_if_probe()?;
    ctx.charge(cost::HEAVY);
    let note = Some(ctx.tag_note(note));
    let result = ctx.host().press_keys(script, None, note).map_err(err_to_rhai)?;
    ctx.record(keys_failure(script, &result));
    rhai::serde::to_dynamic(result)
}

/// A `keys` call that had anything refused counts as a failure (`Context::record`): the script
/// asked for something the host wouldn't press, same as a player finding a key doesn't work.
fn keys_failure(script: &str, result: &PressResult) -> Option<String> {
    (result.refused > 0).then(|| format!("keys {script:?}: {} refused", result.refused))
}

/// Runs `turns` (at least 1) turns of `Host::end_turn(1)` each, in a loop that checks the
/// script's own turn budget (`run_script`'s own `turns`, [`Context::max_turns`]) before every
/// single turn -- so a `wait` that asks for more than is left still runs whatever still fits,
/// then stops the whole script with [`Signal::Budget`] instead of quietly returning early and
/// letting the script read a half-run `wait` as if it had finished -- and checks, after every
/// turn it does run, whether that turn produced an event in [`Context::wake_on`]
/// (docs/design/agent-scripting.md, "`run_script`": "A `wake_on` event lands during a `wait`
/// ends the script with reason `event` at the end of that turn"), stopping with [`Signal::Event`]
/// right there rather than running the rest of `turns`.
fn do_wait(ctx: &Context, turns: i64) -> Result<Dynamic, Box<EvalAltResult>> {
    ctx.refuse_if_probe()?;
    let n = turns.max(1) as u32;
    let mut last = None;
    for _ in 0..n {
        if ctx.turns_used() >= ctx.max_turns {
            return Err(terminate(Signal::Budget));
        }
        let before = ctx.host().event_mark();
        last = Some(ctx.host().end_turn(1));
        ctx.bump_turns();
        let fresh = ctx.host().events_since(before);
        if let Some(ev) = fresh.into_iter().find(|e| ctx.wake_on.contains(&e.kind)) {
            return Err(terminate(Signal::Event { kind: ev.kind, text: ev.text, turn: ev.turn }));
        }
    }
    // The script is running again: its operation budget starts over (#564).
    ctx.reset_work();
    rhai::serde::to_dynamic(last.expect("n is at least 1 (turns.max(1))"))
}

fn err_to_rhai(e: String) -> Box<EvalAltResult> {
    e.into()
}

fn clamp_u16(n: i64) -> u16 {
    n.clamp(0, i64::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> Host {
        let nation = fn_tui::cli::parse_nation("earth").unwrap();
        Host::new(7, nation, None, None, None, false).unwrap()
    }

    /// Every function [`register`] adds shows up in [`plumbing_reference`] with a non-empty
    /// doc line: the moment a new `register_fn` call lands with no matching
    /// [`plumbing_docs`] entry, this fails rather than `sdk_reference` quietly serving a blank
    /// line for it.
    #[test]
    fn every_registered_plumbing_function_has_a_non_empty_doc_line() {
        let mut h = host();
        let reference = plumbing_reference(&mut h);
        assert!(!reference.is_empty());
        for entry in &reference {
            let doc = entry["doc"].as_str().unwrap();
            assert!(!doc.is_empty(), "{}: no doc line for {entry}", entry["signature"]);
        }
    }

    /// [`plumbing_reference`] names every plumbing call the design doc lists, each with its own
    /// arity (`observe` takes none, `look`/`cells` take four, and so on) -- not just a plain
    /// list of names, so a signature with the wrong parameter count would show up as a mismatch
    /// too.
    #[test]
    fn plumbing_reference_lists_every_plumbing_call_the_design_doc_names() {
        let mut h = host();
        let reference = plumbing_reference(&mut h);
        let signatures: Vec<&str> =
            reference.iter().map(|e| e["signature"].as_str().unwrap()).collect();
        for want in [
            "observe()",
            "cells(_, _, _, _)",
            "inspect(_, _)",
            "screen(_)",
            "can_build(_, _, _)",
            "wait(_)",
            "events()",
            "plan()",
            "wake(_)",
            "log(_)",
        ] {
            assert!(signatures.contains(&want), "{want:?} missing from {signatures:?}");
        }
        // `look` and `act`/`keys` are each registered twice (with and without a trailing
        // `note`), so both arities should show up rather than just one.
        assert!(signatures.contains(&"look(_, _, _, _)"));
        assert!(signatures.contains(&"act(_)"));
        assert!(signatures.contains(&"act(_, _)"));
        assert!(signatures.contains(&"keys(_)"));
        assert!(signatures.contains(&"keys(_, _)"));
    }

    /// [`plumbing_reference`] never actually runs anything against `host`: building its own
    /// throwaway [`Context`] and [`Engine`] touches only [`Context::new`]'s own read
    /// (`Host::event_mark`), so the turn, the log and everything else are exactly what they
    /// were before the call.
    #[test]
    fn plumbing_reference_does_not_touch_the_host_it_was_given() {
        let mut h = host();
        let before = h.observe();
        plumbing_reference(&mut h);
        let after = h.observe();
        assert_eq!(before.turn, after.turn);
        assert_eq!(h.log().lines().len(), 0, "{:?}", h.log().lines());
    }
}

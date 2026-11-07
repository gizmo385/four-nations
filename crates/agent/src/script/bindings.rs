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
}

impl Context {
    pub(super) fn new(
        host: &mut Host,
        tag: String,
        version: u32,
        max_turns: u32,
        wake_on: Vec<EventKind>,
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
        })
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

    /// Tallies one `act`/`keys` step toward the eventual wake report's own `acts` counts:
    /// `Some(msg)` counts as a failure and becomes the new `last_failure`; `None` counts as ok.
    fn record(&self, failure: Option<String>) {
        match failure {
            Some(msg) => {
                self.acts_failed.set(self.acts_failed.get() + 1);
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
        rhai::serde::to_dynamic(c.host().observe())
    });

    let c = ctx.clone();
    engine.register_fn(
        "look",
        move |x: i64, y: i64, w: i64, h: i64| -> Result<Dynamic, Box<EvalAltResult>> {
            let view = c.host().look(x as i32, y as i32, clamp_u16(w), clamp_u16(h));
            rhai::serde::to_dynamic(view)
        },
    );

    let c = ctx.clone();
    engine.register_fn("inspect", move |x: i64, y: i64| -> Result<Dynamic, Box<EvalAltResult>> {
        rhai::serde::to_dynamic(c.host().inspect(x as i32, y as i32))
    });

    let c = ctx.clone();
    engine.register_fn(
        "cells",
        move |x: i64, y: i64, w: i64, h: i64| -> Result<Dynamic, Box<EvalAltResult>> {
            let view = c.host().cells(x as i32, y as i32, clamp_u16(w), clamp_u16(h));
            rhai::serde::to_dynamic(view)
        },
    );

    let c = ctx.clone();
    engine.register_fn("screen", move |name: &str| -> Result<Dynamic, Box<EvalAltResult>> {
        let view = c.host().screen(name).map_err(err_to_rhai)?;
        rhai::serde::to_dynamic(view)
    });

    let c = ctx.clone();
    engine.register_fn(
        "can_build",
        move |building: &str, x: i64, y: i64| -> Result<Dynamic, Box<EvalAltResult>> {
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
        rhai::serde::to_dynamic(c.events_since_start())
    });

    engine.register_fn("wake", |msg: &str| -> Result<(), Box<EvalAltResult>> {
        Err(terminate(Signal::Wake { msg: msg.to_owned() }))
    });

    let c = ctx.clone();
    engine.register_fn("log", move |msg: &str| {
        c.push_log(msg.to_owned());
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
    let ctx = Context::new(host, "sdk_reference".to_owned(), 0, 0, Vec::new());
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
            "act(steps) or act(steps, note) -> #{keys, label, notices, error, observe}: runs \
             named steps (designate, build, select, research, cursor, keys) in order, each \
             lowered to the keys a player would press.",
        ),
        (
            "keys",
            "keys(script) or keys(script, note) -> #{pressed, refused}: a --keys token script, \
             pressed directly.",
        ),
        (
            "wait",
            "wait(turns) -> the last turn's own observation: runs at least 1 turn \
             (end_turn(1), repeated), stopping the whole script early if a wake_on event lands.",
        ),
        ("events", "events() -> [#{turn, kind, text}], every event since this run started."),
        (
            "wake",
            "wake(msg) -> never returns normally: ends the whole script right there with \
             reason \"asked\" and msg.",
        ),
        ("log", "log(msg) -> (); the script's own log line, kept in the wake report's log tail."),
    ]
}

fn do_act(
    ctx: &Context,
    steps: Dynamic,
    note: Option<&str>,
) -> Result<Dynamic, Box<EvalAltResult>> {
    let steps: Vec<ActionStep> = rhai::serde::from_dynamic(&steps)?;
    let note = Some(ctx.tag_note(note));
    let result = ctx.host().act(&steps, note);
    ctx.record(act_failure(&result));
    rhai::serde::to_dynamic(result)
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

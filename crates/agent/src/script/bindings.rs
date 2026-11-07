//! The plumbing layer (docs/design/agent-scripting.md, "The SDK", "Plumbing: 1:1 with `Host`"):
//! [`register`] adds `observe`, `look`, `inspect`, `screen`, `act`, `keys`, `wait`, `events`,
//! `wake` and `log` to an [`Engine`], each exactly the [`Host`] call its name says (`events` and
//! `wake` are new, #478), with its result serialized to a Rhai map through `rhai::serde` --
//! nothing a plumbing call does reaches anything but `Host`'s own public methods, so a script
//! can't do anything a human's keyboard or an MCP agent couldn't.

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
    /// `run_script`'s own tag for every `act`/`keys` step this run makes (the caller's `note`,
    /// or `"script"` with none given, #477's stand-in for the saved script identity #479 adds).
    tag: String,
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
        max_turns: u32,
        wake_on: Vec<EventKind>,
    ) -> Rc<Self> {
        let events_start = host.event_mark();
        Rc::new(Self {
            host: std::ptr::from_mut(host),
            tag,
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

    /// `note` tagged with [`Self::tag`] (docs/design/agent-scripting.md, "Determinism, replay
    /// and watching": "Its label is the step and its note is `[keep-fed v3]` plus the script's
    /// own note").
    fn tag_note(&self, note: Option<&str>) -> String {
        match note {
            Some(n) if !n.is_empty() => format!("[{}] {n}", self.tag),
            _ => format!("[{}]", self.tag),
        }
    }

    /// `self.tag`, for [`crate::script::runner::run`]'s own wake report (`Outcome::script`).
    pub(super) fn tag(&self) -> &str {
        &self.tag
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
    engine.register_fn("screen", move |name: &str| -> Result<Dynamic, Box<EvalAltResult>> {
        let view = c.host().screen(name).map_err(err_to_rhai)?;
        rhai::serde::to_dynamic(view)
    });

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

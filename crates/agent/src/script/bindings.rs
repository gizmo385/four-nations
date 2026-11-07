//! The plumbing layer (docs/design/agent-scripting.md, "The SDK", "Plumbing: 1:1 with `Host`"):
//! [`register`] adds `observe`, `look`, `inspect`, `screen`, `act`, `keys`, `wait` and `log` to
//! an [`Engine`], each exactly the [`Host`] call its name says, with its result serialized to a
//! Rhai map through `rhai::serde` -- nothing a plumbing call does reaches anything but `Host`'s
//! own public methods, so a script can't do anything a human's keyboard or an MCP agent
//! couldn't.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fn_tui::surface::{ActionStep, Host};
use rhai::{Dynamic, Engine, EvalAltResult, Position};

/// The termination token [`register`]'s `wait` raises through `EvalAltResult::ErrorTerminated`
/// once a script's own turn budget runs out mid-`wait` (`crate::script::runner::run` reads it
/// back to tell "budget" apart from a genuine script error or a `runaway`, which Rhai's own
/// `Engine::set_max_operations` reports as `ErrorTooManyOperations` instead).
pub(super) const BUDGET_TOKEN: &str = "fn_agent:budget";

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
}

impl Context {
    pub(super) fn new(host: &mut Host, tag: String, max_turns: u32) -> Rc<Self> {
        Rc::new(Self {
            host: std::ptr::from_mut(host),
            tag,
            log: RefCell::new(Vec::new()),
            max_turns,
            turns_used: Cell::new(0),
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
    rhai::serde::to_dynamic(ctx.host().act(&steps, note))
}

fn do_keys(ctx: &Context, script: &str, note: Option<&str>) -> Result<Dynamic, Box<EvalAltResult>> {
    let note = Some(ctx.tag_note(note));
    let result = ctx.host().press_keys(script, None, note).map_err(err_to_rhai)?;
    rhai::serde::to_dynamic(result)
}

/// Runs `turns` (at least 1) turns of `Host::end_turn(1)` each, in a loop that checks the
/// script's own turn budget (`run_script`'s own `turns`, [`Context::max_turns`]) before every
/// single turn -- so a `wait` that asks for more than is left still runs whatever still fits,
/// then stops the whole script with [`BUDGET_TOKEN`] instead of quietly returning early and
/// letting the script read a half-run `wait` as if it had finished.
fn do_wait(ctx: &Context, turns: i64) -> Result<Dynamic, Box<EvalAltResult>> {
    let n = turns.max(1) as u32;
    let mut last = None;
    for _ in 0..n {
        if ctx.turns_used() >= ctx.max_turns {
            return Err(budget_exceeded());
        }
        last = Some(ctx.host().end_turn(1));
        ctx.bump_turns();
    }
    rhai::serde::to_dynamic(last.expect("n is at least 1 (turns.max(1))"))
}

fn budget_exceeded() -> Box<EvalAltResult> {
    Box::new(EvalAltResult::ErrorTerminated(Dynamic::from(BUDGET_TOKEN.to_owned()), Position::NONE))
}

fn err_to_rhai(e: String) -> Box<EvalAltResult> {
    e.into()
}

fn clamp_u16(n: i64) -> u16 {
    n.clamp(0, i64::from(u16::MAX)) as u16
}

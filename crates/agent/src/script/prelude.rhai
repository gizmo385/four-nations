// The porcelain layer (#480, docs/design/agent-scripting.md, "The SDK", "Porcelain:
// prelude.rhai"): written only against the plumbing (`observe`, `look`, `cells`, `inspect`,
// `screen`, `act`, `keys`, `wait`, `events`, `wake`, `log`), loaded before every script
// (`crate::script::runner::run` merges this file's own compiled AST with the caller's, so
// line:col in a wake report always points into the caller's own code). An action-shaped call
// (`designate_near`, `build_near`) returns `#{ok, msg, ...}`, reusing the underlying `act`
// step's own notice text for `msg` where there is one, the same words a player's Tool section
// would show; a plain reader (`find`, `store`, `idle`, `need`) just returns the value.
//
// `stalled` is the one helper that needs to remember something between calls. Rhai functions
// are pure -- they can never see a script's own local or global variables, only their own
// parameters -- but a function that uses `this` in its body can be called with "method"
// syntax (`receiver.name(...)`), and then `this` *is* the receiver, by reference. Calling it as
// `memory.stalled(...)` therefore reads and writes `memory` itself: the one thing that already
// persists both within a script's own loop (`wait` doesn't reset it) and across separate
// `run_script` calls in a session (#479's own `memory`), so a wake that interrupts a loop and a
// fresh run of the same script afterwards both pick the same count back up.

/// `near`'s own search box for `find`/`build_near`: `radius` clamped to 19, so `w` and `h`
/// (`2 * radius + 1`, at most 39) always fit in one `cells()` call under `Host::cells`'s own
/// 64x40 cap, whichever edge of the map `near` is near.
fn search_box(near, radius) {
    let r = min(radius, 19);
    let span = r * 2 + 1;
    #{x: near.x - r, y: near.y - r, w: span, h: span}
}

/// Whether every cell of a `w`x`h` footprint whose top-left corner is `(ox, oy)` is a key of
/// `empty` (`free_origins`'s own `"x,y"` lookup): a private helper, not a plumbing call itself.
fn footprint_is_free(empty, ox, oy, w, h) {
    for dy in range(0, h) {
        for dx in range(0, w) {
            if !empty.contains(`${ox + dx},${oy + dy}`) {
                return false;
            }
        }
    }
    true
}
/// Every top-left corner within `radius` of `near` (`search_box`'s own clamp) where a `w`x`h`
/// footprint lands on cells with nothing on them and no mark, nearest first: `build_near`'s own
/// first pass. "Free" here is only what `cells()` shows, not whether the ground takes a
/// building: a lake, a mountain and a spot the nation can't build on all read "empty" too. So
/// don't `act` on a candidate unchecked: `build_near` asks `can_build` about each one first.
fn free_origins(near, radius, w, h) {
    let region = search_box(near, radius);
    let empty = #{};
    for c in cells(region.x, region.y, region.w, region.h) {
        if c.thing == "-" && c.marks.len() == 0 {
            empty[`${c.x},${c.y}`] = true;
        }
    }
    let origins = [];
    for oy in range(region.y, region.y + region.h - h + 1) {
        for ox in range(region.x, region.x + region.w - w + 1) {
            if footprint_is_free(empty, ox, oy, w, h) {
                origins.push(#{x: ox, y: oy});
            }
        }
    }
    origins.sort(|a, b| {
        let da = (a.x - near.x) * (a.x - near.x) + (a.y - near.y) * (a.y - near.y);
        let db = (b.x - near.x) * (b.x - near.x) + (b.y - near.y) * (b.y - near.y);
        da - db
    });
    origins
}

/// Tiles within `radius` of `near` (clamped to `search_box`'s own 19) whose `thing` contains
/// `desc` (case-insensitive), nearest first: `cells()`'s own `#{x, y, thing, ground, marks}`
/// maps, so a script never has to decode `look`'s own legend to find something.
fn find(desc, near, radius) {
    let region = search_box(near, radius);
    let needle = desc.to_lower();
    let matches = cells(region.x, region.y, region.w, region.h)
        .filter(|c| c.thing.to_lower().contains(needle));
    matches.sort(|a, b| {
        let da = (a.x - near.x) * (a.x - near.x) + (a.y - near.y) * (a.y - near.y);
        let db = (b.x - near.x) * (b.x - near.x) + (b.y - near.y) * (b.y - near.y);
        da - db
    });
    matches
}

/// Designates every tile `find(desc, near, radius)` turns up for `tool` (`"harvest"`,
/// `"remove"`, `"clear"`, `"stockpile"`, `"bend"` or `"sow"`), each its own 1x1 rectangle, in
/// one `act` call: `#{ok, msg, found, done}`. `found` is how many tiles matched `desc` at all;
/// `done` is how many of those the designation actually took (short of `found` when some are
/// out of reach or already designated, say). `ok` is `done > 0`. `standing` is `act`'s own
/// rule: only `"harvest"` takes it, and only there does a `true` re-queue the designation once
/// the resource ripens again.
fn designate_near(tool, desc, near, radius, standing) {
    let matches = find(desc, near, radius);
    if matches.len() == 0 {
        return #{
            ok: false,
            msg: `designate_near: no ${desc} found within ${radius} of (${near.x},${near.y})`,
            found: 0,
            done: 0,
        };
    }
    let steps = [];
    for m in matches {
        let at = #{x: m.x, y: m.y};
        steps.push(#{step: "designate", tool: tool, from: at, to: at, standing: standing});
    }
    let note = `designate ${tool} ${desc} near (${near.x},${near.y})`;
    let r = act(steps, note);
    if r.error != () {
        return #{
            ok: false,
            msg: `designate_near: step ${r.error.step} failed: ${r.error.reason}`,
            found: matches.len(),
            done: r.error.step,
        };
    }
    let done = 0;
    for n in r.notices {
        if !n.notice.contains("0 done,") {
            done += 1;
        }
    }
    let msg = "";
    if done == matches.len() {
        msg = `designated all ${done} ${desc} tile(s) near (${near.x},${near.y})`;
    } else {
        msg = `designated ${done} of ${matches.len()} ${desc} tile(s) near (${near.x},${near.y})`;
    }
    #{ok: done > 0, msg: msg, found: matches.len(), done: done}
}

/// The text inside the last `(...)` of a "<what>: N done, M skipped (reason)" notice
/// (`fn_ui::tools::apply_rect`'s own shape, the same text a player's Tool section shows):
/// `notice` unchanged if it carries no parentheses (an `act` step that failed to plan at all
/// has no notice to begin with -- `build_near`'s own caller uses `r.error.reason` for that
/// case instead).
fn refusal_reason(notice) {
    let open = notice.index_of("(");
    if open < 0 {
        return notice;
    }
    let close = notice.index_of(")", open);
    if close < 0 {
        return notice;
    }
    notice.sub_string(open + 1, close - open - 1)
}

/// `reasons`'s own most frequent key (`#{reason: count}`, `build_near`'s own tally): ties break
/// on whichever `reasons.keys()` lists first (Rhai's own object map order, insertion order), so
/// the first distinct reason seen among equally common ones wins. Never actually called with an
/// empty map -- `build_near` only reaches this after at least one refused candidate.
fn most_common_reason(reasons) {
    let best = "";
    let best_count = -1;
    for key in reasons.keys() {
        if reasons[key] > best_count {
            best = key;
            best_count = reasons[key];
        }
    }
    best
}

/// Finds a spot for `building` (its size from `screen("build")`'s own rows) within 15 tiles of
/// `near`, builds it there, and returns `#{ok, msg, tries, checked}` with the `build` step's own
/// notice text. Walks the candidates `free_origins` finds, nearest first, and asks the
/// read-only `can_build` (what a player's build ghost shows) about each: it spends an `act`
/// only on a spot `can_build` accepts, so water, rock and anything else the ground refuses cost
/// no `act`. `checked` is how many candidates `can_build` was asked about, at most 120 (40
/// operations each, #564's budget); `tries` is how many `act`s were spent, at most `max_tries`
/// (#508: a spot `can_build` accepts but the `build` step still refuses is rare, and counts).
/// `ok` is false with `tries: 0` when `building` isn't a name `screen("build")` lists, or is
/// locked, or no free footprint exists at all; when none of the candidates was accepted, `msg`
/// says how many were checked and the refusal `can_build` gave most often.
fn build_near(building, near, max_tries) {
    let row = screen("build").rows.find(|r| r.name == building);
    if row == () {
        return #{ok: false, msg: `build_near: no such building ${building}`, tries: 0, checked: 0};
    }
    if row.locked {
        return #{ok: false, msg: `build_near: ${building} is locked`, tries: 0, checked: 0};
    }
    let size = row.size.split("x");
    let w = parse_int(size[0]);
    let h = parse_int(size[1]);
    let origins = free_origins(near, 15, w, h);
    if origins.len() == 0 {
        return #{
            ok: false,
            msg: `build_near: no free ${row.size} spot for ${building} within 15 of (${near.x},${near.y})`,
            tries: 0,
            checked: 0,
        };
    }
    let note = `build ${building} near (${near.x},${near.y})`;
    let tries = 0;
    let checked = 0;
    let reasons = #{};
    for o in origins {
        if tries >= max_tries || checked >= 120 {
            break;
        }
        checked += 1;
        let can = can_build(building, o.x, o.y);
        if !can.ok {
            let why = can.reason ?? "refused";
            reasons[why] = (reasons[why] ?? 0) + 1;
            continue;
        }
        tries += 1;
        let to = #{x: o.x + w - 1, y: o.y + h - 1};
        let r = act([#{step: "build", building: building, from: o, to: to}], note);
        if r.error == () && r.notices.len() > 0 && !r.notices[0].notice.contains("0 done,") {
            return #{ok: true, msg: r.notices[0].notice, tries: tries, checked: checked};
        }
        let reason = if r.error != () {
            r.error.reason
        } else if r.notices.len() > 0 {
            refusal_reason(r.notices[0].notice)
        } else {
            "refused"
        };
        reasons[reason] = (reasons[reason] ?? 0) + 1;
    }
    #{
        ok: false,
        msg: `build_near: no spot for ${building} near (${near.x},${near.y}) that can_build accepts; checked ${checked}, built on ${tries}; most said: ${most_common_reason(reasons)}`,
        tries: tries,
        checked: checked,
    }
}

/// `build_near(building, near, 8)`: at most 8 `act`s by default (#508).
fn build_near(building, near) {
    build_near(building, near, 8)
}

/// Waits until `cond` (a function pointer or closure taking no arguments, called with
/// `cond.call()`) returns true, checking once after every turn, for at most `max_turns` turns:
/// `true` the moment it does, `false` once `max_turns` is spent without it. Built on the
/// plumbing's own `wait(1)`, so a `wake_on` event landing during the wait still ends the whole
/// script with reason `"event"` right there, exactly as a plain `wait` would -- `wait_until`
/// never shields a script from that.
fn wait_until(cond, max_turns) {
    if cond.call() {
        return true;
    }
    let left = max_turns;
    while left > 0 {
        wait(1);
        left -= 1;
        if cond.call() {
            return true;
        }
    }
    false
}

/// Stuck detection: call as `memory.stalled(name, value, turns)` (this file's own doc comment
/// explains why `memory.` isn't optional). Records `value` the first time `name` is seen (or
/// the first time it changes), and calls `wake` once it has stayed exactly there for `turns`
/// game turns since -- not merely unlucky this one turn. Always returns `false` itself; the
/// wake, if any, is how a caller learns it fired.
fn stalled(name, value, turns) {
    let key = "_stalled_" + name;
    let now = observe().turn;
    let prev = this[key];
    if prev == () || prev.value != value {
        this[key] = #{value: value, since: now};
        return false;
    }
    if now - prev.since >= turns {
        wake(`stalled: ${name} has stayed at ${value} for ${turns} turns`);
    }
    false
}

/// `observe().stores`'s own count for `name` (zero if the nation has never had any): the same
/// name the Build modal's cost text and the side panel's Stores section use.
fn store(name) {
    let found = observe().stores.find(|s| s.name == name);
    if found == () {
        0
    } else {
        found.count
    }
}

/// How many settlers `observe().people.activity` currently reports idle (zero if none): the
/// same count the side panel's own People row shows.
fn idle() {
    let activity = observe().people.activity;
    if activity.contains("Idle") {
        activity.Idle
    } else {
        0
    }
}

/// `observe().people.needs`'s own count for `name` (zero if nobody currently has it): the same
/// word the side panel's own need flags use ("Thirsty", "Hungry", "Tired", ...).
fn need(name) {
    let needs = observe().people.needs;
    if needs.contains(name) {
        needs[name]
    } else {
        0
    }
}

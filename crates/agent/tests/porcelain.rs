//! The porcelain layer end-to-end (#480, docs/design/agent-scripting.md, "The SDK",
//! "Porcelain: `prelude.rhai`"), on seed 7 Earth: `build_near` finds a free footprint and
//! builds it, `designate_near` marks every matching tile in range, and `stalled` wakes the
//! model once a tracked value has sat still for long enough. Each is run through
//! [`fn_agent::script::run`] exactly as `run_script` would, against the real starting world
//! (`#480`'s own acceptance criteria asks for seed 7 Earth specifically).

use fn_agent::script::{Limits, Request, run};
use fn_tui::surface::{EventKind, Host, LogLine};

const SEED: u64 = 7;

/// Earth's own starting camp (#480's own diagnostics): six settlers (Oma, Ping, Shan, Bai,
/// Fen, Hong) clustered around here, with Wild Cabbage and free ground both within a short
/// walk -- `camp = #{x: 94, y: 64}` in every script below.
const CAMP_X: i32 = 94;
const CAMP_Y: i32 = 64;

fn host() -> Host {
    let nation = fn_tui::cli::parse_nation("earth").unwrap();
    Host::new(SEED, nation, None, None, None, false).unwrap()
}

fn limits() -> Limits {
    Limits {
        default_turns: 15,
        max_turns: 180,
        max_operations: 2_000_000,
        log_tail: 30,
        default_wake: vec![EventKind::Death, EventKind::Need, EventKind::Endeavor],
        memory_bytes: 8192,
        plan_bytes: 8192,
    }
}

/// Runs `code` for at most `turns` turns with no `wake_on` events (the porcelain's own `wake`
/// calls, like `stalled`'s, still stop it): the common shape every test below shares.
fn run_porcelain(host: &mut Host, code: &str, turns: u32) -> fn_agent::script::Outcome {
    let memory = serde_json::json!({});
    let request = Request {
        code,
        turns: Some(turns),
        note: None,
        version: 0,
        wake_on: &[],
        memory: &memory,
        plan: &serde_json::json!({}),
        probe: false,
    };
    run(host, request, &limits())
}

/// `build_near("Bed", camp)` finds a free 1x1 footprint near the camp, builds it there, and
/// -- given enough turns for a settler to actually fetch the wood and finish it -- a Bed really
/// is standing there afterwards, the way a player's own Build tool would have left one.
#[test]
fn build_near_places_a_bed_that_later_finishes() {
    let mut h = host();
    let code = r#"
        let camp = #{x: 94, y: 64};
        let r = build_near("Bed", camp);
        wait(90);
        r
    "#;
    let out = run_porcelain(&mut h, code, 90);
    assert_eq!(out.reason, "done", "{out:?}");
    let r = out.value.expect("build_near returns a map");
    assert_eq!(r["ok"], true, "{r}");

    let area = h.cells(CAMP_X - 15, CAMP_Y - 15, 31, 31);
    assert!(
        area.iter().any(|c| c.thing.contains("Bed")),
        "no Bed standing near the camp after 90 turns: {:?}",
        area.iter().map(|c| &c.thing).collect::<Vec<_>>()
    );
    let tries = r["tries"].as_u64().expect("tries is a number");
    assert!((1..=8).contains(&tries), "tries should be within the default max_tries: {r}");
}

/// How many `build` acts the run log holds so far: a `build_near` that never acted writes none.
fn builds_logged(h: &Host) -> usize {
    h.log()
        .lines()
        .iter()
        .filter(|l| {
            matches!(l, LogLine::Keys { label, note, .. }
                if label.as_deref().is_some_and(|s| s.starts_with("build"))
                    || note.as_deref().is_some_and(|s| s.starts_with("build")))
        })
        .count()
}

/// A spot deep enough in the sea that every tile within `build_near`'s own 15-tile search box
/// is water (#508): `free_origins` still calls every one of them "free" (it only looks at
/// `thing`, never the ground), but `build_near` asks `can_build` before it acts (#575), so it
/// spends no `act` at all: `tries` is 0, `checked` is the 120 candidates it looked at (its
/// cap), and `msg` names the refusal every one of them shared. The run log has no build act.
#[test]
fn build_near_on_a_spot_surrounded_by_water_acts_on_nothing() {
    let mut h = host();
    let code = r#"build_near("Bed", #{x: 30, y: 10})"#;
    let out = run_porcelain(&mut h, code, 1);
    assert_eq!(out.reason, "done", "{out:?}");
    let r = out.value.expect("a result map");
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["tries"], 0, "{r}");
    assert_eq!(r["checked"], 120, "{r}");
    let msg = r["msg"].as_str().unwrap();
    assert!(msg.contains("that can_build accepts"), "{msg}");
    assert!(msg.contains("most said: on water"), "{msg}");
    assert_eq!(builds_logged(&h), 0, "no act was spent on water");
}

/// `max_tries` caps `act`s, not `can_build` checks: 0 acts on nothing even on good ground.
#[test]
fn build_near_max_tries_caps_the_acts() {
    let mut h = host();
    let code = r#"build_near("Bed", #{x: 94, y: 64}, 0)"#;
    let out = run_porcelain(&mut h, code, 1);
    assert_eq!(out.reason, "done", "{out:?}");
    let r = out.value.expect("a result map");
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["tries"], 0, "{r}");
    assert_eq!(builds_logged(&h), 0);
}

/// The first land position (scanning rows, then columns) where the 7x7 square centred on it is
/// all water ground, yet a tile `can_build` accepts for a Bed lies within 12: a camp with a
/// lake beside it. `free_origins` offers all 49 water tiles first, nearest first.
fn lake_camp(h: &mut Host) -> (i32, i32) {
    for y in (0..130).step_by(1) {
        for x in 0..200 {
            let row = h.cells(x, y, 7, 1);
            if row.len() < 7 || row.iter().any(|c| c.ground != "water" || c.thing != "-") {
                continue;
            }
            let (cx, cy) = (x + 3, y);
            let square = h.cells(cx - 3, cy - 3, 7, 7);
            if square.len() == 49 && square.iter().all(|c| c.ground == "water" && c.thing == "-") {
                let near = h.cells(cx - 12, cy - 12, 25, 25);
                if near.iter().any(|c| h.can_build("Bed", c.x, c.y).ok) {
                    return (cx, cy);
                }
            }
        }
    }
    panic!("no lake with land within 12 on seed 7");
}

/// #575: with a lake beside the camp, `build_near` walks past the water without spending an
/// `act` on it: one `can_build`-accepted spot, one `build` act on the first try, a site that is
/// not on water, and a Bed that later finishes. The camp is the middle of a 7x7 lake, so the
/// 49 nearest candidates `free_origins` offers are all water.
#[test]
fn build_near_places_a_bed_past_a_lake_on_its_first_try() {
    let mut h = host();
    let (x, y) = lake_camp(&mut h);
    let code = format!(
        r#"
        let camp = #{{x: {x}, y: {y}}};
        let offered = free_origins(camp, 15, 1, 1);
        let first = can_build("Bed", offered[0].x, offered[0].y);
        let r = build_near("Bed", camp);
        #{{r: r, first_ok: first.ok, offered: offered.len()}}
        "#
    );
    let out = run_porcelain(&mut h, &code, 1);
    assert_eq!(out.reason, "done", "{out:?}");
    let v = out.value.expect("a result map");
    assert_eq!(v["first_ok"], false, "the nearest candidate is water: {v}");
    let r = &v["r"];
    assert_eq!(r["ok"], true, "{v}");
    assert_eq!(r["tries"], 1, "placed on the first act: {v}");
    assert!(r["checked"].as_u64().unwrap() > 49, "it checked past the lake: {v}");
    assert_eq!(builds_logged(&h), 1, "exactly one build act, none on water");

    let area = h.cells(x - 15, y - 15, 31, 31);
    let sites: Vec<_> = area.iter().filter(|c| c.marks.iter().any(|m| m == "build")).collect();
    assert_eq!(sites.len(), 1, "one build site: {sites:?}");
    assert_ne!(sites[0].ground, "water", "{sites:?}");
}

/// `build_near` with a building `screen("build")` doesn't offer (or that's locked) fails
/// cleanly with `ok: false` and a message naming why, rather than placing nothing and staying
/// silent about it.
#[test]
fn build_near_an_unknown_or_locked_building_fails_with_a_clear_message() {
    let mut h = host();
    let out = run_porcelain(
        &mut h,
        r#"[build_near("Nonsense Hut", #{x: 94, y: 64}), build_near("Campfire", #{x: 94, y: 64})]"#,
        1,
    );
    assert_eq!(out.reason, "done", "{out:?}");
    let results = out.value.expect("an array of two results");
    let unknown = &results[0];
    assert_eq!(unknown["ok"], false);
    assert!(unknown["msg"].as_str().unwrap().contains("Nonsense Hut"), "{unknown}");
    assert_eq!(unknown["tries"], 0, "{unknown}");
    let locked = &results[1];
    assert_eq!(locked["ok"], false);
    assert!(locked["msg"].as_str().unwrap().contains("locked"), "{locked}");
    assert_eq!(locked["tries"], 0, "{locked}");
}

/// `designate_near("harvest", "Wild Cabbage", camp, 15, false)` designates every Wild Cabbage
/// tile `find` turns up in range -- not just the first, not a mix of hits and misses -- the
/// same count a player's own Harvest tool would mark drawing one rectangle per bush by hand.
#[test]
fn designate_near_harvest_designates_every_matching_tile_in_range() {
    let mut h = host();
    let code = r#"
        let camp = #{x: 94, y: 64};
        let matches = find("Wild Cabbage", camp, 15);
        let r = designate_near("harvest", "Wild Cabbage", camp, 15, false);
        #{matches: matches.len(), result: r}
    "#;
    let out = run_porcelain(&mut h, code, 1);
    assert_eq!(out.reason, "done", "{out:?}");
    let value = out.value.expect("a map of matches and result");
    let matches = value["matches"].as_u64().unwrap();
    assert!(matches > 0, "seed 7 Earth should have Wild Cabbage within 15 of the camp: {value}");
    let r = &value["result"];
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["found"], matches, "{r}");
    assert_eq!(r["done"], matches, "every matching tile should have been designated: {r}");

    // Every matching tile actually carries the harvest mark now, the same thing a player
    // reading the map would see.
    let area = h.cells(CAMP_X - 15, CAMP_Y - 15, 31, 31);
    let cabbage_with_harvest_mark = area
        .iter()
        .filter(|c| c.thing == "Wild Cabbage" && c.marks.iter().any(|m| m == "harvest"))
        .count();
    assert_eq!(cabbage_with_harvest_mark as u64, matches, "{area:?}");
}

/// `designate_near` for a `desc` nothing matches fails cleanly instead of silently designating
/// zero tiles and calling it a success.
#[test]
fn designate_near_with_no_matches_fails_cleanly() {
    let mut h = host();
    let out = run_porcelain(
        &mut h,
        r#"designate_near("harvest", "Rainbow Unicorn Tree", #{x: 94, y: 64}, 10, false)"#,
        1,
    );
    assert_eq!(out.reason, "done", "{out:?}");
    let r = out.value.expect("a result map");
    assert_eq!(r["ok"], false);
    assert_eq!(r["found"], 0);
    assert!(r["msg"].as_str().unwrap().contains("Rainbow Unicorn Tree"), "{r}");
}

/// `memory.stalled(name, value, turns)` wakes the model once a tracked value has sat exactly
/// still for `turns` game turns: a script that watches a count that never moves wakes with
/// reason `"asked"` and a message naming what stalled, not silently running to its own turn
/// budget instead.
#[test]
fn stalled_wakes_after_n_unchanged_turns() {
    let mut h = host();
    let code = r#"
        loop {
            memory.stalled("wood", store("Wood"), 5);
            wait(1);
        }
    "#;
    let out = run_porcelain(&mut h, code, 60);
    assert_eq!(out.reason, "asked", "{out:?}");
    let why = out.why.clone().expect("a wake always says why");
    assert!(why.contains("wood"), "{why}");
    assert!(why.contains("stayed at"), "{why}");
    // It should have fired close to the 5-turn mark, not after running out the whole budget.
    assert!(out.turns_run <= 10, "stalled should have woken well before the budget: {out:?}");
}

/// `stalled` never fires while the tracked value keeps moving: watching something that's
/// actually changing every turn never wakes, however long the script watches it.
#[test]
fn stalled_never_fires_while_the_value_keeps_changing() {
    let mut h = host();
    let code = r#"
        let n = 0;
        loop {
            n += 1;
            memory.stalled("counter", n, 5);
            wait(1);
        }
    "#;
    let out = run_porcelain(&mut h, code, 20);
    assert_eq!(out.reason, "budget", "{out:?}");
    assert_eq!(out.turns_run, 20);
}

/// `store(name)`, `idle()` and `need(name)` read single values out of `observe()`, zero for
/// anything not currently present (a store nobody's ever had, nobody idle, nobody with that
/// need) rather than an error or a missing field.
#[test]
fn store_idle_and_need_read_single_values_with_a_zero_default() {
    let mut h = host();
    let out = run_porcelain(
        &mut h,
        r#"#{wood: store("Wood"), nonsense: store("Nonsense"), idle: idle(), need: need("Nonsense")}"#,
        1,
    );
    assert_eq!(out.reason, "done", "{out:?}");
    let v = out.value.unwrap();
    assert_eq!(v["nonsense"], 0);
    assert_eq!(v["need"], 0);
    // Earth starts with no Wood banked and nobody idle at tick 0 (every settler already has a
    // task): both read back their own real value, not just a hardcoded zero.
    let obs = h.observe();
    let want_wood = obs.stores.iter().find(|s| s.name == "Wood").map_or(0, |s| s.count);
    assert_eq!(v["wood"], want_wood);
    let want_idle = *obs.people.activity.get("Idle").unwrap_or(&0);
    assert_eq!(v["idle"], want_idle);
}

/// `wait_until(|| cond, max_turns)` waits only as long as it has to: it returns `true` the
/// turn a condition first holds, not after running the whole budget regardless.
#[test]
fn wait_until_returns_true_the_turn_the_condition_first_holds() {
    let mut h = host();
    let code = r#"
        wait_until(|| observe().turn >= 4, 30)
    "#;
    let out = run_porcelain(&mut h, code, 30);
    assert_eq!(out.reason, "done", "{out:?}");
    assert_eq!(out.value, Some(serde_json::json!(true)));
    assert_eq!(out.turns_run, 4, "should stop the moment the condition holds, not run the budget");
}

/// `wait_until` returns `false` once `max_turns` is spent with the condition never holding,
/// rather than running past its own limit or raising an error.
#[test]
fn wait_until_returns_false_once_its_own_turns_run_out() {
    let mut h = host();
    let code = r#"
        wait_until(|| false, 5)
    "#;
    let out = run_porcelain(&mut h, code, 30);
    assert_eq!(out.reason, "done", "{out:?}");
    assert_eq!(out.value, Some(serde_json::json!(false)));
    assert_eq!(out.turns_run, 5);
}

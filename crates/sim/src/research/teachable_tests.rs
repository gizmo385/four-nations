//! Borrowed techs (`teachable = true`, docs/design/horizons-phase-6.md, "Borrowed techs:
//! `teachable`", #753): what the data may say, what the bundled flags are, and the pure rules
//! (`borrow_prereqs`, costs, the gate) that need no world.

use super::*;
use crate::defs::{BUNDLED_FILES, DefsError};

/// The bundled files plus the given extra techs, `(id, keys)`, in order.
fn with_techs(techs: &[(&str, &str)]) -> Result<Defs, DefsError> {
    let mut files = BUNDLED_FILES;
    let mut research = files[3].to_owned();
    for (id, extra) in techs {
        research.push_str(&format!(
            "\n[[tech]]\nid = \"{id}\"\nname = \"{id}\"\ndescription = \"\"\ncost = 120\n{extra}\n"
        ));
    }
    files[3] = &research;
    Defs::from_files(files)
}

fn assert_err(r: Result<Defs, DefsError>, needle: &str) {
    let err = r.expect_err("should be rejected");
    assert!(err.0.contains(needle), "expected {needle:?} in: {err}");
}

/// The bundled research file with `from` replaced by `to` once.
fn edited(from: &str, to: &str) -> Result<Defs, DefsError> {
    let mut files = BUNDLED_FILES;
    assert!(files[3].contains(from), "{from}");
    let research = files[3].replacen(from, to, 1);
    files[3] = &research;
    Defs::from_files(files)
}

const TEACHABLE: [(&str, Nation, u32); 19] = [
    ("earthen_walls", Nation::Earth, 120),
    ("earth_rail", Nation::Earth, 250),
    ("omashu_chutes", Nation::Earth, 250),
    ("brickmaking", Nation::Fire, 120),
    ("smelting", Nation::Fire, 120),
    ("steam_engines", Nation::Fire, 250),
    ("hothouses", Nation::Fire, 250),
    ("cargo_balloons", Nation::Fire, 500),
    ("healing", Nation::Water, 120),
    ("tide_mills", Nation::Water, 250),
    ("water_travel", Nation::Water, 250),
    ("oil_lamps", Nation::Water, 120),
    ("orchards", Nation::Air, 25),
    ("wind_turbines", Nation::Air, 250),
    ("lemur_keeping", Nation::Air, 120),
    ("wind_gates", Nation::Air, 250),
    // The leaf bends (#969): a newcomer of the teaching nation casts them in the borrower's city.
    ("canals", Nation::Water, 250),
    ("plantbending", Nation::Water, 250),
    ("spirit_projection", Nation::Air, 250),
];

/// The teachable techs that unlock an ability, and so are leaves of the tree (#969).
const LEAF_BENDS: [&str; 3] = ["canals", "plantbending", "spirit_projection"];

#[test]
fn the_documented_techs_are_teachable_and_no_others() {
    let defs = Defs::bundled();
    for (id, nation, cost) in TEACHABLE {
        let t = defs.tech(defs.tech_id(id).unwrap());
        assert!(t.teachable, "{id}");
        assert_eq!((t.nation, t.cost), (Some(nation), cost), "{id}");
        assert!(t.school.is_none() && t.fusion.is_none(), "{id}");
        // Only the leaf bends unlock an ability, and of them `plantbending` and
        // `spirit_projection` are phase 5's teachings (borrowed without finding the scroll).
        assert_eq!(!t.unlocks.abilities.is_empty(), LEAF_BENDS.contains(&id), "{id}");
        assert_eq!(t.teaching.is_some(), id != "canals" && LEAF_BENDS.contains(&id), "{id}");
        assert!(t.cost_items.is_empty(), "{id}: a borrowed tech has no item bill");
        // Borrowed by every other nation, never by its own.
        for n in Nation::ALL {
            assert_eq!(t.borrowed_from(n), (n != nation).then_some(nation), "{id}");
            assert!(t.learnable_by(n), "{id}");
        }
    }
    let flagged: Vec<&str> =
        defs.techs.iter().filter(|t| t.teachable).map(|t| t.id.as_str()).collect();
    let mut want: Vec<&str> = TEACHABLE.iter().map(|t| t.0).collect();
    flagged.iter().for_each(|f| assert!(want.contains(f), "{f} is flagged but not documented"));
    want.sort_unstable();
    assert_eq!(flagged.len(), want.len());
    // The capstones, the other bends' techs and `herding` stay home.
    for kept in ["lightning_generation", "spirit_oasis", "sky_bison", "forging", "herding"] {
        let t = defs.tech(defs.tech_id(kept).unwrap());
        assert!(!t.teachable && t.borrowed_from(Nation::Water).is_none(), "{kept}");
    }
    // A foreign branch's other techs can't be learned at all.
    let forging = defs.tech(defs.tech_id("forging").unwrap());
    assert!(!forging.learnable_by(Nation::Water) && forging.learnable_by(Nation::Fire));
}

#[test]
fn the_settings_carry_the_provisional_borrow_numbers() {
    let defs = Defs::bundled();
    assert_eq!((defs.research.borrow_cost, defs.research.borrow_residents), (2.0, 3));
    let bad = |k: &str| edited("borrow_cost = 2.0 ", &format!("borrow_cost = {k} #"));
    assert_err(bad("0.5"), "borrow_cost must be 1 to");
    assert_err(bad("11.0"), "borrow_cost must be 1 to");
    assert!(bad("1.0").is_ok());
    assert_err(edited("borrow_residents = 3 ", "borrow_residents = 0 #"), "borrow_residents");
    assert_err(edited("borrow_residents = 3 ", "borrow_residents = 51 #"), "borrow_residents");
}

#[test]
fn teachable_needs_a_single_nation_and_no_school_or_fusion() {
    assert_err(
        with_techs(&[("t", "teachable = true")]),
        "`teachable` needs a single `nation`, not a shared tech",
    );
    assert_err(
        with_techs(&[("t", "teachable = true\nnations = ['earth', 'fire']")]),
        "can't be combined with `nations`",
    );
    // A teachable tech may unlock an ability (#969), but then nothing may require it: `forging`
    // unlocks Weld and `brickmaking` requires it, `irrigation` unlocks Draw Water and `canals`
    // requires it.
    assert_err(
        edited("id = \"forging\"", "id = \"forging\"\nteachable = true"),
        "is a teachable bend, which no other tech can require",
    );
    assert_err(
        edited("id = \"irrigation\"", "id = \"irrigation\"\nteachable = true"),
        "is a teachable bend, which no other tech can require",
    );
    assert_err(
        edited("id = \"herding\"", "id = \"herding\"\nteachable = true"),
        "can't be combined with `nations`",
    );
    assert_err(
        edited("id = \"high_temple\"", "id = \"high_temple\"\nteachable = true"),
        "can't be combined with `school`",
    );
    assert_err(
        edited("id = \"steelworks\"", "id = \"steelworks\"\nteachable = true"),
        "can't be combined with `teachable`",
    );
    assert_err(
        edited(
            "id = \"cargo_balloons\"",
            "id = \"cargo_balloons\"\ncost_items = [{ item = 'tool', count = 1 }]",
        ),
        "cost_items",
    );
    // The good case.
    with_techs(&[("t", "teachable = true\nnation = 'fire'")]).unwrap();
}

#[test]
fn a_teachable_tech_may_be_a_fusions_prereq_only_from_one_of_its_two_nations() {
    let fusion = |id: &str, pair: &str, req: &str| {
        let a_b = pair;
        (
            id.to_owned(),
            format!(
                "fusion = {a_b}\nprereqs = ['{req}']\n\
                 requires = {{ residents = {{ {} }}, masters = 1, masters_of = {a_b} }}",
                a_b.trim_matches(|c| c == '[' || c == ']')
                    .replace('\'', "")
                    .split(',')
                    .map(|n| format!("{} = 3", n.trim()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    };
    let try_it = |pair: &str, req: &str| {
        let (id, keys) = fusion("fused", pair, req);
        with_techs(&[(&id, &keys)])
    };
    // A Fire teachable tech for a Fire + Water fusion: the phase's `steam_power`.
    try_it("['fire', 'water']", "steam_engines").unwrap();
    // A Fire teachable tech for an Earth + Water fusion is neither shared nor of the pair.
    assert_err(try_it("['earth', 'water']", "steam_engines"), "can only require shared techs");
    // A non-teachable tech of the pair's nation, too.
    assert_err(try_it("['fire', 'water']", "forging"), "can only require shared techs");
    // Shared ones are still fine.
    try_it("['earth', 'water']", "masonry").unwrap();
}

#[test]
fn borrow_prereqs_keep_shared_and_teachable_prereqs_and_waive_the_rest() {
    let defs = Defs::bundled();
    let id = |s: &str| defs.tech_id(s).unwrap();
    let ids = |l: &[&str]| l.iter().map(|s| id(s)).collect::<Vec<_>>();
    // `forging` unlocks Weld: waived. `pottery` is shared.
    assert_eq!(defs.borrow_prereqs(id("brickmaking")), ids(&["pottery"]));
    assert_eq!(defs.borrow_prereqs(id("smelting")), ids(&[]));
    // Both teachable: borrowed first.
    assert_eq!(defs.borrow_prereqs(id("steam_engines")), ids(&["smelting", "brickmaking"]));
    assert_eq!(defs.borrow_prereqs(id("earthen_walls")), ids(&["masonry"]));
    assert_eq!(defs.borrow_prereqs(id("earth_rail")), ids(&["earthen_walls"]));
    assert_eq!(defs.borrow_prereqs(id("hothouses")), ids(&["brickmaking", "farming"]));
    assert_eq!(defs.borrow_prereqs(id("wind_gates")), ids(&[]));
    assert_eq!(defs.borrow_prereqs(id("orchards")), ids(&[]));
}

#[test]
fn only_a_borrowed_tech_is_left_out_of_the_home_nations_modifiers() {
    let defs = Defs::bundled();
    let brick = defs.tech(defs.tech_id("brickmaking").unwrap());
    let shared = defs.tech(defs.tech_id("carpentry").unwrap());
    let steel = defs.tech(defs.tech_id("steelworks").unwrap());
    // (unit's nation, home): a Fire tech reaches Fire units and a Fire home, nobody else.
    assert!(brick.modifiers_apply(Nation::Fire, Nation::Water));
    assert!(brick.modifiers_apply(Nation::Water, Nation::Fire));
    assert!(!brick.modifiers_apply(Nation::Water, Nation::Water));
    assert!(!brick.modifiers_apply(Nation::Earth, Nation::Water));
    for (u, h) in [(Nation::Earth, Nation::Water), (Nation::Water, Nation::Water)] {
        assert!(shared.modifiers_apply(u, h) && steel.modifiers_apply(u, h));
    }
}

#[test]
fn a_borrowed_tech_is_refused_before_the_gate_and_needs_its_residents_after_it() {
    let defs = Defs::bundled();
    let brick = defs.tech_id("brickmaking").unwrap();
    let mut r = Research::new(Nation::Water, &defs);
    let facts = |united, fire| Facts { united, residents: [0, 5, 0, fire], ..Facts::default() };
    assert_eq!(
        r.set_target(brick, Nation::Water, &facts(false, 4), &defs),
        Err(ResearchError::GateFirst)
    );
    let err = r.set_target(brick, Nation::Water, &facts(true, 2), &defs).unwrap_err();
    assert_eq!(err.to_string(), "needs 3 Fire residents (2 live here)");
    r.set_target(brick, Nation::Water, &facts(true, 3), &defs).unwrap();
    assert_eq!(r.queue().last(), Some(&brick));
    // A foreign tech that isn't teachable stays the wrong nation's, gate or no gate.
    let forging = defs.tech_id("forging").unwrap();
    assert_eq!(
        r.set_target(forging, Nation::Water, &facts(true, 9), &defs),
        Err(ResearchError::WrongNation)
    );
}

#[test]
fn a_requires_residents_on_a_teachable_tech_is_the_larger_of_the_two() {
    let defs = with_techs(&[(
        "extra",
        "teachable = true\nnation = 'fire'\nrequires = { residents = { fire = 5 } }",
    )])
    .unwrap();
    let t = defs.tech_id("extra").unwrap();
    let have = |n| Facts { residents: [0, 0, 0, n], ..Facts::default() };
    let unmet = |n| defs.tech(t).unmet(Nation::Water, &have(n), &defs);
    // Borrowing needs 3, the tech itself asks 5: one condition, the 5.
    assert_eq!(unmet(4), [Condition::Residents { nation: Nation::Fire, need: 5, have: 4 }]);
    assert!(unmet(5).is_empty());
    // Fire itself, researching its own tech, is asked the 5 too.
    assert_eq!(defs.tech(t).unmet(Nation::Fire, &have(4), &defs).len(), 1);
}

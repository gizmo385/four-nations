//! Fusion techs (`fusion = [a, b]`, docs/design/horizons-phase-6.md, "Fusions: the `fusion`
//! field", #754): what the data may say, and what the bundled `steelworks` is.

use super::*;
use crate::defs::{BUNDLED_FILES, DefsError};

/// A fusion's conditions as the design writes them, to build variants from.
const REQ: &str = "requires = { residents = { earth = 3, fire = 3 }, masters = 1, masters_of = ['earth', 'fire'] }";

/// The bundled files plus one extra tech, `fused`, with `extra` as its keys.
fn with_fused(extra: &str) -> Result<Defs, DefsError> {
    with_techs(&[("fused", extra)])
}

/// The bundled files plus the given extra techs, `(id, keys)`, in order.
fn with_techs(techs: &[(&str, &str)]) -> Result<Defs, DefsError> {
    let mut files = BUNDLED_FILES;
    let mut research = files[3].to_owned();
    for (id, extra) in techs {
        research.push_str(&format!(
            "\n[[tech]]\nid = \"{id}\"\nname = \"{id}\"\ndescription = \"\"\ncost = 1000\n{extra}\n"
        ));
    }
    files[3] = &research;
    Defs::from_files(files)
}

fn assert_err(r: Result<Defs, DefsError>, needle: &str) {
    let err = r.expect_err("should be rejected");
    assert!(err.0.contains(needle), "expected {needle:?} in: {err}");
}

#[test]
fn the_bundled_steelworks_is_the_earth_fire_fusion() {
    let defs = Defs::bundled();
    let id = defs.tech_id("steelworks").unwrap();
    let t = defs.tech(id);
    assert_eq!(t.fusion, Some([Nation::Earth, Nation::Fire]));
    assert!(t.is_fusion() && !t.core && !t.is_shared());
    assert_eq!((t.nation, t.nations.is_empty(), t.school), (None, true, None));
    assert_eq!(t.cost, 1000);
    assert_eq!(t.prereqs, [defs.tech_id("prospecting").unwrap()]);
    let tools = defs.items.iter().position(|i| i.id == "tool").unwrap();
    assert_eq!(
        t.cost_items.iter().map(|s| (s.item.0 as usize, s.count)).collect::<Vec<_>>(),
        [(tools, 8)]
    );
    assert_eq!(t.requires.residents.get(&Nation::Earth), Some(&3));
    assert_eq!(t.requires.residents.get(&Nation::Fire), Some(&3));
    assert_eq!(t.requires.masters, Some(1));
    assert_eq!(t.requires.masters_of.as_deref(), Some(&[Nation::Earth, Nation::Fire][..]));
    // Open to every nation, and to none of them before the gate.
    for nation in Nation::ALL {
        assert!(t.open_to(nation), "{nation:?}");
        assert!(defs.techs_for(nation).any(|(i, _)| i == id), "{nation:?}");
    }
    assert!(!t.visible_when(false) && t.visible_when(true));
    // No nation starts with it.
    for n in &defs.nations {
        assert!(!n.starting_techs.contains(&id), "{:?}", n.id);
    }
}

#[test]
fn the_steelworks_makes_one_metal_from_one_ore_and_two_wood() {
    let defs = Defs::bundled();
    let works = defs.building(defs.building_id("steelworks").unwrap());
    let tech = defs.tech_id("steelworks").unwrap();
    assert_eq!(works.unlocked_by, Some(tech));
    let r = works.recipe.as_ref().expect("a recipe building");
    let metal = defs.items.iter().position(|i| i.id == "metal").unwrap();
    assert_eq!((r.output.item.0 as usize, r.output.count), (metal, 1));
    let inputs: Vec<(&str, u32)> =
        r.inputs.iter().map(|s| (defs.items[s.item.0 as usize].id.as_str(), s.count)).collect();
    assert_eq!(inputs, [("gold_ore", 1), ("wood", 2)]);
    assert_eq!(r.stock_limit, 8);
    // The Smelter it improves on: two ore and one wood for the same metal.
    let smelter = defs.building(defs.building_id("smelter").unwrap());
    let ore = |b: &crate::building::BuildingDef| {
        b.recipe
            .as_ref()
            .unwrap()
            .inputs
            .iter()
            .find(|s| defs.items[s.item.0 as usize].id == "gold_ore")
            .unwrap()
            .count
    };
    assert_eq!(ore(smelter), 2 * ore(works));
}

#[test]
fn a_fusion_loads_with_one_or_both_nations_as_masters() {
    for of in ["['earth']", "['fire']", "['earth', 'fire']", "['fire', 'earth']"] {
        let extra = format!(
            "fusion = ['earth', 'fire']\nrequires = {{ residents = {{ earth = 3, fire = 3 }}, \
             masters = 1, masters_of = {of} }}"
        );
        with_fused(&extra).unwrap_or_else(|e| panic!("{of}: {e}"));
    }
    // The order of the pair is the file's, and the residents' order doesn't matter.
    let defs = with_fused(&format!("fusion = ['fire', 'earth']\n{REQ}")).unwrap();
    let t = defs.tech(defs.tech_id("fused").unwrap());
    assert_eq!(t.fusion, Some([Nation::Fire, Nation::Earth]));
    assert!(t.is_fusion() && !t.is_shared());
}

#[test]
fn a_fusion_lists_two_distinct_nations() {
    for (fusion, needle) in [
        ("['earth']", "exactly two"),
        ("[]", "exactly two"),
        ("['earth', 'fire', 'air']", "exactly two"),
        ("['earth', 'earth']", "earth twice"),
        ("['earth', 'mars']", "mars"),
    ] {
        assert_err(with_fused(&format!("fusion = {fusion}\n{REQ}")), needle);
    }
}

#[test]
fn a_fusion_excludes_nation_nations_school_teaching_and_core() {
    for (extra, needle) in [
        ("nation = 'earth'", "`nation`"),
        ("nations = ['earth', 'fire']", "`nations`"),
        ("school = 'any'", "`school`"),
        ("core = true", "`core`"),
        ("[tech.teaching]", "`teaching`"),
    ] {
        let src = format!("fusion = ['earth', 'fire']\n{REQ}\n{extra}");
        assert_err(with_fused(&src), needle);
    }
}

#[test]
fn a_fusions_conditions_name_exactly_its_two_nations() {
    let masters = "masters = 1, masters_of = ['earth', 'fire']";
    for (requires, needle) in [
        // Residents: missing, one, three, or the wrong pair.
        (format!("{{ {masters} }}"), "must name exactly earth and fire"),
        (
            format!("{{ residents = {{ earth = 3 }}, {masters} }}"),
            "must name exactly earth and fire",
        ),
        (
            format!("{{ residents = {{ earth = 3, fire = 3, air = 3 }}, {masters} }}"),
            "must name exactly earth and fire",
        ),
        (
            format!("{{ residents = {{ earth = 3, water = 3 }}, {masters} }}"),
            "must name exactly earth and fire",
        ),
        // Masters: absent, or of a nation outside the pair.
        ("{ residents = { earth = 3, fire = 3 } }".into(), "masters_of"),
        ("{ residents = { earth = 3, fire = 3 }, masters = 1 }".into(), "masters_of"),
        (
            "{ residents = { earth = 3, fire = 3 }, masters = 1, masters_of = ['earth', 'air'] }"
                .into(),
            "names air",
        ),
    ] {
        assert_err(
            with_fused(&format!("fusion = ['earth', 'fire']\nrequires = {requires}")),
            needle,
        );
    }
}

#[test]
fn a_fusion_requires_shared_techs_and_nothing_requires_a_fusion() {
    let fused = "fusion = ['earth', 'fire']";
    // Shared prereqs are fine.
    with_fused(&format!("{fused}\nprereqs = ['prospecting', 'farming']\n{REQ}")).unwrap();
    // A nation's own tech is not shared. (Teachable techs of the pair join in #753.)
    let own = defs_own_tech();
    assert_err(
        with_fused(&format!("{fused}\nprereqs = ['{own}']\n{REQ}")),
        "a fusion can only require shared techs",
    );
    // No tech, fusion or not, requires another fusion.
    let first = ("fused", format!("{fused}\n{REQ}"));
    for nation_keys in ["", "nation = 'earth'"] {
        let second = ("after", format!("prereqs = ['fused']\n{nation_keys}"));
        let r = with_techs(&[(first.0, &first.1), (second.0, &second.1)]);
        assert_err(r, "is a fusion, which no other tech can require");
    }
    let again = format!("{fused}\nprereqs = ['fused']\n{REQ}");
    assert_err(
        with_techs(&[(first.0, &first.1), ("again", &again)]),
        "a fusion can only require shared techs",
    );
}

/// The id of a bundled Fire-only tech with no school and no teaching.
fn defs_own_tech() -> String {
    let defs = Defs::bundled();
    let t = defs
        .techs
        .iter()
        .find(|t| t.nation == Some(Nation::Fire) && t.school.is_none() && t.teaching.is_none())
        .unwrap();
    t.id.clone()
}

#[test]
fn no_nation_may_start_with_a_fusion() {
    let mut files = BUNDLED_FILES;
    let nations = files[2].replacen("starting_techs = [", "starting_techs = [\"steelworks\", ", 1);
    assert_ne!(nations, files[2], "the fixture found a starting_techs list");
    files[2] = &nations;
    let e = Defs::from_files(files).expect_err("a starting fusion");
    assert!(e.0.contains("steelworks") && e.0.contains("fusion"), "{e}");
}

#[test]
fn a_fusion_is_refused_before_the_gate_and_otherwise_checked_like_any_tech() {
    let defs = Defs::bundled();
    let steel = defs.tech_id("steelworks").unwrap();
    let ready =
        Facts { residents: [0, 0, 3, 3], masters: [0, 0, 1, 0], population: 6, ..Facts::default() };
    let mut r = Research::new(Nation::Water, &defs);
    assert_eq!(r.set_target(steel, Nation::Water, &ready, &defs), Err(ResearchError::GateFirst));
    assert_eq!(ResearchError::GateFirst.to_string(), "a fusion waits for the Harmony Gate");
    assert!(r.queue().is_empty());
    // After the gate it is a Water city's to learn, once the Earth and Fire people live there...
    let open = Facts { united: true, ..ready.clone() };
    r.set_target(steel, Nation::Water, &open, &defs).unwrap();
    assert_eq!(r.queue().last(), Some(&steel));
    // ...and not before.
    let mut r = Research::new(Nation::Water, &defs);
    let few = Facts { residents: [0, 0, 3, 2], ..open.clone() };
    assert_eq!(
        r.set_target(steel, Nation::Water, &few, &defs),
        Err(ResearchError::NotYet(Condition::Residents { nation: Nation::Fire, need: 3, have: 2 }))
    );
}

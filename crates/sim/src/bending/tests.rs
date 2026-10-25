use super::BendingFile;
use super::testing::{BENDING, PROBE_TECH, load, load_only, probe_defs};
use crate::defs::{Defs, DefsError};
use crate::nation::Nation;
use crate::world::TICKS_PER_SECOND;

fn err_of(r: Result<Defs, DefsError>) -> String {
    r.expect_err("should be rejected").0
}

/// The bundled bending with its first occurrence of `from` replaced by `to`.
fn tweaked(from: &str, to: &str) -> Result<Defs, DefsError> {
    assert!(BENDING.contains(from), "{from}");
    load_only(&BENDING.replacen(from, to, 1))
}

const WIND: &str = "[ability.effect]\nkind='wind_carry'\nrange=3";

/// A fire ability called `x`, with `rest` as the remaining fields and tables.
fn ability(rest: &str) -> String {
    format!("[[ability]]\nid='x'\nnation='fire'\nname='X'\nverb='X'\nchi=10.0\nwork=1.0\n{rest}")
}

/// An earth ability called `probe` that needs no tech.
const PROBE: &str = "[[ability]]\nid='probe'\nnation='earth'\nname='P'\nverb='P'\nchi=10.0\n\
                     work=1.0\n[ability.effect]\nkind='probe'";

#[test]
fn bundled_abilities_load_one_signature_per_nation() {
    let defs = Defs::bundled();
    assert_eq!(defs.chi.max, 100.0);
    assert_eq!(defs.chi.regen_per_minute, 2.0);
    assert_eq!(defs.chi.sleep_regen_per_minute, 8.0);
    for n in Nation::ALL {
        // A signature ability needs no research, and each nation has exactly one. Others come
        // from the branch techs (Fire's Inner Fire, from Dancing Dragon).
        let signature: Vec<_> =
            defs.abilities.iter().filter(|a| a.nation == n && a.unlocked_by.is_none()).collect();
        assert_eq!(signature.len(), 1, "{n:?}");
    }
    let inner = defs.ability(defs.ability_id("inner_fire").unwrap());
    assert_eq!(inner.nation, Nation::Fire);
    assert_eq!(inner.unlocked_by, defs.tech_id("dancing_dragon"));
    assert_eq!(inner.magnitude_modifier.as_deref(), Some("heating"));
    let earth = defs.ability(defs.ability_id("raise_earth").unwrap());
    assert_eq!(earth.nation, Nation::Earth);
    assert_eq!((earth.chi, earth.work), (50.0, 15.0));
    assert_eq!(earth.speed_modifier.as_deref(), Some("earthworks"));
    assert!(defs.ability_id("nonsense").is_none());
}

#[test]
fn chi_regenerates_per_minute_and_is_clamped() {
    let defs = Defs::bundled();
    let ticks = 60 * TICKS_PER_SECOND;
    let minute =
        |sleeping, mult| (0..ticks).fold(0.0, |chi, _| defs.chi.regen(chi, sleeping, mult));
    assert!((minute(false, 1.0) - 2.0).abs() < 0.01);
    assert!((minute(true, 1.0) - 8.0).abs() < 0.01);
    assert!((minute(false, 1.25) - 2.5).abs() < 0.01);
    assert_eq!(defs.chi.regen(99.999, true, 1.0), 100.0);
}

#[test]
fn every_nation_starts_with_unit_chi_regen_and_meditation_speeds_it_up() {
    let defs = Defs::bundled();
    for n in Nation::ALL {
        assert_eq!(defs.nation(n).modifiers.chi_regen, 1.0, "{n:?}");
    }
    let meditation = defs.tech(defs.tech_id("meditation").unwrap());
    assert_eq!(meditation.unlocks.modifiers.chi_regen, Some(1.25));
}

#[test]
fn duplicate_ability_ids_are_rejected() {
    let dup = ability(WIND).replace("id='x'", "id='raise_earth'");
    assert!(err_of(load(&dup, "")).contains("duplicate ability raise_earth"));
}

#[test]
fn the_nation_must_exist() {
    let bad = ability(WIND).replace("fire", "mud");
    assert!(err_of(load(&bad, "")).contains("mud"));
}

#[test]
fn chi_costs_must_be_in_range() {
    let with_chi = |c: &str| load(&ability(WIND).replace("chi=10.0", &format!("chi={c}")), "");
    assert!(with_chi("100.0").is_ok());
    for bad in ["0.0", "-5.0", "100.5", "inf"] {
        assert!(err_of(with_chi(bad)).contains("chi must be in (0, chi.max]"), "{bad}");
    }
    assert!(err_of(tweaked("max = 100.0", "max = 0.0")).contains("chi.max"));
    let rate = tweaked("regen_per_minute = 2.0", "regen_per_minute = -1.0");
    assert!(err_of(rate).contains("regen_per_minute"));
}

#[test]
fn work_must_not_be_negative() {
    assert!(load(&ability(WIND).replace("work=1.0", "work=0.0"), "").is_ok());
    let bad = load(&ability(WIND).replace("work=1.0", "work=-1.0"), "");
    assert!(err_of(bad).contains("work must be"));
}

#[test]
fn modifier_names_must_resolve() {
    let ok = ability(&format!("speed_modifier='earthworks'\nmagnitude_modifier='heating'\n{WIND}"));
    assert!(load(&ok, "").is_ok());
    let bad = ability(&format!("speed_modifier='earthwork'\n{WIND}"));
    assert!(err_of(load(&bad, "")).contains("earthwork"));
    let bad = ability(&format!("magnitude_modifier='heatin'\n{WIND}"));
    assert!(err_of(load(&bad, "")).contains("heatin"));
}

#[test]
fn unknown_keys_and_kinds_are_rejected() {
    assert!(err_of(load(&ability(&format!("chii=1.0\n{WIND}")), "")).contains("chii"));
    assert!(err_of(load(&ability(&format!("{WIND}\nrang=1")), "")).contains("rang"));
    assert!(err_of(load(&ability("[ability.effect]\nkind='teleport'"), "")).contains("teleport"));
    assert!(err_of(tweaked("max = 100.0", "max = 100.0\nmaxx = 1.0")).contains("maxx"));
}

#[test]
fn effect_ids_and_numbers_are_validated() {
    let bad = |from: &str, to: &str| err_of(tweaked(from, to));
    assert!(bad("sprout_resource = \"cabbage\"", "sprout_resource = \"kale\"").contains("kale"));
    // A sprout resource needs a harvest entry (no resource lacks one in the bundled data, so
    // resolve against an empty list).
    let defs = Defs::bundled();
    let file: BendingFile = toml::from_str(BENDING).unwrap();
    let no_harvests = file.resolve(&defs.biomes, &defs.resources, &[], &defs.techs);
    assert!(no_harvests.unwrap_err().0.contains("no harvest entry"));
    assert!(bad("\"swamp\"]", "\"mangrove\"]").contains("mangrove"));
    assert!(bad("to = \"ford\"", "to = \"bridge\"").contains("bridge"));
    assert!(bad("from = [\"river\", \"lake\"]", "from = []").contains("at least one"));
    assert!(bad("radius = 3", "radius = 0").contains("radius"));
    assert!(bad("range = 8 ", "range = 0 ").contains("range"));
    assert!(bad("sleep_bonus = 1.5", "sleep_bonus = 0.5").contains("sleep_bonus"));
    assert!(bad("sprouts = 2", "sprouts = 0").contains("sprouts"));
    // Inner Fire's aura.
    assert!(bad("radius = 2 ", "radius = 0 ").contains("heat_aura"));
    assert!(bad("sleep_bonus = 1.25", "sleep_bonus = 0.5").contains("heat_aura"));
    assert!(bad("duration_minutes = 4.0", "duration_minutes = 0.0").contains("heat_aura"));
}

#[test]
fn a_tech_records_the_ability_it_unlocks() {
    let defs = probe_defs(1.0, "");
    let locked = defs.ability(defs.ability_id("probe_locked").unwrap());
    assert_eq!(locked.unlocked_by, defs.tech_id("probe_lore"));
    assert_eq!(defs.ability(defs.ability_id("probe").unwrap()).unlocked_by, None);
}

#[test]
fn a_tech_may_only_unlock_abilities_that_exist() {
    // `probe_locked` isn't defined in this tree, so the tech's reference dangles.
    assert!(err_of(load("", PROBE_TECH)).contains("probe_locked"));
}

#[test]
fn only_a_tech_of_the_abilitys_nation_may_unlock_it() {
    let tech = |extra: &str| {
        format!(
            "[[tech]]\nid='t'\nname='T'\ndescription=''\ncost=60\n{extra}\n\
             [tech.unlocks]\nabilities=['probe']"
        )
    };
    assert!(load(PROBE, &tech("nation='earth'")).is_ok());
    // A fire tech, and a shared tech (every nation could research it), are both refused.
    let wrong = err_of(load(PROBE, &tech("nation='fire'")));
    assert!(wrong.contains("only a earth tech"), "{wrong}");
    let shared = err_of(load(PROBE, &tech("")));
    assert!(shared.contains("only a earth tech"), "{shared}");
}

#[test]
fn an_ability_has_at_most_one_tech() {
    let tech = |id: &str| {
        format!(
            "[[tech]]\nid='{id}'\nname='T'\ndescription=''\ncost=60\nnation='earth'\n\
             [tech.unlocks]\nabilities=['probe']\n"
        )
    };
    let two = err_of(load(PROBE, &format!("{}{}", tech("a"), tech("b"))));
    assert!(two.contains("one tech"), "{two}");
    let twice = tech("a").replace("['probe']", "['probe','probe']");
    assert!(err_of(load(PROBE, &twice)).contains("listed twice"));
}

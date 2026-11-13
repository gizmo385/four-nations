//! Joint bends (#759): `nations` on an ability, Call Rain (the `rain` effect) and `cloudbending`.
//! A Water settlement with Air newcomers; Water units are `0..` and Air units follow them.

use super::*;
use crate::bending::testing::{load, load_only};
use crate::defs::BiomeId;
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn arena(size: i32) -> TileMap {
    Grid::from_fn(size, size, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn rain(defs: &Defs) -> AbilityId {
    defs.ability_id("call_rain").unwrap()
}

fn cloudbending(defs: &Defs) -> TechId {
    defs.tech_id("cloudbending").unwrap()
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for n in 0..limit {
        if done(w) {
            return n;
        }
        w.tick(defs);
        w.assert_invariants(defs);
    }
    panic!("not done after {limit} ticks");
}

/// Makes unit `i` an adept of its own nation: enough bends in enough forms.
fn make_adept(w: &mut World, defs: &Defs, i: usize) {
    let nation = w.nation_of(&w.units[i]);
    let bends = defs.mastery.adept_bends_for(nation);
    w.units[i].bends = bends;
    w.units[i].bends_by.clear();
    let forms: Vec<AbilityId> = defs
        .abilities
        .iter()
        .enumerate()
        .filter(|(_, a)| !a.is_joint() && a.nation == nation)
        .take(defs.mastery.adept_forms as usize)
        .map(|(n, _)| AbilityId(n as u16))
        .collect();
    for f in forms {
        w.units[i].bends_by.insert(f, 1);
    }
    assert_eq!(w.rank_of(&w.units[i], defs).rank, Rank::Adept);
}

/// A Water settlement that knows `cloudbending`, with `water` Water adepts at the west edge and
/// `air` Air adepts at the east edge, full of chi.
fn world(water: usize, air: usize, defs: &Defs) -> World {
    let spawns: Vec<Pos> = (0..water)
        .map(|n| at(1, 2 + n as i32))
        .chain((0..air).map(|n| at(23, 2 + n as i32)))
        .collect();
    let mut w = World::with_map(5, arena(25), Nation::Water, &spawns, defs);
    w.research.grant(cloudbending(defs));
    w.refresh_modifiers(defs);
    for i in 0..spawns.len() {
        if i >= water {
            w.units[i].nation = Some(Nation::Air);
        }
        make_adept(&mut w, defs, i);
        w.units[i].chi = defs.chi.max;
    }
    w
}

/// Puts a finished, empty farm plot on `pos`.
fn raise_farm_plot(w: &mut World, defs: &Defs, pos: Pos) {
    let id = defs.building_id("farm_plot").unwrap();
    w.put_building(Footprint::single(pos), id, defs);
}

#[test]
fn call_rain_is_a_joint_bend_of_the_cloudbending_fusion() {
    let defs = Defs::bundled();
    let def = defs.ability(rain(&defs));
    assert_eq!(def.nations, Some([Nation::Water, Nation::Air]));
    assert!(def.is_joint() && def.of_nation(Nation::Water) && def.of_nation(Nation::Air));
    assert!(!def.of_nation(Nation::Fire));
    assert_eq!(def.unlocked_by, Some(cloudbending(&defs)));
    assert_eq!(def.speed_modifier, None);
    assert_eq!(defs.tech(cloudbending(&defs)).fusion, Some([Nation::Water, Nation::Air]));
    // Every other ability is a plain single-nation one, and none counts Call Rain as its own.
    assert_eq!(defs.abilities.iter().filter(|a| a.is_joint()).count(), 1);
}

/// `nations` in the data: two distinct nations, instead of `nation`, with no `speed_modifier`,
/// and only on an ability that a fusion of the same two nations unlocks.
#[test]
fn nations_is_valid_only_when_a_matching_fusion_unlocks_it() {
    let joint = |extra: &str| {
        format!(
            "[[ability]]\nid='duet'\nnations={extra}\nname='D'\nverb='D'\nchi=10.0\nwork=1.0\n\
             [ability.effect]\nkind='probe'\n"
        )
    };
    let fusion = |pair: &str| {
        format!(
            "[[tech]]\nid='duet_tech'\nname='T'\ndescription=''\ncost=60\nfusion={pair}\n\
             requires = {{ residents = {{ water = 1, air = 1 }}, masters = 1, \
             masters_of = ['water'] }}\n[tech.unlocks]\nabilities=['duet']\n"
        )
    };
    let tried = |nations: &str, tech: &str| match load(&joint(nations), tech) {
        Ok(_) => String::new(),
        Err(e) => e.to_string(),
    };
    assert_eq!(tried("['water','air']", &fusion("['water','air']")), "");
    // Either order of the fusion's pair is the same pair.
    assert_eq!(tried("['air','water']", &fusion("['water','air']")), "");
    // No fusion at all, or one of another pair, or a plain nation tech.
    assert!(tried("['water','air']", "").contains("must be unlocked by a fusion"));
    let other = fusion("['water','fire']").replace("air = 1", "fire = 1");
    assert!(tried("['water','air']", &other).contains("joint bend"), "other pair");
    let plain = "[[tech]]\nid='p'\nname='T'\ndescription=''\ncost=60\nnation='water'\n\
                 [tech.unlocks]\nabilities=['duet']\n";
    assert!(tried("['water','air']", plain).contains("joint bend"), "a nation's own tech");
    // Two distinct nations, exactly.
    assert!(tried("['water','water']", &fusion("['water','air']")).contains("twice"));
    assert!(tried("['water']", &fusion("['water','air']")).contains("exactly two"));
    assert!(tried("['water','air','fire']", &fusion("['water','air']")).contains("exactly two"));
    // Not both `nation` and `nations`, and no speed_modifier.
    let both = joint("['water','air']").replace("nations=", "nation='water'\nnations=");
    assert!(
        load(&both, &fusion("['water','air']")).unwrap_err().to_string().contains("exactly one")
    );
    let speedy = joint("['water','air']").replace("chi=", "speed_modifier='water_work'\nchi=");
    assert!(
        load(&speedy, &fusion("['water','air']"))
            .unwrap_err()
            .to_string()
            .contains("no speed_modifier")
    );
    // A plain ability can't be unlocked by a fusion.
    let plain_ability =
        joint("['water','air']").replace("nations=['water','air']", "nation='water'");
    assert!(load(&plain_ability, &fusion("['water','air']")).is_err());
}

#[test]
fn the_rain_effect_checks_its_numbers() {
    let tweak = |from: &str, to: &str| {
        let src = include_str!("../../../../../data/bending.toml");
        assert!(src.contains(from), "{from}");
        src.replacen(from, to, 1)
    };
    let bad = |from: &str, to: &str| load_only(&tweak(from, to)).expect_err(to).to_string();
    assert!(bad("radius = 6", "radius = 0").contains("rain radius"));
    assert!(
        bad("growth_boost = 1.5\nminutes = 15.0", "growth_boost = 0.5\nminutes = 15.0")
            .contains("growth_boost")
    );
    assert!(bad("minutes = 15.0\n", "minutes = 0.0\n").contains("minutes"));
}

#[test]
fn it_is_locked_until_cloudbending_and_open_to_a_settlement_of_either_nation() {
    let defs = Defs::bundled();
    let mut w = world(1, 1, &defs);
    w.research = crate::research::Research::new(Nation::Water, &defs);
    let tech = cloudbending(&defs);
    // A fusion is out of sight until the Harmony Gate stands, so the bend isn't even offered.
    assert_eq!(w.check_ability(rain(&defs), &defs), Err(BendError::WrongNation));
    w.endeavor = EndeavorState::Complete { tick: 0 };
    assert_eq!(w.check_ability(rain(&defs), &defs), Err(BendError::Locked { tech }));
    w.research.grant(tech);
    assert_eq!(w.check_ability(rain(&defs), &defs), Ok(()));
    // The fusion is open to every nation: an Earth settlement with Water and Air residents too.
    let mut e = World::with_map(5, arena(25), Nation::Earth, &[at(1, 1), at(2, 1)], &defs);
    e.units[0].nation = Some(Nation::Water);
    e.units[1].nation = Some(Nation::Air);
    e.research.grant(tech);
    assert_eq!(e.check_ability(rain(&defs), &defs), Ok(()));
    assert!(
        e.can_cast(&e.units[0], rain(&defs), &defs)
            || e.rank_of(&e.units[0], &defs).rank < Rank::Adept
    );
    // Nobody of either nation lives in a Fire city: not offered there.
    let fire = World::with_map(5, arena(25), Nation::Fire, &[at(1, 1)], &defs);
    assert_eq!(fire.check_ability(rain(&defs), &defs), Err(BendError::WrongNation));
    // An Earth unit is of neither nation.
    e.units[1].nation = None;
    assert!(!e.can_cast(&e.units[1], rain(&defs), &defs));
}

#[test]
fn only_an_adept_of_either_nation_can_cast_it() {
    let defs = Defs::bundled();
    let mut w = world(2, 2, &defs);
    let a = rain(&defs);
    assert_eq!(w.joint_caster_counts(a, &defs), Some([(Nation::Water, 2), (Nation::Air, 2)]));
    w.units[1].bends = 0;
    w.units[1].bends_by.clear();
    w.units[3].bends = 0;
    w.units[3].bends_by.clear();
    assert_eq!(w.joint_caster_counts(a, &defs), Some([(Nation::Water, 1), (Nation::Air, 1)]));
    assert_eq!(w.joint_caster_counts(defs.ability_id("hearthflame").unwrap(), &defs), None);
}

/// Why the scripted United City never casts Call Rain (#958). A newcomer casts only its own
/// nation's signature ability (`can_cast`: a branch ability needs the settlement's own nation),
/// and each nation has one, so a newcomer has one form however many bends it finishes, while an
/// adept needs `adept_forms`. A joint bend wants an adept of each of its two nations, so a city
/// has one slot it can never fill: its own nation fills one, and the other is always a newcomer.
/// If a decision gives newcomers a second form (or relaxes the rank), this test changes with it.
#[test]
fn a_newcomer_has_one_form_so_it_never_ranks_up_to_call_rain() {
    let defs = Defs::bundled();
    let spawns = [at(1, 2), at(2, 2)];
    let mut w = World::with_map(5, arena(25), Nation::Fire, &spawns, &defs);
    for t in 0..defs.techs.len() {
        w.research.grant(TechId(t as u16));
    }
    w.refresh_modifiers(&defs);
    w.units[0].nation = Some(Nation::Water);
    w.units[1].nation = Some(Nation::Air);
    for i in 0..2 {
        let forms: Vec<AbilityId> = (0..defs.abilities.len() as u16)
            .map(AbilityId)
            .filter(|&a| !defs.ability(a).is_joint() && w.can_cast(&w.units[i], a, &defs))
            .collect();
        assert_eq!(forms.len(), 1, "{:?}: only its signature", w.nation_of(&w.units[i]));
        assert!(defs.mastery.adept_forms as usize > forms.len());
        w.units[i].bends = 1000;
        w.units[i].bends_by = forms.into_iter().map(|a| (a, 1000)).collect();
        assert_eq!(w.rank_of(&w.units[i], &defs).rank, Rank::Novice);
        assert!(!w.can_cast(&w.units[i], rain(&defs), &defs));
    }
}

/// A 13x13 patch of plots needs one plot in it; the target is walkable ground.
#[test]
fn it_needs_a_farm_plot_in_the_patch() {
    let defs = Defs::bundled();
    let mut w = world(1, 1, &defs);
    let bend =
        |w: &mut World| w.apply(Command::Bend { ability: rain(&defs), pos: at(12, 12) }, &defs);
    assert_eq!(
        bend(&mut w),
        Err(CommandError::Bend(BendError::InvalidTarget("no farm plots in reach")))
    );
    raise_farm_plot(&mut w, &defs, at(18, 12)); // Six tiles away: the patch's edge.
    assert_eq!(bend(&mut w), Ok(()));
    let mut far = world(1, 1, &defs);
    raise_farm_plot(&mut far, &defs, at(19, 12)); // Seven: just outside.
    assert!(bend(&mut far).is_err());
}

/// The slots are claimed separately, by the nearest able unit of each nation; progress holds until
/// both are at the target; each pays the chi and gets a bend and a form.
#[test]
fn two_casters_one_of_each_nation_channel_it_together() {
    let defs = Defs::bundled();
    let mut w = world(2, 2, &defs);
    let (plot, target) = (at(12, 12), at(12, 12));
    raise_farm_plot(&mut w, &defs, plot);
    w.apply(Command::Bend { ability: rain(&defs), pos: target }, &defs).unwrap();
    assert_eq!(w.joint_missing(target, &defs), Some(Nation::Water), "no one has claimed yet");

    let def = defs.ability(rain(&defs));
    let before: Vec<f32> = w.units.iter().map(|u| u.chi).collect();
    let base: Vec<(u32, u32)> = w.units.iter().map(|u| (u.bends, u.forms())).collect();
    run_until(&mut w, &defs, 400, |w| {
        w.jobs()[0].claimed_by.is_some() && w.jobs()[0].partner.is_some()
    });
    let job = &w.jobs()[0];
    let (water, air) = (job.claimed_by.unwrap(), job.partner.unwrap());
    let (wi, ai) = (
        w.units.iter().position(|u| u.id == water).unwrap(),
        w.units.iter().position(|u| u.id == air).unwrap(),
    );
    assert_eq!(w.nation_of(&w.units[wi]), Nation::Water);
    assert_eq!(w.nation_of(&w.units[ai]), Nation::Air, "the second slot is the second nation's");
    assert_eq!(w.joint_missing(target, &defs), None);
    assert!(w.bend_channels(&defs).is_empty(), "walking isn't channelling");

    // The Water caster (1 tile from the west edge) arrives long before the Air caster does:
    // nothing is channelled and nothing progresses until both are there.
    run_until(&mut w, &defs, 2000, |w| w.units[wi].pos == target);
    if w.units[ai].pos != target {
        assert_eq!(w.jobs()[0].progress, 0.0, "the Air caster is still on its way");
        assert!(w.bend_channels(&defs).is_empty());
    }
    run_until(&mut w, &defs, 2000, |w| w.bend_events().next().is_some());
    let event = w.bend_events().next().unwrap();
    assert_eq!(event.outcome, BendOutcome::Done);
    assert!(w.jobs().is_empty());
    assert_eq!(w.rains().count(), 1);

    // Both paid the full chi (regen aside), and each has a finished bend and a form.
    for (i, u) in w.units.iter().enumerate() {
        if i == wi || i == ai {
            assert!(u.chi < before[i] - def.chi + 5.0, "{i}: {} of {}", u.chi, before[i]);
            assert_eq!(u.bends_of(rain(&defs)), 1, "{i}");
            assert_eq!(u.bends, base[i].0 + 1, "{i}");
            assert_eq!(u.forms(), base[i].1 + 1, "{i}");
            assert!(u.job_task.is_none() && u.need_task.is_none() || u.need_task.is_some());
        } else {
            assert_eq!(u.bends_of(rain(&defs)), 0, "{i}");
        }
    }
}

/// Without an able Air bender the designation waits and says so; one arriving later finishes it.
#[test]
fn without_an_able_air_bender_it_waits_and_says_who_is_missing() {
    let defs = Defs::bundled();
    let mut w = world(1, 1, &defs);
    raise_farm_plot(&mut w, &defs, at(12, 12));
    w.units[1].chi = 0.0; // The only Air adept has no chi.
    w.units[1].bends = 0; // And is not an adept anymore.
    w.units[1].bends_by.clear();
    w.apply(Command::Bend { ability: rain(&defs), pos: at(12, 12) }, &defs).unwrap();
    run_until(&mut w, &defs, 200, |w| w.jobs()[0].claimed_by.is_some());
    for _ in 0..3000 {
        w.tick(&defs);
    }
    assert_eq!(w.joint_missing(at(12, 12), &defs), Some(Nation::Air));
    assert_eq!(w.jobs()[0].progress, 0.0, "the Water caster waits at the target");
    assert!(w.bend_channels(&defs).is_empty());
    assert!(w.bend_events().next().is_none() && w.rains().count() == 0);
    // The Air bender becomes an adept with chi to spare: it takes the open slot and the bend runs.
    make_adept(&mut w, &defs, 1);
    w.units[1].chi = defs.chi.max;
    w.wake_idle();
    run_until(&mut w, &defs, 6000, |w| w.bend_events().next().is_some());
    assert_eq!(w.bend_events().next().unwrap().outcome, BendOutcome::Done);
}

/// If one caster leaves for a need, progress holds and its slot opens for another unit of its
/// nation, which finishes the bend.
#[test]
fn a_caster_who_leaves_for_a_need_gives_up_the_slot_and_progress_holds() {
    let defs = Defs::bundled();
    let mut w = world(1, 2, &defs);
    raise_farm_plot(&mut w, &defs, at(12, 12));
    w.apply(Command::Bend { ability: rain(&defs), pos: at(12, 12) }, &defs).unwrap();
    run_until(&mut w, &defs, 6000, |w| w.jobs().first().is_some_and(|j| j.progress > 3.0));
    let held = w.jobs()[0].progress;
    let air = w.jobs()[0].partner.expect("an Air caster is channelling");
    let air_i = w.units.iter().position(|u| u.id == air).unwrap();
    // Exhaustion pulls the Air caster away from the channel.
    w.units[air_i].needs.energy = 0.0;
    run_until(&mut w, &defs, 400, |w| w.jobs()[0].partner != Some(air));
    assert!(w.jobs()[0].progress >= held, "progress holds");
    let stalled = w.jobs()[0].progress;
    w.tick(&defs);
    assert!(w.jobs()[0].progress <= stalled + 1e-3 || w.jobs()[0].partner.is_some());
    // The other Air adept takes the slot; the bend completes.
    run_until(&mut w, &defs, 9000, |w| w.bend_events().next().is_some());
    assert_eq!(w.bend_events().next().unwrap().outcome, BendOutcome::Done);
    assert_eq!(w.rains().count(), 1);
}

/// Designating more joint bends doesn't queue more waiting jobs than there are pairs of casters.
#[test]
fn it_queues_no_more_waiting_jobs_than_there_are_pairs() {
    let defs = Defs::bundled();
    let mut w = world(2, 1, &defs);
    raise_farm_plot(&mut w, &defs, at(12, 12));
    let bend =
        |w: &mut World, x| w.apply(Command::Bend { ability: rain(&defs), pos: at(x, 12) }, &defs);
    assert_eq!(bend(&mut w, 12), Ok(()));
    assert_eq!(
        bend(&mut w, 13),
        Err(CommandError::Bend(BendError::InvalidTarget("no more pairs of casters to wait on")))
    );
    // With no pair at all, one still waits (it says who is missing).
    let mut none = world(1, 0, &defs);
    raise_farm_plot(&mut none, &defs, at(12, 12));
    assert_eq!(bend(&mut none, 12), Ok(()));
    assert_eq!(none.joint_missing(at(12, 12), &defs), Some(Nation::Water));
    assert!(bend(&mut none, 13).is_err());
    // Cancelling frees the room.
    assert_eq!(none.apply(Command::CancelBend { pos: at(12, 12) }, &defs), Ok(()));
    assert_eq!(bend(&mut none, 13), Ok(()));
}

/// Plots in the patch grow at the rain's boost until it ends; winter and distance are unchanged.
#[test]
fn rain_speeds_the_plots_under_it_and_ends() {
    let defs = Defs::bundled();
    let mut w = world(1, 1, &defs);
    let (inside, edge, outside) = (at(12, 12), at(18, 12), at(19, 12));
    for p in [inside, edge, outside] {
        raise_farm_plot(&mut w, &defs, p);
    }
    let boost = match defs.ability(rain(&defs)).effect {
        Effect::Rain { growth_boost, .. } => growth_boost,
        _ => unreachable!(),
    };
    assert_eq!(w.irrigation_rate(inside, &defs), 1.0);
    w.rains.push(Rain { center: at(12, 12), radius: 6, boost, until_tick: w.tick + 100 });
    assert_eq!(w.irrigation_rate(inside, &defs), boost);
    assert_eq!(w.irrigation_rate(edge, &defs), boost, "Chebyshev 6 is in");
    assert_eq!(w.irrigation_rate(outside, &defs), 1.0, "Chebyshev 7 is out");
    // It ends, and is dropped.
    for _ in 0..100 {
        w.tick(&defs);
    }
    assert_eq!(w.irrigation_rate(inside, &defs), 1.0);
    assert_eq!(w.rains().count(), 0);
    assert!(w.rains.is_empty(), "the ended rain is dropped");
}

/// The best boost in reach applies: rain never stacks with a channel, a canal or other rain, and
/// a second rain over the same patch replaces the first.
#[test]
fn rain_does_not_stack_and_a_new_rain_replaces_the_old_over_the_same_patch() {
    let defs = Defs::bundled();
    let mut w = world(1, 1, &defs);
    let plot = at(12, 12);
    raise_farm_plot(&mut w, &defs, plot);
    let channel = defs.building_id("irrigation_channel").unwrap();
    w.put_building(Footprint::single(at(12, 13)), channel, &defs);
    let channel_boost = defs.building(channel).irrigation.unwrap().growth_boost;
    // A weaker rain than the channel: the channel's boost wins, not their product.
    w.rains.push(Rain { center: plot, radius: 6, boost: 1.25, until_tick: 1_000_000 });
    assert_eq!(w.irrigation_rate(plot, &defs), channel_boost.max(1.25));
    // Two overlapping rains: the better one, not the sum.
    w.rains.push(Rain { center: at(10, 10), radius: 6, boost: 3.0, until_tick: 1_000_000 });
    assert_eq!(w.irrigation_rate(plot, &defs), 3.0);

    // Casting over the same patch replaces: one rain at that centre afterwards.
    let mut w = world(1, 1, &defs);
    raise_farm_plot(&mut w, &defs, plot);
    for _ in 0..2 {
        w.apply(Command::Bend { ability: rain(&defs), pos: plot }, &defs).unwrap();
        run_until(&mut w, &defs, 9000, |w| w.jobs().is_empty());
        for u in &mut w.units {
            u.chi = defs.chi.max;
        }
    }
    assert_eq!(w.rains().count(), 1, "replaced, not stacked");
    let r = w.rains().next().unwrap();
    assert_eq!((r.center, r.radius), (plot, 6));
}

#[test]
fn a_ripening_plot_under_rain_grows_faster() {
    let defs = Defs::bundled();
    let grow = |rained: bool| {
        let mut w = world(1, 1, &defs);
        let plot = at(12, 12);
        raise_farm_plot(&mut w, &defs, plot);
        if rained {
            w.rains.push(Rain { center: plot, radius: 6, boost: 1.5, until_tick: u64::MAX });
        }
        w.apply(Command::Sow { pos: plot }, &defs).unwrap();
        let start = w.tick;
        run_until(&mut w, &defs, 40_000, |w| {
            w.plot_stage(plot) == Some(crate::world::PlotStage::Ripe)
        });
        w.tick - start
    };
    let (plain, wet) = (grow(false), grow(true));
    assert!(wet < plain, "{wet} vs {plain}");
}

/// A save mid-rain keeps the rain; one from before #759 loads with none.
#[test]
fn rains_survive_a_save_and_old_saves_have_none() {
    let defs = Defs::bundled();
    let mut w = world(1, 1, &defs);
    w.rains.push(Rain { center: at(12, 12), radius: 6, boost: 1.5, until_tick: w.tick + 5000 });
    let text = ron::to_string(&w).unwrap();
    let back: World = ron::from_str(&text).unwrap();
    assert_eq!(back.rains().collect::<Vec<_>>(), w.rains().collect::<Vec<_>>());
    let mut plain = world(1, 1, &defs);
    plain.rains.clear();
    let text = ron::to_string(&plain).unwrap();
    assert!(!text.contains("rains"), "an empty list isn't written");
    let back: World = ron::from_str(&text).unwrap();
    assert_eq!(back.rains().count(), 0);
}

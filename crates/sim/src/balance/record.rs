//! One run, recorded whole (`docs/design/balance-harness.md`, sections 3 and 8; #534): [`run`]
//! plays a [`RunSpec`] and returns a [`RunRecord`] with everything the run knows about itself, the
//! numbers the old `run_economy` report carried plus the site profile, the care tally, the run's
//! health flags and an autopsy for every death. The `balance` binary and the `fn_harness` crate
//! (#537) read records; nothing here prints, reads a file or spawns a thread.
//!
//! A record is a pure function of its spec and `Defs`: the same spec always gives an equal one,
//! because the world draws only seeded randomness, the player reads only the world and the probe
//! only reads it.

use std::collections::BTreeSet;

use crate::autoplay::{self, Economy, Opening, Player};
use crate::building::BuildingId;
use crate::defs::Defs;
use crate::embark::Survey;
use crate::endeavor;
use crate::jobs::ItemId;
use crate::mapgen::MapGenConfig;
use crate::nation::Nation;
use crate::world::{
    CareNeed, EndeavorEventKind, Event, SeasonEvent, SeasonEventKind, Severe, World,
};

use super::autopsy::Autopsy;
use super::probe::{Detectors, Probe, Raised, Sample, Watch};
use super::site::SiteProfile;
use super::{
    DecisionEvent, EconomyReport, EconomySeason, IDLE_WINDOW_MINUTES, PEOPLE, decision_events,
    decision_gaps, economy_season, minutes_to_ticks, ready_minute, sample_idle_share,
    ticks_to_minutes,
};

/// What one run varies: the start, who plays and how closely the run is watched.
#[derive(Debug, Clone)]
pub struct RunSpec {
    pub seed: u64,
    pub nation: Nation,
    /// A `data/difficulty.toml` preset id, or `None` for identity (`World::found`'s meaning).
    pub preset: Option<String>,
    pub minutes: f32,
    pub width: i32,
    pub height: i32,
    /// Who plays: nobody, or the economy opening with a policy for each knob (the play style,
    /// `docs/design/balance-harness.md`, section 2; `fn_harness` maps its `data/harness.toml`
    /// styles onto this).
    pub player: Player,
    /// The health probe's thresholds.
    pub detectors: Detectors,
    /// Tick the player's steps run late by: 0 is the opening's own grid (`Opening::due`), and 1
    /// makes every decision one tick later. Nothing else about the run changes (the probe still
    /// reads on the original grid), so the difference between a run and its offset twin is the
    /// chaos floor of the sim, not a balance change (the jitter run, `balance jitter`, #538).
    pub step_offset: u32,
    /// The minute the gate is stood by `World::complete_endeavor` (#751), unless the opening has
    /// finished it by then: a United City to play in. The economy opening does not finish the gate
    /// inside 420 minutes on its own, so a `united` run plays the opening to this minute and
    /// continues from a standing gate. `None`: nothing is stood, the gate is whatever the opening
    /// builds.
    pub gate_at: Option<f32>,
}

impl RunSpec {
    /// The reference run for `seed` and `nation`: the small map, Normal, the economy opening in
    /// its default style, the doc's detector thresholds.
    pub fn new(seed: u64, nation: Nation, minutes: f32, defs: &Defs) -> Self {
        let size = defs.embark.map_size("small").expect("bundled data always has a small map size");
        Self {
            seed,
            nation,
            preset: Some("normal".to_owned()),
            minutes,
            width: size.width,
            height: size.height,
            player: Player::Economy(Economy::new(PEOPLE)),
            detectors: Detectors::default(),
            step_offset: 0,
            gate_at: None,
        }
    }
}

/// The care tally (`World::care()`) in minutes, the way a consumer reads it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CareSummary {
    /// Minutes lived, summed over settlers: the denominator of every rate.
    pub person_minutes: f64,
    /// Person-minutes with hunger, thirst and energy at 0, in that order.
    pub depleted_minutes: [f64; 3],
    /// Person-minutes with each severe flag (Starving, Parched, Exhausted, Dying), a breakdown
    /// rather than a total.
    pub severe_minutes: [f64; 4],
    /// Person-minutes with at least one severe flag.
    pub severe_any_minutes: f64,
    /// Person-minutes asleep without shelter in the cold, with exposure applying.
    pub cold_minutes: f64,
    pub close_calls: u32,
    pub lowest_health: Option<f32>,
}

impl CareSummary {
    pub fn of(world: &World) -> Self {
        let care = world.care();
        Self {
            person_minutes: care.person_minutes(),
            depleted_minutes: CareNeed::ALL.map(|n| care.depleted_minutes(n)),
            severe_minutes: Severe::ALL.map(|s| care.severe_minutes(s)),
            severe_any_minutes: care.severe_any_minutes(),
            cold_minutes: care.cold_minutes(),
            close_calls: care.close_calls(),
            lowest_health: care.lowest_health(),
        }
    }
}

/// Everything one run knows about itself (module doc).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RunRecord {
    pub seed: u64,
    pub nation: Nation,
    pub preset: Option<String>,
    pub minutes: f32,
    /// The player, as `Debug` prints it: for a human reading the JSON, not a key.
    pub style: String,
    pub site: SiteProfile,
    /// One entry at tick 0, plus one per season boundary reached (`EconomyReport::seasons`).
    pub seasons: Vec<EconomySeason>,
    /// Every sign in `data/endeavor.toml`'s order with the minute it was first met.
    pub sign_minutes: Vec<(String, Option<f32>)>,
    /// Every decision event in minute order (`EconomyReport::timeline`).
    pub timeline: Vec<DecisionEvent>,
    pub first_built: Vec<(String, Option<f32>)>,
    pub recipe_stores: Vec<(String, u32)>,
    pub techs: Vec<(String, f32)>,
    pub longest_gap: Option<(f32, f32)>,
    pub gaps_over_15: u32,
    pub idle_share: Vec<(f32, f32)>,
    pub care: CareSummary,
    pub first_death_minute: Option<f32>,
    pub deaths: u32,
    /// How many deals and requests were completed, by what they were paid in (a one-item `give`'s
    /// item id; "food" for the any-food side), in item order: `wares` and `scroll_copy` say
    /// whether the opening trades what phase 3 makes (#713). Absent from an older record.
    #[serde(default)]
    pub paid_in: Vec<(String, u32)>,
    /// Living settlers when the run ended.
    pub population: u32,
    /// An autopsy per death, oldest first.
    pub autopsies: Vec<Autopsy>,
    /// Every health flag raised, in `Flag::ALL`'s order, with the minute each first was.
    pub flags: Vec<Raised>,
    /// Every endeavor stage in `data/endeavor.toml`'s order with the minute it was finished
    /// (`stage_minute.N`, docs/design/horizons-phase-4.md "Measuring it"). Absent from an older
    /// record.
    #[serde(default)]
    pub stage_minutes: Vec<(String, Option<f32>)>,
    /// The minute the gate stood and the endeavor was Complete (`gate_minute`, #751): the
    /// opening finishing the last stage, or `RunSpec::gate_at`. `None` if it never stood. Absent
    /// from an older record.
    #[serde(default)]
    pub gate_minute: Option<f32>,
    /// Whether the gate was stood by `RunSpec::gate_at`'s hook and not built (`gate_minute` is
    /// then that minute). Absent from an older record.
    #[serde(default)]
    pub gate_stood: bool,
    /// What each school did (`SCHOOL_TALLIES`'s order): absent from an older record.
    #[serde(default)]
    pub school_tallies: Vec<(String, u32)>,
    /// The lost teachings (docs/design/horizons-phase-5.md, "Measuring it", #745): absent from
    /// an older record.
    #[serde(default)]
    pub teachings: TeachingRecord,
}

/// What a run did with phase 5's great ruins and teaching drafts (#745), read off the world's
/// event log at the end of the run.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TeachingRecord {
    /// The minute each great ruin was found, earliest first: spotted by a reveal bend, charted
    /// by a Map Room, or (at the latest) searched. `great_ruins_found_by.M`.
    pub great_ruins_found: Vec<f32>,
    /// The minute each draft was offered, earliest first. `first_offer_minute`.
    pub offers: Vec<f32>,
    /// Each teaching kept, with the minute. `teachings_kept_by.M`.
    pub kept: Vec<(String, f32)>,
    /// Teaching scrolls read with nothing left to offer. `empty_scrolls`.
    pub empty_scrolls: u32,
}

/// [`TeachingRecord`] for a finished run.
fn teaching_record(world: &World, defs: &Defs) -> TeachingRecord {
    use std::collections::BTreeMap;

    use crate::geom::Pos;
    use crate::world::{ChartEventKind, TeachingEventKind};

    let mut out = TeachingRecord::default();
    let Some(great) = defs.resource_id("great_ruins") else { return out };
    // Every great ruin the run could have found: still on the map, or searched.
    let mut found: BTreeMap<Pos, Option<u64>> = world
        .map
        .iter()
        .filter(|(_, t)| t.resource == Some(great))
        .map(|(p, _)| (p, None))
        .collect();
    let earliest = |found: &mut BTreeMap<Pos, Option<u64>>, pos: Pos, tick: u64| {
        if let Some(at) = found.get_mut(&pos) {
            *at = Some(at.map_or(tick, |t| t.min(tick)));
        }
    };
    for event in world.events() {
        if let Event::Teaching(e) = event {
            match &e.kind {
                TeachingEventKind::Searched { at, resource } if *resource == great => {
                    found.insert(*at, Some(e.tick));
                }
                TeachingEventKind::Offered { .. } => out.offers.push(ticks_to_minutes(e.tick)),
                TeachingEventKind::Kept { kept, .. } => {
                    out.kept.push((defs.tech(*kept).id.clone(), ticks_to_minutes(e.tick)));
                }
                TeachingEventKind::Empty { .. } => out.empty_scrolls += 1,
                _ => {}
            }
        }
    }
    for event in world.events() {
        match event {
            Event::Find(e) if e.resource == great => earliest(&mut found, e.pos, e.tick),
            Event::Chart(e) => {
                if let ChartEventKind::Found { pos, .. } = e.kind {
                    earliest(&mut found, pos, e.tick);
                }
            }
            _ => {}
        }
    }
    // A ruin on the map with no find logged was in sight from the start.
    out.great_ruins_found = found.values().map(|t| t.map_or(0.0, ticks_to_minutes)).collect();
    out.great_ruins_found.sort_by(f32::total_cmp);
    out
}

/// The per-school tallies a run reports (docs/design/horizons-phase-4.md, "Measuring it"), one per
/// school option: tiles inside a closed ring at the end, buildings powered along rail at the end,
/// settler-minutes in a sun-warmed fire's aura, batches a yard finished alone, ice delivered for
/// stone, shore catches, scholar-minutes seated under a study aura, and items packed-up buildings
/// gave back.
pub const SCHOOL_TALLIES: [&str; 8] = [
    "sheltered_tiles",
    "rail_powered",
    "aura_heal",
    "automated_batches",
    "ice_paid",
    "shore_fish",
    "temple_study",
    "packed_refund",
];

/// The sampled school tallies of one probe minute: settlers inside a heat aura (`aura_heal`) and
/// scholars seated under a study aura (`temple_study`).
fn sample_school(world: &World, defs: &Defs) -> (u32, u32) {
    use crate::geom::Footprint;
    let auras = world.auras(defs);
    let fires: Vec<_> = auras
        .iter()
        .filter(|(f, _)| {
            world
                .building_at(f.anchor)
                .is_some_and(|b| defs.building(b).heat_aura_upgrade.is_some())
        })
        .collect();
    let warmed = if fires.is_empty() {
        0
    } else {
        world
            .units
            .iter()
            .filter(|u| !u.is_beast() && u.inside().is_none())
            .filter(|u| {
                fires.iter().any(|(f, a)| f.gap(Footprint::single(u.pos)) <= a.radius as i32)
            })
            .count() as u32
    };
    let studying = if auras.iter().any(|(_, a)| a.study > 1.0) {
        world
            .buildings()
            .filter(|b| defs.building(b.id).seats > 0)
            .filter(|b| world.aura_over(b.footprint(), defs).study_millis > 1000)
            .map(|b| world.occupants(b.anchor).count() as u32)
            .sum()
    } else {
        0
    };
    (warmed, studying)
}

/// `RunRecord::school_tallies` for a finished run: the world's own counters, the two sampled
/// tallies, and the two read off the world at the end.
fn school_tallies(world: &World, defs: &Defs, warmed: u32, studying: u32) -> Vec<(String, u32)> {
    let sheltered = if world.has_feature(crate::research::Feature::ShelteredRings, defs) {
        world.map.iter().filter(|&(p, _)| world.is_sheltered(p, defs)).count() as u32
    } else {
        0
    };
    let rail = world
        .buildings()
        .filter(|b| defs.building(b.id).uses_power)
        .filter(|b| world.powered_by_rail(b.footprint(), defs))
        .count() as u32;
    SCHOOL_TALLIES
        .iter()
        .map(|&name| {
            let n = match name {
                "sheltered_tiles" => sheltered,
                "rail_powered" => rail,
                "aura_heal" => warmed,
                "temple_study" => studying,
                other => world.tallies().school(other),
            };
            (name.to_owned(), n)
        })
        .collect()
}

impl RunRecord {
    /// The record as the older economy report: the part of it `run_economy` has always returned.
    pub fn economy_report(&self) -> EconomyReport {
        let reached = self
            .sign_minutes
            .iter()
            .all(|(id, m)| id == "masters" || id == "scrolls" || m.is_some());
        let ready = ready_minute(&self.sign_minutes, self.minutes);
        EconomyReport {
            ready: reached.then_some(ready),
            tail: decision_gaps(&self.timeline, ready).last().copied(),
            seasons: self.seasons.clone(),
            sign_minutes: self.sign_minutes.clone(),
            first_built: self.first_built.clone(),
            recipe_stores: self.recipe_stores.clone(),
            techs: self.techs.clone(),
            timeline: self.timeline.clone(),
            longest_gap: self.longest_gap,
            gaps_over_15: self.gaps_over_15,
            idle_share: self.idle_share.clone(),
            first_death_minute: self.first_death_minute,
            deaths: self.deaths,
        }
    }

    /// The minute `flag` was first raised, if it was.
    pub fn flag_minute(&self, flag: super::probe::Flag) -> Option<f32> {
        self.flags.iter().find(|r| r.flag == flag).map(|r| r.minute)
    }
}

/// Plays `spec` and records it (module doc). Panics only if `spec.preset` or the map size names
/// something `World::found` refuses at the default site, which the bundled data never does.
pub fn run(spec: &RunSpec, defs: &Defs) -> RunRecord {
    run_world(spec, defs).0
}

/// One probe minute of a run, as [`run_observed`] hands it to an observer: the world as the
/// minute found it (before the player acts on it), the sample the detectors just read and the
/// watch with that sample taken in.
#[derive(Clone, Copy)]
pub struct Minute<'a> {
    pub world: &'a World,
    pub sample: &'a Sample,
    pub watch: &'a Watch,
}

/// [`run`], calling `observe` once per probe minute, in order, from minute 0. This is how a trace
/// (`balance trace`, #535) reads a run: the observer sees the world and cannot change it, so the
/// record is exactly the one `run` returns for the same spec.
pub fn run_observed(spec: &RunSpec, defs: &Defs, observe: &mut dyn FnMut(Minute<'_>)) -> RunRecord {
    run_world_observed(spec, defs, observe).0
}

/// `run`, and the world as the run left it, for the scored runs (`run_economy_scored`).
pub(super) fn run_world(spec: &RunSpec, defs: &Defs) -> (RunRecord, World) {
    run_world_observed(spec, defs, &mut |_| {})
}

fn run_world_observed(
    spec: &RunSpec,
    defs: &Defs,
    observe: &mut dyn FnMut(Minute<'_>),
) -> (RunRecord, World) {
    let map_cfg = MapGenConfig {
        seed: spec.seed,
        width: spec.width,
        height: spec.height,
        ..MapGenConfig::default()
    };
    let survey = Survey::new(&map_cfg, defs);
    let start = survey.default_site(defs);
    let site = SiteProfile::compute(&survey, start, spec.nation, defs);
    let mut world = World::found(survey, spec.nation, start, PEOPLE, spec.preset.clone(), defs)
        .expect("Survey::default_site always names a site World::found accepts");

    let stone_items = autoplay::sign_items(defs, "stone");
    let ore_items = autoplay::sign_items(defs, "ore");
    let wood = defs.item_id("wood");

    let reachable: Vec<BuildingId> =
        endeavor::reachable_buildings(spec.nation, &defs.techs, &defs.buildings).collect();
    // The ones the opening raises (`autoplay::raised_buildings`): the reachable kinds plus the
    // optional horizon buildings, which the endeavor doesn't count (#713).
    let raised = autoplay::raised_buildings(spec.nation, defs);
    let mut first_built: Vec<Option<f32>> = vec![None; raised.len()];
    // One `(idle, total)` tally per 15-minute window (`EconomyReport::idle_share`), grown as the
    // run reaches a later window.
    let mut idle_windows: Vec<(u32, u32)> = Vec::new();

    let mut probe = Probe::new(&world, defs, &site);
    let mut watch = Watch::new(spec.detectors);
    let mut autopsies: Vec<Autopsy> = Vec::new();

    let total_ticks = minutes_to_ticks(spec.minutes);
    // `World::tick_season` only ever logs a `Changed` event at a later boundary, so spring, year
    // 1 is reported here, the way `World::season_index` already treats a fresh world.
    let mut seasons = vec![economy_season(
        &world,
        defs,
        world.season_index(defs),
        &stone_items,
        &ore_items,
        wood,
    )];
    let (mut warmed, mut studying) = (0_u32, 0_u32);
    let gate_tick = spec.gate_at.map(minutes_to_ticks);
    let mut gate_stood = false;
    for tick in 0..total_ticks {
        if gate_tick == Some(tick) {
            gate_stood = world.complete_endeavor(defs);
        }
        let offset = u64::from(spec.step_offset);
        if Opening::due(tick) {
            let (w, s) = sample_school(&world, defs);
            warmed += w;
            studying += s;
            let sample = probe.sample(&world, defs);
            watch.observe(&sample);
            observe(Minute { world: &world, sample: &sample, watch: &watch });
        }
        if tick >= offset && Opening::due(tick - offset) {
            spec.player.step(&mut world, defs);
            if first_built.iter().any(Option::is_none) {
                let standing: BTreeSet<BuildingId> = world.buildings().map(|b| b.id).collect();
                for (slot, b) in first_built.iter_mut().zip(&raised) {
                    if slot.is_none() && standing.contains(b) {
                        *slot = Some(ticks_to_minutes(world.tick));
                    }
                }
            }
            sample_idle_share(&world, &mut idle_windows);
        }
        world.tick(defs);
        world.assert_invariants(defs);

        if world.events_this_tick().iter().any(|e| matches!(e, Event::Death(_))) {
            autopsies.extend(probe.autopsies(&world, defs));
        }
        for ev in world.events_this_tick() {
            if let Event::Season(SeasonEvent {
                kind: SeasonEventKind::Changed { season }, ..
            }) = *ev
            {
                seasons.push(economy_season(&world, defs, season, &stone_items, &ore_items, wood));
            }
        }
    }

    let sign_minutes: Vec<(String, Option<f32>)> = defs
        .endeavor
        .signs
        .iter()
        .map(|s| {
            let minute = world.endeavor_events().find_map(|e| match &e.kind {
                EndeavorEventKind::SignMet(id) if *id == s.id => Some(ticks_to_minutes(e.tick)),
                _ => None,
            });
            (s.id.clone(), minute)
        })
        .collect();

    let first_built = raised
        .iter()
        .zip(first_built)
        .map(|(&b, minute)| (defs.building(b).id.clone(), minute))
        .collect();
    let recipe_items: BTreeSet<ItemId> = reachable
        .iter()
        .filter_map(|&b| defs.building(b).recipe.as_ref())
        .flat_map(|r| r.inputs.iter().chain(std::iter::once(&r.output)).map(|c| c.item))
        .chain(ore_items.iter().copied())
        .collect();
    let recipe_stores = recipe_items
        .into_iter()
        .map(|item| (defs.item(item).id.clone(), world.stored_total(item)))
        .collect();

    let techs: Vec<(String, f32)> = world
        .tech_completions()
        .map(|c| (defs.tech(c.tech).id.clone(), ticks_to_minutes(c.tick)))
        .collect();
    let timeline = decision_events(&world, defs, &sign_minutes);
    let ready = ready_minute(&sign_minutes, spec.minutes);
    let gaps = decision_gaps(&timeline, ready);
    let longest_gap = gaps.iter().copied().max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)));
    let gaps_over_15 = gaps.iter().filter(|&&(start, end)| end - start > 15.0).count() as u32;
    let idle_share: Vec<(f32, f32)> = idle_windows
        .iter()
        .enumerate()
        .map(|(window, &(idle, total))| {
            let share = if total > 0 { idle as f32 / total as f32 } else { 0.0 };
            (window as f32 * IDLE_WINDOW_MINUTES, share)
        })
        .collect();

    let record = RunRecord {
        seed: spec.seed,
        nation: spec.nation,
        preset: spec.preset.clone(),
        minutes: spec.minutes,
        style: format!("{:?}", spec.player),
        site,
        seasons,
        sign_minutes,
        timeline,
        first_built,
        recipe_stores,
        techs,
        longest_gap,
        gaps_over_15,
        idle_share,
        care: CareSummary::of(&world),
        first_death_minute: world.deaths().next().map(|d| ticks_to_minutes(d.tick)),
        deaths: world.deaths().count() as u32,
        paid_in: paid_in(&world, defs),
        population: world.population() as u32,
        autopsies,
        flags: watch.raised(),
        gate_minute: world.endeavor_events().find_map(|e| match e.kind {
            EndeavorEventKind::Complete => Some(ticks_to_minutes(e.tick)),
            _ => None,
        }),
        gate_stood,
        stage_minutes: defs
            .endeavor
            .stages
            .iter()
            .map(|s| {
                let minute = world.endeavor_events().find_map(|e| match &e.kind {
                    EndeavorEventKind::StageDone(id) if *id == s.id => {
                        Some(ticks_to_minutes(e.tick))
                    }
                    _ => None,
                });
                (s.id.clone(), minute)
            })
            .collect(),
        school_tallies: school_tallies(&world, defs, warmed, studying),
        teachings: teaching_record(&world, defs),
    };
    (record, world)
}

#[cfg(test)]
mod tests;

/// `RunRecord::paid_in`: the completed deals and requests of the run, by the good each paid in.
fn paid_in(world: &World, defs: &Defs) -> Vec<(String, u32)> {
    use crate::world::CaravanEventKind;
    let mut paid: std::collections::BTreeMap<String, u32> = Default::default();
    for e in world.caravan_events() {
        let give = match &e.kind {
            CaravanEventKind::DealDone { nation, deal, united } => {
                let Some(c) = defs.trade.caravan(*nation, *united) else {
                    continue;
                };
                &c.deals[usize::from(*deal)].give
            }
            CaravanEventKind::RequestDone { nation, request, united } => {
                let Some(c) = defs.trade.caravan(*nation, *united) else {
                    continue;
                };
                &c.requests[usize::from(*request)].give
            }
            _ => continue,
        };
        for g in give {
            let name = if g.any_food {
                "food".to_owned()
            } else {
                g.items.first().map_or_else(String::new, |&i| defs.item(i).id.clone())
            };
            *paid.entry(name).or_default() += 1;
        }
    }
    paid.into_iter().collect()
}

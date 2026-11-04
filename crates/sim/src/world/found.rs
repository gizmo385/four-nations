//! Founding a world at a chosen site (`docs/design/embark.md`, issue #289): turning a survey and
//! a cursor position into a live `World`, with the settlers placed near the start instead of
//! `new`'s map-centre spiral.
//!
//! Mapgen never sees the start (`Survey::new` already ran before a site was chosen), so founding
//! consumes the survey (`Survey::into_parts`) instead of generating again: a map stays a pure
//! function of its seed and size alone, and the regions a report approved are exactly the ones
//! settlers spawn into.

use crate::embark::{SETTLERS, SiteProblem, Survey, SurveyMode};

use super::*;

impl World {
    /// Builds a world from `survey`'s map at `start`, refusing any site `Survey::report` would
    /// refuse. `units` settlers land on open tiles (walkable, no resource) in `start`'s region,
    /// nearest first by squared distance and then by position, within `EmbarkDef::spawn_radius`.
    /// Draws no randomness of its own: `with_map` places them with the same `world.units` and
    /// `world.names` streams `new` always has, so the same seed gives the same people wherever
    /// they settle.
    pub fn found(
        survey: Survey,
        nation: Nation,
        start: Pos,
        units: u32,
        defs: &Defs,
    ) -> Result<Self, SiteProblem> {
        // `Survey::report`'s refusal checks room for `SETTLERS`, today's fixed starting count
        // (`fn_ui::STARTING_UNITS`); it doesn't read `units` (docs/design/embark.md, "Spawning").
        // The two are expected to always match; this is a tripwire if they ever drift, not a
        // redesign to share one constant across the sim/UI boundary.
        debug_assert_eq!(
            units, SETTLERS,
            "World::found's unit count no longer matches what Survey::report checked room for"
        );
        if let Some(problem) = survey.report(start, SurveyMode::Exact, defs).problem {
            return Err(problem);
        }
        let (cfg, map, buried, regions) = survey.into_parts();
        let region = regions.region(start).expect("report just confirmed this tile is open");
        let radius = defs.embark.spawn_radius;
        let mut spawns: Vec<Pos> = (start.y - radius..=start.y + radius)
            .flat_map(|y| (start.x - radius..=start.x + radius).map(move |x| Pos::new(x, y)))
            .filter(|&p| {
                map.get(p).is_some_and(|t| t.terrain.is_walkable() && t.resource.is_none())
            })
            .filter(|&p| regions.region(p) == Some(region))
            .collect();
        spawns.sort_by_key(|p| ((p.x - start.x).pow(2) + (p.y - start.y).pow(2), *p));
        spawns.truncate(units as usize);

        let mut world = Self::with_map(cfg.seed, map, nation, &spawns, defs);
        // The survey already took these off its map exactly as `bury` would, so `with_map`'s own
        // `bury()` call (run against an already-clean map) found nothing. Restore them here
        // instead of leaving the founded world thinking nothing is buried.
        world.buried = buried;
        world.start = Some(start);
        Ok(world)
    }

    /// Where this world's settlers began, if it was founded at a chosen site (`found`) rather
    /// than `new`'s map-centre spiral.
    pub fn start(&self) -> Option<Pos> {
        self.start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs() -> Defs {
        Defs::bundled()
    }

    fn survey_at(seed: u64, defs: &Defs) -> Survey {
        let size = defs.embark.map_size("small").unwrap();
        let cfg = Survey::config(seed, size);
        Survey::new(&cfg, defs)
    }

    #[test]
    fn founding_twice_at_the_same_seed_and_start_serializes_identically() {
        let defs = defs();
        let start = survey_at(7, &defs).default_site(&defs);
        let a = World::found(survey_at(7, &defs), Nation::Earth, start, SETTLERS, &defs).unwrap();
        let b = World::found(survey_at(7, &defs), Nation::Earth, start, SETTLERS, &defs).unwrap();
        assert_eq!(a.save(&defs).unwrap(), b.save(&defs).unwrap());
    }

    #[test]
    fn settlers_land_in_the_start_s_region_within_the_radius() {
        let defs = defs();
        let survey = survey_at(7, &defs);
        let start = survey.default_site(&defs);
        let radius = defs.embark.spawn_radius;
        let world = World::found(survey, Nation::Earth, start, SETTLERS, &defs).unwrap();
        assert_eq!(world.units.len(), SETTLERS as usize);
        for u in &world.units {
            assert!(
                (u.pos.x - start.x).abs() <= radius && (u.pos.y - start.y).abs() <= radius,
                "{:?} is outside the spawn radius of {start:?}",
                u.pos
            );
            assert!(world.map.get(u.pos).unwrap().terrain.is_walkable());
        }
        assert_eq!(world.start(), Some(start));
    }

    #[test]
    fn a_refused_site_returns_its_problem() {
        let defs = defs();
        let survey = survey_at(7, &defs);
        // The map's forced-sea border: always water, always refused.
        let err = World::found(survey, Nation::Earth, Pos::new(0, 0), SETTLERS, &defs).unwrap_err();
        assert_eq!(err, SiteProblem::NotLand);
    }

    #[test]
    fn a_founded_world_round_trips_through_a_save_with_its_start() {
        let defs = defs();
        let survey = survey_at(7, &defs);
        let start = survey.default_site(&defs);
        let world = World::found(survey, Nation::Earth, start, SETTLERS, &defs).unwrap();
        let text = world.save(&defs).unwrap();
        let loaded = World::load(&text, &defs).unwrap();
        assert_eq!(loaded.start(), Some(start));
        assert_eq!(loaded.save(&defs).unwrap(), text);
    }

    #[test]
    fn a_world_made_by_new_has_no_start() {
        let defs = defs();
        let cfg = MapGenConfig { seed: 7, ..Default::default() };
        let world = World::new(&cfg, Nation::Earth, 4, &defs);
        assert_eq!(world.start(), None);
    }

    #[test]
    fn founding_works_at_every_map_size_preset() {
        let defs = defs();
        for size in &defs.embark.map_sizes {
            let cfg = Survey::config(7, size);
            let survey = Survey::new(&cfg, &defs);
            let start = survey.default_site(&defs);
            let world = World::found(survey, Nation::Earth, start, SETTLERS, &defs)
                .unwrap_or_else(|e| panic!("{}: refused its own default site: {e:?}", size.id));
            assert_eq!((world.map.width(), world.map.height()), (size.width, size.height));
        }
    }
}

//! One way to start a game and drive it: the world, the game data and the clock together.

use fn_sim::bending::AbilityId;
use fn_sim::embark::{SiteProblem, Survey};
use fn_sim::world::{Command, CommandError};
use fn_sim::{Defs, MapGenConfig, Nation, Pos, World};

use crate::clock::Clock;

/// How many settlers a new game starts with.
pub const STARTING_UNITS: u32 = 6;

#[derive(Debug, Clone)]
pub struct Session {
    pub world: World,
    pub defs: Defs,
    pub clock: Clock,
    /// The ability the Bend tool channels, when the nation can use more than one (Fire's Inner
    /// Fire, #123): `B` again moves on to the next (`tools::cycle_ability`). `tools::bend_ability`
    /// reads it, and falls back to the nation's first usable ability if it isn't usable (yet).
    pub bend_choice: Option<AbilityId>,
}

impl Session {
    /// A new game on the map for `seed`, with the bundled data, running at the default speed.
    pub fn new(seed: u64, nation: Nation, width: i32, height: i32) -> Self {
        let defs = Defs::bundled();
        let cfg = MapGenConfig { seed, width, height, ..MapGenConfig::default() };
        let world = World::new(&cfg, nation, STARTING_UNITS, &defs);
        Self { world, defs, clock: Clock::default(), bend_choice: None }
    }

    /// A new game founded at `start` on `survey`'s map (`docs/design/embark.md`, #289), with the
    /// bundled data and the same starting unit count as `new`. `difficulty` is the chosen
    /// preset's id, or `None` for identity (`docs/design/difficulty.md`, #294). Refuses whatever
    /// `World::found` refuses.
    pub fn found(
        survey: Survey,
        nation: Nation,
        start: Pos,
        difficulty: Option<String>,
    ) -> Result<Self, SiteProblem> {
        let defs = Defs::bundled();
        let world = World::found(survey, nation, start, STARTING_UNITS, difficulty, &defs)?;
        Ok(Self { world, defs, clock: Clock::default(), bend_choice: None })
    }

    /// `World::apply` with this session's data.
    pub fn apply(&mut self, command: Command) -> Result<(), CommandError> {
        self.world.apply(command, &self.defs)
    }

    /// Runs one tick, whatever the clock says.
    pub fn tick(&mut self) {
        self.world.tick(&self.defs);
    }

    /// Runs `n` ticks, whatever the clock says.
    pub fn tick_n(&mut self, n: u32) {
        for _ in 0..n {
            self.tick();
        }
    }

    /// Runs the ticks `real_secs` of elapsed wall time is worth (`Clock::advance`) and returns
    /// how many that was.
    pub fn advance(&mut self, real_secs: f64) -> u32 {
        let n = self.clock.advance(real_secs);
        self.tick_n(n);
        n
    }

    /// Single-steps one tick while paused (`Clock::step`) and returns how many ran.
    pub fn step(&mut self) -> u32 {
        let n = self.clock.step();
        self.tick_n(n);
        n
    }

    // ----------------------------------------------------------------------------------------
    // Read-only helpers for `fn_tui::surface::Host` (#320): the agent surface's source lint
    // forbids a bare `.world` in `crates/tui/src/surface.rs`, so a handful of small, named
    // reads live here instead of there, the same way every other screen's read already goes
    // through an `fn_ui` view model rather than touching `World` directly.
    // ----------------------------------------------------------------------------------------

    /// The world's own tick count, for a run log's diagnostics (not part of any observation).
    pub fn ticks_elapsed(&self) -> u64 {
        self.world.tick
    }

    /// The map's width and height, for an off-map check (`Host::look`'s own regions already
    /// clamp to this; a step's exact tile has to be on the map too).
    pub fn map_size(&self) -> (i32, i32) {
        (self.world.map.width(), self.world.map.height())
    }

    /// This session's nation, for naming a building or ghost the way `defs.building(..)
    /// .display_name` wants it.
    pub fn nation(&self) -> Nation {
        self.world.nation
    }

    /// The settler standing at `pos`, if any, by name -- `World::units_at`'s first occupant,
    /// the same one a cell's `Thing::Unit` means.
    pub fn unit_name_at(&self, pos: Pos) -> Option<&str> {
        self.world.units_at(pos).next().map(|u| u.name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_session_has_the_starting_settlers_and_runs() {
        let s = Session::new(7, Nation::Earth, 48, 32);
        assert_eq!(s.world.units.len(), STARTING_UNITS as usize);
        assert_eq!(s.world.nation, Nation::Earth);
        assert_eq!((s.world.map.width(), s.world.map.height()), (48, 32));
        assert!(!s.clock.paused);
    }

    #[test]
    fn the_clock_decides_how_many_ticks_run() {
        let mut s = Session::new(7, Nation::Earth, 48, 32);
        assert_eq!(s.advance(1.0), 10);
        assert_eq!(s.world.tick, 10);
        s.clock.paused = true;
        assert_eq!(s.advance(5.0), 0);
        assert_eq!(s.world.tick, 10);
        assert_eq!(s.step(), 1);
        assert_eq!(s.world.tick, 11);
        s.clock.paused = false;
        assert_eq!(s.step(), 0, "stepping is for pausing");
        assert_eq!(s.world.tick, 11);
    }

    #[test]
    fn apply_goes_through_the_world_and_reports_its_refusals() {
        let mut s = Session::new(7, Nation::Earth, 48, 32);
        assert!(s.apply(Command::DesignateHarvest { pos: Pos::new(-1, -1) }).is_err());
    }

    #[test]
    fn found_starts_a_session_at_the_chosen_site() {
        let defs = Defs::bundled();
        let size = defs.embark.map_size("small").unwrap();
        let cfg = Survey::config(7, size);
        let survey = Survey::new(&cfg, &defs);
        let start = survey.default_site(&defs);
        let s = Session::found(survey, Nation::Earth, start, None).unwrap();
        assert_eq!(s.world.units.len(), STARTING_UNITS as usize);
        assert_eq!(s.world.nation, Nation::Earth);
        assert_eq!(s.world.start(), Some(start));
    }

    #[test]
    fn found_refuses_a_site_the_world_would_refuse() {
        let defs = Defs::bundled();
        let size = defs.embark.map_size("small").unwrap();
        let cfg = Survey::config(7, size);
        let survey = Survey::new(&cfg, &defs);
        // The map's forced-sea border: always water, always refused.
        let err = Session::found(survey, Nation::Earth, Pos::new(0, 0), None).unwrap_err();
        assert_eq!(err, SiteProblem::NotLand);
    }
}

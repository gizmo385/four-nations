//! Death autopsies (`docs/design/balance-harness.md`, section 3; #534): what a run knows about
//! each settler who died, so "why" is one line of a record instead of an hour of trace programs
//! (#487). "Starvation, Unreachable for 14 minutes before death, 52 food within 15 tiles, no
//! path to a stockpile" is a sim lock.

use crate::autoplay::FORAGE_RADIUS;
use crate::defs::Defs;
use crate::geom::Pos;
use crate::needs::DeathCause;
use crate::world::{Death, HarmKind, World};

use super::probe::{Occupation, Track};
use super::ticks_to_minutes;

/// One settler's death, with the facts that explain it. Times are as fine as the probe's minute:
/// the settler was last seen eating or drinking at a probe, not at the tick it did.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Autopsy {
    pub name: String,
    pub minute: f32,
    pub cause: DeathCause,
    /// The harm that struck shortly before, if the cause is a depleted need (`Death::weakened_by`).
    pub weakened_by: Option<HarmKind>,
    /// Minutes since it was last seen to eat, counting from its arrival if it never did.
    pub minutes_since_ate: f32,
    /// Minutes since it was last seen to drink, counted the same way.
    pub minutes_since_drank: f32,
    /// What it was doing in each of its last ten minutes, oldest first (fewer if it was newer).
    pub last_minutes: Vec<Occupation>,
    /// Minutes running it was idle with only unreachable work when it died.
    pub unreachable_minutes: u32,
    /// Food within `FORAGE_RADIUS` tiles of where it fell, in items (`World::food_near`).
    pub food_nearby: u32,
    /// Whether it could walk to a tile beside water.
    pub water_reachable: bool,
    /// Whether it could walk to a stockpile; `None` if the settlement had marked none.
    pub stockpile_reachable: Option<bool>,
    /// Whether it could walk to the settlement's anchor.
    pub home_reachable: bool,
}

impl Autopsy {
    /// The autopsy of `death`, from the probe's memory of the settler (`None` if it died before
    /// the probe first saw it: a newcomer, say) and the world right after.
    pub(super) fn of(
        death: &Death,
        track: Option<&Track>,
        anchor: Pos,
        world: &World,
        defs: &Defs,
    ) -> Self {
        let minute = ticks_to_minutes(death.tick);
        let at = track.map_or(anchor, |t| t.pos);
        let reach = world.reach_from(at);
        let since = |seen: Option<u32>| seen.map_or(0.0, |m| (minute - m as f32).max(0.0));
        Self {
            name: death.name.clone(),
            minute,
            cause: death.cause,
            weakened_by: death.weakened_by,
            minutes_since_ate: since(track.map(|t| t.last_ate)),
            minutes_since_drank: since(track.map(|t| t.last_drank)),
            last_minutes: track.map_or_else(Vec::new, |t| t.recent.iter().copied().collect()),
            unreachable_minutes: track.map_or(0, |t| t.unreachable),
            food_nearby: world.food_near(at, FORAGE_RADIUS, defs),
            water_reachable: reach.water,
            stockpile_reachable: reach.stockpile,
            home_reachable: reach.anchor,
        }
    }

    /// The autopsy in a line, the way the doc's example reads.
    pub fn summary(&self) -> String {
        let mut line = format!("{:?}", self.cause);
        if let Some(w) = self.weakened_by {
            line.push_str(&format!(", weakened by {w:?}"));
        }
        if self.unreachable_minutes > 0 {
            line.push_str(&format!(", Unreachable for {} minutes", self.unreachable_minutes));
        }
        line.push_str(&format!(", {} food within {FORAGE_RADIUS} tiles", self.food_nearby));
        if !self.water_reachable {
            line.push_str(", no path to water");
        }
        if self.stockpile_reachable == Some(false) {
            line.push_str(", no path to a stockpile");
        }
        if !self.home_reachable {
            line.push_str(", no path home");
        }
        line
    }
}

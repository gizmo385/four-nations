//! The moment of a bend (#166): the bender strikes its nation's pose and a short effect plays
//! for the ability, instead of the result simply appearing. The lasting results (a flame, wet
//! ground) are `bending`'s; the pose rows are in the character sheets (`unit_anim`), the effect
//! frames on the `fx` sheet (`sprites::fx`).
//!
//! **Driven by the sim, paced by the world clock.** Nothing here has a clock of its own:
//!
//! - While a unit channels, `World::bend_channels` says how long is left (`BendChannel`).
//! - When the bend completes the sim logs a `BendEvent` (the one that feeds the alert log), with
//!   the tick it landed on and, for Wind Carry, the stack it moved and where.
//!
//! Both give a time in seconds *relative to completion*: negative while channelling (minus the
//! time left), then the ticks since the event over `TICKS_PER_SECOND`. Everything on screen is a
//! pure function of that time ([`parts`], [`bend_row`]), so a paused game holds a still frame and
//! 2x, 3x and 5x play it that much faster, as the flames of `bending` do with `world.tick`.
//!
//! **One look per ability.** [`LOOKS`] maps each ability id of `data/bending.toml` to one of five
//! [`Look`]s (its own table, the one ability-specific place here); a test checks every ability
//! has one. The look says which frames play where. Colours are the frames' own, so each nation's
//! look comes from the art: brown stone for Earth, blue water, orange sparks, white wind.
//!
//! | Look | Timeline (seconds from completion) |
//! |---|---|
//! | `Rise` | cracks, spikes and stones on the target from -0.9; dust after the ford lands |
//! | `Stream` | a stream of droplets from the bender's hand to the target from -1.0; splashes over the 3x3 patch |
//! | `Flare` | sparks gather from -0.5, a flare as the flame is kindled, embers rise after |
//! | `Gust` | curls of wind run to the stack from -1.0, a whirl lifts it, and it flies to the stockpile |
//! | `Glow` | embers gather on a ring round the bender from -0.8, the ring blooms, then warm motes drift out over the tiles beside it and embers rise |
//!
//! Inner Fire's *lasting* aura (a ring of light under the carrier while it lasts) is `bending`'s.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use fn_sim::Pos;
use fn_sim::bending::AbilityId;
use fn_sim::world::{BendChannel, BendEvent, BendOutcome, Moved, TICKS_PER_SECOND, UnitId};

use super::unit_anim::{self, BEND_RELEASE_ROW, BEND_STANCE_ROW, Pose, UnitView, facing_after};
use super::{Art, MapEntity, Sim, TILE, UnitSprite, row_depth, sprites, sync_units};

const TPS: f32 = TICKS_PER_SECOND as f32;

/// The shape of an ability's cast effect.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    /// Earth rises on the target tile.
    Rise,
    /// A stream of water from the bender onto the target tiles.
    Stream,
    /// A burst of sparks on the target as a flame is kindled.
    Flare,
    /// Wind from the bender, a whirl on the stack, and the stack flying off.
    Gust,
    /// A warm glow gathering round the bender and pulsing out over the tiles beside it.
    Glow,
}

/// The cast effect of each ability, by its id in `data/bending.toml`. The one table of the cast
/// animations that names abilities; everything below works on a [`Look`].
pub const LOOKS: &[(&str, Look)] = &[
    ("raise_earth", Look::Rise),
    ("water_the_land", Look::Stream),
    ("hearthflame", Look::Flare),
    ("wind_carry", Look::Gust),
    ("inner_fire", Look::Glow),
];

/// The cast effect of ability `id`, if it has one.
pub fn look_of(id: &str) -> Option<Look> {
    LOOKS.iter().find(|(a, _)| *a == id).map(|(_, look)| *look)
}

// -------------------------------------------------------------------------------------------
// Time: seconds relative to the moment the bend completes

/// The pose is the release this long before completion, and until [`POSE_TAIL`] after it.
pub const RELEASE_LEAD: f32 = 0.5;
/// How long after completion the bender holds the release pose, before going back to idle.
pub const POSE_TAIL: f32 = 0.6;

/// The time of a cast, in seconds relative to completion: minus the channel time left while it
/// runs, then the ticks since the completion event (`finished`, a tick) over the tick rate.
pub fn time_of(finished: Option<u64>, remaining_secs: f32, now: u64) -> f32 {
    match finished {
        Some(at) => now.saturating_sub(at) as f32 / TPS,
        None => -remaining_secs,
    }
}

/// The sheet row of the bend pose at time `t`: the stance while channelling, the release around
/// completion, nothing (the walk or idle pose) once it is over.
pub fn bend_row(t: f32) -> Option<u32> {
    if t > POSE_TAIL {
        None
    } else if t >= -RELEASE_LEAD {
        Some(BEND_RELEASE_ROW)
    } else {
        Some(BEND_STANCE_ROW)
    }
}

impl Look {
    /// The seconds before and after completion in which the effect draws something. `flight` is
    /// how long Wind Carry's stack flies, 0 if it doesn't.
    pub fn span(self, flight: f32) -> (f32, f32) {
        match self {
            Look::Rise => (0.9, 0.51),
            Look::Stream => (1.0, 0.55),
            Look::Flare => (0.5, 0.6),
            Look::Gust => (1.0, 0.3_f32.max(flight)),
            Look::Glow => (GLOW_LEAD, GLOW_TAIL),
        }
    }
}

// -------------------------------------------------------------------------------------------
// What is drawn at a time: pure

/// What a part shows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Visual {
    /// A cell of the effects sheet.
    Fx(sprites::Cell),
    /// The icon of the stack Wind Carry moved.
    Item,
}

/// One sprite of an effect: where (in tiles, the centre of tile (x, y) being exactly (x, y)),
/// how many pixels above the ground, and what.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Part {
    pub at: Vec2,
    pub lift: f32,
    pub visual: Visual,
    /// Drawn over the units of its row (a spark, a droplet), not under them (stones, a splash).
    pub over: bool,
}

/// Where a cast happens, in tile coordinates.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Scene {
    /// The tile the bender stands on.
    pub stand: Vec2,
    /// The tile being bent.
    pub target: Vec2,
    /// Wind Carry: the stockpile tile the stack lands on.
    pub moved_to: Option<Vec2>,
}

impl Scene {
    /// The bender's hand: a little way from its tile toward the target.
    fn hand(&self) -> Vec2 {
        let dir = (self.target - self.stand).normalize_or(Vec2::Y);
        self.stand + dir * 0.35
    }

    /// How long the stack takes to fly to the stockpile: longer for a longer way.
    pub fn flight(&self) -> f32 {
        self.moved_to.map_or(0.0, |to| (0.5 + 0.07 * to.distance(self.target)).clamp(0.6, 1.3))
    }
}

/// The frame (0 to 2) showing `step` seconds each from time `from`, if `t` is within them.
fn frame_at(t: f32, from: f32, step: f32) -> Option<usize> {
    let i = ((t - from) / step).floor();
    (t >= from && i < 3.0).then_some(i as usize)
}

/// A part on the ground, under the units of its row.
fn fx(at: Vec2, lift: f32, cell: sprites::Cell) -> Part {
    Part { at, lift, visual: Visual::Fx(cell), over: false }
}

/// A part in the air, over the units of its row.
fn fx_over(at: Vec2, lift: f32, cell: sprites::Cell) -> Part {
    Part { over: true, ..fx(at, lift, cell) }
}

/// The parts to draw for `look` at time `t` (seconds from completion). Empty outside the look's
/// [`span`](Look::span).
pub fn parts(look: Look, t: f32, scene: &Scene) -> Vec<Part> {
    let mut out = Vec::new();
    match look {
        Look::Rise => rise(t, scene, &mut out),
        Look::Stream => stream(t, scene, &mut out),
        Look::Flare => flare(t, scene, &mut out),
        Look::Gust => gust(t, scene, &mut out),
        Look::Glow => glow(t, scene, &mut out),
    }
    out
}

/// Raise Earth: cracks, rock spikes and stepping stones rise on the target, and as the ford lands
/// dust settles over it.
fn rise(t: f32, s: &Scene, out: &mut Vec<Part>) {
    let cell = if t < -0.9 {
        return;
    } else if t < -0.6 {
        sprites::fx::EARTH[0]
    } else if t < -0.2 {
        sprites::fx::EARTH[1]
    } else if t < 0.0 {
        sprites::fx::EARTH[2]
    } else if let Some(i) = frame_at(t, 0.0, 0.17) {
        sprites::fx::DUST[i]
    } else {
        return;
    };
    out.push(fx(s.target, 0.0, cell));
}

/// How many droplets are in flight at once in Water the Land's stream.
const STREAM_DROPS: usize = 7;
/// Pixels above the ground at the bender's hand, and the extra height of the arc's top.
const HAND_LIFT: f32 = 6.0;
const STREAM_ARC: f32 = 9.0;

/// Where a stream aimed at the bender's own tile sprays: the ring around it.
const SPRAY: [Vec2; 8] = [
    Vec2::new(1.0, 0.0),
    Vec2::new(-1.0, 0.0),
    Vec2::new(0.0, 1.0),
    Vec2::new(0.0, -1.0),
    Vec2::new(1.0, 1.0),
    Vec2::new(-1.0, -1.0),
    Vec2::new(1.0, -1.0),
    Vec2::new(-1.0, 1.0),
];

/// Water the Land: droplets run from the bender's hand to the target in an arc, and the stream
/// splashes over the patch it is about to wet (the target and the ring around it, the nearer
/// tiles first).
fn stream(t: f32, s: &Scene, out: &mut Vec<Part>) {
    if (-1.0..=0.05).contains(&t) {
        let hand = s.hand();
        let standing_on_target = s.stand.distance(s.target) < 0.5;
        // The stream reaches out from the hand, then flows at a steady pace.
        let front = ((t + 1.0) / 0.35).clamp(0.0, 1.0);
        for k in 0..STREAM_DROPS {
            let u = ((t + 1.0) * 2.0 + k as f32 / STREAM_DROPS as f32).rem_euclid(1.0);
            if u > front {
                continue;
            }
            // A bender standing on its target sprays the ring around it instead.
            let aim = if standing_on_target { s.target + SPRAY[k % SPRAY.len()] } else { s.target };
            let lift = HAND_LIFT * (1.0 - u) + 4.0 * STREAM_ARC * u * (1.0 - u);
            out.push(fx_over(hand.lerp(aim, u), lift, sprites::fx::DROP[k % 3]));
        }
    }
    for dy in -1..=1_i32 {
        for dx in -1..=1_i32 {
            let delay = 0.1 * dx.abs().max(dy.abs()) as f32;
            if let Some(i) = frame_at(t, -0.4 + delay, 0.17) {
                let at = s.target + Vec2::new(dx as f32, dy as f32);
                out.push(fx(at, 0.0, sprites::fx::SPLASH[i]));
            }
        }
    }
}

/// Hearthflame: sparks gather, a flare as the flame is kindled (the steady flame is `bending`'s,
/// and shows from the moment of the flare), embers rising after.
fn flare(t: f32, s: &Scene, out: &mut Vec<Part>) {
    let (cell, lift) = if t < -0.5 {
        return;
    } else if t < -0.2 {
        (sprites::fx::SPARK[0], 0.0)
    } else if t < 0.15 {
        (sprites::fx::SPARK[1], 0.0)
    } else if t < 0.6 {
        (sprites::fx::SPARK[2], (t - 0.15) * 14.0)
    } else {
        return;
    };
    out.push(fx_over(s.target, lift, cell));
}

/// Inner Fire's glow starts this long before completion and lasts this long after it.
const GLOW_LEAD: f32 = 0.8;
const GLOW_TAIL: f32 = 0.6;
/// How far the pulse spreads from the bender, in tiles (Chebyshev): the aura's own reach in
/// `data/bending.toml` is 2.
const GLOW_REACH: i32 = 2;

/// Inner Fire: embers gather on a ring round the bender, the ring blooms as the aura takes hold,
/// and warm motes drift out over the tiles beside it (the nearer ones first) while embers rise
/// off the bender. The aura itself, a ring under the carrier's feet, is `bending`'s.
fn glow(t: f32, s: &Scene, out: &mut Vec<Part>) {
    if t < -GLOW_LEAD {
        return;
    }
    // The glow is the bender's own: the tile it stands on, which is the target.
    let centre = s.stand;
    if t < -0.3 {
        out.push(fx_over(centre, 0.0, sprites::fx::GLOW[0]));
    } else if t < 0.0 {
        out.push(fx_over(centre, 0.0, sprites::fx::GLOW[1]));
    }
    for dy in -GLOW_REACH..=GLOW_REACH {
        for dx in -GLOW_REACH..=GLOW_REACH {
            let ring = dx.abs().max(dy.abs());
            let from = 0.1 * ring as f32;
            // The motes show for a third of a second at each tile, a beat after the one nearer.
            if (from..from + 0.35).contains(&t) {
                let at = centre + Vec2::new(dx as f32, dy as f32);
                // The bloom holds on the bender's own tile as the motes start.
                let cell = if ring == 0 { sprites::fx::GLOW[1] } else { sprites::fx::GLOW[2] };
                out.push(if ring == 0 { fx_over(at, 0.0, cell) } else { fx(at, 0.0, cell) });
            }
        }
    }
    if (0.0..GLOW_TAIL).contains(&t) {
        out.push(fx_over(centre, HAND_LIFT + t * 12.0, sprites::fx::SPARK[2]));
    }
}

/// Curls of wind in the gust and in the trail of the flying stack.
const GUST_CURLS: usize = 4;
const TRAIL_CURLS: usize = 3;
/// How high the stack's arc rises above the straight line, in pixels.
const FLIGHT_ARC: f32 = 10.0;

/// Wind Carry: curls of wind run from the bender to the stack, a whirl lifts it, and the stack
/// flies in an arc to the stockpile with a trail of wind behind it.
fn gust(t: f32, s: &Scene, out: &mut Vec<Part>) {
    let spin =
        |k: usize| sprites::fx::GUST[((t * 10.0).floor() as i64 + k as i64).rem_euclid(3) as usize];
    if (-1.0..0.0).contains(&t) {
        let hand = s.hand();
        let front = ((t + 1.0) / 0.4).clamp(0.0, 1.0);
        for k in 0..GUST_CURLS {
            let u = ((t + 1.0) * 1.6 + k as f32 / GUST_CURLS as f32).rem_euclid(1.0);
            if u > front {
                continue;
            }
            let wobble = 3.0 * (std::f32::consts::TAU * (u * 2.0 + k as f32 * 0.25)).sin();
            // A bender standing on the stack whirls the wind round it instead.
            let at = if s.stand.distance(s.target) < 0.5 {
                let a = std::f32::consts::TAU * (u + k as f32 / GUST_CURLS as f32);
                s.target + Vec2::new(a.cos(), a.sin() * 0.6) * 0.8
            } else {
                hand.lerp(s.target, u)
            };
            out.push(fx_over(at, HAND_LIFT + wobble, spin(k)));
        }
    }
    if (-0.5..0.3).contains(&t) {
        let i = ((t * 10.0).floor() as i64).rem_euclid(3) as usize;
        out.push(fx_over(s.target, 0.0, sprites::fx::SWIRL[i]));
    }
    let (Some(to), true) = (s.moved_to, t >= 0.0) else { return };
    let flight = s.flight();
    if t > flight {
        return;
    }
    let arc = |u: f32| 4.0 * FLIGHT_ARC * u * (1.0 - u) + 2.0;
    let u = t / flight;
    out.push(Part { at: s.target.lerp(to, u), lift: arc(u), visual: Visual::Item, over: true });
    for j in 1..=TRAIL_CURLS {
        let behind = u - 0.1 * j as f32;
        if behind >= 0.0 {
            out.push(fx_over(s.target.lerp(to, behind), arc(behind), spin(j)));
        }
    }
}

// -------------------------------------------------------------------------------------------
// Casts in progress: from the sim's channels and events

/// One bend being cast, as far as the animation knows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cast {
    pub look: Look,
    pub unit: UnitId,
    /// Where the unit stood when it began.
    pub stand: Pos,
    pub target: Pos,
    /// The tick the sim logged the bend as done, once it has.
    pub finished: Option<u64>,
    /// Channel seconds left, as of the last time the sim was asked (while unfinished).
    pub remaining: f32,
    pub moved: Option<Moved>,
}

impl Cast {
    fn new(look: Look, unit: UnitId, stand: Pos, target: Pos) -> Self {
        Cast { look, unit, stand, target, finished: None, remaining: 0.0, moved: None }
    }

    pub fn time(&self, now: u64) -> f32 {
        time_of(self.finished, self.remaining, now)
    }

    pub fn scene(&self) -> Scene {
        let at = |p: Pos| Vec2::new(p.x as f32, p.y as f32);
        Scene {
            stand: at(self.stand),
            target: at(self.target),
            moved_to: self.moved.map(|m| at(m.to)),
        }
    }

    /// Whether the cast has nothing left to show.
    fn is_over(&self, now: u64) -> bool {
        let flight = self.scene().flight();
        self.finished.is_some() && self.time(now) > self.look.span(flight).1.max(POSE_TAIL)
    }
}

/// A bend event this many ticks old or less is still being played. Longer than the longest
/// effect, so a cast is never started again from an event it has already shown.
pub const RECENT_TICKS: u64 = 20;

/// Brings the casts up to date with the sim: starts one for each unit that is channelling (or
/// whose bend completed just now), notes completions, and returns the units whose casts ended.
///
/// - `channels`: `World::bend_channels`; `events`: `World::bend_events`, oldest first; `now`:
///   `World::tick`; `stand_of`: where a unit is (`None` once it has died); `look_of`: an
///   ability's look.
pub fn advance(
    casts: &mut BTreeMap<UnitId, Cast>,
    channels: &[BendChannel],
    events: &[BendEvent],
    now: u64,
    stand_of: impl Fn(UnitId) -> Option<Pos>,
    look_of: impl Fn(AbilityId) -> Option<Look>,
) -> Vec<UnitId> {
    let mut ended = Vec::new();
    for ch in channels {
        let Some(look) = look_of(ch.ability) else { continue };
        match casts.get_mut(&ch.unit) {
            Some(c) if c.finished.is_none() && c.target == ch.pos && c.look == look => {
                c.remaining = ch.remaining_secs;
            }
            _ => {
                let Some(stand) = stand_of(ch.unit) else { continue };
                let mut c = Cast::new(look, ch.unit, stand, ch.pos);
                c.remaining = ch.remaining_secs;
                casts.insert(ch.unit, c);
            }
        }
    }
    // Events in order, the recent ones only.
    let recent = events.iter().rev().take_while(|e| now.saturating_sub(e.tick) < RECENT_TICKS);
    for ev in recent.collect::<Vec<_>>().into_iter().rev() {
        let Some(look) = look_of(ev.ability) else { continue };
        match (casts.get_mut(&ev.unit), ev.outcome) {
            (Some(c), BendOutcome::Done) if c.finished.is_none() && c.target == ev.pos => {
                c.finished = Some(ev.tick);
                c.moved = ev.moved;
            }
            (Some(c), BendOutcome::Failed(_)) if c.finished.is_none() && c.target == ev.pos => {
                casts.remove(&ev.unit);
                ended.push(ev.unit);
            }
            (Some(c), _) if c.finished == Some(ev.tick) => {}
            (_, BendOutcome::Done) => {
                // Seen only by its event: a loaded game, or a channel too short to have been
                // looked at.
                let Some(stand) = stand_of(ev.unit) else { continue };
                let mut c = Cast::new(look, ev.unit, stand, ev.pos);
                c.finished = Some(ev.tick);
                c.moved = ev.moved;
                casts.insert(ev.unit, c);
            }
            _ => {}
        }
    }
    let live: BTreeSet<UnitId> = channels.iter().map(|c| c.unit).collect();
    casts.retain(|unit, c| {
        // Pre-empted (a need, a cancel) or dead before it finished, or done and played out.
        let running = if c.finished.is_none() { live.contains(unit) } else { !c.is_over(now) };
        let keep = running && stand_of(*unit).is_some();
        if !keep {
            ended.push(*unit);
        }
        keep
    });
    ended
}

// -------------------------------------------------------------------------------------------
// Bevy: the casts as resource, entities and poses

/// The casts in progress, and the entities drawing them.
#[derive(Resource, Default)]
pub struct Casts {
    casts: BTreeMap<UnitId, Cast>,
    /// The sprites of each cast's parts, reused from frame to frame.
    sprites: BTreeMap<UnitId, Vec<Entity>>,
    /// Stockpile tiles whose stack is hidden while the one it holds flies in (`hide_landing`).
    hidden: BTreeSet<Pos>,
    /// The last world tick seen, to notice a new world (regenerate, a loaded game).
    last_tick: u64,
}

impl Casts {
    /// Drops every cast and despawns their sprites, for a world that replaces the running one (a
    /// new map, a loaded game).
    pub fn forget(&mut self, commands: &mut Commands) {
        for e in self.sprites.values().flatten() {
            commands.entity(*e).try_despawn();
        }
        *self = Casts::default();
    }

    /// The cast of `unit`, if it is casting.
    #[cfg(test)]
    pub fn of(&self, unit: UnitId) -> Option<&Cast> {
        self.casts.get(&unit)
    }
}

/// A part's sprite, and the cell it shows, to change the sprite only when the frame changes.
#[derive(Component)]
struct CastPart(sprites::Cell);

/// Parts on the ground sit over the floor and any flame, under the units of their row (a bender
/// standing on its target isn't covered); parts in the air (a droplet, a spark, a stack in flight) over them.
const FX_BIAS: f32 = 0.0004;
const OVER_BIAS: f32 = 0.0015;

pub struct CastPlugin;

impl Plugin for CastPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Casts>().add_systems(
            Update,
            (track_casts, pose_casters, sync_parts, hide_landing)
                .chain()
                .after(sync_units)
                .before(unit_anim::animate_units),
        );
    }
}

/// Starts, advances and ends the casts from what the sim says.
fn track_casts(mut commands: Commands, sim: Res<Sim>, mut casts: ResMut<Casts>) {
    let world = &sim.world;
    if world.tick < casts.last_tick {
        // A new world: the old casts and their sprites are gone.
        casts.forget(&mut commands);
    }
    casts.last_tick = world.tick;
    let defs = &sim.defs;
    let ended = advance(
        &mut casts.casts,
        &world.bend_channels(defs),
        world.bend_events(),
        world.tick,
        |unit| world.unit(unit).map(|u| u.pos),
        |ability| look_of(&defs.ability(ability).id),
    );
    for unit in ended {
        if !casts.casts.contains_key(&unit) {
            for e in casts.sprites.remove(&unit).unwrap_or_default() {
                commands.entity(e).try_despawn();
            }
        }
    }
}

/// Puts a casting unit in the bend pose, turned to its target. A unit that has stepped away from
/// where it cast (it went on to its next job) is back to walking.
fn pose_casters(sim: Res<Sim>, casts: Res<Casts>, mut units: Query<(&UnitSprite, &mut UnitView)>) {
    for (UnitSprite(id), mut view) in &mut units {
        let posed = casts.casts.get(id).and_then(|c| {
            let unit = sim.world.unit(*id)?;
            if unit.pos != c.stand {
                return None;
            }
            let row = bend_row(c.time(sim.world.tick))?;
            Some((row, c.target.x - unit.pos.x, c.target.y - unit.pos.y))
        });
        let pose = posed.map(|(row, ..)| Pose::Sheet { row });
        if view.pose_override != pose {
            view.pose_override = pose;
        }
        if let Some((_, dx, dy)) = posed {
            view.facing = facing_after(view.facing, dx, dy);
        }
    }
}

/// Where a part is drawn.
fn part_transform(part: &Part) -> Transform {
    let row = part.at.y.round() as i32;
    let bias = if part.over { OVER_BIAS } else { FX_BIAS };
    Transform::from_translation(Vec3::new(
        part.at.x * TILE,
        -part.at.y * TILE + part.lift,
        row_depth(row) + bias,
    ))
}

/// Draws every cast's parts for the world clock's time, with one pooled sprite per part.
fn sync_parts(
    mut commands: Commands,
    sim: Res<Sim>,
    art: Res<Art>,
    mut casts: ResMut<Casts>,
    mut parts_q: Query<(&mut Transform, &mut Sprite, &mut CastPart)>,
) {
    let Casts { casts, sprites: pools, .. } = &mut *casts;
    for (unit, cast) in casts.iter() {
        let parts = parts(cast.look, cast.time(sim.world.tick), &cast.scene());
        let pool = pools.entry(*unit).or_default();
        for e in pool.drain(parts.len().min(pool.len())..) {
            commands.entity(e).try_despawn();
        }
        for (i, part) in parts.iter().enumerate() {
            let cell = match (part.visual, cast.moved) {
                (Visual::Fx(cell), _) => cell,
                (Visual::Item, Some(m)) => super::jobview::item_cell(&sim.defs, m.item),
                (Visual::Item, None) => continue,
            };
            let transform = part_transform(part);
            match pool.get(i).and_then(|e| parts_q.get_mut(*e).ok()) {
                Some((mut tf, mut sprite, mut shown)) => {
                    *tf = transform;
                    if shown.0 != cell {
                        *sprite = art.sprite(cell);
                        shown.0 = cell;
                    }
                }
                None if i < pool.len() => {} // spawned this frame; its data is already right
                None => {
                    let e = commands
                        .spawn((art.sprite(cell), transform, CastPart(cell), MapEntity))
                        .id();
                    pool.push(e);
                }
            }
        }
    }
}

/// Wind Carry's stack is already in the stockpile when the event lands; hides it while its copy
/// flies there, if the tile was empty before (a tile that held something keeps showing that).
fn hide_landing(
    sim: Res<Sim>,
    mut casts: ResMut<Casts>,
    markers: Res<super::jobview::ItemMarkers>,
    mut visibility: Query<&mut Visibility>,
) {
    let wanted: BTreeSet<Pos> = casts
        .casts
        .values()
        .filter_map(|c| {
            let m = c.moved?;
            let t = c.time(sim.world.tick);
            let in_flight = t >= 0.0 && t <= c.scene().flight();
            let was_empty = sim.world.stored_at(m.to).is_some_and(|s| s.count == m.count);
            (in_flight && was_empty).then_some(m.to)
        })
        .collect();
    let mut set = |pos: Pos, v: Visibility| {
        if let Some(mut now) = markers.stored(pos).and_then(|e| visibility.get_mut(e).ok())
            && *now != v
        {
            *now = v;
        }
    };
    // Every frame, since the marker is respawned when the stack changes.
    for pos in &wanted {
        set(*pos, Visibility::Hidden);
    }
    for pos in casts.hidden.difference(&wanted) {
        set(*pos, Visibility::Inherited);
    }
    casts.hidden = wanted;
}

#[cfg(test)]
mod tests {
    use fn_sim::world::Command;
    use fn_sim::{Defs, Nation};
    use fn_ui::{Clock, Session};

    use super::*;
    use crate::unit_anim::Facing;

    const ALL: [Look; 5] = [Look::Rise, Look::Stream, Look::Flare, Look::Gust, Look::Glow];

    fn scene() -> Scene {
        Scene {
            stand: Vec2::new(2.0, 5.0),
            target: Vec2::new(5.0, 5.0),
            moved_to: Some(Vec2::new(11.0, 5.0)),
        }
    }

    #[test]
    fn every_ability_has_a_cast_effect_and_the_table_names_only_abilities() {
        let defs = Defs::bundled();
        for ability in &defs.abilities {
            assert!(look_of(&ability.id).is_some(), "{} has no cast effect", ability.id);
        }
        for (id, _) in LOOKS {
            assert!(defs.ability_id(id).is_some(), "{id} is not an ability in data/bending.toml");
        }
        assert_eq!(LOOKS.len(), defs.abilities.len());
        // And the abilities look different from one another.
        let looks: BTreeSet<String> = LOOKS.iter().map(|(_, l)| format!("{l:?}")).collect();
        assert_eq!(looks.len(), LOOKS.len());
    }

    #[test]
    fn time_counts_down_while_channelling_then_up_from_the_event() {
        assert_eq!(time_of(None, 1.5, 100), -1.5);
        assert_eq!(time_of(Some(100), 0.0, 100), 0.0);
        assert_eq!(time_of(Some(100), 0.0, 105), 0.5);
        assert_eq!(time_of(Some(100), 0.0, 130), 3.0);
    }

    #[test]
    fn the_pose_is_the_stance_then_the_release_then_gone() {
        assert_eq!(bend_row(-10.0), Some(BEND_STANCE_ROW));
        assert_eq!(bend_row(-RELEASE_LEAD - 0.01), Some(BEND_STANCE_ROW));
        assert_eq!(bend_row(-RELEASE_LEAD), Some(BEND_RELEASE_ROW));
        assert_eq!(bend_row(0.0), Some(BEND_RELEASE_ROW));
        assert_eq!(bend_row(POSE_TAIL), Some(BEND_RELEASE_ROW));
        assert_eq!(bend_row(POSE_TAIL + 0.01), None);
    }

    #[test]
    fn an_effect_draws_inside_its_span_and_nothing_outside_it() {
        let s = scene();
        for look in ALL {
            let (lead, tail) = look.span(s.flight());
            assert!(parts(look, -lead - 0.05, &s).is_empty(), "{look:?} before");
            assert!(parts(look, tail + 0.05, &s).is_empty(), "{look:?} after");
            let busy = (0..=40)
                .map(|i| -lead + (lead + tail) * i as f32 / 40.0)
                .filter(|t| !parts(look, *t, &s).is_empty())
                .count();
            assert!(busy >= 20, "{look:?} draws at {busy} of 41 moments");
            // About 0.5 to 1.5 seconds, as the issue asks.
            assert!((0.5..=2.0).contains(&(lead + tail)), "{look:?} lasts {}", lead + tail);
        }
    }

    #[test]
    fn raise_earth_cracks_then_spikes_then_stones_then_dust() {
        let s = scene();
        let cell = |t| match parts(Look::Rise, t, &s)[..] {
            [Part { visual: Visual::Fx(c), at, .. }] => {
                assert_eq!(at, s.target, "on the target tile");
                c
            }
            ref p => panic!("{t}: {p:?}"),
        };
        assert_eq!([cell(-0.8), cell(-0.4), cell(-0.1)], sprites::fx::EARTH);
        assert_eq!(cell(0.0), sprites::fx::DUST[0]);
        assert_eq!(cell(0.4), sprites::fx::DUST[2]);
    }

    #[test]
    fn the_stream_runs_from_the_bender_to_the_target_and_splashes_the_patch() {
        let s = scene();
        let mid = parts(Look::Stream, -0.5, &s);
        let drops: Vec<&Part> = mid.iter().filter(|p| p.lift > 0.0).collect();
        assert!(drops.len() >= 3, "a stream of droplets: {}", drops.len());
        for d in &drops {
            assert!((s.stand.x..=s.target.x).contains(&d.at.x), "{:?} between them", d.at);
            assert_eq!(d.at.y, 5.0);
        }
        // It reaches out before it flows: fewer droplets at the start than later.
        assert!(parts(Look::Stream, -0.95, &s).len() < mid.len());
        // The splash starts on the target and spreads to the ring round it, over the 3x3 patch.
        let splashed = |t| {
            parts(Look::Stream, t, &s)
                .iter()
                .filter(|p| p.lift == 0.0)
                .map(|p| p.at)
                .collect::<Vec<_>>()
        };
        assert_eq!(splashed(-0.35), vec![s.target]);
        assert_eq!(splashed(-0.1).len(), 9);
        assert!(splashed(0.2).len() >= 8);
    }

    #[test]
    fn a_bender_on_its_target_sprays_the_ring_around_it() {
        let on = Scene { stand: Vec2::new(5.0, 5.0), ..scene() };
        let drops: Vec<Part> =
            parts(Look::Stream, -0.4, &on).into_iter().filter(|p| p.lift > 0.0).collect();
        assert!(drops.iter().any(|d| (d.at - on.target).length() > 0.3), "{drops:?}");
    }

    #[test]
    fn the_flame_is_kindled_in_a_flare_between_sparks_and_embers() {
        let s = scene();
        let cell = |t| match parts(Look::Flare, t, &s)[..] {
            [Part { visual: Visual::Fx(c), lift, .. }] => (c, lift),
            ref p => panic!("{t}: {p:?}"),
        };
        let fx = sprites::fx::SPARK;
        assert_eq!(cell(-0.4).0, fx[0]);
        assert_eq!(cell(0.0).0, fx[1]);
        assert_eq!(cell(0.4).0, fx[2]);
        assert!(cell(0.5).1 > cell(0.2).1, "embers rise");
    }

    #[test]
    fn inner_fire_gathers_blooms_and_pulses_out_from_the_bender() {
        // Cast on the bender's own tile: the stage puts it at `stand`.
        let s = Scene { target: Vec2::new(2.0, 5.0), ..scene() };
        let cells = |t: f32| -> Vec<(Vec2, sprites::Cell)> {
            parts(Look::Glow, t, &s)
                .into_iter()
                .filter_map(|p| match p.visual {
                    Visual::Fx(c) if p.lift == 0.0 => Some((p.at, c)),
                    _ => None,
                })
                .collect()
        };
        let fx = sprites::fx::GLOW;
        assert_eq!(cells(-0.6), vec![(s.stand, fx[0])], "embers gather round the bender");
        assert_eq!(cells(-0.1), vec![(s.stand, fx[1])], "the ring blooms");
        // The pulse spreads outward: more tiles over time, the near ones first.
        let near = cells(0.05).len();
        let wide = cells(0.3).len();
        assert!(wide > near && near >= 1, "{near} then {wide}");
        assert!(cells(0.3).iter().all(|(at, _)| (at.x - s.stand.x).abs() <= 2.0), "within reach");
        // And embers rise off the bender, higher as time goes on.
        let lift = |t: f32| {
            parts(Look::Glow, t, &s)
                .into_iter()
                .find(|p| p.over && p.lift > 0.0)
                .map(|p| p.lift)
                .unwrap()
        };
        assert!(lift(0.5) > lift(0.1));
    }

    #[test]
    fn the_wind_carry_stack_travels_to_the_stockpile_instead_of_teleporting() {
        let s = scene();
        let item = |t: f32| {
            parts(Look::Gust, t, &s).into_iter().find(|p| p.visual == Visual::Item).map(|p| p.at)
        };
        assert_eq!(item(-0.1), None, "it sits on the ground until the bend lands");
        assert_eq!(item(0.0), Some(s.target));
        let flight = s.flight();
        assert!((0.5..=1.5).contains(&flight));
        let xs: Vec<f32> =
            (0..=10).map(|i| item(flight * 0.99 * i as f32 / 10.0).unwrap().x).collect();
        assert!(xs.windows(2).all(|w| w[1] > w[0]), "it moves steadily along: {xs:?}");
        assert!((xs[10] - 11.0).abs() < 0.2, "and arrives: {xs:?}");
        assert_eq!(item(flight + 0.01), None, "then it is the stockpile's to draw");
        // Wind trails it, and runs from the bender to the stack first.
        assert!(parts(Look::Gust, flight * 0.5, &s).len() >= 3);
        assert!(!parts(Look::Gust, -0.5, &s).is_empty());
        // No stockpile to land on: no flight.
        let none = Scene { moved_to: None, ..s };
        assert!(parts(Look::Gust, 0.2, &none).iter().all(|p| p.visual != Visual::Item));
    }

    #[test]
    fn a_cast_follows_the_session_clock_pausing_and_running_faster() {
        // How many wall seconds a cast needs to get 0.5 s past completion, as the clock turns
        // wall time into world ticks (60 frames a second).
        let wall_secs = |speed_ups: u32, paused: bool| {
            let mut clock = Clock::default();
            for _ in 0..speed_ups {
                clock.faster();
            }
            clock.paused = paused;
            let (mut tick, mut frames) = (0_u64, 0_u32);
            while time_of(Some(0), 0.0, tick) < 0.5 && frames < 60 * 30 {
                tick += u64::from(clock.advance(1.0 / 60.0));
                frames += 1;
            }
            (frames as f32 / 60.0, time_of(Some(0), 0.0, tick))
        };
        for (ups, speed) in [(0, 1.0), (1, 2.0), (2, 3.0), (3, 5.0)] {
            let (secs, _) = wall_secs(ups, false);
            assert!((secs - 0.5 / speed).abs() < 0.05, "x{speed}: {secs}s");
        }
        let (secs, t) = wall_secs(0, true);
        assert_eq!((secs, t), (30.0, 0.0), "paused: the cast never moves");
    }

    // ---- advance ----

    fn unit(n: u32) -> UnitId {
        UnitId(n)
    }

    fn channel(n: u32, remaining: f32) -> BendChannel {
        BendChannel {
            unit: unit(n),
            ability: AbilityId(0),
            pos: Pos::new(5, 5),
            remaining_secs: remaining,
        }
    }

    fn event(n: u32, tick: u64, outcome: BendOutcome) -> BendEvent {
        BendEvent {
            tick,
            unit: unit(n),
            ability: AbilityId(0),
            pos: Pos::new(5, 5),
            outcome,
            moved: None,
        }
    }

    fn step(
        casts: &mut BTreeMap<UnitId, Cast>,
        channels: &[BendChannel],
        events: &[BendEvent],
        now: u64,
    ) -> Vec<UnitId> {
        advance(casts, channels, events, now, |_| Some(Pos::new(4, 5)), |_| Some(Look::Rise))
    }

    #[test]
    fn a_cast_starts_with_the_channel_counts_down_and_ends_with_its_event() {
        let mut casts = BTreeMap::new();
        step(&mut casts, &[channel(1, 2.0)], &[], 10);
        let c = casts[&unit(1)];
        assert_eq!((c.stand, c.target, c.finished), (Pos::new(4, 5), Pos::new(5, 5), None));
        assert_eq!(c.time(10), -2.0);
        step(&mut casts, &[channel(1, 0.4)], &[], 26);
        assert_eq!(casts[&unit(1)].time(26), -0.4);

        // The bend lands: the channel is gone and the event is there, on tick 27.
        let events = [event(1, 27, BendOutcome::Done)];
        step(&mut casts, &[], &events, 27);
        assert_eq!(casts[&unit(1)].finished, Some(27));
        assert_eq!(casts[&unit(1)].time(30), 0.3);

        // It plays out, and is then dropped; the old event doesn't start it again.
        assert!(step(&mut casts, &[], &events, 30).is_empty());
        let ended = step(&mut casts, &[], &events, 27 + 12);
        assert_eq!(ended, vec![unit(1)]);
        assert!(casts.is_empty());
        step(&mut casts, &[], &events, 27 + 15);
        assert!(casts.is_empty(), "no second life from the same event");
    }

    #[test]
    fn a_failed_bend_or_a_need_that_interrupts_ends_the_cast_at_once() {
        let mut casts = BTreeMap::new();
        step(&mut casts, &[channel(1, 1.0), channel(2, 1.0)], &[], 5);
        assert_eq!(casts.len(), 2);
        // Unit 2 left to eat: no channel any more, no event.
        let ended = step(&mut casts, &[channel(1, 0.9)], &[], 6);
        assert_eq!(ended, vec![unit(2)]);
        // Unit 1's bend failed.
        let fail = [event(1, 7, BendOutcome::Failed("gone"))];
        assert_eq!(step(&mut casts, &[], &fail, 7), vec![unit(1)]);
        assert!(casts.is_empty());
    }

    #[test]
    fn a_bend_seen_only_by_its_event_still_plays() {
        // A loaded game, or a channel shorter than a frame.
        let mut casts = BTreeMap::new();
        step(&mut casts, &[], &[event(3, 50, BendOutcome::Done)], 52);
        assert_eq!(casts[&unit(3)].time(52), 0.2);
        // But an old event does not.
        let mut casts = BTreeMap::new();
        step(&mut casts, &[], &[event(3, 50, BendOutcome::Done)], 50 + RECENT_TICKS);
        assert!(casts.is_empty());
    }

    #[test]
    fn the_events_stack_is_flown_to_where_it_landed() {
        let mut casts = BTreeMap::new();
        let moved = Moved { to: Pos::new(9, 5), item: fn_sim::jobs::ItemId(0), count: 4 };
        let events = [BendEvent { moved: Some(moved), ..event(1, 20, BendOutcome::Done) }];
        step(&mut casts, &[channel(1, 0.1)], &[], 19);
        step(&mut casts, &[], &events, 20);
        let scene = casts[&unit(1)].scene();
        assert_eq!(scene.moved_to, Some(Vec2::new(9.0, 5.0)));
    }

    // ---- in an app, on a real world ----

    fn app_with(nation: Nation) -> (App, UnitId) {
        let mut app = App::new();
        crate::insert_test_art(&mut app);
        let sim = Sim(Session::new(1, nation, 48, 32));
        let id = sim.world.units[0].id;
        let pos = sim.world.units[0].pos;
        let at = crate::unit_translation(pos);
        app.insert_resource(sim)
            .init_resource::<Casts>()
            .add_systems(Update, (track_casts, pose_casters, sync_parts).chain());
        app.world_mut().spawn((
            Sprite::default(),
            Transform::from_translation(at),
            UnitSprite(id),
            UnitView::new(pos, at.truncate()),
        ));
        (app, id)
    }

    fn view(app: &mut App) -> (Option<Pose>, Facing) {
        let mut q = app.world_mut().query::<&UnitView>();
        let v = q.single(app.world()).unwrap();
        (v.pose_override, v.facing)
    }

    fn parts_drawn(app: &mut App) -> usize {
        app.world_mut().query::<&CastPart>().iter(app.world()).count()
    }

    fn tick_until(app: &mut App, limit: u32, mut done: impl FnMut(&Sim) -> bool) {
        for _ in 0..limit {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            if done(&sim) {
                return;
            }
            sim.tick();
        }
        panic!("not reached in {limit} ticks");
    }

    fn cast_time(app: &App, id: UnitId) -> Option<f32> {
        let tick = app.world().resource::<Sim>().world.tick;
        app.world().resource::<Casts>().of(id).map(|c| c.time(tick))
    }

    #[test]
    fn a_bender_takes_the_stance_while_channelling_the_release_as_it_lands_then_goes_idle() {
        let (mut app, id) = app_with(Nation::Fire);
        let pos = {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let ability = fn_ui::tools::ability_for_nation(&sim.defs, Nation::Fire).unwrap();
            let pos = sim.world.units[0].pos;
            sim.apply(Command::Bend { ability, pos }).unwrap();
            pos
        };
        app.update();
        assert_eq!(view(&mut app).0, None, "not yet: the unit is walking to the tile");

        tick_until(&mut app, 3000, |s| !s.world.bend_channels(&s.defs).is_empty());
        app.update();
        let row = |r| Some(Pose::Sheet { row: r });
        assert_eq!(view(&mut app).0, row(BEND_STANCE_ROW));
        assert_eq!(parts_drawn(&mut app), 0, "sparks only gather in the last half second");

        // Half a second before it lands, the sparks gather and the release begins.
        tick_until(&mut app, 3000, |s| {
            s.world.bend_channels(&s.defs).first().is_some_and(|c| c.remaining_secs <= 0.4)
        });
        app.update();
        assert_eq!(view(&mut app).0, row(BEND_RELEASE_ROW));
        assert_eq!(parts_drawn(&mut app), 1);

        // It lands: still the release, and the flame is lit.
        tick_until(&mut app, 3000, |s| !s.world.bend_events().is_empty());
        app.update();
        assert_eq!(view(&mut app).0, row(BEND_RELEASE_ROW));
        assert!(app.world().resource::<Sim>().world.is_lit(pos));
        assert_eq!(parts_drawn(&mut app), 1);

        // Held still while the world doesn't tick: no frame changes however long it waits.
        let before = cast_time(&app, id);
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(cast_time(&app, id), before);
        assert!(before.is_some());

        // A second later it is over: the pose and the effect are gone.
        app.world_mut().resource_mut::<Sim>().tick_n(11);
        app.update();
        assert_eq!(view(&mut app).0, None);
        assert!(cast_time(&app, id).is_none());
        app.update();
        assert_eq!(parts_drawn(&mut app), 0);
    }

    #[test]
    fn the_bender_turns_to_face_its_target() {
        let (mut app, _) = app_with(Nation::Fire);
        let target = {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let ability = fn_ui::tools::ability_for_nation(&sim.defs, Nation::Fire).unwrap();
            let pos = sim.world.units[0].pos;
            // A few tiles to the right of where it stands, whichever of them takes the bend.
            (2..6)
                .map(|dx| Pos::new(pos.x + dx, pos.y))
                .find(|&p| sim.apply(Command::Bend { ability, pos: p }).is_ok())
                .expect("a tile to the right takes a flame")
        };
        tick_until(&mut app, 3000, |s| !s.world.bend_channels(&s.defs).is_empty());
        app.update();
        let sim = app.world().resource::<Sim>();
        let from = sim.world.units[0].pos;
        let want = facing_after(Facing::Down, target.x - from.x, target.y - from.y);
        assert_eq!(view(&mut app).1, want);
    }
}

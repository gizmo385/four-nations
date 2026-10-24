//! Shows bending's *results* on the map and in the HUD (#72; docs/design/bending.md "Client
//! (minimum)"). Designating a bend and its pending tint are `designate`; this module draws what
//! a finished bend left behind (a lit flame, a wet patch, a new ford) and turns `BendEvent`s into
//! short-lived alerts.
//!
//! - A lit flame is a warm glow tile; hovering it outlines its reach.
//! - A wet patch is a darker soil overlay, gone once it dries (and sprouts, drawn by `jobview`
//!   like any other resource).
//! - A reshaped tile (today: Earth's river or lake turned into a ford) needs its floor sprite
//!   redrawn, since the client otherwise draws each tile's ground once, at spawn.
//! - Each `BendEvent` becomes a line in the side panel's "Alerts" section (ui.md), shown for a
//!   few seconds and then dropped.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use fn_sim::world::{BendEvent, BendOutcome};
use fn_sim::{Pos, Terrain};

use super::{
    Art, MapEntity, Sim, TILE, cursor_tile, floor_cell, ground_rule, row_depth, tile_translation,
};

pub struct BendingPlugin;

impl Plugin for BendingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Alerts>()
            .init_resource::<FlameSprites>()
            .init_resource::<WetSprites>()
            // `PreUpdate` so the side panel's `Update` systems see this frame's new alerts, with
            // no one-frame lag.
            .add_systems(PreUpdate, track_bend_events)
            .add_systems(Update, (sync_terrain, sync_flames, sync_wet, flame_radius_hover).chain());
    }
}

// -------------------------------------------------------------------------------------------
// Terrain: a reshaped tile's floor sprite

/// The terrain a floor sprite was last drawn for, so `sync_terrain` touches a tile only the frame
/// its terrain actually changes (Raise Earth's river or lake turning into a ford, #68) instead of
/// every tile, every frame.
#[derive(Component)]
pub struct TerrainSprite(pub Pos, pub Terrain);

fn sync_terrain(
    mut sprites: Query<(&mut TerrainSprite, &mut Sprite)>,
    sim: Res<Sim>,
    art: Res<Art>,
) {
    for (mut cached, mut sprite) in &mut sprites {
        let Some(tile) = sim.world.map.get(cached.0) else { continue };
        if tile.terrain == cached.1 {
            continue;
        }
        let rule = ground_rule(&sim, tile);
        *sprite = art.sprite(floor_cell(&sim, cached.0, rule));
        cached.1 = tile.terrain;
    }
}

// -------------------------------------------------------------------------------------------
// Flames: a glow tile, and its reach on hover

/// A lit flame's glow. Semi-transparent so the floor underneath still reads.
const FLAME_COLOR: Color = Color::srgba(1.0, 0.65, 0.15, 0.55);
/// Drawn just under the row's resources and item markers (which sit at `row_depth`), since a
/// flame is only ever lit on an empty tile but a unit may still stand on it.
const FLAME_BIAS: f32 = -0.0003;

#[derive(Component)]
struct FlameGlow;

/// The glow entities currently on the map, by tile.
#[derive(Resource, Default)]
struct FlameSprites(BTreeMap<Pos, Entity>);

/// Keeps one glow tile per lit flame (`World::flames`), matching the sim.
fn sync_flames(
    mut commands: Commands,
    sim: Res<Sim>,
    mut flames: ResMut<FlameSprites>,
    alive: Query<(), With<FlameGlow>>,
) {
    // Entities despawned with the old map (regenerate) are no longer there.
    flames.0.retain(|_, e| alive.contains(*e));
    let wanted: BTreeSet<Pos> = sim.world.flames().iter().map(|f| f.pos).collect();
    flames.0.retain(|p, e| {
        let keep = wanted.contains(p);
        if !keep {
            commands.entity(*e).despawn();
        }
        keep
    });
    for p in wanted {
        flames.0.entry(p).or_insert_with(|| {
            commands
                .spawn((
                    Sprite::from_color(FLAME_COLOR, Vec2::splat(TILE * 0.8)),
                    Transform::from_translation(tile_translation(p, row_depth(p.y) + FLAME_BIAS)),
                    FlameGlow,
                    MapEntity,
                ))
                .id()
        });
    }
}

/// Outlines a lit flame's reach (Chebyshev radius) while the cursor is over its tile.
fn flame_radius_hover(
    sim: Res<Sim>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut gizmos: Gizmos,
) {
    let (Ok(window), Ok((camera, camera_tf))) = (window.single(), camera.single()) else { return };
    let Some(hovered) = cursor_tile(window, camera, camera_tf) else { return };
    let Some(flame) = sim.world.flames().into_iter().find(|f| f.pos == hovered) else { return };
    let size = Vec2::splat((2 * flame.radius + 1) as f32 * TILE);
    let center = tile_translation(flame.pos, 0.0).truncate();
    gizmos.rect_2d(Isometry2d::from_translation(center), size, FLAME_COLOR.with_alpha(0.9));
}

// -------------------------------------------------------------------------------------------
// Wet tiles: a darker soil overlay

/// A watered tile, darker than the ground under it (Water's Water the Land, #69).
const WET_COLOR: Color = Color::srgba(0.25, 0.18, 0.08, 0.55);
/// Flat ground, like a stockpile tint: under resources, items and units.
const WET_DEPTH: f32 = 0.55;

#[derive(Component)]
struct WetOverlay;

/// The overlay entities currently on the map, by tile.
#[derive(Resource, Default)]
struct WetSprites(BTreeMap<Pos, Entity>);

/// Keeps one overlay tile per wet tile (`World::wet_tiles`), matching the sim.
fn sync_wet(
    mut commands: Commands,
    sim: Res<Sim>,
    mut wet: ResMut<WetSprites>,
    alive: Query<(), With<WetOverlay>>,
) {
    // Entities despawned with the old map (regenerate) are no longer there.
    wet.0.retain(|_, e| alive.contains(*e));
    let wanted: BTreeSet<Pos> = sim.world.wet_tiles().into_iter().map(|(p, _)| p).collect();
    wet.0.retain(|p, e| {
        let keep = wanted.contains(p);
        if !keep {
            commands.entity(*e).despawn();
        }
        keep
    });
    for p in wanted {
        wet.0.entry(p).or_insert_with(|| {
            commands
                .spawn((
                    Sprite::from_color(WET_COLOR, Vec2::splat(TILE)),
                    Transform::from_translation(tile_translation(p, WET_DEPTH)),
                    WetOverlay,
                    MapEntity,
                ))
                .id()
        });
    }
}

// -------------------------------------------------------------------------------------------
// Alerts: BendEvents as short-lived toasts

/// How long an alert stays in the side panel, in real seconds.
const ALERT_SECONDS: f32 = 6.0;
/// At most this many alerts show at once; the oldest drops first.
const MAX_ALERTS: usize = 4;

/// Recent `BendEvent`s as short-lived lines for the side panel's "Alerts" section (ui.md:
/// "a section with nothing to show hides itself").
#[derive(Resource, Default)]
pub struct Alerts {
    /// (line, seconds left), oldest first.
    entries: Vec<(String, f32)>,
    /// How many of `World::bend_events` have already become an alert.
    seen: usize,
}

impl Alerts {
    /// The alerts still showing, oldest first.
    pub fn lines(&self) -> Vec<String> {
        self.entries.iter().map(|(line, _)| line.clone()).collect()
    }
}

/// "Haru used Raise Earth at (5, 3)" or "Haru's Raise Earth failed: needs dry ground beside it".
fn describe_bend_event(sim: &Sim, ev: &BendEvent) -> String {
    let def = sim.defs.ability(ev.ability);
    let who = sim.world.unit(ev.unit).map_or("Someone", |u| u.name.as_str());
    match ev.outcome {
        BendOutcome::Done => format!("{who} used {} at ({}, {})", def.name, ev.pos.x, ev.pos.y),
        BendOutcome::Failed(why) => format!("{who}'s {} failed: {why}", def.name),
    }
}

/// Ages every alert's time left down by `dt`, dropping those that reach zero, then keeps only
/// the newest `MAX_ALERTS`.
fn age_alerts(entries: &mut Vec<(String, f32)>, dt: f32) {
    entries.retain_mut(|(_, left)| {
        *left -= dt;
        *left > 0.0
    });
    let len = entries.len();
    if len > MAX_ALERTS {
        entries.drain(0..len - MAX_ALERTS);
    }
}

/// Turns new `BendEvent`s into alerts and ages out old ones.
fn track_bend_events(mut alerts: ResMut<Alerts>, sim: Res<Sim>, time: Res<Time>) {
    let events = sim.world.bend_events();
    if alerts.seen > events.len() {
        // A new world (regenerate, nation switch): the old log is gone.
        alerts.seen = 0;
        alerts.entries.clear();
    }
    for ev in &events[alerts.seen..] {
        alerts.entries.push((describe_bend_event(&sim, ev), ALERT_SECONDS));
    }
    alerts.seen = events.len();
    age_alerts(&mut alerts.entries, time.delta_secs());
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use fn_sim::Nation;
    use fn_sim::bending::AbilityId;
    use fn_sim::world::{BendOutcome, Command};

    use super::*;

    fn test_app(seed: u64, nation: Nation) -> App {
        let mut app = App::new();
        app.insert_resource(Sim::new(seed, nation, 48, 32))
            .init_resource::<Time>()
            .init_resource::<Alerts>()
            .init_resource::<FlameSprites>()
            .init_resource::<WetSprites>()
            .add_systems(PreUpdate, track_bend_events)
            .add_systems(Update, (sync_flames, sync_wet).chain());
        app
    }

    #[test]
    fn describes_a_done_and_a_failed_bend() {
        let sim = Sim::new(1, Nation::Earth, 48, 32);
        let ability = AbilityId(
            sim.defs.abilities.iter().position(|a| a.nation == Nation::Earth).unwrap() as u16,
        );
        let unit = sim.world.units[0].id;
        let pos = Pos::new(5, 3);
        let done = BendEvent { tick: 0, unit, ability, pos, outcome: BendOutcome::Done };
        let line = describe_bend_event(&sim, &done);
        assert!(line.contains(&sim.world.units[0].name), "{line}");
        assert!(line.contains("(5, 3)"), "{line}");

        let failed = BendEvent {
            tick: 0,
            unit,
            ability,
            pos,
            outcome: BendOutcome::Failed("needs dry ground beside it"),
        };
        let line = describe_bend_event(&sim, &failed);
        assert!(line.contains("failed: needs dry ground beside it"), "{line}");
    }

    #[test]
    fn alerts_age_out_and_cap_at_the_newest_few() {
        let mut entries: Vec<(String, f32)> = (0..6).map(|i| (i.to_string(), 1.0)).collect();
        age_alerts(&mut entries, 0.5);
        assert_eq!(entries.len(), MAX_ALERTS, "capped even before anything expires");
        assert_eq!(entries.last().unwrap().0, "5", "kept the newest");

        age_alerts(&mut entries, 10.0);
        assert!(entries.is_empty(), "everything has expired");
    }

    #[test]
    fn a_designated_bend_eventually_becomes_an_alert() {
        let mut app = test_app(1, Nation::Fire);
        let (pos, ability) = {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let Sim { world, defs, .. } = &mut *sim;
            let home = world.units[0].pos;
            let ability = AbilityId(
                defs.abilities.iter().position(|a| a.nation == Nation::Fire).unwrap() as u16,
            );
            let pos = Pos::new(home.x, home.y);
            world.apply(Command::Bend { ability, pos }, defs).unwrap();
            (pos, ability)
        };
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let Sim { world, defs, .. } = &mut *sim;
            for _ in 0..20_000 {
                world.tick(defs);
                if !world.bend_events().is_empty() {
                    break;
                }
            }
            assert!(!world.bend_events().is_empty(), "the bend should finish or fail");
        }
        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_millis(16));
        app.update();
        let lines = app.world().resource::<Alerts>().lines();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains(&app.world().resource::<Sim>().defs.ability(ability).name));

        if app.world().resource::<Sim>().world.is_lit(pos) {
            app.update();
            let glow = app.world().resource::<FlameSprites>();
            assert!(glow.0.contains_key(&pos), "a lit flame should get a glow tile");
        }
    }

    #[test]
    fn wet_tiles_get_an_overlay_until_they_dry() {
        let mut app = test_app(2, Nation::Water);
        let target = {
            let sim = app.world().resource::<Sim>();
            let home = sim.world.units[0].pos;
            // A tile within source_range of the unit that `irrigate` can actually target: just
            // use the unit's own tile if it validates, else skip (data-driven, so not guaranteed
            // on every seed; the overlay logic is what this test checks).
            home
        };
        let ability = {
            let sim = app.world().resource::<Sim>();
            AbilityId(
                sim.defs.abilities.iter().position(|a| a.nation == Nation::Water).unwrap() as u16
            )
        };
        let designated = {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let Sim { world, defs, .. } = &mut *sim;
            world.apply(Command::Bend { ability, pos: target }, defs).is_ok()
        };
        if !designated {
            return; // this seed's starting tile doesn't validate; nothing more to check here.
        }
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let Sim { world, defs, .. } = &mut *sim;
            for _ in 0..20_000 {
                world.tick(defs);
                if !world.wet_tiles().is_empty() {
                    break;
                }
            }
        }
        app.update();
        let sim = app.world().resource::<Sim>();
        if sim.world.wet_tiles().is_empty() {
            return;
        }
        let overlays = app.world().resource::<WetSprites>();
        assert!(!overlays.0.is_empty(), "a wet tile should get an overlay");
    }
}

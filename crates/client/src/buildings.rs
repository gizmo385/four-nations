//! Draws buildings (#149): a finished building as its sprite, and a build site as the same sprite
//! faded by how far the work has got, over the site's footprint tint (`designate`).
//!
//! Which sprite a building uses is data (`BuildingDef::art_for`, `sprites::buildings`), and a
//! building with no art yet draws the fallback signboard. Sprites are 2x2 cells, bottom-aligned to
//! their tile and sorted by `row_depth`, like trees, so a unit standing behind a hall is hidden by
//! its roof and one on a passable building (a bed, a campfire) stands in front of it.
//!
//! A multi-frame sprite (the campfire's flames) plays when finished, each tile out of step with
//! its neighbours so two fires don't flicker in unison. A site shows its first frame.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use fn_sim::Pos;
use fn_sim::building::BuildingId;

use super::sprites::{self, Cell};
use super::{Art, MapEntity, Sim, TILE, row_depth, tile_translation};

pub struct BuildingsPlugin;

impl Plugin for BuildingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BuildingSprites>()
            .add_systems(Update, (sync_buildings, animate_buildings).chain());
    }
}

/// A site's sprite is this opaque before any work is done, and this opaque when it is nearly
/// finished (a finished building is fully opaque).
const SITE_ALPHA_MIN: f32 = 0.3;
const SITE_ALPHA_MAX: f32 = 0.8;

/// The sprite of the building on a tile: finished, or a site (`site`) with its progress.
#[derive(Component)]
struct BuildingSprite {
    building: BuildingId,
    frames: &'static [Cell],
    /// Offsets the frame, so neighbouring fires are out of step.
    phase: usize,
    /// The frame shown now, so the sprite is touched only when it changes.
    shown: usize,
}

/// The sprite entity of each tile that has a building or a site on it.
#[derive(Resource, Default)]
pub struct BuildingSprites(BTreeMap<Pos, Entity>);

/// How opaque a sprite is: a finished building (`None`) fully, a site by `progress`, the share of
/// the work done (0 to 1).
fn alpha(progress: Option<f32>) -> f32 {
    match progress {
        None => 1.0,
        Some(p) => SITE_ALPHA_MIN + (SITE_ALPHA_MAX - SITE_ALPHA_MIN) * p.clamp(0.0, 1.0),
    }
}

/// Keeps one sprite per finished building and per build site, matching the sim.
fn sync_buildings(
    mut commands: Commands,
    sim: Res<Sim>,
    art: Res<Art>,
    mut drawn: ResMut<BuildingSprites>,
    mut sprites: Query<(&mut Sprite, &BuildingSprite)>,
) {
    // Where something is drawn, and how opaque: a site by its progress, a finished building not
    // at all. A finished building wins over a site on the same tile (there can't be both).
    let mut wanted: BTreeMap<Pos, (BuildingId, Option<f32>)> = sim
        .world
        .build_sites()
        .map(|(p, id, work)| (p, (id, Some(work / sim.defs.building(id).work))))
        .collect();
    wanted
        .extend(sim.world.map.iter().filter_map(|(p, t)| Some((p, (t.building?.id, None::<f32>)))));

    // Entities despawned with the old map (regenerate) are no longer there, and a tile whose
    // building changed (a site finished into another one's tile, say) is drawn afresh.
    drawn.0.retain(|pos, e| {
        let Ok((_, sprite)) = sprites.get(*e) else { return false };
        let keep = wanted.get(pos).is_some_and(|(id, _)| *id == sprite.building);
        if !keep {
            commands.entity(*e).despawn();
        }
        keep
    });

    for (pos, (id, progress)) in wanted {
        let color = Color::srgba(1.0, 1.0, 1.0, alpha(progress));
        if let Some(&e) = drawn.0.get(&pos) {
            if let Ok((mut sprite, _)) = sprites.get_mut(e)
                && sprite.color != color
            {
                sprite.color = color;
            }
            continue;
        }
        let frames = sprites::buildings::frames(sim.defs.building(id).art_for(sim.world.nation));
        let mut sprite = art.sprite(frames[0]);
        sprite.color = color;
        // The anchor is the bottom middle of the tile, so a sprite of any height stands on it.
        let mut at = tile_translation(pos, row_depth(pos.y));
        at.y -= TILE / 2.0;
        let phase =
            (pos.x as usize).wrapping_mul(7).wrapping_add((pos.y as usize).wrapping_mul(13));
        let entity = commands
            .spawn((
                sprite,
                Anchor::BOTTOM_CENTER,
                Transform::from_translation(at),
                BuildingSprite { building: id, frames, phase, shown: 0 },
                MapEntity,
            ))
            .id();
        drawn.0.insert(pos, entity);
    }
}

/// The frame of a `len`-frame sprite to show `elapsed` seconds in, for a tile with `phase`.
fn frame_index(elapsed: f32, phase: usize, len: usize) -> usize {
    ((elapsed * sprites::buildings::FRAMES_PER_SECOND) as usize + phase) % len
}

/// Plays the frames of a finished building that has several. A site (translucent) stays on its
/// first frame: nothing burns there yet.
fn animate_buildings(time: Res<Time>, mut sprites: Query<(&mut Sprite, &mut BuildingSprite)>) {
    for (mut sprite, mut building) in &mut sprites {
        if building.frames.len() < 2 || sprite.color.alpha() < 1.0 {
            continue;
        }
        let frame = frame_index(time.elapsed_secs(), building.phase, building.frames.len());
        if frame != building.shown {
            building.shown = frame;
            let [x0, y0, x1, y1] = building.frames[frame].pixel_rect();
            sprite.rect = Some(Rect::new(x0, y0, x1, y1));
        }
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::world::Command;
    use fn_sim::{Nation, Terrain};
    use fn_ui::Session;

    use super::*;

    /// A test app with the sprite sync and a stand-in `Art` (no window or GPU: the handles are
    /// never loaded, only compared).
    fn app(nation: Nation) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .insert_resource(Sim(Session::new(7, nation, 64, 64)))
            .init_resource::<BuildingSprites>()
            .add_systems(Update, (sync_buildings, animate_buildings).chain());
        let art = Art::new(app.world().resource::<AssetServer>().clone());
        app.insert_resource(art);
        app
    }

    /// A free land tile with no unit on it.
    fn free_tile(sim: &Sim) -> Pos {
        sim.world
            .map
            .iter()
            .find(|(p, t)| {
                t.terrain == Terrain::Land
                    && t.resource.is_none()
                    && !sim.world.units.iter().any(|u| u.pos == *p)
            })
            .map(|(p, _)| p)
            .unwrap()
    }

    fn sprite_at(app: &App, pos: Pos) -> Option<(Entity, &Sprite, &BuildingSprite)> {
        let e = *app.world().resource::<BuildingSprites>().0.get(&pos)?;
        let world = app.world();
        Some((e, world.get::<Sprite>(e)?, world.get::<BuildingSprite>(e)?))
    }

    #[test]
    fn a_site_is_drawn_translucent_and_vanishes_when_cancelled() {
        let mut app = app(Nation::Earth);
        let bed = app.world().resource::<Sim>().defs.building_id("bed").unwrap();
        let pos = free_tile(app.world().resource::<Sim>());
        app.world_mut()
            .resource_mut::<Sim>()
            .apply(Command::DesignateBuild { pos, building: bed })
            .unwrap();
        app.update();
        let (_, sprite, building) = sprite_at(&app, pos).expect("a site has a sprite");
        assert_eq!(building.building, bed);
        assert!((sprite.color.alpha() - SITE_ALPHA_MIN).abs() < 1e-6, "no work done yet");
        assert_eq!(sprite.rect.map(|r| r.size()), Some(Vec2::splat(32.0)));

        app.world_mut().resource_mut::<Sim>().apply(Command::CancelBuild { pos }).unwrap();
        app.update();
        assert!(sprite_at(&app, pos).is_none());
    }

    #[test]
    fn a_finished_building_is_drawn_opaque_with_its_nations_art() {
        for nation in Nation::ALL {
            let mut app = app(nation);
            let hall = app.world().resource::<Sim>().defs.building_id("study_hall").unwrap();
            let pos = free_tile(app.world().resource::<Sim>());
            app.world_mut().resource_mut::<Sim>().world.map.get_mut(pos).unwrap().building =
                Some(fn_sim::building::Building { id: hall, passable: true });
            app.update();
            let (_, sprite, building) = sprite_at(&app, pos).expect("a finished building");
            assert_eq!(sprite.color.alpha(), 1.0);
            let want = app.world().resource::<Sim>().defs.building(hall).art_for(nation);
            assert_eq!(building.frames, sprites::buildings::frames(want), "{nation:?}");
            assert_ne!(building.frames, &[sprites::buildings::UNFINISHED], "{nation:?}");
        }
    }

    #[test]
    fn a_building_with_no_art_yet_draws_the_fallback() {
        // Whichever building is still waiting for its art; once every one has it, nothing to test.
        let Some(waiting) = sprites::buildings::NO_ART_YET.first() else { return };
        let mut app = app(Nation::Earth);
        let wall = app.world().resource::<Sim>().defs.building_id(waiting).unwrap();
        let pos = free_tile(app.world().resource::<Sim>());
        app.world_mut().resource_mut::<Sim>().world.map.get_mut(pos).unwrap().building =
            Some(fn_sim::building::Building { id: wall, passable: false });
        app.update();
        let (_, _, building) = sprite_at(&app, pos).expect("drawn anyway");
        assert_eq!(building.frames, &[sprites::buildings::UNFINISHED]);
    }

    #[test]
    fn a_site_that_finishes_becomes_opaque_in_place() {
        let mut app = app(Nation::Fire);
        let bed = app.world().resource::<Sim>().defs.building_id("bed").unwrap();
        let pos = free_tile(app.world().resource::<Sim>());
        app.world_mut()
            .resource_mut::<Sim>()
            .apply(Command::DesignateBuild { pos, building: bed })
            .unwrap();
        app.update();
        let (site, _, _) = sprite_at(&app, pos).unwrap();
        // The sim finishes the site: the tile gets the building and the site goes.
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            sim.apply(Command::CancelBuild { pos }).unwrap();
            sim.world.map.get_mut(pos).unwrap().building =
                Some(fn_sim::building::Building { id: bed, passable: true });
        }
        app.update();
        let (done, sprite, _) = sprite_at(&app, pos).unwrap();
        assert_eq!(sprite.color.alpha(), 1.0);
        assert_eq!(site, done, "the same building: the same sprite, now opaque");
    }

    #[test]
    fn progress_fades_a_site_in() {
        assert_eq!(alpha(None), 1.0);
        assert_eq!(alpha(Some(0.0)), SITE_ALPHA_MIN);
        assert_eq!(alpha(Some(1.0)), SITE_ALPHA_MAX);
        assert!(alpha(Some(0.5)) > alpha(Some(0.25)));
        assert_eq!(alpha(Some(7.0)), SITE_ALPHA_MAX, "clamped");
    }

    #[test]
    fn the_flames_play_out_of_step_between_tiles() {
        let at = |t: f32, phase| frame_index(t, phase, 3);
        assert_eq!(frame_index(9.0, 4, 1), 0, "one frame is always frame 0");
        let seen: std::collections::BTreeSet<_> = (0..12).map(|i| at(i as f32 * 0.1, 0)).collect();
        assert_eq!(seen.len(), 3, "all three frames play");
        assert_ne!(at(0.0, 0), at(0.0, 1), "a neighbouring fire is a frame ahead");
    }
}

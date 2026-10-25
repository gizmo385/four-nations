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
//!
//! The rail is the one building whose picture depends on its neighbours (#135): its sixteen
//! frames are pieces of track, and a rail tile (or rail site) draws the piece that joins the rail
//! tiles beside it, so a line of rail reads as one track and a corner turns. The sim does not know
//! about this; it only says which tiles are rail.

use std::collections::{BTreeMap, BTreeSet};

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
    /// Whether the frames are pieces of track chosen by neighbours, not an animation.
    connects: bool,
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
    mut sprites: Query<(&mut Sprite, &mut BuildingSprite)>,
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

    // Which tiles are rail, built or planned, for the pieces of track to join up.
    let rails: BTreeSet<Pos> = wanted
        .iter()
        .filter(|(_, (id, _))| sim.defs.building(*id).rail.is_some())
        .map(|(&p, _)| p)
        .collect();

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
        let art_name = sim.defs.building(id).art_for(sim.world.nation);
        let connects = sprites::buildings::connects(art_name);
        let links = if connects { links(&rails, pos) } else { 0 };
        if let Some(&e) = drawn.0.get(&pos) {
            if let Ok((mut sprite, mut building)) = sprites.get_mut(e) {
                if sprite.color != color {
                    sprite.color = color;
                }
                if connects && building.shown != usize::from(links) {
                    // A neighbour was laid or cancelled: turn this piece to join it.
                    building.shown = usize::from(links);
                    let cell = sprites::buildings::link_frame(building.frames, links);
                    let [x0, y0, x1, y1] = cell.pixel_rect();
                    sprite.rect = Some(Rect::new(x0, y0, x1, y1));
                }
            }
            continue;
        }
        let frames = sprites::buildings::frames(art_name);
        let first =
            if connects { sprites::buildings::link_frame(frames, links) } else { frames[0] };
        let mut sprite = art.sprite(first);
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
                BuildingSprite { building: id, frames, phase, shown: usize::from(links), connects },
                MapEntity,
            ))
            .id();
        drawn.0.insert(pos, entity);
    }
}

/// Which of the four neighbours of `pos` are rail, as the `LINK_*` bits of a piece of track.
fn links(rails: &BTreeSet<Pos>, pos: Pos) -> u8 {
    use sprites::buildings::{LINK_E, LINK_N, LINK_S, LINK_W};
    [((0, -1), LINK_N), ((1, 0), LINK_E), ((0, 1), LINK_S), ((-1, 0), LINK_W)]
        .into_iter()
        .filter(|((dx, dy), _)| rails.contains(&Pos::new(pos.x + dx, pos.y + dy)))
        .fold(0, |mask, (_, bit)| mask | bit)
}

/// The frame of a `len`-frame sprite to show `elapsed` seconds in, for a tile with `phase`.
fn frame_index(elapsed: f32, phase: usize, len: usize) -> usize {
    ((elapsed * sprites::buildings::FRAMES_PER_SECOND) as usize + phase) % len
}

/// Plays the frames of a finished building that has several. A site (translucent) stays on its
/// first frame: nothing burns there yet.
fn animate_buildings(time: Res<Time>, mut sprites: Query<(&mut Sprite, &mut BuildingSprite)>) {
    for (mut sprite, mut building) in &mut sprites {
        if building.connects || building.frames.len() < 2 || sprite.color.alpha() < 1.0 {
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
                Some(fn_sim::building::Building { id: hall, passable: true, rail: false });
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
            Some(fn_sim::building::Building { id: wall, passable: false, rail: false });
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
                Some(fn_sim::building::Building { id: bed, passable: true, rail: false });
        }
        app.update();
        let (done, sprite, _) = sprite_at(&app, pos).unwrap();
        assert_eq!(sprite.color.alpha(), 1.0);
        assert_eq!(site, done, "the same building: the same sprite, now opaque");
    }

    #[test]
    fn rail_draws_the_piece_that_joins_the_rail_beside_it() {
        use sprites::buildings::{LINK_E, LINK_N, LINK_S, link_frame};
        let mut app = app(Nation::Earth);
        let rail = app.world().resource::<Sim>().defs.building_id("rail").unwrap();
        // Terrain does not matter to the sprite; keep clear of the map's edge for the neighbours.
        let free = free_tile(app.world().resource::<Sim>());
        let pos = Pos::new(free.x.clamp(2, 60), free.y.clamp(2, 60));
        let lay = |app: &mut App, at: Pos| {
            app.world_mut().resource_mut::<Sim>().world.map.get_mut(at).unwrap().building =
                Some(fn_sim::building::Building { id: rail, passable: true, rail: true });
        };
        let piece = |app: &App, at: Pos| {
            let (_, sprite, building) = sprite_at(app, at).expect("rail is drawn");
            assert!(building.connects);
            sprite.rect
        };
        let frames = sprites::buildings::frames("rail");
        let want = |links| {
            let [x0, y0, x1, y1] = link_frame(frames, links).pixel_rect();
            Some(Rect::new(x0, y0, x1, y1))
        };

        lay(&mut app, pos);
        app.update();
        assert_eq!(piece(&app, pos), want(0), "alone it is a stub");

        // Rail beside it turns it to join: east, then also north, then south too.
        let east = Pos::new(pos.x + 1, pos.y);
        lay(&mut app, east);
        app.update();
        assert_eq!(piece(&app, pos), want(LINK_E));
        assert_eq!(piece(&app, east), want(sprites::buildings::LINK_W), "and it joins back");
        let north = Pos::new(pos.x, pos.y - 1);
        lay(&mut app, north);
        app.update();
        assert_eq!(piece(&app, pos), want(LINK_N | LINK_E));
        let south = Pos::new(pos.x, pos.y + 1);
        lay(&mut app, south);
        app.update();
        assert_eq!(piece(&app, pos), want(LINK_N | LINK_E | LINK_S));
        // Other buildings never join: a bed next to the rail doesn't turn it.
        let bed = app.world().resource::<Sim>().defs.building_id("bed").unwrap();
        let west = Pos::new(pos.x - 1, pos.y);
        app.world_mut().resource_mut::<Sim>().world.map.get_mut(west).unwrap().building =
            Some(fn_sim::building::Building { id: bed, passable: true, rail: false });
        app.update();
        assert_eq!(piece(&app, pos), want(LINK_N | LINK_E | LINK_S));
        // The rail is drawn the same whatever the time: pieces do not animate.
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(piece(&app, pos), want(LINK_N | LINK_E | LINK_S));
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

//! Draws buildings (#149): a finished building as its sprite, and a build site as the same sprite
//! faded by how far the work has got, over the site's footprint tint (`designate`).
//!
//! Which sprite a building uses is data (`BuildingDef::art_for`, `sprites::buildings`), and a
//! building with no art yet draws the fallback signboard. A building covers a footprint of tiles
//! (#196, docs/design/buildings.md, "Drawing") and is drawn once, with its sprite's bottom-centre
//! on the bottom-middle of the footprint and sorted by the `row_depth` of the footprint's front
//! row, like a tree on its tile: a unit standing behind a hall is hidden by its roof, and one on
//! a passable building (a bed, a campfire) stands in front of it. Hovering any tile of a building
//! or a site outlines its whole footprint.
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
use bevy::window::PrimaryWindow;
use fn_sim::Pos;
use fn_sim::building::BuildingId;
use fn_sim::geom::Footprint;

use super::canopy::{BuildingFade, CanopySet};
use super::sprites::{self, Cell};
use super::{Art, MapEntity, Sim, TILE, cursor_tile, row_depth, tile_translation};

pub struct BuildingsPlugin;

impl Plugin for BuildingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BuildingSprites>().add_systems(
            Update,
            ((sync_buildings, animate_buildings).chain().before(CanopySet), outline_hovered),
        );
    }
}

/// The colour of the outline round a hovered building or site.
const HOVER_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.7);

/// A site's sprite is this opaque before any work is done, and this opaque when it is nearly
/// finished (a finished building is fully opaque).
const SITE_ALPHA_MIN: f32 = 0.3;
const SITE_ALPHA_MAX: f32 = 0.8;

/// The sprite of a building: finished, or a site with its progress.
#[derive(Component)]
pub struct BuildingSprite {
    building: BuildingId,
    /// The tiles it covers; a change (a grandfathered 1x1 replaced by a bigger one) redraws it.
    footprint: Footprint,
    frames: &'static [Cell],
    /// Offsets the frame, so neighbouring fires are out of step.
    phase: usize,
    /// The frame shown now, so the sprite is touched only when it changes.
    shown: usize,
    /// Whether the frames are pieces of track chosen by neighbours, not an animation.
    connects: bool,
}

/// The sprite entity of each building and site, by its anchor.
#[derive(Resource, Default)]
pub struct BuildingSprites(BTreeMap<Pos, Entity>);

/// Where a sprite stands on `footprint`: the middle of its bottom edge, in world pixels, at the
/// depth of its front row. For a 1x1 that is the bottom middle of its tile.
pub fn footprint_foot(footprint: Footprint) -> Vec3 {
    let front = footprint.anchor.y + i32::from(footprint.h) - 1;
    let mut at = tile_translation(Pos::new(footprint.anchor.x, front), row_depth(front));
    at.x += (f32::from(footprint.w) - 1.0) * TILE / 2.0;
    at.y -= TILE / 2.0;
    at
}

/// The world rectangle `footprint` covers: its centre and size, for an outline.
pub fn footprint_rect(footprint: Footprint) -> (Vec2, Vec2) {
    let size = Vec2::new(f32::from(footprint.w), f32::from(footprint.h)) * TILE;
    let foot = footprint_foot(footprint).truncate();
    (foot + Vec2::Y * size.y / 2.0, size)
}

/// Whether a building on `footprint` drawn with `cell` should fade like a tree canopy (#77) when
/// it may hide something: if its sprite is bigger than its footprint (it overhangs the tiles
/// behind) or units walk on it. A solid building whose sprite fills its footprint hides nothing.
pub fn fades(footprint: Footprint, cell: Cell, walkable: bool) -> bool {
    walkable || cell.w > u32::from(footprint.w) || cell.h > u32::from(footprint.h)
}

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
    // What is drawn, by anchor, and how opaque: a site by its progress, a finished building not
    // at all. A finished building wins over a site on the same anchor (there can't be both).
    let mut wanted: BTreeMap<Pos, (BuildingId, Footprint, Option<f32>)> = sim
        .world
        .build_sites()
        .map(|(f, id, work)| (f.anchor, (id, f, Some(work / sim.defs.building(id).work))))
        .collect();
    wanted.extend(sim.world.buildings().map(|(f, id)| (f.anchor, (id, f, None::<f32>))));

    // Which tiles are rail, built or planned, for the pieces of track to join up.
    let rails: BTreeSet<Pos> = wanted
        .iter()
        .filter(|(_, (id, _, _))| sim.defs.building(*id).rail.is_some())
        .map(|(&p, _)| p)
        .collect();

    // Entities despawned with the old map (regenerate) are no longer there, and an anchor whose
    // building changed (a site finished into another one's place, say) is drawn afresh.
    drawn.0.retain(|pos, e| {
        let Ok((_, sprite)) = sprites.get(*e) else { return false };
        let keep = wanted
            .get(pos)
            .is_some_and(|(id, f, _)| *id == sprite.building && *f == sprite.footprint);
        if !keep {
            commands.entity(*e).despawn();
        }
        keep
    });

    for (pos, (id, footprint, progress)) in wanted {
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
        // Anchored at its bottom middle, so a sprite of any height stands on the footprint.
        let at = footprint_foot(footprint);
        let phase =
            (pos.x as usize).wrapping_mul(7).wrapping_add((pos.y as usize).wrapping_mul(13));
        let mut entity = commands.spawn((
            sprite,
            Anchor::BOTTOM_CENTER,
            Transform::from_translation(at),
            BuildingSprite {
                building: id,
                footprint,
                frames,
                phase,
                shown: usize::from(links),
                connects,
            },
            MapEntity,
        ));
        if fades(footprint, first, sim.defs.building(id).passable) {
            entity.insert(BuildingFade::new(at, first));
        }
        drawn.0.insert(pos, entity.id());
    }
}

/// Outlines the whole footprint of the building or site under the cursor.
fn outline_hovered(
    sim: Res<Sim>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut gizmos: Gizmos,
) {
    let (Ok(window), Ok((camera, camera_tf))) = (window.single(), camera.single()) else { return };
    let Some(footprint) =
        cursor_tile(window, camera, camera_tf).and_then(|p| sim.world.footprint_at(p))
    else {
        return;
    };
    let (centre, size) = footprint_rect(footprint);
    gizmos.rect_2d(Isometry2d::from_translation(centre), size, HOVER_COLOR);
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

    /// A free land tile with no unit on it, with room for a 1x1 building's door (#226): some of
    /// these tests designate a build on it, which now checks that too.
    fn free_tile(sim: &Sim) -> Pos {
        let bed = sim.defs.building_id("bed").unwrap();
        sim.world
            .map
            .iter()
            .find(|(p, t)| {
                t.terrain == Terrain::Land
                    && t.resource.is_none()
                    && !sim.world.units.iter().any(|u| u.pos == *p)
                    && sim.world.check_build(*p, bed, &sim.defs).is_ok()
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
                Some(fn_sim::building::Building::new(hall, true, false));
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
            Some(fn_sim::building::Building::new(wall, false, false));
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
                Some(fn_sim::building::Building::new(bed, true, false));
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
                Some(fn_sim::building::Building::new(rail, true, true));
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
            Some(fn_sim::building::Building::new(bed, true, false));
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

    #[test]
    fn a_one_tile_building_stands_where_it_always_did() {
        let p = Pos::new(5, 9);
        let mut old = tile_translation(p, row_depth(p.y));
        old.y -= TILE / 2.0;
        assert_eq!(footprint_foot(Footprint::single(p)), old);
        let (centre, size) = footprint_rect(Footprint::single(p));
        assert_eq!(size, Vec2::splat(TILE));
        assert_eq!(centre, tile_translation(p, 0.0).truncate());
    }

    #[test]
    fn a_two_by_two_stands_on_the_middle_of_its_bottom_edge_at_its_front_row_depth() {
        let foot = Footprint::new(Pos::new(4, 6), 2, 2);
        let at = footprint_foot(foot);
        let left = tile_translation(Pos::new(4, 7), 0.0);
        assert_eq!(at.x, left.x + TILE / 2.0, "halfway across its two columns");
        assert_eq!(at.y, left.y - TILE / 2.0, "on the bottom edge of its front row");
        assert_eq!(at.z, row_depth(7), "sorted by its front row");
        let (centre, size) = footprint_rect(foot);
        assert_eq!(size, Vec2::splat(2.0 * TILE));
        let mid = (tile_translation(Pos::new(4, 6), 0.0) + tile_translation(Pos::new(5, 7), 0.0))
            .truncate()
            / 2.0;
        assert_eq!(centre, mid);
    }

    #[test]
    fn only_a_sprite_that_overhangs_its_footprint_or_a_walkable_one_fades() {
        let big = sprites::buildings::UNFINISHED;
        assert_eq!((big.w, big.h), (2, 2));
        let two = Footprint::new(Pos::new(0, 0), 2, 2);
        let one = Footprint::single(Pos::new(0, 0));
        assert!(!fades(two, big, false), "a solid 2x2 filling its footprint hides nothing");
        assert!(fades(two, big, true), "units walk among an orchard's trees");
        assert!(fades(one, big, false), "a 32px sprite on one tile overhangs the tiles behind");
        let tall = Cell { h: 3, ..big };
        assert!(fades(two, tall, false), "a tall hall's roof overhangs the row behind");
    }

    #[test]
    fn a_two_by_two_building_is_drawn_once_across_its_footprint() {
        let mut app = app(Nation::Earth);
        let (hall, at) = {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let hall = sim.defs.building_id("study_hall").unwrap();
            let def = &mut sim.defs.buildings[hall.0 as usize];
            def.footprint = (2, 2);
            def.passable = false;
            (hall, Pos::new(20, 20))
        };
        let foot = Footprint::new(at, 2, 2);
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            for p in foot.tiles() {
                let part = [(p.x - at.x) as u8, (p.y - at.y) as u8];
                let b = fn_sim::building::Building {
                    part,
                    size: [2, 2],
                    ..fn_sim::building::Building::new(hall, false, false)
                };
                sim.world.map.get_mut(p).unwrap().building = Some(b);
            }
        }
        app.update();
        assert_eq!(app.world().resource::<BuildingSprites>().0.len(), 1, "one sprite");
        let (e, _, building) = sprite_at(&app, at).expect("drawn by its anchor");
        assert_eq!(building.footprint, foot);
        assert_eq!(app.world().get::<Transform>(e).unwrap().translation, footprint_foot(foot));
        assert!(app.world().get::<BuildingFade>(e).is_none(), "a solid 2x2 never fades");
    }
}

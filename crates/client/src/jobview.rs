//! Shows job state on the map and in the unit panel: resource tiles that disappear when cleared,
//! item stacks on the ground and in stockpiles, and what a unit is doing. Designation tints live in
//! `designate`. A stack draws its item's icon from `art/icons.toml` (the sheet the Stores section
//! uses, #130) at 1:1 with the count beside it (#154).

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use fn_sim::jobs::{ItemId, Stack};
use fn_sim::{Defs, Pos};

use super::{
    Art, MARKER_BIAS, MapEntity, Sim, TILE, row_depth, sprites, tile_translation, ui_style,
};

pub struct JobViewPlugin;

impl Plugin for JobViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemMarkers>()
            .add_systems(Update, (sync_resources, sync_item_markers).chain());
    }
}

/// The sprite of the resource on a tile, so it can be removed when the sim clears the tile.
#[derive(Component)]
pub struct ResourceSprite(pub Pos);

/// Despawns resource sprites whose tile no longer has a resource (harvested), and greys out the
/// renewable ones that are growing back until they ripen again.
fn sync_resources(
    mut commands: Commands,
    sim: Res<Sim>,
    mut sprites: Query<(Entity, &ResourceSprite, &mut Sprite)>,
) {
    for (entity, ResourceSprite(pos), mut sprite) in &mut sprites {
        if sim.world.map.get(*pos).is_none_or(|t| t.resource.is_none()) {
            commands.entity(entity).despawn();
            continue;
        }
        let tint = if sim.world.is_resting(*pos) { RESTING_TINT } else { Color::WHITE };
        if sprite.color != tint {
            sprite.color = tint;
        }
    }
}

/// How a renewable resource looks between being worked and ripening again.
const RESTING_TINT: Color = Color::srgba(0.6, 0.6, 0.6, 0.6);
/// Where an item marker is: lying on the ground, or held in a stockpile tile.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Place {
    Ground,
    Stored,
}

#[derive(Component)]
struct ItemMarker;

/// The marker entities currently on the map, with the stack each shows (item id, count).
#[derive(Resource, Default)]
pub struct ItemMarkers(BTreeMap<(Pos, Place), ((u16, u32), Entity)>);

impl ItemMarkers {
    /// The marker entity of the stack stored on `pos`, if it has one.
    pub fn stored(&self, pos: Pos) -> Option<Entity> {
        self.0.get(&(pos, Place::Stored)).map(|(_, e)| *e)
    }
}

/// The icon cell of an item (`icon` in data/jobs.toml and data/needs.toml): the sprite of that
/// name in `icons.png`, one 16px cell, so a stack draws at the size of a tile.
pub fn item_cell(defs: &Defs, item: ItemId) -> sprites::Cell {
    let def = defs.item(item);
    let col = ui_style::icon_cell(&def.icon).unwrap_or_else(|| {
        panic!("item {} names icon {:?}, which art/icons.toml doesn't have", def.id, def.icon)
    });
    sprites::Cell { sheet: ui_style::ICONS_SHEET, col, row: 0, w: 1, h: 1 }
}

/// Where the count sits, as an offset from the tile's centre: the bottom right corner, clear of the
/// icon's middle.
const COUNT_CORNER: Vec2 = Vec2::new(TILE * 0.5, -TILE * 0.5);

/// The count of a stack as white digits with a 1px ink edge (four shadow copies behind the text),
/// so it reads over any icon and any floor. Anchored at the bottom right of the tile.
fn count_label(count: u32) -> Vec<impl Bundle> {
    let text = |dx: f32, dy: f32, color: Color, z: f32| {
        (
            Text2d::new(count.to_string()),
            TextFont { font_size: 8.0.into(), ..default() },
            TextColor(color),
            Anchor::BOTTOM_RIGHT,
            Transform::from_translation((COUNT_CORNER + Vec2::new(dx, dy)).extend(z)),
        )
    };
    let ink = Color::srgb_u8(0x16, 0x18, 0x21);
    let mut label: Vec<_> = [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)]
        .into_iter()
        .map(|(dx, dy)| text(dx, dy, ink, 0.1))
        .collect();
    label.push(text(0.0, 0.0, Color::WHITE, 0.11));
    label
}

/// Keeps one item icon (with the count under it) per ground stack and per stored stack. Stored
/// ones sit on a stockpile tint.
fn sync_item_markers(
    mut commands: Commands,
    sim: Res<Sim>,
    art: Res<Art>,
    mut markers: ResMut<ItemMarkers>,
    alive: Query<(), With<ItemMarker>>,
) {
    // Entities despawned with the old map (regenerate) are no longer there.
    markers.0.retain(|_, (_, e)| alive.contains(*e));
    let world = &sim.world;
    let mut wanted: BTreeMap<(Pos, Place), Stack> =
        world.ground_items().map(|(p, s)| ((p, Place::Ground), s)).collect();
    wanted.extend(
        world.stockpile_zones().filter_map(|p| Some(((p, Place::Stored), world.stored_at(p)?))),
    );
    markers.0.retain(|key, (shown, e)| {
        let keep = wanted.get(key).is_some_and(|s| (s.item.0, s.count) == *shown);
        if !keep {
            commands.entity(*e).despawn();
        }
        keep
    });
    for (key, stack) in wanted {
        markers.0.entry(key).or_insert_with(|| {
            let (pos, _) = key;
            let entity = commands
                .spawn((
                    art.sprite(item_cell(&sim.defs, stack.item)),
                    // Above the stockpile tint, below units on the same row.
                    Transform::from_translation(tile_translation(
                        pos,
                        row_depth(pos.y) + MARKER_BIAS,
                    )),
                    ItemMarker,
                    MapEntity,
                    Children::spawn(SpawnIter(count_label(stack.count).into_iter())),
                ))
                .id();
            ((stack.item.0, stack.count), entity)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fn_sim::Nation;
    use fn_sim::world::Command;
    use fn_ui::Session;

    fn test_app(seed: u64) -> App {
        let mut app = App::new();
        crate::insert_test_art(&mut app);
        app.insert_resource(Sim(Session::new(seed, Nation::Earth, 48, 32)))
            .init_resource::<ItemMarkers>()
            .add_systems(Update, (sync_resources, sync_item_markers).chain());
        app
    }

    /// The resource that is used up when worked (a tree, not a fishing spot) and that anyone can
    /// work, closest to the first unit so it is within reach.
    fn first_tree(sim: &Sim) -> Pos {
        nearest(sim, |h| h.regrow.is_none() && h.unlocked_by.is_none())
    }

    /// The closest resource to the first unit whose harvest satisfies `want`.
    fn nearest(sim: &Sim, want: impl Fn(&fn_sim::jobs::HarvestDef) -> bool) -> Pos {
        let home = sim.world.units[0].pos;
        sim.world
            .map
            .iter()
            .filter(|(_, t)| t.resource.and_then(|r| sim.defs.harvest(r)).is_some_and(&want))
            .map(|(p, _)| p)
            .min_by_key(|p| (p.x - home.x).abs().max((p.y - home.y).abs()))
            .expect("a matching resource on the map")
    }

    /// Every item in data draws an icon from `icons.png`: `item_cell` panics for an `icon` the
    /// sheet doesn't have, and the cell must lie inside the sheet.
    #[test]
    fn every_item_draws_an_icon_from_the_sheet() {
        let defs = Defs::bundled();
        assert!(!defs.items.is_empty());
        let (w, h) = sprites::test_support::png_size(ui_style::ICONS_SHEET);
        for i in 0..defs.items.len() {
            let cell = item_cell(&defs, ItemId(i as u16));
            assert_eq!(cell.sheet, ui_style::ICONS_SHEET);
            let [_, _, x1, y1] = cell.pixel_rect();
            assert!(x1 <= w as f32 && y1 <= h as f32, "{}", defs.items[i].id);
        }
    }

    #[test]
    fn a_stack_is_drawn_as_its_items_icon() {
        let mut app = test_app(1);
        let tree = first_tree(app.world().resource::<Sim>());
        let wood = {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let Session { world, defs, .. } = &mut **sim;
            world.apply(Command::DesignateHarvest { pos: tree }, defs).unwrap();
            for _ in 0..20_000 {
                world.tick(defs);
                if world.ground_items().next().is_some() {
                    break;
                }
            }
            world.ground_items().next().expect("wood should drop").1.item
        };
        app.update();
        let want = {
            let sim = app.world().resource::<Sim>();
            app.world().resource::<Art>().sprite(item_cell(&sim.defs, wood)).rect
        };
        assert!(want.is_some());
        let mut markers = app.world_mut().query_filtered::<&Sprite, With<ItemMarker>>();
        let rects: Vec<_> = markers.iter(app.world()).map(|s| s.rect).collect();
        assert!(!rects.is_empty() && rects.iter().all(|r| *r == want), "{rects:?}");
    }

    fn shown(app: &App) -> Vec<(Pos, Place)> {
        app.world().resource::<ItemMarkers>().0.keys().copied().collect()
    }

    #[test]
    fn cleared_resource_loses_its_sprite() {
        let mut app = test_app(1);
        let pos = first_tree(app.world().resource::<Sim>());
        let sprite = app.world_mut().spawn((ResourceSprite(pos), Sprite::default())).id();
        app.update();
        assert!(app.world().get_entity(sprite).is_ok(), "still has its resource");

        app.world_mut().resource_mut::<Sim>().world.map.get_mut(pos).unwrap().resource = None;
        app.update();
        assert!(app.world().get_entity(sprite).is_err(), "sprite should be gone");
    }

    #[test]
    fn a_resting_resource_is_greyed_out_until_it_ripens() {
        let mut app = test_app(1);
        let (pos, ripe_after) = {
            let sim = app.world().resource::<Sim>();
            let cabbage = sim.defs.resource_id("cabbage").unwrap();
            let pos = nearest(sim, |h| h.item == sim.defs.item_id("cabbage").unwrap());
            (pos, sim.defs.harvest(cabbage).unwrap().regrow.unwrap())
        };
        let sprite = app.world_mut().spawn((ResourceSprite(pos), Sprite::default())).id();
        let color = |app: &App| app.world().get::<Sprite>(sprite).unwrap().color;
        app.update();
        assert_eq!(color(&app), Color::WHITE);

        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let Session { world, defs, .. } = &mut **sim;
            world.apply(Command::DesignateHarvest { pos }, defs).unwrap();
            for _ in 0..20_000 {
                world.tick(defs);
                if world.is_resting(pos) {
                    break;
                }
            }
            assert!(world.is_resting(pos), "gathered, so the plant is regrowing");
        }
        app.update();
        assert_eq!(color(&app), RESTING_TINT);

        let ticks = (ripe_after * 10.0) as u64 + 1;
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let Session { world, defs, .. } = &mut **sim;
            for _ in 0..ticks {
                world.tick(defs);
            }
        }
        app.update();
        assert_eq!(color(&app), Color::WHITE);
    }

    #[test]
    fn items_and_stockpile_stacks_get_markers_that_follow_the_sim() {
        let mut app = test_app(1);
        let tree = first_tree(app.world().resource::<Sim>());
        let pos;
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let Session { world, defs, .. } = &mut **sim;
            world.apply(Command::DesignateHarvest { pos: tree }, defs).unwrap();
            // Run until the tree is felled and its wood is lying about.
            for _ in 0..20_000 {
                world.tick(defs);
                if world.ground_items().next().is_some() {
                    break;
                }
            }
            // The wood lands on or near the felled tree's tile.
            pos = world.ground_items().next().expect("wood should drop").0;
        }
        app.update();
        assert!(shown(&app).contains(&(pos, Place::Ground)));

        // A stockpile next to it: the wood is hauled there and the ground marker goes away.
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let Session { world, defs, .. } = &mut **sim;
            let neighbors: Vec<Pos> = world.map.neighbors(pos, false).collect();
            let zone = neighbors
                .into_iter()
                .find(|p| world.apply(Command::DesignateStockpile { pos: *p }, defs).is_ok())
                .expect("a free neighbor tile");
            for _ in 0..20_000 {
                world.tick(defs);
                if world.stored_at(zone).is_some() {
                    break;
                }
            }
            assert!(world.stored_at(zone).is_some(), "wood should be hauled");
        }
        app.update();
        let now = shown(&app);
        assert!(now.iter().any(|(_, place)| *place == Place::Stored), "stored stack drawn");
        assert!(!now.contains(&(pos, Place::Ground)), "ground marker removed once hauled");
    }
}

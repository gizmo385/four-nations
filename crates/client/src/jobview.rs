//! Shows job state on the map and in the unit panel: resource tiles that disappear when cleared,
//! item stacks on the ground and in stockpiles, and what a unit is doing. Designation tints live in
//! `designate`. Plain shapes and tints only, until the art (#1) and the UI style (#39) land.

use std::collections::BTreeMap;

use bevy::prelude::*;
use fn_sim::jobs::{ItemId, JobKind, Stack};
use fn_sim::world::{FoodSource, Task, Unit};
use fn_sim::{Defs, Pos, World};

use super::{MapEntity, Sim, TILE, tile_translation};

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

/// Placeholder colors per item until the real item art exists.
fn item_color(defs: &Defs, item: ItemId) -> Color {
    let def = defs.item(item);
    match def.id.as_str() {
        "wood" => Color::srgb(0.55, 0.33, 0.14),
        "stone" => Color::srgb(0.62, 0.62, 0.66),
        "gold_ore" => Color::srgb(0.95, 0.8, 0.15),
        _ if def.food.is_some() => Color::srgb(0.45, 0.85, 0.35),
        _ => Color::srgb(0.9, 0.2, 0.8),
    }
}

/// Keeps one small square (with the count under it) per ground stack and per stored stack. Ground
/// stacks are smaller than stored ones, which sit on a stockpile tint.
fn sync_item_markers(
    mut commands: Commands,
    sim: Res<Sim>,
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
            let (pos, place) = key;
            let side = if place == Place::Ground { 6.0 } else { 9.0 };
            let entity = commands
                .spawn((
                    Sprite::from_color(item_color(&sim.defs, stack.item), Vec2::splat(side)),
                    // Above the stockpile tint, below units.
                    Transform::from_translation(tile_translation(pos, 4.7)),
                    ItemMarker,
                    MapEntity,
                    children![(
                        Text2d::new(stack.count.to_string()),
                        TextFont { font_size: 7.0.into(), ..default() },
                        TextColor(Color::WHITE),
                        Transform::from_translation(Vec3::new(0.0, -TILE * 0.38, 0.1)),
                    )],
                ))
                .id();
            ((stack.item.0, stack.count), entity)
        });
    }
}

/// "3 Wood", or "nothing".
pub fn describe_stack(defs: &Defs, stack: Option<Stack>) -> String {
    match stack {
        Some(s) => format!("{} {}", s.count, defs.item(s.item).name),
        None => "nothing".into(),
    }
}

/// What the unit is doing, in a short phrase for the info panel.
pub fn describe_task(world: &World, defs: &Defs, unit: &Unit) -> String {
    let Some(task) = unit.task() else { return "Idle".into() };
    match task {
        Task::Harvest(id) => {
            let Some(job) = world.job(id) else { return "Harvesting".into() };
            let JobKind::Harvest { pos } = job.kind else { return "Harvesting".into() };
            let resource = world.map.get(pos).and_then(|t| t.resource);
            match resource.and_then(|r| Some((defs.harvest(r)?, defs.resource(r)))) {
                Some((h, r)) => format!(
                    "{} {} at ({}, {}), {:.0}% done",
                    h.verb,
                    r.name,
                    pos.x,
                    pos.y,
                    (job.progress / h.work * 100.0).clamp(0.0, 100.0)
                ),
                None => format!("Harvesting at ({}, {})", pos.x, pos.y),
            }
        }
        Task::Haul(id) => {
            let at = world.job(id).and_then(|j| match j.kind {
                JobKind::Haul { pos } => Some(pos),
                JobKind::Harvest { .. } => None,
            });
            let stack = at.and_then(|p| world.ground_items().find(|(q, _)| *q == p));
            match (stack, at) {
                (Some((p, s)), _) => {
                    format!("Fetching {} from ({}, {})", describe_stack(defs, Some(s)), p.x, p.y)
                }
                _ => "Fetching items".into(),
            }
        }
        Task::Deliver => {
            format!("Delivering {} to a stockpile", describe_stack(defs, unit.carrying))
        }
        Task::Eat(source) => match source {
            FoodSource::Carried => "Eating what it carries".into(),
            FoodSource::Stockpile(p) => format!("Eating from the stockpile at ({}, {})", p.x, p.y),
            FoodSource::Ground(p) => format!("Eating from the ground at ({}, {})", p.x, p.y),
            FoodSource::Forage(p) => format!("Foraging at ({}, {})", p.x, p.y),
        },
        Task::Drink => "Drinking".into(),
        Task::Sleep => "Sleeping".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fn_sim::Nation;
    use fn_sim::world::Command;

    fn test_app(seed: u64) -> App {
        let mut app = App::new();
        app.insert_resource(Sim::new(seed, Nation::Earth, 48, 32))
            .init_resource::<ItemMarkers>()
            .add_systems(Update, (sync_resources, sync_item_markers).chain());
        app
    }

    /// The resource that is used up when worked (a tree, not a fishing spot) and that anyone can
    /// work, closest to the first unit so it is within reach.
    fn first_tree(sim: &Sim) -> Pos {
        nearest(sim, |h| h.regrow.is_none() && h.requires_tech.is_none())
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
            let Sim { world, defs, .. } = &mut *sim;
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
            let Sim { world, defs, .. } = &mut *sim;
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
            let Sim { world, defs, .. } = &mut *sim;
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
            let Sim { world, defs, .. } = &mut *sim;
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

    #[test]
    fn a_harvester_names_its_work_and_idle_units_say_so() {
        let mut sim = Sim::new(1, Nation::Earth, 48, 32);
        let pos = first_tree(&sim);
        let Sim { world, defs, .. } = &mut sim;
        assert!(world.units.iter().all(|u| describe_task(world, defs, u) == "Idle"));
        world.apply(Command::DesignateHarvest { pos }, defs).unwrap();
        world.tick(defs);
        let busy = world
            .units
            .iter()
            .map(|u| describe_task(world, defs, u))
            .find(|d| d != "Idle")
            .expect("a unit takes the job");
        assert!(busy.contains(&format!("({}, {})", pos.x, pos.y)), "{busy}");
    }

    #[test]
    fn stacks_read_naturally() {
        let defs = Defs::bundled();
        let wood = defs.item_id("wood").unwrap();
        assert_eq!(describe_stack(&defs, Some(Stack { item: wood, count: 3 })), "3 Wood");
        assert_eq!(describe_stack(&defs, None), "nothing");
    }
}

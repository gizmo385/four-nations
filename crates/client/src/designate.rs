//! Designation tools: pick a tool with a key, drag a rectangle on the map, and every tile in it
//! becomes a `Command` for `World::apply`. Designated tiles are tinted so you can see what is
//! queued. Minimal on purpose: the overall UI style is #39 and job state display is #57.
//!
//! Keys: `H` harvest (chop, quarry, gather, mine, whatever each tile's resource gives), `X` cancel
//! harvest, `P` stockpile, `Esc` back to selecting units. While a tool is active the left mouse
//! button drags a rectangle; middle and right still pan.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use fn_sim::Pos;
use fn_sim::world::Command;

use super::{MapEntity, Sim, TILE, cursor_tile, tile_translation};

pub struct DesignatePlugin;

impl Plugin for DesignatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tool>()
            .init_resource::<Notice>()
            .init_resource::<Overlays>()
            .add_systems(Update, (choose_tool, designate_drag, sync_overlays).chain());
    }
}

/// What the left mouse button does on the map.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    /// Click to select a unit.
    #[default]
    Select,
    Harvest,
    CancelHarvest,
    Stockpile,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "select units",
            Self::Harvest => "designate harvest (chop, mine, gather)",
            Self::CancelHarvest => "cancel harvest",
            Self::Stockpile => "designate stockpile",
        }
    }

    /// The command this tool issues for one tile, if it issues any.
    fn command(self, pos: Pos) -> Option<Command> {
        match self {
            Self::Select => None,
            Self::Harvest => Some(Command::DesignateHarvest { pos }),
            Self::CancelHarvest => Some(Command::CancelHarvest { pos }),
            Self::Stockpile => Some(Command::DesignateStockpile { pos }),
        }
    }

    /// Color of the drag rectangle.
    fn drag_color(self) -> Color {
        match self {
            Self::CancelHarvest => Layer::Harvest.outline().with_alpha(0.5),
            Self::Stockpile => Layer::Stockpile.outline(),
            _ => Layer::Harvest.outline(),
        }
    }
}

/// The result of the last designation, shown in the HUD.
#[derive(Resource, Default)]
pub struct Notice(pub String);

/// Every tile in the rectangle spanned by two corners, row by row.
fn tiles_between(a: Pos, b: Pos) -> impl Iterator<Item = Pos> {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| Pos::new(x, y)))
}

fn choose_tool(
    keys: Res<ButtonInput<KeyCode>>,
    mut tool: ResMut<Tool>,
    mut notice: ResMut<Notice>,
) {
    let next = [
        (KeyCode::KeyH, Tool::Harvest),
        (KeyCode::KeyX, Tool::CancelHarvest),
        (KeyCode::KeyP, Tool::Stockpile),
        (KeyCode::Escape, Tool::Select),
    ]
    .into_iter()
    .find(|(k, _)| keys.just_pressed(*k))
    .map(|(_, t)| t);
    if let Some(next) = next
        && *tool != next
    {
        *tool = next;
        notice.0.clear();
    }
}

/// Tracks a left-button drag with a designation tool and issues its commands on release.
fn designate_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    tool: Res<Tool>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut sim: ResMut<Sim>,
    mut notice: ResMut<Notice>,
    mut start: Local<Option<Pos>>,
    mut gizmos: Gizmos,
) {
    if *tool == Tool::Select {
        *start = None;
        return;
    }
    let (Ok(window), Ok((camera, camera_tf))) = (window.single(), camera.single()) else { return };
    let here = cursor_tile(window, camera, camera_tf);
    if mouse.just_pressed(MouseButton::Left) {
        *start = here;
    }
    let (Some(from), Some(to)) = (*start, here) else {
        if mouse.just_released(MouseButton::Left) {
            *start = None;
        }
        return;
    };
    let min = Pos::new(from.x.min(to.x), from.y.min(to.y));
    let max = Pos::new(from.x.max(to.x), from.y.max(to.y));
    if mouse.pressed(MouseButton::Left) {
        let center = (tile_translation(min, 0.0) + tile_translation(max, 0.0)).truncate() / 2.0;
        let size = Vec2::new((max.x - min.x + 1) as f32, (max.y - min.y + 1) as f32) * TILE;
        gizmos.rect_2d(Isometry2d::from_translation(center), size, tool.drag_color());
    }
    if mouse.just_released(MouseButton::Left) {
        *start = None;
        let Sim { world, defs, .. } = &mut *sim;
        let (mut done, mut refused) = (0, 0);
        for pos in tiles_between(from, to) {
            if let Some(command) = tool.command(pos) {
                match world.apply(command, defs) {
                    Ok(()) => done += 1,
                    Err(_) => refused += 1,
                }
            }
        }
        notice.0 = format!("{}: {done} done, {refused} skipped", tool.label());
    }
}

/// Which designation an overlay tile shows.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Layer {
    Harvest,
    Stockpile,
}

impl Layer {
    fn tint(self) -> Color {
        match self {
            Self::Harvest => Color::srgba(1.0, 0.65, 0.1, 0.4),
            Self::Stockpile => Color::srgba(0.2, 0.55, 1.0, 0.4),
        }
    }

    fn outline(self) -> Color {
        self.tint().with_alpha(1.0)
    }
}

#[derive(Component)]
struct Overlay;

/// The tint entities currently on the map, by tile and layer.
#[derive(Resource, Default)]
pub struct Overlays(BTreeMap<(Pos, Layer), Entity>);

/// Keeps one translucent tile per designated harvest and stockpile tile, matching the sim.
fn sync_overlays(
    mut commands: Commands,
    sim: Res<Sim>,
    mut overlays: ResMut<Overlays>,
    alive: Query<(), With<Overlay>>,
) {
    // Entities despawned with the old map (regenerate) are no longer there.
    overlays.0.retain(|_, e| alive.contains(*e));
    let mut wanted: BTreeSet<(Pos, Layer)> =
        sim.world.stockpile_zones().map(|p| (p, Layer::Stockpile)).collect();
    wanted.extend(sim.world.harvest_designations().map(|p| (p, Layer::Harvest)));
    overlays.0.retain(|key, e| {
        let keep = wanted.contains(key);
        if !keep {
            commands.entity(*e).despawn();
        }
        keep
    });
    for key in wanted {
        overlays.0.entry(key).or_insert_with(|| {
            let (pos, layer) = key;
            commands
                .spawn((
                    Sprite::from_color(layer.tint(), Vec2::splat(TILE)),
                    // Above resources, below units.
                    Transform::from_translation(tile_translation(pos, 4.5)),
                    Overlay,
                    MapEntity,
                ))
                .id()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_covers_both_corners_whatever_the_drag_direction() {
        let a = Pos::new(5, 7);
        let b = Pos::new(3, 8);
        let tiles: Vec<Pos> = tiles_between(a, b).collect();
        assert_eq!(tiles.len(), 6);
        assert_eq!(tiles.first(), Some(&Pos::new(3, 7)));
        assert_eq!(tiles.last(), Some(&Pos::new(5, 8)));
        assert_eq!(tiles, tiles_between(b, a).collect::<Vec<_>>());
    }

    #[test]
    fn a_click_is_a_one_tile_rectangle() {
        let p = Pos::new(2, 2);
        assert_eq!(tiles_between(p, p).collect::<Vec<_>>(), [p]);
    }

    #[test]
    fn each_tool_issues_its_own_command() {
        let pos = Pos::new(1, 2);
        assert_eq!(Tool::Select.command(pos), None);
        assert_eq!(Tool::Harvest.command(pos), Some(Command::DesignateHarvest { pos }));
        assert_eq!(Tool::CancelHarvest.command(pos), Some(Command::CancelHarvest { pos }));
        assert_eq!(Tool::Stockpile.command(pos), Some(Command::DesignateStockpile { pos }));
    }
}

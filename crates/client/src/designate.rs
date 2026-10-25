//! Designation tools: pick a tool with a key, drag a rectangle on the map, and every tile in it
//! becomes a `Command` for `World::apply`. Designated tiles are tinted so you can see what is
//! queued. Which command each tool issues, and its key, are `fn_ui::tools`; this module reads
//! the mouse and keyboard and draws the tints. Job state display is in `jobview`.
//!
//! Keys: `H` harvest (chop, quarry, gather, mine, whatever each tile's resource gives), `X` cancel
//! harvest (also withdraws a build site), `P` stockpile, `B` the nation's bending ability (#72),
//! `U` the build tool (#111; pressing it again cycles through the buildings the nation can
//! place, and the side panel's picker rows can be clicked), `Esc` back to selecting units.
//! While a tool is active the left mouse button drags a rectangle; middle and right still pan.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use fn_sim::Pos;
use fn_sim::building::BuildingId;
use fn_ui::tools::{self, Tool};

use super::{MapEntity, Sim, TILE, cursor_tile, tile_translation};
use crate::hud;
use crate::ui_style::{self, Theme};

pub struct DesignatePlugin;

impl Plugin for DesignatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveTool>()
            .init_resource::<Notice>()
            .init_resource::<Overlays>()
            .init_resource::<BuildChoice>()
            .add_systems(
                Update,
                (
                    choose_tool.run_if(super::screen_closed),
                    click_picker.run_if(super::screen_closed),
                    designate_drag.run_if(super::screen_closed),
                    sync_overlays,
                )
                    .chain(),
            );
    }
}

/// What the left mouse button does on the map (`fn_ui`'s `Tool`, as a resource).
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct ActiveTool(pub Tool);

/// The building the Build tool places. Kept across tool changes so `U` returns to it.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct BuildChoice(pub Option<BuildingId>);

/// The result of the last designation, shown in the HUD.
#[derive(Resource, Default)]
pub struct Notice(pub String);

/// Color of the drag rectangle.
fn drag_color(tool: Tool, theme: &Theme, repeating: bool) -> Color {
    match tool {
        Tool::CancelHarvest => Layer::Harvest.outline(theme).with_alpha(0.5),
        Tool::Harvest if repeating => Layer::RepeatingHarvest.outline(theme),
        Tool::Stockpile => Layer::Stockpile.outline(theme),
        Tool::Bend => Layer::Bend.outline(theme),
        Tool::Build => Layer::Site.outline(theme),
        _ => Layer::Harvest.outline(theme),
    }
}

/// The key code of the letter key that picks a tool (`Tool::letter`).
fn letter_key(letter: char) -> Option<KeyCode> {
    const LETTERS: [KeyCode; 26] = [
        KeyCode::KeyA,
        KeyCode::KeyB,
        KeyCode::KeyC,
        KeyCode::KeyD,
        KeyCode::KeyE,
        KeyCode::KeyF,
        KeyCode::KeyG,
        KeyCode::KeyH,
        KeyCode::KeyI,
        KeyCode::KeyJ,
        KeyCode::KeyK,
        KeyCode::KeyL,
        KeyCode::KeyM,
        KeyCode::KeyN,
        KeyCode::KeyO,
        KeyCode::KeyP,
        KeyCode::KeyQ,
        KeyCode::KeyR,
        KeyCode::KeyS,
        KeyCode::KeyT,
        KeyCode::KeyU,
        KeyCode::KeyV,
        KeyCode::KeyW,
        KeyCode::KeyX,
        KeyCode::KeyY,
        KeyCode::KeyZ,
    ];
    letter.is_ascii_uppercase().then(|| LETTERS[(letter as u8 - b'A') as usize])
}

/// The tool whose key was pressed this frame: `Esc` is `Select`, the rest are letters.
fn tool_pressed(keys: &ButtonInput<KeyCode>) -> Option<Tool> {
    if keys.just_pressed(KeyCode::Escape) {
        return Some(Tool::Select);
    }
    Tool::ALL
        .into_iter()
        .find(|t| t.letter().and_then(letter_key).is_some_and(|k| keys.just_pressed(k)))
}

fn choose_tool(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<Sim>,
    mut tool: ResMut<ActiveTool>,
    mut choice: ResMut<BuildChoice>,
    mut notice: ResMut<Notice>,
) {
    let Some(requested) = tool_pressed(&keys) else { return };
    // `B` pressed again moves on to the next ability the settlement can use (#123).
    tools::cycle_ability(&mut sim, tool.0, requested);
    // `U` picks the build tool; pressed again it moves on to the next building the nation can
    // place (`switch_tool`).
    let (next, building) = tools::switch_tool(&sim, tool.0, choice.0, requested);
    choice.0 = building;
    if next != tool.0 {
        tool.0 = next;
        notice.0.clear();
    }
}

/// Picks the building whose picker row the left button was pressed on. The client has no UI
/// picking, so this hit-tests the rows (`hud::spawn_picker` makes only the rows that can be
/// placed into `PickerButton`s) against the cursor itself.
fn click_picker(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    rows: Query<(&hud::PickerButton, &ComputedNode, &UiGlobalTransform)>,
    mut choice: ResMut<BuildChoice>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = window.single() else { return };
    let hit = ui_style::node_under_cursor(window, rows.iter().map(|(b, n, t)| (b.0, n, t)));
    if let Some(building) = hit {
        choice.0 = Some(building);
    }
}

/// Tracks a left-button drag with a designation tool and issues its commands on release. Holding
/// Shift while dragging the harvest tool marks the designation repeating (#81).
fn designate_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    tool: Res<ActiveTool>,
    choice: Res<BuildChoice>,
    theme: Res<Theme>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut sim: ResMut<Sim>,
    mut notice: ResMut<Notice>,
    mut start: Local<Option<Pos>>,
    mut gizmos: Gizmos,
) {
    if tool.0 == Tool::Select {
        *start = None;
        return;
    }
    let repeating = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let (Ok(window), Ok((camera, camera_tf))) = (window.single(), camera.single()) else { return };
    let here = cursor_tile(window, camera, camera_tf);
    if mouse.just_pressed(MouseButton::Left) {
        // A press on the side panel (a picker row, a tool button) isn't a press on the map
        // under it.
        let on_panel =
            window.cursor_position().is_some_and(|c| hud::over_side_panel(c, window.size()));
        *start = if on_panel { None } else { here };
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
        gizmos.rect_2d(
            Isometry2d::from_translation(center),
            size,
            drag_color(tool.0, &theme, repeating),
        );
    }
    if mouse.just_released(MouseButton::Left) {
        *start = None;
        notice.0 = tools::apply_rect(&mut sim, tool.0, from, to, repeating, choice.0);
    }
}

/// Which designation an overlay tile shows. `RepeatingHarvest` is a harvest with a standing
/// order on it (#81), tinted differently so it reads apart from a one-shot `Harvest`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Layer {
    Harvest,
    RepeatingHarvest,
    Stockpile,
    Bend,
    /// A pending construction site (#111).
    Site,
}

impl Layer {
    /// `Bend` uses the nation's accent, so it reads as "your ability" rather than a fixed hue.
    /// `RepeatingHarvest` is `Harvest`'s hue pushed toward green, reading as "standing" rather
    /// than "one-shot" without needing a second colour family.
    fn tint(self, theme: &Theme) -> Color {
        match self {
            Self::Harvest => Color::srgba(1.0, 0.65, 0.1, 0.4),
            Self::RepeatingHarvest => Color::srgba(0.65, 0.85, 0.1, 0.4),
            Self::Stockpile => Color::srgba(0.2, 0.55, 1.0, 0.4),
            Self::Bend => theme.accent.with_alpha(0.4),
            Self::Site => Color::srgba(0.8, 0.6, 0.3, 0.5),
        }
    }

    /// Harvest and bend marks draw over everything, trees included, so a marked tile is easy to
    /// see. A stockpile is flat ground: it goes under whatever stands on it (item markers, units).
    fn depth(self) -> f32 {
        match self {
            Self::Harvest | Self::RepeatingHarvest | Self::Bend => 4.5,
            Self::Stockpile => 0.6,
            // Flat ground like a stockpile, but above one, and under units.
            Self::Site => 0.65,
        }
    }

    fn outline(self, theme: &Theme) -> Color {
        self.tint(theme).with_alpha(1.0)
    }
}

#[derive(Component)]
struct Overlay;

/// The tint entities currently on the map, by tile and layer.
#[derive(Resource, Default)]
pub struct Overlays(BTreeMap<(Pos, Layer), Entity>);

/// Keeps one translucent tile per designated harvest, stockpile and build-site tile, matching the
/// sim. Finished buildings and the sprite of a site are drawn by `buildings`.
fn sync_overlays(
    mut commands: Commands,
    sim: Res<Sim>,
    theme: Res<Theme>,
    mut overlays: ResMut<Overlays>,
    alive: Query<(), With<Overlay>>,
) {
    // Entities despawned with the old map (regenerate) are no longer there.
    overlays.0.retain(|_, e| alive.contains(*e));
    let mut wanted: BTreeSet<(Pos, Layer)> =
        sim.world.stockpile_zones().map(|p| (p, Layer::Stockpile)).collect();
    wanted.extend(sim.world.harvest_designations().map(|p| {
        let layer = if sim.world.is_repeating_harvest(p) {
            Layer::RepeatingHarvest
        } else {
            Layer::Harvest
        };
        (p, layer)
    }));
    wanted.extend(sim.world.bend_designations().map(|(_, p)| (p, Layer::Bend)));
    wanted.extend(sim.world.build_sites().map(|(f, _, _)| (f.anchor, Layer::Site)));
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
                    Sprite::from_color(layer.tint(&theme), Vec2::splat(TILE)),
                    Transform::from_translation(tile_translation(pos, layer.depth())),
                    Overlay,
                    MapEntity,
                ))
                .id()
        });
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use fn_sim::world::Command;
    use fn_ui::Session;

    use super::*;

    #[test]
    fn every_tool_key_is_a_keyboard_key() {
        let keys: Vec<KeyCode> = Tool::ALL
            .iter()
            .filter_map(|t| t.letter())
            .map(|c| letter_key(c).unwrap_or_else(|| panic!("no key for {c}")))
            .collect();
        assert_eq!(
            keys,
            [KeyCode::KeyH, KeyCode::KeyX, KeyCode::KeyP, KeyCode::KeyB, KeyCode::KeyU]
        );
        assert_eq!(letter_key('h'), None, "letters are upper case");
    }

    #[test]
    fn a_tool_is_picked_by_its_key_and_esc_selects() {
        let mut keys = ButtonInput::<KeyCode>::default();
        assert_eq!(tool_pressed(&keys), None);
        keys.press(KeyCode::KeyX);
        assert_eq!(tool_pressed(&keys), Some(Tool::CancelHarvest));
        keys.clear();
        keys.press(KeyCode::Escape);
        assert_eq!(tool_pressed(&keys), Some(Tool::Select));
    }

    #[test]
    fn a_build_site_gets_an_overlay_and_loses_it_when_cancelled() {
        let mut sim = Session::new(7, Nation::Earth, 64, 64);
        let bed = sim.defs.building_id("bed").unwrap();
        let pos = sim
            .world
            .map
            .iter()
            .find(|(p, t)| {
                t.terrain == fn_sim::Terrain::Land
                    && t.resource.is_none()
                    && !sim.world.units.iter().any(|u| u.pos == *p)
            })
            .map(|(p, _)| p)
            .unwrap();
        sim.apply(Command::DesignateBuild { pos, building: bed }).unwrap();
        let theme = Theme::for_nation(sim.defs.nation(Nation::Earth).color);
        let mut app = App::new();
        app.insert_resource(Sim(sim))
            .insert_resource(theme)
            .init_resource::<Overlays>()
            .add_systems(Update, sync_overlays);
        app.update();
        let overlays = |app: &mut App| app.world().resource::<Overlays>().0.clone();
        assert_eq!(overlays(&mut app).keys().collect::<Vec<_>>(), [&(pos, Layer::Site)]);

        let mut sim = app.world_mut().resource_mut::<Sim>();
        sim.apply(Command::CancelBuild { pos }).unwrap();
        app.update();
        assert!(overlays(&mut app).is_empty());
    }

    #[test]
    fn a_stockpile_tile_loses_its_tint_when_cancelled() {
        let mut sim = Session::new(7, Nation::Earth, 64, 64);
        let pos = sim
            .world
            .map
            .iter()
            .find(|(p, t)| {
                t.terrain == fn_sim::Terrain::Land
                    && t.resource.is_none()
                    && !sim.world.units.iter().any(|u| u.pos == *p)
            })
            .map(|(p, _)| p)
            .unwrap();
        sim.apply(Command::DesignateStockpile { pos }).unwrap();
        let theme = Theme::for_nation(sim.defs.nation(Nation::Earth).color);
        let mut app = App::new();
        app.insert_resource(Sim(sim))
            .insert_resource(theme)
            .init_resource::<Overlays>()
            .add_systems(Update, sync_overlays);
        app.update();
        let overlays = |app: &mut App| app.world().resource::<Overlays>().0.clone();
        assert_eq!(overlays(&mut app).keys().collect::<Vec<_>>(), [&(pos, Layer::Stockpile)]);

        let mut sim = app.world_mut().resource_mut::<Sim>();
        sim.apply(Command::CancelStockpile { pos }).unwrap();
        app.update();
        assert!(overlays(&mut app).is_empty());
    }
}

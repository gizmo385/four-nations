//! Designation tools: pick a tool with a key, drag a rectangle on the map, and every tile in it
//! becomes a `Command` for `World::apply`. Designated tiles are tinted so you can see what is
//! queued. Minimal on purpose: the overall UI style is #39. Job state display is in `jobview`.
//!
//! Keys: `H` harvest (chop, quarry, gather, mine, whatever each tile's resource gives), `X` cancel
//! harvest (also withdraws a build site), `P` stockpile, `B` the nation's bending ability (#72),
//! `U` the build tool (#111; pressing it again cycles through the buildings the nation can
//! place, and the side panel's picker rows can be clicked), `Esc` back to selecting units.
//! While a tool is active the left mouse button drags a rectangle; middle and right still pan.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use fn_sim::bending::AbilityId;
use fn_sim::building::BuildingId;
use fn_sim::world::Command;
use fn_sim::{Defs, Nation, Pos};

use super::{MapEntity, Sim, TILE, cursor_tile, tile_translation};
use crate::build::{self, BuildChoice};
use crate::hud;
use crate::ui_style::Theme;

pub struct DesignatePlugin;

impl Plugin for DesignatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tool>()
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

/// What the left mouse button does on the map.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    /// Click to select a unit.
    #[default]
    Select,
    Harvest,
    CancelHarvest,
    Stockpile,
    /// Channel the nation's signature ability (docs/design/bending.md).
    Bend,
    /// Place the building picked in `BuildChoice` (#111).
    Build,
}

/// What a drag needs besides the tool and the tile to decide on a command.
#[derive(Clone, Copy, Default, Debug)]
struct Pick {
    /// The nation's bending ability, resolved once per drag (`ability_for_nation`); `Bend`
    /// issues nothing without one.
    ability: Option<AbilityId>,
    /// Held Shift (#81): a harvest designation becomes a standing order that re-queues itself
    /// every time the resource ripens again. No effect on any other tool.
    repeating: bool,
    /// The building to place; `Build` issues nothing without one.
    building: Option<BuildingId>,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "select units",
            Self::Harvest => "designate harvest (chop, mine, gather); hold Shift to repeat",
            Self::CancelHarvest => "cancel harvest or build site",
            Self::Stockpile => "designate stockpile",
            Self::Bend => "channel the nation's ability",
            Self::Build => "build: pick a building, then click a tile (U: next building)",
        }
    }

    /// The command this tool issues for one tile, if it issues any. `site_here` says a build
    /// site is pending on the tile, which the cancel tool withdraws instead of a harvest.
    fn command(self, pos: Pos, site_here: bool, pick: Pick) -> Option<Command> {
        match self {
            Self::Select => None,
            Self::Harvest if pick.repeating => Some(Command::DesignateRepeatingHarvest { pos }),
            Self::Harvest => Some(Command::DesignateHarvest { pos }),
            Self::CancelHarvest if site_here => Some(Command::CancelBuild { pos }),
            Self::CancelHarvest => Some(Command::CancelHarvest { pos }),
            Self::Stockpile => Some(Command::DesignateStockpile { pos }),
            Self::Bend => Some(Command::Bend { ability: pick.ability?, pos }),
            Self::Build => Some(Command::DesignateBuild { pos, building: pick.building? }),
        }
    }

    /// Color of the drag rectangle.
    fn drag_color(self, theme: &Theme, repeating: bool) -> Color {
        match self {
            Self::CancelHarvest => Layer::Harvest.outline(theme).with_alpha(0.5),
            Self::Harvest if repeating => Layer::RepeatingHarvest.outline(theme),
            Self::Stockpile => Layer::Stockpile.outline(theme),
            Self::Bend => Layer::Bend.outline(theme),
            Self::Build => Layer::Site.outline(theme),
            _ => Layer::Harvest.outline(theme),
        }
    }
}

/// The nation's one bending ability right now (docs/design/bending.md: "There's one ability
/// now; a small picker appears once research unlocks more"). `None` only if a nation has no
/// ability defined at all, which the data tests guard against.
pub(crate) fn ability_for_nation(defs: &Defs, nation: Nation) -> Option<AbilityId> {
    defs.abilities.iter().position(|a| a.nation == nation).map(|i| AbilityId(i as u16))
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
    sim: Res<Sim>,
    mut tool: ResMut<Tool>,
    mut choice: ResMut<BuildChoice>,
    mut notice: ResMut<Notice>,
) {
    // `U` picks the build tool; pressed again it moves on to the next building the nation can
    // place.
    if keys.just_pressed(KeyCode::KeyU) {
        let rows = build::picker_rows(&sim);
        choice.0 = if *tool == Tool::Build {
            build::next_choice(&rows, choice.0)
        } else {
            build::valid_choice(&rows, choice.0)
        };
        if *tool != Tool::Build {
            *tool = Tool::Build;
            notice.0.clear();
        }
        return;
    }
    let next = [
        (KeyCode::KeyH, Tool::Harvest),
        (KeyCode::KeyX, Tool::CancelHarvest),
        (KeyCode::KeyP, Tool::Stockpile),
        (KeyCode::KeyB, Tool::Bend),
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
    let Some(cursor) =
        window.single().ok().and_then(|w| Some(w.cursor_position()? * w.scale_factor()))
    else {
        return;
    };
    // Node geometry is in physical pixels; the cursor above is scaled to match.
    for (button, node, transform) in &rows {
        if node.contains_point(*transform, cursor) {
            choice.0 = Some(button.0);
        }
    }
}

/// Tracks a left-button drag with a designation tool and issues its commands on release. Holding
/// Shift while dragging the harvest tool marks the designation repeating (#81).
fn designate_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    tool: Res<Tool>,
    choice: Res<BuildChoice>,
    theme: Res<Theme>,
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
            tool.drag_color(&theme, repeating),
        );
    }
    if mouse.just_released(MouseButton::Left) {
        *start = None;
        let rows = build::picker_rows(&sim);
        let building = build::valid_choice(&rows, choice.0);
        let pick =
            Pick { ability: ability_for_nation(&sim.defs, sim.world.nation), repeating, building };
        let what = match (*tool, building) {
            (Tool::Build, Some(id)) => {
                let name = rows.iter().find(|r| r.id == id).map_or("", |r| r.name.as_str());
                format!("build {name}")
            }
            _ => tool.label().to_owned(),
        };
        let (mut done, mut refused) = (0, 0);
        let mut last_error = None;
        for pos in tiles_between(from, to) {
            let site_here = sim.world.build_site(pos).is_some();
            if let Some(command) = tool.command(pos, site_here, pick) {
                let Sim { world, defs, .. } = &mut *sim;
                match world.apply(command, defs) {
                    Ok(()) => done += 1,
                    Err(e) => {
                        refused += 1;
                        last_error = Some(e);
                    }
                }
            }
        }
        // A reason is shown even for a one-tile click, so a rejected bend (or harvest, or
        // stockpile, or build) says why.
        notice.0 = match last_error {
            Some(e) => {
                let why = build::describe_error(&sim, e);
                format!("{what}: {done} done, {refused} skipped ({why})")
            }
            None => format!("{what}: {done} done, {refused} skipped"),
        };
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
    /// A finished building: a placeholder block until there is building art.
    Built,
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
            Self::Built => Color::srgb(0.55, 0.38, 0.22),
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
            Self::Built => 0.7,
        }
    }

    /// A site tints the whole tile; a finished building is a block inset in it.
    fn size(self) -> f32 {
        match self {
            Self::Built => TILE * 0.7,
            _ => TILE,
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

/// Keeps one translucent tile per designated harvest, stockpile and build-site tile, and a block
/// per finished building, matching the sim.
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
    wanted.extend(sim.world.build_sites().map(|(p, _, _)| (p, Layer::Site)));
    wanted.extend(
        sim.world.map.iter().filter(|(_, t)| t.building.is_some()).map(|(p, _)| (p, Layer::Built)),
    );
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
                    Sprite::from_color(layer.tint(&theme), Vec2::splat(layer.size())),
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
        let none = Pick::default();
        assert_eq!(Tool::Select.command(pos, false, none), None);
        assert_eq!(
            Tool::Harvest.command(pos, false, none),
            Some(Command::DesignateHarvest { pos })
        );
        assert_eq!(
            Tool::CancelHarvest.command(pos, false, none),
            Some(Command::CancelHarvest { pos })
        );
        assert_eq!(
            Tool::Stockpile.command(pos, false, none),
            Some(Command::DesignateStockpile { pos })
        );
        let ability = AbilityId(0);
        let bend = Pick { ability: Some(ability), ..none };
        assert_eq!(Tool::Bend.command(pos, false, bend), Some(Command::Bend { ability, pos }));
        let building = BuildingId(2);
        let build = Pick { building: Some(building), ..none };
        assert_eq!(
            Tool::Build.command(pos, false, build),
            Some(Command::DesignateBuild { pos, building })
        );
    }

    #[test]
    fn a_build_site_gets_an_overlay_and_loses_it_when_cancelled() {
        let mut sim = Sim::new(7, Nation::Earth, 64, 64);
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
        let Sim { world, defs, .. } = &mut sim;
        world.apply(Command::DesignateBuild { pos, building: bed }, defs).unwrap();
        let theme = Theme::for_nation(sim.defs.nation(Nation::Earth).color);
        let mut app = App::new();
        app.insert_resource(sim)
            .insert_resource(theme)
            .init_resource::<Overlays>()
            .add_systems(Update, sync_overlays);
        app.update();
        let overlays = |app: &mut App| app.world().resource::<Overlays>().0.clone();
        assert_eq!(overlays(&mut app).keys().collect::<Vec<_>>(), [&(pos, Layer::Site)]);

        let mut sim = app.world_mut().resource_mut::<Sim>();
        let Sim { world, defs, .. } = &mut *sim;
        world.apply(Command::CancelBuild { pos }, defs).unwrap();
        app.update();
        assert!(overlays(&mut app).is_empty());
    }

    #[test]
    fn build_needs_a_chosen_building_to_issue_a_command() {
        assert_eq!(Tool::Build.command(Pos::new(0, 0), false, Pick::default()), None);
    }

    #[test]
    fn cancel_withdraws_a_build_site_where_there_is_one() {
        let pos = Pos::new(4, 4);
        let none = Pick::default();
        assert_eq!(
            Tool::CancelHarvest.command(pos, true, none),
            Some(Command::CancelBuild { pos })
        );
        // Only the cancel tool does.
        assert_eq!(Tool::Harvest.command(pos, true, none), Some(Command::DesignateHarvest { pos }));
    }

    #[test]
    fn holding_shift_makes_a_harvest_designation_repeating() {
        let pos = Pos::new(1, 2);
        let shift = Pick { repeating: true, ..Pick::default() };
        assert_eq!(
            Tool::Harvest.command(pos, false, shift),
            Some(Command::DesignateRepeatingHarvest { pos })
        );
        // Shift only changes what the harvest tool does.
        assert_eq!(
            Tool::CancelHarvest.command(pos, false, shift),
            Some(Command::CancelHarvest { pos })
        );
        assert_eq!(Tool::Select.command(pos, false, shift), None);
    }

    #[test]
    fn bend_needs_an_ability_to_issue_a_command() {
        assert_eq!(Tool::Bend.command(Pos::new(0, 0), false, Pick::default()), None);
    }

    #[test]
    fn ability_for_nation_finds_each_nation_s_one_signature_ability() {
        let defs = fn_sim::Defs::bundled();
        for nation in fn_sim::Nation::ALL {
            let id = ability_for_nation(&defs, nation).unwrap_or_else(|| panic!("{nation:?}"));
            assert_eq!(defs.ability(id).nation, nation);
        }
    }
}

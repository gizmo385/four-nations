//! The HUD (docs/design/ui.md): the side panel on the right, the selection bar along the bottom
//! and the hover tooltip, all built from `ui_style`. Owns no game rules: it reads the sim and
//! shows it.
//!
//! Each panel keeps a small *view* of what it shows (plain data, `PartialEq`) and rebuilds its
//! contents only when the view changes, so a text that changes once a second isn't re-laid-out
//! every frame. The view functions are pure, which is what the tests cover.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use fn_sim::Nation;
use fn_sim::building::BuildingId;
use fn_ui::Tool;
use fn_ui::build::PickerRow;
use fn_ui::inspect::{self, Hint, Line};
use fn_ui::view::{self, SideView, Status, StoreRow, UnitView};

use crate::bending::AlertLog;
use crate::designate::{ActiveTool, BuildChoice, Notice};
use crate::research::ScreenOpen;
use crate::ui_style::{self, Frame, Skin, Theme, UiArt, font, layout, space};
use crate::{Art, Selection, Sim, cursor_tile, unit_character};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        let sim = app.world().resource::<Sim>();
        let theme = Theme::for_nation(sim.defs.nation(sim.world.nation).color);
        let art = UiArt::load(app.world().resource::<AssetServer>());
        let skin = art.skin(sim.world.nation);
        app.insert_resource(theme).insert_resource(skin).insert_resource(art).add_systems(
            Update,
            (sync_hud_root, update_side_panel, update_selection_bar, update_tooltip).chain(),
        );
    }
}

/// Everything the HUD spawns at the top level, so a nation change can replace it all.
#[derive(Component)]
struct HudRoot;

/// The side panel's contents, rebuilt when its `SideView` changes.
#[derive(Component)]
struct SideContent;

/// The selection bar's panel; hidden while nothing is selected.
#[derive(Component)]
struct SelectionBar;

/// The selection bar's contents, rebuilt when its `UnitView` changes.
#[derive(Component)]
struct SelectionContent;

/// A building picker row that can be chosen (locked rows don't get one); `designate.rs` hit-tests
/// clicks against it.
#[derive(Component)]
pub struct PickerButton(pub BuildingId);

#[derive(Component)]
struct Tooltip;

/// The tooltip's lines, rebuilt when they change.
#[derive(Component)]
struct TooltipLines;

/// The portrait is drawn at this integer scale so the 38px faceset stays crisp.
const PORTRAIT_SCALE: f32 = 2.0;
/// The portrait well around it (ui.md "Portraits").
const PORTRAIT_WELL: f32 = 96.0;
/// Width of the needs column in the selection bar.
const NEEDS_WIDTH: f32 = 260.0;
/// Cursor offset of the tooltip's near corner.
const TOOLTIP_GAP: f32 = 16.0;

// ---------------------------------------------------------------------------------------------
// Root and theme

/// Builds the HUD on the first frame, and rebuilds it with a fresh `Theme` and `Skin` whenever the
/// player's nation changes (keys 1 to 4).
fn sync_hud_root(
    mut commands: Commands,
    sim: Res<Sim>,
    mut theme: ResMut<Theme>,
    mut skin: ResMut<Skin>,
    art: Res<UiArt>,
    roots: Query<Entity, With<HudRoot>>,
    mut built_for: Local<Option<Nation>>,
) {
    let nation = sim.world.nation;
    if *built_for == Some(nation) {
        return;
    }
    *built_for = Some(nation);
    *theme = Theme::for_nation(sim.defs.nation(nation).color);
    *skin = art.skin(nation);
    for e in &roots {
        commands.entity(e).despawn();
    }
    spawn_hud(&mut commands, &theme, &skin);
}

fn spawn_hud(commands: &mut Commands, theme: &Theme, skin: &Skin) {
    let margin = Val::Px(layout::SCREEN_MARGIN);
    let column = |gap: f32| Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(gap),
        ..default()
    };

    let side = Node {
        position_type: PositionType::Absolute,
        right: margin,
        top: margin,
        bottom: margin,
        width: Val::Px(layout::SIDE_PANEL_WIDTH),
        padding: UiRect::all(Val::Px(space::XL)),
        ..column(space::M)
    };
    commands.spawn((HudRoot, ui_style::panel(theme, Frame::Panel, side))).with_children(|p| {
        ui_style::frame(p, theme, skin, Frame::Panel);
        // Clips here, not on the panel: the frame draws outside the panel's edge.
        p.spawn((
            SideContent,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..column(space::M)
            },
        ));
    });

    let bar = Node {
        position_type: PositionType::Absolute,
        left: margin,
        bottom: margin,
        // Up to the side panel, with the same gap as between the panel and the screen edge.
        right: Val::Px(layout::SIDE_PANEL_WIDTH + 2.0 * layout::SCREEN_MARGIN),
        height: Val::Px(layout::SELECTION_BAR_HEIGHT),
        padding: UiRect::all(Val::Px(space::L)),
        align_items: AlignItems::Center,
        ..default()
    };
    commands
        .spawn((
            HudRoot,
            SelectionBar,
            Visibility::Hidden,
            ui_style::panel(theme, Frame::Panel, bar),
        ))
        .with_children(|p| {
            ui_style::frame(p, theme, skin, Frame::Panel);
            p.spawn((
                SelectionContent,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    column_gap: Val::Px(space::XL),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
        });

    // Spawned last so it draws over the other panels.
    let tooltip = Node {
        position_type: PositionType::Absolute,
        padding: UiRect::all(Val::Px(space::M)),
        ..default()
    };
    commands
        .spawn((
            HudRoot,
            Tooltip,
            Visibility::Hidden,
            ui_style::panel(theme, Frame::Tooltip, tooltip),
        ))
        .with_children(|p| {
            ui_style::frame(p, theme, skin, Frame::Tooltip);
            p.spawn((TooltipLines, column(2.0)));
        });
}

// ---------------------------------------------------------------------------------------------
// Side panel

/// The camera and game keys, shown at the foot of the side panel.
const CONTROLS: [&str; 7] = [
    "WASD or drag: pan",
    "Wheel: zoom",
    "Space: pause",
    "1-4: nation",
    "R: new map",
    "T: research",
    "Shift+H: repeating harvest",
];

fn update_side_panel(
    mut commands: Commands,
    sim: Res<Sim>,
    tool: Res<ActiveTool>,
    notice: Res<Notice>,
    choice: Res<BuildChoice>,
    alerts: Res<AlertLog>,
    theme: Res<Theme>,
    skin: Res<Skin>,
    content: Query<Entity, With<SideContent>>,
    fresh: Query<(), Added<SideContent>>,
    mut last: Local<Option<SideView>>,
) {
    let Ok(content) = content.single() else { return };
    let view = view::side_view(&sim, tool.0, &notice.0, choice.0, &alerts);
    if last.as_ref() == Some(&view) && fresh.is_empty() {
        return;
    }
    commands
        .entity(content)
        .despawn_children()
        .with_children(|p| spawn_side(p, &theme, &skin, &view));
    *last = Some(view);
}

fn spawn_side(p: &mut ChildSpawnerCommands, theme: &Theme, skin: &Skin, v: &SideView) {
    // Header: emblem, nation, seed and people.
    p.spawn(Node { column_gap: Val::Px(space::L), align_items: AlignItems::Center, ..default() })
        .with_children(|h| {
            h.spawn(ui_style::emblem(skin));
            h.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(
                |c| {
                    c.spawn(ui_style::text(v.nation.clone(), font::TITLE, theme.text));
                    let people = if v.people == 1 { "person" } else { "people" };
                    let sub = format!("Seed {} - {} {people}", v.seed, v.people);
                    c.spawn(ui_style::text(sub, font::LABEL, theme.dim));
                },
            );
        });

    // Clock, pause state and speed.
    p.spawn(Node {
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|r| {
        r.spawn(ui_style::text(format!("Time {}", v.clock), font::BODY, theme.text));
        let (state, color) = if v.paused { ("Paused", theme.warn) } else { ("Running", theme.dim) };
        r.spawn(ui_style::text(format!("{state}  x{}", v.speed), font::BODY, color));
    });

    spawn_stores(p, theme, skin, &v.stores);

    // The tool the left mouse button has, and the last designation's result.
    ui_style::section_title(p, theme, skin, "Tool");
    p.spawn(Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(space::XS),
        row_gap: Val::Px(space::XS),
        ..default()
    })
    .with_children(|row| {
        for tool in Tool::ALL {
            ui_style::tool_button(row, theme, skin, tool, tool == v.tool);
        }
    });
    p.spawn(ui_style::text(v.tool_hint.clone(), font::BODY, theme.dim));
    spawn_picker(p, theme, &v.picker, v.choice);
    if !v.notice.is_empty() {
        p.spawn(ui_style::text(v.notice.clone(), font::BODY, theme.accent_text));
    }

    // Secondary: the nation's modifiers, only those that differ from the baseline.
    if !v.modifiers.is_empty() {
        ui_style::section_title(p, theme, skin, "Nation");
        for (label, value) in &v.modifiers {
            p.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() })
                .with_children(|r| {
                    r.spawn(ui_style::text(label.clone(), font::BODY, theme.dim));
                    r.spawn(ui_style::text(value.clone(), font::BODY, theme.text));
                });
        }
    }

    // Research (#23): the tech the pool is paying for right now, its progress and rate. The full
    // tree is `T`'s screen (`research.rs`).
    ui_style::section_title(p, theme, skin, "Research");
    p.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(
        |r| {
            r.spawn(ui_style::text(v.research.label.clone(), font::BODY, theme.text));
            r.spawn(ui_style::text(v.research.rate.clone(), font::LABEL, theme.dim));
        },
    );
    p.spawn(Node::default())
        .with_children(|b| ui_style::bar(b, theme, v.research.frac, theme.accent, 8.0));
    p.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(
        |r| {
            r.spawn(ui_style::text(v.research.pool.clone(), font::LABEL, theme.dim));
            if v.research.more_queued > 0 {
                let more = format!("+{} queued", v.research.more_queued);
                r.spawn(ui_style::text(more, font::LABEL, theme.dim));
            }
        },
    );

    // Recent bend events (#72) and tech completions (#23), newest last. Hidden while there's
    // nothing to show.
    if !v.alerts.is_empty() {
        ui_style::section_title(p, theme, skin, "Alerts");
        for line in &v.alerts {
            p.spawn(ui_style::text(line.clone(), font::BODY, theme.accent_text));
        }
    }

    ui_style::section_title(p, theme, skin, "Controls");
    for line in CONTROLS {
        p.spawn(ui_style::text(line, font::LABEL, theme.dim));
    }
}

/// What the stockpiles hold (#130): a two-column grid of an icon in a well, the item's name in
/// `dim` and its count in `text`, in the order of the item data. Hidden while nothing is stored.
fn spawn_stores(p: &mut ChildSpawnerCommands, theme: &Theme, skin: &Skin, rows: &[StoreRow]) {
    if rows.is_empty() {
        return;
    }
    ui_style::section_title(p, theme, skin, "Stores");
    let grid = Node {
        flex_wrap: FlexWrap::Wrap,
        row_gap: Val::Px(space::S),
        width: Val::Percent(100.0),
        ..default()
    };
    p.spawn(grid).with_children(|g| {
        for row in rows {
            let cell = Node {
                width: Val::Percent(50.0),
                column_gap: Val::Px(space::S),
                align_items: AlignItems::Center,
                ..default()
            };
            g.spawn(cell).with_children(|c| {
                ui_style::item_well(c, theme, skin, &row.icon);
                c.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(
                    |t| {
                        t.spawn(ui_style::text(row.name.clone(), font::LABEL, theme.dim));
                        t.spawn(ui_style::text(row.count.to_string(), font::BODY, theme.text));
                    },
                );
            });
        }
    });
}

/// The building picker (#111): a row per building with its cost and work. A building the nation
/// can't place yet is greyed out, says what it requires, and isn't a button. The chosen one is
/// filled with the accent's soft tone.
fn spawn_picker(
    p: &mut ChildSpawnerCommands,
    theme: &Theme,
    rows: &[PickerRow],
    choice: Option<BuildingId>,
) {
    for row in rows {
        let (bg, border) = if choice == Some(row.id) {
            (theme.accent_soft, theme.accent)
        } else {
            (theme.inset, theme.accent_soft)
        };
        let node = Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(space::S)),
            border: UiRect::all(Val::Px(1.0)),
            row_gap: Val::Px(2.0),
            ..default()
        };
        let mut entry = p.spawn((node, BackgroundColor(bg), BorderColor::all(border)));
        if !row.locked() {
            entry.insert(PickerButton(row.id));
        }
        let name_color = if row.locked() { theme.dim } else { theme.text };
        entry.with_children(|r| {
            r.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() })
                .with_children(|l| {
                    l.spawn(ui_style::text(row.name.clone(), font::BODY, name_color));
                    l.spawn(ui_style::text(row.cost.clone(), font::LABEL, theme.dim));
                });
            match &row.requires {
                Some(requires) => {
                    r.spawn(ui_style::text(requires.clone(), font::LABEL, theme.warn))
                }
                None => r.spawn(ui_style::text(row.work.clone(), font::LABEL, theme.dim)),
            };
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Selection bar

/// A meter's colour: the same for every nation (`Theme::ok`, `warn`, `bad`).
fn status_color(status: Status, theme: &Theme) -> Color {
    match status {
        Status::Ok => theme.ok,
        Status::Warn => theme.warn,
        Status::Bad => theme.bad,
    }
}

/// What the selection bar shows: the unit's view and the art id of its portrait.
#[derive(Clone, PartialEq, Debug)]
struct SelectionView {
    unit: UnitView,
    character: String,
}

fn update_selection_bar(
    mut commands: Commands,
    sim: Res<Sim>,
    art: Res<Art>,
    selection: Res<Selection>,
    screen_open: Res<ScreenOpen>,
    theme: Res<Theme>,
    mut bar: Query<&mut Visibility, With<SelectionBar>>,
    content: Query<Entity, With<SelectionContent>>,
    fresh: Query<(), Added<SelectionContent>>,
    mut last: Local<Option<SelectionView>>,
) {
    let (Ok(mut visibility), Ok(content)) = (bar.single_mut(), content.single()) else { return };
    // The research screen (#23) is a full `ui.md` "Screens" overlay: the selection bar hides
    // while it's open so it never fights the tech cards for space or clicks.
    if screen_open.0 {
        *visibility = Visibility::Hidden;
        return;
    }
    let Some(unit) = selection.0.and_then(|id| sim.world.unit(id)) else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Inherited;
    let view = SelectionView {
        unit: view::unit_view(&sim, unit),
        character: unit_character(&sim, unit).to_owned(),
    };
    if last.as_ref() == Some(&view) && fresh.is_empty() {
        return;
    }
    let face = art.character(&view.character).face.clone();
    commands
        .entity(content)
        .despawn_children()
        .with_children(|p| spawn_selection(p, &theme, &view.unit, face));
    *last = Some(view);
}

fn spawn_selection(p: &mut ChildSpawnerCommands, theme: &Theme, v: &UnitView, face: Handle<Image>) {
    // Portrait: the faceset at an integer scale on an inset well.
    let well = Val::Px(PORTRAIT_WELL);
    p.spawn((
        Node {
            width: well,
            height: well,
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(theme.inset),
        BorderColor::all(theme.accent_soft),
    ))
    .with_children(|w| {
        w.spawn((
            ImageNode::new(face),
            Node {
                width: Val::Px(crate::sprites::FACE_SIZE * PORTRAIT_SCALE),
                height: Val::Px(crate::sprites::FACE_SIZE * PORTRAIT_SCALE),
                ..default()
            },
        ));
    });

    // Name, nation, current task and what it carries.
    p.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        row_gap: Val::Px(2.0),
        ..default()
    })
    .with_children(|c| {
        c.spawn(ui_style::text(v.name.clone(), font::NAME, theme.text));
        let place = format!("{} - ({}, {})", v.nation, v.tile.x, v.tile.y);
        c.spawn(ui_style::text(place, font::BODY, theme.accent_text));
        c.spawn(ui_style::text(v.task.clone(), font::BODY, theme.text));
        c.spawn(ui_style::text(format!("Carrying: {}", v.carrying), font::BODY, theme.dim));
        c.spawn(ui_style::text("Esc: close", font::LABEL, theme.dim));
    });

    // Needs, then the chi bar (#72) last in the same column.
    p.spawn(Node {
        width: Val::Px(NEEDS_WIDTH),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(space::XS),
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|col| {
        for need in &v.needs {
            col.spawn(meter_row_node()).with_children(|row| {
                meter_row(
                    row,
                    theme,
                    need.label,
                    need.frac,
                    status_color(need.status, theme),
                    &need.percent(),
                );
            });
        }
        col.spawn(meter_row_node()).with_children(|row| {
            let percent = format!("{:.0}%", v.chi_frac * 100.0);
            meter_row(row, theme, "Chi", v.chi_frac, theme.accent, &percent);
        });
    });
}

fn meter_row_node() -> Node {
    Node { column_gap: Val::Px(space::S), align_items: AlignItems::Center, ..default() }
}

/// One meter row: a short label, a fill bar and a right-aligned percent, shared by the needs and
/// the chi bar.
fn meter_row(
    row: &mut ChildSpawnerCommands,
    theme: &Theme,
    label: &str,
    frac: f32,
    fill: Color,
    percent: &str,
) {
    row.spawn((
        Node { width: Val::Px(52.0), flex_shrink: 0.0, ..default() },
        children![ui_style::text(label.to_owned(), font::LABEL, theme.dim)],
    ));
    row.spawn(Node { flex_grow: 1.0, ..default() })
        .with_children(|b| ui_style::bar(b, theme, frac, fill, 8.0));
    row.spawn((
        Node {
            width: Val::Px(36.0),
            justify_content: JustifyContent::End,
            flex_shrink: 0.0,
            ..default()
        },
        children![ui_style::text(percent.to_owned(), font::LABEL, theme.text)],
    ));
}

// ---------------------------------------------------------------------------------------------
// Tooltip

/// A hint line in this frontend's words: the mouse clicks and drags.
fn hint_text(hint: Hint) -> String {
    match hint {
        Hint::SelectUnit => "Click to select".into(),
        Hint::ApplyTool(Tool::CancelHarvest) => "Click to cancel the build".into(),
        Hint::ApplyTool(Tool::Harvest) => "Drag to designate (Shift: repeating)".into(),
        Hint::ApplyTool(tool) => format!("Drag to {}", tool.label()),
        Hint::StartTool(tool) => {
            let then = match tool {
                Tool::CancelHarvest => "click to cancel",
                Tool::Harvest => "drag to harvest",
                other => other.label(),
            };
            format!("[{}] then {then}", tool.key())
        }
    }
}

/// How much of the view the panels cover: the width the side panel takes from the
/// right and the height the selection bar takes from the bottom, margins included. The camera
/// centres a selected unit in what is left. Only called while a unit is selected, so the bar
/// counts.
pub fn covered_by_panels() -> (f32, f32) {
    (
        layout::SIDE_PANEL_WIDTH + 2.0 * layout::SCREEN_MARGIN,
        layout::SELECTION_BAR_HEIGHT + 2.0 * layout::SCREEN_MARGIN,
    )
}

/// Whether the cursor is over the side panel.
pub(crate) fn over_side_panel(cursor: Vec2, window: Vec2) -> bool {
    over_panel(cursor, window, false)
}

/// Whether the cursor is over the side panel or the open selection bar, where the map tooltip
/// would be both wrong (the cursor isn't over the map) and in the way.
fn over_panel(cursor: Vec2, window: Vec2, bar_open: bool) -> bool {
    let side_left = window.x - layout::SCREEN_MARGIN - layout::SIDE_PANEL_WIDTH;
    let bar_top = window.y - layout::SCREEN_MARGIN - layout::SELECTION_BAR_HEIGHT;
    cursor.x >= side_left || (bar_open && cursor.y >= bar_top)
}

/// The tooltip's position as `(left, right, top, bottom)` insets. It sits below and right of the
/// cursor, flipping to the other side of it near the screen's right and bottom edges, so it never
/// leaves the screen and needs no measured size.
fn tooltip_insets(cursor: Vec2, window: Vec2) -> (Val, Val, Val, Val) {
    let (left, right) = if cursor.x > window.x / 2.0 {
        (Val::Auto, Val::Px(window.x - cursor.x + TOOLTIP_GAP))
    } else {
        (Val::Px(cursor.x + TOOLTIP_GAP), Val::Auto)
    };
    let (top, bottom) = if cursor.y > window.y / 2.0 {
        (Val::Auto, Val::Px(window.y - cursor.y + TOOLTIP_GAP))
    } else {
        (Val::Px(cursor.y + TOOLTIP_GAP), Val::Auto)
    };
    (left, right, top, bottom)
}

fn update_tooltip(
    mut commands: Commands,
    sim: Res<Sim>,
    tool: Res<ActiveTool>,
    theme: Res<Theme>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    bar: Query<&Visibility, (With<SelectionBar>, Without<Tooltip>)>,
    mut tooltip: Query<(&mut Node, &mut Visibility), With<Tooltip>>,
    lines: Query<Entity, With<TooltipLines>>,
    fresh: Query<(), Added<TooltipLines>>,
    mut last: Local<inspect::Lines>,
) {
    let (Ok((mut node, mut visibility)), Ok(lines_entity)) = (tooltip.single_mut(), lines.single())
    else {
        return;
    };
    let hovered = (|| {
        let (window, (camera, camera_tf)) = (window.single().ok()?, camera.single().ok()?);
        let cursor = window.cursor_position()?;
        let bar_open = bar.single().is_ok_and(|v| *v != Visibility::Hidden);
        if over_panel(cursor, window.size(), bar_open) {
            return None;
        }
        let text = inspect::tooltip_lines(&sim, tool.0, cursor_tile(window, camera, camera_tf)?)?;
        Some((cursor, window.size(), text))
    })();
    let Some((cursor, size, text)) = hovered else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Inherited;
    (node.left, node.right, node.top, node.bottom) = tooltip_insets(cursor, size);
    if *last == text && fresh.is_empty() {
        return;
    }
    commands.entity(lines_entity).despawn_children().with_children(|p| {
        for line in &text {
            let (color, text) = match line {
                Line::Title(t) => (theme.text, t.clone()),
                Line::Detail(t) => (theme.dim, t.clone()),
                Line::Hint(h) => (theme.accent_text, hint_text(*h)),
            };
            p.spawn(ui_style::text(text, font::BODY, color));
        }
    });
    *last = text;
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use fn_ui::Session;
    use fn_ui::build::picker_rows;

    use super::*;

    #[test]
    fn status_colours_come_from_the_theme() {
        let t = Theme::for_nation([70, 130, 210]);
        assert_eq!(status_color(Status::Ok, &t), t.ok);
        assert_eq!(status_color(Status::Warn, &t), t.warn);
        assert_eq!(status_color(Status::Bad, &t), t.bad);
    }

    #[test]
    fn panels_cover_the_cursor_where_they_are() {
        let window = Vec2::new(1280.0, 720.0);
        let map = Vec2::new(300.0, 300.0);
        assert!(!over_panel(map, window, true));
        assert!(over_panel(Vec2::new(1100.0, 100.0), window, false));
        // The bar only counts while it's open.
        let bar = Vec2::new(300.0, 650.0);
        assert!(over_panel(bar, window, true));
        assert!(!over_panel(bar, window, false));
    }

    #[test]
    fn tooltip_flips_toward_the_screen_centre() {
        let window = Vec2::new(1000.0, 600.0);
        let (l, r, t, b) = tooltip_insets(Vec2::new(100.0, 100.0), window);
        assert_eq!((l, r, t, b), (Val::Px(116.0), Val::Auto, Val::Px(116.0), Val::Auto));
        let (l, r, t, b) = tooltip_insets(Vec2::new(900.0, 500.0), window);
        assert_eq!((l, r, t, b), (Val::Auto, Val::Px(116.0), Val::Auto, Val::Px(116.0)));
    }

    /// The mouse wording the client has always used.
    #[test]
    fn hints_read_as_mouse_actions() {
        assert_eq!(hint_text(Hint::SelectUnit), "Click to select");
        assert_eq!(
            hint_text(Hint::ApplyTool(Tool::Harvest)),
            "Drag to designate (Shift: repeating)"
        );
        assert_eq!(hint_text(Hint::StartTool(Tool::Harvest)), "[H] then drag to harvest");
        assert_eq!(hint_text(Hint::ApplyTool(Tool::CancelHarvest)), "Click to cancel the build");
        assert_eq!(hint_text(Hint::StartTool(Tool::CancelHarvest)), "[X] then click to cancel");
    }

    #[test]
    fn only_placeable_picker_rows_are_clickable() {
        let session = Session::new(7, Nation::Earth, 64, 64);
        let rows = picker_rows(&session);
        let theme = Theme::for_nation(session.defs.nation(Nation::Earth).color);
        let mut world = bevy::ecs::world::World::new();
        let root = world.spawn(Node::default()).id();
        world.commands().entity(root).with_children(|p| spawn_picker(p, &theme, &rows, None));
        world.flush();
        let clickable: Vec<BuildingId> =
            world.query::<&PickerButton>().iter(&world).map(|b| b.0).collect();
        let open: Vec<BuildingId> = rows.iter().filter(|r| !r.locked()).map(|r| r.id).collect();
        assert_eq!(clickable, open);
        assert!(rows.iter().any(PickerRow::locked), "Earth can't build a campfire yet");
        assert_eq!(
            world.query::<&Node>().iter(&world).filter(|n| n.border.left == Val::Px(1.0)).count(),
            rows.len()
        );
    }
}

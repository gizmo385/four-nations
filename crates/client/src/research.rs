//! Research (#23): the full tech-tree screen. What it shows is `fn_ui::research::screen_view` (the
//! side panel's progress line is `fn_ui::research::research_status`); this module draws it and
//! turns a click on a card into `Command::SetResearch`. `fn_sim` does the rest
//! (`docs/design/research.md`).
//!
//! `T` toggles the screen; `Esc` closes it. It is centred over a dimmed map (ui.md "Screens"): the
//! side panel stays visible, techs are laid out in columns by depth (tier), and clicking a tech
//! card sets it as the research target. A tech already known is a no-op if clicked; one with
//! unmet prereqs queues its whole prereq chain (`Research::set_target`), which is why a queued
//! prereq is drawn the same as the target itself.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use fn_sim::Nation;
use fn_sim::research::TechId;
use fn_sim::world::Command;
use fn_ui::research::{ScreenView, TechCard, TechState, screen_view};

use crate::Sim;
use crate::ui_style::{self, Frame, Skin, Theme, font, layout, space};

pub struct ResearchPlugin;

impl Plugin for ResearchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenOpen>().add_systems(
            Update,
            (toggle_screen, sync_screen_root, update_screen_content, handle_tech_click).chain(),
        );
    }
}

/// Whether the tech-tree screen is open. Other input systems (camera, selection, designation)
/// read this to stay out of the screen's way while it's up.
#[derive(Resource, Default)]
pub struct ScreenOpen(pub bool);

/// Toggles the screen with `T`; `Esc` only closes it (so it never fights `designate`'s own `Esc`
/// handling for leaving a tool).
fn toggle_screen(keys: Res<ButtonInput<KeyCode>>, mut open: ResMut<ScreenOpen>) {
    if keys.just_pressed(KeyCode::KeyT) {
        open.0 = !open.0;
    } else if open.0 && keys.just_pressed(KeyCode::Escape) {
        open.0 = false;
    }
}

// ---------------------------------------------------------------------------------------------
// The tech-tree screen (its view, `fn_ui::research::screen_view`, is plain data)

/// Marks everything the screen spawns at the top level, so it can be despawned on close or when
/// the player's nation changes.
#[derive(Component)]
struct ScreenRoot;

/// The columns' container; rebuilt when its `ScreenView` changes.
#[derive(Component)]
struct ScreenGrid;

/// A clickable tech card; `handle_tech_click` hit-tests the cursor against these.
#[derive(Component)]
struct TechButton(TechId);

const CARD_WIDTH: f32 = 168.0;
/// Fits the deepest bundled chain (5 tiers, `docs/design/research.md`'s Earth branch) in the area
/// left of the side panel at the 1280×720 reference size, with room to spare for a narrower one.
const PANEL_MAX_WIDTH: f32 = 960.0;
const COLUMN_GAP: f32 = space::M;

fn spawn_screen(commands: &mut Commands, theme: &Theme, skin: &Skin) {
    // Covers everything left of the side panel, full height, and dims the map under it.
    let dim = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        bottom: Val::Px(0.0),
        right: Val::Px(layout::SIDE_PANEL_WIDTH + 2.0 * layout::SCREEN_MARGIN),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };
    commands
        .spawn((ScreenRoot, dim, BackgroundColor(Color::BLACK.with_alpha(0.55))))
        .with_children(|root| {
            let panel = Node {
                max_width: Val::Px(PANEL_MAX_WIDTH),
                max_height: Val::Percent(90.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::M),
                padding: UiRect::all(Val::Px(space::XL)),
                ..default()
            };
            root.spawn(ui_style::panel(theme, Frame::Panel, panel)).with_children(|p| {
                ui_style::frame(p, theme, skin, Frame::Panel);
                p.spawn(ui_style::text("Research", font::TITLE, theme.text));
                p.spawn((
                    ScreenGrid,
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(COLUMN_GAP),
                        align_items: AlignItems::FlexStart,
                        // The panel itself can't clip: its frame draws outside it.
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ));
                p.spawn(ui_style::text(
                    "Click a tech to set it as your target. Esc: close",
                    font::LABEL,
                    theme.dim,
                ));
            });
        });
}

fn spawn_tech_card(p: &mut ChildSpawnerCommands, theme: &Theme, card: &TechCard) {
    let (bg, border, fg) = match card.state {
        TechState::Known => (theme.inset, theme.accent_soft, theme.dim),
        TechState::Queued => (theme.accent, theme.accent, theme.on_accent),
        TechState::Available => (theme.inset, theme.accent, theme.text),
        TechState::Locked => (theme.inset, theme.accent_soft, theme.dim),
    };
    let node = Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(2.0),
        padding: UiRect::all(Val::Px(space::S)),
        border: UiRect::all(Val::Px(1.0)),
        width: Val::Px(CARD_WIDTH),
        ..default()
    };
    p.spawn((TechButton(card.id), node, BackgroundColor(bg), BorderColor::all(border)))
        .with_children(|b| {
            b.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() })
                .with_children(|r| {
                    r.spawn(ui_style::text(card.name.clone(), font::BODY, fg));
                    r.spawn(ui_style::text(card.cost.to_string(), font::LABEL, fg));
                });
            let state_color = if card.state == TechState::Queued { fg } else { theme.accent_text };
            b.spawn(ui_style::text(card.state.label(), font::LABEL, state_color));
            b.spawn(ui_style::text(card.unlock.clone(), font::LABEL, theme.dim));
        });
}

fn spawn_columns(p: &mut ChildSpawnerCommands, theme: &Theme, skin: &Skin, view: &ScreenView) {
    for (depth, column) in view.columns.iter().enumerate() {
        if column.is_empty() {
            continue;
        }
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S),
            width: Val::Px(CARD_WIDTH),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|col| {
            ui_style::section_title(col, theme, skin, &format!("Tier {depth}"));
            for card in column {
                spawn_tech_card(col, theme, card);
            }
        });
    }
}

/// Spawns the screen on open, despawns it on close, and rebuilds it if the player's nation
/// changes while it's open (so it never shows a stale nation's branch).
fn sync_screen_root(
    mut commands: Commands,
    open: Res<ScreenOpen>,
    sim: Res<Sim>,
    theme: Res<Theme>,
    skin: Res<Skin>,
    roots: Query<Entity, With<ScreenRoot>>,
    mut shown_for: Local<Option<Nation>>,
) {
    if !open.0 {
        if shown_for.take().is_some() {
            for e in &roots {
                commands.entity(e).despawn();
            }
        }
        return;
    }
    if *shown_for == Some(sim.world.nation) {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    *shown_for = Some(sim.world.nation);
    spawn_screen(&mut commands, &theme, &skin);
}

fn update_screen_content(
    mut commands: Commands,
    open: Res<ScreenOpen>,
    sim: Res<Sim>,
    theme: Res<Theme>,
    skin: Res<Skin>,
    grid: Query<Entity, With<ScreenGrid>>,
    fresh: Query<(), Added<ScreenGrid>>,
    mut last: Local<Option<ScreenView>>,
) {
    if !open.0 {
        return;
    }
    let Ok(grid) = grid.single() else { return };
    let view = screen_view(&sim);
    if last.as_ref() == Some(&view) && fresh.is_empty() {
        return;
    }
    commands
        .entity(grid)
        .despawn_children()
        .with_children(|p| spawn_columns(p, &theme, &skin, &view));
    *last = Some(view);
}

/// Issues `Command::SetResearch` for the tech card under the cursor, if any, on a left click.
/// Clicking a known tech is a harmless no-op (`ResearchError::AlreadyKnown`, ignored); clicking a
/// locked one queues its whole prereq chain (`Research::set_target`).
fn handle_tech_click(
    mouse: Res<ButtonInput<MouseButton>>,
    open: Res<ScreenOpen>,
    mut sim: ResMut<Sim>,
    window: Query<&Window, With<PrimaryWindow>>,
    buttons: Query<(&TechButton, &ComputedNode, &UiGlobalTransform)>,
) {
    if !open.0 || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = window.single() else { return };
    let hit = ui_style::node_under_cursor(window, buttons.iter().map(|(b, n, t)| (b.0, n, t)));
    if let Some(tech) = hit {
        let _ = sim.apply(Command::SetResearch { tech });
    }
}

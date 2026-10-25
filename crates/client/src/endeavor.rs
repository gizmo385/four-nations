//! The Endeavor screen (#235, `docs/design/endgame.md`, "Showing progress"): the road to the end
//! game. What it shows is `fn_ui::endeavor::screen_view` (the side panel's section is
//! `fn_ui::endeavor::endeavor_status`, the toasts `fn_ui::alerts`); this module only draws it.
//!
//! `G` toggles the screen; `Esc` closes it, and so does opening the research screen (`T`), since
//! only one screen covers the map at a time. Like research it is centred over the dimmed map with
//! the side panel still visible, and map input is suspended while it is up (`crate::screen_closed`).
//! On the left the Harmony Gate is drawn in grey and coloured in from the bottom up to the share
//! of signs met; on the right are the signs under their headings, each with a check when met, a
//! hint when not, its count and a bar.

use bevy::prelude::*;
use fn_sim::Nation;
use fn_ui::endeavor::{ScreenView, SignRow, screen_view};

use crate::Sim;
use crate::research::ScreenOpen;
use crate::save::{LIST_KEY, SavesOpen};
use crate::ui_style::{self, Frame, Skin, Theme, font, layout, space};

pub struct EndeavorPlugin;

impl Plugin for EndeavorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GreyGate>().add_systems(
            Update,
            (toggle_screen, make_grey_gate, sync_screen_root, update_screen_content).chain(),
        );
    }
}

/// Whether the Endeavor screen is open. `crate::screen_closed` reads it, with research's
/// `ScreenOpen` and the save list's `SavesOpen`, to keep map input out of the way while a screen is up.
#[derive(Resource, Default)]
pub struct EndeavorOpen(pub bool);

/// `G` toggles the screen and closes research and the save list; `T` or `F6` (their keys) close
/// it; `Esc` only closes it, so it never fights `designate`'s own `Esc`.
fn toggle_screen(
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<EndeavorOpen>,
    mut research: ResMut<ScreenOpen>,
    mut saves: ResMut<SavesOpen>,
) {
    if keys.just_pressed(KeyCode::KeyG) {
        open.0 = !open.0;
        if open.0 {
            research.0 = false;
            saves.0 = false;
        }
    } else if open.0 && keys.any_just_pressed([KeyCode::Escape, KeyCode::KeyT, LIST_KEY]) {
        open.0 = false;
    }
}

// ---------------------------------------------------------------------------------------------
// The gate picture

/// One sprite cell of `buildings_endeavor.png` (`art/buildings_endeavor.toml`).
const GATE_CELL: Vec2 = Vec2::new(64.0, 80.0);
/// The finished gate with its emblem unlit is row 3 plus the nation's place in the data, column 0.
const COMPLETE_ROW: f32 = 3.0;
/// The gate is drawn at three times its pixel size: 192x240.
const GATE_SCALE: f32 = 3.0;

/// A greyscale copy of the gate sheet, made once the sheet has loaded, for the part of the gate
/// the signs haven't coloured in yet.
#[derive(Resource, Default)]
struct GreyGate(Option<Handle<Image>>);

/// Makes `GreyGate` from the skin's gate sheet the first frame the sheet is loaded.
fn make_grey_gate(skin: Res<Skin>, mut images: ResMut<Assets<Image>>, mut grey: ResMut<GreyGate>) {
    if grey.0.is_some() {
        return;
    }
    let Some(sheet) = images.get(&skin.gate) else { return };
    let mut copy = sheet.clone();
    if let Some(data) = copy.data.as_mut() {
        greyscale(data);
    }
    grey.0 = Some(images.add(copy));
}

/// Turns RGBA8 pixels into a pale grey of their own brightness, leaving alpha alone: stone that
/// isn't built yet.
fn greyscale(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let [r, g, b] = [px[0], px[1], px[2]].map(f32::from);
        let luma = 0.299 * r + 0.587 * g + 0.114 * b;
        // Lift it towards white so the unbuilt part reads as a pale sketch, not a shadow.
        let v = (luma * 0.6 + 255.0 * 0.3).round().clamp(0.0, 255.0) as u8;
        px[..3].fill(v);
    }
}

/// The finished gate's cell for `nation` (data order picks the roof colour).
fn gate_rect(defs: &fn_sim::Defs, nation: Nation) -> Rect {
    let index = defs.nations.iter().position(|n| n.id == nation).unwrap_or(0) as f32;
    let min = Vec2::new(0.0, (COMPLETE_ROW + index) * GATE_CELL.y);
    Rect::from_corners(min, min + GATE_CELL)
}

/// The gate: the grey copy, with the coloured sheet over it clipped to the bottom `frac`.
fn spawn_gate(
    p: &mut ChildSpawnerCommands,
    colour: &Handle<Image>,
    grey: &Handle<Image>,
    rect: Rect,
    frac: f32,
) {
    let size = GATE_CELL * GATE_SCALE;
    let picture = |image: &Handle<Image>| {
        (
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(size.x),
                height: Val::Px(size.y),
                ..default()
            },
            ImageNode { image: image.clone(), rect: Some(rect), ..default() },
        )
    };
    p.spawn(Node {
        width: Val::Px(size.x),
        height: Val::Px(size.y),
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|g| {
        g.spawn(picture(grey));
        g.spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Px(size.x),
            // Whole art pixels, so the edge between grey and colour never splits one.
            height: Val::Px((frac.clamp(0.0, 1.0) * GATE_CELL.y).round() * GATE_SCALE),
            overflow: Overflow::clip(),
            ..default()
        })
        .with_children(|c| {
            c.spawn(picture(colour));
        });
    });
}

// ---------------------------------------------------------------------------------------------
// The screen

/// Everything the screen spawns at the top level.
#[derive(Component)]
struct ScreenRoot;

/// The panel's contents, rebuilt when the `ScreenView` changes.
#[derive(Component)]
struct ScreenBody;

/// Wide enough for a sign's name, its hint and its count on one line beside the gate.
const PANEL_WIDTH: f32 = 900.0;
const SIGNS_WIDTH: f32 = 600.0;
const BAR_WIDTH: f32 = 120.0;
/// The check box beside each sign.
const CHECK: f32 = 10.0;

fn spawn_screen(commands: &mut Commands, theme: &Theme, skin: &Skin) {
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
                width: Val::Px(PANEL_WIDTH),
                max_width: Val::Percent(96.0),
                max_height: Val::Percent(94.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::M),
                padding: UiRect::all(Val::Px(space::XL)),
                ..default()
            };
            root.spawn(ui_style::panel(theme, Frame::Panel, panel)).with_children(|p| {
                ui_style::frame(p, theme, skin, Frame::Panel);
                p.spawn((
                    ScreenBody,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(space::M),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ));
            });
        });
}

fn spawn_body(
    p: &mut ChildSpawnerCommands,
    theme: &Theme,
    skin: &Skin,
    gate: Gate,
    v: &ScreenView,
) {
    p.spawn(Node {
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Baseline,
        ..default()
    })
    .with_children(|r| {
        r.spawn(ui_style::text(v.name.clone(), font::NAME, theme.text));
        r.spawn(ui_style::text(v.count.clone(), font::BODY, theme.accent_text));
    });
    if !v.description.is_empty() {
        p.spawn(ui_style::text(v.description.clone(), font::LABEL, theme.dim));
    }
    p.spawn(Node {
        column_gap: Val::Px(space::XL),
        align_items: AlignItems::FlexStart,
        ..default()
    })
    .with_children(|row| {
        // The gate in a well, with where the endeavor stands under it.
        row.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(space::M),
                padding: UiRect::all(Val::Px(space::XL)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(theme.inset),
        ))
        .with_children(|w| {
            if let Some(grey) = &gate.grey {
                spawn_gate(w, gate.colour, grey, gate.rect, v.frac);
            }
            w.spawn(ui_style::text(v.stage.clone(), font::BODY, theme.text));
            w.spawn(ui_style::text("G or Esc: close", font::LABEL, theme.dim));
        });
        row.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::XS),
            width: Val::Px(SIGNS_WIDTH),
            flex_shrink: 1.0,
            ..default()
        })
        .with_children(|col| {
            for group in &v.groups {
                ui_style::section_title(col, theme, skin, &group.heading);
                for sign in &group.signs {
                    spawn_sign(col, theme, sign);
                }
            }
        });
    });
    p.spawn(ui_style::text(
        "Every sign counts from your own settlement. Stores eaten after the Avatar hears don't undo it.",
        font::LABEL,
        theme.dim,
    ));
}

fn spawn_sign(p: &mut ChildSpawnerCommands, theme: &Theme, sign: &SignRow) {
    let (name_color, value_color) =
        if sign.met { (theme.dim, theme.ok) } else { (theme.text, theme.text) };
    p.spawn(Node {
        column_gap: Val::Px(space::M),
        align_items: AlignItems::Center,
        padding: UiRect::left(Val::Px(space::S)),
        ..default()
    })
    .with_children(|r| {
        // A filled box for a sign met, an empty one for a sign still to meet.
        let (fill, border) = if sign.met { (theme.ok, theme.ok) } else { (theme.inset, theme.dim) };
        r.spawn((
            Node {
                width: Val::Px(CHECK),
                height: Val::Px(CHECK),
                border: UiRect::all(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(fill),
            BorderColor::all(border),
        ));
        // The name and count never wrap; a long hint is the part that gives way.
        r.spawn((ui_style::text(sign.name.clone(), font::BODY, name_color), TextLayout::no_wrap()));
        if let Some(hint) = &sign.hint {
            r.spawn(ui_style::text(hint.clone(), font::LABEL, theme.warn));
        }
        // Pushes the count and the bar to the right edge.
        r.spawn(Node { flex_grow: 1.0, ..default() });
        r.spawn((
            ui_style::text(sign.value.clone(), font::BODY, value_color),
            TextLayout::no_wrap(),
        ));
        r.spawn(Node { width: Val::Px(BAR_WIDTH), flex_shrink: 0.0, ..default() })
            .with_children(|b| ui_style::bar(b, theme, sign.frac, theme.ok, 8.0));
    });
}

/// The two sheets and the cell the gate is drawn from.
#[derive(Clone, Copy)]
struct Gate<'a> {
    colour: &'a Handle<Image>,
    grey: &'a Option<Handle<Image>>,
    rect: Rect,
}

/// Spawns the screen on open and despawns it on close; rebuilds it when the player's nation
/// changes while it's open (the theme and the gate's roof change with it).
fn sync_screen_root(
    mut commands: Commands,
    open: Res<EndeavorOpen>,
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

#[allow(clippy::too_many_arguments)]
fn update_screen_content(
    mut commands: Commands,
    open: Res<EndeavorOpen>,
    sim: Res<Sim>,
    theme: Res<Theme>,
    skin: Res<Skin>,
    grey: Res<GreyGate>,
    body: Query<Entity, With<ScreenBody>>,
    fresh: Query<(), Added<ScreenBody>>,
    mut last: Local<Option<(ScreenView, bool)>>,
) {
    if !open.0 {
        return;
    }
    let Ok(body) = body.single() else { return };
    // The grey copy arriving is a change too: the gate appears with it.
    let view = (screen_view(&sim), grey.0.is_some());
    if last.as_ref() == Some(&view) && fresh.is_empty() {
        return;
    }
    let gate =
        Gate { colour: &skin.gate, grey: &grey.0, rect: gate_rect(&sim.defs, sim.world.nation) };
    commands
        .entity(body)
        .despawn_children()
        .with_children(|p| spawn_body(p, &theme, &skin, gate, &view.0));
    *last = Some(view);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greyscale_keeps_alpha_and_brightness_order() {
        let mut px = [255, 0, 0, 255, 0, 0, 0, 0, 255, 255, 255, 128];
        greyscale(&mut px);
        assert!(px[0] == px[1] && px[1] == px[2], "grey");
        assert_eq!(px[3], 255);
        assert_eq!(px[7], 0, "transparent stays transparent");
        assert_eq!(px[11], 128);
        assert!(px[8] > px[0] && px[0] > px[4], "white > red > black");
    }

    #[test]
    fn each_nation_has_its_own_gate_row() {
        let defs = fn_sim::Defs::bundled();
        let rows: Vec<f32> = Nation::ALL.iter().map(|&n| gate_rect(&defs, n).min.y).collect();
        assert_eq!(rows, [240.0, 320.0, 400.0, 480.0]);
        assert_eq!(gate_rect(&defs, Nation::Earth).size(), GATE_CELL);
    }

    #[test]
    fn g_opens_and_closes_the_screen_and_t_f6_or_esc_close_it() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<EndeavorOpen>()
            .insert_resource(ScreenOpen(true))
            .insert_resource(SavesOpen(true))
            .add_systems(Update, toggle_screen);
        let press = |app: &mut App, key| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(key);
            app.update();
            (app.world().resource::<EndeavorOpen>().0, app.world().resource::<ScreenOpen>().0)
        };
        assert_eq!(press(&mut app, KeyCode::KeyG), (true, false), "G opens it and closes research");
        assert_eq!(press(&mut app, KeyCode::KeyG), (false, false));
        press(&mut app, KeyCode::KeyG);
        assert_eq!(press(&mut app, KeyCode::Escape), (false, false));
        press(&mut app, KeyCode::KeyG);
        assert!(!press(&mut app, KeyCode::KeyT).0, "T closes it (research opens itself)");
        app.world_mut().resource_mut::<SavesOpen>().0 = true;
        press(&mut app, KeyCode::KeyG);
        assert!(!app.world().resource::<SavesOpen>().0, "G closes the save list");
        assert!(!press(&mut app, LIST_KEY).0, "F6 closes it (the list opens itself)");
    }
}

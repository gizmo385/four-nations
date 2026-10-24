//! The HUD (docs/design/ui.md): the side panel on the right, the selection bar along the bottom
//! and the hover tooltip, all built from `ui_style`. Owns no game rules: it reads the sim and
//! shows it.
//!
//! Each panel keeps a small *view* of what it shows (plain data, `PartialEq`) and rebuilds its
//! contents only when the view changes, so a text that changes once a second isn't re-laid-out
//! every frame. The view functions are pure, which is what the tests cover.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use fn_sim::nation::Modifiers;
use fn_sim::world::{TICKS_PER_SECOND, Unit, UnitId};
use fn_sim::{Nation, Pos};

use crate::bending::Alerts;
use crate::designate::{Notice, Tool, ability_for_nation};
use crate::research::{self, CompletionLog, ResearchStatus, ScreenOpen};
use crate::ui_style::{self, Frame, Theme, font, layout, space};
use crate::{Art, Selection, Sim, cursor_tile, jobview, unit_character};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        let sim = app.world().resource::<Sim>();
        let theme = Theme::for_nation(sim.defs.nation(sim.world.nation).color);
        app.insert_resource(theme).add_systems(
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
/// In-game speed shown in the side panel. The sim runs at one fixed rate for now.
const SPEED: u32 = 1;

// ---------------------------------------------------------------------------------------------
// Root and theme

/// Builds the HUD on the first frame, and rebuilds it with a fresh `Theme` whenever the player's
/// nation changes (keys 1 to 4).
fn sync_hud_root(
    mut commands: Commands,
    sim: Res<Sim>,
    mut theme: ResMut<Theme>,
    roots: Query<Entity, With<HudRoot>>,
    mut built_for: Local<Option<Nation>>,
) {
    let nation = sim.world.nation;
    if *built_for == Some(nation) {
        return;
    }
    *built_for = Some(nation);
    *theme = Theme::for_nation(sim.defs.nation(nation).color);
    for e in &roots {
        commands.entity(e).despawn();
    }
    spawn_hud(&mut commands, &theme);
}

fn spawn_hud(commands: &mut Commands, theme: &Theme) {
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
        overflow: Overflow::clip(),
        ..column(space::M)
    };
    commands.spawn((HudRoot, ui_style::panel(theme, side))).with_children(|p| {
        ui_style::frame(p, theme, Frame::Panel);
        p.spawn((SideContent, Node { width: Val::Percent(100.0), ..column(space::M) }));
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
        .spawn((HudRoot, SelectionBar, Visibility::Hidden, ui_style::panel(theme, bar)))
        .with_children(|p| {
            ui_style::frame(p, theme, Frame::Panel);
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
        .spawn((HudRoot, Tooltip, Visibility::Hidden, ui_style::panel(theme, tooltip)))
        .with_children(|p| {
            ui_style::frame(p, theme, Frame::Tooltip);
            p.spawn((TooltipLines, column(2.0)));
        });
}

// ---------------------------------------------------------------------------------------------
// Side panel

/// The tools the side panel lists: the tool, its label and its key.
const TOOLS: [(Tool, &str, &str); 5] = [
    (Tool::Select, "Select", "Esc"),
    (Tool::Harvest, "Harvest", "H"),
    (Tool::CancelHarvest, "Cancel", "X"),
    (Tool::Stockpile, "Stockpile", "P"),
    (Tool::Bend, "Bend", "B"),
];

/// The camera and game keys, shown at the foot of the side panel.
const CONTROLS: [&str; 6] = [
    "WASD or drag: pan",
    "Wheel: zoom",
    "Space: pause",
    "1-4: nation",
    "R: new map",
    "T: research",
];

/// Everything the side panel shows.
#[derive(Clone, PartialEq, Debug)]
struct SideView {
    nation: String,
    seed: u64,
    people: usize,
    clock: String,
    paused: bool,
    tool: Tool,
    /// What the current tool does, shown under its buttons. Generic for most tools; for `Bend`
    /// it names the nation's actual ability (#72).
    tool_hint: String,
    notice: String,
    /// The nation's modifiers that differ from 1.0, as (label, value).
    modifiers: Vec<(String, String)>,
    /// The tech the knowledge pool is currently paying for, its progress and rate (#23).
    research: ResearchStatus,
    /// Recent `BendEvent`s and tech completions, newest last. Empty most of the time, which
    /// hides the section (ui.md: "a section with nothing to show hides itself").
    alerts: Vec<String>,
}

fn side_view(
    sim: &Sim,
    tool: Tool,
    notice: &str,
    alerts: &Alerts,
    completions: &CompletionLog,
) -> SideView {
    let mut alert_lines = alerts.lines();
    alert_lines.extend(completions.lines());
    SideView {
        nation: sim.defs.nation(sim.world.nation).name.clone(),
        seed: sim.world.seed,
        people: sim.world.units.len(),
        clock: format_clock(sim.world.tick),
        paused: sim.paused,
        tool,
        tool_hint: tool_hint(sim, tool),
        notice: notice.to_owned(),
        modifiers: modifier_rows(sim.world.modifiers()),
        research: research::research_status(sim.world.research(), &sim.defs),
        alerts: alert_lines,
    }
}

/// `Tool::label` for most tools; `Tool::Bend` names the nation's one ability instead, since
/// "channel the nation's ability" alone doesn't say which (docs/design/bending.md).
fn tool_hint(sim: &Sim, tool: Tool) -> String {
    if tool == Tool::Bend
        && let Some(id) = ability_for_nation(&sim.defs, sim.world.nation)
    {
        let def = sim.defs.ability(id);
        return format!("{}: {}", def.verb, def.description);
    }
    tool.label().to_owned()
}

/// In-game time since the start as `MM:SS`. There is no calendar yet.
fn format_clock(tick: u64) -> String {
    let secs = tick / u64::from(TICKS_PER_SECOND);
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// The modifiers that aren't 1.0, as (label, "x1.6"). A modifier at its baseline says nothing.
fn modifier_rows(m: &Modifiers) -> Vec<(String, String)> {
    Modifiers::NAMES
        .iter()
        .filter_map(|name| Some((*name, m.get(name)?)))
        .filter(|(_, v)| (v - 1.0).abs() > 0.005)
        .map(|(name, v)| (humanize(name), multiplier(v)))
        .collect()
}

/// 1.6 becomes "x1.6" and 2.0 becomes "x2".
pub(crate) fn multiplier(v: f32) -> String {
    let s = format!("{v:.2}");
    format!("x{}", s.trim_end_matches('0').trim_end_matches('.'))
}

/// "move_speed" becomes "Move speed".
pub(crate) fn humanize(name: &str) -> String {
    let mut s = name.replace('_', " ");
    if let Some(first) = s.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    s
}

fn update_side_panel(
    mut commands: Commands,
    sim: Res<Sim>,
    tool: Res<Tool>,
    notice: Res<Notice>,
    alerts: Res<Alerts>,
    completions: Res<CompletionLog>,
    theme: Res<Theme>,
    content: Query<Entity, With<SideContent>>,
    fresh: Query<(), Added<SideContent>>,
    mut last: Local<Option<SideView>>,
) {
    let Ok(content) = content.single() else { return };
    let view = side_view(&sim, *tool, &notice.0, &alerts, &completions);
    if last.as_ref() == Some(&view) && fresh.is_empty() {
        return;
    }
    commands.entity(content).despawn_children().with_children(|p| spawn_side(p, &theme, &view));
    *last = Some(view);
}

fn spawn_side(p: &mut ChildSpawnerCommands, theme: &Theme, v: &SideView) {
    // Header: emblem, nation, seed and people.
    p.spawn(Node { column_gap: Val::Px(space::L), align_items: AlignItems::Center, ..default() })
        .with_children(|h| {
            h.spawn((
                Node {
                    width: Val::Px(36.0),
                    height: Val::Px(36.0),
                    border: UiRect::all(Val::Px(3.0)),
                    border_radius: BorderRadius::MAX,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(theme.inset),
                BorderColor::all(theme.accent),
            ))
            .with_children(|e| {
                e.spawn((
                    Node {
                        width: Val::Px(12.0),
                        height: Val::Px(12.0),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(theme.accent),
                ));
            });
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
        r.spawn(ui_style::text(format!("{state}  x{SPEED}"), font::BODY, color));
    });

    // The tool the left mouse button has, and the last designation's result.
    ui_style::section_title(p, theme, "Tool");
    p.spawn(Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(space::S),
        row_gap: Val::Px(space::S),
        ..default()
    })
    .with_children(|row| {
        for (tool, label, key) in TOOLS {
            ui_style::button(row, theme, label, key, tool == v.tool);
        }
    });
    p.spawn(ui_style::text(v.tool_hint.clone(), font::BODY, theme.dim));
    if !v.notice.is_empty() {
        p.spawn(ui_style::text(v.notice.clone(), font::BODY, theme.accent_text));
    }

    // Secondary: the nation's modifiers, only those that differ from the baseline.
    if !v.modifiers.is_empty() {
        ui_style::section_title(p, theme, "Nation");
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
    ui_style::section_title(p, theme, "Research");
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
        ui_style::section_title(p, theme, "Alerts");
        for line in &v.alerts {
            p.spawn(ui_style::text(line.clone(), font::BODY, theme.accent_text));
        }
    }

    ui_style::section_title(p, theme, "Controls");
    for line in CONTROLS {
        p.spawn(ui_style::text(line, font::LABEL, theme.dim));
    }
}

// ---------------------------------------------------------------------------------------------
// Selection bar

/// How a meter reads: the same colours for every nation (`Theme::ok`, `warn`, `bad`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status {
    Ok,
    Warn,
    Bad,
}

impl Status {
    fn color(self, theme: &Theme) -> Color {
        match self {
            Self::Ok => theme.ok,
            Self::Warn => theme.warn,
            Self::Bad => theme.bad,
        }
    }
}

/// A meter at `frac` of full is `Ok` down to `low` (the level where a unit goes to fix it),
/// `Warn` down to half of that, and `Bad` below.
fn need_status(frac: f32, low: f32) -> Status {
    if frac >= low {
        Status::Ok
    } else if frac >= low / 2.0 {
        Status::Warn
    } else {
        Status::Bad
    }
}

/// Health has no `low` in the data; it reads as a warning at half.
const HEALTH_LOW: f32 = 0.5;

#[derive(Clone, PartialEq, Debug)]
struct NeedView {
    label: &'static str,
    /// In `[0, 1]`.
    frac: f32,
    status: Status,
}

impl NeedView {
    fn percent(&self) -> String {
        format!("{:.0}%", self.frac * 100.0)
    }
}

/// Everything the selection bar shows for a unit.
#[derive(Clone, PartialEq, Debug)]
struct UnitView {
    id: UnitId,
    name: String,
    nation: String,
    tile: Pos,
    task: String,
    carrying: String,
    /// Art id of the character, for the portrait.
    character: String,
    needs: Vec<NeedView>,
    /// Chi, as a fraction of `data/bending.toml`'s `chi.max`. Not a `NeedView`: chi never hurts
    /// the unit, so it has no "low" threshold or warning colour (docs/design/bending.md).
    chi_frac: f32,
}

fn unit_view(sim: &Sim, unit: &Unit) -> UnitView {
    let s = &sim.defs.needs;
    let meter = |label, value: f32, max: f32, low: f32| {
        let frac = (value / max).clamp(0.0, 1.0);
        NeedView { label, frac, status: need_status(frac, low) }
    };
    let low = |n: fn_sim::needs::NeedDef| n.low / s.max();
    UnitView {
        id: unit.id,
        name: unit.name.clone(),
        nation: sim.defs.nation(unit.nation).name.clone(),
        tile: unit.pos,
        task: jobview::describe_task(&sim.world, &sim.defs, unit),
        carrying: jobview::describe_stack(&sim.defs, unit.carrying),
        character: unit_character(sim, unit).to_owned(),
        needs: vec![
            meter("Health", unit.needs.health, s.health.max, HEALTH_LOW),
            meter("Hunger", unit.needs.hunger, s.max(), low(s.hunger)),
            meter("Thirst", unit.needs.thirst, s.max(), low(s.thirst)),
            meter("Energy", unit.needs.energy, s.max(), low(s.energy)),
        ],
        chi_frac: (unit.chi / sim.defs.chi.max).clamp(0.0, 1.0),
    }
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
    mut last: Local<Option<UnitView>>,
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
    let view = unit_view(&sim, unit);
    if last.as_ref() == Some(&view) && fresh.is_empty() {
        return;
    }
    let face = art.character(&view.character).face.clone();
    commands
        .entity(content)
        .despawn_children()
        .with_children(|p| spawn_selection(p, &theme, &view, face));
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
                    need.status.color(theme),
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

/// What a tooltip line is for, and so its colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Line {
    Title,
    Detail,
    Hint,
}

type TooltipText = Vec<(Line, String)>;

/// "Lit: 12m 03s left" for a flame on `tile`, or "Wet: sprouts in 3m 20s" for a watered one.
/// `None` for anything else.
fn bending_status_line(sim: &Sim, tile: Pos) -> Option<String> {
    let clock = |secs: f32| {
        let secs = secs.max(0.0) as u64;
        format!("{}m {:02}s", secs / 60, secs % 60)
    };
    if let Some(flame) = sim.world.flames().into_iter().find(|f| f.pos == tile) {
        return Some(format!("Lit: {} left", clock(flame.seconds_left)));
    }
    let (_, left) = sim.world.wet_tiles().into_iter().find(|(p, _)| *p == tile)?;
    Some(format!("Wet: sprouts in {}", clock(left)))
}

/// The tooltip for what is under the cursor at `tile`: up to 4 lines (name, context, a yield or
/// reason, then an action hint). `None` when `tile` is off the map.
fn tooltip_lines(sim: &Sim, tool: Tool, tile: Pos) -> Option<TooltipText> {
    let t = sim.world.map.get(tile)?;
    let mut lines = TooltipText::new();
    if let Some(unit) = sim.world.units.iter().find(|u| u.pos == tile) {
        lines.push((Line::Title, unit.name.clone()));
        let task = jobview::describe_task(&sim.world, &sim.defs, unit);
        lines.push((Line::Detail, format!("{} - {task}", sim.defs.nation(unit.nation).name)));
        lines.push((
            Line::Detail,
            format!("Carrying: {}", jobview::describe_stack(&sim.defs, unit.carrying)),
        ));
        if tool == Tool::Select {
            lines.push((Line::Hint, "Click to select".into()));
        }
        return Some(lines);
    }
    let biome = &sim.defs.biome(t.biome).name;
    let at = format!("{biome} - {:?} ({}, {})", t.terrain, tile.x, tile.y);
    let Some(res) = t.resource else {
        lines.push((Line::Title, format!("{:?}", t.terrain)));
        lines.push((Line::Detail, format!("{biome} ({}, {})", tile.x, tile.y)));
        lines.push((Line::Detail, format!("Elevation {}", t.elevation)));
        // A lit flame or a wet patch (#72): the result of a bend, so hovering explains it
        // (pillar 4, "readable simulation").
        if let Some(status) = bending_status_line(sim, tile) {
            lines.push((Line::Detail, status));
        }
        return Some(lines);
    };
    lines.push((Line::Title, sim.defs.resource(res).name.clone()));
    lines.push((Line::Detail, at));
    if let Some(h) = sim.defs.harvest(res) {
        if sim.world.is_resting(tile) {
            lines.push((Line::Detail, "Growing back".into()));
        } else {
            let item = &sim.defs.item(h.item).name;
            lines.push((Line::Detail, format!("{}: {} {item} - {:.0}s", h.verb, h.count, h.work)));
            let hint = if tool == Tool::Harvest {
                "Drag to designate"
            } else {
                "[H] then drag to harvest"
            };
            lines.push((Line::Hint, hint.into()));
        }
    }
    Some(lines)
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
    tool: Res<Tool>,
    theme: Res<Theme>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    bar: Query<&Visibility, (With<SelectionBar>, Without<Tooltip>)>,
    mut tooltip: Query<(&mut Node, &mut Visibility), With<Tooltip>>,
    lines: Query<Entity, With<TooltipLines>>,
    fresh: Query<(), Added<TooltipLines>>,
    mut last: Local<TooltipText>,
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
        let text = tooltip_lines(&sim, *tool, cursor_tile(window, camera, camera_tf)?)?;
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
        for (kind, line) in &text {
            let color = match kind {
                Line::Title => theme.text,
                Line::Detail => theme.dim,
                Line::Hint => theme.accent_text,
            };
            p.spawn(ui_style::text(line.clone(), font::BODY, color));
        }
    });
    *last = text;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn need_status_thresholds() {
        // Hunger's `low` is 40% of full.
        assert_eq!(need_status(1.0, 0.4), Status::Ok);
        assert_eq!(need_status(0.4, 0.4), Status::Ok);
        assert_eq!(need_status(0.39, 0.4), Status::Warn);
        assert_eq!(need_status(0.2, 0.4), Status::Warn);
        assert_eq!(need_status(0.19, 0.4), Status::Bad);
        assert_eq!(need_status(0.0, 0.4), Status::Bad);
    }

    #[test]
    fn status_colours_come_from_the_theme() {
        let t = Theme::for_nation([70, 130, 210]);
        assert_eq!(Status::Ok.color(&t), t.ok);
        assert_eq!(Status::Warn.color(&t), t.warn);
        assert_eq!(Status::Bad.color(&t), t.bad);
    }

    #[test]
    fn clock_counts_minutes_and_seconds() {
        let tps = u64::from(TICKS_PER_SECOND);
        assert_eq!(format_clock(0), "00:00");
        assert_eq!(format_clock(59 * tps + tps - 1), "00:59");
        assert_eq!(format_clock(60 * tps), "01:00");
        assert_eq!(format_clock((100 * 60 + 5) * tps), "100:05");
    }

    #[test]
    fn modifier_rows_skip_baseline_and_format_values() {
        let defs = fn_sim::Defs::bundled();
        let m = defs.nation(Nation::Air).modifiers;
        let rows = modifier_rows(&m);
        assert!(rows.contains(&("Move speed".to_owned(), "x1.6".to_owned())), "{rows:?}");
        assert!(rows.contains(&("Power generation".to_owned(), "x2".to_owned())), "{rows:?}");
        assert!(rows.iter().all(|(label, _)| label != "Heating"), "baseline shown: {rows:?}");
    }

    #[test]
    fn humanize_capitalises_and_spaces() {
        assert_eq!(humanize("water_work"), "Water work");
        assert_eq!(humanize("study"), "Study");
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

    #[test]
    fn tooltips_have_at_most_four_lines() {
        let sim = Sim::new(7, Nation::Earth, 64, 64);
        let mut seen_resource = false;
        let mut seen_unit = false;
        for (p, t) in sim.world.map.iter() {
            for tool in [Tool::Select, Tool::Harvest] {
                let lines = tooltip_lines(&sim, tool, p).expect("on the map");
                assert!((2..=4).contains(&lines.len()), "{p:?}: {lines:?}");
                assert_eq!(lines[0].0, Line::Title);
            }
            seen_resource |= t.resource.is_some();
        }
        for u in &sim.world.units {
            seen_unit = true;
            let lines = tooltip_lines(&sim, Tool::Select, u.pos).unwrap();
            assert_eq!(lines[0].1, u.name);
            assert_eq!(lines.last().unwrap().0, Line::Hint);
        }
        assert!(seen_resource && seen_unit);
        assert_eq!(tooltip_lines(&sim, Tool::Select, Pos::new(-1, -1)), None);
    }

    #[test]
    fn unit_view_has_every_meter_in_range() {
        let sim = Sim::new(7, Nation::Water, 64, 64);
        let v = unit_view(&sim, &sim.world.units[0]);
        let labels: Vec<_> = v.needs.iter().map(|n| n.label).collect();
        assert_eq!(labels, ["Health", "Hunger", "Thirst", "Energy"]);
        // New units start full.
        assert!(v.needs.iter().all(|n| n.frac == 1.0 && n.status == Status::Ok), "{v:?}");
        assert_eq!(v.needs[0].percent(), "100%");
        assert_eq!(v.chi_frac, 1.0, "a fresh unit's chi starts full too");
    }

    #[test]
    fn tool_hint_names_the_nation_s_own_ability() {
        let sim = Sim::new(1, Nation::Fire, 48, 32);
        let hint = tool_hint(&sim, Tool::Bend);
        let id = ability_for_nation(&sim.defs, Nation::Fire).unwrap();
        assert!(hint.starts_with(&sim.defs.ability(id).verb), "{hint}");
        assert_eq!(tool_hint(&sim, Tool::Harvest), Tool::Harvest.label());
    }

    #[test]
    fn bending_status_line_names_a_lit_flame() {
        use fn_sim::world::Command;

        let mut sim = Sim::new(1, Nation::Fire, 48, 32);
        let pos = sim.world.units[0].pos;
        let ability = ability_for_nation(&sim.defs, Nation::Fire).unwrap();
        let Sim { world, defs, .. } = &mut sim;
        world.apply(Command::Bend { ability, pos }, defs).expect("a unit's own tile validates");
        for _ in 0..20_000 {
            world.tick(defs);
            if world.is_lit(pos) {
                break;
            }
        }
        assert!(world.is_lit(pos), "the flame should finish lighting");
        let line = bending_status_line(&sim, pos).expect("a lit flame");
        assert!(line.starts_with("Lit:"), "{line}");
        assert_eq!(bending_status_line(&sim, Pos::new(-1, -1)), None);
    }
}

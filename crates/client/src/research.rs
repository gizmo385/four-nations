//! Research (#23): the HUD's progress line and the full tech-tree screen. Reads `World::research`
//! and shows it; the only rule this module applies on its own is picking which techs a nation can
//! see (shared plus its own branch). Choosing a tech issues `Command::SetResearch`; `fn_sim` does
//! the rest (`docs/design/research.md`).
//!
//! `T` toggles the screen; `Esc` closes it. It is centred over a dimmed map (ui.md "Screens"): the
//! side panel stays visible, techs are laid out in columns by depth (tier), and clicking a tech
//! card sets it as the research target. A tech already known is a no-op if clicked; one with
//! unmet prereqs queues its whole prereq chain (`Research::set_target`), which is why a queued
//! prereq is drawn the same as the target itself.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use fn_sim::research::{MILLIS_PER_POINT, Research, TechDef, TechId};
use fn_sim::world::Command;
use fn_sim::{Defs, Nation};

use crate::Sim;
use crate::hud::{humanize, multiplier};
use crate::ui_style::{self, Frame, Theme, font, layout, space};

pub struct ResearchPlugin;

impl Plugin for ResearchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenOpen>()
            .init_resource::<CompletionLog>()
            // `PreUpdate` so the side panel's `Update` systems see this tick's new completions
            // with no one-frame lag (same reasoning as `bending::track_bend_events`).
            .add_systems(PreUpdate, track_completions)
            .add_systems(
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
// HUD progress (side panel)

/// The side panel's Research section: the tech the pool is currently paying for, its progress,
/// and the knowledge rate. There is no Study job yet (#4), so the rate is the passive trickle
/// only; it will pick up the study modifier once scholars exist.
#[derive(Clone, PartialEq, Debug)]
pub struct ResearchStatus {
    /// The tech currently being paid for (the queue's front), or a "no target" message.
    pub label: String,
    /// Progress toward `label`, in `[0, 1]`. Zero when nothing is queued.
    pub frac: f32,
    /// "18/25 pts" while queued, "42 pts banked" otherwise.
    pub pool: String,
    /// "3.0/min".
    pub rate: String,
    /// Further techs queued behind `label` (the rest of the chosen tech's prereq chain).
    pub more_queued: usize,
}

/// Knowledge millis as a fractional point count.
fn points(millis: u64) -> f64 {
    millis as f64 / MILLIS_PER_POINT as f64
}

pub fn research_status(research: &Research, defs: &Defs) -> ResearchStatus {
    let rate = format!("{:.1}/min", defs.research.passive_knowledge_per_minute);
    let pool = points(research.pool_millis());
    let Some(&front) = research.queue().first() else {
        return ResearchStatus {
            label: "No target set".to_owned(),
            frac: 0.0,
            pool: format!("{pool:.0} pts banked"),
            rate,
            more_queued: 0,
        };
    };
    let cost = f64::from(defs.tech(front).cost);
    ResearchStatus {
        label: defs.tech(front).name.clone(),
        frac: (pool / cost).clamp(0.0, 1.0) as f32,
        pool: format!("{pool:.0}/{cost:.0} pts"),
        rate,
        more_queued: research.queue().len() - 1,
    }
}

/// How long a completion stays as a toast in the side panel's Alerts section, in real seconds.
const LOG_SECONDS: f32 = 8.0;
/// At most this many completion toasts show at once; the oldest drops first.
const MAX_LOG: usize = 3;

/// Recent tech completions as short-lived toast lines (same pattern as `bending::Alerts`), shown
/// in the side panel's Alerts section alongside bend events.
#[derive(Resource, Default)]
pub struct CompletionLog {
    /// (line, seconds left), oldest first.
    entries: Vec<(String, f32)>,
    /// How many of `Research::completions` have already become a toast.
    seen: usize,
}

impl CompletionLog {
    /// The toasts still showing, oldest first.
    pub fn lines(&self) -> Vec<String> {
        self.entries.iter().map(|(line, _)| line.clone()).collect()
    }
}

fn age(entries: &mut Vec<(String, f32)>, dt: f32) {
    entries.retain_mut(|(_, left)| {
        *left -= dt;
        *left > 0.0
    });
    let len = entries.len();
    if len > MAX_LOG {
        entries.drain(0..len - MAX_LOG);
    }
}

fn track_completions(mut log: ResMut<CompletionLog>, sim: Res<Sim>, time: Res<Time>) {
    let completions = sim.world.research().completions();
    if log.seen > completions.len() {
        // A new world (regenerate, nation switch): the old log is gone.
        log.seen = 0;
        log.entries.clear();
    }
    for c in &completions[log.seen..] {
        log.entries.push((format!("Learned {}", sim.defs.tech(c.tech).name), LOG_SECONDS));
    }
    log.seen = completions.len();
    age(&mut log.entries, time.delta_secs());
}

// ---------------------------------------------------------------------------------------------
// The tech-tree screen

/// How a tech reads in the screen: never both known and queued, so one colour tells you which.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TechState {
    Known,
    /// In the current queue: either the chosen target or one of its unknown prereqs.
    Queued,
    Available,
    /// At least one prereq isn't known yet. Still clickable: `Research::set_target` queues the
    /// whole chain.
    Locked,
}

impl TechState {
    fn label(self) -> &'static str {
        match self {
            Self::Known => "Known",
            Self::Queued => "Queued",
            Self::Available => "Available",
            Self::Locked => "Needs prereqs",
        }
    }
}

fn tech_state(research: &Research, id: TechId, def: &TechDef) -> TechState {
    if research.is_known(id) {
        TechState::Known
    } else if research.queue().contains(&id) {
        TechState::Queued
    } else if def.prereqs.iter().all(|p| research.is_known(*p)) {
        TechState::Available
    } else {
        TechState::Locked
    }
}

/// Each tech's depth: 0 for one with no prereqs, otherwise one past its deepest prereq. Prereqs
/// always have a smaller `TechId` (`research.rs`'s file-order invariant), so a single ascending
/// pass is enough.
fn tech_depths(techs: &[TechDef]) -> Vec<usize> {
    let mut depths = vec![0usize; techs.len()];
    for (i, t) in techs.iter().enumerate() {
        depths[i] = t.prereqs.iter().map(|p| depths[p.0 as usize] + 1).max().unwrap_or(0);
    }
    depths
}

/// "Study x1.25, Air Scooter" (a modifier and an ability this tech unlocks), or "-" for neither.
/// Buildings and jobs aren't tech unlocks yet (out of scope; `docs/design/research.md` "Unlocks").
fn unlock_summary(defs: &Defs, id: TechId, def: &TechDef) -> String {
    let mut parts: Vec<String> = def
        .unlocks
        .modifiers
        .iter()
        .map(|(n, m)| format!("{} {}", humanize(n), multiplier(m)))
        .collect();
    parts.extend(
        defs.abilities.iter().filter(|a| a.unlocked_by == Some(id)).map(|a| a.name.clone()),
    );
    if parts.is_empty() { "-".to_owned() } else { parts.join(", ") }
}

#[derive(Clone, PartialEq, Debug)]
struct TechCard {
    id: TechId,
    name: String,
    cost: u32,
    state: TechState,
    unlock: String,
}

/// Columns by depth, each holding the techs at that depth: shared techs plus `nation`'s own
/// branch, in `TechId` order within a column. Other nations' branches are never built.
#[derive(Clone, PartialEq, Debug, Default)]
struct ScreenView {
    columns: Vec<Vec<TechCard>>,
}

fn screen_view(sim: &Sim) -> ScreenView {
    let defs = &sim.defs;
    let nation = sim.world.nation;
    let research = sim.world.research();
    let depths = tech_depths(&defs.techs);
    let mut columns: Vec<Vec<TechCard>> = Vec::new();
    for (i, def) in defs.techs.iter().enumerate() {
        if def.nation.is_some_and(|n| n != nation) {
            continue;
        }
        let id = TechId(i as u16);
        let depth = depths[i];
        if columns.len() <= depth {
            columns.resize(depth + 1, Vec::new());
        }
        columns[depth].push(TechCard {
            id,
            name: def.name.clone(),
            cost: def.cost,
            state: tech_state(research, id, def),
            unlock: unlock_summary(defs, id, def),
        });
    }
    ScreenView { columns }
}

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

fn spawn_screen(commands: &mut Commands, theme: &Theme) {
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
                overflow: Overflow::clip(),
                ..default()
            };
            root.spawn(ui_style::panel(theme, panel)).with_children(|p| {
                ui_style::frame(p, theme, Frame::Panel);
                p.spawn(ui_style::text("Research", font::TITLE, theme.text));
                p.spawn((
                    ScreenGrid,
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(COLUMN_GAP),
                        align_items: AlignItems::FlexStart,
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

fn spawn_columns(p: &mut ChildSpawnerCommands, theme: &Theme, view: &ScreenView) {
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
            ui_style::section_title(col, theme, &format!("Tier {depth}"));
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
    spawn_screen(&mut commands, &theme);
}

fn update_screen_content(
    mut commands: Commands,
    open: Res<ScreenOpen>,
    sim: Res<Sim>,
    theme: Res<Theme>,
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
    commands.entity(grid).despawn_children().with_children(|p| spawn_columns(p, &theme, &view));
    *last = Some(view);
}

/// Whether `cursor` (logical pixels, window-relative) falls inside a node centred at `center`
/// with size `size`, both already converted to logical pixels.
fn contains(cursor: Vec2, center: Vec2, size: Vec2) -> bool {
    (cursor - center).abs().cmple(size / 2.0).all()
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
    let Some(cursor) = window.cursor_position() else { return };
    let hit = buttons.iter().find_map(|(TechButton(id), node, transform)| {
        let scale = node.inverse_scale_factor;
        let size = node.size() * scale;
        let center = transform.affine().translation * scale;
        contains(cursor, center, size).then_some(*id)
    });
    if let Some(tech) = hit {
        let Sim { world, defs, .. } = &mut *sim;
        let _ = world.apply(Command::SetResearch { tech }, defs);
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use fn_sim::research::Unlocks;
    use fn_sim::world::Command;

    use super::*;
    use crate::Sim;

    fn def(nation: Option<Nation>, cost: u32, prereqs: &[TechId]) -> TechDef {
        TechDef {
            id: "x".into(),
            name: "X".into(),
            description: String::new(),
            nation,
            cost,
            prereqs: prereqs.to_vec(),
            unlocks: Unlocks::default(),
        }
    }

    #[test]
    fn depths_follow_the_deepest_prereq() {
        let techs = vec![
            def(None, 1, &[]),                     // 0: depth 0
            def(None, 1, &[]),                     // 1: depth 0
            def(None, 1, &[TechId(0)]),            // 2: depth 1
            def(None, 1, &[TechId(1), TechId(2)]), // 3: depth 2
        ];
        assert_eq!(tech_depths(&techs), [0, 0, 1, 2]);
    }

    #[test]
    fn depths_are_empty_for_no_techs() {
        assert_eq!(tech_depths(&[]), Vec::<usize>::new());
    }

    #[test]
    fn tech_state_reflects_known_queued_and_locked() {
        let sim = Sim::new(7, Nation::Fire, 48, 32);
        let research = sim.world.research();
        let defs = &sim.defs;
        let fm = defs.tech_id("fire_making").unwrap();
        assert_eq!(tech_state(research, fm, defs.tech(fm)), TechState::Known);
        let shelter = defs.tech_id("shelter").unwrap();
        assert_eq!(tech_state(research, shelter, defs.tech(shelter)), TechState::Available);
        let masonry = defs.tech_id("masonry").unwrap();
        assert_eq!(tech_state(research, masonry, defs.tech(masonry)), TechState::Locked);
    }

    #[test]
    fn set_research_marks_target_and_its_prereqs_as_queued() {
        let mut sim = Sim::new(7, Nation::Fire, 48, 32);
        let masonry = sim.defs.tech_id("masonry").unwrap();
        let Sim { world, defs, .. } = &mut sim;
        world.apply(Command::SetResearch { tech: masonry }, defs).unwrap();
        let research = world.research();
        assert_eq!(tech_state(research, masonry, defs.tech(masonry)), TechState::Queued);
        let carpentry = defs.tech_id("carpentry").unwrap();
        assert_eq!(tech_state(research, carpentry, defs.tech(carpentry)), TechState::Queued);
    }

    #[test]
    fn unlock_summary_names_modifiers_and_is_a_dash_otherwise() {
        let defs = fn_sim::Defs::bundled();
        let writing = defs.tech_id("writing").unwrap();
        let summary = unlock_summary(&defs, writing, defs.tech(writing));
        assert_eq!(summary, "Study x1.25");
        let fire_making = defs.tech_id("fire_making").unwrap();
        assert_eq!(unlock_summary(&defs, fire_making, defs.tech(fire_making)), "-");
    }

    #[test]
    fn screen_view_hides_other_nations_branches() {
        let sim = Sim::new(7, Nation::Fire, 48, 32);
        let view = screen_view(&sim);
        let names: Vec<_> = view.columns.iter().flatten().map(|c| c.name.clone()).collect();
        // A Water-only tech never shows for a Fire player.
        let healing = sim.defs.tech(sim.defs.tech_id("healing").unwrap());
        assert!(!names.contains(&healing.name), "{names:?}");
        // The shared tier and Fire's own branch both show.
        assert!(
            names.contains(&"Fire Making".to_owned()) || names.iter().any(|n| n == "Fire Making")
        );
    }

    #[test]
    fn research_status_without_a_target_banks_knowledge() {
        let mut sim = Sim::new(7, Nation::Fire, 48, 32);
        for _ in 0..1000 {
            sim.world.tick(&sim.defs.clone());
        }
        let status = research_status(sim.world.research(), &sim.defs);
        assert_eq!(status.label, "No target set");
        assert_eq!(status.frac, 0.0);
        assert!(status.pool.ends_with("pts banked"), "{}", status.pool);
    }

    #[test]
    fn research_status_with_a_target_shows_progress_toward_the_front() {
        let mut sim = Sim::new(7, Nation::Fire, 48, 32);
        let shelter = sim.defs.tech_id("shelter").unwrap();
        let Sim { world, defs, .. } = &mut sim;
        world.apply(Command::SetResearch { tech: shelter }, defs).unwrap();
        let status = research_status(world.research(), defs);
        assert_eq!(status.label, "Shelter");
        assert_eq!(status.more_queued, 0);
        assert!(status.frac >= 0.0 && status.frac <= 1.0);
    }

    #[test]
    fn cursor_inside_and_outside_a_card() {
        let center = Vec2::new(100.0, 50.0);
        let size = Vec2::new(40.0, 20.0);
        assert!(contains(Vec2::new(100.0, 50.0), center, size));
        assert!(contains(Vec2::new(119.0, 59.0), center, size));
        assert!(!contains(Vec2::new(121.0, 50.0), center, size));
        assert!(!contains(Vec2::new(100.0, 61.0), center, size));
    }

    #[test]
    fn completion_log_turns_new_completions_into_toasts() {
        let mut sim = Sim::new(7, Nation::Fire, 48, 32);
        let shelter = sim.defs.tech_id("shelter").unwrap();
        let Sim { world, defs, .. } = &mut sim;
        world.apply(Command::SetResearch { tech: shelter }, defs).unwrap();
        for _ in 0..20_000 {
            world.tick(defs);
            if !world.research().completions().is_empty() {
                break;
            }
        }
        let completions = world.research().completions();
        assert!(!completions.is_empty(), "shelter should finish on passive knowledge alone");
        let mut log = CompletionLog::default();
        for c in completions {
            log.entries.push((format!("Learned {}", defs.tech(c.tech).name), LOG_SECONDS));
        }
        assert_eq!(log.lines(), ["Learned Shelter"]);
    }
}

//! Saving and loading (#112, #216): `F5` quicksaves, `F9` quickloads, and `F6` opens the save
//! list, where any number of named slots can be saved to and loaded. The sim turns a `World` into
//! text and back (`fn_sim::save`) and does no I/O; the files, the directory and the slot list are
//! `fn_ui::saves`, shared with the terminal UI. This module is the keys, the screen and the toasts.
//!
//! - **The save list** (`F6`, `Esc` closes): every slot in the save directory, newest first, with
//!   its nation, seed and play time read from the file's header only (`fn_ui::saves::list`). Each
//!   row has a Save button (write the running game over that slot) and, if the slot can be loaded,
//!   a Load button; a damaged file, one from another save format or one made with other game data
//!   says why instead. "New save" writes a new slot named `save-1`, `save-2`, ... Map input is
//!   suspended while the list is open, as for the research screen, and the two screens never show
//!   at once.
//! - **Errors are toasts**: a failed save or load puts one line in the side panel's notice (the
//!   one designations use), for a few seconds. A load that fails changes nothing: the file is read
//!   and checked into a new `World` first, and only a good one replaces the running game.
//! - **What is not in a save**: only the `World` is. Client state resets on a load: the selection
//!   is cleared, the tool goes back to Select, the alert log starts over (the loaded game's old
//!   events don't replay as toasts) and the casts in flight are dropped. The camera stays where it
//!   is (it is clamped to the loaded map), and so does the clock (speed and pause), which is a
//!   preference of the player's rather than a part of the game.
//! - **Sprites** are respawned as for a new map, so each unit sprite starts on its place and
//!   `unit_motion` has nothing to glide from: units don't slide across the map from where they
//!   were.

use std::path::PathBuf;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use fn_sim::World;
use fn_ui::saves::{self, QUICKSAVE, Slot};
use fn_ui::{Session, Tool};

use crate::bending::AlertLog;
use crate::cast::Casts;
use crate::designate::{ActiveTool, Notice};
use crate::research::ScreenOpen;
use crate::ui_style::{self, Frame, Skin, Theme, font, layout, space};
use crate::{Art, MapEntity, Selection, Sim, spawn_entities};

/// The quicksave and quickload keys, and the key that opens the save list.
pub const SAVE_KEY: KeyCode = KeyCode::F5;
pub const LOAD_KEY: KeyCode = KeyCode::F9;
pub const LIST_KEY: KeyCode = KeyCode::F6;

/// How long a save or load result shows in the side panel, in seconds.
const TOAST_SECONDS: f32 = 5.0;

const PANEL_WIDTH: f32 = 560.0;

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveFiles>()
            .init_resource::<SaveToast>()
            .init_resource::<SavesOpen>()
            .init_resource::<SaveList>()
            .add_message::<SaveAction>()
            .add_systems(
                Update,
                (
                    // Before `controls`, so a load and a regenerate pressed on one frame don't
                    // both respawn the map.
                    (toggle_list, quick_keys, click_list, run_actions)
                        .chain()
                        .before(crate::controls),
                    sync_list_screen,
                    expire_toast,
                )
                    .chain(),
            );
    }
}

/// Where saves go: `None` when the platform has no data directory to put them in.
#[derive(Resource)]
pub struct SaveFiles(pub Option<PathBuf>);

impl Default for SaveFiles {
    fn default() -> Self {
        SaveFiles(saves::default_save_dir())
    }
}

/// Whether the save list is open. Map input is suspended while it is (`crate::screen_closed`).
#[derive(Resource, Default)]
pub struct SavesOpen(pub bool);

/// The slots the list shows, read when it opens and after every save. `Err` is why the directory
/// couldn't be listed.
#[derive(Resource, PartialEq)]
struct SaveList(Result<Vec<Slot>, String>);

impl Default for SaveList {
    fn default() -> Self {
        SaveList(Ok(Vec::new()))
    }
}

/// The notice a save or load wrote, and how long it has left.
#[derive(Resource, Default)]
struct SaveToast {
    text: String,
    left: f32,
}

/// Something the keys or the save list asked for.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum SaveAction {
    /// Write the running game to this slot, replacing what is there.
    Save(String),
    /// Make this slot the running game.
    Load(String),
    /// Write the running game to a slot with a new name.
    New,
}

/// `F6` toggles the list; `Esc` only closes it. Opening it closes the research screen, and opening
/// the research screen closes it. A list open from the start (`FN_SAVES_OPEN`) is read at once.
fn toggle_list(
    keys: Res<ButtonInput<KeyCode>>,
    files: Res<SaveFiles>,
    sim: Res<Sim>,
    mut open: ResMut<SavesOpen>,
    mut research: ResMut<ScreenOpen>,
    mut list: ResMut<SaveList>,
    mut started: Local<bool>,
) {
    if !std::mem::replace(&mut *started, true) && open.0 {
        list.0 = read_list(&files, &sim);
    }
    if keys.just_pressed(LIST_KEY) {
        open.0 = !open.0;
        if open.0 {
            research.0 = false;
            list.0 = read_list(&files, &sim);
        }
    } else if open.0 && (keys.just_pressed(KeyCode::Escape) || research.0) {
        open.0 = false;
    }
}

fn read_list(files: &SaveFiles, sim: &Session) -> Result<Vec<Slot>, String> {
    match files.0.as_deref() {
        None => Err(NO_SAVE_DIR.to_owned()),
        Some(dir) => saves::list(dir, &sim.defs),
    }
}

const NO_SAVE_DIR: &str = "this platform has no data directory (set FN_SAVE_DIR)";

/// `F5` saves to the quicksave slot and `F9` loads it.
fn quick_keys(keys: Res<ButtonInput<KeyCode>>, mut actions: MessageWriter<SaveAction>) {
    if keys.just_pressed(SAVE_KEY) {
        actions.write(SaveAction::Save(QUICKSAVE.to_owned()));
    }
    if keys.just_pressed(LOAD_KEY) {
        actions.write(SaveAction::Load(QUICKSAVE.to_owned()));
    }
}

#[derive(SystemParam)]
struct Reset<'w, 's> {
    selection: ResMut<'w, Selection>,
    tool: ResMut<'w, ActiveTool>,
    alerts: ResMut<'w, AlertLog>,
    casts: ResMut<'w, Casts>,
    existing: Query<'w, 's, Entity, With<MapEntity>>,
}

/// Does what was asked, says how it went in the side panel, and keeps the list up to date. A
/// load closes the list.
#[allow(clippy::too_many_arguments)]
fn run_actions(
    mut commands: Commands,
    mut actions: MessageReader<SaveAction>,
    files: Res<SaveFiles>,
    art: Res<Art>,
    mut sim: ResMut<Sim>,
    mut notice: ResMut<Notice>,
    mut toast: ResMut<SaveToast>,
    mut open: ResMut<SavesOpen>,
    mut list: ResMut<SaveList>,
    mut reset: Reset,
) {
    for action in actions.read() {
        let quick = matches!(action, SaveAction::Save(n) | SaveAction::Load(n) if n == QUICKSAVE);
        let text = match (files.0.as_deref(), action) {
            (None, _) => format!("Save failed: {NO_SAVE_DIR}"),
            (Some(dir), SaveAction::Save(_) | SaveAction::New) => {
                let name = match action {
                    SaveAction::Save(name) => name.clone(),
                    _ => saves::new_name(list.0.as_deref().unwrap_or_default()),
                };
                let tick = sim.world.tick;
                let result = saves::save(dir, &name, &sim);
                list.0 = read_list(&files, &sim);
                match (result, quick) {
                    (Ok(()), true) => format!("Quicksaved (tick {tick})"),
                    (Ok(()), false) => format!("Saved {name} (tick {tick})"),
                    (Err(why), true) => format!("Quicksave failed: {why}"),
                    (Err(why), false) => format!("Save failed: {why}"),
                }
            }
            (Some(dir), SaveAction::Load(name)) => match saves::load(dir, name, &sim.defs) {
                Ok(world) => {
                    let tick = load_into(&mut commands, &art, &mut sim, world, &mut reset);
                    open.0 = false;
                    if quick {
                        format!("Quickloaded (tick {tick})")
                    } else {
                        format!("Loaded {name} (tick {tick})")
                    }
                }
                Err(why) if quick => format!("Quickload failed: {why}"),
                Err(why) => format!("Load failed: {why}"),
            },
        };
        info!("{text}");
        notice.0 = text.clone();
        *toast = SaveToast { text, left: TOAST_SECONDS };
    }
}

/// Makes `world` the running game: replaces the sim's world and rebuilds everything drawn from
/// it, as for a new map. Returns its tick.
fn load_into(
    commands: &mut Commands,
    art: &Art,
    sim: &mut Sim,
    world: World,
    reset: &mut Reset,
) -> u64 {
    sim.world = world;
    let tick = sim.world.tick;
    reset.selection.0 = None;
    reset.tool.0 = Tool::Select;
    // The alert log reads a world with a later tick as the same game going on, so start it over
    // by hand, and let the loaded game's past events age out at once instead of flashing by.
    reset.alerts.0 = fn_ui::Alerts::default();
    reset.alerts.0.update(sim, f32::MAX);
    reset.casts.forget(commands);
    for e in &reset.existing {
        commands.entity(e).despawn();
    }
    spawn_entities(commands, art, sim);
    tick
}

/// Takes the save result off the side panel when its time is up, unless something else has
/// replaced it since.
fn expire_toast(time: Res<Time>, mut toast: ResMut<SaveToast>, mut notice: ResMut<Notice>) {
    if toast.left <= 0.0 {
        return;
    }
    toast.left -= time.delta_secs();
    if toast.left <= 0.0 && notice.0 == toast.text {
        notice.0.clear();
    }
}

// ---------------------------------------------------------------------------------------------
// The save list screen

/// Marks the screen's top-level entity, so it can be despawned on close or rebuilt.
#[derive(Component)]
struct ListRoot;

/// A clickable button of the list; `click_list` hit-tests the cursor against these.
#[derive(Component, Clone)]
struct ListButton(SaveAction);

/// A left click on a button of the open list asks for its action.
fn click_list(
    mouse: Res<ButtonInput<MouseButton>>,
    open: Res<SavesOpen>,
    window: Query<&Window, With<PrimaryWindow>>,
    buttons: Query<(&ListButton, &ComputedNode, &UiGlobalTransform)>,
    mut actions: MessageWriter<SaveAction>,
) {
    if !open.0 || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = window.single() else { return };
    let hit = ui_style::node_under_cursor(window, buttons.iter().map(|(b, n, t)| (&b.0, n, t)));
    if let Some(action) = hit {
        actions.write(action.clone());
    }
}

/// Spawns the screen when it opens, despawns it when it closes, and rebuilds it when the list or
/// the nation (its colours) changes.
#[allow(clippy::too_many_arguments)]
fn sync_list_screen(
    mut commands: Commands,
    open: Res<SavesOpen>,
    list: Res<SaveList>,
    sim: Res<Sim>,
    theme: Res<Theme>,
    skin: Res<Skin>,
    roots: Query<Entity, With<ListRoot>>,
    mut shown: Local<bool>,
) {
    let changed = list.is_changed() || theme.is_changed() || skin.is_changed();
    if open.0 && *shown && !changed {
        return;
    }
    if !open.0 && !*shown {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    *shown = open.0;
    if open.0 {
        spawn_list_screen(&mut commands, &theme, &skin, &sim, &list.0);
    }
}

fn spawn_list_screen(
    commands: &mut Commands,
    theme: &Theme,
    skin: &Skin,
    sim: &Session,
    list: &Result<Vec<Slot>, String>,
) {
    // Covers everything left of the side panel, full height, and dims the map under it, as the
    // research screen does.
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
    commands.spawn((ListRoot, dim, BackgroundColor(Color::BLACK.with_alpha(0.55)))).with_children(
        |root| {
            let panel = Node {
                width: Val::Px(PANEL_WIDTH),
                max_height: Val::Percent(90.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::M),
                padding: UiRect::all(Val::Px(space::XL)),
                ..default()
            };
            root.spawn(ui_style::panel(theme, Frame::Panel, panel)).with_children(|p| {
                ui_style::frame(p, theme, skin, Frame::Panel);
                p.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|r| {
                    r.spawn(ui_style::text("Saves", font::TITLE, theme.text));
                    if list.is_ok() {
                        button(r, theme, "New save", SaveAction::New, true);
                    }
                });
                let rows = Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S),
                    overflow: Overflow::clip(),
                    ..default()
                };
                p.spawn(rows).with_children(|rows| match list {
                    Err(why) => {
                        rows.spawn(ui_style::text(
                            format!("Can't list the saves: {why}"),
                            font::BODY,
                            theme.bad,
                        ));
                    }
                    Ok(slots) if slots.is_empty() => {
                        rows.spawn(ui_style::text(
                            "No saves yet. New save (or F5) makes one.",
                            font::BODY,
                            theme.dim,
                        ));
                    }
                    Ok(slots) => {
                        for slot in slots {
                            slot_row(rows, theme, sim, slot);
                        }
                    }
                });
                p.spawn(ui_style::text(
                    "Save writes the running game over that slot; Load replaces the running \
                     game. F5 quicksave, F9 quickload. Esc: close",
                    font::LABEL,
                    theme.dim,
                ));
            });
        },
    );
}

fn slot_row(p: &mut ChildSpawnerCommands, theme: &Theme, sim: &Session, slot: &Slot) {
    let node = Node {
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        column_gap: Val::Px(space::M),
        padding: UiRect::all(Val::Px(space::S)),
        border: UiRect::all(Val::Px(1.0)),
        ..default()
    };
    let border = if slot.loadable() { theme.accent_soft } else { theme.bad };
    p.spawn((node, BackgroundColor(theme.inset), BorderColor::all(border))).with_children(|r| {
        r.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            flex_shrink: 1.0,
            ..default()
        })
        .with_children(|c| {
            c.spawn(ui_style::text(slot.name.clone(), font::BODY, theme.text));
            let color = if slot.loadable() { theme.dim } else { theme.bad };
            c.spawn(ui_style::text(slot.describe(&sim.defs), font::LABEL, color));
        });
        r.spawn(Node { column_gap: Val::Px(space::S), flex_shrink: 0.0, ..default() })
            .with_children(|b| {
                button(b, theme, "Save", SaveAction::Save(slot.name.clone()), false);
                if slot.loadable() {
                    button(b, theme, "Load", SaveAction::Load(slot.name.clone()), true);
                }
            });
    });
}

fn button(
    p: &mut ChildSpawnerCommands,
    theme: &Theme,
    label: &str,
    action: SaveAction,
    main: bool,
) {
    let (bg, fg) = if main { (theme.accent, theme.on_accent) } else { (theme.panel, theme.text) };
    let node = Node {
        padding: UiRect::axes(Val::Px(space::M), Val::Px(space::XS)),
        border: UiRect::all(Val::Px(1.0)),
        ..default()
    };
    p.spawn((ListButton(action), node, BackgroundColor(bg), BorderColor::all(theme.accent)))
        .with_children(|b| {
            b.spawn(ui_style::text(label, font::LABEL, fg));
        });
}

#[cfg(test)]
mod tests {
    use std::fs;

    use fn_sim::{Defs, Nation};

    use super::*;

    /// A fresh empty directory for one test, removed when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("fn-save-test-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn session() -> Session {
        let mut s = Session::new(3, Nation::Water, 48, 32);
        s.tick_n(40);
        s
    }

    // The keys, the list, the toast and the respawn.

    use crate::bending::AlertLog;
    use crate::unit_motion::UnitMotion;
    use crate::{UnitSprite, insert_test_art};

    fn app(dir: Option<PathBuf>) -> App {
        let mut app = App::new();
        insert_test_art(&mut app);
        app.add_plugins(bevy::time::TimePlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Notice>()
            .init_resource::<ActiveTool>()
            .init_resource::<Selection>()
            .init_resource::<AlertLog>()
            .init_resource::<Casts>()
            .init_resource::<SaveToast>()
            .init_resource::<ScreenOpen>()
            .init_resource::<SavesOpen>()
            .init_resource::<SaveList>()
            .add_message::<SaveAction>()
            .insert_resource(SaveFiles(dir))
            .insert_resource(Sim(session()))
            .add_systems(Update, (toggle_list, quick_keys, run_actions, expire_toast).chain());
        app.update();
        spawn_for_test(&mut app);
        app
    }

    fn spawn_for_test(app: &mut App) {
        let units: Vec<_> =
            app.world().resource::<Sim>().world.units.iter().map(|u| (u.id, u.pos)).collect();
        for (id, pos) in units {
            let at = crate::unit_translation(pos);
            app.world_mut().spawn((
                UnitSprite(id),
                Transform::from_translation(at),
                UnitMotion::new(pos, at),
                MapEntity,
            ));
        }
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(key);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
    }

    fn notice(app: &App) -> String {
        app.world().resource::<Notice>().0.clone()
    }

    #[test]
    fn f5_saves_and_f9_replaces_the_running_game_and_resets_the_client_state() {
        let tmp = TempDir::new("keys");
        let mut app = app(Some(tmp.0.clone()));
        press(&mut app, SAVE_KEY);
        assert!(notice(&app).starts_with("Quicksaved (tick 40)"), "{}", notice(&app));
        let saved = app.world().resource::<Sim>().world.save(&Defs::bundled()).unwrap();

        // Play on, select a unit, pick a tool; then load.
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            sim.tick_n(200);
        }
        let first = app.world().resource::<Sim>().world.units[0].id;
        app.world_mut().resource_mut::<Selection>().0 = Some(first);
        app.world_mut().resource_mut::<ActiveTool>().0 = Tool::Harvest;
        press(&mut app, LOAD_KEY);

        assert!(notice(&app).starts_with("Quickloaded (tick 40)"), "{}", notice(&app));
        let sim = app.world().resource::<Sim>();
        assert_eq!(sim.world.save(&sim.defs).unwrap(), saved);
        assert_eq!(app.world().resource::<Selection>().0, None);
        assert_eq!(app.world().resource::<ActiveTool>().0, Tool::Select);

        // Every unit has exactly one sprite on its place, ready to be drawn there, not to
        // glide: the new sprites start on their tiles with nothing to chase.
        app.update();
        let mut sprites = app.world_mut().query::<(&UnitSprite, &Transform, &UnitMotion)>();
        let found: Vec<_> =
            sprites.iter(app.world()).map(|(u, t, _)| (u.0, t.translation)).collect();
        let sim = app.world().resource::<Sim>();
        assert_eq!(found.len(), sim.world.units.len());
        for u in &sim.world.units {
            let at = found.iter().find(|(id, _)| *id == u.id).expect("a sprite").1;
            assert_eq!(at, crate::unit_translation(u.pos));
        }
    }

    #[test]
    fn a_failed_load_toasts_and_leaves_the_running_game_alone() {
        let tmp = TempDir::new("failed-load");
        fs::write(saves::slot_path(&tmp.0, QUICKSAVE), "garbage").unwrap();
        let mut app = app(Some(tmp.0.clone()));
        let before = app.world().resource::<Sim>().world.save(&Defs::bundled()).unwrap();
        let first = app.world().resource::<Sim>().world.units[0].id;
        app.world_mut().resource_mut::<Selection>().0 = Some(first);
        app.world_mut().resource_mut::<ActiveTool>().0 = Tool::Harvest;
        press(&mut app, LOAD_KEY);
        assert!(
            notice(&app).starts_with("Quickload failed: not a Four Nations save"),
            "{}",
            notice(&app)
        );
        let sim = app.world().resource::<Sim>();
        assert_eq!(sim.world.save(&sim.defs).unwrap(), before);
        assert_eq!(app.world().resource::<Selection>().0, Some(first), "selection kept");
        assert_eq!(app.world().resource::<ActiveTool>().0, Tool::Harvest, "tool kept");
    }

    #[test]
    fn without_a_data_directory_both_keys_toast_instead_of_panicking() {
        let mut app = app(None);
        press(&mut app, SAVE_KEY);
        assert!(notice(&app).starts_with("Save failed: this platform has no data directory"));
        press(&mut app, LOAD_KEY);
        assert!(notice(&app).starts_with("Save failed"));
    }

    #[test]
    fn the_toast_leaves_the_side_panel_when_its_time_is_up_but_not_someone_elses_text() {
        let tmp = TempDir::new("expire");
        let mut app = app(Some(tmp.0.clone()));
        press(&mut app, SAVE_KEY);
        assert!(!notice(&app).is_empty());
        app.world_mut().resource_mut::<SaveToast>().left = 0.001;
        std::thread::sleep(std::time::Duration::from_millis(5));
        app.update();
        app.update();
        assert!(notice(&app).is_empty(), "{}", notice(&app));

        press(&mut app, SAVE_KEY);
        app.world_mut().resource_mut::<Notice>().0 = "Harvest: 3 tiles".into();
        app.world_mut().resource_mut::<SaveToast>().left = 0.001;
        std::thread::sleep(std::time::Duration::from_millis(5));
        app.update();
        app.update();
        assert_eq!(notice(&app), "Harvest: 3 tiles");
    }

    fn act(app: &mut App, action: SaveAction) {
        app.world_mut().write_message(action);
        app.update();
    }

    fn listed(app: &App) -> Vec<(String, bool)> {
        let list = app.world().resource::<SaveList>();
        let mut names: Vec<_> =
            list.0.as_ref().unwrap().iter().map(|s| (s.name.clone(), s.loadable())).collect();
        names.sort();
        names
    }

    #[test]
    fn f6_lists_the_slots_and_new_save_overwrite_and_load_go_through_them() {
        let tmp = TempDir::new("slots");
        fs::write(saves::slot_path(&tmp.0, "damaged"), "not a save").unwrap();
        let mut app = app(Some(tmp.0.clone()));
        press(&mut app, LIST_KEY);
        assert!(app.world().resource::<SavesOpen>().0);
        assert_eq!(
            listed(&app),
            [("damaged".into(), false)],
            "a damaged file is listed, unloadable"
        );

        act(&mut app, SaveAction::New);
        assert_eq!(notice(&app), "Saved save-1 (tick 40)");
        let at_40 = app.world().resource::<Sim>().world.save(&Defs::bundled()).unwrap();
        app.world_mut().resource_mut::<Sim>().tick_n(20);
        act(&mut app, SaveAction::New);
        assert_eq!(notice(&app), "Saved save-2 (tick 60)");
        assert_eq!(
            listed(&app),
            [("damaged".into(), false), ("save-1".into(), true), ("save-2".into(), true)],
            "the list is read again after each save"
        );

        // Overwrite save-2 at tick 80, play on, then load save-1 and save-2 in turn.
        app.world_mut().resource_mut::<Sim>().tick_n(20);
        act(&mut app, SaveAction::Save("save-2".into()));
        assert_eq!(notice(&app), "Saved save-2 (tick 80)");
        app.world_mut().resource_mut::<Sim>().tick_n(100);
        act(&mut app, SaveAction::Load("save-1".into()));
        assert_eq!(notice(&app), "Loaded save-1 (tick 40)");
        assert!(!app.world().resource::<SavesOpen>().0, "a load closes the list");
        let sim = app.world().resource::<Sim>();
        assert_eq!(sim.world.save(&sim.defs).unwrap(), at_40);
        act(&mut app, SaveAction::Load("save-2".into()));
        assert_eq!(app.world().resource::<Sim>().world.tick, 80);

        // The damaged one fails as a toast, and the game is kept.
        act(&mut app, SaveAction::Load("damaged".into()));
        assert!(
            notice(&app).starts_with("Load failed: not a Four Nations save"),
            "{}",
            notice(&app)
        );
        assert_eq!(app.world().resource::<Sim>().world.tick, 80);
    }

    #[test]
    fn the_list_and_the_research_screen_never_show_together() {
        let tmp = TempDir::new("screens");
        let mut app = app(Some(tmp.0.clone()));
        app.world_mut().resource_mut::<ScreenOpen>().0 = true;
        press(&mut app, LIST_KEY);
        assert!(app.world().resource::<SavesOpen>().0);
        assert!(!app.world().resource::<ScreenOpen>().0, "opening the list closes research");
        app.world_mut().resource_mut::<ScreenOpen>().0 = true;
        app.update();
        assert!(!app.world().resource::<SavesOpen>().0, "opening research closes the list");
        press(&mut app, LIST_KEY);
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<SavesOpen>().0, "Esc closes it");
    }
}

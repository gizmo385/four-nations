//! Quicksave and quickload (#112): `F5` writes the running game to a file, `F9` replaces the
//! running game with it. The sim turns a `World` into text and back (`fn_sim::save`) and does no
//! I/O; the files, the directory and the toasts are all here.
//!
//! - **Where**: `quicksave.ron` in the platform's data directory (`save_dir`): `$XDG_DATA_HOME` or
//!   `~/.local/share` on Linux, `~/Library/Application Support` on macOS, `%APPDATA%` on Windows,
//!   each followed by `four-nations/saves`. `FN_SAVE_DIR` names another directory (tests, and
//!   anyone who wants their saves elsewhere). The directory is created on the first save.
//! - **Atomic**: a save is written to a temporary file beside the real one, flushed to disk and
//!   then renamed over it, so a crash or a full disk mid-save leaves the previous save as it was.
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

use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use fn_sim::{Defs, World};
use fn_ui::{Session, Tool};

use crate::bending::AlertLog;
use crate::cast::Casts;
use crate::designate::{ActiveTool, Notice};
use crate::{Art, MapEntity, Selection, Sim, spawn_entities};

/// The quicksave and quickload keys.
pub const SAVE_KEY: KeyCode = KeyCode::F5;
pub const LOAD_KEY: KeyCode = KeyCode::F9;

/// The file the quicksave key writes, in the save directory.
pub const QUICKSAVE_FILE: &str = "quicksave.ron";

/// How long a save or load result shows in the side panel, in seconds.
const TOAST_SECONDS: f32 = 5.0;

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveFiles>()
            .init_resource::<SaveToast>()
            // Before `controls`, so a load and a regenerate pressed on one frame don't both
            // respawn the map.
            .add_systems(Update, (quick_keys.before(crate::controls), expire_toast).chain());
    }
}

/// Where saves go: `None` when the platform has no data directory to put them in.
#[derive(Resource)]
pub struct SaveFiles(pub Option<PathBuf>);

impl Default for SaveFiles {
    fn default() -> Self {
        SaveFiles(save_dir(|k| std::env::var_os(k), std::env::consts::OS))
    }
}

/// The notice a save or load wrote, and how long it has left.
#[derive(Resource, Default)]
struct SaveToast {
    text: String,
    left: f32,
}

/// The directory saves live in, from `env` (a lookup of environment variables) on operating
/// system `os` (`std::env::consts::OS`).
pub fn save_dir(env: impl Fn(&str) -> Option<OsString>, os: &str) -> Option<PathBuf> {
    let set = |k: &str| env(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(dir) = set("FN_SAVE_DIR") {
        return Some(dir);
    }
    let base = match os {
        "windows" => set("APPDATA")?,
        "macos" => set("HOME")?.join("Library").join("Application Support"),
        _ => {
            set("XDG_DATA_HOME").or_else(|| set("HOME").map(|h| h.join(".local").join("share")))?
        }
    };
    Some(base.join("four-nations").join("saves"))
}

/// Writes `text` to `path` so that `path` is either what it was or all of `text`: into a
/// temporary file in the same directory (a rename is only atomic within one filesystem), flushed
/// to disk, then renamed over `path`. Creates the directory if it is missing. The temporary file
/// is removed if anything fails.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = dir.join(tmp_name);
    let written = fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(text.as_bytes())?;
        f.sync_all()
    });
    match written.and_then(|()| fs::rename(&tmp, path)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Saves `session`'s world to the quicksave file in `dir`. The error is a line for the toast.
pub fn quicksave(dir: &Path, session: &Session) -> Result<(), String> {
    let text = session.world.save(&session.defs).map_err(|e| e.to_string())?;
    write_atomic(&dir.join(QUICKSAVE_FILE), &text)
        .map_err(|e| format!("could not write {}: {e}", dir.join(QUICKSAVE_FILE).display()))
}

/// Reads the quicksave file in `dir` into a world checked against `defs`. The error is a line for
/// the toast.
pub fn quickload(dir: &Path, defs: &Defs) -> Result<World, String> {
    let path = dir.join(QUICKSAVE_FILE);
    let text = fs::read_to_string(&path).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => format!("no quicksave yet ({})", path.display()),
        _ => format!("could not read {}: {e}", path.display()),
    })?;
    World::load(&text, defs).map_err(|e| e.to_string())
}

#[derive(SystemParam)]
struct Reset<'w, 's> {
    selection: ResMut<'w, Selection>,
    tool: ResMut<'w, ActiveTool>,
    alerts: ResMut<'w, AlertLog>,
    casts: ResMut<'w, Casts>,
    existing: Query<'w, 's, Entity, With<MapEntity>>,
}

/// `F5` saves, `F9` loads, and either says how it went in the side panel.
fn quick_keys(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    files: Res<SaveFiles>,
    art: Res<Art>,
    mut sim: ResMut<Sim>,
    mut notice: ResMut<Notice>,
    mut toast: ResMut<SaveToast>,
    mut reset: Reset,
) {
    let saving = keys.just_pressed(SAVE_KEY);
    let loading = keys.just_pressed(LOAD_KEY);
    if !saving && !loading {
        return;
    }
    let text = match files.0.as_deref() {
        None => "Save failed: this platform has no data directory (set FN_SAVE_DIR)".to_owned(),
        Some(dir) if saving => match quicksave(dir, &sim) {
            Ok(()) => format!("Quicksaved (tick {})", sim.world.tick),
            Err(why) => format!("Quicksave failed: {why}"),
        },
        Some(dir) => match quickload(dir, &sim.defs) {
            Ok(world) => {
                let tick = load_into(&mut commands, &art, &mut sim, world, &mut reset);
                format!("Quickloaded (tick {tick})")
            }
            Err(why) => format!("Quickload failed: {why}"),
        },
    };
    info!("{text}");
    notice.0 = text.clone();
    *toast = SaveToast { text, left: TOAST_SECONDS };
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

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

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

    #[test]
    fn the_save_dir_is_the_override_else_the_platforms_data_dir() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| OsString::from(v))
        };
        let both = env(&[("FN_SAVE_DIR", "/x"), ("HOME", "/h")]);
        assert_eq!(save_dir(both, "linux"), Some(PathBuf::from("/x")));
        let xdg = env(&[("XDG_DATA_HOME", "/d"), ("HOME", "/h")]);
        assert_eq!(save_dir(xdg, "linux"), Some(PathBuf::from("/d/four-nations/saves")));
        let home = env(&[("HOME", "/h")]);
        assert_eq!(
            save_dir(home, "linux"),
            Some(PathBuf::from("/h/.local/share/four-nations/saves"))
        );
        assert_eq!(
            save_dir(env(&[("HOME", "/h")]), "macos"),
            Some(PathBuf::from("/h/Library/Application Support/four-nations/saves"))
        );
        assert_eq!(
            save_dir(env(&[("APPDATA", "C:/a")]), "windows"),
            Some(PathBuf::from("C:/a/four-nations/saves"))
        );
        assert_eq!(save_dir(env(&[]), "linux"), None);
        assert_eq!(save_dir(env(&[("FN_SAVE_DIR", "")]), "linux"), None, "empty is unset");
    }

    #[test]
    fn a_quicksave_round_trips_and_creates_the_directory() {
        let tmp = TempDir::new("roundtrip");
        let dir = tmp.0.join("not").join("there").join("yet");
        let s = session();
        quicksave(&dir, &s).unwrap();
        let loaded = quickload(&dir, &s.defs).unwrap();
        assert_eq!(loaded.tick, s.world.tick);
        assert_eq!(loaded.save(&s.defs), s.world.save(&s.defs));
        let names: Vec<_> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, [OsString::from(QUICKSAVE_FILE)], "no temporary file left behind");
    }

    #[test]
    fn a_failed_write_leaves_the_previous_save_untouched() {
        let tmp = TempDir::new("atomic");
        let path = tmp.0.join(QUICKSAVE_FILE);
        write_atomic(&path, "first").unwrap();
        // A directory where the temporary file should go makes the write fail.
        fs::create_dir(tmp.0.join(format!("{QUICKSAVE_FILE}.tmp"))).unwrap();
        assert!(write_atomic(&path, "second").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");
        // And a good write replaces it whole.
        fs::remove_dir(tmp.0.join(format!("{QUICKSAVE_FILE}.tmp"))).unwrap();
        write_atomic(&path, "second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
    }

    #[test]
    fn loading_what_is_missing_or_damaged_is_an_error_line_not_a_panic() {
        let tmp = TempDir::new("errors");
        let defs = Defs::bundled();
        let missing = quickload(&tmp.0, &defs).unwrap_err();
        assert!(missing.contains("no quicksave yet"), "{missing}");

        fs::write(tmp.0.join(QUICKSAVE_FILE), "this is not a save").unwrap();
        let junk = quickload(&tmp.0, &defs).unwrap_err();
        assert!(junk.contains("not a Four Nations save"), "{junk}");

        let s = session();
        let text = s.world.save(&defs).unwrap();
        fs::write(tmp.0.join(QUICKSAVE_FILE), &text[..text.len() / 2]).unwrap();
        let cut = quickload(&tmp.0, &defs).unwrap_err();
        assert!(cut.contains("damaged save"), "{cut}");
    }

    // The Bevy half: the keys, the toast and the respawn.

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
            .insert_resource(SaveFiles(dir))
            .insert_resource(Sim(session()))
            .add_systems(Update, (quick_keys, expire_toast).chain());
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
        fs::write(tmp.0.join(QUICKSAVE_FILE), "garbage").unwrap();
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
}

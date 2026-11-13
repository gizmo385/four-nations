//! Saves (#112, #216, #860): where they live, how one is written so a crash can't eat it, and
//! the list of named slots the frontend shows. The sim turns a `World` into text and back
//! (`fn_sim::save`) and does no I/O; this module is the one place in `fn_ui` that touches the
//! filesystem, so saves are read and written the same way everywhere.
//!
//! - **A store** ([`SaveStore`]): where the slots are kept. Everything else here (names, headers,
//!   the list, errors) is the same for every store and is written once, over four small methods.
//!   [`DiskStore`] is the native one, below. The browser build has no filesystem: `fn_gui`'s
//!   wasm target implements the trait over `localStorage` (`crates/gui/src/browser_saves.rs`),
//!   which is why the trait is here and that implementation is there (`web-sys` is a
//!   dependency of that target, not of this crate).
//! - **Where** (on disk): one `NAME.ron` file per slot in the platform's data directory (`save_dir`):
//!   `$XDG_DATA_HOME` or `~/.local/share` on Linux, `~/Library/Application Support` on macOS,
//!   `%APPDATA%` on Windows, each followed by `four-nations/saves`. `FN_SAVE_DIR` names another
//!   directory (tests, and anyone who wants their saves elsewhere). The directory is created on
//!   the first save. The quicksave is the slot named [`QUICKSAVE`].
//! - **Atomic**: a save is written to a temporary file beside the real one, flushed to disk and
//!   then renamed over it, so a crash or a full disk mid-save leaves the previous save as it was.
//! - **Listing is cheap**: [`list`] reads only each file's first line, the header
//!   (`fn_sim::save::inspect`), never the multi-megabyte world. A file that isn't a save, is from
//!   another format version or is a v1 save of data this build has no id tables for is listed as
//!   unloadable with the reason, rather than failing the list. A file whose header is fine but
//!   whose world is damaged, or which uses content this build's data no longer has (#602), only
//!   shows that when it is loaded, as the load's error.
//! - **Errors are lines**: every failure is a `String` a frontend can show as it is.

use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use fn_sim::save::{SaveInfo, inspect};
use fn_sim::{Defs, World};

use crate::Session;
use crate::time::{clock_text_at, game_time};

/// The slot the quicksave and quickload keys use.
pub const QUICKSAVE: &str = "quicksave";

/// The extension of a slot's file.
pub const EXTENSION: &str = "ron";

/// The longest slot name, in characters.
pub const MAX_NAME: usize = 40;

/// The most of a file [`list`] reads looking for the header line. Real headers are about a
/// hundred bytes; anything longer is not one.
const HEADER_LIMIT: u64 = 4096;

/// What a store knows of one slot without reading its world: enough for the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The slot's name. The list drops names [`check_name`] refuses.
    pub name: String,
    /// The first line of the save, its header (at most [`HEADER_LIMIT`] bytes of it), or why it
    /// couldn't be read.
    pub header: Result<String, String>,
    /// When the slot was last written, if the store says.
    pub modified: Option<SystemTime>,
}

/// Where the named slots are kept. A store moves text and knows nothing of what is in it: names
/// are checked, headers inspected, worlds parsed and errors worded by this module's functions
/// ([`list`], [`save`], [`load`]), the same for every store. Every error is a line a frontend
/// can show as it is.
pub trait SaveStore: Send + Sync {
    /// Every slot that exists, in any order, each with its header line. Cheap: never the whole
    /// world of every slot where the store can help it. A store with nothing in it yet is `Ok`
    /// and empty.
    fn entries(&self) -> Result<Vec<Entry>, String>;

    /// The text of slot `name`, or `None` if there is no such slot.
    fn read(&self, name: &str) -> Result<Option<String>, String>;

    /// Replaces slot `name` with `text`, all or nothing: if it fails, the slot is as it was.
    fn write(&self, name: &str, text: &str) -> Result<(), String>;

    /// Where slot `name` is, for the status line: a path on disk, a storage key in a browser.
    fn place(&self, name: &str) -> String;
}

/// A store an [`crate::App`] and the shell hold: shared, so cloning the app is cheap.
pub type Store = Arc<dyn SaveStore>;

/// The save files in a directory (see the module doc for which one by default): one
/// `NAME.ron` per slot, written atomically ([`write_atomic`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskStore {
    dir: PathBuf,
}

impl DiskStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        DiskStore { dir: dir.into() }
    }

    /// A [`Store`] handle on the directory `dir`.
    pub fn shared(dir: impl Into<PathBuf>) -> Store {
        Arc::new(Self::new(dir))
    }

    /// The store for this process: [`default_save_dir`], if the platform has one.
    pub fn default_shared() -> Option<Store> {
        default_save_dir().map(Self::shared)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl SaveStore for DiskStore {
    /// Only `*.ron` files are slots; anything else there, the temporary files of a save in
    /// progress included, is left out. A directory that doesn't exist yet has no slots.
    fn entries(&self) -> Result<Vec<Entry>, String> {
        let dir = &self.dir;
        let found = match fs::read_dir(dir) {
            Ok(found) => found,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("could not list {}: {e}", dir.display())),
        };
        let mut entries = Vec::new();
        for entry in found.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != EXTENSION) || !path.is_file() {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            let modified = entry.metadata().and_then(|m| m.modified()).ok();
            entries.push(Entry {
                name: name.to_owned(),
                header: read_header_line(&path),
                modified,
            });
        }
        Ok(entries)
    }

    fn read(&self, name: &str) -> Result<Option<String>, String> {
        let path = slot_path(&self.dir, name);
        match fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("could not read {}: {e}", path.display())),
        }
    }

    fn write(&self, name: &str, text: &str) -> Result<(), String> {
        let path = slot_path(&self.dir, name);
        write_atomic(&path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
    }

    fn place(&self, name: &str) -> String {
        slot_path(&self.dir, name).display().to_string()
    }
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

/// [`save_dir`] for this process: its environment, on the platform it was built for.
pub fn default_save_dir() -> Option<PathBuf> {
    save_dir(|k| std::env::var_os(k), std::env::consts::OS)
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

/// Checks that `name` can be a slot: 1 to [`MAX_NAME`] letters, digits, `-` and `_`, so it is a
/// file name on every platform and can't reach outside the save directory.
pub fn check_name(name: &str) -> Result<(), String> {
    let ok = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    if name.is_empty() || name.chars().count() > MAX_NAME || !name.chars().all(ok) {
        return Err(format!(
            "{name:?} can't be a save name (use 1 to {MAX_NAME} letters, digits, - and _)"
        ));
    }
    Ok(())
}

/// The file slot `name` is kept in.
pub fn slot_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.{EXTENSION}"))
}

/// One save file, as the save list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub name: String,
    pub state: SlotState,
    /// When the file was last written, if the filesystem says.
    pub modified: Option<SystemTime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotState {
    /// The header reads and matches this build's format and game data.
    Ready(SaveInfo),
    /// It can't be loaded, and this says why.
    Unloadable(String),
}

impl Slot {
    pub fn loadable(&self) -> bool {
        matches!(self.state, SlotState::Ready(_))
    }

    /// What the list says about the slot: "Water Tribes, seed 3, Spring Y1 - 14:56 left (tick 40)", or why it
    /// can't be loaded.
    pub fn describe(&self, defs: &Defs) -> String {
        match &self.state {
            SlotState::Ready(info) => format!(
                "{}, seed {}, {} (tick {})",
                defs.nation(info.nation).name,
                info.seed,
                clock_text_at(defs, info.tick, info.seasons.unwrap_or(true)),
                info.tick
            ),
            SlotState::Unloadable(why) => format!("can't load: {why}"),
        }
    }

    /// The short, player-facing line (#943): "Water Tribes · Spring Y4", joined with `dot`. No
    /// seed, tick or time left, which a title row has no room for and a player doesn't read; an
    /// unloadable slot says why, as [`Slot::describe`] does.
    pub fn summary(&self, defs: &Defs, dot: &str) -> String {
        match &self.state {
            SlotState::Ready(info) => {
                let when = game_time(defs, info.tick, info.seasons.unwrap_or(true)).label();
                format!("{} {dot} {when}", defs.nation(info.nation).name)
            }
            SlotState::Unloadable(why) => format!("can't load: {why}"),
        }
    }
}

/// Every slot in `store`, newest first (then by name). A store with nothing in it has no slots.
pub fn list(store: &dyn SaveStore) -> Result<Vec<Slot>, String> {
    let mut slots = Vec::new();
    for Entry { name, header, modified } in store.entries()? {
        if check_name(&name).is_err() {
            continue;
        }
        let state =
            match header.and_then(|line| inspect(line.trim_end()).map_err(|e| e.to_string())) {
                Err(why) => SlotState::Unloadable(why),
                Ok(info) => SlotState::Ready(info),
            };
        slots.push(Slot { name, state, modified });
    }
    slots.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    Ok(slots)
}

/// The header line of the save at `path`, from its first line only.
fn read_header_line(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| format!("could not read it ({e})"))?;
    let mut line = String::new();
    BufReader::new(file.take(HEADER_LIMIT))
        .read_line(&mut line)
        .map_err(|e| format!("not a Four Nations save ({e})"))?;
    Ok(line)
}

/// A name for a new slot that isn't taken: `save-1`, `save-2`, ...
pub fn new_name(slots: &[Slot]) -> String {
    (1..)
        .map(|n| format!("save-{n}"))
        .find(|name| slots.iter().all(|s| s.name != *name))
        .expect("an unused name")
}

/// A name for a new slot made without typing (#979's touch play, #976): the nation and the game
/// time, "Earth-Kingdom-Spring-Y1", as a slot name allows (letters, digits and `-`), with `-2`,
/// `-3` and on added when that is taken.
pub fn auto_name(slots: &[Slot], session: &Session) -> String {
    let nation = &session.defs.nation(session.world.nation).name;
    let when = crate::time::session_time(session).label();
    let mut base = String::new();
    for c in format!("{nation} {when}").chars() {
        if c.is_ascii_alphanumeric() {
            base.push(c);
        } else if !base.is_empty() && !base.ends_with('-') {
            base.push('-');
        }
    }
    let base = base.trim_end_matches('-');
    // Room for a `-NN` on the end.
    let base: String = base.chars().take(MAX_NAME - 4).collect();
    let taken = |name: &str| slots.iter().any(|s| s.name == name);
    if !taken(&base) {
        return base;
    }
    (2..).map(|n| format!("{base}-{n}")).find(|name| !taken(name)).expect("an unused name")
}

/// Saves `session`'s world to slot `name` in `store`, replacing what was there.
pub fn save(store: &dyn SaveStore, name: &str, session: &Session) -> Result<(), String> {
    check_name(name)?;
    let text = session.world.save(&session.defs).map_err(|e| e.to_string())?;
    store.write(name, &text)
}

/// Reads slot `name` in `store` into a world checked against `defs`.
pub fn load(store: &dyn SaveStore, name: &str, defs: &Defs) -> Result<World, String> {
    check_name(name)?;
    let text = store.read(name)?.ok_or_else(|| match name {
        QUICKSAVE => format!("no quicksave yet ({})", store.place(name)),
        _ => format!("no save named {name} ({})", store.place(name)),
    })?;
    World::load(&text, defs).map_err(|e| e.to_string())
}

/// Saves to the quicksave slot.
pub fn quicksave(store: &dyn SaveStore, session: &Session) -> Result<(), String> {
    save(store, QUICKSAVE, session)
}

/// Loads the quicksave slot.
pub fn quickload(store: &dyn SaveStore, defs: &Defs) -> Result<World, String> {
    load(store, QUICKSAVE, defs)
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
                std::env::temp_dir().join(format!("fn-saves-test-{}-{name}", std::process::id()));
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
        quicksave(&DiskStore::new(&dir), &s).unwrap();
        let loaded = quickload(&DiskStore::new(&dir), &s.defs).unwrap();
        assert_eq!(loaded.tick, s.world.tick);
        assert_eq!(loaded.save(&s.defs), s.world.save(&s.defs));
        let names: Vec<_> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, [OsString::from("quicksave.ron")], "no temporary file left behind");
    }

    #[test]
    fn a_failed_write_leaves_the_previous_save_untouched() {
        let tmp = TempDir::new("atomic");
        let path = tmp.0.join("quicksave.ron");
        write_atomic(&path, "first").unwrap();
        // A directory where the temporary file should go makes the write fail.
        fs::create_dir(tmp.0.join("quicksave.ron.tmp")).unwrap();
        assert!(write_atomic(&path, "second").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");
        // And a good write replaces it whole.
        fs::remove_dir(tmp.0.join("quicksave.ron.tmp")).unwrap();
        write_atomic(&path, "second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
    }

    #[test]
    fn loading_what_is_missing_or_damaged_is_an_error_line_not_a_panic() {
        let tmp = TempDir::new("errors");
        let defs = Defs::bundled();
        let missing = quickload(&DiskStore::new(&tmp.0), &defs).unwrap_err();
        assert!(missing.contains("no quicksave yet"), "{missing}");
        let missing = load(&DiskStore::new(&tmp.0), "other", &defs).unwrap_err();
        assert!(missing.contains("no save named other"), "{missing}");

        fs::write(tmp.0.join("quicksave.ron"), "this is not a save").unwrap();
        let junk = quickload(&DiskStore::new(&tmp.0), &defs).unwrap_err();
        assert!(junk.contains("not a Four Nations save"), "{junk}");

        let s = session();
        let text = s.world.save(&defs).unwrap();
        fs::write(tmp.0.join("quicksave.ron"), &text[..text.len() / 2]).unwrap();
        let cut = quickload(&DiskStore::new(&tmp.0), &defs).unwrap_err();
        assert!(cut.contains("damaged save"), "{cut}");
    }

    #[test]
    fn names_are_plain_file_names() {
        for good in ["quicksave", "save-1", "Ba_Sing_Se"] {
            assert_eq!(check_name(good), Ok(()), "{good}");
        }
        for bad in ["", "../escape", "a/b", "a b", "a.ron", &"x".repeat(MAX_NAME + 1)] {
            assert!(check_name(bad).is_err(), "{bad:?}");
        }
        let tmp = TempDir::new("names");
        assert!(save(&DiskStore::new(&tmp.0), "../out", &session()).is_err());
        assert!(load(&DiskStore::new(&tmp.0), "../out", &Defs::bundled()).is_err());
    }

    #[test]
    fn the_list_reads_headers_and_marks_what_cannot_load() {
        let tmp = TempDir::new("list");
        let defs = Defs::bundled();
        assert_eq!(
            list(&DiskStore::new(tmp.0.join("missing"))),
            Ok(Vec::new()),
            "no directory, no slots"
        );

        let s = session();
        save(&DiskStore::new(&tmp.0), "save-1", &s).unwrap();
        quicksave(&DiskStore::new(&tmp.0), &s).unwrap();
        fs::write(tmp.0.join("junk.ron"), "this is not a save").unwrap();
        // A newer format, and a v1 save of data this build has no id tables for: the header
        // reads, but this build can't load it. Other data in a current save is fine (#602): its
        // ids are mapped by name when it loads.
        let text = s.world.save(&defs).unwrap();
        let version = format!("version:{}", fn_sim::SAVE_VERSION);
        fs::write(tmp.0.join("future.ron"), text.replacen(&version, "version:99", 1)).unwrap();
        let hash = format!("data_hash:{}", defs.id_hash());
        let modded = text.replacen(&hash, "data_hash:1", 1);
        fs::write(tmp.0.join("modded.ron"), &modded).unwrap();
        fs::write(tmp.0.join("old.ron"), modded.replacen(&version, "version:1", 1)).unwrap();
        // Not slots: another extension, a save in progress, a name that isn't allowed.
        fs::write(tmp.0.join("notes.txt"), "hi").unwrap();
        fs::write(tmp.0.join("save-1.ron.tmp"), "half").unwrap();
        fs::write(tmp.0.join("bad name.ron"), &text).unwrap();

        let mut slots = list(&DiskStore::new(&tmp.0)).unwrap();
        slots.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<_> = slots.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["future", "junk", "modded", "old", "quicksave", "save-1"]);
        let get = |n: &str| slots.iter().find(|s| s.name == n).unwrap();

        let SlotState::Ready(info) = &get("save-1").state else { panic!("save-1 loads") };
        assert_eq!((info.nation, info.seed, info.tick), (Nation::Water, 3, 40));
        // `Session::new` plays with no difficulty, so no seasons: the slot reads "Day N".
        assert_eq!(info.seasons, Some(false));
        assert_eq!(
            get("save-1").describe(&defs),
            "Water Tribes, seed 3, Day 1 - 4:56 left (tick 40)"
        );
        assert!(get("quicksave").loadable());
        assert!(get("modded").loadable(), "the data hash doesn't gate loading");
        for (name, why) in [
            ("junk", "not a Four Nations save"),
            ("future", "version 99"),
            ("old", "old version of the game"),
        ] {
            let slot = get(name);
            assert!(!slot.loadable(), "{name}");
            let text = slot.describe(&defs);
            assert!(text.starts_with("can't load: ") && text.contains(why), "{name}: {text}");
        }
    }

    #[test]
    fn a_slot_words_its_clock_as_the_game_did_when_saved() {
        let tmp = TempDir::new("clock");
        let defs = Defs::bundled();
        // Seasons off (the identity knobs): a day count.
        let off = session().world.save(&defs).unwrap();
        assert!(off.contains("seasons:Some(false)"), "the header records it");
        fs::write(tmp.0.join("off.ron"), &off).unwrap();
        // Seasons on, and a save from before the field existed: both name the season.
        let on = off.replacen("seasons:Some(false)", "seasons:Some(true)", 1);
        fs::write(tmp.0.join("on.ron"), &on).unwrap();
        let old = off.replacen(",seasons:Some(false)", "", 1);
        assert!(!old.contains("seasons:Some"));
        fs::write(tmp.0.join("old.ron"), &old).unwrap();

        let slots = list(&DiskStore::new(&tmp.0)).unwrap();
        let said = |n: &str| slots.iter().find(|s| s.name == n).unwrap().describe(&defs);
        assert_eq!(said("off"), "Water Tribes, seed 3, Day 1 - 4:56 left (tick 40)");
        assert_eq!(said("on"), "Water Tribes, seed 3, Spring Y1 - 14:56 left (tick 40)");
        assert_eq!(said("old"), said("on"));
        // The short line a title row shows (#943): the nation and the season, nothing else.
        let short = |n: &str| slots.iter().find(|s| s.name == n).unwrap().summary(&defs, "·");
        assert_eq!(short("off"), "Water Tribes · Day 1");
        assert_eq!(short("on"), "Water Tribes · Spring Y1");
        // And the same words as the game's own clock, bar the ASCII separator.
        let game = crate::time::clock_text(&session()).replace('·', "-");
        assert!(said("off").contains(&game), "{game}");
    }

    #[test]
    fn a_new_slot_takes_the_first_free_name_and_saving_over_one_replaces_it() {
        let tmp = TempDir::new("slots");
        let defs = Defs::bundled();
        assert_eq!(new_name(&[]), "save-1");
        let mut s = session();
        save(&DiskStore::new(&tmp.0), &new_name(&list(&DiskStore::new(&tmp.0)).unwrap()), &s)
            .unwrap();
        save(&DiskStore::new(&tmp.0), &new_name(&list(&DiskStore::new(&tmp.0)).unwrap()), &s)
            .unwrap();
        fs::remove_file(slot_path(&tmp.0, "save-1")).unwrap();
        assert_eq!(
            new_name(&list(&DiskStore::new(&tmp.0)).unwrap()),
            "save-1",
            "a freed name is reused"
        );

        s.tick_n(10);
        save(&DiskStore::new(&tmp.0), "save-2", &s).unwrap();
        assert_eq!(load(&DiskStore::new(&tmp.0), "save-2", &defs).unwrap().tick, 50);
        assert_eq!(list(&DiskStore::new(&tmp.0)).unwrap().len(), 1);
    }

    /// A save made without typing is named for the nation and the time, and a clash gets a
    /// number.
    #[test]
    fn an_auto_name_is_the_nation_and_the_time_and_a_clash_is_numbered() {
        let tmp = TempDir::new("auto");
        let s = session();
        let when = crate::time::session_time(&s).label().replace(' ', "-");
        let first = auto_name(&[], &s);
        assert_eq!(first, format!("Water-Tribes-{when}"));
        check_name(&first).unwrap();
        let store = DiskStore::new(&tmp.0);
        save(&store, &first, &s).unwrap();
        let second = auto_name(&list(&store).unwrap(), &s);
        assert_eq!(second, format!("{first}-2"));
        save(&store, &second, &s).unwrap();
        assert_eq!(auto_name(&list(&store).unwrap(), &s), format!("{first}-3"));
    }
}

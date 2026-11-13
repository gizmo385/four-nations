//! Browser saves (#860) against an in-memory stand-in for `localStorage`: the slot value, the
//! compression, the quota error and the list, through the same `fn_ui::saves` functions the
//! game calls. `localStorage` itself is checked in a real browser (`web/play/check-saves.cjs`).

use std::collections::BTreeMap;
use std::sync::Mutex;

use fn_gui::browser_saves::{self, Kv, KvSaves, SetError};
use fn_sim::{Defs, Nation};
use fn_ui::Session;
use fn_ui::saves::{self, SaveStore, SlotState};

/// `localStorage`'s shape: strings by key, and a quota on the characters held in all.
struct MemKv {
    items: Mutex<BTreeMap<String, String>>,
    quota: usize,
}

impl MemKv {
    fn new(quota: usize) -> Self {
        MemKv { items: Mutex::new(BTreeMap::new()), quota }
    }
}

impl Kv for &MemKv {
    fn keys(&self) -> Result<Vec<String>, String> {
        Ok(self.items.lock().unwrap().keys().cloned().collect())
    }

    fn get(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.items.lock().unwrap().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SetError> {
        let mut items = self.items.lock().unwrap();
        let others: usize =
            items.iter().filter(|(k, _)| *k != key).map(|(k, v)| k.len() + v.len()).sum();
        if others + key.len() + value.len() > self.quota {
            return Err(SetError::Full);
        }
        items.insert(key.to_owned(), value.to_owned());
        Ok(())
    }
}

fn session(w: i32, h: i32, ticks: u32) -> Session {
    let mut s = Session::new(3, Nation::Water, w, h);
    s.tick_n(ticks);
    s
}

fn clock() -> u64 {
    1_000
}

#[test]
fn a_save_comes_back_byte_for_byte_through_the_store() {
    let kv = MemKv::new(usize::MAX);
    let store = KvSaves::new(&kv, clock);
    let s = session(48, 32, 40);
    saves::quicksave(&store, &s).unwrap();
    let loaded = saves::quickload(&store, &s.defs).unwrap();
    assert_eq!(loaded.tick, 40);
    assert_eq!(loaded.save(&s.defs), s.world.save(&s.defs), "the save format is untouched");
    let text = store.read(saves::QUICKSAVE).unwrap().unwrap();
    assert_eq!(Some(text), s.world.save(&s.defs).ok());
}

#[test]
fn a_save_is_stored_small_enough_for_local_storage() {
    // The default small map (192 x 128) is the one players get first: it must not eat the
    // browser's ~5 MB (5 242 880 character) quota alone, nor a handful of slots.
    let kv = MemKv::new(usize::MAX);
    let store = KvSaves::new(&kv, clock);
    let s = session(192, 128, 0);
    let text = s.world.save(&s.defs).unwrap();
    saves::quicksave(&store, &s).unwrap();
    let stored: usize = kv.items.lock().unwrap().values().map(String::len).sum();
    assert!(stored * 8 < text.len(), "{stored} stored for {} of save text", text.len());
}

#[test]
fn a_full_store_refuses_the_save_and_keeps_the_old_one() {
    let s = session(48, 32, 40);
    let one = browser_saves::encode(&s.world.save(&s.defs).unwrap(), clock()).len();
    // Room for one slot and a little more, not two.
    let kv = MemKv::new(one + one / 2);
    let store = KvSaves::new(&kv, clock);
    saves::save(&store, "first", &s).unwrap();
    let refused = saves::save(&store, "second", &s).unwrap_err();
    assert!(refused.contains("browser storage is full"), "{refused}");
    assert!(refused.contains("Shift+S"), "it says what to do: {refused}");
    let names: Vec<_> = saves::list(&store).unwrap().into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["first"], "nothing half written");

    // Saving over the slot that is there still works, and a failed overwrite leaves it whole.
    let mut later = s.clone();
    later.tick_n(10);
    saves::save(&store, "first", &later).unwrap();
    assert_eq!(saves::load(&store, "first", &s.defs).unwrap().tick, 50);
    let tight = MemKv::new(one - 1);
    let store = KvSaves::new(&tight, clock);
    assert!(saves::save(&store, "first", &s).is_err());
    assert!(saves::list(&store).unwrap().is_empty());
}

#[test]
fn the_list_is_newest_first_and_marks_what_cannot_load() {
    let kv = MemKv::new(usize::MAX);
    let defs = Defs::bundled();
    let s = session(48, 32, 40);
    saves::save(&KvSaves::new(&kv, || 1_000), "old", &s).unwrap();
    saves::save(&KvSaves::new(&kv, || 3_000), "new", &s).unwrap();
    saves::save(&KvSaves::new(&kv, || 2_000), "mid", &s).unwrap();
    {
        let mut items = kv.items.lock().unwrap();
        // Another page's key is not ours, and a slot whose value is damaged is listed, as
        // unloadable, rather than failing the list.
        items.insert("something-else".to_owned(), "x".to_owned());
        items.insert(format!("{}junk", browser_saves::KEY_PREFIX), "not a save".to_owned());
        let damaged = browser_saves::encode(&s.world.save(&defs).unwrap(), 5).replace('\n', "~");
        items.insert(format!("{}cut", browser_saves::KEY_PREFIX), damaged);
    }
    let store = KvSaves::new(&kv, clock);
    let slots = saves::list(&store).unwrap();
    let names: Vec<_> = slots.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["new", "mid", "old", "cut", "junk"]);
    assert!(matches!(slots[0].state, SlotState::Ready(_)));
    assert!(!slots[3].loadable() && !slots[4].loadable());
    assert!(slots[4].describe(&defs).contains("damaged browser storage"));
    assert!(saves::load(&store, "junk", &defs).unwrap_err().contains("damaged"));
}

#[test]
fn missing_and_damaged_slots_are_error_lines() {
    let kv = MemKv::new(usize::MAX);
    let store = KvSaves::new(&kv, clock);
    let defs = Defs::bundled();
    let none = saves::quickload(&store, &defs).unwrap_err();
    assert_eq!(none, "no quicksave yet (browser storage as quicksave)");
    assert!(saves::load(&store, "other", &defs).unwrap_err().contains("no save named other"));
    assert!(saves::load(&store, "../x", &defs).is_err(), "names are checked as on disk");

    let s = session(48, 32, 40);
    let good = browser_saves::encode(&s.world.save(&defs).unwrap(), 1);
    let key = format!("{}quicksave", browser_saves::KEY_PREFIX);
    for bad in [
        good[..good.len() / 2].to_owned(),
        format!("{}!!", good),
        good.replacen("fn-save-1", "fn-save-9", 1),
    ] {
        kv.items.lock().unwrap().insert(key.clone(), bad);
        let why = saves::quickload(&store, &defs).unwrap_err();
        assert!(why.contains("damaged"), "{why}");
    }
}

#[test]
fn the_slot_names_where_it_went() {
    let kv = MemKv::new(usize::MAX);
    let store = KvSaves::new(&kv, clock);
    assert_eq!(store.place("save-1"), "browser storage as save-1");
}

/// Export and import (#976): a slot's exported bytes come back as a slot in an empty store and
/// load; a clash gets a number; a file that isn't a save is refused and adds nothing.
#[test]
fn an_exported_slot_imports_into_an_empty_store_and_loads() {
    let from = MemKv::new(usize::MAX);
    let s = session(48, 32, 40);
    saves::save(&KvSaves::new(&from, clock), "camp", &s).unwrap();
    let file = browser_saves::export_value(&&from, "camp").unwrap();
    assert_eq!(browser_saves::export_file_name("camp"), "camp.fnsave");

    let to = MemKv::new(usize::MAX);
    assert_eq!(browser_saves::import_value(&&to, "camp.fnsave", &file).unwrap(), "camp");
    let store = KvSaves::new(&to, clock);
    let world = saves::load(&store, "camp", &Defs::bundled()).unwrap();
    assert_eq!(world.tick, s.world.tick);
    assert_eq!(
        browser_saves::import_value(&&to, "camp.fnsave", &file).unwrap(),
        "camp-2",
        "a taken name gets a number"
    );
    // A name a slot can't have is made into one.
    assert_eq!(
        browser_saves::import_value(&&to, "my camp (1).fnsave", &file).unwrap(),
        "my-camp--1-"
    );
    assert_eq!(saves::list(&store).unwrap().len(), 3);
}

#[test]
fn a_file_that_is_not_a_save_is_refused() {
    let kv = MemKv::new(usize::MAX);
    let why = browser_saves::import_value(&&kv, "notes.fnsave", "hello").unwrap_err();
    assert!(why.starts_with("notes.fnsave isn't a Four Nations save"), "{why}");
    assert!(!why.contains('\n'), "one line");
    // Stored-value shape but not a save inside.
    let fake = browser_saves::encode("not a save\n", 1);
    assert!(browser_saves::import_value(&&kv, "fake.fnsave", &fake).is_err());
    assert!((&kv).keys().unwrap().is_empty(), "nothing was added");
}

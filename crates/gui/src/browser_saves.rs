//! Saves in the browser (#860): [`fn_ui::saves::SaveStore`] over a string key-value store, which on
//! the web is `localStorage` ([`local`]). The slot logic (value format, compression, quota
//! errors) is here and builds on every target, so its tests run natively against an in-memory
//! store; only the `localStorage` calls themselves are wasm32-only.
//!
//! **Why `localStorage`, and compressed.** It is synchronous, which `SaveStore` is and a
//! save key press needs; IndexedDB is not, and would put an await between F5 and the status
//! line. Its quota is about 5 MB of characters per site, and a RON save is far bigger than that
//! on the larger maps, so a slot is stored deflated and base64'd (the save format itself is
//! unchanged: the text is what `fn_sim::save` wrote, and comes back byte for byte):
//!
//! | map                | save text | stored (deflate + base64) |
//! |--------------------|-----------|---------------------------|
//! | small, 192 x 128   | 2.4 MB    | about 190 KB              |
//! | large, 384 x 256   | 9.5 MB    | about 740 KB              |
//!
//! So about six large saves, or twenty-odd small ones, fit (a character counts once against the
//! quota; Chromium's is 5 Mi of them). A save that doesn't fit is refused
//! with a line saying so ([`SetError::Full`]), and the slot it would have replaced is as it was.
//!
//! **Value**: one key per slot, `four-nations/saves/NAME`, whose value is four lines,
//! `fn-save-1`, the write time in milliseconds since 1970, the save's own header line, and the
//! base64 body. Listing reads the three short lines of each slot and never inflates a world.
//! Because a slot is one `setItem`, a write is all or nothing, as the disk's rename is.

use std::time::{Duration, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use fn_ui::saves::{Entry, SaveStore};

/// Every slot's key starts with this, so other things in the same storage are none of ours.
pub const KEY_PREFIX: &str = "four-nations/saves/";

/// The first line of a stored value: which layout the rest is.
const MAGIC: &str = "fn-save-1";

/// The most a slot may inflate to, a guard against a damaged value, well above any real save.
const INFLATE_LIMIT: usize = 256 << 20;

/// Why a [`Kv::set`] failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetError {
    /// The storage is out of room.
    Full,
    /// Anything else, as a line.
    Other(String),
}

/// A string key-value store with `localStorage`'s shape. `set` must leave the old value alone
/// when it fails.
pub trait Kv: Send + Sync {
    fn keys(&self) -> Result<Vec<String>, String>;
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    fn set(&self, key: &str, value: &str) -> Result<(), SetError>;
}

/// Slots stored in a [`Kv`], compressed. `now` is the clock for "last written" (milliseconds
/// since 1970); the page's, never the game's.
pub struct KvSaves<K> {
    kv: K,
    now: fn() -> u64,
}

impl<K: Kv> KvSaves<K> {
    pub fn new(kv: K, now: fn() -> u64) -> Self {
        KvSaves { kv, now }
    }
}

fn key(name: &str) -> String {
    format!("{KEY_PREFIX}{name}")
}

/// The value that stores `text` written at `millis`.
pub fn encode(text: &str, millis: u64) -> String {
    let header = text.lines().next().unwrap_or("");
    let packed = miniz_oxide::deflate::compress_to_vec_zlib(text.as_bytes(), 6);
    format!("{MAGIC}\n{millis}\n{header}\n{}", STANDARD.encode(packed))
}

/// The write time and header line of a stored value, without inflating it.
pub fn decode_header(value: &str) -> Result<(u64, &str), String> {
    let mut lines = value.splitn(4, '\n');
    let damaged = || "not a Four Nations save (damaged browser storage)".to_owned();
    if lines.next() != Some(MAGIC) {
        return Err(damaged());
    }
    let millis = lines.next().and_then(|m| m.parse().ok()).ok_or_else(damaged)?;
    let header = lines.next().ok_or_else(damaged)?;
    Ok((millis, header))
}

/// The save text a stored value holds.
pub fn decode(value: &str) -> Result<String, String> {
    decode_header(value)?;
    let body = value.splitn(4, '\n').nth(3).unwrap_or("");
    let packed = STANDARD
        .decode(body)
        .map_err(|e| format!("damaged save in browser storage (not base64: {e})"))?;
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&packed, INFLATE_LIMIT)
        .map_err(|e| format!("damaged save in browser storage ({e:?})"))?;
    String::from_utf8(bytes).map_err(|_| "damaged save in browser storage (not text)".to_owned())
}

impl<K: Kv> SaveStore for KvSaves<K> {
    fn entries(&self) -> Result<Vec<Entry>, String> {
        let mut entries = Vec::new();
        for full in self.kv.keys()? {
            let Some(name) = full.strip_prefix(KEY_PREFIX) else { continue };
            let (header, modified) = match self.kv.get(&full) {
                Ok(Some(value)) => match decode_header(&value) {
                    Ok((millis, header)) => {
                        (Ok(header.to_owned()), Some(UNIX_EPOCH + Duration::from_millis(millis)))
                    }
                    Err(why) => (Err(why), None),
                },
                // Gone between the key listing and now.
                Ok(None) => continue,
                Err(why) => (Err(why), None),
            };
            entries.push(Entry { name: name.to_owned(), header, modified });
        }
        Ok(entries)
    }

    fn read(&self, name: &str) -> Result<Option<String>, String> {
        self.kv.get(&key(name))?.map(|value| decode(&value)).transpose()
    }

    fn write(&self, name: &str, text: &str) -> Result<(), String> {
        let value = encode(text, (self.now)());
        self.kv.set(&key(name), &value).map_err(|e| match e {
            SetError::Full => format!(
                "browser storage is full (this save needs about {} KB, and the browser gives \
                 this site about 5 MB in all): save over an older slot (Shift+S in the saves \
                 screen) or clear this site's data",
                value.len().div_ceil(1000)
            ),
            SetError::Other(why) => format!("could not write browser storage: {why}"),
        })
    }

    fn place(&self, name: &str) -> String {
        format!("browser storage as {name}")
    }
}

/// The extension of an exported save file (#976). Its bytes are the slot's stored value.
pub const FILE_EXTENSION: &str = "fnsave";

/// The file name slot `name` is exported as.
pub fn export_file_name(name: &str) -> String {
    format!("{name}.{FILE_EXTENSION}")
}

/// The stored value of slot `name`, for export: exactly what `localStorage` holds.
pub fn export_value(kv: &impl Kv, name: &str) -> Result<String, String> {
    kv.get(&key(name))?.ok_or_else(|| format!("no save named {name}"))
}

/// Adds an exported file (`file_name`, its text `value`) as a slot (#976): named for the file,
/// with `-2`, `-3` and on added if that is taken. Returns the slot's name. A file that isn't a
/// save this build can read is refused with a line saying so, and nothing is added.
pub fn import_value(kv: &impl Kv, file_name: &str, value: &str) -> Result<String, String> {
    let refused = |why: String| format!("{file_name} isn't a Four Nations save ({why})");
    let value = value.trim_end_matches(['\r', '\n']);
    let text = decode(value).map_err(|_| refused("not an exported save file".to_owned()))?;
    fn_sim::save::inspect(&text).map_err(|e| refused(e.to_string()))?;
    let stem = file_name.rsplit_once('.').map_or(file_name, |(stem, _)| stem);
    let mut base: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .take(fn_ui::saves::MAX_NAME - 4)
        .collect();
    if base.trim_matches('-').is_empty() {
        base = "imported".to_owned();
    }
    let keys = kv.keys()?;
    let taken = |name: &str| keys.iter().any(|k| *k == key(name));
    let name = if taken(&base) {
        (2..).map(|n| format!("{base}-{n}")).find(|n| !taken(n)).expect("an unused name")
    } else {
        base
    };
    kv.set(&key(&name), value).map_err(|e| match e {
        SetError::Full => {
            "browser storage is full: delete or save over an older slot first".to_owned()
        }
        SetError::Other(why) => format!("could not write browser storage: {why}"),
    })?;
    Ok(name)
}

/// The page's clock in whole milliseconds since 1970, for [`KvSaves::new`]. `web_time` reads the
/// page's `Date.now()` on the web, where std's `SystemTime::now` panics.
pub fn now_millis() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// `localStorage` itself, on the web.
#[cfg(target_arch = "wasm32")]
pub mod local {
    use eframe::wasm_bindgen::JsCast;
    use fn_ui::saves::Store;
    use std::sync::Arc;

    use super::{Kv, KvSaves, SetError, now_millis};

    /// The page's `localStorage`, looked up on each call (it can be missing: blocked site data,
    /// some private modes, and then every call says so as an error line).
    pub struct LocalStorage;

    fn storage() -> Result<web_sys::Storage, String> {
        const OFF: &str = "browser storage is unavailable (is site data blocked?)";
        web_sys::window()
            .ok_or(OFF)?
            .local_storage()
            .map_err(|_| OFF.to_owned())?
            .ok_or_else(|| OFF.to_owned())
    }

    fn describe(e: &eframe::wasm_bindgen::JsValue) -> String {
        match e.dyn_ref::<web_sys::DomException>() {
            Some(d) => format!("{}: {}", d.name(), d.message()),
            None => format!("{e:?}"),
        }
    }

    /// Browsers disagree on the error's name: `QuotaExceededError` (Chrome, Safari, Firefox now),
    /// `NS_ERROR_DOM_QUOTA_REACHED` (older Firefox), or just code 22 or 1014.
    fn is_quota(e: &eframe::wasm_bindgen::JsValue) -> bool {
        e.dyn_ref::<web_sys::DomException>()
            .is_some_and(|d| d.name().contains("Quota") || matches!(d.code(), 22 | 1014))
    }

    impl Kv for LocalStorage {
        fn keys(&self) -> Result<Vec<String>, String> {
            let s = storage()?;
            let n = s.length().map_err(|e| describe(&e))?;
            Ok((0..n).filter_map(|i| s.key(i).ok().flatten()).collect())
        }

        fn get(&self, key: &str) -> Result<Option<String>, String> {
            storage()?.get_item(key).map_err(|e| describe(&e))
        }

        fn set(&self, key: &str, value: &str) -> Result<(), SetError> {
            storage().map_err(SetError::Other)?.set_item(key, value).map_err(|e| {
                if is_quota(&e) { SetError::Full } else { SetError::Other(describe(&e)) }
            })
        }
    }

    /// The store the web build keeps its saves in.
    pub fn store() -> Store {
        Arc::new(KvSaves::new(LocalStorage, now_millis))
    }
}

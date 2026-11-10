//! The on-disk cache shared by everything that saves itself repeated work between runs: a
//! scenario prelude's world (`crate::surface::prelude`) and `fn_agent`'s par (`par.rs`).
//!
//! The folder is `$FN_AGENT_CACHE`, else `$XDG_CACHE_HOME/four-nations`, else
//! `$HOME/.cache/four-nations`. Everything in it is filed under the build that wrote it
//! (`crate::buildid`), so a changed sim or changed data never reads what another build left.
//! Reads and writes that fail are skipped, never errors: the cache is only a saving. Writes go
//! through a temporary file and a rename, so parallel readers and writers see a whole file or
//! none.

use std::path::{Path, PathBuf};

/// The cache folder (module doc), or `None` when neither `$FN_AGENT_CACHE` nor a home is known,
/// and always on wasm, which has no files. `xtask`'s `references::cache_root` resolves the same
/// folder the same way; keep the two together.
pub fn root() -> Option<PathBuf> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    let from = |var: &str| std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from);
    from("FN_AGENT_CACHE")
        .or_else(|| from("XDG_CACHE_HOME").map(|p| p.join("four-nations")))
        .or_else(|| from("HOME").map(|p| p.join(".cache").join("four-nations")))
}

/// Writes `text` to `path` through a temporary file and a rename, creating the folder. Does
/// nothing if any step fails.
pub fn write(path: &Path, text: &str) {
    let Some(dir) = path.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let tmp = dir.join(format!(".{}.{name}.tmp", std::process::id()));
    if std::fs::write(&tmp, text).is_err() || std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// FNV-1a, as `Defs::id_hash` and `build.rs` use: stable across platforms and Rust versions.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// `name` with anything but ASCII letters, digits and `-` turned into `_`, for a file or folder
/// name (a dirty build id has a `+`).
pub fn safe(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_lands_whole_and_leaves_no_temporary_file() {
        let dir = std::env::temp_dir().join(format!("fn_tui_cache_{}", std::process::id()));
        let path = dir.join("a").join("b.txt");
        write(&path, "hello");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
        let left: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
        assert_eq!(left.len(), 1, "only the file itself");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn names_are_made_safe() {
        assert_eq!(safe("abc1234+deadbeef"), "abc1234_deadbeef");
    }
}

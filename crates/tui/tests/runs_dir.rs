//! The runs folder (#846): `fn_runfmt::runs_dir` puts runs beside the saves, by the same platform
//! rule as `fn_ui::saves::save_dir`. The two crates can't share the code (`xtask` depends on
//! `fn_runfmt` but not `fn_ui`), so this checks they agree, and that runs left in the old
//! `~/fn-agent-runs` stay in use on a real filesystem until they are moved.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use fn_runfmt::runs_dir::{RunsDir, data_home_runs};
use fn_ui::saves::save_dir;

fn lookup(pairs: Vec<(&'static str, OsString)>) -> impl Fn(&str) -> Option<OsString> {
    move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone())
}

#[test]
fn runs_sit_beside_the_saves_on_every_platform() {
    let cases: [(&str, Vec<(&str, &str)>); 4] = [
        ("linux", vec![("XDG_DATA_HOME", "/d"), ("HOME", "/h")]),
        ("linux", vec![("HOME", "/h")]),
        ("macos", vec![("HOME", "/h")]),
        ("windows", vec![("APPDATA", "C:/a"), ("HOME", "/h")]),
    ];
    for (os, pairs) in cases {
        let env = || lookup(pairs.iter().map(|(k, v)| (*k, OsString::from(v))).collect());
        let runs = data_home_runs(env(), os).unwrap();
        let saves = save_dir(env(), os).unwrap();
        assert_eq!(runs.parent(), saves.parent(), "{os} {pairs:?}");
        assert_eq!(runs.file_name().unwrap(), "runs");
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fn_tui_runs_dir_{}_{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn resolve(home: &Path) -> RunsDir {
    let env = lookup(vec![("HOME", home.as_os_str().to_owned())]);
    RunsDir::resolve(env, "linux", Path::exists, |p| {
        fs::read_dir(p).is_ok_and(|mut d| d.next().is_some())
    })
    .unwrap()
}

#[test]
fn old_runs_stay_in_use_until_they_are_moved() {
    let home = scratch("legacy");
    let old = home.join("fn-agent-runs");
    let new = home.join(".local/share/four-nations/runs");

    // No runs anywhere: the new folder, with nothing to say.
    assert_eq!(resolve(&home), RunsDir { path: new.clone(), moved_to: None });

    // Runs in the old folder and no new one: the old folder, and a note on moving them.
    fs::create_dir_all(old.join("20261101-120000-water-7-normal")).unwrap();
    let kept = resolve(&home);
    assert_eq!(kept.path, old);
    assert_eq!(kept.moved_to.as_ref(), Some(&new));
    assert!(kept.note().unwrap().contains(&old.display().to_string()));

    // Moved as the note says: the new folder, and the runs are there.
    fs::create_dir_all(new.parent().unwrap()).unwrap();
    fs::rename(&old, &new).unwrap();
    let moved = resolve(&home);
    assert_eq!(moved, RunsDir { path: new.clone(), moved_to: None });
    assert!(new.join("20261101-120000-water-7-normal").is_dir());

    let _ = fs::remove_dir_all(&home);
}

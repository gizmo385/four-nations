//! The build id in a run log (#566): `fn_agent score` warns when a log was made by another build,
//! the `score` line it appends records its own, and a header or `score` line from before builds
//! were recorded still reads. The tests use a short real log; the campaign side is covered with
//! stubs in `xtask/tests/agent_campaign.rs`.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use fn_tui::buildid::CURRENT;
use fn_tui::surface::{Host, LogLine, RunLogHeader};
use fn_tui::watch;
use serde_json::Value;

fn fn_agent() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fn_agent"));
    cmd.stdin(Stdio::null());
    cmd
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fn_agent_build_id_test_{}_{name}", std::process::id()))
}

/// A header and lines of a live host after two turns.
fn played() -> (RunLogHeader, Vec<LogLine>) {
    let nation = fn_tui::cli::parse_nation("earth").unwrap();
    let mut host = Host::new(7, nation, None, None, None, false).unwrap();
    host.end_turn(2);
    (host.header(), host.log().lines())
}

/// Writes `header` and `lines` as a run log.
fn write_log(name: &str, header: RunLogHeader, lines: &[LogLine]) -> PathBuf {
    let log = temp_path(name);
    let mut text = serde_json::to_string(&LogLine::Header(header)).unwrap();
    for line in lines {
        text.push('\n');
        text.push_str(&serde_json::to_string(line).unwrap());
    }
    text.push('\n');
    std::fs::write(&log, text).unwrap();
    log
}

/// Scoring a log another build wrote still works, with a warning on stderr that says the score
/// may not reproduce and names the build. A log of this build is silent, a log from before
/// builds were recorded gets the warning too, and `--json` stdout stays parseable.
#[test]
fn scoring_a_log_from_another_build_warns_and_still_scores() {
    let (header, lines) = played();
    assert_eq!(header.build.as_deref(), Some(CURRENT), "a live host records this build");

    let own = write_log("own.jsonl", header.clone(), &lines);
    let out = fn_agent().arg("score").arg(&own).output().unwrap();
    std::fs::remove_file(&own).ok();
    assert!(out.status.success());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("warning"), "the same build is quiet");

    let other = RunLogHeader { build: Some("0badc0d+deadbeef".into()), ..header.clone() };
    let log = write_log("other.jsonl", other, &lines);
    let out = fn_agent().arg("score").arg(&log).arg("--json").output().unwrap();
    std::fs::remove_file(&log).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("warning") && err.contains("0badc0d+deadbeef"), "{err}");
    assert!(err.contains("may not reproduce") && err.contains(CURRENT), "{err}");
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["log_build"], "0badc0d+deadbeef");
    assert_eq!(json["build"], CURRENT);

    let old = RunLogHeader { build: None, ..header };
    let log = write_log("old.jsonl", old, &lines);
    let out = fn_agent().arg("score").arg(&log).output().unwrap();
    std::fs::remove_file(&log).ok();
    assert!(out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("an unrecorded build") && err.contains("may not reproduce"), "{err}");
}

/// `--append` writes the build that computed the score, whichever build the log is from.
#[test]
fn the_appended_score_line_records_the_build_that_computed_it() {
    let (header, lines) = played();
    let other = RunLogHeader { build: Some("0badc0d".into()), ..header };
    let log = write_log("append.jsonl", other, &lines);
    let out = fn_agent().arg("score").arg(&log).arg("--append").output().unwrap();
    assert!(out.status.success());
    let (_, after) = watch::read_file(&log).unwrap();
    std::fs::remove_file(&log).ok();
    let Some(LogLine::Score { build, .. }) = after.last() else { panic!("{:?}", after.last()) };
    assert_eq!(build.as_deref(), Some(CURRENT));
}

/// A header and a `score` line written before #566 have no `build`; they read back as `None`
/// and are written back without one. A new one writes it.
#[test]
fn the_build_is_optional_in_the_header_and_the_score_line() {
    let (header, _) = played();
    let new_header = serde_json::to_string(&LogLine::Header(header.clone())).unwrap();
    assert!(new_header.contains(&format!("\"build\":\"{CURRENT}\"")));
    let mut old_header: Value = serde_json::from_str(&new_header).unwrap();
    old_header.as_object_mut().unwrap().remove("build");
    let LogLine::Header(old) = serde_json::from_value(old_header).unwrap() else { panic!() };
    assert_eq!(old, RunLogHeader { build: None, ..header });
    assert!(!serde_json::to_string(&LogLine::Header(old)).unwrap().contains("build"));

    let old_score = r#"{"type":"score","score":355.0,"progress":0.5,"care":0.997,"horizon":540}"#;
    let Ok(LogLine::Score { build, .. }) = serde_json::from_str(old_score) else { panic!() };
    assert_eq!(build, None);
}

#[test]
fn build_id_prints_this_binarys_build() {
    let out = fn_agent().arg("build-id").output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), CURRENT);
}

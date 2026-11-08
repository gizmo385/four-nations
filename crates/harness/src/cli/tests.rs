use std::process::ExitCode;

use super::*;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn verdict_flags_parse() {
    let a = parse_verdict_args(&args(&[
        "--seeds",
        "1-8",
        "--styles",
        "steady,idle",
        "--presets",
        "normal",
        "--nations",
        "earth",
        "--threads",
        "2",
        "--label",
        "x",
        "--out",
        "/tmp/o",
        "--minutes",
        "30",
    ]))
    .unwrap();
    assert_eq!(a.seeds.as_deref(), Some("1-8"));
    assert_eq!(a.styles, Some(args(&["steady", "idle"])));
    assert_eq!(a.presets, Some(args(&["normal"])));
    assert_eq!(a.nations, Some(args(&["earth"])));
    assert_eq!((a.threads, a.minutes), (Some(2), Some(30.0)));
    assert_eq!((a.label.as_deref(), a.out), (Some("x"), Some(PathBuf::from("/tmp/o"))));
    assert_eq!(parse_verdict_args(&[]).unwrap(), VerdictArgs::default());
}

#[test]
fn verdict_flag_errors_are_named() {
    let err = |list: &[&str]| parse_verdict_args(&args(list)).unwrap_err();
    assert!(err(&["--seeds"]).contains("needs a value"));
    assert!(err(&["--sedes", "1"]).contains("unknown argument"));
    assert!(err(&["--threads", "many"]).contains("whole number"));
    assert!(err(&["--minutes", "soon"]).contains("number"));
}

#[test]
fn bad_values_are_errors_before_any_run() {
    let err = |list: &[&str]| run_verdict(&args(list)).unwrap_err();
    assert!(err(&["--seeds", "1-"]).contains("seed list"));
    assert!(err(&["--nations", "mud"]).contains("unknown nation"));
    assert!(err(&["--styles", "loud"]).contains("unknown style"));
    assert!(err(&["--presets", "easy"]).contains("unknown preset"));
    assert!(err(&["--threads", "0"]).contains("at least 1"));
    assert!(err(&["--styles", "branch_first", "--presets", "sandbox"]).contains("nothing to run"));
}

#[test]
fn help_lists_every_command_and_unknown_ones_fail() {
    let text = help();
    for (name, _, _) in COMMANDS {
        assert!(text.contains(name), "{name}");
    }
    assert_eq!(main(args(&["--help"])), ExitCode::SUCCESS);
    assert_eq!(main(args(&["verdict", "--help"])), ExitCode::SUCCESS);
    assert_eq!(main(args(&["tracee"])), ExitCode::from(2));
    assert_eq!(main(Vec::new()), ExitCode::from(2));
    assert_eq!(main(args(&["verdict", "--nope"])), ExitCode::from(2));
}

#[test]
fn the_old_flag_spellings_still_reach_their_commands() {
    // `supply` is cheap, so it can run for real: one seed, one radius.
    assert_eq!(main(args(&["supply", "--seeds", "1", "--radii", "20"])), ExitCode::SUCCESS);
    assert_eq!(main(args(&["--supply", "--seeds", "1", "--radii", "20"])), ExitCode::SUCCESS);
    for (flag, name) in LEGACY_FLAGS {
        assert!(COMMANDS.iter().any(|(n, _, _)| n == name), "{flag} -> {name}");
    }
}

#[test]
fn a_short_panel_runs_end_to_end_and_writes_its_records() {
    let dir = std::env::temp_dir().join(format!("fn_harness_cli_{}", std::process::id()));
    let out = dir.to_string_lossy().into_owned();
    let run = || {
        main(args(&[
            "verdict",
            "--seeds",
            "1-2",
            "--nations",
            "earth,fire",
            "--styles",
            "steady",
            "--presets",
            "normal",
            "--minutes",
            "3",
            "--threads",
            "2",
            "--out",
            &out,
            "--label",
            "smoke",
        ]))
    };
    // Three-minute runs read none of the targets: no data, which is not a failure.
    assert_eq!(run(), ExitCode::SUCCESS);
    let file = dir.join("smoke.json");
    let first = std::fs::read_to_string(&file).expect("records written");
    assert_eq!(first.lines().count(), 2 * 2 + 2, "four runs and the brackets");
    assert_eq!(run(), ExitCode::SUCCESS);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), first, "same panel, same bytes");
    std::fs::remove_dir_all(&dir).unwrap();
}

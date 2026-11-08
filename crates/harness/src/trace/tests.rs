use super::*;
use crate::report::trace_command;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

/// The arguments of a printed `balance trace` command.
fn words(cmd: &str) -> Vec<String> {
    let rest = cmd.strip_prefix("balance trace ").expect("a trace command");
    rest.split_whitespace().map(str::to_owned).collect()
}

#[test]
fn flags_parse_and_default_to_the_reference_run() {
    let a = parse_args(&args(&["--seed", "3", "--nation", "water", "--from", "97", "--to", "145"]))
        .unwrap();
    assert_eq!(
        a,
        TraceArgs {
            seed: 3,
            nation: Nation::Water,
            style: "steady".to_owned(),
            preset: "normal".to_owned(),
            from: 97,
            to: 145,
            map_at: None,
            radius: DEFAULT_RADIUS,
            help: false,
        }
    );
    let a = parse_args(&args(&[
        "--seed", "9", "--nation", "earth", "--style", "careless", "--preset", "hard", "--from",
        "1", "--to", "2", "--map-at", "2", "--radius", "8",
    ]))
    .unwrap();
    assert_eq!((a.style.as_str(), a.preset.as_str()), ("careless", "hard"));
    assert_eq!((a.map_at, a.radius), (Some(2), 8));
}

#[test]
fn every_command_the_verdict_prints_parses() {
    // The report prints these (`report::trace_command`); the trace has to take them as printed.
    for (style, preset, nation, seed, window, map_at) in [
        ("steady", "normal", Nation::Water, 3, (97, 145), 140),
        ("steady", "normal", Nation::Fire, 4, (38, 50), 45),
        ("careless", "hard", Nation::Earth, 9, (1, 2), 2),
        ("idle", "normal", Nation::Air, 12, (0, 30), 0),
    ] {
        let cmd = trace_command(style, preset, nation, seed, window, map_at);
        let a = parse_args(&words(&cmd)).unwrap_or_else(|e| panic!("{cmd}: {e}"));
        assert_eq!((a.seed, a.nation, a.from, a.to), (seed, nation, window.0, window.1), "{cmd}");
        assert_eq!((a.style.as_str(), a.preset.as_str()), (style, preset), "{cmd}");
        assert_eq!(a.map_at, Some(map_at), "{cmd}");
    }
}

#[test]
fn bad_arguments_are_named() {
    let err = |list: &[&str]| parse_args(&args(list)).unwrap_err();
    assert!(err(&["--nation", "water", "--from", "1", "--to", "2"]).contains("--seed is required"));
    assert!(err(&["--seed", "1", "--from", "1", "--to", "2"]).contains("--nation is required"));
    assert!(
        err(&["--seed", "1", "--nation", "mud", "--from", "1", "--to", "2"]).contains("nation")
    );
    assert!(err(&["--seed", "1", "--nation", "fire", "--to", "2"]).contains("--from is required"));
    assert!(
        err(&["--seed", "1", "--nation", "fire", "--from", "9", "--to", "2"])
            .contains("after --to")
    );
    assert!(err(&["--seed", "x"]).contains("whole number"));
    assert!(err(&["--seed"]).contains("needs a value"));
    assert!(err(&["--sead", "1"]).contains("unknown argument"));
    let run_err = |list: &[&str]| run(&args(list)).unwrap_err();
    let base = ["--seed", "1", "--nation", "fire", "--from", "0", "--to", "1"];
    let with = |extra: &[&'static str]| [&base[..], extra].concat();
    assert!(run_err(&with(&["--style", "loud"])).contains("unknown style"));
    assert!(run_err(&with(&["--preset", "easy"])).contains("unknown preset"));
}

#[test]
fn help_is_not_an_error() {
    assert!(run(&args(&["--help"])).unwrap().starts_with("usage: balance trace"));
}

#[test]
fn a_short_run_prints_one_row_per_minute_and_a_marked_crop() {
    let out = run(&args(&[
        "--seed", "3", "--nation", "water", "--from", "2", "--to", "5", "--map-at", "4",
        "--radius", "8",
    ]))
    .unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines[0], "balance trace: seed 3 water style steady preset normal, minutes 2-5",
        "{out}"
    );

    // The header, then minutes 2 to 5, one row each.
    let header = lines.iter().position(|l| l.starts_with(" min pop idle")).expect(&out);
    for col in ["idle by reason", "stores", "lowest need"] {
        assert!(lines[header].contains(col), "{}", lines[header]);
    }
    let rows = &lines[header + 1..header + 5];
    for (row, minute) in rows.iter().zip(2..=5) {
        let cells: Vec<&str> = row.split_whitespace().collect();
        assert_eq!(cells[0], minute.to_string(), "{row}");
        assert_eq!(cells[1], "6", "six settlers: {row}");
        assert!(cells[2].parse::<u32>().is_ok(), "idle count: {row}");
        // Each of the six settlers has a mark and a lowest need: "1:thi42".
        for mark in '1'..='6' {
            let pat = format!("{mark}:");
            let cell =
                cells.iter().find(|c| c.starts_with(&pat)).unwrap_or_else(|| panic!("{row}"));
            let (name, pct) = cell[2..].split_at(3);
            assert!(["hun", "thi", "ene", "hea"].contains(&name), "{row}");
            assert!(pct.parse::<u32>().is_ok_and(|p| p <= 100), "{row}");
        }
    }
    assert!(lines[header + 5].is_empty(), "no row for minute 6: {out}");

    // The crop: 17 rows of 17 tiles (radius 8), a ruler, every settler, and a legend.
    let at = lines.iter().position(|l| *l == "map at minute 4").expect(&out);
    assert!(lines[at + 2].starts_with("x "), "{}", lines[at + 2]);
    let grid: Vec<&str> = lines[at + 4..].iter().take_while(|l| !l.is_empty()).copied().collect();
    assert_eq!(grid.len(), 17, "{out}");
    assert!(grid.iter().all(|r| r.chars().count() == 4 + 17), "{out}");
    let drawn: String = grid.iter().flat_map(|r| r.chars().skip(4)).collect();
    // Settlers wander off a small crop and share tiles; the ones in view carry their marks, and
    // the legend lists all six wherever they are.
    assert!(drawn.chars().any(|c| ('1'..='6').contains(&c)), "a settler is in view: {out}");
    for mark in '1'..='6' {
        assert!(out.contains(&format!("\n  {mark}  ")), "settler {mark} is listed: {out}");
    }
    assert!(drawn.contains('P'), "the stockpile is marked: {out}");
    for needle in [
        "legend",
        "stockpile",
        "building that blocks walking",
        "resources (each blocks",
        "settlers\n",
    ] {
        assert!(out.contains(needle), "{needle}: {out}");
    }
}

#[test]
fn a_map_minute_after_the_window_runs_on_to_reach_it() {
    // A map minute after the window is still reached: the run goes on to it.
    let out = run(&args(&[
        "--seed", "3", "--nation", "water", "--from", "1", "--to", "2", "--map-at", "3",
        "--radius", "4",
    ]))
    .unwrap();
    assert!(out.contains("map at minute 3"), "{out}");
    assert!(!out.contains("\n   3   6 "), "minute 3 is past the window, so it has no row: {out}");
}

#[test]
fn watching_does_not_change_the_run() {
    use fn_sim::balance::run_record;
    let defs = Defs::bundled();
    let spec = RunSpec::new(3, Nation::Water, 8.5, &defs);
    let mut minutes = Vec::new();
    let watched = run_observed(&spec, &defs, &mut |m| minutes.push(m.sample.minute));
    assert_eq!(minutes, (0..=8).collect::<Vec<u32>>());
    assert_eq!(watched, run_record(&spec, &defs));
}

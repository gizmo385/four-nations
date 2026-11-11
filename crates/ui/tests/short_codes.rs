//! #663: player-facing text spells things out. No readout built in `fn_ui` carries a short code
//! like "5/8b" (a count with a one-letter unit glued on) or "13/head".
//!
//! Two scans. One reads the text the view builders produce for a few real games, so a code made
//! from a format string is caught too. The other reads the non-test source for literal strings,
//! which catches a code in a branch the games above never reach.

use fn_sim::{Nation, Pos};
use fn_ui::alerts::Alerts;
use fn_ui::tools::Tool;
use fn_ui::{Session, build, endeavor, inspect, text, view};

/// Whether `s` holds `\d+/\d+[a-z]\b` ("5/8b", "0/20s") or `/head`.
fn short_code_in(s: &str) -> Option<String> {
    if let Some(at) = s.find("/head") {
        return Some(s[at.saturating_sub(6)..(at + 5).min(s.len())].to_owned());
    }
    let b = s.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    for (i, &c) in b.iter().enumerate() {
        if c != b'/' || i == 0 || !b[i - 1].is_ascii_digit() {
            continue;
        }
        let mut j = i + 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > i + 1
            && j < b.len()
            && b[j].is_ascii_lowercase()
            && (j + 1 == b.len() || !word(b[j + 1]))
        {
            return Some(s[i.saturating_sub(4)..(j + 1).min(s.len())].to_owned());
        }
    }
    None
}

#[test]
fn the_matcher_finds_codes_and_leaves_plain_counts_alone() {
    assert!(short_code_in("Closest: Bai 5/8b 1/3f").is_some());
    assert!(short_code_in("Work: 0/20s").is_some());
    assert!(short_code_in("Food 159 · 13/head").is_some());
    assert!(short_code_in("Materials: 0/5 Wood").is_none());
    assert!(short_code_in("Bends 9/10").is_none());
    assert!(short_code_in("Food 159 · 13 each").is_none());
    assert!(short_code_in("9/10bc").is_none(), "a longer word is not a one-letter unit");
}

fn check(what: &str, debug: &impl std::fmt::Debug) {
    let rendered = format!("{debug:?}");
    if let Some(code) = short_code_in(&rendered) {
        panic!("{what} has a short code: {code:?}");
    }
}

#[test]
fn no_view_text_carries_a_short_code() {
    for nation in [Nation::Fire, Nation::Earth, Nation::Water, Nation::Air] {
        let mut s = Session::new(7, nation, 64, 64);
        s.world.set_difficulty(Some("normal".to_owned()));
        s.tick_n(300);

        check("the endeavor screen", &endeavor::screen_view(&s));
        check("the endeavor status", &endeavor::endeavor_status(&s));
        check("the season view", &view::season_view(&s, 4));
        check("the nation screen", &view::nation_view(&s));
        for unit in s.world.units.iter() {
            check("a unit's view", &view::unit_view(&s, unit));
        }
        for tool in [Tool::Select, Tool::Build, Tool::Harvest] {
            let side = view::side_view(&s, tool, "", None, &Alerts::default());
            check("the side view", &side);
        }
        for y in 0..64 {
            for x in 0..64 {
                let at = Pos::new(x, y);
                check("a tooltip", &inspect::tooltip_lines(&s, Tool::Select, at));
                check("a tile's lines", &inspect::tile_lines(&s, Tool::Harvest, at));
            }
        }
        for row in build::picker_rows(&s) {
            check("a build picker row", &row);
            check("a building's detail", &build::building_detail(&s, row.id));
        }
    }
}

#[test]
fn no_mastery_text_carries_a_short_code_or_a_count_past_its_target() {
    use fn_sim::bending::Rank;
    use fn_sim::world::MasteryProgress;
    for bends in 0..14 {
        for forms in 0..5 {
            let progress = MasteryProgress {
                rank: Rank::Novice,
                bends,
                forms,
                next: Some((Rank::Adept, 5, 2)),
            };
            let line = text::mastery_line(progress, 0.8);
            check("a mastery line", &line);
            for (have, total) in [(bends, 5), (forms, 2)] {
                if have >= total {
                    assert!(line.contains("done"), "{line}");
                }
            }
            assert!(!line.contains("of 5 bends") || bends < 5, "{line}");
            assert!(!line.contains("of 2 forms") || forms < 2, "{line}");
            let cells = text::mastery_cells(progress, 10, 3);
            check("the mastery cells", &cells);
        }
    }
}

/// Every `.rs` file under `dir`, with the part from its first `#[cfg(test)]` on cut off, and
/// comment lines dropped: what a player's strings can come from.
fn player_source(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            player_source(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let source = std::fs::read_to_string(&path).unwrap();
            let code = source.split("#[cfg(test)]").next().unwrap_or_default();
            let code: Vec<&str> =
                code.lines().filter(|l| !l.trim_start().starts_with("//")).collect();
            out.push((path.display().to_string(), code.join("\n")));
        }
    }
}

#[test]
fn no_source_string_carries_a_short_code() {
    let mut files = Vec::new();
    player_source(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    assert!(!files.is_empty());
    for (path, code) in files {
        if let Some(hit) = short_code_in(&code) {
            panic!("{path} has a short code in a string: {hit:?}");
        }
    }
}

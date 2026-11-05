//! Golden snapshot of embark site reports (`docs/design/embark.md`, issue #288).
//!
//! If you changed the report on purpose, regenerate with `FN_BLESS=1 cargo test -p fn_sim` and
//! review the diff.

use fn_sim::embark::{LandKind, Level, Survey, SurveyMode};
use fn_sim::{Defs, Pos};

const SEED: u64 = 7;

fn level_name(level: Level) -> &'static str {
    match level {
        Level::None => "none",
        Level::Scarce => "scarce",
        Level::Some => "some",
        Level::Plenty => "plenty",
    }
}

fn format_report(
    defs: &Defs,
    survey: &Survey,
    label: &str,
    at: Pos,
    preset: Option<&str>,
) -> String {
    let r = survey.report(at, SurveyMode::Exact, preset, defs);
    let mut out = format!("=== {label}: ({}, {}) ===\n", at.x, at.y);
    let Some(problem) = r.problem else {
        let biomes: Vec<String> = r
            .biomes
            .iter()
            .map(|(k, pct)| {
                let name = match k {
                    LandKind::Biome(id) => defs.biome(*id).id.clone(),
                    LandKind::Mountain => "mountain".into(),
                    LandKind::Water => "water".into(),
                };
                format!("{name} {pct}%")
            })
            .collect();
        out += "problem: none\n";
        out += &format!("biomes: {}\n", biomes.join(", "));
        out += &format!("room: {} tiles, mainland: {}\n", r.room, r.mainland);
        match r.water {
            Some((t, steps)) => out += &format!("water: {t:?}, {steps} steps\n"),
            None => out += "water: none within the cap\n",
        }
        for g in &r.groups {
            if g.hidden {
                out += &format!("{}: ?\n", g.id);
            } else {
                let level = level_name(g.level);
                match g.count {
                    Some(c) => out += &format!("{}: {c} ({level})\n", g.id),
                    None => out += &format!("{}: ({level})\n", g.id),
                }
            }
        }
        if preset.is_some() {
            out += &format!(
                "climate: {} (frost: {}, cold: {})\n",
                r.climate.name, r.climate.frost, r.climate.cold
            );
        }
        out += &format!("outlook: {:?}\n", r.outlook);
        out += &format!("reasons: {}\n", r.reasons.join("; "));
        out += &format!("rough: {}\n", r.rough);
        return out;
    };
    out += &format!("problem: {problem:?}\n");
    out
}

#[test]
fn golden_seed_7_three_sites() {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    let cfg = Survey::config(SEED, size);
    let survey = Survey::new(&cfg, &defs);

    let default_site = survey.default_site(&defs);
    // Chosen so its box holds the buried gold seam at (103, 89): the surfaced gold vein there
    // must be the only one counted (see `report_never_shows_a_buried_resource_in_its_counts` in
    // `embark.rs` for the fixed count this depends on).
    let near_seam = Pos::new(98, 79);
    // The map's forced-sea border: always water, so always refused.
    let refused = Pos::new(0, 0);

    let mut actual = String::new();
    actual += &format_report(&defs, &survey, "default site", default_site, None);
    actual += &format_report(&defs, &survey, "near a buried gold seam", near_seam, None);
    actual += &format_report(&defs, &survey, "open water", refused, None);

    let path = format!("{}/tests/golden/embark_seed7.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, &actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "embark report output changed; see module docs");
}

/// The same three sites under the Normal preset (seasons on, #297): a golden showing the
/// climate line and, wherever the winter term pushes the outlook, the "cold winters" reason.
#[test]
fn golden_seed_7_three_sites_with_seasons() {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    let cfg = Survey::config(SEED, size);
    let survey = Survey::new(&cfg, &defs);

    let default_site = survey.default_site(&defs);
    let near_seam = Pos::new(98, 79);
    let refused = Pos::new(0, 0);

    let mut actual = String::new();
    actual += &format_report(&defs, &survey, "default site", default_site, Some("normal"));
    actual += &format_report(&defs, &survey, "near a buried gold seam", near_seam, Some("normal"));
    actual += &format_report(&defs, &survey, "open water", refused, Some("normal"));

    let path = format!("{}/tests/golden/embark_seed7_seasons.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, &actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "embark report output changed; see module docs");
}

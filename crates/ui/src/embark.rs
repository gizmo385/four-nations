//! The embark screen's words (#883): the site report as toned [`Row`]s, the refusal sentences and
//! the key help, for whichever frontend draws the screen (`fn_tui::embark` in cells,
//! `fn_gui::embark` in a window). The screen's state and its keys are the terminal crate's
//! (`fn_tui::embark::EmbarkScreen`); this module only says what the report reads.
//!
//! [`report_rows`] is the whole "CHOOSE A SITE" panel under its frame: the seed, the preset, the
//! zoom, then LAND, WITHIN and OUTLOOK sections, or the refusal alone for a site that cannot be
//! settled. A blank line is an empty [`Row`].

use fn_sim::defs::Terrain;
use fn_sim::embark::{GroupCount, LandKind, Level, Outlook, SiteProblem, SiteReport, SurveyMode};
use fn_sim::{Defs, Nation, Pos};

use crate::rows::{Marks, Row, Tone, wrap};

/// What the report is about besides the [`SiteReport`] itself: the screen's own state as the panel
/// reads it.
#[derive(Clone, Copy, Debug)]
pub struct Site<'a> {
    pub seed: u64,
    pub nation: Nation,
    /// The difficulty preset's id (`EmbarkScreen::difficulty`).
    pub difficulty: Option<&'a str>,
    pub mode: SurveyMode,
    /// The map size's name (`Small`).
    pub size_name: &'a str,
    /// The zoom level's scale: tiles to a block (1 at the closest zoom).
    pub scale: i32,
    pub overview: bool,
    pub cursor: Pos,
    /// The terrain under the cursor, for the clearer wording of a refusal.
    pub terrain: Terrain,
}

/// The panel's title, with the seal either side: `◆ CHOOSE A SITE ◆`.
pub fn title(marks: &Marks) -> String {
    format!("{0} CHOOSE A SITE {0}", marks.seal)
}

/// The key help's title and its rows (keys, what they do).
pub const HELP_TITLE: &str = "EMBARK KEYS";
pub const HELP: [(&str, &str); 7] = [
    ("Arrows/WASD", "move (Shift: 10 cells)"),
    ("Enter, +", "zoom in; at the closest zoom, settle"),
    ("-", "zoom out"),
    ("Esc", "zoom out, or back at the overview"),
    ("E", "settle at the cursor"),
    ("R", "a new random seed"),
    ("Q", "quit"),
];

/// Why `problem` refuses a site, in words; `terrain` (the tile itself) picks the clearer wording
/// for `NotLand`.
pub fn problem_text(problem: SiteProblem, terrain: Terrain) -> String {
    match problem {
        SiteProblem::NotLand if terrain == Terrain::Mountain => "a mountain".to_owned(),
        SiteProblem::NotLand if terrain.is_water() => "open water".to_owned(),
        SiteProblem::NotLand => "not open ground".to_owned(),
        SiteProblem::NearEdge => "too close to the map's edge".to_owned(),
        SiteProblem::TooSmall => "the landmass is too small to live on".to_owned(),
        SiteProblem::NoRoom => "not enough open room for the settlers".to_owned(),
    }
}

/// The refusal as the status line and the panel say it: `Can't settle: open water.`
pub fn refusal(problem: SiteProblem, terrain: Terrain) -> String {
    format!("Can't settle: {}.", problem_text(problem, terrain))
}

fn land_label(kind: LandKind, defs: &Defs) -> String {
    match kind {
        LandKind::Biome(id) => defs.biome(id).name.clone(),
        LandKind::Mountain => "Mountain".to_owned(),
        LandKind::Water => "Sea".to_owned(),
    }
}

/// The climate band's name and effects, lower case (`docs/design/difficulty.md`, "Embark"): for
/// example `cool: cold, frost`, or `warm: mild` for a band with neither. The effects list is
/// `crate::view::climate_effects`'s own, which the Season section shares (#299).
fn climate_word(climate: &fn_sim::seasons::ClimateDef) -> String {
    format!("{}: {}", climate.name.to_lowercase(), crate::view::climate_effects(climate))
}

fn level_word(level: Level) -> &'static str {
    match level {
        Level::None => "none",
        Level::Scarce => "scarce",
        Level::Some => "some",
        Level::Plenty => "plenty",
    }
}

fn water_kind_word(t: Terrain) -> &'static str {
    match t {
        Terrain::Water => "sea",
        Terrain::Lake => "lake",
        Terrain::River => "river",
        Terrain::Ford => "ford",
        _ => "water",
    }
}

fn water_text(report: &SiteReport, mode: SurveyMode, defs: &Defs) -> String {
    match report.water {
        None => "none within reach".to_owned(),
        Some((terrain, steps)) => match mode {
            SurveyMode::Rough => {
                if steps <= defs.embark.outlook.water_far {
                    "near".to_owned()
                } else {
                    "far".to_owned()
                }
            }
            SurveyMode::Exact => format!("{}, {steps} steps", water_kind_word(terrain)),
        },
    }
}

/// `n` with a thousands separator ("4,563"), as the mockup shows room-to-roam counts.
fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

fn group_row(g: &GroupCount) -> Row {
    let left = match g.count {
        Some(c) => format!("{:<8}{c:>3}", g.name),
        None => format!("{:<8}", g.name),
    };
    let right = if g.hidden { "?".to_owned() } else { level_word(g.level).to_owned() };
    Row::pair(left, Tone::Text, right, Tone::Text)
}

/// The panel's rows for `site` and its `report`, `width` cells wide (what decides how the seed
/// and the long lines wrap; pass `usize::MAX` for no limit). The terminal draws them under its
/// frame's title; the window under its own.
pub fn report_rows(
    site: &Site,
    report: &SiteReport,
    defs: &Defs,
    marks: &Marks,
    width: usize,
) -> Vec<Row> {
    let mut rows = Vec::new();
    let seed_text = format!("Seed {}", site.seed);
    let nation_name = defs.nation(site.nation).name.clone();
    if seed_text.chars().count() + 1 + nation_name.chars().count() <= width {
        rows.push(Row::pair(seed_text, Tone::Dim, nation_name, Tone::Text));
    } else {
        // A random seed runs to 20 digits. Sharing the row would cut the seed, and the seed is
        // what a player copies to replay a map, so the nation gets its own row instead.
        rows.push(Row::text(seed_text, Tone::Dim));
        rows.push(Row::text(nation_name, Tone::Text));
    }
    // A line under the seed (#298, docs/design/difficulty.md "Embark"): the chosen preset, and
    // the size/survey mode already shown on the seed step.
    let preset_name = site
        .difficulty
        .and_then(|id| defs.difficulty.preset(id))
        .map_or("Normal", |p| p.name.as_str());
    let survey_name = match site.mode {
        SurveyMode::Exact => "Exact",
        SurveyMode::Rough => "Rough",
    };
    rows.push(Row::pair(
        preset_name.to_owned(),
        Tone::Dim,
        format!("{} · {survey_name}", site.size_name),
        Tone::Text,
    ));
    let zoom_label = if site.overview {
        "overview"
    } else if site.scale == 1 {
        "site"
    } else {
        "region"
    };
    rows.push(Row::pair(
        format!("Zoom 1:{} {zoom_label}", site.scale),
        Tone::Dim,
        format!("({}, {})", site.cursor.x, site.cursor.y),
        Tone::Text,
    ));
    rows.push(Row::default());

    if let Some(problem) = report.problem {
        rows.push(Row::text(format!("{} OUTLOOK", marks.bullet), Tone::Title));
        let cross = if marks.ascii { "x" } else { "✗" };
        // Wrapped like the outlook reasons below: the reason is the whole point of this line,
        // and the longer ones ("the landmass is too small to live on") don't fit one row.
        let text = format!("{cross} {}", refusal(problem, site.terrain));
        for line in wrap(&text, width, 3, marks.ellipsis) {
            rows.push(Row::text(line, Tone::Bad));
        }
        return rows;
    }

    rows.push(Row::pair(
        format!("{} LAND", marks.bullet),
        Tone::Title,
        if report.mainland { "mainland" } else { "island" },
        Tone::Text,
    ));
    let biomes: Vec<_> = report.biomes.iter().take(4).collect();
    for pair in biomes.chunks(2) {
        let text = pair
            .iter()
            .map(|&&(k, pct)| format!("{} {pct}%", land_label(k, defs)))
            .collect::<Vec<_>>()
            .join("  ");
        rows.push(Row::text(text, Tone::Text));
    }
    // The start tile's climate band (#297, docs/design/difficulty.md "Embark"), only when the
    // chosen preset has seasons on: Sandbox, or no preset, never shows it. Packed left like the
    // biome rows above it: the value is short, so a right-aligned pair left it stranded at the
    // panel's far edge.
    let seasons_on =
        site.difficulty.and_then(|id| defs.difficulty.preset(id)).is_some_and(|k| k.seasons);
    if seasons_on {
        rows.push(Row::text(format!("Winter  {}", climate_word(&report.climate)), Tone::Text));
    }
    rows.push(Row::pair(
        "Room to roam",
        Tone::Text,
        format!("{} tiles", thousands(report.room)),
        Tone::Text,
    ));
    if !site.overview {
        rows.push(Row::pair("Water", Tone::Text, water_text(report, site.mode, defs), Tone::Text));
    }

    if site.overview {
        return rows;
    }
    rows.push(Row::default());

    let radius = defs.embark.report_radius;
    let side = radius * 2 + 1;
    rows.push(Row::pair(
        format!("{} WITHIN {radius} TILES", marks.bullet),
        Tone::Title,
        format!("{side}x{side}"),
        Tone::Text,
    ));
    rows.extend(report.groups.iter().map(group_row));

    rows.push(Row::default());
    let outlook_word = match report.outlook {
        Outlook::Easy => "Easy",
        Outlook::Fair => "Fair",
        Outlook::Hard => "Hard",
    };
    let outlook_text = if report.rough {
        format!("{outlook_word} (rough guess)")
    } else {
        outlook_word.to_owned()
    };
    rows.push(Row::pair(
        format!("{} OUTLOOK", marks.bullet),
        Tone::Title,
        outlook_text,
        Tone::Text,
    ));
    let reasons = format!("{}.", report.reasons.join("; "));
    for line in wrap(&reasons, width, 2, marks.ellipsis) {
        rows.push(Row::text(line, Tone::Text));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use fn_sim::embark::Survey;

    fn site(survey: &Survey, defs: &Defs, at: Pos, overview: bool) -> (Site<'static>, SiteReport) {
        let report = survey.report(at, SurveyMode::Exact, Some("normal"), defs);
        let terrain = survey.map().get(at).map_or(Terrain::Water, |t| t.terrain);
        let site = Site {
            seed: 7,
            nation: Nation::Earth,
            difficulty: Some("normal"),
            mode: SurveyMode::Exact,
            size_name: "Small",
            scale: if overview { 6 } else { 1 },
            overview,
            cursor: at,
            terrain,
        };
        (site, report)
    }

    fn survey(defs: &Defs) -> Survey {
        let size = defs.embark.map_size("small").unwrap();
        Survey::new(&Survey::config(7, size), defs)
    }

    fn plain(rows: &[Row]) -> Vec<String> {
        rows.iter().map(Row::plain).collect()
    }

    #[test]
    fn a_refused_site_is_the_refusal_alone_and_a_good_one_has_its_sections() {
        let defs = Defs::bundled();
        let survey = survey(&defs);
        let marks = Marks::new(false);

        // The map's border is sea: the header, then why it is refused.
        let (s, r) = site(&survey, &defs, Pos::new(0, 0), false);
        let lines = plain(&report_rows(&s, &r, &defs, &marks, 40));
        assert_eq!(lines[0], "Seed 7 Earth Kingdom");
        assert_eq!(lines.last().unwrap(), "✗ Can't settle: open water.");
        assert!(!lines.iter().any(|l| l.contains("LAND")), "{lines:?}");

        let at = survey.default_site(&defs);
        let (s, r) = site(&survey, &defs, at, false);
        let lines = plain(&report_rows(&s, &r, &defs, &marks, 40));
        for section in ["LAND", "Room to roam", "Water", "WITHIN", "OUTLOOK"] {
            assert!(lines.iter().any(|l| l.contains(section)), "{section}: {lines:?}");
        }
        // The overview stops after the land: no water line, no groups.
        let (s, r) = site(&survey, &defs, at, true);
        let lines = plain(&report_rows(&s, &r, &defs, &marks, 40));
        assert!(lines.iter().any(|l| l.contains("Room to roam")));
        assert!(!lines.iter().any(|l| l.contains("WITHIN") || l.starts_with("Water")));
    }

    #[test]
    fn a_long_seed_gives_the_nation_a_row_of_its_own() {
        let defs = Defs::bundled();
        let survey = survey(&defs);
        let (mut s, r) = site(&survey, &defs, survey.default_site(&defs), true);
        s.seed = 17_940_285_611_847_812_345;
        let rows = report_rows(&s, &r, &defs, &Marks::new(false), 30);
        assert_eq!(rows[0].plain(), "Seed 17940285611847812345");
        assert_eq!(rows[1].plain(), "Earth Kingdom");
    }
}

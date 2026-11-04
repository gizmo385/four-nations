//! Seasons, climate bands, frost and cold (`data/seasons.toml`, `docs/design/difficulty.md`,
//! issue #295): the knobs the year and the land turn, once a difficulty preset's `seasons` flag
//! is on (`crate::difficulty::Knobs::seasons`).
//!
//! `World::season`/`year`/`season_left` (`world/season.rs`) are pure functions of the tick
//! against `[calendar]` and the four `[[season]]` entries here, in file order: a world starts on
//! index 0 (spring, in the bundled data). `World::climate_at` classifies a tile by its mapgen
//! `temperature` into the first `[[climate]]` band (in file order) whose `max_temperature`
//! reaches it. Neither reads `World::knobs`: whether their result *does* anything (growth, frost,
//! cold) is for the systems that read them to decide, the same way `World::modifiers_for` is
//! always computable but only `tick`'s systems decide when it matters.

use serde::Deserialize;

use crate::defs::DefsError;

/// `[calendar]` of `data/seasons.toml`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalendarDef {
    /// In-game minutes in one season; a year is four of these.
    pub season_minutes: f32,
    /// How long before winter the `SeasonEventKind::WinterWarning` event fires.
    pub winter_warning_minutes: f32,
    /// Growth factor a `frost_hardy` harvest (fishing) uses in winter, in every climate band,
    /// instead of the season or climate factor (`World::growth_rate`).
    pub hardy_winter_growth: f32,
}

/// One `[[season]]` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct SeasonDef {
    pub id: String,
    pub name: String,
    /// Multiplies `World::growth_rate`'s season factor. Meaningless (and left at 1.0) for the
    /// winter season, whose growth instead comes from the tile's climate band.
    pub growth: f32,
    /// Is this the winter season: frost strikes on its first tick and cold applies throughout it
    /// (`World::tick_season`, `World::tick_needs`)? Validation requires exactly one.
    pub winter: bool,
}

/// One `[[climate]]` band.
#[derive(Debug, Clone, PartialEq)]
pub struct ClimateDef {
    pub id: String,
    pub name: String,
    /// The top of this band's `Tile::temperature` range, as `defs::climate_unit`, in `(0, 1]`.
    /// `World::climate_at` picks the first band (file order) whose `max_temperature` reaches a
    /// tile's own.
    pub max_temperature: f32,
    /// Growth factor in winter, before `winter_severity` scales it toward 1.0
    /// (`World::growth_rate`).
    pub winter_growth: f32,
    /// Does the first frost of winter strike a ripe, non-`frost_hardy` wild plant here?
    pub frost: bool,
    /// Does a unit standing here in winter, with no heat source in reach, feel the cold
    /// (`World::tick_needs`)?
    pub cold: bool,
}

/// `[cold]` of `data/seasons.toml`: how much faster a cold unit's needs move, before
/// `winter_severity` scales each toward 1.0 (`World::tick_needs`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColdDef {
    pub energy_decay: f32,
    pub hunger_decay: f32,
    /// How much slower (below 1.0) a cold unit regains energy asleep, unless it's heated or in a
    /// bed whose own `sleep_bonus` is above 1.0 (the igloo).
    pub sleep: f32,
}

/// Everything in `data/seasons.toml`, resolved and validated. A default (unloaded) value has one
/// season per quarter and one climate band covering every temperature, all at the identity
/// (1.0); nothing reads it unless a difficulty preset's `seasons` flag is on, and the bundled data
/// is always loaded by `Defs::bundled` before any preset can be chosen.
#[derive(Debug, Clone, PartialEq)]
pub struct SeasonsDef {
    pub calendar: CalendarDef,
    /// In file order: `World::season_index` indexes straight into this, so a world starts on
    /// whichever is first (spring, in the bundled data).
    pub seasons: Vec<SeasonDef>,
    /// In file order, ascending `max_temperature`.
    pub climates: Vec<ClimateDef>,
    pub cold: ColdDef,
}

impl Default for SeasonsDef {
    fn default() -> Self {
        let season =
            |winter| SeasonDef { id: String::new(), name: String::new(), growth: 1.0, winter };
        Self {
            calendar: CalendarDef {
                season_minutes: 15.0,
                winter_warning_minutes: 3.0,
                hardy_winter_growth: 1.0,
            },
            seasons: vec![season(false), season(false), season(false), season(true)],
            climates: vec![ClimateDef {
                id: String::new(),
                name: String::new(),
                max_temperature: 1.0,
                winter_growth: 1.0,
                frost: false,
                cold: false,
            }],
            cold: ColdDef { energy_decay: 1.0, hunger_decay: 1.0, sleep: 1.0 },
        }
    }
}

impl SeasonsDef {
    /// The band a tile's quantized `temperature` falls in: the first (file order) whose
    /// `max_temperature` reaches it. Total, because validation requires the bands to ascend to
    /// 1.0.
    pub fn climate_for(&self, temperature: u8) -> &ClimateDef {
        let t = crate::defs::climate_unit(temperature) as f32;
        self.climates
            .iter()
            .find(|c| t <= c.max_temperature)
            .unwrap_or_else(|| self.climates.last().expect("validated: at least one climate band"))
    }

    /// The index of the one winter season. Validation guarantees exactly one.
    pub fn winter_index(&self) -> usize {
        self.seasons.iter().position(|s| s.winter).expect("validated: exactly one winter season")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeasonsFile {
    calendar: RawCalendar,
    #[serde(rename = "season")]
    seasons: Vec<RawSeason>,
    #[serde(rename = "climate")]
    climates: Vec<RawClimate>,
    cold: RawCold,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCalendar {
    season_minutes: f32,
    winter_warning_minutes: f32,
    hardy_winter_growth: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSeason {
    id: String,
    name: String,
    #[serde(default)]
    growth: Option<f32>,
    #[serde(default)]
    winter: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClimate {
    id: String,
    name: String,
    max_temperature: f32,
    winter_growth: f32,
    #[serde(default)]
    frost: bool,
    #[serde(default)]
    cold: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCold {
    energy_decay: f32,
    hunger_decay: f32,
    sleep: f32,
}

/// A factor the acceptance criteria calls out as bounded to `[0, 4]`.
fn in_zero_to_four(v: f32) -> bool {
    v.is_finite() && (0.0..=4.0).contains(&v)
}

impl SeasonsFile {
    fn resolve(self) -> Result<SeasonsDef, DefsError> {
        let err = |msg: String| Err(DefsError(format!("seasons.toml: {msg}")));
        let c = &self.calendar;
        if !(c.season_minutes.is_finite() && c.season_minutes > 0.0) {
            return err("calendar.season_minutes must be a positive number".into());
        }
        if !(c.winter_warning_minutes.is_finite() && c.winter_warning_minutes >= 0.0) {
            return err("calendar.winter_warning_minutes must be a non-negative number".into());
        }
        if c.winter_warning_minutes >= c.season_minutes {
            return err("calendar.winter_warning_minutes must be less than season_minutes".into());
        }
        if !in_zero_to_four(c.hardy_winter_growth) {
            return err("calendar.hardy_winter_growth must be in [0, 4]".into());
        }

        if self.seasons.len() != 4 {
            return err(format!("need exactly 4 [[season]] entries, got {}", self.seasons.len()));
        }
        let winters = self.seasons.iter().filter(|s| s.winter).count();
        if winters != 1 {
            return err(format!("need exactly one winter season, got {winters}"));
        }
        let mut seasons = Vec::with_capacity(4);
        for raw in self.seasons {
            if seasons.iter().any(|s: &SeasonDef| s.id == raw.id) {
                return err(format!("season {}: duplicate id", raw.id));
            }
            let growth = if raw.winter {
                if raw.growth.is_some() {
                    return err(format!(
                        "season {}: the winter season must not set growth (it comes from the \
                         tile's climate band)",
                        raw.id
                    ));
                }
                1.0
            } else {
                let Some(g) = raw.growth else {
                    return err(format!("season {}: growth is required", raw.id));
                };
                if !in_zero_to_four(g) {
                    return err(format!("season {}: growth must be in [0, 4]", raw.id));
                }
                g
            };
            seasons.push(SeasonDef { id: raw.id, name: raw.name, growth, winter: raw.winter });
        }

        if self.climates.is_empty() {
            return err("need at least one [[climate]] band".into());
        }
        let mut climates = Vec::with_capacity(self.climates.len());
        let mut prev_max = 0.0f32;
        for (i, raw) in self.climates.into_iter().enumerate() {
            if climates.iter().any(|c: &ClimateDef| c.id == raw.id) {
                return err(format!("climate {}: duplicate id", raw.id));
            }
            if !(raw.max_temperature.is_finite() && raw.max_temperature > 0.0) {
                return err(format!(
                    "climate {}: max_temperature must be a positive number",
                    raw.id
                ));
            }
            if i > 0 && raw.max_temperature <= prev_max {
                return err(format!(
                    "climate {}: max_temperature must ascend strictly over the previous band",
                    raw.id
                ));
            }
            if !in_zero_to_four(raw.winter_growth) {
                return err(format!("climate {}: winter_growth must be in [0, 4]", raw.id));
            }
            prev_max = raw.max_temperature;
            climates.push(ClimateDef {
                id: raw.id,
                name: raw.name,
                max_temperature: raw.max_temperature,
                winter_growth: raw.winter_growth,
                frost: raw.frost,
                cold: raw.cold,
            });
        }
        if climates.last().unwrap().max_temperature != 1.0 {
            return err("the last climate band's max_temperature must be exactly 1.0".into());
        }

        let cold = &self.cold;
        for (what, v) in [
            ("energy_decay", cold.energy_decay),
            ("hunger_decay", cold.hunger_decay),
            ("sleep", cold.sleep),
        ] {
            if !in_zero_to_four(v) || v <= 0.0 {
                return err(format!("cold.{what} must be in (0, 4]"));
            }
        }

        Ok(SeasonsDef {
            calendar: CalendarDef {
                season_minutes: c.season_minutes,
                winter_warning_minutes: c.winter_warning_minutes,
                hardy_winter_growth: c.hardy_winter_growth,
            },
            seasons,
            climates,
            cold: ColdDef {
                energy_decay: cold.energy_decay,
                hunger_decay: cold.hunger_decay,
                sleep: cold.sleep,
            },
        })
    }
}

impl crate::defs::Defs {
    /// These definitions with the seasons read from `seasons` (`data/seasons.toml`), replacing
    /// any they had. A separate step from `from_toml`, as `with_difficulty` is, so data without
    /// one (most tests) simply has `SeasonsDef::default()`, which `World::season`,
    /// `World::climate_at` and friends can still read safely; it is only ever consulted by
    /// systems that first check a difficulty preset's `seasons` flag, and no preset exists
    /// without this also being loaded (`Defs::bundled`).
    pub fn with_seasons(mut self, seasons: &str) -> Result<Self, DefsError> {
        self.seasons = toml::from_str::<SeasonsFile>(seasons)
            .map_err(|e| DefsError(format!("seasons.toml: {e}")))?
            .resolve()?;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::Defs;

    const SEASONS: &str = include_str!("../../../data/seasons.toml");

    fn defs_without_seasons() -> Defs {
        Defs::from_toml(
            include_str!("../../../data/biomes.toml"),
            include_str!("../../../data/resources.toml"),
            include_str!("../../../data/nations.toml"),
            include_str!("../../../data/research.toml"),
            include_str!("../../../data/needs.toml"),
            include_str!("../../../data/jobs.toml"),
            include_str!("../../../data/bending.toml"),
            include_str!("../../../data/buildings.toml"),
        )
        .unwrap()
    }

    #[test]
    fn unloaded_defs_fall_back_to_a_safe_default() {
        let defs = defs_without_seasons();
        assert_eq!(defs.seasons, SeasonsDef::default());
        assert_eq!(defs.seasons.seasons.len(), 4);
        assert_eq!(defs.seasons.winter_index(), 3);
        assert_eq!(defs.seasons.climate_for(0).max_temperature, 1.0);
    }

    #[test]
    fn bundled_seasons_data_is_valid() {
        let defs = Defs::bundled();
        assert_eq!(defs.seasons.seasons.len(), 4);
        assert_eq!(defs.seasons.seasons.iter().filter(|s| s.winter).count(), 1);
        assert_eq!(defs.seasons.seasons[0].id, "spring");
        assert!(defs.seasons.seasons[0].growth > 1.0, "spring should boost growth");
        assert_eq!(defs.seasons.seasons[3].id, "winter");
        assert!(defs.seasons.climates.len() >= 2);
        assert_eq!(defs.seasons.climates.last().unwrap().max_temperature, 1.0);
        // Ascending, strictly.
        assert!(
            defs.seasons.climates.windows(2).all(|w| w[0].max_temperature < w[1].max_temperature)
        );
        let polar = &defs.seasons.climates[0];
        assert!(polar.frost && polar.cold);
        assert_eq!(polar.winter_growth, 0.0);
    }

    #[test]
    fn wrong_season_count_is_rejected() {
        let bad = SEASONS.replacen(
            "[[season]]\nid = \"spring\"\nname = \"Spring\"\ngrowth = 1.25\n\n",
            "",
            1,
        );
        let err = defs_without_seasons().with_seasons(&bad).unwrap_err();
        assert!(err.0.contains("exactly 4"), "{err}");
    }

    #[test]
    fn more_than_one_winter_is_rejected() {
        let bad = SEASONS.replacen(
            "id = \"autumn\"\nname = \"Autumn\"\ngrowth = 0.75",
            "id = \"autumn\"\nname = \"Autumn\"\ngrowth = 0.75\nwinter = true",
            1,
        );
        let err = defs_without_seasons().with_seasons(&bad).unwrap_err();
        assert!(err.0.contains("winter"), "{err}");
    }

    #[test]
    fn winter_season_may_not_set_growth() {
        let bad = SEASONS.replacen("\nwinter = true", "\nwinter = true\ngrowth = 1.0", 1);
        let err = defs_without_seasons().with_seasons(&bad).unwrap_err();
        assert!(err.0.contains("must not set growth"), "{err}");
    }

    #[test]
    fn climate_bands_must_ascend_to_1() {
        let bad = SEASONS.replacen("max_temperature = 1.0", "max_temperature = 0.9", 1);
        let err = defs_without_seasons().with_seasons(&bad).unwrap_err();
        assert!(err.0.contains("1.0"), "{err}");
    }

    #[test]
    fn a_factor_out_of_range_is_rejected() {
        let bad = SEASONS.replacen("growth = 1.25", "growth = 5.0", 1);
        let err = defs_without_seasons().with_seasons(&bad).unwrap_err();
        assert!(err.0.contains("growth"), "{err}");
    }

    #[test]
    fn cold_multipliers_must_be_positive() {
        let bad = SEASONS.replacen("sleep = 0.5", "sleep = 0.0", 1);
        let err = defs_without_seasons().with_seasons(&bad).unwrap_err();
        assert!(err.0.contains("cold.sleep"), "{err}");
    }
}

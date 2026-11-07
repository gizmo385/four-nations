//! `data/scenarios.toml` (#481, docs/design/agent-scripting.md, "Scenario starts"): a normal
//! start plus an optional [`crate::surface::Prelude`], so testing the midgame or late game
//! doesn't mean playing the early game first. Read here and by `crate::surface::Host` and
//! `crate::shell::Shell`, never by `fn_sim`: a scenario only ever names the same seed, nation,
//! map size, difficulty and site a player could type on the command line, plus commands a
//! normal autoplay opening would issue anyway.

use fn_sim::Nation;
use serde::Deserialize;

use crate::surface::{Prelude, RunLogHeader};

/// One `[[scenario]]` entry: a normal start (`seed`, `nation`, `map_size`, `difficulty`, the
/// same meanings `fn_tui`'s own CLI flags give them) plus an optional [`Prelude`].
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub brief: String,
    pub seed: u64,
    pub nation: Nation,
    pub map_size: Option<String>,
    pub difficulty: Option<String>,
    /// `"default"` (the plain `SEED NATION` path, centred on the map) or `"X,Y"` (a founded game
    /// at that site): the same vocabulary `fn_agent serve --start` already uses. [`Self::start`]
    /// parses it.
    pub site: String,
    pub prelude: Option<Prelude>,
}

impl Scenario {
    /// [`Self::site`], parsed: `None` for `"default"`, `Some((x, y))` for `"X,Y"`.
    pub fn start(&self) -> Result<Option<(i32, i32)>, String> {
        if self.site == "default" {
            return Ok(None);
        }
        let (x, y) = self.site.split_once(',').ok_or_else(|| {
            format!("scenario {:?} site wants default or X,Y, got {:?}", self.id, self.site)
        })?;
        match (x.trim().parse(), y.trim().parse()) {
            (Ok(x), Ok(y)) => Ok(Some((x, y))),
            _ => Err(format!(
                "scenario {:?} site wants default or X,Y, got {:?}",
                self.id, self.site
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScenarioFile {
    #[serde(rename = "scenario")]
    scenarios: Vec<Scenario>,
}

/// Every preset in `data/scenarios.toml`, embedded at compile time.
pub fn bundled() -> Vec<Scenario> {
    from_toml(include_str!("../../../data/scenarios.toml"))
        .expect("bundled data/scenarios.toml must be valid; run `cargo test -p fn_tui`")
}

pub fn from_toml(src: &str) -> Result<Vec<Scenario>, toml::de::Error> {
    Ok(toml::from_str::<ScenarioFile>(src)?.scenarios)
}

/// The bundled preset with id `id`, or an error naming it and listing every id that is bundled.
pub fn find(id: &str) -> Result<Scenario, String> {
    let scenarios = bundled();
    scenarios.iter().find(|s| s.id == id).cloned().ok_or_else(|| {
        let ids: Vec<&str> = scenarios.iter().map(|s| s.id.as_str()).collect();
        format!("unknown scenario {id:?}; known scenarios: {}", ids.join(", "))
    })
}

/// The bundled scenario whose own seed, nation, map size, difficulty, site and prelude exactly
/// match `header`, if any: what `fn_agent keys` (#481) uses to print `--scenario ID` instead of
/// spelling out every flag that preset already sets.
pub fn id_for(header: &RunLogHeader) -> Option<String> {
    bundled()
        .into_iter()
        .find(|s| {
            s.seed == header.seed
                && s.nation == header.nation
                && s.map_size == header.map_size
                && s.difficulty == header.difficulty
                && s.start().unwrap_or(None) == header.site
                && s.prelude == header.prelude
        })
        .map(|s| s.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::Host;

    #[test]
    fn bundled_scenarios_parse_and_build_a_host() {
        let scenarios = bundled();
        assert!(!scenarios.is_empty());
        for nation in Nation::ALL {
            assert!(
                scenarios.iter().any(|s| s.nation == nation),
                "{nation:?} has no bundled scenario"
            );
        }
        for scenario in &scenarios {
            let host = Host::from_scenario(&scenario.id)
                .unwrap_or_else(|e| panic!("scenario {:?}: {e}", scenario.id));
            assert_eq!(host.observe().turn, 0, "turn 0 starts after the prelude, not during it");
        }
    }

    #[test]
    fn an_unknown_id_names_itself_and_lists_what_is_bundled() {
        let err = find("not-a-scenario").unwrap_err();
        assert!(err.contains("not-a-scenario"), "{err}");
        for scenario in bundled() {
            assert!(err.contains(&scenario.id), "{err}");
        }
    }

    #[test]
    fn default_and_literal_sites_both_parse() {
        let mut s = bundled().into_iter().next().unwrap();
        s.site = "default".to_owned();
        assert_eq!(s.start().unwrap(), None);
        s.site = "12,34".to_owned();
        assert_eq!(s.start().unwrap(), Some((12, 34)));
        s.site = "garbage".to_owned();
        assert!(s.start().is_err());
    }

    #[test]
    fn id_for_finds_a_bundled_preset_by_its_own_header() {
        let scenario = &bundled()[0];
        let header = Host::from_scenario(&scenario.id).unwrap().header();
        assert_eq!(id_for(&header), Some(scenario.id.clone()));
    }
}

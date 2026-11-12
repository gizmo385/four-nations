//! Play styles (`docs/design/balance-harness.md`, section 2; #536): `[[style]]` in
//! `data/harness.toml`, parsed with strict keys and resolved (`inherits` followed, defaults
//! filled in) into a `Style`, which maps onto `fn_sim::autoplay::Player`.
//!
//! The mapping is generic: no field names a nation, so a style plays the same way for every one.
//! The knobs themselves (`BedPolicy`, `ResearchOrder`, `HealingPolicy`) are plain types in
//! `fn_sim::autoplay`; this file is the only place TOML touches them.

use std::collections::BTreeMap;
use std::fmt;

use fn_sim::autoplay::{BedPolicy, Economy, HealingPolicy, Player, ResearchOrder, SchoolPick};
use serde::Deserialize;

/// The bundled `data/harness.toml`, compiled in the same way `fn_sim::Defs::bundled` compiles
/// the game's data in. `fn_sim` itself never sees this file.
const BUNDLED: &str = include_str!("../../../data/harness.toml");

/// Why `data/harness.toml` didn't load. One line, naming the style (or the TOML position).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleError(String);

impl fmt::Display for StyleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StyleError {}

fn err<T>(msg: String) -> Result<T, StyleError> {
    Err(StyleError(msg))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawOpening {
    Economy,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawBeds {
    ToTarget,
    Ahead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawSchool {
    First,
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawResearch {
    SharedFirst,
    BranchFirst,
}

/// One `[[style]]` as written. Unknown keys are an error.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStyle {
    id: String,
    inherits: Option<String>,
    opening: Option<RawOpening>,
    farms: Option<bool>,
    healing: Option<bool>,
    beds: Option<RawBeds>,
    bed_margin: Option<u32>,
    research: Option<RawResearch>,
    school: Option<RawSchool>,
    gate_at: Option<u32>,
}

/// The whole file, as far as styles are concerned: `[[style]]` is read here, and the other
/// tables (`[panel]`, `[detectors]`, `[[target]]`, `[[pair]]`) are `config`'s, so they're let
/// through unread. A key that is none of those is still an error.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // the let-through tables exist only so their keys are allowed
struct RawFile {
    #[serde(default)]
    style: Vec<RawStyle>,
    #[serde(default)]
    panel: Option<toml::Value>,
    #[serde(default)]
    detectors: Option<toml::Value>,
    #[serde(default)]
    target: Vec<toml::Value>,
    #[serde(default)]
    pair: Vec<toml::Value>,
    #[serde(default)]
    fork: Vec<toml::Value>,
    #[serde(default)]
    noise: Option<toml::Value>,
}

impl RawStyle {
    /// `self` over `parent`: every field `self` sets wins. `beds` and `bed_margin` move as a
    /// pair when `self` sets `beds` (a child that says `beds = "ahead"` must not inherit its
    /// parent's margin), and `bed_margin` alone keeps the parent's `beds`.
    fn over(&self, parent: &RawStyle) -> RawStyle {
        let (beds, bed_margin) = if self.beds.is_some() {
            (self.beds, self.bed_margin)
        } else {
            (parent.beds, self.bed_margin.or(parent.bed_margin))
        };
        RawStyle {
            id: self.id.clone(),
            inherits: None,
            opening: self.opening.or(parent.opening),
            farms: self.farms.or(parent.farms),
            healing: self.healing.or(parent.healing),
            beds,
            bed_margin,
            research: self.research.or(parent.research),
            school: self.school.or(parent.school),
            gate_at: self.gate_at.or(parent.gate_at),
        }
    }
}

/// What a resolved style plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Play {
    /// `opening = "none"`: no command, ever.
    Idle,
    /// `opening = "economy"`, with every knob settled.
    Economy {
        farms: bool,
        healing: HealingPolicy,
        beds: BedPolicy,
        research: ResearchOrder,
        school: Option<SchoolPick>,
    },
}

/// A play style with `inherits` followed and defaults filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    pub id: String,
    pub play: Play,
    /// The minute the gate is stood by `fn_sim::World::complete_endeavor` (`gate_at`, #751):
    /// the style plays in a United City from then on. `None`: the gate is the opening's to build.
    pub gate_at: Option<u32>,
}

impl Style {
    /// The `fn_sim` player for a settlement of `people` (the economy opening keeps that many
    /// bends designated). The same for every nation.
    pub fn player(&self, people: u32) -> Player {
        match self.play {
            Play::Idle => Player::Idle,
            Play::Economy { farms, healing, beds, research, school } => Player::Economy(
                Economy::new(people)
                    .with_farms(farms)
                    .with_healing(healing)
                    .with_beds(beds)
                    .with_research(research)
                    .with_school(school),
            ),
        }
    }

    fn resolve(raw: &RawStyle) -> Result<Style, StyleError> {
        let id = &raw.id;
        let Some(opening) = raw.opening else {
            return err(format!(
                "style {id:?}: no `opening` (set it, or inherit from a style that does)"
            ));
        };
        match opening {
            RawOpening::None => {
                let extra = [
                    ("farms", raw.farms.is_some()),
                    ("healing", raw.healing.is_some()),
                    ("beds", raw.beds.is_some()),
                    ("bed_margin", raw.bed_margin.is_some()),
                    ("research", raw.research.is_some()),
                    ("school", raw.school.is_some()),
                    ("gate_at", raw.gate_at.is_some()),
                ];
                if let Some((field, _)) = extra.iter().find(|(_, set)| *set) {
                    return err(format!(
                        "style {id:?}: `{field}` does nothing under opening = \"none\""
                    ));
                }
                Ok(Style { id: id.clone(), play: Play::Idle, gate_at: None })
            }
            RawOpening::Economy => {
                let beds = match (raw.beds, raw.bed_margin) {
                    (None, None) => BedPolicy::default(),
                    (Some(RawBeds::Ahead), None) => BedPolicy::Ahead,
                    (Some(RawBeds::Ahead), Some(_)) => {
                        return err(format!(
                            "style {id:?}: `bed_margin` only goes with beds = \"to_target\""
                        ));
                    }
                    (Some(RawBeds::ToTarget) | None, Some(margin)) => {
                        BedPolicy::ToTarget { margin }
                    }
                    (Some(RawBeds::ToTarget), None) => match BedPolicy::default() {
                        BedPolicy::ToTarget { margin } => BedPolicy::ToTarget { margin },
                        BedPolicy::Ahead => BedPolicy::ToTarget { margin: 0 },
                    },
                };
                let healing = match raw.healing {
                    None => HealingPolicy::default(),
                    Some(true) => HealingPolicy::Planned,
                    Some(false) => HealingPolicy::Off,
                };
                let research = match raw.research {
                    None | Some(RawResearch::SharedFirst) => ResearchOrder::SharedFirst,
                    Some(RawResearch::BranchFirst) => ResearchOrder::BranchFirst,
                };
                Ok(Style {
                    id: id.clone(),
                    play: Play::Economy {
                        farms: raw.farms.unwrap_or(true),
                        healing,
                        beds,
                        research,
                        school: raw.school.map(|s| match s {
                            RawSchool::First => SchoolPick::First,
                            RawSchool::Second => SchoolPick::Second,
                        }),
                    },
                    gate_at: raw.gate_at,
                })
            }
        }
    }
}

/// Every style in `data/harness.toml`, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Styles {
    styles: Vec<Style>,
}

impl Styles {
    /// The bundled `data/harness.toml`. Panics if it doesn't load (a test keeps that from
    /// happening), the way `fn_sim::Defs::bundled` does.
    pub fn bundled() -> Styles {
        Styles::parse(BUNDLED).unwrap_or_else(|e| panic!("bundled data/harness.toml: {e}"))
    }

    /// Parses `text` (the whole of a `harness.toml`). Strict: an unknown key, a repeated id, an
    /// `inherits` that names no style or loops, and a field that does nothing for the style's
    /// opening are all errors.
    pub fn parse(text: &str) -> Result<Styles, StyleError> {
        let file: RawFile = toml::from_str(text).map_err(|e| StyleError(e.to_string()))?;
        let mut by_id: BTreeMap<&str, &RawStyle> = BTreeMap::new();
        for s in &file.style {
            if by_id.insert(&s.id, s).is_some() {
                return err(format!("style {:?}: id used twice", s.id));
            }
        }
        let mut styles = Vec::with_capacity(file.style.len());
        for s in &file.style {
            let merged = Self::merged(s, &by_id)?;
            styles.push(Style::resolve(&merged)?);
        }
        Ok(Styles { styles })
    }

    /// `s` with its `inherits` chain folded in.
    fn merged(s: &RawStyle, by_id: &BTreeMap<&str, &RawStyle>) -> Result<RawStyle, StyleError> {
        let mut chain: Vec<&RawStyle> = vec![s];
        while let Some(parent) = chain.last().and_then(|c| c.inherits.as_deref()) {
            let Some(&p) = by_id.get(parent) else {
                return err(format!("style {:?}: inherits unknown style {parent:?}", s.id));
            };
            if chain.iter().any(|c| c.id == p.id) {
                return err(format!("style {:?}: `inherits` loops through {:?}", s.id, p.id));
            }
            chain.push(p);
        }
        // Fold from the root down, each child over what its parent resolved to.
        let mut out = chain.pop().expect("chain starts with s").clone();
        while let Some(child) = chain.pop() {
            out = child.over(&out);
        }
        Ok(out)
    }

    /// The style with this id.
    pub fn get(&self, id: &str) -> Option<&Style> {
        self.styles.iter().find(|s| s.id == id)
    }

    /// Every style, in file order.
    pub fn iter(&self) -> impl Iterator<Item = &Style> {
        self.styles.iter()
    }
}

#[cfg(test)]
mod tests;

//! `data/tui.toml`: a glyph, an ASCII fallback and colours for everything the map draws.
//!
//! Presentation only. `fn_sim` never reads it; this crate embeds it with `include_str!`, like
//! `Defs::bundled`, and the tests fail if the game data gains a terrain, biome, resource, item or
//! building without an entry here.

use std::collections::BTreeMap;

use fn_sim::Terrain;
use serde::Deserialize;

/// An sRGB colour, `[r, g, b]`.
pub type Rgb8 = [u8; 3];

/// How one thing draws: a glyph (or its `--ascii` fallback) in `fg`, on `bg` when it has one.
/// Resources, items and buildings have no `bg`: they draw on their tile's own colour.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    pub glyph: char,
    pub ascii: char,
    pub fg: Rgb8,
    pub bg: Option<Rgb8>,
}

impl Look {
    pub fn glyph(&self, ascii: bool) -> char {
        if ascii { self.ascii } else { self.glyph }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitLook {
    pub glyph: char,
    pub ascii: char,
    /// How far the nation colour is mixed toward white, in `[0, 1]`.
    pub lift: f32,
}

impl UnitLook {
    pub fn glyph(&self, ascii: bool) -> char {
        if ascii { self.ascii } else { self.glyph }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayGlyph {
    pub glyph: char,
    pub ascii: char,
    pub fg: Rgb8,
}

impl OverlayGlyph {
    pub fn glyph(&self, ascii: bool) -> char {
        if ascii { self.ascii } else { self.glyph }
    }
}

/// The background tints (`[overlay]`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overlays {
    pub harvest: Rgb8,
    pub stockpile: Rgb8,
    pub build: Rgb8,
    pub wet: Rgb8,
    pub cursor: Rgb8,
    pub warm: Rgb8,
    pub flame: OverlayGlyph,
    /// A fruit tree an orchard planted that hasn't borne fruit yet (#142).
    pub sapling: OverlayGlyph,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Glyphs {
    pub terrain: BTreeMap<String, Look>,
    pub biome: BTreeMap<String, Look>,
    pub resource: BTreeMap<String, Look>,
    pub item: BTreeMap<String, Look>,
    pub building: BTreeMap<String, Look>,
    pub unit: UnitLook,
    pub overlay: Overlays,
}

/// The key of a terrain in `[terrain.*]`. `Land` has none: bare land draws as its biome. The
/// match is exhaustive, so a new terrain kind can't be forgotten here.
pub fn terrain_key(terrain: Terrain) -> Option<&'static str> {
    match terrain {
        Terrain::Water => Some("water"),
        Terrain::Coast => Some("coast"),
        Terrain::Land => None,
        Terrain::Mountain => Some("mountain"),
        Terrain::River => Some("river"),
        Terrain::Lake => Some("lake"),
        Terrain::Ford => Some("ford"),
        Terrain::Tunnel => Some("tunnel"),
    }
}

/// Every terrain kind that has a `[terrain.*]` entry.
pub const TERRAINS: [Terrain; 7] = [
    Terrain::Water,
    Terrain::Coast,
    Terrain::Mountain,
    Terrain::River,
    Terrain::Lake,
    Terrain::Ford,
    Terrain::Tunnel,
];

impl Glyphs {
    /// The glyphs shipped in `data/tui.toml`, embedded at compile time.
    pub fn bundled() -> Self {
        Self::from_toml(include_str!("../../../data/tui.toml"))
            .expect("bundled data/tui.toml must be valid; run `cargo test -p fn_tui`")
    }

    pub fn from_toml(src: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(src)
    }

    pub fn terrain(&self, terrain: Terrain) -> Option<&Look> {
        self.terrain.get(terrain_key(terrain)?)
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Defs;
    use unicode_width::UnicodeWidthChar;

    use super::*;

    /// Every `Look` in the file with where it is, for the per-entry checks.
    fn all_looks(g: &Glyphs) -> Vec<(String, char, char)> {
        let tables = [
            ("terrain", &g.terrain),
            ("biome", &g.biome),
            ("resource", &g.resource),
            ("item", &g.item),
            ("building", &g.building),
        ];
        let mut out: Vec<_> = tables
            .into_iter()
            .flat_map(|(table, looks)| {
                looks.iter().map(move |(id, l)| (format!("{table}.{id}"), l.glyph, l.ascii))
            })
            .collect();
        out.push(("unit".into(), g.unit.glyph, g.unit.ascii));
        out.push(("overlay.flame".into(), g.overlay.flame.glyph, g.overlay.flame.ascii));
        out.push(("overlay.sapling".into(), g.overlay.sapling.glyph, g.overlay.sapling.ascii));
        out
    }

    fn keys<T>(m: &BTreeMap<String, T>) -> Vec<&str> {
        m.keys().map(String::as_str).collect()
    }

    #[test]
    fn every_id_in_the_game_data_has_an_entry_and_nothing_else_does() {
        let (g, defs) = (Glyphs::bundled(), Defs::bundled());
        fn want(mut ids: Vec<&str>) -> Vec<&str> {
            ids.sort_unstable();
            ids
        }
        assert_eq!(
            keys(&g.terrain),
            want(TERRAINS.iter().filter_map(|&t| terrain_key(t)).collect())
        );
        assert_eq!(keys(&g.biome), want(defs.biomes.iter().map(|b| b.id.as_str()).collect()));
        assert_eq!(keys(&g.resource), want(defs.resources.iter().map(|r| r.id.as_str()).collect()));
        assert_eq!(keys(&g.item), want(defs.items.iter().map(|i| i.id.as_str()).collect()));
        assert_eq!(keys(&g.building), want(defs.buildings.iter().map(|b| b.id.as_str()).collect()));
    }

    #[test]
    fn terrain_and_biome_entries_have_a_background_and_the_rest_do_not() {
        let g = Glyphs::bundled();
        for (id, l) in g.terrain.iter().chain(&g.biome) {
            assert!(l.bg.is_some(), "{id} needs a bg");
        }
        for (id, l) in g.resource.iter().chain(&g.item).chain(&g.building) {
            assert!(l.bg.is_none(), "{id} draws on its tile, so it has no bg");
        }
    }

    #[test]
    fn every_glyph_is_one_cell_wide() {
        for (what, glyph, _) in all_looks(&Glyphs::bundled()) {
            assert_eq!(UnicodeWidthChar::width(glyph), Some(1), "{what}: {glyph:?}");
        }
    }

    #[test]
    fn every_ascii_fallback_is_printable_ascii() {
        for (what, _, ascii) in all_looks(&Glyphs::bundled()) {
            assert!(ascii.is_ascii_graphic(), "{what}: {ascii:?}");
        }
    }

    /// `--ascii` is the mapgen renderer's glyph set for the map itself, so the ASCII frame and
    /// `cargo run -p fn_sim --bin mapgen` agree.
    #[test]
    fn the_ascii_set_matches_the_mapgen_renderer() {
        let (g, defs) = (Glyphs::bundled(), Defs::bundled());
        for b in &defs.biomes {
            assert_eq!(g.biome[&b.id].ascii, b.glyph, "biome {}", b.id);
        }
        for r in &defs.resources {
            assert_eq!(g.resource[&r.id].ascii, r.glyph, "resource {}", r.id);
        }
        let mapgen = |t| match t {
            Terrain::Water => '~',
            Terrain::Lake => '-',
            Terrain::River => '=',
            Terrain::Ford => '+',
            Terrain::Coast => '.',
            Terrain::Mountain => '^',
            Terrain::Tunnel => '_',
            Terrain::Land => unreachable!("land draws as its biome"),
        };
        for t in TERRAINS {
            assert_eq!(g.terrain(t).unwrap().ascii, mapgen(t), "{t:?}");
        }
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let src = include_str!("../../../data/tui.toml").replace("lift = 0.4", "lyft = 0.4");
        assert!(Glyphs::from_toml(&src).is_err());
    }
}

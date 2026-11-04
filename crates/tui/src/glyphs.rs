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

/// Which of a rail tile's four neighbours are rail (#279): bit 0 north, bit 1 east, bit 2 south,
/// bit 3 west, the same shape as the client's `sprites::buildings::LINK_*` bits (#135), though
/// the two frontends pick their own glyph for it.
pub const RAIL_N: u8 = 1;
pub const RAIL_E: u8 = 2;
pub const RAIL_S: u8 = 4;
pub const RAIL_W: u8 = 8;

/// Which neighbours of a rail tile are rail, as a [`RAIL_N`]-[`RAIL_W`] mask for [`link_glyph`].
pub fn rail_mask(n: bool, e: bool, s: bool, w: bool) -> u8 {
    u8::from(n) * RAIL_N + u8::from(e) * RAIL_E + u8::from(s) * RAIL_S + u8::from(w) * RAIL_W
}

/// The fifteen box-drawing pieces a rail tile or site with at least one rail neighbour picks
/// between (`[rail_track]`, #279), indexed `mask - 1` by the mask [`rail_mask`] builds.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RailLinks {
    pub glyph: Vec<char>,
    pub ascii: Vec<char>,
}

/// The piece of track a rail tile with these neighbours draws (#279): `None` for a lone tile
/// (`mask == 0`), which keeps the rail building's own glyph instead. Pure: a table lookup, with
/// no sim or terminal state.
pub fn link_glyph(links: &RailLinks, mask: u8, ascii: bool) -> Option<char> {
    let table = if ascii { &links.ascii } else { &links.glyph };
    let index = usize::from(mask.checked_sub(1)?);
    table.get(index).copied()
}

/// The embark screen's overview thresholds (`[embark]`, #290, docs/design/embark.md).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbarkGlyphDef {
    pub min_overview_scale: i32,
    pub sea_share: f64,
    pub mountain_share: f64,
    pub river_lake_share: f64,
}

/// The background tints (`[overlay]`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overlays {
    pub harvest: Rgb8,
    /// A harvest tile with a standing order on it (#81, #277): the client's `RepeatingHarvest`.
    pub repeating: Rgb8,
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
    pub rail_track: RailLinks,
    pub unit: UnitLook,
    pub overlay: Overlays,
    pub embark: EmbarkGlyphDef,
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
        for mask in 1..=15u8 {
            out.push((
                format!("rail_track[{mask}]"),
                link_glyph(&g.rail_track, mask, false).unwrap(),
                link_glyph(&g.rail_track, mask, true).unwrap(),
            ));
        }
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

    /// `rail_track` has exactly one piece for every mask from 1 to 15 (#279): a lone tile
    /// (`mask == 0`) has none, since it keeps `building.rail`'s own glyph instead.
    #[test]
    fn rail_track_has_a_piece_for_every_nonzero_mask() {
        let g = Glyphs::bundled();
        assert_eq!(g.rail_track.glyph.len(), 15);
        assert_eq!(g.rail_track.ascii.len(), 15);
    }

    /// Every mask (bit 0 north, 1 east, 2 south, 3 west) picks the box-drawing piece that joins
    /// exactly those neighbours (#279): straight through a tile with one or two opposite
    /// neighbours, a corner with two adjacent ones, a T with three, a cross with all four. A
    /// lone tile (mask 0) has no piece: it keeps the rail building's own glyph.
    #[test]
    fn every_mask_picks_the_piece_that_joins_those_neighbours() {
        let g = Glyphs::bundled();
        let cases: [(u8, char, char); 16] = [
            (0, '\0', '\0'), // lone: no piece (see below)
            (RAIL_N, '│', '|'),
            (RAIL_E, '─', '-'),
            (RAIL_N | RAIL_E, '└', '+'),
            (RAIL_S, '│', '|'),
            (RAIL_N | RAIL_S, '│', '|'),
            (RAIL_E | RAIL_S, '┌', '+'),
            (RAIL_N | RAIL_E | RAIL_S, '├', '+'),
            (RAIL_W, '─', '-'),
            (RAIL_N | RAIL_W, '┘', '+'),
            (RAIL_E | RAIL_W, '─', '-'),
            (RAIL_N | RAIL_E | RAIL_W, '┴', '+'),
            (RAIL_S | RAIL_W, '┐', '+'),
            (RAIL_N | RAIL_S | RAIL_W, '┤', '+'),
            (RAIL_E | RAIL_S | RAIL_W, '┬', '+'),
            (RAIL_N | RAIL_E | RAIL_S | RAIL_W, '┼', '+'),
        ];
        assert_eq!(cases.len(), 16, "every mask from 0 to 15");
        for (mask, glyph, ascii) in cases {
            if mask == 0 {
                assert_eq!(link_glyph(&g.rail_track, mask, false), None, "mask {mask:04b}");
                assert_eq!(link_glyph(&g.rail_track, mask, true), None, "mask {mask:04b}");
            } else {
                assert_eq!(link_glyph(&g.rail_track, mask, false), Some(glyph), "mask {mask:04b}");
                assert_eq!(link_glyph(&g.rail_track, mask, true), Some(ascii), "mask {mask:04b}");
            }
        }
    }

    /// [`rail_mask`] builds the same bitmask the table above indexes by.
    #[test]
    fn rail_mask_builds_the_bit_for_each_neighbour() {
        assert_eq!(rail_mask(false, false, false, false), 0);
        assert_eq!(rail_mask(true, false, false, false), RAIL_N);
        assert_eq!(rail_mask(false, true, false, false), RAIL_E);
        assert_eq!(rail_mask(false, false, true, false), RAIL_S);
        assert_eq!(rail_mask(false, false, false, true), RAIL_W);
        assert_eq!(rail_mask(true, true, true, true), RAIL_N | RAIL_E | RAIL_S | RAIL_W);
    }
}

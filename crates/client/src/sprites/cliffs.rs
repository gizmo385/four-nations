//! Mountains (#104, #147): autotiled cliffs around a flat interior, and the boulders on it.

use fn_sim::{Pos, Terrain, TileMap};

use super::{Cell, Decor, GroundRule, one};

/// The sheet the cliff pieces and boulders are on (`art/cliffs.toml`).
pub const SHEET: &str = "original/cliffs.png";

/// Sprites per row of the sheet.
const COLUMNS: u32 = 10;

/// The sprite at `index` (file order in `art/cliffs.toml`).
const fn at(index: u32) -> Cell {
    one(SHEET, index % COLUMNS, index / COLUMNS)
}

/// The sides of a tile that are lower ground, as bits.
const N: u8 = 1;
const S: u8 = 2;
const E: u8 = 4;
const W: u8 = 8;
/// The diagonals that are lower ground, as bits.
const NW: u8 = 1;
const NE: u8 = 2;
const SW: u8 = 4;
const SE: u8 = 8;

/// The inner corners a tile with these lower sides can have: the diagonals whose two sides are
/// both mountain. A diagonal beside a lower side is already part of that side's rim.
const fn corners(lower: u8) -> u8 {
    let mut c = 0;
    if lower & (N | W) == 0 {
        c |= NW;
    }
    if lower & (N | E) == 0 {
        c |= NE;
    }
    if lower & (S | W) == 0 {
        c |= SW;
    }
    if lower & (S | E) == 0 {
        c |= SE;
    }
    c
}

/// How many pieces of a set come before the first one for the lower sides `lower`: each mask has
/// a piece for every subset of its inner corners.
const fn pieces_before(lower: u8) -> u32 {
    let mut n = 0;
    let mut m = 0;
    while m < lower {
        n += 1 << corners(m).count_ones();
        m += 1;
    }
    n
}

/// The pieces in a set: all 16 masks of lower sides, each with every subset of its inner corners.
const PIECES: u32 = pieces_before(16);

/// Autotiled mountain cliffs (#104, #147): a raised plateau drawn from `art/cliffs.toml`, one
/// 16px piece per tile. A piece is picked by which cardinal neighbors are lower ground (any
/// non-mountain tile) and, for a corner whose two sides are both mountain, by whether that
/// diagonal is lower ground (an inner corner). Every combination has its own piece (47 a set), so
/// a plateau one tile wide, one tile tall, or ending in a cap needs no special case. Named by
/// their lower sides in the order `n s e w`, with `flat` for none, and by the inner corners after:
///
/// ```text
/// nw   n   ne      ew       a ridge one tile wide, running north to south
/// w  flat  e       ns       a ridge one tile tall, running west to east
/// sw   s   se      nsw      the east end of that ridge (an end cap)
///                  flat_se  the interior, with lower ground only to its south-east
/// ```
///
/// The cliff face is painted inside the south pieces (the lower rows of any piece with lower
/// ground to the south), so it never overlaps the row below and needs no depth banding of its
/// own, unlike a tree.
pub struct CliffSet {
    /// The index of the set's first piece, `flat`.
    first: u32,
}

impl CliffSet {
    const fn new(first: u32) -> Self {
        CliffSet { first }
    }

    /// The flat plateau top, which the interior of a range uses.
    const fn top(&self) -> Cell {
        at(self.first)
    }

    /// The piece for a mountain tile whose lower sides are `lower` (bits `N`, `S`, `E`, `W`) and
    /// whose lower diagonals are `diag` (bits `NW`, `NE`, `SW`, `SE`); `None` for a tile with no
    /// lower neighbor at all, which stays the flat top.
    fn cell(&self, lower: u8, diag: u8) -> Option<Cell> {
        let diag = diag & corners(lower);
        if lower == 0 && diag == 0 {
            return None;
        }
        // The inner corners as a number: one bit for each corner this mask can have, in the order
        // nw, ne, sw, se.
        let mut subset = 0;
        let mut bit = 0;
        for corner in [NW, NE, SW, SE] {
            if corners(lower) & corner != 0 {
                subset |= u32::from(diag & corner != 0) << bit;
                bit += 1;
            }
        }
        Some(at(self.first + pieces_before(lower) + subset))
    }
}

/// The cliff piece for a mountain tile at `p` (#104, #147), bitmask-autotiled against its eight
/// neighbors: a neighbor is "lower" unless it is mountain too. Off the edge of the map doesn't
/// count, so a range running off the map doesn't grow a false cliff there. `None` for a tile
/// with no lower neighbor, which is the rule's flat `base`.
pub fn cliff_cell(map: &TileMap, p: Pos, set: &CliffSet) -> Option<Cell> {
    let lower = |dx: i32, dy: i32| {
        map.get(Pos::new(p.x + dx, p.y + dy)).is_some_and(|t| t.terrain != Terrain::Mountain)
    };
    let bits = |flags: [(bool, u8); 4]| {
        flags.into_iter().filter(|(l, _)| *l).fold(0, |mask, (_, bit)| mask | bit)
    };
    let sides = bits([(lower(0, -1), N), (lower(0, 1), S), (lower(1, 0), E), (lower(-1, 0), W)]);
    let diag =
        bits([(lower(-1, -1), NW), (lower(1, -1), NE), (lower(-1, 1), SW), (lower(1, 1), SE)]);
    set.cell(sides, diag)
}

impl GroundRule {
    /// The decoration for the tile at `p`, if it rolls one. A mountain's cliff rim carries none
    /// (#104): boulders sit on the flat interior, not across a cliff face.
    pub fn decor_on(&self, map: &TileMap, p: Pos) -> Option<Cell> {
        let on_cliff = self.cliff.is_some_and(|set| cliff_cell(map, p, set).is_some());
        if on_cliff { None } else { self.decor_at(p) }
    }

    /// The decoration for the tile at `p`, if it rolls one, wherever it is on the map.
    pub fn decor_at(&self, p: Pos) -> Option<Cell> {
        let d = self.decor.as_ref()?;
        let h = super::tile_hash(p, 2);
        (h % d.den < d.num).then(|| d.cells[(h >> 8) as usize % d.cells.len()])
    }
}

/// The two plateaus (`docs/design/pixel-art.md`): a grey-faced cliff with a snow top for tundra,
/// and a grey-brown-faced cliff with a rock top for every other biome. The tops have the tones
/// of the snow and rock floors, so the interior and the rim show no seam. The sheet holds the
/// rock set, then the snow set, then the boulders.
const ROCK_CLIFF: CliffSet = CliffSet::new(0);
const SNOW_CLIFF: CliffSet = CliffSet::new(PIECES);

const MOUNTAIN: &[Terrain] = &[Terrain::Mountain];

/// Boulders on a range's flat interior; its rim is a cliff (#104). Grey, and snow-capped for
/// tundra, three of each after the cliff sets.
const BOULDER_0: u32 = 2 * PIECES;
const BOULDERS: Decor =
    Decor { cells: &[at(BOULDER_0), at(BOULDER_0 + 1), at(BOULDER_0 + 2)], num: 1, den: 3 };
const SNOWY_BOULDERS: Decor =
    Decor { cells: &[at(BOULDER_0 + 3), at(BOULDER_0 + 4), at(BOULDER_0 + 5)], num: 1, den: 3 };

pub const GROUND: &[GroundRule] = &[
    GroundRule {
        terrains: MOUNTAIN,
        biome: Some("tundra"),
        base: SNOW_CLIFF.top(),
        variants: &[],
        decor: Some(SNOWY_BOULDERS),
        water: None,
        cliff: Some(&SNOW_CLIFF),
    },
    GroundRule {
        terrains: MOUNTAIN,
        biome: None,
        base: ROCK_CLIFF.top(),
        variants: &[],
        decor: Some(BOULDERS),
        water: None,
        cliff: Some(&ROCK_CLIFF),
    },
];

#[cfg(test)]
mod tests {
    use super::super::ground_rule;
    use super::super::test_support::{TERRAINS, png_size, terrain_map};
    use super::*;
    use fn_sim::Defs;

    /// The sprite names of `art/cliffs.toml` in file order.
    fn sprite_names() -> Vec<String> {
        let path = format!("{}/../../art/cliffs.toml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let doc: toml::Table = text.parse().unwrap();
        doc["sprite"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap().to_owned())
            .collect()
    }

    /// The cell of the sprite called `name` in `art/cliffs.toml`.
    fn named(name: &str) -> Cell {
        let i = sprite_names().iter().position(|n| n == name);
        at(i.unwrap_or_else(|| panic!("no sprite {name} in art/cliffs.toml")) as u32)
    }

    /// The name `art/cliffs.toml` gives the piece with these lower sides and inner corners.
    fn piece_name(prefix: &str, lower: u8, diag: u8) -> String {
        let sides: String = [(N, 'n'), (S, 's'), (E, 'e'), (W, 'w')]
            .iter()
            .filter(|(bit, _)| lower & bit != 0)
            .map(|(_, c)| *c)
            .collect();
        let mut name = format!("{prefix}_{}", if sides.is_empty() { "flat" } else { &sides });
        for (bit, c) in [(NW, "nw"), (NE, "ne"), (SW, "sw"), (SE, "se")] {
            if diag & bit != 0 {
                name = format!("{name}_{c}");
            }
        }
        name
    }

    /// A 3x3 map whose center is a mountain and whose neighbors are lower ground (land) where
    /// the bits of `sides` and `diag` say so, and more mountain elsewhere.
    fn map_around(sides: u8, diag: u8) -> TileMap {
        terrain_map(3, 3, |p| {
            let lower = match (p.x, p.y) {
                (1, 0) => sides & N != 0,
                (1, 2) => sides & S != 0,
                (2, 1) => sides & E != 0,
                (0, 1) => sides & W != 0,
                (0, 0) => diag & NW != 0,
                (2, 0) => diag & NE != 0,
                (0, 2) => diag & SW != 0,
                (2, 2) => diag & SE != 0,
                _ => false,
            };
            if lower { Terrain::Land } else { Terrain::Mountain }
        })
    }

    fn cliff_at(sides: u8, diag: u8, set: &CliffSet) -> Option<Cell> {
        cliff_cell(&map_around(sides, diag), Pos::new(1, 1), set)
    }

    #[test]
    fn about_one_in_three_mountain_tiles_get_a_boulder() {
        let rule = ground_rule(Terrain::Mountain, "plains").unwrap();
        let n = (0..900).filter(|i| rule.decor_at(Pos::new(i % 30, i / 30)).is_some()).count();
        assert!((240..360).contains(&n), "{n}");
    }

    #[test]
    fn every_mountain_is_a_cliff_on_the_cliffs_sheet() {
        let defs = Defs::bundled();
        for biome in &defs.biomes {
            let rule = ground_rule(Terrain::Mountain, &biome.id).unwrap();
            let set = rule.cliff.unwrap_or_else(|| panic!("{} mountain has no cliff", biome.id));
            assert_eq!(rule.base, set.top(), "{}: interior blends into the rim", biome.id);
            assert!(rule.decor.is_some(), "{}: interior keeps its boulders", biome.id);
        }
        for terrain in TERRAINS.iter().filter(|t| **t != Terrain::Mountain) {
            let rule = ground_rule(*terrain, "plains").unwrap();
            assert!(rule.cliff.is_none(), "{terrain:?}");
        }
    }

    #[test]
    fn tundra_cliffs_are_snowy_and_the_rest_are_rocky() {
        let snow = ground_rule(Terrain::Mountain, "tundra").unwrap().cliff.unwrap();
        let rock = ground_rule(Terrain::Mountain, "plains").unwrap().cliff.unwrap();
        assert_eq!(snow.top(), named("snow_flat"));
        assert_eq!(rock.top(), named("rock_flat"));
        let desert = ground_rule(Terrain::Mountain, "desert").unwrap().cliff.unwrap();
        assert_eq!(desert.top(), rock.top());
    }

    /// The sheet lists every piece of both sets in the order the code indexes them, then the
    /// boulders the decor rows use.
    #[test]
    fn the_sheet_holds_every_piece_then_the_boulders() {
        assert_eq!(PIECES, 47);
        let names = sprite_names();
        for (prefix, set) in [("rock", &ROCK_CLIFF), ("snow", &SNOW_CLIFF)] {
            let mut i = set.first as usize;
            for lower in 0..16u8 {
                let mask = corners(lower);
                for diag in 0..16u8 {
                    if diag & !mask == 0 {
                        assert_eq!(names[i], piece_name(prefix, lower, diag), "sprite {i}");
                        assert_eq!(
                            set.cell(lower, diag),
                            (i as u32 > set.first).then(|| at(i as u32))
                        );
                        i += 1;
                    }
                }
            }
            assert_eq!(i, (set.first + PIECES) as usize, "{prefix}");
        }
        let boulders = ["boulder_a", "boulder_b", "boulder_c"];
        let snowy = ["snow_boulder_a", "snow_boulder_b", "snow_boulder_c"];
        assert_eq!(&names[BOULDER_0 as usize..], [boulders, snowy].concat());
        assert_eq!(BOULDERS.cells, boulders.map(named));
        assert_eq!(SNOWY_BOULDERS.cells, snowy.map(named));
    }

    #[test]
    fn cliff_pieces_stay_inside_the_cliffs_sheet() {
        let (w, h) = png_size(SHEET);
        let total = 2 * PIECES + 6;
        assert_eq!(sprite_names().len() as u32, total);
        assert_eq!((w, h), (COLUMNS * 16, total.div_ceil(COLUMNS) * 16));
        for i in 0..total {
            let c = at(i);
            assert_eq!((c.sheet, c.w, c.h), (SHEET, 1, 1));
            let [x0, y0, x1, y1] = c.pixel_rect();
            assert!(x0 >= 0.0 && y0 >= 0.0 && x1 <= w as f32 && y1 <= h as f32, "{c:?}");
        }
    }

    #[test]
    fn cliff_cell_picks_every_piece_from_its_lower_neighbors() {
        let set = &ROCK_CLIFF;
        let cases = [
            (N, "rock_n"),
            (S, "rock_s"),
            (E, "rock_e"),
            (W, "rock_w"),
            (N | W, "rock_nw"),
            (N | E, "rock_ne"),
            (S | W, "rock_sw"),
            (S | E, "rock_se"),
            // A ridge one tile wide, running north to south, and its caps.
            (E | W, "rock_ew"),
            (N | E | W, "rock_new"),
            (S | E | W, "rock_sew"),
            (N | S | E | W, "rock_nsew"),
        ];
        for (sides, name) in cases {
            assert_eq!(cliff_at(sides, 0, set), Some(named(name)), "{name}");
        }
    }

    /// A ridge one tile tall keeps both its north rim and its south face, and has a cap at each
    /// end (#104 left it without the rim).
    #[test]
    fn a_one_tall_ridge_has_a_rim_a_face_and_end_caps() {
        let set = &ROCK_CLIFF;
        assert_eq!(cliff_at(N | S, 0, set), Some(named("rock_ns")));
        assert_eq!(cliff_at(N | S | W, 0, set), Some(named("rock_nsw")));
        assert_eq!(cliff_at(N | S | E, 0, set), Some(named("rock_nse")));
        let pieces = [N | S, N | S | W, N | S | E, N, S];
        let cells: Vec<_> = pieces.iter().map(|m| cliff_at(*m, 0, set)).collect();
        for (i, a) in cells.iter().enumerate() {
            assert!(cells[i + 1..].iter().all(|b| a != b), "ridge pieces are all different");
        }
        // A real row: three mountains with lower ground all round.
        let row = terrain_map(5, 3, |p| {
            if p.y == 1 && (1..4).contains(&p.x) { Terrain::Mountain } else { Terrain::Land }
        });
        let at_x = |x| cliff_cell(&row, Pos::new(x, 1), set);
        assert_eq!(at_x(1), Some(named("rock_nsw")));
        assert_eq!(at_x(2), Some(named("rock_ns")));
        assert_eq!(at_x(3), Some(named("rock_nse")));
    }

    #[test]
    fn a_lower_diagonal_with_mountain_sides_is_an_inner_corner() {
        let set = &ROCK_CLIFF;
        for (diag, name) in [
            (NW, "rock_flat_nw"),
            (NE, "rock_flat_ne"),
            (SW, "rock_flat_sw"),
            (SE, "rock_flat_se"),
            (NW | SE, "rock_flat_nw_se"),
            (NW | NE | SW | SE, "rock_flat_nw_ne_sw_se"),
        ] {
            assert_eq!(cliff_at(0, diag, set), Some(named(name)), "{name}");
        }
        // Beside a lower side it can still have the corners on the other side.
        assert_eq!(cliff_at(N, SE, set), Some(named("rock_n_se")));
        assert_eq!(cliff_at(N, SW | SE, set), Some(named("rock_n_sw_se")));
        assert_eq!(cliff_at(S | E, NW, set), Some(named("rock_se_nw")));
        // A diagonal next to a lower side is part of that side's rim: no corner piece.
        assert_eq!(cliff_at(N, NW | NE, set), Some(named("rock_n")));
        assert_eq!(cliff_at(E | W, NW | NE | SW | SE, set), Some(named("rock_ew")));
    }

    #[test]
    fn the_snow_set_has_the_same_pieces() {
        for (sides, diag, name) in
            [(N | S | E, 0, "snow_nse"), (0, SW, "snow_flat_sw"), (S | W, NE, "snow_sw_ne")]
        {
            assert_eq!(cliff_at(sides, diag, &SNOW_CLIFF), Some(named(name)), "{name}");
        }
    }

    /// Mountain on every side stays a flat floor, whatever the sets draw for its corners.
    #[test]
    fn interior_mountain_tiles_have_no_cliff() {
        assert_eq!(cliff_at(0, 0, &ROCK_CLIFF), None);
        assert_eq!(cliff_at(0, 0, &SNOW_CLIFF), None);
    }

    /// Water, coast and ford all count as lower ground.
    #[test]
    fn any_non_mountain_neighbor_is_lower_ground() {
        for lower in [Terrain::Water, Terrain::Coast, Terrain::River, Terrain::Lake, Terrain::Ford]
        {
            let map = terrain_map(1, 2, |p| if p.y == 0 { lower } else { Terrain::Mountain });
            assert_eq!(cliff_cell(&map, Pos::new(0, 1), &ROCK_CLIFF), Some(named("rock_n")));
        }
    }

    /// A range running off the map doesn't grow a false cliff along the edge.
    #[test]
    fn off_map_is_not_lower_ground() {
        let map = terrain_map(1, 1, |_| Terrain::Mountain);
        assert_eq!(cliff_cell(&map, Pos::new(0, 0), &ROCK_CLIFF), None);
    }

    #[test]
    fn boulders_sit_on_the_interior_not_the_cliff() {
        let rule = ground_rule(Terrain::Mountain, "plains").unwrap();
        // A tile that rolls a boulder, in the middle of a tall column of map.
        let p = (1..40).map(|y| Pos::new(1, y)).find(|p| rule.decor_at(*p).is_some()).unwrap();
        let range = terrain_map(3, 42, |_| Terrain::Mountain);
        assert_eq!(rule.decor_on(&range, p), rule.decor_at(p));
        let peak = terrain_map(3, 42, |q| if q == p { Terrain::Mountain } else { Terrain::Land });
        assert_eq!(rule.decor_on(&peak, p), None, "a lone peak is all cliff");
    }

    /// An inner corner is part of the rim, so a boulder doesn't sit on it either.
    #[test]
    fn no_boulder_on_an_inner_corner() {
        let rule = ground_rule(Terrain::Mountain, "plains").unwrap();
        let p = (1..400).map(|y| Pos::new(1, y)).find(|p| rule.decor_at(*p).is_some()).unwrap();
        let notch = terrain_map(3, 402, |q| {
            if q == Pos::new(2, p.y + 1) { Terrain::Land } else { Terrain::Mountain }
        });
        assert!(cliff_cell(&notch, p, &ROCK_CLIFF).is_some());
        assert_eq!(rule.decor_on(&notch, p), None);
    }
}

//! Mountains (#104): autotiled cliffs around a flat interior, and the boulders on it.

use fn_sim::{Pos, Terrain, TileMap};

use super::{Cell, Decor, GroundRule, one};

/// The tileset sheet the cliff art is on.
pub const SHEET: &str = "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetRelief.png";

/// The sheet the boulders are on.
pub const BOULDER_SHEET: &str =
    "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetNature.png";

/// Autotiled mountain cliffs (#104): a raised plateau from `TilesetRelief`, 1 cell per piece. A
/// piece is picked by which cardinal neighbors are lower ground (any non-mountain tile). The sheet
/// has a 3x3 block for a plateau and a 1-wide, 3-tall column for a ridge:
///
/// ```text
/// strip_top |NW   N    NE
/// strip_mid |W    top  E
/// strip_bot |SW   S    SE
/// ```
///
/// Every piece is one tile and the cliff face is painted inside the south pieces (the lower two
/// thirds of `S`, `SW`, `SE` and `strip_bot`), so it never overlaps the row below and needs no
/// depth banding of its own, unlike a tree.
///
/// The sheet has no inner-corner pieces, no end caps for a horizontal ridge and no face-only
/// pieces for a plateau two cells thick, so a tile whose only lower neighbor is diagonal reads as
/// interior, and a ridge one tile tall (lower ground north and south) draws its south face
/// without a north rim.
pub struct CliffSet {
    top: Cell,
    n: Cell,
    s: Cell,
    e: Cell,
    w: Cell,
    nw: Cell,
    ne: Cell,
    sw: Cell,
    se: Cell,
    strip_top: Cell,
    strip_mid: Cell,
    strip_bot: Cell,
}

impl CliffSet {
    /// `col`/`row` is the top-left cell of the 1-wide column; the 3x3 block starts one right.
    const fn new(sheet: &'static str, col: u32, row: u32) -> Self {
        CliffSet {
            strip_top: one(sheet, col, row),
            strip_mid: one(sheet, col, row + 1),
            strip_bot: one(sheet, col, row + 2),
            nw: one(sheet, col + 1, row),
            n: one(sheet, col + 2, row),
            ne: one(sheet, col + 3, row),
            w: one(sheet, col + 1, row + 1),
            top: one(sheet, col + 2, row + 1),
            e: one(sheet, col + 3, row + 1),
            sw: one(sheet, col + 1, row + 2),
            s: one(sheet, col + 2, row + 2),
            se: one(sheet, col + 3, row + 2),
        }
    }

    /// The piece for a mountain tile whose cardinal neighbors are lower ground (`true`) or more
    /// mountain (`false`); `None` for a tile with no lower neighbor, which stays flat floor.
    fn cell(&self, n: bool, s: bool, e: bool, w: bool) -> Option<Cell> {
        Some(match (n, s, e, w) {
            (false, false, false, false) => return None,
            // Lower ground on both sides: a ridge one tile wide, capped where it ends. A lone
            // tile (lower on all four sides) takes the base of a ridge, face included.
            (_, _, true, true) => match (n, s) {
                (false, false) => self.strip_mid,
                (true, false) => self.strip_top,
                (_, true) => self.strip_bot,
            },
            // Anything facing south wears the face: with lower ground north as well (a ridge one
            // tile tall) the north rim is dropped, there is no piece for it.
            (_, true, false, true) => self.sw,
            (_, true, true, false) => self.se,
            (_, true, false, false) => self.s,
            (true, false, false, true) => self.nw,
            (true, false, true, false) => self.ne,
            (true, false, false, false) => self.n,
            (false, false, true, false) => self.e,
            (false, false, false, true) => self.w,
        })
    }
}

/// The cliff piece for a mountain tile at `p` (#104), bitmask-autotiled against its four cardinal
/// neighbors: a neighbor is "lower" unless it is mountain too. Off the edge of the map doesn't
/// count, so a range running off the map doesn't grow a false cliff there. `None` for a tile
/// with no lower neighbor, which is the rule's flat `base`.
pub fn cliff_cell(map: &TileMap, p: Pos, set: &CliffSet) -> Option<Cell> {
    let lower = |dx: i32, dy: i32| {
        map.get(Pos::new(p.x + dx, p.y + dy)).is_some_and(|t| t.terrain != Terrain::Mountain)
    };
    set.cell(lower(0, -1), lower(0, 1), lower(1, 0), lower(-1, 0))
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

/// The pack's two plateaus (`docs/design/art.md`): a grey-faced cliff with a snow top for tundra,
/// and a brown-faced cliff with a sand top for every other biome.
const SNOW_CLIFF: CliffSet = CliffSet::new(SHEET, 0, 0);
const SAND_CLIFF: CliffSet = CliffSet::new(SHEET, 0, 5);

const MOUNTAIN: &[Terrain] = &[Terrain::Mountain];

/// Boulders on a range's flat interior; its rim is a cliff (#104).
const BOULDERS: Decor = Decor { cells: &[one(BOULDER_SHEET, 18, 9)], num: 2, den: 3 };
const SNOWY_BOULDERS: Decor = Decor { cells: &[one(BOULDER_SHEET, 4, 12)], num: 2, den: 3 };

pub const GROUND: &[GroundRule] = &[
    GroundRule {
        terrains: MOUNTAIN,
        biome: Some("tundra"),
        base: SNOW_CLIFF.top,
        variants: &[],
        decor: Some(SNOWY_BOULDERS),
        water: None,
        cliff: Some(&SNOW_CLIFF),
    },
    GroundRule {
        terrains: MOUNTAIN,
        biome: None,
        base: SAND_CLIFF.top,
        variants: &[],
        decor: Some(BOULDERS),
        water: None,
        cliff: Some(&SAND_CLIFF),
    },
];

#[cfg(test)]
mod tests {
    use super::super::ground_rule;
    use super::super::test_support::{TERRAINS, png_size, terrain_map};
    use super::*;
    use fn_sim::Defs;

    /// A 3x3 map whose center is a mountain and whose cardinal neighbors are lower ground
    /// (`true`, land) or more mountain (`false`). Corners are mountain.
    fn mountain_neighbor_map(n: bool, s: bool, e: bool, w: bool) -> TileMap {
        let at = |lower: bool| if lower { Terrain::Land } else { Terrain::Mountain };
        terrain_map(3, 3, |p| match (p.x, p.y) {
            (1, 0) => at(n),
            (1, 2) => at(s),
            (2, 1) => at(e),
            (0, 1) => at(w),
            _ => Terrain::Mountain,
        })
    }

    #[test]
    fn about_two_in_three_mountain_tiles_get_a_boulder() {
        let rule = ground_rule(Terrain::Mountain, "plains").unwrap();
        let n = (0..900).filter(|i| rule.decor_at(Pos::new(i % 30, i / 30)).is_some()).count();
        assert!((540..660).contains(&n), "{n}");
    }

    #[test]
    fn every_mountain_is_a_cliff_on_the_relief_sheet() {
        let defs = Defs::bundled();
        for biome in &defs.biomes {
            let rule = ground_rule(Terrain::Mountain, &biome.id).unwrap();
            let set = rule.cliff.unwrap_or_else(|| panic!("{} mountain has no cliff", biome.id));
            assert_eq!(rule.base, set.top, "{}: interior blends into the rim", biome.id);
            assert!(rule.decor.is_some(), "{}: interior keeps its boulders", biome.id);
        }
        for terrain in TERRAINS.iter().filter(|t| **t != Terrain::Mountain) {
            let rule = ground_rule(*terrain, "plains").unwrap();
            assert!(rule.cliff.is_none(), "{terrain:?}");
        }
    }

    #[test]
    fn tundra_cliffs_are_snowy_and_the_rest_are_sandy() {
        let snow = ground_rule(Terrain::Mountain, "tundra").unwrap().cliff.unwrap();
        let sand = ground_rule(Terrain::Mountain, "plains").unwrap().cliff.unwrap();
        assert_eq!(snow.top, one(SHEET, 2, 1));
        assert_eq!(sand.top, one(SHEET, 2, 6));
        assert_eq!(ground_rule(Terrain::Mountain, "desert").unwrap().cliff.unwrap().top, sand.top);
    }

    #[test]
    fn cliff_pieces_stay_inside_the_relief_sheet() {
        let (w, h) = png_size(SHEET);
        for set in [&SNOW_CLIFF, &SAND_CLIFF] {
            let pieces = [
                set.top,
                set.n,
                set.s,
                set.e,
                set.w,
                set.nw,
                set.ne,
                set.sw,
                set.se,
                set.strip_top,
                set.strip_mid,
                set.strip_bot,
            ];
            for c in pieces {
                assert_eq!((c.sheet, c.w, c.h), (SHEET, 1, 1));
                let [x0, y0, x1, y1] = c.pixel_rect();
                assert!(x0 >= 0.0 && y0 >= 0.0 && x1 <= w as f32 && y1 <= h as f32, "{c:?}");
            }
        }
    }

    #[test]
    fn cliff_cell_picks_every_piece_from_its_lower_neighbors() {
        let set = &SAND_CLIFF;
        let cases = [
            ((true, false, false, false), Some(set.n)),
            ((false, true, false, false), Some(set.s)),
            ((false, false, true, false), Some(set.e)),
            ((false, false, false, true), Some(set.w)),
            ((true, false, false, true), Some(set.nw)),
            ((true, false, true, false), Some(set.ne)),
            ((false, true, false, true), Some(set.sw)),
            ((false, true, true, false), Some(set.se)),
            // A ridge one tile wide, running north to south.
            ((false, false, true, true), Some(set.strip_mid)),
            ((true, false, true, true), Some(set.strip_top)),
            ((false, true, true, true), Some(set.strip_bot)),
            ((true, true, true, true), Some(set.strip_bot)),
            // A ridge one tile tall keeps its south face.
            ((true, true, false, false), Some(set.s)),
            ((true, true, false, true), Some(set.sw)),
            ((true, true, true, false), Some(set.se)),
        ];
        for ((n, s, e, w), expected) in cases {
            let map = mountain_neighbor_map(n, s, e, w);
            assert_eq!(cliff_cell(&map, Pos::new(1, 1), set), expected, "n={n} s={s} e={e} w={w}");
        }
    }

    /// Mountain on every side, even with lower ground on the diagonals, stays a flat floor.
    #[test]
    fn interior_mountain_tiles_have_no_cliff() {
        let map = mountain_neighbor_map(false, false, false, false);
        assert_eq!(cliff_cell(&map, Pos::new(1, 1), &SAND_CLIFF), None);
        let diagonals =
            terrain_map(
                3,
                3,
                |p| {
                    if p.x != 1 && p.y != 1 { Terrain::Land } else { Terrain::Mountain }
                },
            );
        assert_eq!(cliff_cell(&diagonals, Pos::new(1, 1), &SAND_CLIFF), None);
    }

    /// Water, coast and ford all count as lower ground.
    #[test]
    fn any_non_mountain_neighbor_is_lower_ground() {
        for lower in [Terrain::Water, Terrain::Coast, Terrain::River, Terrain::Lake, Terrain::Ford]
        {
            let map = terrain_map(1, 2, |p| if p.y == 0 { lower } else { Terrain::Mountain });
            assert_eq!(cliff_cell(&map, Pos::new(0, 1), &SAND_CLIFF), Some(SAND_CLIFF.n));
        }
    }

    /// A range running off the map doesn't grow a false cliff along the edge.
    #[test]
    fn off_map_is_not_lower_ground() {
        let map = terrain_map(1, 1, |_| Terrain::Mountain);
        assert_eq!(cliff_cell(&map, Pos::new(0, 0), &SAND_CLIFF), None);
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
}

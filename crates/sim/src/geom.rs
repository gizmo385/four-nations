//! Grid coordinates and a dense 2D grid container.

/// A tile coordinate. `(0, 0)` is the top-left corner; `y` grows downward.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Chebyshev distance: the number of king's moves between two tiles.
    pub fn chebyshev(self, other: Pos) -> i32 {
        (self.x - other.x).abs().max((self.y - other.y).abs())
    }
}

/// The rectangle of tiles a building covers (docs/design/buildings.md, #207): `w` by `h` tiles
/// with `anchor` as its top-left corner. Every walk over it (`tiles`, `ring`) is row-major, so
/// anything that picks among them breaks ties the same way on every machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Footprint {
    pub anchor: Pos,
    pub w: u8,
    pub h: u8,
}

impl Footprint {
    pub const fn new(anchor: Pos, w: u8, h: u8) -> Self {
        Self { anchor, w, h }
    }

    /// A 1x1 footprint on `pos`.
    pub const fn single(pos: Pos) -> Self {
        Self::new(pos, 1, 1)
    }

    /// The tiles it covers, row-major from the anchor.
    pub fn tiles(self) -> impl Iterator<Item = Pos> {
        let Pos { x, y } = self.anchor;
        let (w, h) = (i32::from(self.w), i32::from(self.h));
        (y..y + h).flat_map(move |ty| (x..x + w).map(move |tx| Pos::new(tx, ty)))
    }

    pub fn contains(self, p: Pos) -> bool {
        let (dx, dy) = (p.x - self.anchor.x, p.y - self.anchor.y);
        (0..i32::from(self.w)).contains(&dx) && (0..i32::from(self.h)).contains(&dy)
    }

    /// The tiles touching it from outside, corners included, row-major. For a 1x1 footprint
    /// these are the same eight tiles as `path::NEIGHBORS`. Tiles off the map are included; the
    /// caller filters them.
    pub fn ring(self) -> impl Iterator<Item = Pos> {
        let Pos { x, y } = self.anchor;
        let (w, h) = (i32::from(self.w), i32::from(self.h));
        (y - 1..=y + h)
            .flat_map(move |ty| (x - 1..=x + w).map(move |tx| Pos::new(tx, ty)))
            .filter(move |&p| !self.contains(p))
    }

    /// Chebyshev distance from `p` to the nearest tile it covers: 0 inside, 1 on the ring.
    pub fn distance(self, p: Pos) -> i32 {
        let gap = |v: i32, lo: i32, len: u8| (lo - v).max(v - (lo + i32::from(len) - 1)).max(0);
        gap(p.x, self.anchor.x, self.w).max(gap(p.y, self.anchor.y, self.h))
    }
}

const CARDINAL: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];
const DIAGONAL: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

/// A dense, row-major 2D grid.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Grid<T> {
    width: i32,
    height: i32,
    cells: Vec<T>,
}

impl<T> Grid<T> {
    /// Do the dimensions agree with the number of cells? Always true for a grid built here; a
    /// grid read from a save file might not be.
    pub fn is_well_formed(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.cells.len() == self.width as usize * self.height as usize
    }

    pub fn from_fn(width: i32, height: i32, mut f: impl FnMut(Pos) -> T) -> Self {
        assert!(width > 0 && height > 0, "grid dimensions must be positive");
        let cells =
            (0..height).flat_map(|y| (0..width).map(move |x| Pos::new(x, y))).map(&mut f).collect();
        Self { width, height, cells }
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn in_bounds(&self, p: Pos) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height
    }

    fn index(&self, p: Pos) -> usize {
        (p.y * self.width + p.x) as usize
    }

    pub fn get(&self, p: Pos) -> Option<&T> {
        self.in_bounds(p).then(|| &self.cells[self.index(p)])
    }

    pub fn get_mut(&mut self, p: Pos) -> Option<&mut T> {
        if self.in_bounds(p) {
            let i = self.index(p);
            Some(&mut self.cells[i])
        } else {
            None
        }
    }

    /// All positions in row-major order.
    pub fn positions(&self) -> impl Iterator<Item = Pos> + use<T> {
        let (w, h) = (self.width, self.height);
        (0..h).flat_map(move |y| (0..w).map(move |x| Pos::new(x, y)))
    }

    pub fn iter(&self) -> impl Iterator<Item = (Pos, &T)> {
        self.positions().zip(self.cells.iter())
    }

    /// In-bounds neighbors of `p`: 4-connected, plus diagonals when `diagonal` is true.
    pub fn neighbors(&self, p: Pos, diagonal: bool) -> impl Iterator<Item = Pos> + '_ {
        let extra: &[(i32, i32)] = if diagonal { &DIAGONAL } else { &[] };
        CARDINAL
            .iter()
            .chain(extra)
            .map(move |(dx, dy)| Pos::new(p.x + dx, p.y + dy))
            .filter(|n| self.in_bounds(*n))
    }

    /// Builds a new grid by applying `f` to every cell. `f` sees the whole *old* grid, so
    /// neighbor reads are unaffected by earlier writes in the same pass.
    pub fn map_with<U>(&self, mut f: impl FnMut(Pos, &T) -> U) -> Grid<U> {
        Grid::from_fn(self.width, self.height, |p| f(p, &self[p]))
    }
}

impl<T> std::ops::Index<Pos> for Grid<T> {
    type Output = T;
    fn index(&self, p: Pos) -> &T {
        self.get(p).unwrap_or_else(|| panic!("{p:?} out of bounds"))
    }
}

impl<T> std::ops::IndexMut<Pos> for Grid<T> {
    fn index_mut(&mut self, p: Pos) -> &mut T {
        self.get_mut(p).unwrap_or_else(|| panic!("{p:?} out of bounds"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corner_has_three_neighbors_with_diagonals() {
        let g = Grid::from_fn(4, 3, |_| 0);
        assert_eq!(g.neighbors(Pos::new(0, 0), true).count(), 3);
        assert_eq!(g.neighbors(Pos::new(0, 0), false).count(), 2);
        assert_eq!(g.neighbors(Pos::new(1, 1), true).count(), 8);
    }

    #[test]
    fn positions_are_row_major() {
        let g = Grid::from_fn(3, 2, |p| p);
        let v: Vec<_> = g.iter().map(|(p, v)| (p, *v)).collect();
        assert!(v.iter().all(|(p, v)| p == v));
        assert_eq!(v[3].0, Pos::new(0, 1));
    }

    #[test]
    fn footprint_tiles_are_row_major_from_the_anchor() {
        let f = Footprint::new(Pos::new(2, 5), 3, 2);
        let tiles: Vec<_> = f.tiles().collect();
        let want: Vec<_> =
            [(2, 5), (3, 5), (4, 5), (2, 6), (3, 6), (4, 6)].map(|(x, y)| Pos::new(x, y)).into();
        assert_eq!(tiles, want);
        assert!(tiles.iter().all(|&p| f.contains(p)));
        for outside in [(1, 5), (5, 5), (2, 4), (2, 7), (5, 7)] {
            assert!(!f.contains(Pos::new(outside.0, outside.1)), "{outside:?}");
        }
        assert_eq!(Footprint::single(Pos::new(0, 0)).tiles().count(), 1);
    }

    #[test]
    fn footprint_ring_surrounds_it_row_major_with_corners() {
        let f = Footprint::new(Pos::new(1, 1), 2, 2);
        let ring: Vec<_> = f.ring().collect();
        let want: Vec<_> = [
            (0, 0),
            (1, 0),
            (2, 0),
            (3, 0),
            (0, 1),
            (3, 1),
            (0, 2),
            (3, 2),
            (0, 3),
            (1, 3),
            (2, 3),
            (3, 3),
        ]
        .map(|(x, y)| Pos::new(x, y))
        .into();
        assert_eq!(ring, want);
        assert!(ring.iter().all(|&p| f.distance(p) == 1));

        // A 1x1 ring is the eight neighbours `path::NEIGHBORS` steps to.
        let one = Footprint::single(Pos::new(5, 5));
        let mut ring: Vec<_> = one.ring().collect();
        let mut neighbours: Vec<_> =
            crate::path::NEIGHBORS.map(|(dx, dy)| Pos::new(5 + dx, 5 + dy)).into();
        assert!(ring.is_sorted_by_key(|p| (p.y, p.x)), "row-major");
        ring.sort();
        neighbours.sort();
        assert_eq!(ring, neighbours);
    }

    #[test]
    fn footprint_distance_is_chebyshev_to_the_rectangle() {
        let f = Footprint::new(Pos::new(4, 4), 2, 3);
        assert!(f.tiles().all(|p| f.distance(p) == 0));
        assert_eq!(f.distance(Pos::new(3, 3)), 1);
        assert_eq!(f.distance(Pos::new(7, 5)), 2);
        assert_eq!(f.distance(Pos::new(5, 10)), 4);
        assert_eq!(f.distance(Pos::new(0, 0)), 4);
        // On a 1x1 footprint it is plain Chebyshev distance.
        let p = Pos::new(9, 2);
        assert_eq!(Footprint::single(p).distance(Pos::new(6, 7)), p.chebyshev(Pos::new(6, 7)));
    }
}

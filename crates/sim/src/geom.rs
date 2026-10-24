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
}

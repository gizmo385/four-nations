//! The words and the shape of the legend (the key help's second tab, `?`, #662): the groups the
//! map's glyphs are listed under, and how the entries fill rows of a given width.
//!
//! What the entries are (which tile shows which glyph, in which colours) is the frontend's: the
//! legend is built from the cells the map in view is drawn from (`fn_tui::legend`), so that part
//! stays there. This is the part no frontend decides, and it holds no look at all, so a window
//! that wraps its own text and the terminal's cells name the same groups in the same order.

/// Where the legend lists a glyph, in the order it lists the groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegendGroup {
    People,
    Plants,
    Terrain,
    Buildings,
    Marks,
}

impl LegendGroup {
    pub const ALL: [LegendGroup; 5] =
        [Self::People, Self::Plants, Self::Terrain, Self::Buildings, Self::Marks];

    pub fn title(self) -> &'static str {
        match self {
            Self::People => "People",
            Self::Plants => "Plants",
            Self::Terrain => "Terrain",
            Self::Buildings => "Buildings and goods",
            Self::Marks => "Marks",
        }
    }
}

/// A row of the legend page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LegendLine {
    Heading(LegendGroup),
    Blank,
    /// Indices into the entries, one per column.
    Cells(Vec<usize>),
}

/// The space between two columns of cells.
pub const GAP: usize = 3;

/// How wide a legend cell is: the glyph, a space and the longest name (`longest`, in cells).
pub fn cell_width(longest: usize) -> usize {
    2 + longest
}

/// The legend as rows `width` cells wide: each group's heading, then its entries filling rows
/// left to right in columns of `cell` cells, and a blank row between groups. `groups[i]` is
/// the group entry `i` is listed in; an entry's index is what [`LegendLine::Cells`] holds.
pub fn layout(groups: &[LegendGroup], cell: usize, width: usize) -> Vec<LegendLine> {
    let per_row = ((width + GAP) / (cell + GAP)).max(1);
    let mut out = Vec::new();
    for group in LegendGroup::ALL {
        let members: Vec<usize> = (0..groups.len()).filter(|&i| groups[i] == group).collect();
        if members.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(LegendLine::Blank);
        }
        out.push(LegendLine::Heading(group));
        out.extend(members.chunks(per_row).map(|row| LegendLine::Cells(row.to_vec())));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_lists_each_group_once_in_order_with_a_blank_between() {
        let groups = [LegendGroup::Marks, LegendGroup::People, LegendGroup::People];
        let lines = layout(&groups, cell_width(6), 100);
        assert_eq!(
            lines,
            [
                LegendLine::Heading(LegendGroup::People),
                LegendLine::Cells(vec![1, 2]),
                LegendLine::Blank,
                LegendLine::Heading(LegendGroup::Marks),
                LegendLine::Cells(vec![0]),
            ]
        );
    }

    #[test]
    fn a_narrow_width_gives_one_cell_a_row() {
        let groups = [LegendGroup::Plants; 3];
        let lines = layout(&groups, cell_width(6), 1);
        let cells: Vec<_> = lines.iter().filter(|l| matches!(l, LegendLine::Cells(_))).collect();
        assert_eq!(cells.len(), 3);
    }
}

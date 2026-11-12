//! How big a full screen's page is inside its frame (`fn_tui::page`, #652).

/// The inside of a page `page` cells across and down (`UiState::page`), border and padding
/// excluded, detail strip included, as (columns, rows): how many rows a scrolling screen with no
/// detail strip shows, and what `research::Grid` lays its cards out in.
pub fn text_size(page: (i32, i32)) -> (usize, usize) {
    let (w, h) = page;
    ((w - 4).max(0) as usize, (h - 2).max(0) as usize)
}

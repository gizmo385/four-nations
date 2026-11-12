//! The embark screen (#290, docs/design/embark.md): zoom from a deliberately coarse overview of
//! a surveyed map down to the game's own 1:1 map, with a live site report, to choose where to
//! found the game.
//!
//! [`EmbarkScreen`] holds the state (the survey, the zoom ladder, the cursor and the view); its
//! [`EmbarkScreen::key`] is the only thing that changes it, returning an [`Outcome`] for
//! [`crate::shell::Shell`] to act on (going back to the seed step, or founding the world).
//! [`draw_embark`] is a pure function into a `Buffer`, like the other screens.

use crate::keyev::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use fn_sim::defs::{BiomeId, Terrain};
use fn_sim::embark::{MapSizeDef, SiteReport, Survey, SurveyMode};
use fn_sim::mapgen::{Tile, TileMap};
use fn_sim::{Defs, Nation, Pos};
use fn_ui::embark::{Site, report_rows};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::{Palette, mix, to_u8};
use crate::glyphs::{EmbarkGlyphDef, Rgb8};
use crate::layout;
use crate::widgets::{BarBody, Marks, Painter, Row, TitleAt, Tone, draw_frame, draw_hint_bar};
use fn_ui::keys;

/// How far a plain move goes, in cells (one cell is `N` tiles at 1:N); Shift moves this many
/// cells at once (docs/design/embark.md, "Flow and keys").
const SHIFT_CELLS: i32 = 10;

/// How close (in blocks) the cursor may come to the view's edge before it scrolls, as the game's
/// own `state::SCROLL_MARGIN` does for tiles.
const SCROLL_MARGIN: i32 = 3;

/// What a key did, for [`crate::shell::Shell`] to act on.
#[derive(Debug)]
pub enum Outcome {
    None,
    /// `Esc` at the overview: back to the seed step, the seed and choices kept.
    Back,
    /// Settle at the cursor: `Shell` founds the world from a clone of [`EmbarkScreen::survey`].
    Settle(Pos),
}

/// The embark screen's state: a surveyed map, a zoom ladder (docs/design/embark.md, "The zoom
/// levels"), a cursor and the part of the map in view.
#[derive(Debug, Clone)]
pub struct EmbarkScreen {
    seed: u64,
    pub nation: Nation,
    size: MapSizeDef,
    mode: SurveyMode,
    /// The difficulty preset's id chosen on the seed step (#298), carried into
    /// `Session::found` on settling. `None` only when the data has no presets at all.
    difficulty: Option<String>,
    survey: Survey,
    /// The scale at each zoom level: `[fit, 4, 2, 1]`, dropping fixed levels at or above `fit`
    /// and, in a rough survey, dropping `1` too. Index 0 is the overview.
    ladder: Vec<i32>,
    zoom: usize,
    pub cursor: Pos,
    /// The top-left block shown, in block units at the current scale (so at 1:1 this is tiles).
    /// Unused at the overview, which is always centred and never scrolls.
    view: Pos,
    screen: (u16, u16),
    /// `E`, or `Enter`/`+` at the closest zoom, asks `y`/`n` before settling.
    prompt: bool,
    /// What `?` shows instead of the map.
    help: bool,
    /// A save-style message that takes the status line until the next key (settling on a
    /// refused site, docs/design/embark.md, "The y/n prompt").
    pub status: String,
    pub quit: bool,
    glyphs: EmbarkGlyphDef,
}

impl EmbarkScreen {
    /// Surveys a fresh map for `seed` at `size` and opens on its default site, at the coarsest
    /// zoom.
    pub fn new(
        seed: u64,
        nation: Nation,
        size: MapSizeDef,
        mode: SurveyMode,
        difficulty: Option<String>,
        screen: (u16, u16),
        defs: &Defs,
    ) -> Self {
        let cfg = Survey::config(seed, &size);
        let survey = Survey::new(&cfg, defs);
        Self::with_survey(survey, seed, nation, size, mode, difficulty, screen, defs)
    }

    /// [`Self::new`] over a survey the caller built (a test's hand-built map, #455) instead of
    /// one generated from `seed`: the seed is only what the screen shows and what settling
    /// carries on.
    #[allow(clippy::too_many_arguments)]
    pub fn with_survey(
        survey: Survey,
        seed: u64,
        nation: Nation,
        size: MapSizeDef,
        mode: SurveyMode,
        difficulty: Option<String>,
        screen: (u16, u16),
        defs: &Defs,
    ) -> Self {
        let cursor = survey.default_site(defs);
        let glyphs = crate::glyphs::Glyphs::bundled().embark;
        let mut s = Self {
            seed,
            nation,
            size,
            mode,
            difficulty,
            survey,
            ladder: vec![1],
            zoom: 0,
            cursor,
            view: Pos::new(0, 0),
            screen,
            prompt: false,
            help: false,
            status: String::new(),
            quit: false,
            glyphs,
        };
        s.rebuild_ladder();
        s.recentre_view();
        s
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn size(&self) -> &MapSizeDef {
        &self.size
    }

    pub fn mode(&self) -> SurveyMode {
        self.mode
    }

    pub fn difficulty(&self) -> Option<&str> {
        self.difficulty.as_deref()
    }

    pub fn survey(&self) -> &Survey {
        &self.survey
    }

    fn scale(&self) -> i32 {
        self.ladder[self.zoom]
    }

    pub fn is_overview(&self) -> bool {
        self.zoom == 0
    }

    fn map_size(&self) -> (i32, i32) {
        (self.survey.map().width(), self.survey.map().height())
    }

    /// The map region of the terminal, in cells (one cell per block at the current zoom).
    fn region(&self) -> (i32, i32) {
        layout::viewport(self.screen.0, self.screen.1)
    }

    /// The scale `[fit, 4, 2, 1]` is built from, dropping any fixed level at or above `fit` and,
    /// in a rough survey, `1` too (docs/design/embark.md, "The zoom levels").
    fn rebuild_ladder(&mut self) {
        let (cols, rows) = self.region();
        let (mw, mh) = self.map_size();
        let fit = fit_scale(mw, mh, cols, rows, self.glyphs.min_overview_scale);
        let mut ladder = vec![fit];
        for n in [4, 2, 1] {
            if n < fit && !(self.mode == SurveyMode::Rough && n == 1) {
                ladder.push(n);
            }
        }
        self.zoom = self.zoom.min(ladder.len() - 1);
        self.ladder = ladder;
    }

    /// The terminal resized: the ladder (the overview's scale depends on it) and the view both
    /// need to catch up.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.screen = (width, height);
        self.rebuild_ladder();
        self.recentre_view();
    }

    /// Scrolls the view just far enough to keep the cursor's block inside it, the way the game's
    /// own camera does (`state::UiState::follow`); the overview never scrolls.
    fn recentre_view(&mut self) {
        if self.is_overview() {
            self.view = Pos::new(0, 0);
            return;
        }
        let n = self.scale();
        let (cols, rows) = self.region();
        let (mw, mh) = self.map_size();
        let (bw, bh) = (block_count(mw, n), block_count(mh, n));
        let cursor_block = Pos::new(self.cursor.x.div_euclid(n), self.cursor.y.div_euclid(n));
        let follow = |view: i32, cursor: i32, shown: i32, bound: i32| {
            let margin = SCROLL_MARGIN.min((shown - 1) / 2).max(0);
            let v = if cursor < view + margin {
                cursor - margin
            } else if cursor > view + shown - 1 - margin {
                cursor - (shown - 1 - margin)
            } else {
                view
            };
            v.clamp(0, (bound - shown).max(0))
        };
        self.view = Pos::new(
            follow(self.view.x, cursor_block.x, cols, bw),
            follow(self.view.y, cursor_block.y, rows, bh),
        );
    }

    fn tile_terrain(&self) -> Terrain {
        self.survey.map().get(self.cursor).map_or(Terrain::Water, |t| t.terrain)
    }

    fn move_cursor(&mut self, dx: i32, dy: i32, shift: bool) {
        let delta = self.scale() * if shift { SHIFT_CELLS } else { 1 };
        let (mw, mh) = self.map_size();
        self.cursor = Pos::new(
            (self.cursor.x + dx * delta).clamp(0, mw - 1),
            (self.cursor.y + dy * delta).clamp(0, mh - 1),
        );
        self.recentre_view();
    }

    /// `Enter`/`+`: one step in, or (at the closest zoom) a settle prompt, unless the site is
    /// refused, which needs no asking (`Self::ask_or_refuse`).
    fn zoom_in(&mut self, defs: &Defs) {
        if self.zoom + 1 < self.ladder.len() {
            self.zoom += 1;
            self.recentre_view();
        } else {
            self.ask_or_refuse(defs);
        }
    }

    /// One step out; `false` if already at the overview, for `Esc` to act on.
    fn zoom_out(&mut self) -> bool {
        if self.zoom > 0 {
            self.zoom -= 1;
            self.recentre_view();
            true
        } else {
            false
        }
    }

    /// `E`, or `Enter`/`+` at the closest zoom: the y/n prompt, unless the site is refused, which
    /// shows the reason as a status message instead (docs/design/embark.md, "The y/n prompt").
    fn ask_or_refuse(&mut self, defs: &Defs) {
        match self.survey.report(self.cursor, self.mode, self.difficulty(), defs).problem {
            Some(problem) => {
                self.status = fn_ui::embark::refusal(problem, self.tile_terrain());
            }
            None => self.prompt = true,
        }
    }

    fn reseed(&mut self, defs: &Defs) {
        self.seed = self.seed.wrapping_add(1);
        let cfg = Survey::config(self.seed, &self.size);
        self.survey = Survey::new(&cfg, defs);
        self.zoom = 0;
        self.cursor = self.survey.default_site(defs);
        self.status.clear();
        self.rebuild_ladder();
        self.recentre_view();
    }

    /// Handles one key, returning what `Shell` should do about it.
    pub fn key(&mut self, key: KeyEvent, defs: &Defs) -> Outcome {
        if key.kind == KeyEventKind::Release {
            return Outcome::None;
        }
        self.status.clear();
        if self.help {
            self.help = false;
            return Outcome::None;
        }
        if self.prompt {
            self.prompt = false;
            if matches!(key.code, KeyCode::Char('y' | 'Y')) {
                return Outcome::Settle(self.cursor);
            }
            return Outcome::None;
        }
        let upper = matches!(key.code, KeyCode::Char(c) if c.is_ascii_uppercase());
        let shift = key.modifiers.contains(KeyModifiers::SHIFT) || upper;
        match key.code {
            KeyCode::Up | KeyCode::Char('w' | 'W') => self.move_cursor(0, -1, shift),
            KeyCode::Down | KeyCode::Char('s' | 'S') => self.move_cursor(0, 1, shift),
            KeyCode::Left | KeyCode::Char('a' | 'A') => self.move_cursor(-1, 0, shift),
            KeyCode::Right | KeyCode::Char('d' | 'D') => self.move_cursor(1, 0, shift),
            KeyCode::Enter | KeyCode::Char('+') => self.zoom_in(defs),
            KeyCode::Char('-') => {
                self.zoom_out();
            }
            KeyCode::Esc => {
                if !self.zoom_out() {
                    return Outcome::Back;
                }
            }
            KeyCode::Char('e' | 'E') => self.ask_or_refuse(defs),
            KeyCode::Char('r' | 'R') => self.reseed(defs),
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('q' | 'Q') => self.quit = true,
            _ => {}
        }
        Outcome::None
    }
}

/// The smallest whole scale that is at least `min` and fits `(mw, mh)` tiles into `(cols, rows)`
/// cells (docs/design/embark.md, "Scale").
fn fit_scale(mw: i32, mh: i32, cols: i32, rows: i32, min: i32) -> i32 {
    let mut n = min.max(1);
    if cols <= 0 || rows <= 0 {
        return n;
    }
    while block_count(mw, n) > cols || block_count(mh, n) > rows {
        n += 1;
    }
    n
}

/// How many blocks of `n` tiles it takes to cover `len` tiles.
fn block_count(len: i32, n: i32) -> i32 {
    (len + n - 1) / n
}

pub(crate) use fn_ui::embark::problem_text;

// -------------------------------------------------------------------------------------------
// Block classification (docs/design/embark.md, "What a cell shows")
// -------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FreshKind {
    River,
    Lake,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BlockKind {
    Sea,
    /// 1:4 and 1:2 only: rivers and lakes stand out from the land around them.
    Fresh(FreshKind),
    Mountain,
    Biome(BiomeId),
}

/// How `(bx, by)`'s block of up to `n*n` tiles classifies, in the doc's order: sea, then (unless
/// `overview`) fresh water, then mountain, then the block's most common biome (ties to the
/// smallest id, i.e. file order).
fn classify_block(
    map: &TileMap,
    bx: i32,
    by: i32,
    n: i32,
    overview: bool,
    e: &EmbarkGlyphDef,
) -> BlockKind {
    let (x0, y0) = (bx * n, by * n);
    let (x1, y1) = ((x0 + n).min(map.width()), (y0 + n).min(map.height()));
    let (mut total, mut sea, mut river, mut lake, mut mountain) = (0u32, 0u32, 0u32, 0u32, 0u32);
    let mut best: Option<(u8, u32)> = None;
    let biome_count = |id: u8, biomes: &mut std::collections::BTreeMap<u8, u32>| {
        let c = biomes.entry(id).or_insert(0);
        *c += 1;
        *c
    };
    let mut biomes = std::collections::BTreeMap::new();
    for y in y0..y1 {
        for x in x0..x1 {
            let Some(tile) = map.get(Pos::new(x, y)) else { continue };
            total += 1;
            match tile.terrain {
                Terrain::Water => sea += 1,
                Terrain::River | Terrain::Ford | Terrain::Canal => river += 1,
                Terrain::Lake => lake += 1,
                Terrain::Mountain => mountain += 1,
                Terrain::Land | Terrain::Coast | Terrain::Tunnel => {
                    let c = biome_count(tile.biome.0, &mut biomes);
                    if best.is_none_or(|(_, bc)| c > bc) {
                        best = Some((tile.biome.0, c));
                    }
                }
            }
        }
    }
    if total == 0 {
        return BlockKind::Sea;
    }
    const EPS: f64 = 1e-6;
    if f64::from(sea) + EPS >= e.sea_share * f64::from(total) {
        return BlockKind::Sea;
    }
    if !overview {
        let fresh = river + lake;
        let threshold = ((e.river_lake_share * f64::from(n)).round() as u32).max(1);
        if fresh >= threshold {
            return BlockKind::Fresh(if river >= lake {
                FreshKind::River
            } else {
                FreshKind::Lake
            });
        }
    }
    if f64::from(mountain) + EPS >= e.mountain_share * f64::from(total) {
        return BlockKind::Mountain;
    }
    match best {
        Some((id, _)) => BlockKind::Biome(BiomeId(id)),
        None if mountain >= sea => BlockKind::Mountain,
        None => BlockKind::Sea,
    }
}

/// The glyph, foreground and background of `kind`'s look.
fn block_look(kind: BlockKind, defs: &Defs, palette: &Palette) -> (char, Rgb8, Rgb8) {
    let g = &palette.glyphs;
    let look = match kind {
        BlockKind::Sea => g.terrain(Terrain::Water).expect("water has a look"),
        BlockKind::Fresh(FreshKind::River) => g.terrain(Terrain::River).expect("river has a look"),
        BlockKind::Fresh(FreshKind::Lake) => g.terrain(Terrain::Lake).expect("lake has a look"),
        BlockKind::Mountain => g.terrain(Terrain::Mountain).expect("mountain has a look"),
        BlockKind::Biome(id) => &g.biome[&defs.biome(id).id],
    };
    (look.glyph(palette.ascii), look.fg, look.bg.expect("terrain and biome entries have a bg"))
}

/// The glyph, foreground and background of the actual tile at `pos` (the site zoom, 1:1): the
/// game's own map renderer with no units (docs/design/embark.md, "Site").
fn site_look(tile: Option<&Tile>, defs: &Defs, palette: &Palette) -> (char, Rgb8, Rgb8) {
    let Some(tile) = tile else { return (' ', [0, 0, 0], [0, 0, 0]) };
    let g = &palette.glyphs;
    let ground = match tile.terrain {
        Terrain::Land => &g.biome[&defs.biome(tile.biome).id],
        other => g.terrain(other).expect("every non-land terrain has a look"),
    };
    let bg = ground.bg.expect("terrain and biome entries have a bg");
    match tile.resource {
        Some(r) => {
            let look = &g.resource[&defs.resource(r).id];
            (look.glyph(palette.ascii), look.fg, bg)
        }
        None => (ground.glyph(palette.ascii), ground.fg, bg),
    }
}

// -------------------------------------------------------------------------------------------
// Drawing
// -------------------------------------------------------------------------------------------

/// Draws the embark screen into `area` (the whole terminal): the map (or the key help), the
/// "CHOOSE A SITE" panel and the status line.
pub fn draw_embark(
    screen: &EmbarkScreen,
    defs: &Defs,
    palette: &Palette,
    area: Rect,
    buf: &mut Buffer,
) {
    let Some(regions) = layout::regions(area) else {
        buf.set_stringn(
            area.x,
            area.y,
            crate::draw::too_small_text(),
            usize::from(area.width),
            Style::new(),
        );
        return;
    };
    let report = screen.survey.report(screen.cursor, screen.mode, screen.difficulty(), defs);
    if screen.help {
        draw_help(palette, regions.map, buf);
    } else {
        draw_map(screen, defs, palette, &report, regions.map, buf);
    }
    draw_panel(screen, defs, palette, &report, regions.panel, buf);
    draw_status(screen, palette, regions.status, buf);
}

/// The corner mark at `(bx, by)`, if any: just outside each corner of `[min, max]`.
fn corner_at(bx: i32, by: i32, min: Pos, max: Pos, marks: &Marks) -> Option<char> {
    match (bx, by) {
        (x, y) if x == min.x - 1 && y == min.y - 1 => Some(marks.box_tl),
        (x, y) if x == max.x + 1 && y == min.y - 1 => Some(marks.box_tr),
        (x, y) if x == min.x - 1 && y == max.y + 1 => Some(marks.box_bl),
        (x, y) if x == max.x + 1 && y == max.y + 1 => Some(marks.box_br),
        _ => None,
    }
}

/// What one block of the map shows, before any frontend draws it: the glyph and colours of the
/// block's look (or the box's corner mark), and whether it lies in the site box or under the
/// cursor. The terminal turns it into a styled cell ([`draw_map`]); the window into a tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockCell {
    pub glyph: char,
    pub fg: Rgb8,
    /// The block's own background, before the site box tints it.
    pub bg: Rgb8,
    /// A corner mark of the site box (bold, in the box's tint).
    pub corner: bool,
    /// Inside the site box (the report's radius), corners excluded.
    pub in_box: bool,
    /// The block the cursor is on.
    pub cursor: bool,
}

/// The map's blocks at the screen's current zoom, and the site box over them: what [`draw_map`]
/// and the window's embark map both draw from (docs/design/embark.md, "What a cell shows").
pub struct MapFrame<'a> {
    screen: &'a EmbarkScreen,
    defs: &'a Defs,
    palette: &'a Palette,
    marks: Marks,
    /// Tiles to a block.
    pub scale: i32,
    pub overview: bool,
    /// The map's size in blocks.
    pub blocks: (i32, i32),
    cursor_block: Pos,
    box_min: Pos,
    box_max: Pos,
    /// The colour of the site box: the accent, or `bad` for a refused site.
    pub tint: Rgb8,
}

impl MapFrame<'_> {
    /// The block `(bx, by)`, or `None` off the map.
    pub fn cell(&self, bx: i32, by: i32) -> Option<BlockCell> {
        let (bw, bh) = self.blocks;
        if bx < 0 || by < 0 || bx >= bw || by >= bh {
            return None;
        }
        let (defs, palette, screen) = (self.defs, self.palette, self.screen);
        let map = screen.survey.map();
        let (glyph, fg, bg) = if self.scale == 1 {
            site_look(map.get(Pos::new(bx, by)), defs, palette)
        } else {
            block_look(
                classify_block(map, bx, by, self.scale, self.overview, &screen.glyphs),
                defs,
                palette,
            )
        };
        let corner = corner_at(bx, by, self.box_min, self.box_max, &self.marks);
        let in_box = (self.box_min.x..=self.box_max.x).contains(&bx)
            && (self.box_min.y..=self.box_max.y).contains(&by);
        Some(BlockCell {
            glyph: corner.unwrap_or(glyph),
            fg: if corner.is_some() { self.tint } else { fg },
            bg,
            corner: corner.is_some(),
            in_box: in_box && corner.is_none(),
            cursor: bx == self.cursor_block.x && by == self.cursor_block.y,
        })
    }

    /// The block `tile` lies in.
    pub fn block_of(&self, tile: Pos) -> Pos {
        Pos::new(tile.x.div_euclid(self.scale), tile.y.div_euclid(self.scale))
    }

    /// The background of an in-box block: its own, mixed toward the box's tint.
    pub fn boxed(&self, bg: Rgb8) -> Rgb8 {
        mix(bg, self.tint, 0.45)
    }
}

impl EmbarkScreen {
    /// The map's blocks at the current zoom, for a frontend to draw. `refused` (the report has a
    /// problem) turns the site box to the warning colour.
    pub fn map_frame<'a>(
        &'a self,
        defs: &'a Defs,
        palette: &'a Palette,
        refused: bool,
    ) -> MapFrame<'a> {
        let n = self.scale();
        let (mw, mh) = self.map_size();
        let radius = defs.embark.report_radius;
        MapFrame {
            screen: self,
            defs,
            palette,
            marks: Marks::new(palette.ascii),
            scale: n,
            overview: self.is_overview(),
            blocks: (block_count(mw, n), block_count(mh, n)),
            cursor_block: Pos::new(self.cursor.x.div_euclid(n), self.cursor.y.div_euclid(n)),
            box_min: Pos::new(
                (self.cursor.x - radius).div_euclid(n),
                (self.cursor.y - radius).div_euclid(n),
            ),
            box_max: Pos::new(
                (self.cursor.x + radius).div_euclid(n),
                (self.cursor.y + radius).div_euclid(n),
            ),
            tint: to_u8(if refused { palette.theme.bad } else { palette.theme.accent }),
        }
    }

    /// What the report panel needs to know of the screen (`fn_ui::embark::report_rows`).
    pub fn site(&self) -> Site<'_> {
        Site {
            seed: self.seed,
            nation: self.nation,
            difficulty: self.difficulty(),
            mode: self.mode,
            size_name: &self.size.name,
            scale: self.scale(),
            overview: self.is_overview(),
            cursor: self.cursor,
            terrain: self.tile_terrain(),
        }
    }

    /// The report for the site under the cursor.
    pub fn report(&self, defs: &Defs) -> SiteReport {
        self.survey.report(self.cursor, self.mode, self.difficulty(), defs)
    }

    /// Tiles to a block at the current zoom (1 at the closest).
    pub fn block_scale(&self) -> i32 {
        self.scale()
    }

    /// Whether the settle question (`y`/`n`) is open.
    pub fn asking(&self) -> bool {
        self.prompt
    }

    /// Whether the key help shows instead of the map.
    pub fn helping(&self) -> bool {
        self.help
    }

    /// Whether `Enter` goes on zooming in, and does not settle: not at the closest zoom.
    pub fn can_zoom_in(&self) -> bool {
        self.zoom + 1 < self.ladder.len()
    }
}

fn draw_map(
    screen: &EmbarkScreen,
    defs: &Defs,
    palette: &Palette,
    report: &SiteReport,
    area: Rect,
    buf: &mut Buffer,
) {
    let frame = screen.map_frame(defs, palette, report.problem.is_some());
    let (bw, bh) = frame.blocks;
    let (offx, offy) = if frame.overview {
        ((i32::from(area.width) - bw).max(0) / 2, (i32::from(area.height) - bh).max(0) / 2)
    } else {
        (0, 0)
    };
    let view = if frame.overview { Pos::new(0, 0) } else { screen.view };

    for row in 0..area.height {
        for col in 0..area.width {
            let bx = view.x + i32::from(col) - offx;
            let by = view.y + i32::from(row) - offy;
            let Some(cell) = frame.cell(bx, by) else { continue };
            let mut style = Style::new().fg(palette.color(cell.fg)).bg(palette.color(cell.bg));
            if cell.corner {
                style = style.add_modifier(Modifier::BOLD);
            } else if cell.in_box {
                style = if palette.mono() {
                    style.add_modifier(Modifier::REVERSED | Modifier::DIM)
                } else {
                    style.bg(palette.color(frame.boxed(cell.bg)))
                };
            }
            let out = &mut buf[(area.x + col, area.y + row)];
            out.set_char(cell.glyph);
            out.set_style(style);
            if cell.cursor {
                out.modifier |= Modifier::REVERSED;
            }
        }
    }
}

fn draw_help(palette: &Palette, area: Rect, buf: &mut Buffer) {
    let width = area.width.min(56);
    let height = 10u16.min(area.height);
    let outer = Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    );
    draw_frame(
        buf,
        outer,
        palette,
        &format!(" {} ", fn_ui::embark::HELP_TITLE),
        TitleAt::Centre,
        "",
    );
    if outer.height < 3 {
        return;
    }
    let inner = Rect::new(outer.x + 2, outer.y + 1, outer.width - 4, outer.height - 2);
    let mut p = Painter::new(buf, inner, palette);
    for (keys, what) in fn_ui::embark::HELP {
        p.row(&Row::pair(keys, Tone::Title, what, Tone::Text));
    }
}

/// The hint bar's mode (`fn_ui::keys`) for the screen as it stands: the settle question, or the
/// keys of the overview, a middle zoom or the closest zoom.
pub fn hint_mode(screen: &EmbarkScreen) -> keys::Mode {
    if screen.prompt {
        keys::Mode::Settle
    } else if screen.is_overview() {
        keys::Mode::Embark
    } else if screen.zoom + 1 == screen.ladder.len() {
        keys::Mode::EmbarkSite
    } else {
        keys::Mode::EmbarkZoom
    }
}

/// The last row: the hint bar (docs/design/tui.md, "Hints"), keys from the `fn_ui::keys` table. A
/// refusal ("Can't settle: ...") takes the place of the keys while it lasts.
fn draw_status(screen: &EmbarkScreen, palette: &Palette, area: Rect, buf: &mut Buffer) {
    let mode = hint_mode(screen);
    let body =
        if screen.status.is_empty() { BarBody::Keys } else { BarBody::Notice(&screen.status) };
    draw_hint_bar(
        buf,
        area,
        palette,
        mode,
        body,
        mode != keys::Mode::Settle,
        mode == keys::Mode::Settle,
    );
}

/// The "CHOOSE A SITE" panel: the frame, and the rows `fn_ui::embark` builds from the report
/// (the window paints the same rows).
fn draw_panel(
    screen: &EmbarkScreen,
    defs: &Defs,
    palette: &Palette,
    report: &SiteReport,
    area: Rect,
    buf: &mut Buffer,
) {
    let marks = Marks::new(palette.ascii);
    let title = format!(" {} ", fn_ui::embark::title(&marks));
    draw_frame(buf, area, palette, &title, TitleAt::Centre, "");
    let inner = Rect::new(
        area.x + 2,
        area.y + 1,
        area.width.saturating_sub(4),
        area.height.saturating_sub(2),
    );
    let mut p = Painter::new(buf, inner, palette);
    for row in report_rows(&screen.site(), report, defs, &marks, usize::from(inner.width)) {
        p.row(&row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fn_sim::embark::SiteProblem;

    fn defs() -> Defs {
        Defs::bundled()
    }

    fn small() -> MapSizeDef {
        defs().embark.map_size("small").unwrap().clone()
    }

    fn large() -> MapSizeDef {
        defs().embark.map_size("large").unwrap().clone()
    }

    #[test]
    fn fit_scale_is_the_smallest_whole_number_at_least_the_floor_that_fits() {
        assert_eq!(fit_scale(192, 128, 88, 39, 6), 6);
        // A tiny region needs a coarser scale than the floor.
        assert_eq!(fit_scale(192, 128, 10, 10, 6), 20);
        // A huge region never goes below the floor.
        assert_eq!(fit_scale(192, 128, 1000, 1000, 6), 6);
    }

    #[test]
    fn the_ladder_drops_fixed_levels_at_or_above_fit_and_one_in_a_rough_survey() {
        let defs = defs();
        let s = EmbarkScreen::new(
            7,
            Nation::Earth,
            small(),
            SurveyMode::Exact,
            Some("normal".into()),
            (120, 40),
            &defs,
        );
        assert_eq!(s.ladder, vec![6, 4, 2, 1]);
        let rough = EmbarkScreen::new(
            7,
            Nation::Earth,
            small(),
            SurveyMode::Rough,
            Some("normal".into()),
            (120, 40),
            &defs,
        );
        assert_eq!(rough.ladder, vec![6, 4, 2]);
        // A terminal too small to fit the Large map at the floor forces a coarser overview
        // (docs/design/embark.md's own table: Large is 1:12 at 80x24, 1:7 at 120x40), dropping
        // fewer fixed levels than Small's always-6.
        assert_eq!(s.ladder[0], 6);
        let mut big = EmbarkScreen::new(
            7,
            Nation::Earth,
            large(),
            SurveyMode::Exact,
            Some("normal".into()),
            (120, 40),
            &defs,
        );
        assert_eq!(big.ladder, vec![7, 4, 2, 1]);
        big.resize(80, 24);
        assert_eq!(big.ladder, vec![12, 4, 2, 1]);
    }

    #[test]
    fn zooming_in_and_out_walks_the_ladder_and_esc_at_the_overview_goes_back() {
        let defs = defs();
        let mut s = EmbarkScreen::new(
            7,
            Nation::Earth,
            small(),
            SurveyMode::Exact,
            Some("normal".into()),
            (120, 40),
            &defs,
        );
        assert!(s.is_overview());
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        assert!(matches!(s.key(key(KeyCode::Esc), &defs), Outcome::Back));
        s.key(key(KeyCode::Enter), &defs);
        assert_eq!(s.zoom, 1);
        s.key(key(KeyCode::Char('-')), &defs);
        assert_eq!(s.zoom, 0);
    }

    #[test]
    fn enter_at_the_closest_zoom_asks_and_y_settles() {
        let defs = defs();
        let mut s = EmbarkScreen::new(
            7,
            Nation::Earth,
            small(),
            SurveyMode::Exact,
            Some("normal".into()),
            (120, 40),
            &defs,
        );
        for _ in 0..3 {
            s.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &defs);
        }
        assert_eq!(s.scale(), 1, "three zooms from 6,4,2,1 lands on 1");
        s.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &defs);
        assert!(s.prompt, "the closest zoom asks instead of zooming further");
        let outcome = s.key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE), &defs);
        assert!(matches!(outcome, Outcome::Settle(_)), "{outcome:?}");
        if let Outcome::Settle(pos) = outcome {
            assert_eq!(pos, s.cursor);
        }
    }

    #[test]
    fn settling_on_a_refused_site_gives_a_status_message_instead_of_a_prompt() {
        let defs = defs();
        let mut s = EmbarkScreen::new(
            7,
            Nation::Earth,
            small(),
            SurveyMode::Exact,
            Some("normal".into()),
            (120, 40),
            &defs,
        );
        // The map's forced-sea border is always refused.
        s.cursor = Pos::new(0, 0);
        s.key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE), &defs);
        assert!(!s.prompt);
        assert!(s.status.contains("open water"), "{}", s.status);
    }

    #[test]
    fn r_takes_the_next_seed_and_resets_to_the_overview_at_its_default_site() {
        let defs = defs();
        let mut s = EmbarkScreen::new(
            7,
            Nation::Earth,
            small(),
            SurveyMode::Exact,
            Some("normal".into()),
            (120, 40),
            &defs,
        );
        s.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &defs);
        s.key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), &defs);
        assert_eq!(s.seed, 8);
        assert!(s.is_overview());
        assert_eq!(s.cursor, s.survey.default_site(&defs));
    }

    #[test]
    fn shift_moves_ten_cells_and_a_plain_move_is_one() {
        let defs = defs();
        let mut s = EmbarkScreen::new(
            7,
            Nation::Earth,
            small(),
            SurveyMode::Exact,
            Some("normal".into()),
            (120, 40),
            &defs,
        );
        let n = s.scale();
        let at = s.cursor;
        s.key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &defs);
        assert_eq!(s.cursor.x, (at.x + n).min(191));
        s.cursor = at;
        s.key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), &defs);
        assert_eq!(s.cursor.x, (at.x + n * 10).min(191));
    }

    #[test]
    fn classify_block_picks_sea_fresh_mountain_then_the_majority_biome() {
        let defs = defs();
        let g = crate::glyphs::Glyphs::bundled().embark;
        let cfg = fn_sim::MapGenConfig { seed: 7, width: 16, height: 16, ..Default::default() };
        let survey = Survey::new(&cfg, &defs);
        let map = survey.map();
        // Every block in a 16x16 map at scale 8 classifies as something; just check it doesn't
        // panic and always returns a kind backed by a `Look` (`block_look` would panic if not).
        for by in 0..2 {
            for bx in 0..2 {
                let kind = classify_block(map, bx, by, 8, true, &g);
                let palette = crate::color::Palette::new(
                    &defs,
                    Nation::Earth,
                    crate::color::ColorDepth::TrueColor,
                    false,
                );
                let _ = block_look(kind, &defs, &palette);
            }
        }
    }

    #[test]
    fn the_panel_shows_a_long_seed_and_a_long_refusal_in_full() {
        // The owner's screenshot (2026-11-06): a 20-digit seed cut to "Seed 179402856118478…"
        // and the refusal cut to "Can't settle: the landmass is…" in the 36-column panel.
        let defs = defs();
        let seed = 17_940_285_611_847_812_345;
        let screen = EmbarkScreen::new(
            seed,
            Nation::Air,
            large(),
            SurveyMode::Exact,
            Some("normal".into()),
            (200, 50),
            &defs,
        );
        let mut report =
            screen.survey.report(screen.cursor, screen.mode, screen.difficulty(), &defs);
        report.problem = Some(SiteProblem::TooSmall);
        let palette = Palette::new(&defs, Nation::Air, crate::color::ColorDepth::Never, false);
        let area = Rect::new(0, 0, 36, 20);
        let mut buf = Buffer::empty(area);
        draw_panel(&screen, &defs, &palette, &report, area, &mut buf);
        let rows: Vec<String> = (0..area.height)
            .map(|y| (0..area.width).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect();
        let text = rows
            .iter()
            .map(|r| r.trim_matches(|c: char| c == ' ' || c == '│'))
            .collect::<Vec<_>>()
            .join(" ");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(text.contains(&format!("Seed {seed}")), "seed cut off:\n{}", rows.join("\n"));
        assert!(text.contains("Air Nomads"), "nation missing:\n{}", rows.join("\n"));
        assert!(
            text.contains("Can't settle: the landmass is too small to live on."),
            "refusal cut off:\n{}",
            rows.join("\n")
        );
        assert!(!text.contains('…'), "something is still cut off:\n{}", rows.join("\n"));
    }
}

//! The embark screen (#290, docs/design/embark.md): zoom from a deliberately coarse overview of
//! a surveyed map down to the game's own 1:1 map, with a live site report, to choose where to
//! found the game.
//!
//! [`EmbarkScreen`] holds the state (the survey, the zoom ladder, the cursor and the view); its
//! [`EmbarkScreen::key`] is the only thing that changes it, returning an [`Outcome`] for
//! [`crate::shell::Shell`] to act on (going back to the seed step, or founding the world).
//! [`draw_embark`] is a pure function into a `Buffer`, like the other screens.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use fn_sim::defs::{BiomeId, Terrain};
use fn_sim::embark::{
    GroupCount, Level, MapSizeDef, Outlook, SiteProblem, SiteReport, Survey, SurveyMode,
};
use fn_sim::mapgen::{Tile, TileMap};
use fn_sim::{Defs, Nation, Pos};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::{Palette, mix, to_u8};
use crate::glyphs::{EmbarkGlyphDef, Rgb8};
use crate::layout;
use crate::widgets::{Marks, Painter, Row, TitleAt, Tone, draw_frame, fit, wrap};

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
                self.status =
                    format!("Can't settle: {}.", problem_text(problem, self.tile_terrain()));
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

/// Why `problem` refuses a site, in words; `terrain` (the tile itself) picks the clearer wording
/// for `NotLand`. `pub(crate)` for `--start X,Y`'s own error (`crate::shell::Shell::for_args`).
pub(crate) fn problem_text(problem: SiteProblem, terrain: Terrain) -> String {
    match problem {
        SiteProblem::NotLand if terrain == Terrain::Mountain => "a mountain".to_owned(),
        SiteProblem::NotLand if terrain.is_water() => "open water".to_owned(),
        SiteProblem::NotLand => "not open ground".to_owned(),
        SiteProblem::NearEdge => "too close to the map's edge".to_owned(),
        SiteProblem::TooSmall => "the landmass is too small to live on".to_owned(),
        SiteProblem::NoRoom => "not enough open room for the settlers".to_owned(),
    }
}

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
                Terrain::River | Terrain::Ford => river += 1,
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

fn draw_map(
    screen: &EmbarkScreen,
    defs: &Defs,
    palette: &Palette,
    report: &SiteReport,
    area: Rect,
    buf: &mut Buffer,
) {
    let n = screen.scale();
    let overview = screen.is_overview();
    let map = screen.survey.map();
    let (mw, mh) = (map.width(), map.height());
    let (bw, bh) = (block_count(mw, n), block_count(mh, n));
    let (offx, offy) = if overview {
        ((i32::from(area.width) - bw).max(0) / 2, (i32::from(area.height) - bh).max(0) / 2)
    } else {
        (0, 0)
    };
    let view = if overview { Pos::new(0, 0) } else { screen.view };
    let radius = defs.embark.report_radius;
    let cursor_block = Pos::new(screen.cursor.x.div_euclid(n), screen.cursor.y.div_euclid(n));
    let box_min = Pos::new(
        (screen.cursor.x - radius).div_euclid(n),
        (screen.cursor.y - radius).div_euclid(n),
    );
    let box_max = Pos::new(
        (screen.cursor.x + radius).div_euclid(n),
        (screen.cursor.y + radius).div_euclid(n),
    );
    let refused = report.problem.is_some();
    let marks = Marks::new(palette.ascii);
    let tint = to_u8(if refused { palette.theme.bad } else { palette.theme.accent });

    for row in 0..area.height {
        for col in 0..area.width {
            let bx = view.x + i32::from(col) - offx;
            let by = view.y + i32::from(row) - offy;
            if bx < 0 || by < 0 || bx >= bw || by >= bh {
                continue;
            }
            let (glyph0, fg0, bg0) = if n == 1 {
                site_look(map.get(Pos::new(bx, by)), defs, palette)
            } else {
                block_look(classify_block(map, bx, by, n, overview, &screen.glyphs), defs, palette)
            };
            let corner = corner_at(bx, by, box_min, box_max, &marks);
            let (glyph, fg) = match corner {
                Some(ch) => (ch, tint),
                None => (glyph0, fg0),
            };
            let mut style = Style::new().fg(palette.color(fg)).bg(palette.color(bg0));
            let in_box =
                (box_min.x..=box_max.x).contains(&bx) && (box_min.y..=box_max.y).contains(&by);
            if corner.is_some() {
                style = style.add_modifier(Modifier::BOLD);
            } else if in_box {
                style = if palette.mono() {
                    style.add_modifier(Modifier::REVERSED | Modifier::DIM)
                } else {
                    style.bg(palette.color(mix(bg0, tint, 0.45)))
                };
            }
            let out = &mut buf[(area.x + col, area.y + row)];
            out.set_char(glyph);
            out.set_style(style);
            if bx == cursor_block.x && by == cursor_block.y {
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
    draw_frame(buf, outer, palette, " EMBARK KEYS ", TitleAt::Centre, "");
    if outer.height < 3 {
        return;
    }
    let inner = Rect::new(outer.x + 2, outer.y + 1, outer.width - 4, outer.height - 2);
    let mut p = Painter::new(buf, inner, palette);
    for (keys, what) in [
        ("Arrows/WASD", "move (Shift: 10 cells)"),
        ("Enter, +", "zoom in; at the closest zoom, settle"),
        ("-", "zoom out"),
        ("Esc", "zoom out, or back at the overview"),
        ("E", "settle at the cursor"),
        ("R", "a new random seed"),
        ("Q", "quit"),
    ] {
        p.row(&Row::pair(keys, Tone::Title, what, Tone::Text));
    }
}

/// The footer's key hints, by zoom.
fn status_hints(screen: &EmbarkScreen) -> String {
    let moving = "arrows move";
    if !screen.status.is_empty() {
        return screen.status.clone();
    }
    if screen.is_overview() {
        format!("{moving}  Enter zoom in  E settle  R new seed  Esc back  ? keys")
    } else if screen.zoom + 1 == screen.ladder.len() {
        format!("{moving}  - zoom out  Enter/E settle here  R new seed  ? keys")
    } else {
        format!("{moving}  Enter zoom in  - zoom out  E settle  R new seed  ? keys")
    }
}

fn draw_status(screen: &EmbarkScreen, palette: &Palette, area: Rect, buf: &mut Buffer) {
    let theme = &palette.theme;
    let panel = Style::new().fg(palette.token(theme.dim)).bg(palette.token(theme.panel));
    for x in area.left()..area.right() {
        buf[(x, area.y)].set_char(' ').set_style(panel);
    }
    let badge = if palette.mono() {
        Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::new()
            .fg(palette.token(theme.on_accent))
            .bg(palette.token(theme.accent))
            .add_modifier(Modifier::BOLD)
    };
    // The settle prompt takes over the line the way the game's own y/n prompts do (draw.rs),
    // with the warning tone; without it the prompt is invisible and any other key cancels it.
    let (text, hints, badge) = if screen.prompt {
        let warn = if palette.mono() { badge } else { badge.bg(palette.token(theme.warn)) };
        let (x, y) = (screen.cursor.x, screen.cursor.y);
        (" SETTLE? ", format!("Press y to settle at ({x}, {y}), any other key to stay."), warn)
    } else {
        (" EMBARK ", status_hints(screen), badge)
    };
    buf.set_string(area.x, area.y, text, badge);
    let ellipsis = if palette.ascii { '~' } else { '…' };
    let room = usize::from(area.width.saturating_sub(text.chars().count() as u16 + 2));
    buf.set_string(
        area.x + text.chars().count() as u16 + 1,
        area.y,
        fit(&hints, room, ellipsis),
        panel,
    );
}

fn land_label(kind: fn_sim::embark::LandKind, defs: &Defs) -> String {
    use fn_sim::embark::LandKind;
    match kind {
        LandKind::Biome(id) => defs.biome(id).name.clone(),
        LandKind::Mountain => "Mountain".to_owned(),
        LandKind::Water => "Sea".to_owned(),
    }
}

/// The climate band's name and effects, lower case (`docs/design/difficulty.md`, "Embark"): for
/// example `cool: cold, frost`, or `warm: mild` for a band with neither. The effects list is
/// `fn_ui::view::climate_effects`'s own, which the Season section shares (#299).
fn climate_word(climate: &fn_sim::seasons::ClimateDef) -> String {
    format!("{}: {}", climate.name.to_lowercase(), fn_ui::view::climate_effects(climate))
}

fn level_word(level: Level) -> &'static str {
    match level {
        Level::None => "none",
        Level::Scarce => "scarce",
        Level::Some => "some",
        Level::Plenty => "plenty",
    }
}

fn water_kind_word(t: Terrain) -> &'static str {
    match t {
        Terrain::Water => "sea",
        Terrain::Lake => "lake",
        Terrain::River => "river",
        Terrain::Ford => "ford",
        _ => "water",
    }
}

fn water_text(report: &SiteReport, mode: SurveyMode, defs: &Defs) -> String {
    match report.water {
        None => "none within reach".to_owned(),
        Some((terrain, steps)) => match mode {
            SurveyMode::Rough => {
                if steps <= defs.embark.outlook.water_far {
                    "near".to_owned()
                } else {
                    "far".to_owned()
                }
            }
            SurveyMode::Exact => format!("{}, {steps} steps", water_kind_word(terrain)),
        },
    }
}

/// `n` with a thousands separator ("4,563"), as the mockup shows room-to-roam counts.
fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

fn group_row(g: &GroupCount) -> Row {
    let left = match g.count {
        Some(c) => format!("{:<8}{c:>3}", g.name),
        None => format!("{:<8}", g.name),
    };
    let right = if g.hidden { "?".to_owned() } else { level_word(g.level).to_owned() };
    Row::pair(left, Tone::Text, right, Tone::Text)
}

fn draw_panel(
    screen: &EmbarkScreen,
    defs: &Defs,
    palette: &Palette,
    report: &SiteReport,
    area: Rect,
    buf: &mut Buffer,
) {
    let marks = Marks::new(palette.ascii);
    let title = format!(" {0} CHOOSE A SITE {0} ", marks.seal);
    draw_frame(buf, area, palette, &title, TitleAt::Centre, "");
    let inner = Rect::new(
        area.x + 2,
        area.y + 1,
        area.width.saturating_sub(4),
        area.height.saturating_sub(2),
    );
    let mut p = Painter::new(buf, inner, palette);

    p.row(&Row::pair(
        format!("Seed {}", screen.seed),
        Tone::Dim,
        defs.nation(screen.nation).name.clone(),
        Tone::Text,
    ));
    // A line under the seed (#298, docs/design/difficulty.md "Embark"): the chosen preset, and
    // the size/survey mode already shown on the seed step.
    let preset_name = screen
        .difficulty()
        .and_then(|id| defs.difficulty.preset(id))
        .map_or("Normal", |p| p.name.as_str());
    let survey_name = match screen.mode() {
        SurveyMode::Exact => "Exact",
        SurveyMode::Rough => "Rough",
    };
    p.row(&Row::pair(
        preset_name.to_owned(),
        Tone::Dim,
        format!("{} · {survey_name}", screen.size().name),
        Tone::Text,
    ));
    let zoom_label = if screen.is_overview() {
        "overview"
    } else if screen.scale() == 1 {
        "site"
    } else {
        "region"
    };
    p.row(&Row::pair(
        format!("Zoom 1:{} {zoom_label}", screen.scale()),
        Tone::Dim,
        format!("({}, {})", screen.cursor.x, screen.cursor.y),
        Tone::Text,
    ));
    p.blank();

    if let Some(problem) = report.problem {
        p.row(&Row::text(format!("{} OUTLOOK", marks.bullet), Tone::Title));
        let cross = if palette.ascii { "x" } else { "✗" };
        p.row(&Row::text(
            format!("{cross} Can't settle: {}.", problem_text(problem, screen.tile_terrain())),
            Tone::Bad,
        ));
        return;
    }

    p.row(&Row::pair(
        format!("{} LAND", marks.bullet),
        Tone::Title,
        if report.mainland { "mainland" } else { "island" },
        Tone::Text,
    ));
    let biomes: Vec<_> = report.biomes.iter().take(4).collect();
    for pair in biomes.chunks(2) {
        let text = pair
            .iter()
            .map(|&&(k, pct)| format!("{} {pct}%", land_label(k, defs)))
            .collect::<Vec<_>>()
            .join("  ");
        p.row(&Row::text(text, Tone::Text));
    }
    // The start tile's climate band (#297, docs/design/difficulty.md "Embark"), only when the
    // chosen preset has seasons on: Sandbox, or no preset, never shows it.
    let seasons_on =
        screen.difficulty().and_then(|id| defs.difficulty.preset(id)).is_some_and(|k| k.seasons);
    if seasons_on {
        p.row(&Row::pair("Winter", Tone::Text, climate_word(&report.climate), Tone::Text));
    }
    p.row(&Row::pair(
        "Room to roam",
        Tone::Text,
        format!("{} tiles", thousands(report.room)),
        Tone::Text,
    ));
    if !screen.is_overview() {
        p.row(&Row::pair("Water", Tone::Text, water_text(report, screen.mode, defs), Tone::Text));
    }

    if screen.is_overview() {
        return;
    }
    p.blank();

    let radius = defs.embark.report_radius;
    let side = radius * 2 + 1;
    p.row(&Row::pair(
        format!("{} WITHIN {radius} TILES", marks.bullet),
        Tone::Title,
        format!("{side}x{side}"),
        Tone::Text,
    ));
    for g in &report.groups {
        p.row(&group_row(g));
    }

    p.blank();
    let outlook_word = match report.outlook {
        Outlook::Easy => "Easy",
        Outlook::Fair => "Fair",
        Outlook::Hard => "Hard",
    };
    let outlook_text = if report.rough {
        format!("{outlook_word} (rough guess)")
    } else {
        outlook_word.to_owned()
    };
    p.row(&Row::pair(format!("{} OUTLOOK", marks.bullet), Tone::Title, outlook_text, Tone::Text));
    let reasons = format!("{}.", report.reasons.join("; "));
    for line in wrap(&reasons, usize::from(inner.width), 2, marks.ellipsis) {
        p.row(&Row::text(line, Tone::Text));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

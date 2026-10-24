//! The shared UI look decided in #39 (docs/design/ui.md): the "Seal" style. Flat, opaque slate
//! panels with a thin frame tinted by the nation's accent, accent corner brackets and a diamond
//! seal on the top edge. Every screen builds from these tokens and builders instead of literal
//! colours and sizes, so one edit here restyles the whole UI.
//!
//! Since #129 the frame, seal, section bullet and nation emblem are Claude-authored pixel art
//! (art/ui_frame.toml, art/ui_flair.toml, art/emblems.toml; docs/design/pixel-art.md), drawn at
//! one integer scale ([`ART_SCALE`]) so no two pixel sizes meet. [`Skin`] holds the current
//! nation's sheets.
//!
//! The accent is the player nation's `color` from data/nations.toml; everything else is fixed so
//! status colours (ok/warn/bad) never change meaning between nations.

use bevy::prelude::*;
use bevy::sprite::{BorderRect, SliceScaleMode, TextureSlicer};
use fn_sim::Nation;

/// Every UI sprite is drawn at this integer scale: one art pixel is two screen pixels.
pub const ART_SCALE: f32 = 2.0;

/// Text sizes in logical pixels (Bevy's default font).
pub mod font {
    /// The selected thing's name in the selection bar.
    pub const NAME: f32 = 20.0;
    /// Panel titles, e.g. the nation name in the side panel.
    pub const TITLE: f32 = 18.0;
    /// Rows, values, buttons and tooltips.
    pub const BODY: f32 = 14.0;
    /// Section titles (upper case) and small labels.
    pub const LABEL: f32 = 12.0;
}

/// Spacing steps in logical pixels. Use these instead of ad-hoc numbers.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 6.0;
    pub const M: f32 = 8.0;
    pub const L: f32 = 12.0;
    pub const XL: f32 = 16.0;
}

/// Layout zones (ui.md "Layout zones"), at the 1280x720 reference size.
pub mod layout {
    /// Persistent side panel on the right edge, full height.
    pub const SIDE_PANEL_WIDTH: f32 = 272.0;
    /// Selection bar along the bottom, from the left edge to the side panel.
    pub const SELECTION_BAR_HEIGHT: f32 = 128.0;
    /// Gap between the screen edge and a panel, and between panels.
    pub const SCREEN_MARGIN: f32 = 12.0;
    /// The frame art's transparent margin: its brackets stick out this far past a panel's edge
    /// (4 art pixels at [`super::ART_SCALE`]). The frame sprite is placed this far outside.
    pub const FRAME_MARGIN: f32 = 8.0;
}

/// Colour tokens for one nation, as Bevy colours. The values and the maths behind them are
/// `fn_ui::theme::Theme`'s; build with [`Theme::for_nation`].
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Panel body. Opaque so snow and sand can't wash it out.
    pub panel: Color,
    /// Bar tracks, button bodies, portrait wells.
    pub inset: Color,
    /// Values and primary text.
    pub text: Color,
    /// Labels and secondary text.
    pub dim: Color,
    /// The nation colour, unchanged: fills, active button, emblem, brackets, seal.
    pub accent: Color,
    /// The accent lightened until it reads as text on `panel`.
    pub accent_text: Color,
    /// The accent blended into `panel`: panel frames, section rules, idle button borders.
    pub accent_soft: Color,
    /// Text on an `accent` fill.
    pub on_accent: Color,
    pub ok: Color,
    pub warn: Color,
    pub bad: Color,
}

pub const SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.45);

fn color([r, g, b]: fn_ui::theme::Rgb) -> Color {
    Color::srgb(r, g, b)
}

impl From<fn_ui::theme::Theme> for Theme {
    fn from(t: fn_ui::theme::Theme) -> Self {
        Self {
            panel: color(t.panel),
            inset: color(t.inset),
            text: color(t.text),
            dim: color(t.dim),
            accent: color(t.accent),
            accent_text: color(t.accent_text),
            accent_soft: color(t.accent_soft),
            on_accent: color(t.on_accent),
            ok: color(t.ok),
            warn: color(t.warn),
            bad: color(t.bad),
        }
    }
}

impl Theme {
    /// Tokens for a nation whose `color` in nations.toml is `rgb`.
    pub fn for_nation(rgb: [u8; 3]) -> Self {
        fn_ui::theme::Theme::for_nation(rgb).into()
    }
}

/// One nation's UI sheets (`assets/original/`, drawn by `cargo xtask art`), picked by the nation's
/// id from data. Rebuilt with the [`Theme`] when the player's nation changes.
#[derive(Resource, Clone, Debug)]
pub struct Skin {
    /// `ui_frame.<nation>.png`: the panel as a 9-slice.
    pub frame: Handle<Image>,
    /// `ui_flair.<nation>.png`: the seal and the section bullet.
    pub flair: Handle<Image>,
    /// `emblems.png`: every nation's emblem; [`emblem_cell`] picks one.
    pub emblems: Handle<Image>,
    /// The nation's data id, e.g. `earth`: names the emblem.
    nation: &'static str,
}

/// Every nation's [`Skin`], loaded at startup so switching nation (keys 1 to 4) never shows a
/// frame-less panel while a sheet loads.
#[derive(Resource)]
pub struct UiArt(Vec<(Nation, Skin)>);

impl UiArt {
    pub fn load(assets: &AssetServer) -> Self {
        UiArt(Nation::ALL.iter().map(|&n| (n, Skin::load(assets, n))).collect())
    }

    pub fn skin(&self, nation: Nation) -> Skin {
        self.0.iter().find(|(n, _)| *n == nation).expect("every nation has a skin").1.clone()
    }

    /// Every image the UI needs, for waiting until they are loaded.
    pub fn handles(&self) -> impl Iterator<Item = &Handle<Image>> {
        self.0.iter().flat_map(|(_, s)| [&s.frame, &s.flair, &s.emblems])
    }
}

impl Skin {
    fn load(assets: &AssetServer, nation: Nation) -> Self {
        let id = nation.as_str();
        Self {
            frame: assets.load(format!("original/ui_frame.{id}.png")),
            flair: assets.load(format!("original/ui_flair.{id}.png")),
            emblems: assets.load("original/emblems.png"),
            nation: id,
        }
    }
}

/// Cell size of the flair and emblem sheets, in art pixels.
const CELL: f32 = 16.0;
/// Border of the frame 9-slice, in art pixels: the corner squares.
const FRAME_BORDER: f32 = 14.0;
/// The cells of `ui_flair.<nation>.png`, as (column, sprite name in art/ui_flair.toml).
const FLAIR_SEAL: (u32, &str) = (0, "seal");
const FLAIR_BULLET: (u32, &str) = (1, "bullet");
/// Where the bullet's pixels sit inside its cell, as (min x, min y, max x, max y), so a 4px
/// diamond doesn't make its row 32px tall.
const BULLET_PIXELS: [f32; 4] = [5.0, 6.0, 9.0, 10.0];
/// The cells of `emblems.png` in order: the sprite names in art/emblems.toml, which are nation ids.
const EMBLEMS: [&str; 4] = ["air", "water", "earth", "fire"];

/// The pixel rect of cell `column` in a one-row sheet of 16px cells.
fn cell_rect(column: u32) -> Rect {
    let x = column as f32 * CELL;
    Rect::new(x, 0.0, x + CELL, CELL)
}

/// The emblem's cell in `emblems.png` for a nation id.
fn emblem_cell(nation: &str) -> u32 {
    let i = EMBLEMS.iter().position(|n| *n == nation);
    i.unwrap_or_else(|| panic!("no emblem for nation {nation}")) as u32
}

/// An image node showing `rect` of `image`, at [`ART_SCALE`].
fn sprite_node(image: &Handle<Image>, rect: Rect) -> impl Bundle {
    (
        Node {
            width: Val::Px(rect.width() * ART_SCALE),
            height: Val::Px(rect.height() * ART_SCALE),
            flex_shrink: 0.0,
            ..default()
        },
        ImageNode { image: image.clone(), rect: Some(rect), ..default() },
    )
}

/// The nation's emblem for the side-panel header: a 32px medallion.
pub fn emblem(skin: &Skin) -> impl Bundle {
    sprite_node(&skin.emblems, cell_rect(emblem_cell(skin.nation)))
}

/// Hit testing. The client uses no `Interaction` or `Button` (Bevy 0.20 deprecates them), so a
/// click is tested against the laid-out nodes by hand. `ComputedNode` geometry is in physical
/// pixels, so the cursor is scaled to match.
pub fn node_under_cursor<'a, T>(
    window: &Window,
    nodes: impl IntoIterator<Item = (T, &'a ComputedNode, &'a UiGlobalTransform)>,
) -> Option<T> {
    let cursor = window.cursor_position()? * window.scale_factor();
    nodes
        .into_iter()
        .find_map(|(id, node, transform)| node.contains_point(*transform, cursor).then_some(id))
}

/// A text bundle in the default font.
pub fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(s), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color))
}

/// How much edge flair a panel gets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Frame {
    /// Side panel, selection bar, screens: the pixel-art 9-slice frame and the diamond seal.
    Panel,
    /// Tooltips and small popups: small code-drawn brackets, no seal.
    Tooltip,
}

/// The panel body: opaque slate, square corners, soft drop shadow. `node` supplies position and
/// layout; border and radius are overridden. A [`Frame::Tooltip`] keeps a 1px accent-tinted
/// border; a [`Frame::Panel`] has none, because [`frame`] draws the pixel-art frame over its edge.
/// Spawn it, then call [`frame`] with the same `kind` inside its children so the flair is
/// positioned relative to the panel.
pub fn panel(theme: &Theme, kind: Frame, node: Node) -> impl Bundle {
    let border = if kind == Frame::Tooltip { 1.0 } else { 0.0 };
    (
        Node { border: UiRect::all(Val::Px(border)), border_radius: BorderRadius::ZERO, ..node },
        BackgroundColor(theme.panel),
        BorderColor::all(theme.accent_soft),
        BoxShadow::new(SHADOW, Val::Px(0.0), Val::Px(3.0), Val::Px(0.0), Val::Px(8.0)),
    )
}

/// Edge flair for a panel spawned with [`panel`]. [`Frame::Panel`] gets the 9-slice frame and the
/// seal, [`Frame::Tooltip`] small code-drawn brackets. Everything is absolutely positioned, so it
/// never affects layout.
pub fn frame(p: &mut ChildSpawnerCommands, theme: &Theme, skin: &Skin, kind: Frame) {
    match kind {
        Frame::Panel => {
            nine_slice(p, skin);
            seal(p, skin);
        }
        Frame::Tooltip => corner_brackets(p, theme),
    }
}

/// The frame sheet as a 9-slice over the panel, [`layout::FRAME_MARGIN`] outside every edge. The
/// 14px corners are drawn at [`ART_SCALE`] (`max_corner_scale`); the 4px middle stretches.
fn nine_slice(p: &mut ChildSpawnerCommands, skin: &Skin) {
    let out = Val::Px(-layout::FRAME_MARGIN);
    let slicer = TextureSlicer {
        border: BorderRect::all(FRAME_BORDER),
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Stretch,
        max_corner_scale: ART_SCALE,
    };
    p.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: out,
            right: out,
            top: out,
            bottom: out,
            ..default()
        },
        ImageNode {
            image: skin.frame.clone(),
            image_mode: NodeImageMode::Sliced(slicer),
            ..default()
        },
    ));
}

/// A 1px dark outline for flair drawn over the map. It echoes Ninja Adventure's outlined sprites
/// (docs/design/art.md) and keeps accent brackets visible on terrain of the same hue (Earth's
/// green on grass).
fn flair_outline(theme: &Theme) -> Outline {
    Outline::new(Val::Px(1.0), Val::Px(0.0), theme.inset)
}

/// A tooltip's L-shaped accent bracket at each corner, slightly outside the panel edge: arm
/// length, thickness and offset past the edge. Tooltips are too small for the pixel frame.
const TOOLTIP_BRACKET: (f32, f32, f32) = (8.0, 2.0, -3.0);

fn corner_brackets(p: &mut ChildSpawnerCommands, theme: &Theme) {
    let (len, thick, off) = TOOLTIP_BRACKET;
    for (top, left) in [(true, true), (true, false), (false, true), (false, false)] {
        let at = |near: bool| if near { Val::Px(off) } else { Val::Auto };
        let base = Node {
            position_type: PositionType::Absolute,
            top: at(top),
            bottom: at(!top),
            left: at(left),
            right: at(!left),
            ..default()
        };
        let arm = |w: f32, h: f32| Node { width: Val::Px(w), height: Val::Px(h), ..base.clone() };
        p.spawn((arm(len, thick), BackgroundColor(theme.accent), flair_outline(theme)));
        p.spawn((arm(thick, len), BackgroundColor(theme.accent), flair_outline(theme)));
    }
}

/// The diamond seal, centred on the panel's top edge.
fn seal(p: &mut ChildSpawnerCommands, skin: &Skin) {
    let rect = cell_rect(FLAIR_SEAL.0);
    let size = rect.size() * ART_SCALE;
    p.spawn(Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        right: Val::Px(0.0),
        top: Val::Px(-size.y / 2.0),
        justify_content: JustifyContent::Center,
        ..default()
    })
    .with_children(|h| {
        h.spawn(sprite_node(&skin.flair, rect));
    });
}

/// An upper-case section title with a diamond bullet and a rule filling the rest of the row.
pub fn section_title(p: &mut ChildSpawnerCommands, theme: &Theme, skin: &Skin, title: &str) {
    let row = Node {
        column_gap: Val::Px(space::S),
        align_items: AlignItems::Center,
        margin: UiRect::top(Val::Px(space::XS)),
        ..default()
    };
    let [x0, y0, x1, y1] = BULLET_PIXELS;
    let at = cell_rect(FLAIR_BULLET.0).min;
    let rect = Rect::new(at.x + x0, at.y + y0, at.x + x1, at.y + y1);
    p.spawn(row).with_children(|r| {
        r.spawn(sprite_node(&skin.flair, rect));
        r.spawn(text(title.to_uppercase(), font::LABEL, theme.accent_text));
        r.spawn((
            Node {
                height: Val::Px(1.0),
                flex_grow: 1.0,
                margin: UiRect::left(Val::Px(space::XS)),
                ..default()
            },
            BackgroundColor(theme.accent_soft),
        ));
    });
}

/// A progress bar: `frac` in `[0, 1]` of `fill` over an inset track.
pub fn bar(p: &mut ChildSpawnerCommands, theme: &Theme, frac: f32, fill: Color, height: f32) {
    let track = Node { width: Val::Percent(100.0), height: Val::Px(height), ..default() };
    p.spawn((track, BackgroundColor(theme.inset))).with_children(|t| {
        t.spawn((
            Node {
                width: Val::Percent(frac.clamp(0.0, 1.0) * 100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(fill),
        ));
    });
}

/// A flat button with an optional hotkey hint. `active` fills it with the accent.
pub fn button(p: &mut ChildSpawnerCommands, theme: &Theme, label: &str, key: &str, active: bool) {
    let (bg, fg, border) = if active {
        (theme.accent, theme.on_accent, theme.accent)
    } else {
        (theme.inset, theme.text, theme.accent_soft)
    };
    let node = Node {
        padding: UiRect::axes(Val::Px(10.0), Val::Px(space::S)),
        border: UiRect::all(Val::Px(1.0)),
        column_gap: Val::Px(space::S),
        align_items: AlignItems::Center,
        ..default()
    };
    p.spawn((node, BackgroundColor(bg), BorderColor::all(border))).with_children(|b| {
        b.spawn(text(label, font::BODY, fg));
        if !key.is_empty() {
            b.spawn(text(key, font::LABEL, if active { fg } else { theme.dim }));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_is_the_shared_tokens_as_bevy_colours() {
        let shared = fn_ui::theme::Theme::for_nation([90, 160, 80]);
        let t = Theme::for_nation([90, 160, 80]);
        assert_eq!(t.accent, Color::srgb_u8(90, 160, 80));
        assert_eq!(
            t.accent_soft,
            Color::srgb(shared.accent_soft[0], shared.accent_soft[1], shared.accent_soft[2])
        );
        assert_eq!(t.panel, Color::srgb(0.07, 0.08, 0.1));
        assert_eq!(t, Theme::from(shared));
    }

    /// The names of `art/<sheet>.toml`'s sprites in file order, and the grid of each.
    fn sprites(sheet: &str) -> Vec<(String, Vec<String>)> {
        let path = format!("{}/../../art/{sheet}.toml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let doc: toml::Table = text.parse().unwrap();
        doc["sprite"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                let name = s["name"].as_str().unwrap().to_owned();
                let grid = s["grid"].as_str().unwrap().lines().map(str::to_owned).collect();
                (name, grid)
            })
            .collect()
    }

    #[test]
    fn the_emblem_cells_are_the_sprites_named_after_the_nations() {
        let names: Vec<_> = sprites("emblems").into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, EMBLEMS);
        // Every nation in data has a cell, picked by its id.
        for n in Nation::ALL {
            assert_eq!(EMBLEMS[emblem_cell(n.as_str()) as usize], n.as_str());
        }
    }

    #[test]
    fn the_flair_cells_are_the_sprites_they_claim() {
        let sprites = sprites("ui_flair");
        for (column, name) in [FLAIR_SEAL, FLAIR_BULLET] {
            assert_eq!(sprites[column as usize].0, name);
        }
    }

    #[test]
    fn the_bullet_rect_is_exactly_the_bullets_pixels() {
        let sprites = sprites("ui_flair");
        let grid = &sprites[FLAIR_BULLET.0 as usize].1;
        let filled = |x: usize, y: usize| grid[y].as_bytes()[x] != b'.';
        let cells = (0..CELL as usize).flat_map(|y| (0..CELL as usize).map(move |x| (x, y)));
        let (mut min, mut max) = ([usize::MAX; 2], [0; 2]);
        for (x, y) in cells.filter(|&(x, y)| filled(x, y)) {
            min = [min[0].min(x), min[1].min(y)];
            max = [max[0].max(x + 1), max[1].max(y + 1)];
        }
        assert_eq!(BULLET_PIXELS, [min[0] as f32, min[1] as f32, max[0] as f32, max[1] as f32]);
    }

    #[test]
    fn the_frame_border_is_the_corner_square_of_the_sheet() {
        // 32x32 sheet: 14px corners leave a 4px stretchable middle.
        let frame = &sprites("ui_frame")[0].1;
        assert_eq!((frame.len(), frame[0].len()), (32, 32));
        assert_eq!(32.0 - 2.0 * FRAME_BORDER, 4.0);
        // The transparent margin is `layout::FRAME_MARGIN` on screen: the soft line starts there.
        let margin = (layout::FRAME_MARGIN / ART_SCALE) as usize;
        assert_eq!(frame[16].as_bytes()[margin], b's');
        assert_eq!(&frame[16][..margin], "....");
    }
}

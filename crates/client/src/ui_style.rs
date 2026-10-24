//! The shared UI look decided in #39 (docs/design/ui.md): the "Seal" style. Flat, opaque slate
//! panels with a thin frame tinted by the nation's accent, accent corner brackets and a diamond
//! seal on the top edge. Every screen builds from these tokens and builders instead of literal
//! colours and sizes, so one edit here restyles the whole UI.
//!
//! The accent is the player nation's `color` from data/nations.toml; everything else is fixed so
//! status colours (ok/warn/bad) never change meaning between nations.

use bevy::prelude::*;

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
    /// Corner brackets stick out this far past a panel's edge.
    pub const BRACKET_OFFSET: f32 = 6.0;
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
    /// Side panel, selection bar, screens: large brackets and the diamond seal.
    Panel,
    /// Tooltips and small popups: small brackets, no seal.
    Tooltip,
}

/// The panel body: opaque slate, 1px accent-tinted frame, square corners, soft drop shadow.
/// `node` supplies position and layout; border and radius are overridden. Spawn it, then call
/// [`frame`] inside its children so the flair is positioned relative to the panel.
pub fn panel(theme: &Theme, node: Node) -> impl Bundle {
    (
        Node { border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::ZERO, ..node },
        BackgroundColor(theme.panel),
        BorderColor::all(theme.accent_soft),
        BoxShadow::new(SHADOW, Val::Px(0.0), Val::Px(3.0), Val::Px(0.0), Val::Px(8.0)),
    )
}

/// Edge flair for a panel spawned with [`panel`]: corner brackets, plus the seal for
/// [`Frame::Panel`]. Everything is absolutely positioned, so it never affects layout.
pub fn frame(p: &mut ChildSpawnerCommands, theme: &Theme, kind: Frame) {
    corner_brackets(p, theme, kind);
    if kind == Frame::Panel {
        seal(p, theme);
    }
}

/// Bracket (arm length, thickness, offset past the edge) for a frame kind.
fn bracket_size(kind: Frame) -> (f32, f32, f32) {
    match kind {
        Frame::Panel => (26.0, 3.0, -layout::BRACKET_OFFSET),
        Frame::Tooltip => (8.0, 2.0, -3.0),
    }
}

/// A 1px dark outline for flair drawn over the map. It echoes Ninja Adventure's outlined sprites
/// (docs/design/art.md) and keeps accent brackets visible on terrain of the same hue (Earth's
/// green on grass).
fn flair_outline(theme: &Theme) -> Outline {
    Outline::new(Val::Px(1.0), Val::Px(0.0), theme.inset)
}

/// An L-shaped accent bracket at each corner, slightly outside the panel edge.
pub fn corner_brackets(p: &mut ChildSpawnerCommands, theme: &Theme, kind: Frame) {
    let (len, thick, off) = bracket_size(kind);
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

/// A diamond centred on the panel's top edge: accent, with an inset-coloured core.
pub fn seal(p: &mut ChildSpawnerCommands, theme: &Theme) {
    let holder = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        right: Val::Px(0.0),
        top: Val::Px(-8.0),
        justify_content: JustifyContent::Center,
        ..default()
    };
    let diamond = Node {
        width: Val::Px(16.0),
        height: Val::Px(16.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };
    p.spawn(holder).with_children(|h| {
        let body = (diamond, BackgroundColor(theme.accent), rotated45(), flair_outline(theme));
        h.spawn(body).with_children(|d| {
            d.spawn((
                Node { width: Val::Px(6.0), height: Val::Px(6.0), ..default() },
                BackgroundColor(theme.inset),
            ));
        });
    });
}

fn rotated45() -> UiTransform {
    UiTransform::from_rotation(Rot2::degrees(45.0))
}

/// An upper-case section title with a diamond bullet and a rule filling the rest of the row.
pub fn section_title(p: &mut ChildSpawnerCommands, theme: &Theme, title: &str) {
    let row = Node {
        column_gap: Val::Px(space::S),
        align_items: AlignItems::Center,
        margin: UiRect::top(Val::Px(space::XS)),
        ..default()
    };
    p.spawn(row).with_children(|r| {
        r.spawn((
            Node { width: Val::Px(6.0), height: Val::Px(6.0), ..default() },
            BackgroundColor(theme.accent),
            rotated45(),
        ));
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
}

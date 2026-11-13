//! The Seal look in egui (#874, docs/design/egui-frontend.md, "The look"): `egui::Visuals`,
//! `Spacing` and the text styles, all built from [`fn_ui::theme::Theme`], the tokens the
//! terminal's `Palette` is made of, so nothing new goes in data. Dark only.
//!
//! | egui | Token |
//! |---|---|
//! | `panel_fill`, `window_fill` | `panel` |
//! | `extreme_bg_color` (bar tracks, text edits), a button's body | `inset` |
//! | text, weak text | `text`, `dim` |
//! | every stroke | 1 px `accent_soft` (a focus ring and the text cursor: `accent_text`) |
//! | `selection.bg_fill` | `select` |
//! | hovered and active widgets | `accent` fill, `on_accent` text |
//! | hyperlinks | `accent_text` |
//! | warning and error text | `warn`, `bad` |
//!
//! Every corner radius is 0 and nothing casts a shadow. `override_text_color` stays unset: it
//! would also colour a button's text on its `accent` fill, where the text must be `on_accent`;
//! the same `text` comes from the non-interactive widget stroke instead.
//!
//! The type scale is 20/18/14/12 px ([`SIZE_NAME`] to [`SIZE_LABEL`]), every style in the
//! monospace family (#869's default: one family throughout). The spacing steps are
//! 4/6/8/12/16 ([`STEPS`]). The flair the terminal draws in box glyphs is here as well:
//! [`corner_brackets`] for boxes and full screens. (The seal on the side panel's edge and the
//! `◇` headings are `side`'s; the page frame's seal is `page`'s.)
//!
//! The window rebuilds the style when the nation changes ([`install`], from `GuiApp`).
//!
//! Every vertical list scrolls through [`scroll_vertical`] (#973, docs/design/touch-pwa.md):
//! by wheel, by its bar, and by dragging its content with a finger *or* a mouse. egui's default
//! drags only once it has seen a touch, so a phone's first swipe did nothing.

use std::collections::BTreeMap;

use eframe::egui::containers::scroll_area::{DragScroll, ScrollSource};
use eframe::egui::style::{HandleShape, Selection, TextCursorStyle, WidgetVisuals, Widgets};
use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Painter, Rect, ScrollArea, Shadow,
    Stroke, Style, TextStyle, Theme as EguiTheme, Visuals, vec2,
};
use fn_ui::theme::{Rgb, Theme};

/// A name: a settler's, a screen's (20 px).
pub const SIZE_NAME: f32 = 20.0;
/// A panel's title (18 px).
pub const SIZE_TITLE: f32 = 18.0;
/// Body text (14 px).
pub const SIZE_BODY: f32 = 14.0;
/// Labels and upper-case section headings (12 px).
pub const SIZE_LABEL: f32 = 12.0;

/// The spacing steps, in points, smallest first: 4, 6, 8, 12, 16.
pub const STEPS: [f32; 5] = [4.0, 6.0, 8.0, 12.0, 16.0];

/// The arm of a corner bracket, in points.
pub const BRACKET_ARM: f32 = 10.0;

/// The text style of a panel's title (the 18 px step; egui has no style of its own for it).
pub fn title_style() -> TextStyle {
    TextStyle::Name("title".into())
}

/// A theme token as egui draws it.
pub fn color32(rgb: Rgb) -> Color32 {
    let [r, g, b] = rgb.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
    Color32::from_rgb(r, g, b)
}

/// The type scale: every style in the monospace family.
pub fn text_styles() -> BTreeMap<TextStyle, FontId> {
    let mono = |size| FontId::new(size, FontFamily::Monospace);
    BTreeMap::from([
        (TextStyle::Heading, mono(SIZE_NAME)),
        (title_style(), mono(SIZE_TITLE)),
        (TextStyle::Body, mono(SIZE_BODY)),
        (TextStyle::Button, mono(SIZE_BODY)),
        (TextStyle::Monospace, mono(SIZE_BODY)),
        (TextStyle::Small, mono(SIZE_LABEL)),
    ])
}

/// egui's spacing with the steps applied: padding and gaps are 4, 6, 8 or 12 points.
pub fn spacing() -> egui::Spacing {
    let [s4, s6, s8, s12, _] = STEPS;
    egui::Spacing {
        item_spacing: vec2(s8, s4),
        button_padding: vec2(s8, s4),
        window_margin: Margin::same(s12 as i8),
        menu_margin: Margin::same(s6 as i8),
        indent: s12,
        ..egui::Spacing::default()
    }
}

/// One widget state: `fill` for both its fill kinds, a 1 px `stroke`, `text` on top, no rounding
/// and no growing on hover.
fn widget(fill: Color32, stroke: Color32, text: Color32) -> WidgetVisuals {
    WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0, stroke),
        corner_radius: CornerRadius::ZERO,
        fg_stroke: Stroke::new(1.0, text),
        expansion: 0.0,
    }
}

/// The Seal visuals of `theme`.
pub fn visuals(theme: &Theme) -> Visuals {
    let c = color32;
    let (panel, inset, text, dim) = (c(theme.panel), c(theme.inset), c(theme.text), c(theme.dim));
    let (accent, soft, on_accent) = (c(theme.accent), c(theme.accent_soft), c(theme.on_accent));
    let (accent_text, select) = (c(theme.accent_text), c(theme.select));
    Visuals {
        dark_mode: true,
        override_text_color: None,
        weak_text_color: Some(dim),
        widgets: Widgets {
            // Labels, separators and frames: the panel's own colour with the 1 px rule.
            noninteractive: widget(panel, soft, text),
            // A button at rest: its body is the inset well.
            inactive: widget(inset, soft, text),
            hovered: widget(accent, accent, on_accent),
            active: widget(accent, accent, on_accent),
            open: widget(select, soft, text),
        },
        selection: Selection { bg_fill: select, stroke: Stroke::new(1.0, accent_text) },
        hyperlink_color: accent_text,
        faint_bg_color: select,
        extreme_bg_color: inset,
        text_edit_bg_color: None,
        code_bg_color: inset,
        warn_fg_color: c(theme.warn),
        error_fg_color: c(theme.bad),
        window_corner_radius: CornerRadius::ZERO,
        window_shadow: Shadow::NONE,
        window_fill: panel,
        window_stroke: Stroke::new(1.0, soft),
        menu_corner_radius: CornerRadius::ZERO,
        panel_fill: panel,
        popup_shadow: Shadow::NONE,
        text_cursor: TextCursorStyle {
            stroke: Stroke::new(2.0, accent_text),
            ..Visuals::dark().text_cursor
        },
        handle_shape: HandleShape::Rect { aspect_ratio: 0.5 },
        ..Visuals::dark()
    }
}

/// The whole egui style for `theme`: visuals, spacing and the type scale.
pub fn style(theme: &Theme) -> Style {
    Style {
        visuals: visuals(theme),
        spacing: spacing(),
        text_styles: text_styles(),
        ..Style::default()
    }
}

/// Makes `theme` the dark style of `ctx`. Called again whenever the nation changes.
/// A vertical [`ScrollArea`] that also scrolls when its content is dragged, from the first
/// touch on (egui's default, [`DragScroll::OnTouch`], waits until it has seen one) and with a
/// mouse. A click on a row is still a click: egui tells a press from a drag by distance.
pub fn scroll_vertical() -> ScrollArea {
    ScrollArea::vertical()
        .scroll_source(ScrollSource { drag: DragScroll::Always, ..ScrollSource::default() })
}

pub fn install(ctx: &egui::Context, theme: &Theme) {
    ctx.set_style_of(EguiTheme::Dark, style(theme));
}

/// Accent brackets at the four corners of `rect`: an arm along each edge from each corner, the
/// terminal's `┏ ┓ ┗ ┛` for a box or a full screen. `rect` is the box's outline, whose lines
/// run through its edge; the brackets are 2 px, over a 1 px frame.
pub fn corner_brackets(painter: &Painter, rect: Rect, colour: Color32) {
    let stroke = Stroke::new(2.0, colour);
    let arm = BRACKET_ARM.min(rect.width() / 2.0).min(rect.height() / 2.0);
    for (corner, dx, dy) in [
        (rect.left_top(), 1.0, 1.0),
        (rect.right_top(), -1.0, 1.0),
        (rect.left_bottom(), 1.0, -1.0),
        (rect.right_bottom(), -1.0, -1.0),
    ] {
        painter.line_segment([corner, corner + vec2(dx * arm, 0.0)], stroke);
        painter.line_segment([corner, corner + vec2(0.0, dy * arm)], stroke);
    }
}

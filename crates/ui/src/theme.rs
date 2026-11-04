//! The colour tokens of the shared UI look (docs/design/ui.md, the "Seal" style), as plain sRGB
//! triples in `[0, 1]`. The terminal UI maps them onto its own palette. The maths (the accent's
//! soft and text tones) and the contrast guarantees live here so the frontend gets them.
//!
//! The accent is the player nation's `color` from data/nations.toml; everything else is fixed so
//! status colours (ok/warn/bad) never change meaning between nations.

/// An sRGB colour, each channel in `[0, 1]`.
pub type Rgb = [f32; 3];

pub const WHITE: Rgb = [1.0, 1.0, 1.0];

/// Panel body. Opaque so snow and sand can't wash it out.
pub const PANEL: Rgb = [0.07, 0.08, 0.1];
/// Bar tracks, button bodies, portrait wells.
pub const INSET: Rgb = [0.04, 0.045, 0.06];
/// Values and primary text.
pub const TEXT: Rgb = [0.93, 0.94, 0.96];
/// Labels and secondary text.
pub const DIM: Rgb = [0.58, 0.61, 0.67];
/// Text on an `accent` fill.
pub const ON_ACCENT: Rgb = [0.05, 0.05, 0.07];
pub const OK: Rgb = [0.5, 0.78, 0.55];
pub const WARN: Rgb = [0.92, 0.6, 0.28];
pub const BAD: Rgb = [0.92, 0.38, 0.34];

/// Colour tokens for one nation. Build with [`Theme::for_nation`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub panel: Rgb,
    pub inset: Rgb,
    pub text: Rgb,
    pub dim: Rgb,
    /// The nation colour, unchanged: fills, active button, emblem, brackets, seal.
    pub accent: Rgb,
    /// The accent lightened until it reads as text on `panel`.
    pub accent_text: Rgb,
    /// The accent blended into `panel`: panel frames, section rules, idle button borders.
    pub accent_soft: Rgb,
    pub on_accent: Rgb,
    pub ok: Rgb,
    pub warn: Rgb,
    pub bad: Rgb,
}

impl Theme {
    /// Tokens for a nation whose `color` in nations.toml is `rgb`.
    pub fn for_nation(rgb: [u8; 3]) -> Self {
        let accent = rgb.map(|c| f32::from(c) / 255.0);
        Self {
            panel: PANEL,
            inset: INSET,
            text: TEXT,
            dim: DIM,
            accent,
            accent_text: mix(accent, WHITE, 0.25),
            accent_soft: mix(accent, PANEL, 0.55),
            on_accent: ON_ACCENT,
            ok: OK,
            warn: WARN,
            bad: BAD,
        }
    }
}

/// Linear blend in sRGB space: `t = 0` is `a`, `t = 1` is `b`.
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// WCAG relative luminance of an sRGB colour.
pub fn luminance(c: Rgb) -> f32 {
    let linear = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let [r, g, b] = c.map(linear);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG contrast ratio between two colours, from 1 (identical) to 21.
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod tests {
    use fn_sim::{Defs, Nation};

    use super::*;

    #[test]
    fn accent_is_the_nation_colour() {
        let t = Theme::for_nation([90, 160, 80]);
        assert_eq!(t.accent, [90.0 / 255.0, 160.0 / 255.0, 80.0 / 255.0]);
    }

    /// Every bundled nation's accent text, and the shared text colours, stay readable on the
    /// panel (WCAG AA for body text: 4.5:1). Guards against a new nation colour in data.
    #[test]
    fn text_tokens_are_readable_on_the_panel_for_every_nation() {
        let defs = Defs::bundled();
        for n in Nation::ALL {
            let t = Theme::for_nation(defs.nation(n).color);
            for (name, c) in [("accent_text", t.accent_text), ("text", t.text), ("dim", t.dim)] {
                let ratio = contrast(c, t.panel);
                assert!(ratio >= 4.5, "{n:?} {name} contrast {ratio:.2} < 4.5");
            }
            let on = contrast(t.on_accent, t.accent);
            assert!(on >= 3.0, "{n:?} on_accent contrast {on:.2} < 3");
        }
    }

    #[test]
    fn mix_endpoints() {
        let (a, b) = ([0.0; 3], WHITE);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        assert_eq!(mix(a, b, 0.5), [0.5; 3]);
    }

    #[test]
    fn contrast_runs_from_one_to_twenty_one() {
        assert!((contrast(WHITE, WHITE) - 1.0).abs() < 1e-6);
        assert!((contrast(WHITE, [0.0; 3]) - 21.0).abs() < 1e-3);
    }
}

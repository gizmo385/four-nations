//! Colour depth: which of truecolor, 256, 16 or no colour the terminal gets, and the palette that
//! turns the design's sRGB colours into ratatui `Color`s for it.
//!
//! The renderer asks the [`Palette`] for every colour, so it never knows the depth; the 256- and
//! 16-colour modes are the same sRGB quantised to the nearest entry (docs/design/tui.md,
//! "Terminals without truecolor").

use fn_sim::{Defs, Nation};
use fn_ui::theme::{self, Rgb, Theme};
use ratatui::style::{Color, Modifier, Style};

use crate::glyphs::{Glyphs, Rgb8};

/// How many colours the terminal can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorDepth {
    TrueColor,
    Indexed256,
    Ansi16,
    /// No colour at all: reverse video, underline and the `!` marks carry the meaning.
    Never,
}

impl ColorDepth {
    /// Reads `--color`'s argument.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "truecolor" => Some(Self::TrueColor),
            "256" => Some(Self::Indexed256),
            "16" => Some(Self::Ansi16),
            "never" => Some(Self::Never),
            _ => None,
        }
    }

    /// The depth to use, once at start: `--color` if given, else `NO_COLOR` (set to anything
    /// non-empty), then `COLORTERM` of `truecolor` or `24bit` or a `WT_SESSION` (Windows
    /// Terminal, which WSL passes through but which doesn't set `COLORTERM`), then a `TERM`
    /// ending in `256color`, else 16. `env` looks a variable up, so the tests don't touch the
    /// process environment.
    pub fn detect(flag: Option<Self>, env: impl Fn(&str) -> Option<String>) -> Self {
        if let Some(depth) = flag {
            return depth;
        }
        if env("NO_COLOR").is_some_and(|v| !v.is_empty()) {
            return Self::Never;
        }
        if env("COLORTERM").is_some_and(|v| v == "truecolor" || v == "24bit")
            || env("WT_SESSION").is_some()
        {
            return Self::TrueColor;
        }
        if env("TERM").is_some_and(|v| v.ends_with("256color")) {
            return Self::Indexed256;
        }
        Self::Ansi16
    }
}

// ---------------------------------------------------------------------------------------------
// Quantising
// ---------------------------------------------------------------------------------------------

/// A distance that tracks what the eye sees better than plain RGB distance ("redmean").
fn distance(a: Rgb8, b: Rgb8) -> u32 {
    let rmean = (i32::from(a[0]) + i32::from(b[0])) / 2;
    let d = |i: usize| i32::from(a[i]) - i32::from(b[i]);
    let (dr, dg, db) = (d(0), d(1), d(2));
    ((((512 + rmean) * dr * dr) >> 8) + 4 * dg * dg + (((767 - rmean) * db * db) >> 8)) as u32
}

const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// The nearest xterm-256 entry from the colour cube (16..=231) or the grey ramp (232..=255). The
/// first 16 entries are left alone: terminals let the user redefine them.
pub fn nearest_256(rgb: Rgb8) -> u8 {
    let level = |v: u8| (0..6).min_by_key(|&i| u32::from(v.abs_diff(CUBE[i]))).unwrap();
    let [r, g, b] = rgb.map(level);
    let cube = [CUBE[r], CUBE[g], CUBE[b]];
    let mut best = (distance(rgb, cube), 16 + 36 * r + 6 * g + b);
    for i in 0..24 {
        let v = 8 + 10 * i as u8;
        let d = distance(rgb, [v; 3]);
        if d < best.0 {
            best = (d, 232 + i);
        }
    }
    best.1 as u8
}

/// The colour of xterm-256 entry `i` (16..=255; the first 16 are the terminal's to redefine).
fn entry_256(i: u8) -> Rgb8 {
    if i >= 232 {
        [8 + 10 * (i - 232); 3]
    } else {
        let i = usize::from(i - 16);
        [CUBE[i / 36], CUBE[i / 6 % 6], CUBE[i % 6]]
    }
}

/// How far apart (in [`distance`]) two backgrounds must be to count as different at a glance:
/// about one step of the colour cube in every channel.
const MIN_SEPARATION: u32 = 8000;

/// The xterm-256 entry nearest `target` that is at least [`MIN_SEPARATION`] from every colour in
/// `avoid`; the nearest of all if the palette has none.
fn nearest_256_apart(target: Rgb8, avoid: &[Rgb8]) -> u8 {
    let apart = |i: &u8| avoid.iter().all(|&a| distance(entry_256(*i), a) >= MIN_SEPARATION);
    (16..=255u8)
        .filter(apart)
        .min_by_key(|&i| distance(entry_256(i), target))
        .unwrap_or_else(|| nearest_256(target))
}

/// The 16 ANSI colours with the xterm default values, and the `Color` that names each.
const ANSI: [(Color, Rgb8); 16] = [
    (Color::Black, [0, 0, 0]),
    (Color::Red, [205, 0, 0]),
    (Color::Green, [0, 205, 0]),
    (Color::Yellow, [205, 205, 0]),
    (Color::Blue, [0, 0, 238]),
    (Color::Magenta, [205, 0, 205]),
    (Color::Cyan, [0, 205, 205]),
    (Color::Gray, [229, 229, 229]),
    (Color::DarkGray, [127, 127, 127]),
    (Color::LightRed, [255, 0, 0]),
    (Color::LightGreen, [0, 255, 0]),
    (Color::LightYellow, [255, 255, 0]),
    (Color::LightBlue, [92, 92, 255]),
    (Color::LightMagenta, [255, 0, 255]),
    (Color::LightCyan, [0, 255, 255]),
    (Color::White, [255, 255, 255]),
];

/// The index into [`ANSI`] of the entry nearest `rgb`, skipping the indices in `taken`.
fn nearest_ansi_index(rgb: Rgb8, taken: &[usize]) -> usize {
    (0..ANSI.len())
        .filter(|i| !taken.contains(i))
        .min_by_key(|&i| distance(rgb, ANSI[i].1))
        .expect("16 colours, a handful taken")
}

pub fn nearest_16(rgb: Rgb8) -> Color {
    ANSI[nearest_ansi_index(rgb, &[])].0
}

/// `rgb` as a `Color` on a terminal of `depth`; `Reset` (the terminal's own colour) for `Never`.
pub fn quantize(rgb: Rgb8, depth: ColorDepth) -> Color {
    match depth {
        ColorDepth::TrueColor => Color::Rgb(rgb[0], rgb[1], rgb[2]),
        ColorDepth::Indexed256 => Color::Indexed(nearest_256(rgb)),
        ColorDepth::Ansi16 => nearest_16(rgb),
        ColorDepth::Never => Color::Reset,
    }
}

pub fn to_u8(c: Rgb) -> Rgb8 {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

pub fn to_f32(c: Rgb8) -> Rgb {
    c.map(|v| f32::from(v) / 255.0)
}

/// Linear blend of two sRGB colours: `t = 0` is `a`, `t = 1` is `b`.
pub fn mix(a: Rgb8, b: Rgb8, t: f32) -> Rgb8 {
    to_u8(theme::mix(to_f32(a), to_f32(b), t))
}

// ---------------------------------------------------------------------------------------------
// The palette
// ---------------------------------------------------------------------------------------------

/// The background tints that mark designations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Harvest,
    /// A harvest tile with a standing order on it (#81, #277): the same hue as `Harvest`, pushed
    /// toward green so it reads as "standing" rather than "one-shot" (mirrors the client's
    /// `designate::Layer::RepeatingHarvest`).
    RepeatingHarvest,
    Stockpile,
    /// The nation's accent.
    Bend,
    Build,
    Wet,
    /// The reach of the flame under the cursor.
    Warm,
}

impl Tint {
    /// The tint a tool's rectangle (and its result) wears: the client's hues. Cancelling shows
    /// the harvest hue, as the client's drag does. `None` for the tool that designates nothing.
    pub fn for_tool(tool: fn_ui::Tool) -> Option<Self> {
        use fn_ui::Tool;
        match tool {
            // Sowing wears the harvest hue too: both are "designate a unit's work here" (#198).
            Tool::Harvest | Tool::CancelHarvest | Tool::Sow => Some(Self::Harvest),
            Tool::Stockpile => Some(Self::Stockpile),
            Tool::Bend => Some(Self::Bend),
            Tool::Build => Some(Self::Build),
            Tool::Select => None,
        }
    }
}

/// How much of a tint is mixed into the tile's own background.
const TINT_STRENGTH: f32 = 0.5;

/// How much of a tint the rectangle being dragged mixes in: more than a designation, so the two
/// can be told apart where they overlap.
const DRAG_STRENGTH: f32 = 0.85;

/// Everything the renderer needs to turn the design into styled cells: the glyph table, the
/// nation's theme, the colour depth and whether the glyphs are the ASCII set.
#[derive(Clone, Debug)]
pub struct Palette {
    pub glyphs: Glyphs,
    pub theme: Theme,
    pub depth: ColorDepth,
    pub ascii: bool,
    /// The nation colour, unchanged: the bend tint.
    accent: Rgb8,
    /// The nation colour lifted toward white: its units.
    unit: Rgb8,
    /// On 16 colours a tint can't be mixed into a background (every dark background is plain
    /// black), so each designation gets a colour of its own, picked so that no two share one,
    /// and so do the Build tool's ghost: free and blocked (#212), and its door tile when it's
    /// free (#230; a blocked door reuses the ghost's own blocked colour, index 6), and a
    /// harvest's standing order (#277, the last one here).
    solid: [Color; 8],
}

impl Palette {
    pub fn new(defs: &Defs, nation: Nation, depth: ColorDepth, ascii: bool) -> Self {
        let glyphs = Glyphs::bundled();
        let accent = defs.nation(nation).color;
        let unit = mix(accent, [255; 3], glyphs.unit.lift);
        let o = &glyphs.overlay;
        // Cursor first, so no designation takes its colour.
        let mut taken = vec![0]; // Black is the background.
        // A designation must not look like bare ground either.
        let grounds = glyphs.terrain.values().chain(glyphs.biome.values());
        taken.extend(grounds.filter_map(|l| l.bg).map(|bg| nearest_ansi_index(bg, &[])));
        let mut pick = |rgb| {
            let i = nearest_ansi_index(rgb, &taken);
            taken.push(i);
            ANSI[i].0
        };
        let _cursor = pick(o.cursor);
        let theme = Theme::for_nation(accent);
        let solid = [
            pick(o.harvest),
            pick(o.stockpile),
            pick(accent),
            pick(o.build),
            pick(to_u8(theme.ok)),
            pick(to_u8(theme.bad)),
            pick(to_u8(theme.warn)),
            pick(o.repeating),
        ];
        Self { theme, glyphs, depth, ascii, accent, unit, solid }
    }

    pub fn color(&self, rgb: Rgb8) -> Color {
        quantize(rgb, self.depth)
    }

    /// A theme token (`fn_ui::theme`) as a `Color`.
    pub fn token(&self, rgb: Rgb) -> Color {
        self.color(to_u8(rgb))
    }

    /// No colour: the designs fall back on reverse video and underline.
    pub fn mono(&self) -> bool {
        self.depth == ColorDepth::Never
    }

    /// The nation colour lifted toward white, in sRGB.
    pub fn unit_rgb(&self) -> Rgb8 {
        self.unit
    }

    /// The nation's unit colour as a `Color`.
    pub fn unit(&self) -> Color {
        self.color(self.unit)
    }

    /// A unit's own colour, in sRGB, whatever its nation (#248, the United City's newcomers,
    /// #240): `nation`'s own accent lifted toward white the same amount as the settlement's own
    /// (`unit_rgb`), so a foreign-nation unit reads clearly without standing out more than the
    /// settlement's own do.
    pub fn unit_rgb_for(&self, defs: &Defs, nation: Nation) -> Rgb8 {
        mix(defs.nation(nation).color, [255; 3], self.glyphs.unit.lift)
    }

    /// The tint's sRGB colour.
    pub fn tint_rgb(&self, tint: Tint) -> Rgb8 {
        let o = &self.glyphs.overlay;
        match tint {
            Tint::Harvest => o.harvest,
            Tint::RepeatingHarvest => o.repeating,
            Tint::Stockpile => o.stockpile,
            Tint::Bend => self.accent,
            Tint::Build => o.build,
            Tint::Wet => o.wet,
            Tint::Warm => o.warm,
        }
    }

    /// The background of a tile whose own background is `base` once `tint` marks it.
    pub fn tinted(&self, base: Rgb8, tint: Tint) -> Color {
        match (self.depth, tint) {
            (ColorDepth::Never, _) => Color::Reset,
            (ColorDepth::Ansi16, Tint::Harvest) => self.solid[0],
            (ColorDepth::Ansi16, Tint::Stockpile) => self.solid[1],
            (ColorDepth::Ansi16, Tint::Bend) => self.solid[2],
            (ColorDepth::Ansi16, Tint::Build) => self.solid[3],
            (ColorDepth::Ansi16, Tint::RepeatingHarvest) => self.solid[7],
            (_, Tint::Wet) => self.color(mix(base, self.tint_rgb(tint), 0.75)),
            (ColorDepth::Indexed256, _) => Color::Indexed(self.designation_256(base, tint)),
            (_, _) => self.color(mix(base, self.tint_rgb(tint), TINT_STRENGTH)),
        }
    }

    /// The background of a tile of the rectangle being dragged with a tool that wears `tint`.
    /// Stronger than the designation itself; on 16 colours the designation's own solid colour,
    /// and with no colour `Color::Reset` (the caller marks the cell another way).
    pub fn dragged(&self, base: Rgb8, tint: Tint) -> Color {
        let target = mix(base, self.tint_rgb(tint), DRAG_STRENGTH);
        match self.depth {
            ColorDepth::Never => Color::Reset,
            ColorDepth::Ansi16 => self.tinted(base, tint),
            ColorDepth::Indexed256 => {
                Color::Indexed(nearest_256_apart(target, &[base, self.glyphs.overlay.cursor]))
            }
            ColorDepth::TrueColor => self.color(target),
        }
    }

    /// The background of a tile of the Build tool's ghost (#212): the theme's `ok` green where
    /// the building can go, its `bad` red where something is in the way. As strong as a drag, so
    /// it reads over a designation; with no colour `Color::Reset` (the caller marks the cell
    /// another way).
    pub fn ghost(&self, base: Rgb8, free: bool) -> Color {
        let rgb = to_u8(if free { self.theme.ok } else { self.theme.bad });
        let target = mix(base, rgb, DRAG_STRENGTH);
        match self.depth {
            ColorDepth::Never => Color::Reset,
            ColorDepth::Ansi16 => self.solid[if free { 4 } else { 5 }],
            ColorDepth::Indexed256 => {
                Color::Indexed(nearest_256_apart(target, &[base, self.glyphs.overlay.cursor]))
            }
            ColorDepth::TrueColor => self.color(target),
        }
    }

    /// The background of the entrance tile the Build tool's ghost marks (#230): the theme's
    /// `warn` amber where the door would be free to use, else the ghost's own blocked colour
    /// (`ghost`'s red), so a door reads differently from the green of a plain tile that fits
    /// but the same red everywhere a placement is refused. With no colour `Color::Reset` (the
    /// caller marks the cell another way).
    pub fn door(&self, base: Rgb8, free: bool) -> Color {
        if !free {
            return self.ghost(base, false);
        }
        let target = mix(base, to_u8(self.theme.warn), DRAG_STRENGTH);
        match self.depth {
            ColorDepth::Never => Color::Reset,
            ColorDepth::Ansi16 => self.solid[6],
            ColorDepth::Indexed256 => {
                Color::Indexed(nearest_256_apart(target, &[base, self.glyphs.overlay.cursor]))
            }
            ColorDepth::TrueColor => self.color(target),
        }
    }

    /// A designation's background on 256 colours. Mixed into a dark ground, an amber harvest and
    /// an Air Nomad's yellow bend land on the same palette entry, so each tint, in priority order,
    /// takes the nearest entry that is clearly apart from the ground, the cursor and the tints
    /// before it.
    fn designation_256(&self, base: Rgb8, tint: Tint) -> u8 {
        let mut avoid = vec![base, self.glyphs.overlay.cursor];
        for t in [Tint::Harvest, Tint::RepeatingHarvest, Tint::Stockpile, Tint::Bend, Tint::Build] {
            let target = mix(base, self.tint_rgb(t), TINT_STRENGTH);
            let pick = nearest_256_apart(target, &avoid);
            if t == tint {
                return pick;
            }
            avoid.push(entry_256(pick));
        }
        unreachable!("the caller passes a designation tint")
    }

    /// The cursor's background. On 16 colours its own colour, apart from every tint's.
    pub fn cursor_bg(&self) -> Color {
        self.color(self.glyphs.overlay.cursor)
    }

    /// The style of the cursor tile's text: black on the cursor colour, or reverse video when
    /// there is no colour.
    pub fn cursor_style(&self) -> Style {
        if self.mono() {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new().fg(self.color([0, 0, 0])).bg(self.cursor_bg())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::glyphs::TERRAINS;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        let map: HashMap<&str, &str> = pairs.iter().copied().collect();
        move |k| map.get(k).map(|v| (*v).to_owned())
    }

    #[test]
    fn the_flag_beats_the_environment() {
        let e = env(&[("NO_COLOR", "1"), ("COLORTERM", "truecolor")]);
        assert_eq!(ColorDepth::detect(Some(ColorDepth::Ansi16), &e), ColorDepth::Ansi16);
    }

    #[test]
    fn no_color_beats_colorterm() {
        let e = env(&[("NO_COLOR", "1"), ("COLORTERM", "truecolor")]);
        assert_eq!(ColorDepth::detect(None, e), ColorDepth::Never);
        // The no-color.org rule: an empty value doesn't count.
        let e = env(&[("NO_COLOR", ""), ("COLORTERM", "truecolor")]);
        assert_eq!(ColorDepth::detect(None, e), ColorDepth::TrueColor);
    }

    #[test]
    fn truecolor_comes_from_colorterm_or_windows_terminal() {
        for pairs in [
            &[("COLORTERM", "truecolor")][..],
            &[("COLORTERM", "24bit"), ("TERM", "xterm")][..],
            &[("WT_SESSION", "abc"), ("TERM", "xterm")][..],
        ] {
            assert_eq!(ColorDepth::detect(None, env(pairs)), ColorDepth::TrueColor, "{pairs:?}");
        }
    }

    #[test]
    fn term_decides_between_256_and_16() {
        assert_eq!(
            ColorDepth::detect(None, env(&[("TERM", "xterm-256color")])),
            ColorDepth::Indexed256
        );
        assert_eq!(ColorDepth::detect(None, env(&[("TERM", "xterm")])), ColorDepth::Ansi16);
        assert_eq!(ColorDepth::detect(None, env(&[])), ColorDepth::Ansi16);
        assert_eq!(
            ColorDepth::detect(None, env(&[("COLORTERM", "yes"), ("TERM", "screen-256color")])),
            ColorDepth::Indexed256
        );
    }

    #[test]
    fn color_flag_values() {
        assert_eq!(ColorDepth::parse("256"), Some(ColorDepth::Indexed256));
        assert_eq!(ColorDepth::parse("never"), Some(ColorDepth::Never));
        assert_eq!(ColorDepth::parse("rgb"), None);
    }

    #[test]
    fn the_256_palette_picks_cube_and_grey_entries() {
        assert_eq!(nearest_256([255, 0, 0]), 196);
        assert_eq!(nearest_256([0, 0, 0]), 16);
        assert_eq!(nearest_256([255, 255, 255]), 231);
        // A mid grey is closer to the grey ramp than to any cube corner.
        assert_eq!(nearest_256([128, 128, 128]), 244);
        assert_eq!(nearest_256([95, 135, 175]), 16 + 36 + 6 * 2 + 3);
    }

    #[test]
    fn the_16_palette_picks_the_nearest_ansi_colour() {
        assert_eq!(nearest_16([0, 0, 0]), Color::Black);
        assert_eq!(nearest_16([250, 20, 10]), Color::LightRed);
        assert_eq!(nearest_16([20, 20, 200]), Color::Blue);
        assert_eq!(quantize([1, 2, 3], ColorDepth::TrueColor), Color::Rgb(1, 2, 3));
        assert_eq!(quantize([1, 2, 3], ColorDepth::Never), Color::Reset);
    }

    /// Every background the map can have: each terrain and each biome.
    fn bases(g: &Glyphs) -> Vec<(String, Rgb8)> {
        let mut out = Vec::new();
        for t in TERRAINS {
            out.push((format!("{t:?}"), g.terrain(t).unwrap().bg.unwrap()));
        }
        for (id, l) in &g.biome {
            out.push((id.clone(), l.bg.unwrap()));
        }
        out
    }

    /// The units read on every ground, for every nation: the lifted nation colour has WCAG
    /// contrast of at least 3:1 (large-text level, which a bold glyph is) on each background.
    /// Earth's green on green plains is the case this guards.
    #[test]
    fn units_contrast_with_every_background_for_every_nation() {
        let defs = Defs::bundled();
        for n in Nation::ALL {
            let p = Palette::new(&defs, n, ColorDepth::TrueColor, false);
            for (what, bg) in bases(&p.glyphs) {
                let ratio = theme::contrast(to_f32(p.unit_rgb()), to_f32(bg));
                assert!(ratio >= 3.0, "{n:?} unit on {what}: contrast {ratio:.2} < 3");
            }
        }
    }

    /// #248: a newcomer of any nation reads clearly on every background too, whatever the
    /// settlement's own nation (unlike `unit_rgb`, `unit_rgb_for` doesn't depend on which nation
    /// founded the settlement, so one `Palette` covers every caster's colour).
    #[test]
    fn every_nations_own_unit_colour_contrasts_with_every_background() {
        let defs = Defs::bundled();
        let p = Palette::new(&defs, Nation::Fire, ColorDepth::TrueColor, false);
        assert_eq!(p.unit_rgb_for(&defs, Nation::Fire), p.unit_rgb(), "agrees with the settlement");
        for n in Nation::ALL {
            for (what, bg) in bases(&p.glyphs) {
                let ratio = theme::contrast(to_f32(p.unit_rgb_for(&defs, n)), to_f32(bg));
                assert!(ratio >= 3.0, "{n:?} unit on {what}: contrast {ratio:.2} < 3");
            }
        }
    }

    /// A harvest, a standing-order harvest (#277), stockpile, bend (and build) designation each
    /// look different from the others and from the cursor, on every ground, once quantised to
    /// 256 or 16 colours, for every nation.
    #[test]
    fn designation_tints_stay_distinct_after_quantising() {
        let defs = Defs::bundled();
        for n in Nation::ALL {
            for depth in [ColorDepth::TrueColor, ColorDepth::Indexed256, ColorDepth::Ansi16] {
                let p = Palette::new(&defs, n, depth, false);
                for (what, base) in bases(&p.glyphs) {
                    let shown = [
                        ("harvest", p.tinted(base, Tint::Harvest)),
                        ("repeating", p.tinted(base, Tint::RepeatingHarvest)),
                        ("stockpile", p.tinted(base, Tint::Stockpile)),
                        ("bend", p.tinted(base, Tint::Bend)),
                        ("build", p.tinted(base, Tint::Build)),
                        ("cursor", p.cursor_bg()),
                        ("ground", p.color(base)),
                    ];
                    for (i, (a, ca)) in shown.iter().enumerate() {
                        for (b, cb) in &shown[i + 1..] {
                            assert_ne!(ca, cb, "{n:?} {depth:?} on {what}: {a} == {b}");
                        }
                    }
                }
            }
        }
    }

    /// The rectangle being dragged looks different from a designation of its own kind (so the
    /// two can be told apart where they overlap), from the ground and from the cursor, on every
    /// ground and nation, in every colour depth that has colour.
    #[test]
    fn the_dragged_rectangle_stands_out_from_designations_the_ground_and_the_cursor() {
        let defs = Defs::bundled();
        for n in Nation::ALL {
            for depth in [ColorDepth::TrueColor, ColorDepth::Indexed256, ColorDepth::Ansi16] {
                let p = Palette::new(&defs, n, depth, false);
                for (what, base) in bases(&p.glyphs) {
                    for tint in [Tint::Harvest, Tint::Stockpile, Tint::Bend] {
                        let dragged = p.dragged(base, tint);
                        let who = format!("{n:?} {depth:?} {tint:?} on {what}");
                        assert_ne!(dragged, p.color(base), "{who}: ground");
                        assert_ne!(dragged, p.cursor_bg(), "{who}: cursor");
                        if depth == ColorDepth::TrueColor {
                            assert_ne!(dragged, p.tinted(base, tint), "{who}: designation");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn tools_wear_the_clients_hues() {
        use fn_ui::Tool;
        assert_eq!(Tint::for_tool(Tool::Harvest), Some(Tint::Harvest));
        assert_eq!(Tint::for_tool(Tool::CancelHarvest), Some(Tint::Harvest));
        assert_eq!(Tint::for_tool(Tool::Stockpile), Some(Tint::Stockpile));
        assert_eq!(Tint::for_tool(Tool::Bend), Some(Tint::Bend));
        assert_eq!(Tint::for_tool(Tool::Select), None);
    }

    #[test]
    fn there_is_no_colour_in_never_mode() {
        let defs = Defs::bundled();
        let p = Palette::new(&defs, Nation::Fire, ColorDepth::Never, false);
        assert_eq!(p.tinted([10, 20, 30], Tint::Harvest), Color::Reset);
        assert_eq!(p.color([10, 20, 30]), Color::Reset);
        assert!(p.cursor_style().add_modifier.contains(Modifier::REVERSED));
        assert_eq!(p.door([10, 20, 30], true), Color::Reset);
        assert_eq!(p.door([10, 20, 30], false), Color::Reset);
    }

    /// #230: a free door reads differently from a blocked one, from a plain ghost tile that
    /// fits, and from bare ground, in every colour depth that has colour; a blocked door looks
    /// exactly like the ghost's own blocked tile, since both mean "refused".
    #[test]
    fn a_free_door_stands_out_from_a_blocked_one_the_ghost_and_the_ground() {
        let defs = Defs::bundled();
        for n in Nation::ALL {
            for depth in [ColorDepth::TrueColor, ColorDepth::Indexed256, ColorDepth::Ansi16] {
                let p = Palette::new(&defs, n, depth, false);
                for (what, base) in bases(&p.glyphs) {
                    let who = format!("{n:?} {depth:?} on {what}");
                    assert_eq!(p.door(base, false), p.ghost(base, false), "{who}");
                    assert_ne!(p.door(base, true), p.door(base, false), "{who}");
                    assert_ne!(p.door(base, true), p.ghost(base, true), "{who}: fits");
                    assert_ne!(p.door(base, true), p.color(base), "{who}: ground");
                }
            }
        }
    }

    #[test]
    fn mixing_endpoints() {
        assert_eq!(mix([0, 0, 0], [200, 100, 50], 0.0), [0, 0, 0]);
        assert_eq!(mix([0, 0, 0], [200, 100, 50], 1.0), [200, 100, 50]);
        assert_eq!(mix([0, 0, 0], [200, 100, 50], 0.5), [100, 50, 25]);
    }
}

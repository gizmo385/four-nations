//! `data/tui.toml`: a glyph, an ASCII fallback and colours for everything the map draws.
//!
//! Presentation only. `fn_sim` never reads it; this crate embeds it with `include_str!`, like
//! `Defs::bundled`, and the tests fail if the game data gains a terrain, biome, resource, item or
//! building without an entry here.

use std::collections::BTreeMap;

use fn_sim::Terrain;
use serde::Deserialize;

/// An sRGB colour, `[r, g, b]`.
pub type Rgb8 = [u8; 3];

/// How one thing draws: a glyph (or its `--ascii` fallback) in `fg`, on `bg` when it has one.
/// Resources, items and buildings have no `bg`: they draw on their tile's own colour.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    pub glyph: char,
    pub ascii: char,
    pub fg: Rgb8,
    pub bg: Option<Rgb8>,
    /// A living plant (a tree, a crop): the legend (#662) lists it under Plants, not Terrain.
    /// Only a resource says so.
    #[serde(default)]
    pub plant: bool,
}

impl Look {
    pub fn glyph(&self, ascii: bool) -> char {
        if ascii { self.ascii } else { self.glyph }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitLook {
    pub glyph: char,
    pub ascii: char,
    /// How far the nation colour is mixed toward white, in `[0, 1]`.
    pub lift: f32,
    /// The same, for a bending master (#360, #361, `World::rank_of`): mixed further toward
    /// white, on top of the modifier every unit already wears (`Modifier::BOLD`), so a master
    /// stands out at a glance, styled from here rather than hard-coded.
    pub master_lift: f32,
}

impl UnitLook {
    pub fn glyph(&self, ascii: bool) -> char {
        if ascii { self.ascii } else { self.glyph }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayGlyph {
    pub glyph: char,
    pub ascii: char,
    pub fg: Rgb8,
}

impl OverlayGlyph {
    pub fn glyph(&self, ascii: bool) -> char {
        if ascii { self.ascii } else { self.glyph }
    }
}

/// Which of a rail tile's four neighbours are rail (#279): bit 0 north, bit 1 east, bit 2 south,
/// bit 3 west, the same shape as the client's `sprites::buildings::LINK_*` bits (#135), though
/// the two frontends pick their own glyph for it.
pub const RAIL_N: u8 = 1;
pub const RAIL_E: u8 = 2;
pub const RAIL_S: u8 = 4;
pub const RAIL_W: u8 = 8;

/// Which neighbours of a rail tile are rail, as a [`RAIL_N`]-[`RAIL_W`] mask for [`link_glyph`].
pub fn rail_mask(n: bool, e: bool, s: bool, w: bool) -> u8 {
    u8::from(n) * RAIL_N + u8::from(e) * RAIL_E + u8::from(s) * RAIL_S + u8::from(w) * RAIL_W
}

/// The fifteen box-drawing pieces a rail tile or site with at least one rail neighbour picks
/// between (`[rail_track]`, #279), indexed `mask - 1` by the mask [`rail_mask`] builds.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RailLinks {
    pub glyph: Vec<char>,
    pub ascii: Vec<char>,
}

/// The piece of track a rail tile with these neighbours draws (#279): `None` for a lone tile
/// (`mask == 0`), which keeps the rail building's own glyph instead. Pure: a table lookup, with
/// no sim or terminal state.
pub fn link_glyph(links: &RailLinks, mask: u8, ascii: bool) -> Option<char> {
    let table = if ascii { &links.ascii } else { &links.glyph };
    let index = usize::from(mask.checked_sub(1)?);
    table.get(index).copied()
}

/// The embark screen's overview thresholds (`[embark]`, #290, docs/design/embark.md).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbarkGlyphDef {
    pub min_overview_scale: i32,
    pub sea_share: f64,
    pub mountain_share: f64,
    pub river_lake_share: f64,
}

/// The side panel's Season section's presentation threshold (`[season]`, #299,
/// docs/design/difficulty.md "TUI"): not a sim rule (`data/seasons.toml` has none of it), just
/// when the food line turns to the warn tone.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeasonGlyphDef {
    /// Food per head below which, in autumn or winter, the Season section's food line warns.
    pub winter_food_per_head: u32,
}

/// The background tints (`[overlay]`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overlays {
    pub harvest: Rgb8,
    /// A harvest tile with a standing order on it (#81, #277): the client's `RepeatingHarvest`.
    pub repeating: Rgb8,
    /// A Clear mark (#411/#426): apart from `harvest` and `repeating`.
    pub clear: Rgb8,
    pub stockpile: Rgb8,
    pub build: Rgb8,
    pub wet: Rgb8,
    pub cursor: Rgb8,
    pub warm: Rgb8,
    /// A power source's reach (#428, #429): the cursor on a finished source, or the Build tool
    /// placing one, the way `warm` shows a flame's reach.
    pub powered: Rgb8,
    pub flame: OverlayGlyph,
    /// A fruit tree an orchard planted that hasn't borne fruit yet (#142).
    pub sapling: OverlayGlyph,
    /// A farm plot that has been sown and hasn't ripened yet (#198).
    pub plot_growing: OverlayGlyph,
    /// A farm plot ripe for reaping (#198).
    pub plot_ripe: OverlayGlyph,
    /// Earth's Earth Tent, lit (#362): a shelter apart from `flame`.
    pub earth_tent: OverlayGlyph,
    /// Air's Air Shield, lit (#362): a shelter apart from `flame`.
    pub air_shield: OverlayGlyph,
    /// A chute's link line (#683): `:` from its head to its foot.
    pub link: OverlayGlyph,
}

/// The watcher's studio layout thresholds and proportions (`[studio]`,
/// docs/design/agent-studio.md "Layout"): presentation only, not a sim rule.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudioDef {
    /// Below this width (or `min_height`), the studio's three panes become tabs instead.
    pub min_width: u16,
    pub min_height: u16,
    /// The game column's share of the terminal's width, at least `game_min_width`.
    pub game_share: f64,
    pub game_min_width: u16,
    /// The code column's widest: past it, every extra column goes to the game column instead.
    pub code_max_width: u16,
    /// The agent feed's height: `max(feed_min_rows, height / 4)`.
    pub feed_min_rows: u16,
    /// The sparkline's window, in turns, in the studio and in the tab strip.
    pub spark_turns: u16,
    pub spark_turns_tabs: u16,
    /// Acts in a turn that fill the sparkline's bar, at least.
    pub spark_floor: u16,
}

impl StudioDef {
    /// The game column's width for a terminal `width` columns wide (docs/design/agent-studio.md,
    /// "Layout"): `game_share` of it, at least `game_min_width`, and wide enough that the code
    /// column gets no more than `code_max_width`.
    pub fn game_width(&self, width: u16) -> u16 {
        let share = (f64::from(width) * self.game_share).round() as u16;
        share.max(self.game_min_width).max(width.saturating_sub(self.code_max_width)).min(width)
    }

    /// Whether a `width` x `height` terminal is big enough for the studio layout rather than the
    /// tab fallback.
    pub fn fits(&self, width: u16, height: u16) -> bool {
        width >= self.min_width && height >= self.min_height
    }
}

/// Rhai syntax colours for the studio's code pane (`[code]`), by `fn_ui::code::Class`
/// (docs/design/agent-studio.md, "Highlighting"). `Class::Plain` has no entry: it draws in the
/// terminal's own default foreground.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeColors {
    pub keyword: Rgb8,
    pub literal: Rgb8,
    pub number: Rgb8,
    pub string: Rgb8,
    pub comment: Rgb8,
    pub call: Rgb8,
    pub op: Rgb8,
    /// The gutter's line numbers.
    pub gutter: Rgb8,
    /// Mixed into an added line's own background.
    pub added_bg: Rgb8,
    /// The `▌` marker beside an added line.
    pub added: Rgb8,
    /// The `▁` marker on the line just above a deletion.
    pub removed: Rgb8,
}

impl CodeColors {
    /// The colour for `class`, or `None` for [`fn_ui::code::Class::Plain`] -- there's no
    /// `[code]` entry for it (the table above), so plain text keeps the terminal's own default
    /// foreground.
    pub fn for_class(&self, class: fn_ui::code::Class) -> Option<Rgb8> {
        use fn_ui::code::Class;
        match class {
            Class::Plain => None,
            Class::Keyword => Some(self.keyword),
            Class::Literal => Some(self.literal),
            Class::Number => Some(self.number),
            Class::String => Some(self.string),
            Class::Comment => Some(self.comment),
            Class::Call => Some(self.call),
            Class::Op => Some(self.op),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Glyphs {
    pub terrain: BTreeMap<String, Look>,
    pub biome: BTreeMap<String, Look>,
    pub resource: BTreeMap<String, Look>,
    pub item: BTreeMap<String, Look>,
    pub building: BTreeMap<String, Look>,
    /// How each beast draws (#699), by its id in `data/beasts.toml`: the tests fail if a row
    /// there has no entry here.
    pub beast: BTreeMap<String, OverlayGlyph>,
    pub rail_track: RailLinks,
    pub unit: UnitLook,
    /// The Avatar's own look (#237, #242): always `@`, white, drawn on the nation's accent
    /// instead of his tile's own background (`map::tile_cell`).
    pub avatar: OverlayGlyph,
    /// The Harmony Gate's site, by the current stage's id in `data/endeavor.toml` (#242): a 4x4
    /// glyph block that tells the stage apart, in place of the generic dimmed site
    /// (`map::gate_stage_look`). A stage missing here falls back to the generic site.
    pub gate_stage: BTreeMap<String, OverlayGlyph>,
    pub overlay: Overlays,
    pub embark: EmbarkGlyphDef,
    pub season: SeasonGlyphDef,
    pub studio: StudioDef,
    pub code: CodeColors,
}

/// The key of a terrain in `[terrain.*]`. `Land` has none: bare land draws as its biome. The
/// match is exhaustive, so a new terrain kind can't be forgotten here.
pub fn terrain_key(terrain: Terrain) -> Option<&'static str> {
    match terrain {
        Terrain::Water => Some("water"),
        Terrain::Coast => Some("coast"),
        Terrain::Land => None,
        Terrain::Mountain => Some("mountain"),
        Terrain::River => Some("river"),
        Terrain::Canal => Some("canal"),
        Terrain::Lake => Some("lake"),
        Terrain::Ford => Some("ford"),
        Terrain::Tunnel => Some("tunnel"),
    }
}

/// Every terrain kind that has a `[terrain.*]` entry.
pub const TERRAINS: [Terrain; 8] = [
    Terrain::Water,
    Terrain::Coast,
    Terrain::Mountain,
    Terrain::River,
    Terrain::Canal,
    Terrain::Lake,
    Terrain::Ford,
    Terrain::Tunnel,
];

impl Glyphs {
    /// The glyphs shipped in `data/tui.toml`, embedded at compile time.
    pub fn bundled() -> Self {
        Self::from_toml(include_str!("../../../data/tui.toml"))
            .expect("bundled data/tui.toml must be valid; run `cargo test -p fn_tui`")
    }

    pub fn from_toml(src: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(src)
    }

    pub fn terrain(&self, terrain: Terrain) -> Option<&Look> {
        self.terrain.get(terrain_key(terrain)?)
    }

    /// The look of the beast whose row is `id` in `data/beasts.toml`.
    pub fn beast(&self, id: &str) -> &OverlayGlyph {
        self.beast.get(id).unwrap_or_else(|| panic!("beast {id} has no look (tested)"))
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Defs;
    use unicode_width::UnicodeWidthChar;

    use super::*;

    /// Every `Look` in the file with where it is, for the per-entry checks.
    fn all_looks(g: &Glyphs) -> Vec<(String, char, char)> {
        let tables = [
            ("terrain", &g.terrain),
            ("biome", &g.biome),
            ("resource", &g.resource),
            ("item", &g.item),
            ("building", &g.building),
        ];
        let mut out: Vec<_> = tables
            .into_iter()
            .flat_map(|(table, looks)| {
                looks.iter().map(move |(id, l)| (format!("{table}.{id}"), l.glyph, l.ascii))
            })
            .collect();
        out.push(("unit".into(), g.unit.glyph, g.unit.ascii));
        out.push(("avatar".into(), g.avatar.glyph, g.avatar.ascii));
        for (id, look) in &g.beast {
            out.push((format!("beast.{id}"), look.glyph, look.ascii));
        }
        out.push(("overlay.flame".into(), g.overlay.flame.glyph, g.overlay.flame.ascii));
        out.push(("overlay.sapling".into(), g.overlay.sapling.glyph, g.overlay.sapling.ascii));
        out.push((
            "overlay.plot_growing".into(),
            g.overlay.plot_growing.glyph,
            g.overlay.plot_growing.ascii,
        ));
        out.push((
            "overlay.plot_ripe".into(),
            g.overlay.plot_ripe.glyph,
            g.overlay.plot_ripe.ascii,
        ));
        out.push((
            "overlay.earth_tent".into(),
            g.overlay.earth_tent.glyph,
            g.overlay.earth_tent.ascii,
        ));
        out.push((
            "overlay.air_shield".into(),
            g.overlay.air_shield.glyph,
            g.overlay.air_shield.ascii,
        ));
        for (id, look) in &g.gate_stage {
            out.push((format!("gate_stage.{id}"), look.glyph, look.ascii));
        }
        for mask in 1..=15u8 {
            out.push((
                format!("rail_track[{mask}]"),
                link_glyph(&g.rail_track, mask, false).unwrap(),
                link_glyph(&g.rail_track, mask, true).unwrap(),
            ));
        }
        out
    }

    fn keys<T>(m: &BTreeMap<String, T>) -> Vec<&str> {
        m.keys().map(String::as_str).collect()
    }

    #[test]
    fn every_id_in_the_game_data_has_an_entry_and_nothing_else_does() {
        let (g, defs) = (Glyphs::bundled(), Defs::bundled());
        fn want(mut ids: Vec<&str>) -> Vec<&str> {
            ids.sort_unstable();
            ids
        }
        assert_eq!(
            keys(&g.terrain),
            want(TERRAINS.iter().filter_map(|&t| terrain_key(t)).collect())
        );
        assert_eq!(keys(&g.biome), want(defs.biomes.iter().map(|b| b.id.as_str()).collect()));
        assert_eq!(keys(&g.resource), want(defs.resources.iter().map(|r| r.id.as_str()).collect()));
        assert_eq!(keys(&g.item), want(defs.items.iter().map(|i| i.id.as_str()).collect()));
        assert_eq!(keys(&g.building), want(defs.buildings.iter().map(|b| b.id.as_str()).collect()));
        assert_eq!(keys(&g.beast), want(defs.beasts.iter().map(|b| b.id.as_str()).collect()));
    }

    /// A canal is told apart from the river it is cut from by its own glyph or tint (#699): in
    /// Unicode its colours differ from the river's, and in ASCII both are the mapgen's `=`.
    #[test]
    fn a_canal_has_a_look_of_its_own() {
        let g = Glyphs::bundled();
        let (canal, river) =
            (g.terrain(Terrain::Canal).unwrap(), g.terrain(Terrain::River).unwrap());
        assert_ne!((canal.fg, canal.bg), (river.fg, river.bg));
        assert_eq!(canal.ascii, river.ascii);
    }

    /// Every stage of the endeavor (#242, `data/endeavor.toml`) has its own `[gate_stage.*]`
    /// look, so the gate's site is never left to the generic fallback.
    #[test]
    fn every_endeavor_stage_has_a_gate_stage_look() {
        let (g, defs) = (Glyphs::bundled(), Defs::bundled());
        for stage in &defs.endeavor.stages {
            assert!(g.gate_stage.contains_key(&stage.id), "stage {} has no look", stage.id);
        }
    }

    #[test]
    fn terrain_and_biome_entries_have_a_background_and_the_rest_do_not() {
        let g = Glyphs::bundled();
        for (id, l) in g.terrain.iter().chain(&g.biome) {
            assert!(l.bg.is_some(), "{id} needs a bg");
        }
        for (id, l) in g.resource.iter().chain(&g.item).chain(&g.building) {
            assert!(l.bg.is_none(), "{id} draws on its tile, so it has no bg");
        }
    }

    #[test]
    fn every_glyph_is_one_cell_wide() {
        for (what, glyph, _) in all_looks(&Glyphs::bundled()) {
            assert_eq!(UnicodeWidthChar::width(glyph), Some(1), "{what}: {glyph:?}");
        }
    }

    #[test]
    fn every_ascii_fallback_is_printable_ascii() {
        for (what, _, ascii) in all_looks(&Glyphs::bundled()) {
            assert!(ascii.is_ascii_graphic(), "{what}: {ascii:?}");
        }
    }

    /// `--ascii` is the mapgen renderer's glyph set for the map itself, so the ASCII frame and
    /// `cargo run -p fn_sim --bin mapgen` agree.
    #[test]
    fn the_ascii_set_matches_the_mapgen_renderer() {
        let (g, defs) = (Glyphs::bundled(), Defs::bundled());
        for b in &defs.biomes {
            assert_eq!(g.biome[&b.id].ascii, b.glyph, "biome {}", b.id);
        }
        for r in &defs.resources {
            assert_eq!(g.resource[&r.id].ascii, r.glyph, "resource {}", r.id);
        }
        let mapgen = |t| match t {
            Terrain::Water => '~',
            Terrain::Lake => '-',
            Terrain::River | Terrain::Canal => '=',
            Terrain::Ford => '+',
            Terrain::Coast => '.',
            Terrain::Mountain => '^',
            Terrain::Tunnel => '_',
            Terrain::Land => unreachable!("land draws as its biome"),
        };
        for t in TERRAINS {
            assert_eq!(g.terrain(t).unwrap().ascii, mapgen(t), "{t:?}");
        }
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let src = include_str!("../../../data/tui.toml").replace("lift = 0.4", "lyft = 0.4");
        assert!(Glyphs::from_toml(&src).is_err());
    }

    /// Every `fn_ui::code::Class` the studio's code pane highlights has its own colour in
    /// `[code]`, except `Plain`, which has none by design (it draws in the terminal's default
    /// foreground, docs/design/agent-studio.md "Highlighting").
    #[test]
    fn every_code_class_has_a_colour_except_plain() {
        use fn_ui::code::Class;
        let g = Glyphs::bundled();
        for class in [
            Class::Keyword,
            Class::Literal,
            Class::Number,
            Class::String,
            Class::Comment,
            Class::Call,
            Class::Op,
        ] {
            assert!(g.code.for_class(class).is_some(), "{class:?} should have a colour");
        }
        assert!(g.code.for_class(Class::Plain).is_none(), "Plain should have none");
    }

    /// The studio's game column follows `game_share`, and takes everything past the code
    /// column's `code_max_width` (the doc's own worked examples: 64+96 at 160, 80+120 at 200,
    /// 88+162 at 250).
    #[test]
    fn studio_game_width_follows_the_share_and_caps_the_code() {
        let g = Glyphs::bundled();
        assert_eq!(g.studio.game_width(160), 96);
        assert_eq!(g.studio.game_width(200), 120);
        assert_eq!(g.studio.game_width(250), 162);
        assert_eq!(g.studio.game_width(400), 312);
        assert!(g.studio.fits(160, 45));
        assert!(!g.studio.fits(159, 45));
        assert!(!g.studio.fits(160, 44));
    }

    /// `rail_track` has exactly one piece for every mask from 1 to 15 (#279): a lone tile
    /// (`mask == 0`) has none, since it keeps `building.rail`'s own glyph instead.
    #[test]
    fn rail_track_has_a_piece_for_every_nonzero_mask() {
        let g = Glyphs::bundled();
        assert_eq!(g.rail_track.glyph.len(), 15);
        assert_eq!(g.rail_track.ascii.len(), 15);
    }

    /// Every mask (bit 0 north, 1 east, 2 south, 3 west) picks the box-drawing piece that joins
    /// exactly those neighbours (#279): straight through a tile with one or two opposite
    /// neighbours, a corner with two adjacent ones, a T with three, a cross with all four. A
    /// lone tile (mask 0) has no piece: it keeps the rail building's own glyph.
    #[test]
    fn every_mask_picks_the_piece_that_joins_those_neighbours() {
        let g = Glyphs::bundled();
        let cases: [(u8, char, char); 16] = [
            (0, '\0', '\0'), // lone: no piece (see below)
            (RAIL_N, '│', '|'),
            (RAIL_E, '─', '-'),
            (RAIL_N | RAIL_E, '└', '+'),
            (RAIL_S, '│', '|'),
            (RAIL_N | RAIL_S, '│', '|'),
            (RAIL_E | RAIL_S, '┌', '+'),
            (RAIL_N | RAIL_E | RAIL_S, '├', '+'),
            (RAIL_W, '─', '-'),
            (RAIL_N | RAIL_W, '┘', '+'),
            (RAIL_E | RAIL_W, '─', '-'),
            (RAIL_N | RAIL_E | RAIL_W, '┴', '+'),
            (RAIL_S | RAIL_W, '┐', '+'),
            (RAIL_N | RAIL_S | RAIL_W, '┤', '+'),
            (RAIL_E | RAIL_S | RAIL_W, '┬', '+'),
            (RAIL_N | RAIL_E | RAIL_S | RAIL_W, '┼', '+'),
        ];
        assert_eq!(cases.len(), 16, "every mask from 0 to 15");
        for (mask, glyph, ascii) in cases {
            if mask == 0 {
                assert_eq!(link_glyph(&g.rail_track, mask, false), None, "mask {mask:04b}");
                assert_eq!(link_glyph(&g.rail_track, mask, true), None, "mask {mask:04b}");
            } else {
                assert_eq!(link_glyph(&g.rail_track, mask, false), Some(glyph), "mask {mask:04b}");
                assert_eq!(link_glyph(&g.rail_track, mask, true), Some(ascii), "mask {mask:04b}");
            }
        }
    }

    /// [`rail_mask`] builds the same bitmask the table above indexes by.
    #[test]
    fn rail_mask_builds_the_bit_for_each_neighbour() {
        assert_eq!(rail_mask(false, false, false, false), 0);
        assert_eq!(rail_mask(true, false, false, false), RAIL_N);
        assert_eq!(rail_mask(false, true, false, false), RAIL_E);
        assert_eq!(rail_mask(false, false, true, false), RAIL_S);
        assert_eq!(rail_mask(false, false, false, true), RAIL_W);
        assert_eq!(rail_mask(true, true, true, true), RAIL_N | RAIL_E | RAIL_S | RAIL_W);
    }
}

//! `cargo xtask art`: renders the palette-indexed text sheets in `art/*.toml` to PNG sprite sheets
//! under `assets/original/`, and keeps their `assets/manifest.toml` entries in step. The format
//! is described in `art/README.md` and `docs/design/pixel-art.md`.
//!
//! Stable API for follow-up tools and tests:
//!
//! - [`Palette::parse`] reads `art/palette.toml`; [`Sheet::parse`] reads one sheet.
//! - [`render_sheet`] turns a sheet into one [`Output`] per nation (or one, for a plain sheet).
//!   It is pure: the same inputs always give the same pixels, and [`encode_png`] always gives the
//!   same bytes.
//! - [`generate`] is `cargo xtask art`; [`check`] is the part of `cargo xtask assets` that finds
//!   stale or off-palette PNGs.
//!
//! Theme colours (`panel`, `inset`, `accent`, `accent_soft`, `accent_text`) come from
//! `fn_ui::theme`, so pixel art matches the code-drawn widgets exactly. Errors name the sheet,
//! sprite, and (1-based) row and column inside the sprite's grid.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use fn_ui::theme::{self, Rgb, Theme};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::Result;

/// The manifest `source` id of everything this tool writes.
pub const SOURCE_ID: &str = "original";
/// Directory (relative to `assets/`) the sheets are rendered into.
pub const OUT_DIR: &str = "original";

/// Names the renderer defines itself. A palette colour can't reuse them.
const RESERVED: &[&str] = &["clear", "panel", "inset", "accent", "accent_soft", "accent_text"];

/// A playable nation, as far as art cares: its id and `color` from `data/nations.toml`.
#[derive(Clone, Debug, PartialEq)]
pub struct Nation {
    pub id: String,
    pub color: [u8; 3],
}

pub fn load_nations(root: &Path) -> Result<Vec<Nation>> {
    #[derive(Deserialize)]
    struct File {
        nation: Vec<Row>,
    }
    #[derive(Deserialize)]
    struct Row {
        id: String,
        color: [u8; 3],
    }
    let path = root.join("data/nations.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("data/nations.toml: {e}"))?;
    let file: File = toml::from_str(&text).map_err(|e| format!("data/nations.toml: {e}"))?;
    Ok(file.nation.into_iter().map(|n| Nation { id: n.id, color: n.color }).collect())
}

// ---------------------------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------------------------

/// An RGBA8 image, row-major.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    fn new(width: u32, height: u32) -> Self {
        Self { width, height, rgba: vec![0; (width * height * 4) as usize] }
    }

    fn index(&self, x: u32, y: u32) -> usize {
        ((y * self.width + x) * 4) as usize
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = self.index(x, y);
        self.rgba[i..i + 4].try_into().unwrap()
    }

    fn put(&mut self, x: u32, y: u32, px: [u8; 4]) {
        let i = self.index(x, y);
        self.rgba[i..i + 4].copy_from_slice(&px);
    }
}

/// Encodes with fixed settings and no metadata, so the bytes depend only on the pixels.
pub fn encode_png(img: &Image) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, img.width, img.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&img.rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

/// Decodes an 8-bit RGB, RGBA or indexed PNG to RGBA8.
pub fn decode_png(bytes: &[u8]) -> Result<Image> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("image too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(format!("{:?}-bit PNG, expected 8-bit", info.bit_depth));
    }
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        other => return Err(format!("{other:?} PNG, expected RGB or RGBA")),
    };
    Ok(Image { width: info.width, height: info.height, rgba })
}

// ---------------------------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------------------------

/// `art/palette.toml`, validated.
pub struct Palette {
    colors: BTreeMap<String, Rgb>,
    /// name -> (colour to mix toward, amount); resolved per nation.
    accent: BTreeMap<String, (Rgb, f32)>,
    legend: BTreeMap<char, String>,
}

fn parse_hex(s: &str) -> Option<Rgb> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|v| f32::from(v) / 255.0);
    Some([c(0)?, c(2)?, c(4)?])
}

fn to8(c: Rgb) -> [u8; 4] {
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [ch(c[0]), ch(c[1]), ch(c[2]), 255]
}

fn legend_key(key: &str, what: &str) -> Result<char> {
    let mut it = key.chars();
    match (it.next(), it.next()) {
        (Some(c), None) if !c.is_whitespace() => Ok(c),
        _ => Err(format!("{what}: legend key {key:?} must be a single non-space character")),
    }
}

impl Palette {
    pub fn parse(text: &str) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawAccent {
            toward: String,
            t: f32,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            colors: BTreeMap<String, String>,
            #[serde(default)]
            accent: BTreeMap<String, RawAccent>,
            #[serde(default)]
            legend: BTreeMap<String, String>,
        }
        let raw: Raw = toml::from_str(text).map_err(|e| format!("art/palette.toml: {e}"))?;

        let mut colors = BTreeMap::new();
        for (name, hex) in &raw.colors {
            if RESERVED.contains(&name.as_str()) || raw.accent.contains_key(name) {
                return Err(format!("art/palette.toml: colour {name:?} is a reserved name"));
            }
            let rgb = parse_hex(hex).ok_or_else(|| {
                format!("art/palette.toml: colour {name:?}: {hex:?} is not a #rrggbb colour")
            })?;
            colors.insert(name.clone(), rgb);
        }
        if !colors.contains_key("ink") {
            return Err("art/palette.toml: [colors] needs `ink`, the outline colour".to_string());
        }
        let mut accent = BTreeMap::new();
        for (name, rule) in &raw.accent {
            if RESERVED.contains(&name.as_str()) {
                return Err(format!("art/palette.toml: accent {name:?} is a reserved name"));
            }
            let toward = colors.get(&rule.toward).ok_or_else(|| {
                format!("art/palette.toml: accent {name:?}: unknown colour {:?}", rule.toward)
            })?;
            if !(0.0..=1.0).contains(&rule.t) {
                return Err(format!("art/palette.toml: accent {name:?}: t must be in 0..=1"));
            }
            accent.insert(name.clone(), (*toward, rule.t));
        }
        let mut legend = BTreeMap::new();
        for (k, name) in &raw.legend {
            legend.insert(legend_key(k, "art/palette.toml")?, name.clone());
        }
        let palette = Self { colors, accent, legend };
        // Every shared legend entry must resolve, for a nation and without one.
        for (k, name) in &palette.legend {
            if !palette.knows(name) {
                return Err(format!(
                    "art/palette.toml: legend {k:?} names unknown colour {name:?}"
                ));
            }
        }
        Ok(palette)
    }

    pub fn load(root: &Path) -> Result<Self> {
        let text = fs::read_to_string(root.join("art/palette.toml"))
            .map_err(|e| format!("art/palette.toml: {e}"))?;
        Self::parse(&text)
    }

    fn knows(&self, name: &str) -> bool {
        RESERVED.contains(&name) || self.colors.contains_key(name) || self.accent.contains_key(name)
    }

    /// Whether `name` only exists once a nation's colour is known.
    fn needs_nation(&self, name: &str) -> bool {
        matches!(name, "accent" | "accent_soft" | "accent_text") || self.accent.contains_key(name)
    }

    /// Every name resolved to RGBA for one nation (or none: accent names are then absent).
    fn colours(&self, nation: Option<&Nation>) -> BTreeMap<&str, [u8; 4]> {
        let mut out: BTreeMap<&str, [u8; 4]> =
            self.colors.iter().map(|(n, c)| (n.as_str(), to8(*c))).collect();
        out.insert("clear", [0, 0, 0, 0]);
        out.insert("panel", to8(theme::PANEL));
        out.insert("inset", to8(theme::INSET));
        if let Some(nation) = nation {
            let t = Theme::for_nation(nation.color);
            out.insert("accent", to8(t.accent));
            out.insert("accent_text", to8(t.accent_text));
            out.insert("accent_soft", to8(t.accent_soft));
            for (name, (toward, amount)) in &self.accent {
                out.insert(name, to8(theme::mix(t.accent, *toward, *amount)));
            }
        }
        out
    }

    /// Every colour an `original` PNG may contain: the fixed palette plus every nation's accent
    /// family. Fully transparent pixels are always allowed and aren't in this set.
    pub fn allowed(&self, nations: &[Nation]) -> BTreeSet<[u8; 4]> {
        let mut set: BTreeSet<[u8; 4]> = self.colours(None).into_values().collect();
        for n in nations {
            set.extend(self.colours(Some(n)).into_values());
        }
        set
    }
}

// ---------------------------------------------------------------------------------------------
// Sheets
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SheetFile {
    cell: [u32; 2],
    columns: Option<u32>,
    #[serde(default)]
    outline: bool,
    #[serde(default)]
    per_nation: bool,
    #[serde(default)]
    legend: BTreeMap<String, String>,
    #[serde(default)]
    sprite: Vec<SpriteFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpriteFile {
    name: String,
    grid: String,
    #[serde(default)]
    legend: BTreeMap<String, String>,
    nation: Option<String>,
    outline: Option<bool>,
}

/// One parsed `art/<name>.toml`.
pub struct Sheet {
    pub name: String,
    pub cell: (u32, u32),
    pub columns: u32,
    pub per_nation: bool,
    outline: bool,
    legend: BTreeMap<char, String>,
    sprites: Vec<Sprite>,
}

struct Sprite {
    name: String,
    rows: Vec<Vec<char>>,
    legend: BTreeMap<char, String>,
    nation: Option<String>,
    outline: Option<bool>,
}

impl Sheet {
    /// Parses and shape-checks a sheet: every grid must have exactly `cell` rows and columns.
    pub fn parse(name: &str, text: &str) -> Result<Self> {
        let raw: SheetFile = toml::from_str(text).map_err(|e| format!("art/{name}.toml: {e}"))?;
        let [w, h] = raw.cell;
        if w == 0 || h == 0 {
            return Err(format!("art/{name}.toml: cell must be non-zero, got [{w}, {h}]"));
        }
        if raw.sprite.is_empty() {
            return Err(format!("art/{name}.toml: no [[sprite]] entries"));
        }
        let columns = raw.columns.unwrap_or(raw.sprite.len() as u32);
        if columns == 0 {
            return Err(format!("art/{name}.toml: columns must be at least 1"));
        }
        let mut legend = BTreeMap::new();
        for (k, v) in &raw.legend {
            legend.insert(legend_key(k, &format!("art/{name}.toml"))?, v.clone());
        }
        let mut sprites = Vec::new();
        let mut seen = BTreeSet::new();
        for s in raw.sprite {
            let at = format!("art/{name}.toml: sprite {:?}", s.name);
            if !seen.insert(s.name.clone()) {
                return Err(format!("{at}: duplicate sprite name"));
            }
            let mut sl = BTreeMap::new();
            for (k, v) in &s.legend {
                sl.insert(legend_key(k, &at)?, v.clone());
            }
            let rows: Vec<Vec<char>> =
                s.grid.trim_matches(['\n', '\r']).lines().map(|l| l.chars().collect()).collect();
            if rows.len() != h as usize {
                return Err(format!("{at}: {} rows, expected {h}", rows.len()));
            }
            for (y, r) in rows.iter().enumerate() {
                if r.len() != w as usize {
                    return Err(format!(
                        "{at}: row {} has {} columns, expected {w}: {}",
                        y + 1,
                        r.len(),
                        r.iter().collect::<String>()
                    ));
                }
            }
            sprites.push(Sprite {
                name: s.name,
                rows,
                legend: sl,
                nation: s.nation,
                outline: s.outline,
            });
        }
        Ok(Self {
            name: name.to_string(),
            cell: (w, h),
            columns,
            per_nation: raw.per_nation,
            outline: raw.outline,
            legend,
            sprites,
        })
    }

    #[allow(dead_code)] // Part of the API for tests and follow-up tools.
    pub fn sprite_names(&self) -> impl Iterator<Item = &str> {
        self.sprites.iter().map(|s| s.name.as_str())
    }

    /// The top-left pixel of sprite `index` in the rendered sheet.
    pub fn origin(&self, index: usize) -> (u32, u32) {
        let (i, c) = (index as u32, self.columns);
        ((i % c) * self.cell.0, (i / c) * self.cell.1)
    }
}

/// One rendered PNG of a sheet.
pub struct Output {
    /// File name under `assets/original/`: `<sheet>.png` or `<sheet>.<nation>.png`.
    pub file: String,
    #[allow(dead_code)] // Part of the API for tests and follow-up tools.
    pub nation: Option<String>,
    pub image: Image,
}

/// Renders a sheet: once per nation if `per_nation`, otherwise once. A sprite's own `nation`
/// wins over the sheet's nation.
pub fn render_sheet(palette: &Palette, sheet: &Sheet, nations: &[Nation]) -> Result<Vec<Output>> {
    let variants: Vec<Option<&Nation>> =
        if sheet.per_nation { nations.iter().map(Some).collect() } else { vec![None] };
    let find = |id: &str, at: &str| {
        nations
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| format!("{at}: unknown nation {id:?} (see data/nations.toml)"))
    };
    let (w, h) = sheet.cell;
    let rows = (sheet.sprites.len() as u32).div_ceil(sheet.columns);
    let mut outputs = Vec::new();
    for variant in variants {
        let mut img = Image::new(w * sheet.columns, h * rows);
        for (idx, sprite) in sheet.sprites.iter().enumerate() {
            let at = format!("art/{}.toml: sprite {:?}", sheet.name, sprite.name);
            let nation = match &sprite.nation {
                Some(id) => Some(find(id, &at)?),
                None => variant,
            };
            let colours = palette.colours(nation);
            let mut keys = palette.legend.clone();
            keys.extend(sheet.legend.iter().map(|(k, v)| (*k, v.clone())));
            keys.extend(sprite.legend.iter().map(|(k, v)| (*k, v.clone())));
            let (ox, oy) = sheet.origin(idx);
            for (y, row) in sprite.rows.iter().enumerate() {
                for (x, ch) in row.iter().enumerate() {
                    let here = format!("{at}: row {}, column {}", y + 1, x + 1);
                    let name = keys
                        .get(ch)
                        .ok_or_else(|| format!("{here}: unknown key {ch:?} (not in any legend)"))?;
                    let px = match colours.get(name.as_str()) {
                        Some(px) => *px,
                        None if palette.needs_nation(name) => {
                            return Err(format!(
                                "{here}: key {ch:?} is {name:?}, which needs a nation: set \
                                 `per_nation = true` on the sheet or `nation = \"...\"` on the sprite"
                            ));
                        }
                        None => {
                            return Err(format!(
                                "{here}: key {ch:?} names unknown colour {name:?}"
                            ));
                        }
                    };
                    img.put(ox + x as u32, oy + y as u32, px);
                }
            }
            if sprite.outline.unwrap_or(sheet.outline) {
                outline(&mut img, (ox, oy), (w, h), colours["ink"]);
            }
        }
        let file = match variant {
            Some(n) => format!("{}.{}.png", sheet.name, n.id),
            None => format!("{}.png", sheet.name),
        };
        outputs.push(Output { file, nation: variant.map(|n| n.id.clone()), image: img });
    }
    Ok(outputs)
}

/// `outline = true`: every clear pixel of the cell with a filled 4-neighbour becomes `ink`.
/// Never spills outside the cell, so draw fills at least 1px in from the edge.
fn outline(img: &mut Image, (ox, oy): (u32, u32), (w, h): (u32, u32), ink: [u8; 4]) {
    let filled = |img: &Image, x: i64, y: i64| {
        x >= 0 && y >= 0 && x < i64::from(w) && y < i64::from(h) && {
            img.pixel(ox + x as u32, oy + y as u32)[3] != 0
        }
    };
    let mut paint = Vec::new();
    for y in 0..i64::from(h) {
        for x in 0..i64::from(w) {
            if filled(img, x, y) {
                continue;
            }
            if [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| filled(img, x + dx, y + dy))
            {
                paint.push((x as u32, y as u32));
            }
        }
    }
    for (x, y) in paint {
        img.put(ox + x, oy + y, ink);
    }
}

// ---------------------------------------------------------------------------------------------
// Whole-repo operations
// ---------------------------------------------------------------------------------------------

/// A sheet's name and source text, from `art/*.toml` except the palette.
pub fn load_sheets(root: &Path) -> Result<Vec<(String, String)>> {
    let dir = root.join("art");
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| format!("art/: {e}"))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|e| e == "toml") {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            if stem != "palette" {
                let text =
                    fs::read_to_string(&path).map_err(|e| format!("art/{stem}.toml: {e}"))?;
                out.push((stem, text));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Everything `art/` renders to: `(generated_from, output)` for every PNG. Errors from all
/// sheets are reported together.
pub fn render_all(root: &Path) -> Result<Vec<(String, Output)>> {
    let palette = Palette::load(root)?;
    let nations = load_nations(root)?;
    let mut outputs = Vec::new();
    let mut errors = Vec::new();
    for (name, text) in load_sheets(root)? {
        match Sheet::parse(&name, &text).and_then(|s| render_sheet(&palette, &s, &nations)) {
            Ok(outs) => {
                outputs.extend(outs.into_iter().map(|o| (format!("art/{name}.toml"), o)));
            }
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() { Ok(outputs) } else { Err(errors.join("\n")) }
}

fn sha_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// One `[[file]]` the tool owns.
pub struct Entry {
    pub path: String,
    pub generated_from: String,
    pub sha256: String,
}

const SOURCE_BLOCK: &str = r#"[[source]]
id = "original"
title = "Original art"
author = "Claude (agents) for the owner"
url = "art/"
license = "Original work, owner's"
release_ok = true
retrieved = "2026-10-24"
retrieved_by = "cargo xtask art"
notes = """
Rendered by `cargo xtask art` from the palette-indexed text sheets in art/*.toml (format:
art/README.md). Every file has a generated_from entry; edit the sheet, never the PNG.
"""

"#;

/// Rewrites the manifest text: drops every `[[file]]` that has `generated_from`, appends one per
/// `entries` (sorted by path), and adds the `original` `[[source]]` if it's missing. Everything
/// else (comments, other sources and files) is kept verbatim.
pub fn update_manifest(text: &str, entries: &[Entry]) -> Result<String> {
    #[derive(Deserialize)]
    struct Probe {
        file: Vec<ProbeFile>,
    }
    #[derive(Deserialize)]
    struct ProbeFile {
        generated_from: Option<String>,
    }
    let mut head = String::new();
    let mut blocks: Vec<String> = Vec::new();
    for line in text.split_inclusive('\n') {
        if line.trim_end() == "[[file]]" {
            blocks.push(String::new());
        }
        match blocks.last_mut() {
            Some(b) => b.push_str(line),
            None => head.push_str(line),
        }
    }
    if !head.contains("id = \"original\"") {
        head = head.trim_end().to_string() + "\n\n" + SOURCE_BLOCK;
    }
    let mut out = head;
    for b in blocks {
        let probe: Probe = toml::from_str(&b).map_err(|e| format!("assets/manifest.toml: {e}"))?;
        if probe.file[0].generated_from.is_none() {
            out.push_str(b.trim_end());
            out.push_str("\n\n");
        }
    }
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    for e in sorted {
        out.push_str(&format!(
            "[[file]]\npath = \"{}\"\nsource = \"{SOURCE_ID}\"\ngenerated_from = \"{}\"\nsha256 = \"{}\"\n\n",
            e.path, e.generated_from, e.sha256
        ));
    }
    Ok(out.trim_end().to_string() + "\n")
}

/// `cargo xtask art`: renders every sheet, writes the PNGs that changed and rewrites the
/// manifest entries.
pub fn generate(root: &Path) -> Result {
    let outputs = render_all(root)?;
    let dir = root.join("assets").join(OUT_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("assets/{OUT_DIR}: {e}"))?;
    let manifest_path = root.join("assets/manifest.toml");
    let manifest =
        fs::read_to_string(&manifest_path).map_err(|e| format!("assets/manifest.toml: {e}"))?;

    let mut entries = Vec::new();
    let mut written = 0;
    for (from, out) in &outputs {
        let path = dir.join(&out.file);
        // Keep the existing bytes when the pixels already match, so a `png` upgrade that
        // re-encodes differently doesn't churn files and hashes.
        let kept = fs::read(&path)
            .ok()
            .filter(|bytes| decode_png(bytes).is_ok_and(|old| old == out.image));
        let bytes = match kept {
            Some(b) => b,
            None => {
                let b = encode_png(&out.image)?;
                fs::write(&path, &b).map_err(|e| format!("{}: {e}", path.display()))?;
                written += 1;
                b
            }
        };
        entries.push(Entry {
            path: format!("{OUT_DIR}/{}", out.file),
            generated_from: from.clone(),
            sha256: sha_hex(&bytes),
        });
    }

    // Files an earlier run made that no sheet produces any more.
    let mut removed = 0;
    #[derive(Deserialize)]
    struct Old {
        #[serde(default)]
        file: Vec<OldFile>,
    }
    #[derive(Deserialize)]
    struct OldFile {
        path: String,
        generated_from: Option<String>,
    }
    let old: Old = toml::from_str(&manifest).map_err(|e| format!("assets/manifest.toml: {e}"))?;
    for f in old.file.iter().filter(|f| f.generated_from.is_some()) {
        if !entries.iter().any(|e| e.path == f.path) {
            let _ = fs::remove_file(root.join("assets").join(&f.path));
            removed += 1;
        }
    }

    let updated = update_manifest(&manifest, &entries)?;
    if updated != manifest {
        fs::write(&manifest_path, updated).map_err(|e| format!("assets/manifest.toml: {e}"))?;
    }
    println!("art OK: {} files ({written} written, {removed} removed)", entries.len());
    Ok(())
}

/// A manifest `[[file]]`, as far as the art check needs it.
pub struct Listed<'a> {
    pub path: &'a str,
    pub source: &'a str,
    pub generated_from: Option<&'a str>,
}

/// The art part of `cargo xtask assets`. Returns error lines:
/// - a generated file whose pixels differ from a fresh render ("stale"), whose sheet is gone, or
///   a sheet output with no manifest entry;
/// - an `original` PNG with a pixel that isn't a palette colour.
pub fn check(root: &Path, listed: &[Listed]) -> Vec<String> {
    let mut errors = Vec::new();
    let renders = match render_all(root) {
        Ok(r) => r,
        Err(e) => return e.lines().map(str::to_string).collect(),
    };
    for (from, out) in &renders {
        let rel = format!("{OUT_DIR}/{}", out.file);
        match listed.iter().find(|l| l.path == rel) {
            None => errors.push(format!(
                "{rel}: rendered from {from} but not in the manifest: run cargo xtask art"
            )),
            Some(l) if l.generated_from != Some(from.as_str()) => errors.push(format!(
                "{rel}: manifest generated_from should be {from:?}: run cargo xtask art"
            )),
            Some(_) => match fs::read(root.join("assets").join(&rel)) {
                Err(_) => errors.push(format!("{rel}: missing: run cargo xtask art")),
                Ok(bytes) => match decode_png(&bytes) {
                    Ok(img) if img == out.image => {}
                    Ok(_) => errors.push(format!("{rel}: stale: run cargo xtask art")),
                    Err(e) => errors.push(format!("{rel}: {e}: run cargo xtask art")),
                },
            },
        }
    }
    for l in listed {
        if let Some(from) = l.generated_from
            && !renders.iter().any(|(f, o)| f == from && format!("{OUT_DIR}/{}", o.file) == l.path)
        {
            errors
                .push(format!("{}: nothing in {from} renders to it: run cargo xtask art", l.path));
        }
    }

    let (palette, nations) = match (Palette::load(root), load_nations(root)) {
        (Ok(p), Ok(n)) => (p, n),
        (Err(e), _) | (_, Err(e)) => {
            errors.push(e);
            return errors;
        }
    };
    let allowed = palette.allowed(&nations);
    for l in listed.iter().filter(|l| l.source == SOURCE_ID) {
        let Ok(bytes) = fs::read(root.join("assets").join(l.path)) else { continue };
        match decode_png(&bytes) {
            Err(e) => errors.push(format!("{}: {e}", l.path)),
            Ok(img) => errors.extend(off_palette(l.path, &img, &allowed)),
        }
    }
    errors
}

fn off_palette(path: &str, img: &Image, allowed: &BTreeSet<[u8; 4]>) -> Option<String> {
    let mut count = 0;
    let mut first = None;
    for y in 0..img.height {
        for x in 0..img.width {
            let px = img.pixel(x, y);
            if px[3] != 0 && !allowed.contains(&px) {
                count += 1;
                first.get_or_insert((x, y, px));
            }
        }
    }
    first.map(|(x, y, p)| {
        format!(
            "{path}: {count} pixel(s) not in the palette, first at ({x}, {y}): #{:02x}{:02x}{:02x}{:02x}",
            p[0], p[1], p[2], p[3]
        )
    })
}

#[cfg(test)]
mod tests;

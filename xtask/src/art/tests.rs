use std::path::PathBuf;

use super::*;

const PALETTE: &str = r##"
[colors]
ink = "#101010"
white = "#f0f0f0"
red0 = "#400000"
red1 = "#c00000"

[accent]
accent_dark = { toward = "ink", t = 0.5 }
accent_light = { toward = "white", t = 0.5 }

[legend]
"." = "clear"
o = "ink"
i = "inset"
p = "panel"
s = "accent_soft"
D = "accent_dark"
a = "accent"
L = "accent_light"
"##;

fn nations() -> Vec<Nation> {
    vec![
        Nation { id: "fire".into(), color: [200, 60, 40] },
        Nation { id: "water".into(), color: [70, 130, 210] },
    ]
}

fn palette() -> Palette {
    Palette::parse(PALETTE).unwrap()
}

/// A sheet of 3x2 cells with one sprite named `s`; `head` adds sheet-level keys.
fn sheet_with(head: &str, sprite_extra: &str, grid: &str) -> String {
    format!(
        "cell = [3, 2]\n{head}\n[[sprite]]\nname = \"s\"\n{sprite_extra}\ngrid = \"\"\"\n{grid}\n\"\"\"\n"
    )
}

fn render(text: &str) -> Result<Vec<Output>> {
    render_sheet(&palette(), &Sheet::parse("t", text)?, &nations())
}

fn err(text: &str) -> String {
    match render(text) {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

// --- Theme --------------------------------------------------------------------------------------

/// One pixel of each theme name, against literals and `fn_ui::theme`, so a pixel frame is exactly
/// the colour of the code-drawn widget next to it.
#[test]
fn theme_names_match_fn_ui() {
    let fire = nations()[0].clone();
    let t = Theme::for_nation(fire.color);
    let text = r#"
cell = [5, 1]
per_nation = true
legend = { t = "accent_text" }
[[sprite]]
name = "theme"
grid = "ipast"
"#;
    let outs = render(text).unwrap();
    let img = &outs[0].image;
    assert_eq!(outs[0].file, "t.fire.png");
    assert_eq!(img.pixel(0, 0), [10, 11, 15, 255], "inset");
    assert_eq!(img.pixel(1, 0), [18, 20, 26, 255], "panel");
    assert_eq!(img.pixel(1, 0), to8(theme::PANEL));
    assert_eq!(img.pixel(0, 0), to8(theme::INSET));
    assert_eq!(img.pixel(2, 0), [200, 60, 40, 255], "accent is the nation colour");
    assert_eq!(img.pixel(3, 0), to8(t.accent_soft), "accent_soft");
    assert_eq!(img.pixel(4, 0), to8(t.accent_text), "accent_text");
    // The other nation's file uses its own colour.
    assert_eq!(outs[1].image.pixel(2, 0), [70, 130, 210, 255]);
}

#[test]
fn accent_ramp_mixes_toward_palette_colours() {
    let outs = render(&sheet_with("per_nation = true", "", "DaL\nooo")).unwrap();
    let img = &outs[0].image;
    // fire [200, 60, 40] halfway to ink #101010 (16) and to white #f0f0f0 (240).
    assert_eq!(img.pixel(0, 0), [108, 38, 28, 255]);
    assert_eq!(img.pixel(1, 0), [200, 60, 40, 255]);
    assert_eq!(img.pixel(2, 0), [220, 150, 140, 255]);
}

// --- Layout -------------------------------------------------------------------------------------

#[test]
fn plain_sheet_is_one_png_and_per_nation_is_one_each() {
    let plain = render(&sheet_with("", "", "ooo\nooo")).unwrap();
    assert_eq!(plain.len(), 1);
    assert_eq!(plain[0].file, "t.png");
    assert_eq!(plain[0].nation, None);

    let per = render(&sheet_with("per_nation = true", "", "ooo\nooo")).unwrap();
    let files: Vec<&str> = per.iter().map(|o| o.file.as_str()).collect();
    assert_eq!(files, ["t.fire.png", "t.water.png"]);
}

#[test]
fn sprites_pack_in_order_by_columns() {
    let mut text =
        String::from("cell = [2, 1]\ncolumns = 2\nlegend = { r = \"red1\", b = \"red0\" }\n");
    for (name, row) in [("a", "rr"), ("b", "bb"), ("c", "rb")] {
        text.push_str(&format!("[[sprite]]\nname = \"{name}\"\ngrid = \"{row}\"\n"));
    }
    let sheet = Sheet::parse("t", &text).unwrap();
    assert_eq!(sheet.origin(2), (0, 1));
    assert_eq!(sheet.sprite_names().collect::<Vec<_>>(), ["a", "b", "c"]);
    let img = &render_sheet(&palette(), &sheet, &nations()).unwrap()[0].image;
    assert_eq!((img.width, img.height), (4, 2));
    assert_eq!(img.pixel(2, 0), to8(palette().colors["red0"]));
    assert_eq!(img.pixel(0, 1), to8(palette().colors["red1"]));
    assert_eq!(img.pixel(1, 1), to8(palette().colors["red0"]));
}

#[test]
fn sprite_legend_overrides_sheet_legend_overrides_shared() {
    let text = sheet_with(
        "legend = { x = \"red0\", o = \"red0\" }",
        "legend = { x = \"red1\" }",
        "xo.\n...",
    );
    let img = &render(&text).unwrap()[0].image;
    assert_eq!(img.pixel(0, 0), to8(palette().colors["red1"]), "sprite beats sheet");
    assert_eq!(img.pixel(1, 0), to8(palette().colors["red0"]), "sheet beats shared");
    assert_eq!(img.pixel(2, 0), [0, 0, 0, 0], "shared still applies");
}

#[test]
fn sprite_nation_pins_the_accent() {
    let text = "cell = [1, 1]\nper_nation = true\n[[sprite]]\nname = \"s\"\nnation = \"water\"\ngrid = \"a\"\n";
    let outs = render(text).unwrap();
    for o in outs {
        assert_eq!(o.image.pixel(0, 0), [70, 130, 210, 255], "{}", o.file);
    }
}

#[test]
fn outline_is_four_neighbour_ink_inside_the_cell() {
    let text = "cell = [5, 3]\noutline = true\nlegend = { r = \"red1\" }\n[[sprite]]\nname = \"s\"\ngrid = \"\"\"\n.....\n..r..\n.....\n\"\"\"\n";
    let img = &render(text).unwrap()[0].image;
    let ink = [16, 16, 16, 255];
    for (x, y) in [(2, 0), (1, 1), (3, 1), (2, 2)] {
        assert_eq!(img.pixel(x, y), ink, "({x}, {y})");
    }
    assert_eq!(img.pixel(1, 0), [0, 0, 0, 0], "diagonals stay clear");
    assert_eq!(img.pixel(2, 1), to8(palette().colors["red1"]));
}

#[test]
fn sprite_can_turn_the_outline_off() {
    let text = "cell = [3, 3]\noutline = true\nlegend = { r = \"red1\" }\n[[sprite]]\nname = \"s\"\noutline = false\ngrid = \"\"\"\n...\n.r.\n...\n\"\"\"\n";
    let img = &render(text).unwrap()[0].image;
    assert_eq!(img.pixel(1, 0), [0, 0, 0, 0]);
}

// --- Errors name the sheet, sprite, row and column ------------------------------------------------

#[test]
fn wrong_row_length_is_named() {
    let e = Sheet::parse("t", &sheet_with("", "", "ooo\noo")).err().unwrap();
    assert!(e.contains("art/t.toml") && e.contains("sprite \"s\""), "{e}");
    assert!(e.contains("row 2 has 2 columns, expected 3"), "{e}");
}

#[test]
fn wrong_row_count_is_named() {
    let e = Sheet::parse("t", &sheet_with("", "", "ooo\nooo\nooo")).err().unwrap();
    assert!(e.contains("art/t.toml") && e.contains("sprite \"s\""), "{e}");
    assert!(e.contains("3 rows, expected 2"), "{e}");
}

#[test]
fn unknown_key_is_named() {
    let e = err(&sheet_with("", "", "oo.\no?o"));
    assert!(e.contains("art/t.toml") && e.contains("sprite \"s\""), "{e}");
    assert!(e.contains("row 2, column 2") && e.contains("unknown key '?'"), "{e}");
}

#[test]
fn accent_key_without_a_nation_is_named() {
    for key in ["a", "s", "D", "L"] {
        let e = err(&sheet_with("", "", &format!("oo.\noo{key}")));
        assert!(e.contains("art/t.toml") && e.contains("sprite \"s\""), "{e}");
        assert!(e.contains("row 2, column 3") && e.contains("needs a nation"), "{e}");
    }
    // The same grid is fine with a nation.
    assert!(render(&sheet_with("per_nation = true", "", "oo.\nooa")).is_ok());
    assert!(render(&sheet_with("", "nation = \"fire\"", "oo.\nooa")).is_ok());
}

#[test]
fn unknown_colour_and_nation_are_named() {
    let e = err(&sheet_with("legend = { x = \"nope\" }", "", "xoo\nooo"));
    assert!(e.contains("row 1, column 1") && e.contains("unknown colour \"nope\""), "{e}");
    let e = err(&sheet_with("", "nation = \"metal\"", "ooo\nooo"));
    assert!(e.contains("sprite \"s\"") && e.contains("unknown nation \"metal\""), "{e}");
}

#[test]
fn bad_sheets_and_palettes_are_rejected() {
    for (text, want) in [
        ("cell = [0, 2]\n[[sprite]]\nname = \"s\"\ngrid = \"\"", "non-zero"),
        ("cell = [1, 1]", "no [[sprite]]"),
        ("cell = [1, 1]\ncolums = 2\n[[sprite]]\nname = \"s\"\ngrid = \"o\"", "unknown field"),
        (
            "cell = [1, 1]\n[[sprite]]\nname = \"s\"\ngrid = \"o\"\n[[sprite]]\nname = \"s\"\ngrid = \"o\"",
            "duplicate",
        ),
        (
            "cell = [1, 1]\nlegend = { xy = \"ink\" }\n[[sprite]]\nname = \"s\"\ngrid = \"o\"",
            "single",
        ),
    ] {
        let e = Sheet::parse("t", text).err().unwrap_or_else(|| panic!("{text} parsed"));
        assert!(e.contains("art/t.toml") && e.contains(want), "{want}: {e}");
    }
    for (text, want) in [
        ("[colors]\nink = \"#101010\"\nclear = \"#000000\"", "reserved"),
        ("[colors]\nink = \"101010\"", "not a #rrggbb"),
        ("[colors]\nwhite = \"#ffffff\"", "needs `ink`"),
        (
            "[colors]\nink = \"#101010\"\n[accent]\nz = { toward = \"nope\", t = 0.5 }",
            "unknown colour",
        ),
        ("[colors]\nink = \"#101010\"\n[legend]\n\"o\" = \"nope\"", "unknown colour"),
    ] {
        let e = Palette::parse(text).err().unwrap_or_else(|| panic!("{text} parsed"));
        assert!(e.contains("art/palette.toml") && e.contains(want), "{want}: {e}");
    }
}

// --- Determinism and PNG ------------------------------------------------------------------------

#[test]
fn rendering_twice_is_byte_identical() {
    let render_bytes = || -> Vec<Vec<u8>> {
        render_all(&crate::root())
            .unwrap()
            .iter()
            .map(|(_, o)| encode_png(&o.image).unwrap())
            .collect()
    };
    let (a, b) = (render_bytes(), render_bytes());
    assert!(!a.is_empty());
    assert_eq!(a, b);
}

#[test]
fn png_round_trips() {
    let outs = render(&sheet_with("per_nation = true", "", "DaL\nsip")).unwrap();
    for o in outs {
        let back = decode_png(&encode_png(&o.image).unwrap()).unwrap();
        assert_eq!(back, o.image);
    }
}

/// The real sources all render, and use only colours the palette allows.
#[test]
fn committed_sheets_render_inside_the_palette() {
    let root = crate::root();
    let allowed = Palette::load(&root).unwrap().allowed(&load_nations(&root).unwrap());
    let outputs = render_all(&root).unwrap();
    assert!(outputs.len() >= 7, "ported sheets are missing");
    for (_, o) in outputs {
        assert_eq!(off_palette(&o.file, &o.image, &allowed), None);
    }
}

// --- Manifest -----------------------------------------------------------------------------------

fn entry(path: &str, sha: &str) -> Entry {
    Entry { path: path.into(), generated_from: "art/a.toml".into(), sha256: sha.into() }
}

#[test]
fn manifest_update_keeps_other_entries_and_is_idempotent() {
    let before = "# header\n\n[[source]]\nid = \"x\"\n\n[[file]]\npath = \"third_party/a.png\"\nsource = \"x\"\nsha256 = \"aa\"\n\n[[file]]\npath = \"original/old.png\"\nsource = \"original\"\ngenerated_from = \"art/old.toml\"\nsha256 = \"bb\"\n";
    let entries = [entry("original/b.png", "22"), entry("original/a.png", "11")];
    let once = update_manifest(before, &entries).unwrap();
    assert!(once.starts_with("# header\n"));
    assert!(once.contains("path = \"third_party/a.png\""));
    assert!(!once.contains("old.png"), "stale generated entry removed");
    assert!(once.contains("id = \"original\""), "source added");
    assert!(once.find("original/a.png").unwrap() < once.find("original/b.png").unwrap());
    assert!(once.ends_with("sha256 = \"22\"\n"));
    assert_eq!(update_manifest(&once, &entries).unwrap(), once);
    // Parses as a manifest.
    toml::from_str::<toml::Table>(&once).unwrap();
}

// --- On disk: generate, then check --------------------------------------------------------------

struct Fake(PathBuf);

impl Fake {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("xtask-art-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for d in ["art", "data", "assets"] {
            fs::create_dir_all(dir.join(d)).unwrap();
        }
        fs::write(dir.join("art/palette.toml"), PALETTE.to_string() + "").unwrap();
        fs::write(
            dir.join("data/nations.toml"),
            "[[nation]]\nid = \"fire\"\ncolor = [200, 60, 40]\n[[nation]]\nid = \"water\"\ncolor = [70, 130, 210]\n",
        )
        .unwrap();
        fs::write(dir.join("assets/manifest.toml"), "# manifest\n").unwrap();
        fs::write(dir.join("art/x.toml"), sheet_with("per_nation = true", "", "oLa\nsDo")).unwrap();
        Self(dir)
    }

    fn check(&self) -> Vec<String> {
        let text = fs::read_to_string(self.0.join("assets/manifest.toml")).unwrap();
        #[derive(Deserialize)]
        struct M {
            #[serde(default)]
            file: Vec<F>,
        }
        #[derive(Deserialize)]
        struct F {
            path: String,
            source: String,
            generated_from: Option<String>,
        }
        let m: M = toml::from_str(&text).unwrap();
        let listed: Vec<Listed> = m
            .file
            .iter()
            .map(|f| Listed {
                path: &f.path,
                source: &f.source,
                generated_from: f.generated_from.as_deref(),
            })
            .collect();
        check(&self.0, &listed)
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn generate_then_check_passes_and_is_stable() {
    let fake = Fake::new("ok");
    generate(&fake.0).unwrap();
    assert!(fake.0.join("assets/original/x.fire.png").is_file());
    assert!(fake.0.join("assets/original/x.water.png").is_file());
    assert_eq!(fake.check(), Vec::<String>::new());

    let manifest = fs::read_to_string(fake.0.join("assets/manifest.toml")).unwrap();
    assert!(manifest.contains("generated_from = \"art/x.toml\""));
    generate(&fake.0).unwrap();
    assert_eq!(fs::read_to_string(fake.0.join("assets/manifest.toml")).unwrap(), manifest);
}

#[test]
fn edited_source_makes_the_png_stale() {
    let fake = Fake::new("stale");
    generate(&fake.0).unwrap();
    fs::write(fake.0.join("art/x.toml"), sheet_with("per_nation = true", "", "oLa\nsDL")).unwrap();
    let errors = fake.check();
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(errors[0].contains("x.fire.png: stale: run cargo xtask art"), "{errors:?}");
    generate(&fake.0).unwrap();
    assert_eq!(fake.check(), Vec::<String>::new());
}

#[test]
fn new_sheet_without_a_png_is_reported_and_removed_sheet_cleans_up() {
    let fake = Fake::new("new");
    generate(&fake.0).unwrap();
    fs::write(fake.0.join("art/y.toml"), sheet_with("", "", "ooo\nooo")).unwrap();
    let errors = fake.check();
    assert!(errors.iter().any(|e| e.contains("original/y.png") && e.contains("xtask art")));

    generate(&fake.0).unwrap();
    fs::remove_file(fake.0.join("art/x.toml")).unwrap();
    generate(&fake.0).unwrap();
    assert!(!fake.0.join("assets/original/x.fire.png").exists());
    assert!(fake.0.join("assets/original/y.png").is_file());
    assert_eq!(fake.check(), Vec::<String>::new());
}

#[test]
fn off_palette_pixels_in_an_original_png_fail() {
    let fake = Fake::new("palette");
    generate(&fake.0).unwrap();
    let mut img = Image::new(2, 1);
    img.put(0, 0, [1, 2, 3, 255]);
    let bytes = encode_png(&img).unwrap();
    fs::write(fake.0.join("assets/original/hand.png"), &bytes).unwrap();
    let mut manifest = fs::read_to_string(fake.0.join("assets/manifest.toml")).unwrap();
    manifest.push_str(
        "\n[[file]]\npath = \"original/hand.png\"\nsource = \"original\"\nsha256 = \"00\"\n",
    );
    fs::write(fake.0.join("assets/manifest.toml"), manifest).unwrap();
    let errors = fake.check();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].contains("hand.png") && errors[0].contains("(0, 0): #010203ff"),
        "{errors:?}"
    );
}

#[test]
fn source_errors_surface_through_check() {
    let fake = Fake::new("err");
    fs::write(fake.0.join("art/bad.toml"), sheet_with("", "", "oo\nooo")).unwrap();
    let errors = fake.check();
    assert!(errors[0].contains("art/bad.toml") && errors[0].contains("row 1"), "{errors:?}");
    assert!(generate(&fake.0).is_err());
}

# art/: pixel art as text

Original art is written as palette-indexed character grids and rendered by `cargo xtask art`
(code: `xtask/src/art.rs`). One `art/<sheet>.toml` makes one PNG in `assets/original/`. Edit the
TOML, never the PNG; `cargo xtask assets` (part of `cargo xtask ci`) fails if a PNG is stale or has
a colour outside the palette. The design and style rules are in `docs/design/pixel-art.md`.

```
cargo xtask art      # render every sheet, update assets/manifest.toml
cargo xtask assets   # check: "stale: run cargo xtask art"
```

## Palette: `art/palette.toml`

- `[colors]`: named `#rrggbb` colours. `ink` is required (it is the outline colour).
- `[accent]`: ramp colours mixed from a nation's `color` in `data/nations.toml`:
  `name = { toward = "<colour>", t = 0.4 }` means `mix(nation_color, colour, t)`.
- `[legend]`: single-character keys every sheet can use.
- Built in, not listed in the file: `clear` (transparent), and from `fn_ui::theme` `panel`,
  `inset`, `accent` (the nation colour), `accent_soft`, `accent_text`. Pixel frames therefore match
  the code-drawn widgets exactly. Accent names need a nation (see `per_nation`, `nation`).

## Sheet: `art/<sheet>.toml`

```toml
cell = [16, 16]      # required: every sprite is one cell
columns = 8          # optional: sprites per row of the sheet (default: all in one row)
outline = true       # optional: 1px `ink` around every filled pixel (4-neighbour, inside the cell)
per_nation = false   # true: one PNG per nation, `<sheet>.<nation>.png`, accent keys per nation

[legend]             # optional: adds to or overrides palette.toml's [legend]
g = "gold2"

[[sprite]]
name = "stone"       # unique in the sheet
nation = "fire"      # optional: pin the accent to one nation (wins over per_nation)
outline = false      # optional: overrides the sheet's `outline`
legend = { "1" = "stone0", "2" = "stone1" }   # optional: overrides the sheet's and palette's
grid = """
................
...
"""
```

- The grid has exactly `cell` height rows of `cell` width characters, one character per pixel.
  Each character is a key looked up in the sprite legend, then the sheet legend, then the shared
  one; its value is a colour name.
- Sprite `n` (0-based, file order) sits at cell column `n % columns`, row `n / columns`. These
  positions are the stable contract with the client: name cells by `(column, row)` and test the
  constant against the sprite name in the TOML.
- Outlines never leave the cell: keep filled pixels at least 1px from the edge.
- Errors name the sheet, sprite and the 1-based row and column in the grid, e.g.
  `art/icons.toml: sprite "wood": row 3, column 5: unknown key 'x' (not in any legend)`.

## Manifest

`cargo xtask art` owns every `[[file]]` in `assets/manifest.toml` that has a `generated_from`
line: it rewrites them (sorted, at the end of the file), removes the PNG of a sheet that is gone,
and adds the `original` `[[source]]` if it is missing. Other entries are left untouched. An
unchanged PNG keeps its bytes, so a `png` crate upgrade doesn't churn hashes. The CI check compares
decoded pixels, not bytes.

## Facesets: `art/faces_<nation>.toml`

Portraits (docs/design/pixel-art.md, "Units and portraits") are one sheet per nation:
`art/faces_<nation>.toml` (`cell = [32, 32]`, `columns = 3`, `outline = true`) renders to
`assets/original/faces_<nation>.png`, 96x32. Each sprite is named by the **character id** of the
unit it shows (`earth_farmer`, ...) and sprite `n` is the cell at column `n`, in the order of that
nation's `characters` in `data/nations.toml`. A face is head and shoulders, facing three-quarters
to the right, with the clothes and palette ramps of the unit sheet of the same name.

`sprites/characters.rs` points a character's `face` at the sheet path. The cell is the character's
position among the table rows with the same `face` path, and a test checks the sprite names match.
The HUD takes the portrait's size from the image's height, so a 32px face is drawn at 64px and a
38px pack face at 76px.

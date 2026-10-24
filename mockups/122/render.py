#!/usr/bin/env python3
"""Prototype of `cargo xtask art` (docs/design/pixel-art.md), for the #122 mockups only.

Reads palette-indexed text sheets (src/*.toml) and renders each to a PNG sprite sheet. The real
tool is Rust in xtask; this script fixes the format so the samples are made the way shipped art
will be. Run from the repo root:

    python3 mockups/122/render.py OUT_DIR

Sheets with `per_nation = true`, or sprites that use accent keys, take each nation's `color` from
data/nations.toml. Errors (wrong grid size, unknown key) name the sheet, sprite, row and column.
"""

import sys
import tomllib
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
SRC = HERE / "src"

# fn_ui::theme constants, in [0, 1] sRGB.
PANEL = (0.07, 0.08, 0.10)
INSET = (0.04, 0.045, 0.06)
WHITE = (1.0, 1.0, 1.0)


def mix(a, b, t):
    return tuple(x + (y - x) * t for x, y in zip(a, b))


def hex_rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i : i + 2], 16) / 255 for i in (0, 2, 4))


def to8(c):
    return tuple(round(max(0.0, min(1.0, v)) * 255) for v in c) + (255,)


def nations():
    data = tomllib.loads((ROOT / "data/nations.toml").read_text())
    return {n["id"]: tuple(c / 255 for c in n["color"]) for n in data["nation"]}


def palette():
    return tomllib.loads((SRC / "palette.toml").read_text())


def colours_for(pal, accent):
    """Every palette name resolved to RGBA, for one nation's accent (or None)."""
    cols = {name: to8(hex_rgb(h)) for name, h in pal["colors"].items()}
    cols["clear"] = (0, 0, 0, 0)
    cols["panel"] = to8(PANEL)
    cols["inset"] = to8(INSET)
    if accent is not None:
        base = {n: hex_rgb(h) for n, h in pal["colors"].items()}
        cols["accent"] = to8(accent)
        cols["accent_text"] = to8(mix(accent, WHITE, 0.25))
        cols["accent_soft"] = to8(mix(accent, PANEL, 0.55))
        for name, rule in pal["accent"].items():
            cols[name] = to8(mix(accent, base[rule["toward"]], rule["t"]))
    return cols


def rows(sheet_name, sprite, w, h):
    grid = sprite["grid"].strip("\n").split("\n")
    where = f"{sheet_name}:{sprite['name']}"
    if len(grid) != h:
        sys.exit(f"{where}: {len(grid)} rows, expected {h}")
    for y, r in enumerate(grid):
        if len(r) != w:
            sys.exit(f"{where}: row {y} has {len(r)} columns, expected {w}: {r!r}")
    return grid


def add_outline(img, ox, oy, w, h, ink):
    """`outline = true`: every clear pixel with a filled 4-neighbour becomes `ink`."""
    filled = {(x, y) for y in range(h) for x in range(w) if img.getpixel((ox + x, oy + y))[3]}
    for y in range(h):
        for x in range(w):
            if (x, y) in filled:
                continue
            if any((x + dx, y + dy) in filled for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
                img.putpixel((ox + x, oy + y), ink)


def render_sheet(path, pal, nats):
    sheet = tomllib.loads(path.read_text())
    w, h = sheet["cell"]
    cols = sheet.get("columns", len(sheet["sprite"]))
    n = len(sheet["sprite"])
    legend = {**pal["legend"], **sheet.get("legend", {})}
    variants = list(nats) if sheet.get("per_nation") else [None]
    out = {}
    for nation in variants:
        img = Image.new("RGBA", (w * cols, h * ((n + cols - 1) // cols)))
        for idx, sprite in enumerate(sheet["sprite"]):
            key_nation = sprite.get("nation", nation)
            accent = nats[key_nation] if key_nation else None
            colours = colours_for(pal, accent)
            keys = {**legend, **sprite.get("legend", {})}
            ox, oy = (idx % cols) * w, (idx // cols) * h
            for y, r in enumerate(rows(path.stem, sprite, w, h)):
                for x, ch in enumerate(r):
                    if ch not in keys:
                        sys.exit(f"{path.stem}:{sprite['name']}: unknown key {ch!r} at ({x},{y})")
                    name = keys[ch]
                    if name not in colours:
                        sys.exit(f"{path.stem}:{sprite['name']}: {name!r} needs a nation accent")
                    img.putpixel((ox + x, oy + y), colours[name])
            if sprite.get("outline", sheet.get("outline", False)):
                add_outline(img, ox, oy, w, h, colours["ink"])
        out[nation] = (img, sheet)
    return out


def render_all():
    pal, nats = palette(), nations()
    return {p.stem: render_sheet(p, pal, nats) for p in sorted(SRC.glob("*.toml")) if p.stem != "palette"}


def sprite(sheets, sheet, name, nation=None):
    """One sprite cut from a rendered sheet, by name."""
    img, meta = sheets[sheet][nation]
    w, h = meta["cell"]
    cols = meta.get("columns", len(meta["sprite"]))
    idx = [s["name"] for s in meta["sprite"]].index(name)
    x, y = (idx % cols) * w, (idx // cols) * h
    return img.crop((x, y, x + w, y + h))


if __name__ == "__main__":
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for stem, variants in render_all().items():
        for nation, (img, _) in variants.items():
            name = f"{stem}.{nation}.png" if nation else f"{stem}.png"
            img.save(out / name)
            print(out / name)

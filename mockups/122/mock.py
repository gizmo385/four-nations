#!/usr/bin/env python3
"""Builds the #122 mockup images from the text sheets in src/ (rendered by render.py).

    git show 92ea070:mockups/1/ninja-seal-ui.png > target/seal-ui.png
    python3 mockups/122/mock.py target/seal-ui.png mockups/122

The first argument is the #39 Seal UI reference render (Ninja Adventure map, code-drawn Seal
panels), used for the side-by-side comparison. Ninja Adventure sprites come from the in-repo
subset under assets/third_party/ninja-adventure/.
"""

import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parent))
import render  # noqa: E402

ROOT = render.ROOT
NA = ROOT / "assets/third_party/ninja-adventure/Backgrounds/Tilesets"
FONT = next(Path("/root/.cargo/registry/src").glob("*/bevy_text-0.20.0/src/FiraMono-subset.ttf"), None)
BG = (110, 118, 112, 255)  # neutral grey-green, neither panel nor map
PANEL = render.to8(render.PANEL)
INSET = render.to8(render.INSET)
TEXT = render.to8((0.93, 0.94, 0.96))
DIM = render.to8((0.58, 0.61, 0.67))


def font(size):
    return ImageFont.truetype(str(FONT), size) if FONT else ImageFont.load_default()


def up(img, k):
    return img.resize((img.width * k, img.height * k), Image.NEAREST)


def nine_slice(src, border, w, h):
    """Stretch a 9-slice source to w x h (art pixels), like Bevy's TextureSlicer (stretch)."""
    sw, sh = src.size
    out = Image.new("RGBA", (w, h))
    xs = [(0, border, 0, border), (border, sw - border, border, w - border), (sw - border, sw, w - border, w)]
    ys = [(0, border, 0, border), (border, sh - border, border, h - border), (sh - border, sh, h - border, h)]
    for sx0, sx1, dx0, dx1 in xs:
        for sy0, sy1, dy0, dy1 in ys:
            piece = src.crop((sx0, sy0, sx1, sy1)).resize((dx1 - dx0, dy1 - dy0), Image.NEAREST)
            out.paste(piece, (dx0, dy0))
    return out


def label(draw, xy, s, size=12, fill=(235, 238, 242, 255)):
    draw.text(xy, s, font=font(size), fill=fill)


def theme(nation):
    accent = render.nations()[nation]
    return {
        "accent_text": render.to8(render.mix(accent, render.WHITE, 0.25)),
        "accent_soft": render.to8(render.mix(accent, render.PANEL, 0.55)),
    }


def side_panel(sheets, nation, title, w=272, h=360):
    """The side panel header and first sections, built from Claude art at 2x (screen pixels)."""
    t = theme(nation)
    frame = sheets["ui_frame"][nation][0]
    body = up(nine_slice(frame, 14, w // 2 + 8, h // 2 + 8), 2)  # +4 art px margin each side
    out = Image.new("RGBA", (body.width, body.height + 8), (0, 0, 0, 0))
    out.alpha_composite(body, (0, 8))
    seal = up(render.sprite(sheets, "ui_flair", "seal", nation), 2)
    out.alpha_composite(seal, ((out.width - seal.width) // 2, 8 + 8 - 16))
    d = ImageDraw.Draw(out)
    x0, y = 8 + 14, 8 + 8 + 14
    out.alpha_composite(up(render.sprite(sheets, "emblems", nation), 2), (x0, y))
    label(d, (x0 + 42, y - 2), title, 18, TEXT)
    label(d, (x0 + 42, y + 20), "Settlement of 6", 12, DIM)
    y += 46
    label(d, (x0, y), "Spring 12, Y1", 14, TEXT)
    y += 30

    def section(name, y):
        out.alpha_composite(up(render.sprite(sheets, "ui_flair", "bullet", nation), 2), (x0 - 10, y - 10))
        label(d, (x0 + 12, y), name, 12, t["accent_text"])
        tw = d.textlength(name, font=font(12))
        d.line((x0 + 18 + tw, y + 7, out.width - 22, y + 7), fill=t["accent_soft"], width=2)
        return y + 24

    y = section("STORES", y)
    stores = [("food", "Food", 42), ("water", "Water", 30), ("wood", "Wood", 18), ("stone", "Stone", 25), ("gold", "Gold", 0)]
    colw = (out.width - 2 * x0) // 2
    for i, (icon, name, n) in enumerate(stores):
        cx, cy = x0 + (i % 2) * colw, y + (i // 2) * 38
        well = Image.new("RGBA", (34, 34), INSET)
        out.alpha_composite(well, (cx, cy))
        out.alpha_composite(up(render.sprite(sheets, "icons", icon), 2), (cx + 1, cy + 1))
        label(d, (cx + 42, cy + 1), name, 12, DIM)
        label(d, (cx + 42, cy + 15), str(n), 14, TEXT)
    y += 3 * 38 + 6
    y = section("TOOLS", y)
    tools = [("harvest", "H"), ("cancel", "X"), ("stockpile", "P"), ("build", "U")]
    for i, (tool, key) in enumerate(tools):
        cx = x0 + i * 56
        active = i == 0
        acc = render.to8(render.nations()[nation])
        btn = Image.new("RGBA", (44, 44), acc if active else INSET)
        bd = ImageDraw.Draw(btn)
        bd.rectangle((0, 0, 43, 43), outline=acc if active else t["accent_soft"])
        out.alpha_composite(btn, (cx, y))
        out.alpha_composite(up(render.sprite(sheets, "tools", tool), 2), (cx + 6, y + 4))
        label(d, (cx + 34, y + 30), key, 10, (12, 12, 18, 255) if active else DIM)
    return out


def caption(img, text, pad=28):
    out = Image.new("RGBA", (img.width, img.height + pad), BG)
    out.alpha_composite(img, (0, pad))
    label(ImageDraw.Draw(out), (6, 6), text, 14, (250, 250, 250, 255))
    return out


def hstack(imgs, gap=16, bg=BG):
    w = sum(i.width for i in imgs) + gap * (len(imgs) + 1)
    h = max(i.height for i in imgs) + 2 * gap
    out = Image.new("RGBA", (w, h), bg)
    x = gap
    for i in imgs:
        out.alpha_composite(i, (x, gap))
        x += i.width + gap
    return out


def vstack(imgs, gap=16, bg=BG):
    w = max(i.width for i in imgs) + 2 * gap
    h = sum(i.height for i in imgs) + gap * (len(imgs) + 1)
    out = Image.new("RGBA", (w, h), bg)
    y = gap
    for i in imgs:
        out.alpha_composite(i, (gap, y))
        y += i.height + gap
    return out


def scale_row(sprites, names, k, ground=None):
    """Sprites at 1x and at k x, each over `ground` (a 16px tile) when given."""
    tiles = []
    for s in sprites:
        base = ground.copy() if ground else Image.new("RGBA", s.size, (0, 0, 0, 0))
        base.alpha_composite(s, ((base.width - s.width) // 2, base.height - s.height))
        tiles.append(base)
    one = hstack(tiles, gap=8)
    big = hstack([up(t, k) for t in tiles], gap=8)
    out = vstack([one, big, Image.new("RGBA", (1, 14), BG)], gap=8)
    d = ImageDraw.Draw(out)
    x = 16
    for n, t in zip(names, tiles):
        label(d, (x, out.height - 28), n, 12)
        x += t.width * k + 8
    return out


def samples(sheets, out_dir):
    """One image per element group: 1x (top row) and 4x (bottom row)."""
    groups = {
        "sample-frame": (
            [render.sprite(sheets, "ui_frame", "panel", n) for n in ("air", "water", "earth", "fire")]
            + [render.sprite(sheets, "ui_flair", "seal", n) for n in ("air", "water", "earth", "fire")],
            ["frame air", "water", "earth", "fire", "seal", "", "", ""],
        ),
        "sample-emblems": (
            [render.sprite(sheets, "emblems", n) for n in ("air", "water", "earth", "fire")],
            ["air", "water", "earth", "fire"],
        ),
        "sample-icons": (
            [render.sprite(sheets, "icons", n) for n in ("food", "water", "wood", "stone", "gold")],
            ["food", "water", "wood", "stone", "gold"],
        ),
        "sample-tools": (
            [render.sprite(sheets, "tools", n) for n in ("harvest", "cancel", "stockpile", "build")],
            ["harvest", "cancel", "stockpile", "build"],
        ),
    }
    for name, (sprites, names) in groups.items():
        img = caption(scale_row(sprites, names, 4), f"#122 {name[7:]}: 1x (top), 4x (bottom)")
        img.convert("RGB").save(out_dir / f"{name}.png")


def compare_ui(sheets, seal_ui, out_dir):
    """The shipped code-drawn Seal side panel next to the Claude-art one, both Water Tribes."""
    current = Image.open(seal_ui).convert("RGBA").crop((978, 4, 1270, 372))
    mine = side_panel(sheets, "water", "Water Tribes", w=272, h=352)
    cur = caption(current, "Today: code-drawn Seal")
    new = caption(mine, "Claude-authored art")
    hstack([cur, new]).convert("RGB").save(out_dir / "compare-ui.png")
    panels = [caption(side_panel(sheets, n, t), n) for n, t in
              (("air", "Air Nomads"), ("earth", "Earth Kingdom"), ("fire", "Fire Nation"))]
    hstack(panels).convert("RGB").save(out_dir / "panel-nations.png")


def compare_map(sheets, out_dir):
    """Ninja Adventure's resource sprites next to the Claude-authored ones, on pack grass."""
    floor = Image.open(NA / "TilesetFloor.png").convert("RGBA")
    nature = Image.open(NA / "TilesetNature.png").convert("RGBA")
    cell = lambda im, c, r: im.crop((c * 16, r * 16, c * 16 + 16, r * 16 + 16))  # noqa: E731
    grass = cell(floor, 0, 12)
    na = [cell(nature, 8, 11), cell(nature, 6, 14), cell(nature, 4, 14)]
    ours = [render.sprite(sheets, "map", "cabbage"), render.sprite(sheets, "icons", "stone"),
            render.sprite(sheets, "map", "gold_vein")]
    names = ["cabbage", "stone", "gold vein"]
    left = caption(scale_row(na, names, 6, grass), "Ninja Adventure (in repo)")
    right = caption(scale_row(ours, names, 6, grass), "Claude-authored")
    # A small field at 3x: pack grass with both sets mixed in, to judge the fit.
    field = Image.new("RGBA", (16 * 8, 16 * 3))
    for y in range(3):
        for x in range(8):
            field.alpha_composite(grass, (x * 16, y * 16))
    for i, s in enumerate(na):
        field.alpha_composite(s, (16 * (2 * i), 0))
    for i, s in enumerate(ours):
        field.alpha_composite(s, (16 * (2 * i + 1), 32))
    mix = caption(up(field, 4), "Mixed on one field, 4x: pack top row, ours bottom row")
    vstack([hstack([left, right]), mix]).convert("RGB").save(out_dir / "compare-map.png")


def tile_hash(x, y, salt):
    """crates/client/src/sprites.rs `tile_hash`, in u32 arithmetic."""
    m = 0xFFFFFFFF
    h = ((x * 0x9E3779B1) & m) ^ ((y * 0x85EBCA77) & m)
    h ^= (salt * 0xC2B2AE3D) & m
    h ^= h >> 15
    h = (h * 0x2C1B3C6D) & m
    h ^= h >> 12
    h = (h * 0x297A2D39) & m
    return h ^ (h >> 15)


# Proposed weights for variants 0-3: the quiet tile dominates so marks stay sparse.
WEIGHTS = [8, 3, 3, 2]


def pick_weighted(x, y, n):
    r = tile_hash(x, y, 1) % sum(WEIGHTS[:n])
    for i, w in enumerate(WEIGHTS[:n]):
        if r < w:
            return i
        r -= w
    return 0


def pick_today(x, y, n_variants):
    """GroundRule::floor_at today: one tile in four shows a variant."""
    h = tile_hash(x, y, 1)
    if n_variants == 0 or h % 4 != 0:
        return None
    return (h >> 8) % n_variants


def compare_floors(sheets, out_dir, cols=8, rows=5, k=3):
    """Each biome's floor today (Ninja Adventure cells, as GROUND picks them) next to ours."""
    floor = Image.open(NA / "TilesetFloor.png").convert("RGBA")
    relief = Image.open(NA / "TilesetRelief.png").convert("RGBA")
    cell = lambda im, c, r: im.crop((c * 16, r * 16, c * 16 + 16, r * 16 + 16))  # noqa: E731
    biomes = [  # (label, today's base, today's variants, our floor)
        ("plains", cell(floor, 0, 12), [cell(floor, c, 12) for c in (1, 2, 3)], "grass"),
        ("forest, taiga", cell(floor, 11, 12), [cell(floor, c, 12) for c in (12, 13)], "forest_floor"),
        ("desert, coast", cell(floor, 1, 1), [cell(floor, 0, 4), cell(floor, 1, 4)], "sand"),
        ("tundra", cell(floor, 1, 15), [], "snow"),
        ("swamp", cell(floor, 12, 15), [cell(floor, 13, 19), cell(floor, 14, 19)], "mud"),
        ("mountain top", cell(relief, 2, 6), [], "rock"),
    ]
    blocks = []
    for name, base, variants, ours in biomes:
        today = Image.new("RGBA", (cols * 16, rows * 16))
        mine = Image.new("RGBA", (cols * 16, rows * 16))
        mine_cells = [render.sprite(sheets, "floors", f"{ours}{i}") for i in range(4)]
        for y in range(rows):
            for x in range(cols):
                v = pick_today(x, y, len(variants))
                today.alpha_composite(base if v is None else variants[v], (x * 16, y * 16))
                mine.alpha_composite(mine_cells[pick_weighted(x, y, 4)], (x * 16, y * 16))
        strip = hstack([up(c, k) for c in mine_cells], gap=4)
        row = hstack([caption(up(today, k), f"{name}: today"), caption(up(mine, k), "ours"),
                      caption(strip, "our 4 variants")])
        blocks.append(row)
    vstack(blocks, gap=0).convert("RGB").save(out_dir / "compare-floors.png")


if __name__ == "__main__":
    seal_ui, out_dir = sys.argv[1], Path(sys.argv[2])
    out_dir.mkdir(parents=True, exist_ok=True)
    sheets = render.render_all()
    samples(sheets, out_dir)
    compare_ui(sheets, seal_ui, out_dir)
    compare_map(sheets, out_dir)
    compare_floors(sheets, out_dir)
    for p in sorted(out_dir.glob("*.png")):
        print(p)

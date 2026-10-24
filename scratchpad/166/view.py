"""Stacks the walk-idle row and the two bend rows of character sheets into one enlarged PNG.

python3 -I scratchpad/166/view.py OUT.png name [name ...]
"""

import sys

from PIL import Image

out, names = sys.argv[1], sys.argv[2:]
S = 5
BG = (96, 140, 70)
rows = []
for n in names:
    im = Image.open(f"assets/original/{n}.png").convert("RGBA")
    strip = Image.new("RGBA", (64, 48), BG + (255,))
    for i, r in enumerate((0, 4, 5)):
        strip.alpha_composite(im.crop((0, r * 16, 64, r * 16 + 16)), (0, i * 16))
    rows.append(strip.resize((64 * S, 48 * S), Image.NEAREST))
cols = 3
w, h = 64 * S + 8, 48 * S + 8
sheet = Image.new("RGBA", (cols * w, ((len(rows) + cols - 1) // cols) * h), (30, 30, 30, 255))
for i, r in enumerate(rows):
    sheet.alpha_composite(r, ((i % cols) * w + 4, (i // cols) * h + 4))
sheet.save(out)

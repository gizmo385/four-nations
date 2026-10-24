"""Enlarges rows 2.. of fx.png over a grass and a snow background: python3 -I viewfx.py OUT.png"""

import sys

from PIL import Image

S = 10
im = Image.open("assets/original/fx.png").convert("RGBA")
w, h = im.size
rows = (h // 16) - 2
out = Image.new("RGBA", (w * S * 2 + 30, rows * 16 * S + 20), (30, 30, 30, 255))
for k, bg in enumerate(((96, 140, 70), (150, 120, 80))):
    strip = Image.new("RGBA", (w, rows * 16), bg + (255,))
    strip.alpha_composite(im.crop((0, 32, w, h)))
    out.alpha_composite(strip.resize((w * S, rows * 16 * S), Image.NEAREST), (10 + k * (w * S + 10), 10))
out.save(sys.argv[1])

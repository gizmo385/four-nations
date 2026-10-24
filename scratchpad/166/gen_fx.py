"""Draws the round and swirling cast frames of art/fx.toml (dust, splash rings, gusts, whirls).

python3 -I scratchpad/166/gen_fx.py     (from the repo root; rewrites those sprites' grids)

The shapes are computed (circles, spirals) because they are awkward to place by hand; the result
is plain text in the TOML, which stays the source of truth.
"""

import math
import re

N = 16


def blank():
    return [["."] * N for _ in range(N)]


def put(g, x, y, ch):
    if 0 <= x < N and 0 <= y < N:
        g[y][x] = ch


def edge(g, body, ch):
    """Paints `ch` on clear pixels 4-adjacent to any pixel in `body` (a set of (x, y))."""
    for x, y in list(body):
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            if (x + dx, y + dy) not in body and g[y + dy][x + dx] == ".":
                g[y + dy][x + dx] = ch


def puffs(circles, keep):
    """Dust: lit from the top left. `keep(x, y)` thins the cloud out (a stipple)."""
    g = blank()
    for cx, cy, r in circles:
        for y in range(N):
            for x in range(N):
                d = math.hypot(x - cx, y - cy)
                if d <= r and keep(x, y):
                    shade = (x - cx) * 0.6 + (y - cy) * 0.8  # positive: away from the light
                    g[y][x] = "4" if shade < -r * 0.45 else "3" if shade < r * 0.35 else "2"
    return g


def ring(cx, cy, rx, ry, gap=None, thick=1.0):
    g = blank()
    for y in range(N):
        for x in range(N):
            d = math.hypot((x - cx) / rx, (y - cy) / ry)
            if abs(d - 1.0) * min(rx, ry) <= thick * 0.6:
                if gap and gap(x, y):
                    continue
                lit = (x - cx) / rx + (y - cy) / ry < 0
                g[y][x] = "4" if lit and (x + y) % 2 == 0 else "3" if lit else "2"
    return g


def spiral(turns, rot, cx=7.5, cy=7.5, rmax=5.2, body="4", rim="1"):
    g = blank()
    pts = set()
    steps = 400
    for i in range(steps + 1):
        t = i / steps
        a = rot + turns * 2 * math.pi * t
        r = 0.6 + (rmax - 0.6) * t
        x, y = round(cx + r * math.cos(a)), round(cy + r * math.sin(a) * 0.9)
        pts.add((x, y))
    for x, y in pts:
        put(g, x, y, body)
    edge(g, pts, rim)
    return g


def funnel(k):
    """A whirl of wind: slanting bands round a narrowing cone, gaps between, turning with `k`."""
    g = blank()
    body = set()
    top, bottom = 2, 13
    for r in range(top, bottom + 1):
        f = (r - top) / (bottom - top)
        half = 5.0 * (1 - f) + 1.0 * f
        x0, x1 = round(7.5 - half), round(7.5 + half)
        width = x1 - x0 + 1
        for x in range(x0, x1 + 1):
            if ((x - x0) + r * 3 + k * 4) % width < max(2, round(width * 0.55)):
                g[r][x] = "4" if (x - x0) % 3 else "3"
                body.add((x, r))
    edge(g, body, "1")
    return g


def text(g):
    return "\n".join("".join(r) for r in g)


SPRITES = {
    "dust_0": puffs([(4.5, 9.5, 2.9), (8.5, 6.5, 3.2), (11.5, 9.8, 2.7), (8, 10.5, 2.2)], lambda x, y: True),
    "dust_1": puffs([(3.5, 9, 3.2), (8, 5.5, 3.8), (12.5, 9.3, 3.2), (8, 10.5, 2.8)],
                    lambda x, y: (x + y) % 2 == 0),
    "splash_0": ring(8, 9.5, 4.3, 2.7),
    "splash_2": ring(8, 9.5, 6.4, 3.8, gap=lambda x, y: (x * 3 + y * 5) % 7 < 2),
    "gust_0": spiral(1.6, 0.0),
    "gust_1": spiral(1.6, 2.1),
    "gust_2": spiral(1.6, 4.2),
    "swirl_0": funnel(0),
    "swirl_1": funnel(1),
    "swirl_2": funnel(2),
}

path = "art/fx.toml"
src = open(path).read()
for name, g in SPRITES.items():
    pat = re.compile(r'(name = "%s"\n(?:[^\[]*?)grid = """\n)(.*?)(\n""")' % name, re.S)
    assert pat.search(src), name
    src = pat.sub(lambda m: m.group(1) + text(g) + m.group(3), src, count=1)
open(path, "w").write(src)
print("ok")

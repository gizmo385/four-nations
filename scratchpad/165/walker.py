#!/usr/bin/env python3
# Usage: walker.py NATION KIND  -> target/walker_NATION_KIND.png, close-ups of one demo walker
# over steps 1..10 (from target/raw_NATION_N.png). KIND is "across" (track 0) or "down" (track 3).
import subprocess, sys
nation, kind = sys.argv[1], sys.argv[2]
tile = 45.7
def tri(m, n):
    m %= 2 * n
    return m if m <= n else 2 * n - m
tiles = []
for n in range(1, 11):
    if kind == "across":
        x, y = 1 + tri(n, 5), -5
    else:
        x, y = 12, -6 + tri(n + 6, 6)
    sx = 395 + (x - 7) * tile
    sy = 200 + (y + 5) * tile
    out = f"target/walk_{nation}_{kind}_{n}.png"
    subprocess.run(["convert", f"target/raw_{nation}_{n}.png", "-crop", f"110x110+{int(sx-55)}+{int(sy-55)}",
                    "+repage", "-filter", "point", "-resize", "300%", "-fill", "white", "-pointsize", "20",
                    "-gravity", "NorthWest", "-annotate", "+4+2", f"step {n}", out], check=True)
    tiles.append(out)
subprocess.run(["montage", *tiles, "-tile", "5x2", "-geometry", "+3+3", "-background", "#222",
                f"target/walker_{nation}_{kind}.png"], check=True)

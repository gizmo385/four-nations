"""Renders the cast demo at several moments per nation and stitches a frame strip per ability.

python3 -I scratchpad/166/shots.py [nation ...]

Needs `cargo xtask screenshot` to work (headless). Writes target/166/<nation>_<t>.png and
mockups/166/<ability>.png (one strip per ability, one frame per moment, each captioned).
"""

import os
import subprocess
import sys

from PIL import Image, ImageDraw

SEED = "7"
# Per nation: ability, the moments to catch (seconds from completion), the screen point the
# action is centred on in a 1280x720 shot (a tile is about 57px at the demo's zoom), and the
# crop half-size.
PLAN = {
    "earth": ("raise_earth", [-0.8, -0.5, -0.1, 0.0, 0.2, 0.4], (625, 360)),
    "water": ("water_the_land", [-0.9, -0.6, -0.3, -0.1, 0.2, 0.4], (511, 360)),
    "fire": ("hearthflame", [-0.4, -0.15, 0.1, 0.3, 0.5], (511, 360)),
    "air": ("wind_carry", [-0.6, 0.0, 0.35, 0.7, 1.0], (560, 360)),
}
HALF = (170, 130)
WIDE = {"air": (300, 130)}


def shoot(nation, t):
    out = f"target/166/{nation}_{t}.png"
    os.makedirs("target/166", exist_ok=True)
    env = dict(os.environ, FN_CAST_DEMO=str(t), FN_DEVSHELL="1", CARGO_PROFILE_DEV_DEBUG="0")
    r = subprocess.run(["cargo", "xtask", "screenshot", SEED, nation, out], env=env,
                       capture_output=True, text=True)
    if r.returncode != 0:
        print(r.stdout[-2000:], r.stderr[-2000:])
        raise SystemExit(f"{nation} {t} failed")
    return out


def strip(nation, shots):
    ability, ts, (cx, cy) = PLAN[nation]
    half = WIDE.get(nation, HALF)
    frames = []
    for t, path in zip(ts, shots):
        im = Image.open(path).convert("RGB")
        crop = im.crop((cx - half[0], cy - half[1], cx + half[0], cy + half[1]))
        frames.append((t, crop))
    w, h = frames[0][1].size
    out = Image.new("RGB", (w * len(frames) + 4 * (len(frames) - 1), h + 22), (24, 24, 30))
    d = ImageDraw.Draw(out)
    for i, (t, crop) in enumerate(frames):
        out.paste(crop, (i * (w + 4), 22))
        label = "channelling" if t < 0 else "landed"
        d.text((i * (w + 4) + 6, 5), f"{ability}  t={t:+.2f}s ({label})", fill=(230, 230, 230))
    os.makedirs("mockups/166", exist_ok=True)
    out.save(f"mockups/166/{ability}.png")
    print("wrote", f"mockups/166/{ability}.png")


if __name__ == "__main__":
    for nation in sys.argv[1:] or PLAN:
        _, ts, _ = PLAN[nation]
        strip(nation, [shoot(nation, t) for t in ts])

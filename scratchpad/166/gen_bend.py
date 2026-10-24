"""Appends the bend pose rows (4 and 5) to every character sheet in art/ (#166).

Run from the repo root:  python3 -I scratchpad/166/gen_bend.py [character ...]

Each pose is derived from the character's own idle cells (down_0, up_0, left_0), so the clothes,
hair and palette stay the character's; only the stance and arms change. The right-facing cells
mirror the left-facing ones, as the walk cells do. The generated sprites are written into the
TOML as plain grids (after a marker comment), so the TOML is the source of truth and this script
is only a drawing aid. Re-running replaces everything after the marker.
"""

import re
import sys
import tomllib

MARK = "# --- bend poses (#166)"

NATION = {
    "earth_farmer": "earth", "earth_guard": "earth", "earth_villager": "earth",
    "fire_noble": "fire", "fire_smith": "fire", "fire_soldier": "fire",
    "water_healer": "water", "water_hunter": "water", "water_warrior": "water",
    "air_acolyte": "air", "air_elder": "air", "air_monk": "air",
}
# Characters whose down_0 shows no skin-coloured hand on the arm column.
HAND_ROW = {"water_hunter": 11, "water_warrior": 11}


def px(g, r, c, ch):
    if 0 < r < 15 and 0 < c < 15:
        g[r][c] = ch


def shift(g, r0, r1, dy=0, dx=0):
    """Moves rows r0..r1 by (dy, dx); what they vacate is clear."""
    region = [row[:] for row in g[r0 : r1 + 1]]
    for r in range(r0, r1 + 1):
        g[r] = ["."] * 16
    for i, row in enumerate(region):
        r = r0 + i + dy
        if not 0 <= r < 16:
            continue
        for c in range(16):
            if 0 <= c + dx < 16 and row[c] != ".":
                g[r][c + dx] = row[c]


def mirror(g):
    return [row[::-1] for row in g]


class Char:
    def __init__(self, name):
        self.name = name
        self.path = f"art/{name}.toml"
        text = open(self.path).read()
        self.head = text.split(MARK)[0].rstrip("\n") + "\n"
        doc = tomllib.loads(self.head)
        grids = {s["name"]: [list(r) for r in s["grid"].strip("\n").split("\n")] for s in doc["sprite"]}
        self.idle = {v: grids[f"{v}_0"] for v in ("down", "up", "left")}
        right = grids["right_0"]
        assert right == mirror(grids["left_0"]), f"{name}: right_0 is not left_0 mirrored"
        self.nation = NATION[name]
        d = self.idle["down"]
        self.hr = HAND_ROW.get(name) or max(r for r in range(16) if d[r][3] == "S")
        s = d[self.hr - 1][3]
        self.sleeve = "S" if s in ".S" else s
        row13 = "".join(d[13]).strip(".")
        self.legged = "." in row13
        self.body_end = 12 if self.legged else 13
        self.boot = next(ch for ch in d[14] if ch != ".")
        self.leg = next(ch for ch in d[13] if ch != ".") if self.legged else self.boot

    # ---- shared stance parts -------------------------------------------------------------
    def crouch(self, g, dy=1):
        shift(g, 0, self.body_end, dy, 0)
        return self.hr + dy

    def lift(self, g):
        """Rises a pixel; the legs (or the hem of a long robe) stretch to follow."""
        shift(g, 0, self.body_end, -1, 0)
        gap = self.body_end
        if self.legged:
            g[gap] = [self.leg if c in (5, 6, 9, 10) else "." for c in range(16)]
        else:
            g[gap] = g[gap - 1][:]
        return self.hr - 1

    def wide_feet(self, g, reach=1):
        """Feet planted apart (front and back views); `reach` 0 is a narrow stance."""
        g[14] = ["."] * 16
        for c in (2, 3, 4) if reach else (3, 4, 5):
            g[14][c] = self.boot
            g[14][15 - c] = self.boot
        if self.legged:
            g[13] = ["." if c in (5, 6, 9, 10) else ch for c, ch in enumerate(g[13])]

    def clear_arms(self, g, hr):
        for r in (hr - 1, hr):
            for c in (3, 12):
                if g[r][c] != ".":
                    g[r][c] = "."

    def arms(self, g, hr, left, right=None):
        """Draws arms from (dr, c, 's'|'h'|'t') cells; `right` defaults to `left` mirrored."""
        right = right if right is not None else [(dr, 15 - c, k) for dr, c, k in left]
        for dr, c, k in left + right:
            px(g, hr + dr, c, {"s": self.sleeve, "h": "S", "t": "T"}[k])

    # ---- the poses ---------------------------------------------------------------------
    def pose(self, view, phase):
        g = [row[:] for row in self.idle[view]]
        fn = getattr(self, f"{self.nation}_{'side' if view == 'left' else 'front'}")
        fn(g, phase, back=(view == "up"))
        return g

    # Earth: low and grounded; fists planted, then the earth lifted.
    def earth_front(self, g, phase, back):
        hr = self.crouch(g)
        self.clear_arms(g, hr - 1)
        self.wide_feet(g)
        if phase == 4:
            self.arms(g, hr, [(-1, 3, "s"), (0, 2, "s"), (1, 1, "h"), (1, 2, "h"), (0, 1, "t")])
        else:
            self.arms(g, hr, [(-1, 3, "s"), (-2, 2, "s"), (-3, 2, "s"), (-4, 1, "h"), (-4, 2, "h"),
                              (-5, 1, "t"), (-5, 2, "h")])

    def earth_side(self, g, phase, back):
        hr = self.crouch(g)
        g[hr][5] = g[hr][6]
        self.side_feet(g, wide=True)
        if phase == 4:
            self.arms(g, hr, [(-1, 4, "s"), (0, 3, "s"), (1, 2, "h"), (1, 3, "h"), (0, 2, "t")], [])
        else:
            self.arms(g, hr, [(-1, 4, "s"), (-2, 3, "s"), (-3, 3, "s"), (-4, 2, "h"), (-4, 3, "h"),
                              (-5, 2, "t"), (-5, 3, "h")], [])

    # Fire: a guard, then a punch.
    def fire_front(self, g, phase, back):
        hr = self.hr
        self.clear_arms(g, hr)
        if phase == 4:
            hr = self.crouch(g, 0)
            self.wide_feet(g, reach=0)
            self.arms(g, hr, [(-1, 3, "s"), (-2, 2, "s"), (-3, 2, "h"), (-4, 2, "h"), (-3, 3, "t")],
                      [(-1, 12, "s"), (0, 12, "h"), (0, 11, "h")])
        elif back:
            hr = self.crouch(g)
            self.clear_arms(g, hr - 1)
            self.wide_feet(g)
            self.arms(g, hr, [(0, 4, "h")], [(-1, 12, "s"), (-2, 13, "s"), (-3, 13, "s"), (-4, 13, "h"),
                                              (-5, 13, "h"), (-4, 14, "t")])
        else:
            hr = self.crouch(g)
            self.clear_arms(g, hr - 1)
            self.wide_feet(g)
            self.arms(g, hr, [(0, 3, "h"), (0, 4, "h")],
                      [(-1, 12, "s"), (-2, 11, "s"), (-3, 8, "t"), (-3, 9, "h"), (-3, 10, "s"),
                       (-2, 8, "h"), (-2, 9, "h"), (-4, 8, "h"), (-4, 9, "h")])

    def fire_side(self, g, phase, back):
        hr = self.crouch(g, 0 if phase == 4 else 1)
        g[hr][5] = g[hr][6]
        self.side_feet(g, wide=True)
        if phase == 4:
            self.arms(g, hr, [(-1, 4, "s"), (-2, 3, "s"), (-3, 3, "h"), (-3, 2, "h"), (-2, 2, "t"),
                              (-4, 3, "h")], [])
        else:
            self.arms(g, hr, [(-2, 5, "s"), (-2, 4, "s"), (-2, 3, "s"), (-2, 2, "h"), (-2, 1, "h"),
                              (-3, 1, "h"), (-3, 2, "h"), (-1, 1, "t"), (-1, 2, "h")], [])

    # Water: arms flowing in waves, then sweeping into a lean.
    def water_front(self, g, phase, back):
        hr = self.hr
        self.clear_arms(g, hr)
        if phase == 4:
            self.arms(g, hr, [(-1, 3, "s"), (-2, 2, "s"), (-3, 2, "s"), (-4, 1, "s"), (-5, 1, "h"),
                              (-5, 2, "h")],
                      [(-1, 12, "s"), (-1, 13, "s"), (0, 14, "s"), (1, 14, "h"), (1, 13, "h")])
        else:
            shift(g, 0, self.body_end, 0, 1)
            self.arms(g, hr, [(-1, 4, "s"), (-1, 3, "s"), (-2, 2, "s"), (-3, 1, "s"), (-4, 1, "h"),
                              (-4, 2, "h")],
                      [(-1, 13, "s"), (-2, 14, "s"), (-3, 14, "s"), (-4, 14, "h"), (-4, 13, "h")])

    def water_side(self, g, phase, back):
        hr = self.hr
        g[hr][5] = g[hr][6]
        if phase == 4:
            self.arms(g, hr, [(-1, 4, "s"), (-2, 3, "s"), (-3, 2, "s"), (-3, 3, "s"), (-4, 1, "h"),
                              (-4, 2, "h")], [])
        else:
            shift(g, 0, self.body_end, 0, -1)
            self.arms(g, hr, [(-1, 3, "s"), (-1, 4, "s"), (-2, 2, "s"), (-3, 1, "s"), (-3, 2, "s"),
                              (-4, 1, "h"), (-4, 2, "h")], [])

    # Air: light on its feet, one arm sweeping.
    def air_front(self, g, phase, back):
        hr = self.lift(g)
        self.clear_arms(g, hr)
        if phase == 4:
            self.arms(g, hr, [(-1, 3, "s"), (-2, 2, "s"), (-3, 2, "s"), (-4, 2, "s"), (-5, 2, "s"),
                              (-6, 2, "h"), (-6, 1, "t")],
                      [(-1, 12, "s"), (0, 13, "s"), (1, 13, "h")])
        else:
            self.arms(g, hr, [(-2, 3, "s"), (-2, 2, "s"), (-2, 1, "s"), (-3, 1, "h"), (-1, 1, "t")],
                      [(-2, 12, "s"), (-3, 13, "s"), (-4, 14, "s"), (-5, 14, "h"), (-4, 13, "h")])

    def air_side(self, g, phase, back):
        hr = self.lift(g)
        g[hr][5] = g[hr][6]
        if phase == 4:
            self.arms(g, hr, [(-1, 4, "s"), (-2, 3, "s"), (-3, 2, "s"), (-4, 2, "s"), (-5, 2, "s"),
                              (-6, 2, "h"), (-6, 1, "t")], [])
        else:
            self.arms(g, hr, [(-2, 4, "s"), (-2, 3, "s"), (-2, 2, "s"), (-2, 1, "h"), (-3, 1, "h"),
                              (-1, 1, "t")], [])

    def side_feet(self, g, wide):
        g[14] = ["."] * 16
        for c in (3, 4, 5, 9, 10, 11):
            g[14][c] = self.boot
        if self.legged:
            g[13] = [self.leg if c in (5, 9) else "." for c in range(16)]
            g[13][4] = self.leg
            g[13][10] = self.leg

    # ---- output --------------------------------------------------------------------------
    def sprites(self):
        out = []
        for phase in (4, 5):
            for view in ("down", "up", "left"):
                out.append((f"{view}_{phase}", self.pose(view, phase)))
            out.append((f"right_{phase}", mirror(self.pose("left", phase))))
        return out

    def render(self):
        text = self.head + f"\n{MARK}: row 4 is the stance held while channelling, row 5 the release.\n"
        for name, g in self.sprites():
            body = "\n".join("".join(r) for r in g)
            text += f'\n[[sprite]]\nname = "{name}"\ngrid = """\n{body}\n"""\n'
        return text


if __name__ == "__main__":
    names = sys.argv[1:] or sorted(NATION)
    for n in names:
        c = Char(n)
        open(c.path, "w").write(c.render())
        print("wrote", c.path)

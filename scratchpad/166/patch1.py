import re
p='scratchpad/166/gen_bend.py'
s=open(p).read()
a=s.index('    # Earth: low and grounded')
b=s.index('    def side_feet')
new='''    # Earth: low and grounded; fists planted, then the earth lifted.
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

'''
s=s[:a]+new+s[b:]
open(p,'w').write(s)

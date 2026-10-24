p='scratchpad/166/gen_bend.py'
s=open(p).read()
a=s.index('    def lift(self, g):')
b=s.index('    def clear_arms')
new='''    def lift(self, g):
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

'''
s=s[:a]+new+s[b:]
open(p,'w').write(s)

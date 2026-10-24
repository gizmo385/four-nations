p="scratchpad/166/shots.py"
s=open(p).read()
s=s.replace('"air": ("wind_carry", [-0.8, -0.4, 0.0, 0.3, 0.6, 0.9], (400, 360)),','"air": ("wind_carry", [-0.6, 0.0, 0.35, 0.7, 1.0], (560, 360)),')
s=s.replace("HALF = (170, 130)","HALF = (170, 130)\nWIDE = {\"air\": (300, 130)}")
s=s.replace("ability, ts, (cx, cy) = PLAN[nation]\n","ability, ts, (cx, cy) = PLAN[nation]\n    half = WIDE.get(nation, HALF)\n")
s=s.replace("cx - HALF[0], cy - HALF[1], cx + HALF[0], cy + HALF[1]","cx - half[0], cy - half[1], cx + half[0], cy + half[1]")
open(p,"w").write(s)

import sys, tomllib

for n in sys.argv[1:]:
    d = tomllib.load(open(f"art/{n}.toml", "rb"))
    sp = {s["name"]: s["grid"].strip("\n").split("\n") for s in d["sprite"]}
    print(n, d.get("legend"))
    for r in range(16):
        print(" ".join(sp[f + "_0"][r] for f in ["down", "up", "left", "right"]), "|", " ".join(sp[f + "_1"][r] for f in ["down", "left"]))

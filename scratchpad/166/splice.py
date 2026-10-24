p = "crates/client/src/cast.rs"
s = open(p).read()
i = s.index("#[cfg(test)]\nmod tests")
s = s[:i] + open("scratchpad/166/cast_tests.rs").read()
s = s.replace(
    "lifted parts (a droplet, a stack in flight) over them.",
    "parts in the air (a droplet, a spark, a stack in flight) over them.",
)
open(p, "w").write(s)

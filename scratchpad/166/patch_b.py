p="crates/client/src/bending.rs"
s=open(p).read()
a="//! - Each `BendEvent` becomes a line"
assert a in s
s=s.replace(a,"//! - The moment of a bend (the pose and the cast effect) is `cast`'s, #166.\n"+a,1)
open(p,"w").write(s)

p="crates/client/src/cast.rs"
s=open(p).read()
a='''        if let Some(mut now) = markers.stored(pos).and_then(|e| visibility.get_mut(e).ok()) {
            if *now != v {
                *now = v;
            }
        }'''
b='''        if let Some(mut now) = markers.stored(pos).and_then(|e| visibility.get_mut(e).ok())
            && *now != v
        {
            *now = v;
        }'''
assert a in s
s=s.replace(a,b)
a='''            let target = (2..6)
                .map(|dx| Pos::new(pos.x + dx, pos.y))
                .find(|&p| sim.apply(Command::Bend { ability, pos: p }).is_ok())
                .expect("a tile to the right takes a flame");
            target'''
b='''            (2..6)
                .map(|dx| Pos::new(pos.x + dx, pos.y))
                .find(|&p| sim.apply(Command::Bend { ability, pos: p }).is_ok())
                .expect("a tile to the right takes a flame")'''
assert a in s
s=s.replace(a,b)
open(p,"w").write(s)

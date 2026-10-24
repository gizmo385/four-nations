p='crates/client/src/cast.rs'
s=open(p).read()
def rep(a,b,n=1):
    global s
    assert a in s, a
    s=s.replace(a,b,n)
rep('''pub struct Part {
    pub at: Vec2,
    pub lift: f32,
    pub visual: Visual,
}''','''pub struct Part {
    pub at: Vec2,
    pub lift: f32,
    pub visual: Visual,
    /// Drawn over the units of its row (a spark, a droplet), not under them (stones, a splash).
    pub over: bool,
}''')
rep('''fn fx(at: Vec2, lift: f32, cell: sprites::Cell) -> Part {
    Part { at, lift, visual: Visual::Fx(cell) }
}''','''/// A part on the ground, under the units of its row.
fn fx(at: Vec2, lift: f32, cell: sprites::Cell) -> Part {
    Part { at, lift, visual: Visual::Fx(cell), over: false }
}

/// A part in the air, over the units of its row.
fn fx_over(at: Vec2, lift: f32, cell: sprites::Cell) -> Part {
    Part { over: true, ..fx(at, lift, cell) }
}''')
rep("out.push(fx(hand.lerp(aim, u), lift, sprites::fx::DROP[k % 3]));","out.push(fx_over(hand.lerp(aim, u), lift, sprites::fx::DROP[k % 3]));")
rep("    out.push(fx(s.target, lift, cell));\n}\n\n/// Curls","    out.push(fx_over(s.target, lift, cell));\n}\n\n/// Curls")
rep('''            let wobble = 3.0 * (std::f32::consts::TAU * (u * 2.0 + k as f32 * 0.25)).sin();
            out.push(fx(hand.lerp(s.target, u), HAND_LIFT + wobble, spin(k)));''','''            let wobble = 3.0 * (std::f32::consts::TAU * (u * 2.0 + k as f32 * 0.25)).sin();
            // A bender standing on the stack whirls the wind round it instead.
            let at = if s.stand.distance(s.target) < 0.5 {
                let a = std::f32::consts::TAU * (u + k as f32 / GUST_CURLS as f32);
                s.target + Vec2::new(a.cos(), a.sin() * 0.6) * 0.8
            } else {
                hand.lerp(s.target, u)
            };
            out.push(fx_over(at, HAND_LIFT + wobble, spin(k)));''')
rep("out.push(fx(s.target, 0.0, sprites::fx::SWIRL[i]));","out.push(fx_over(s.target, 0.0, sprites::fx::SWIRL[i]));")
rep("out.push(Part { at: s.target.lerp(to, u), lift: arc(u), visual: Visual::Item });","out.push(Part { at: s.target.lerp(to, u), lift: arc(u), visual: Visual::Item, over: true });")
rep("out.push(fx(s.target.lerp(to, behind), arc(behind), spin(j)));","out.push(fx_over(s.target.lerp(to, behind), arc(behind), spin(j)));")
rep('''    let bias = if part.lift > 0.0 { FLYING_BIAS } else { FX_BIAS };''','''    let bias = if part.over { OVER_BIAS } else { FX_BIAS };''')
rep("const FLYING_BIAS: f32 = 0.0015;","const OVER_BIAS: f32 = 0.0015;")
open(p,'w').write(s)

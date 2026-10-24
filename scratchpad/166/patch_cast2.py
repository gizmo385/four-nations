p='crates/client/src/cast.rs'
s=open(p).read()
def rep(a,b):
    global s
    assert a in s, a
    s=s.replace(a,b,1)
rep('''            let lift = HAND_LIFT * (1.0 - u) + 4.0 * STREAM_ARC * u * (1.0 - u);
            out.push(fx(hand.lerp(s.target, u), lift, sprites::fx::DROP[k % 3]));''','''            // A bender standing on its target sprays the ring around it instead.
            let aim = if standing_on_target { s.target + SPRAY[k % SPRAY.len()] } else { s.target };
            let lift = HAND_LIFT * (1.0 - u) + 4.0 * STREAM_ARC * u * (1.0 - u);
            out.push(fx(hand.lerp(aim, u), lift, sprites::fx::DROP[k % 3]));''')
rep('''        let hand = s.hand();
        // The stream reaches out''','''        let hand = s.hand();
        let standing_on_target = s.stand.distance(s.target) < 0.5;
        // The stream reaches out''')
rep('''/// Water the Land: droplets run''','''/// Where a stream aimed at the bender's own tile sprays: the ring around it.
const SPRAY: [Vec2; 8] = [
    Vec2::new(1.0, 0.0),
    Vec2::new(-1.0, 0.0),
    Vec2::new(0.0, 1.0),
    Vec2::new(0.0, -1.0),
    Vec2::new(1.0, 1.0),
    Vec2::new(-1.0, -1.0),
    Vec2::new(1.0, -1.0),
    Vec2::new(-1.0, 1.0),
];

/// Water the Land: droplets run''')
rep('''/// Above the unit of the same row; flying parts above the row below too.
const FX_BIAS: f32 = 0.0009;''','''/// Parts on the ground sit over the floor and any flame, under the units of their row (a bender
/// standing on its target isn't covered); lifted parts (a droplet, a stack in flight) over them.
const FX_BIAS: f32 = 0.0004;''')
open(p,'w').write(s)

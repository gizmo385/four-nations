//! Small, deterministic 2D gradient noise for map generation.
//!
//! Why in-house instead of the `noise` crate: the output of every map is pinned by golden tests
//! and must be identical on every platform and across dependency upgrades. This file is ~100
//! lines, seeds itself from our own `rng::stream`, and uses only `+`, `-`, `*` and `floor` on
//! `f64` (no `powf`, `sin`, fused multiply-add or other operations whose rounding may differ
//! between platforms or libm versions), so its results are bit-for-bit reproducible.
//!
//! The algorithm is classic Perlin "improved noise" (shuffled permutation table, quintic fade,
//! eight gradient directions), layered as fractal Brownian motion (fBm) and as ridged
//! multifractal noise for mountain ranges.

use rand::seq::SliceRandom;

use crate::rng::SimRng;

/// One octave of 2D gradient noise. Output is roughly in `[-1, 1]`, `0` at lattice points.
#[derive(Debug, Clone)]
pub struct Gradient {
    perm: [u8; 512],
}

impl Gradient {
    pub fn new(rng: &mut SimRng) -> Self {
        let mut p: Vec<u8> = (0..=255).collect();
        p.shuffle(rng);
        let mut perm = [0u8; 512];
        for (i, slot) in perm.iter_mut().enumerate() {
            *slot = p[i & 255];
        }
        Self { perm }
    }

    fn hash(&self, x: i64, y: i64) -> u8 {
        let a = self.perm[(x & 255) as usize] as usize;
        self.perm[a + (y & 255) as usize]
    }

    pub fn get(&self, x: f64, y: f64) -> f64 {
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (ix, iy) = (x0 as i64, y0 as i64);
        let g =
            |dx: i64, dy: i64| grad(self.hash(ix + dx, iy + dy), fx - dx as f64, fy - dy as f64);
        let (u, v) = (fade(fx), fade(fy));
        let top = lerp(g(0, 0), g(1, 0), u);
        let bottom = lerp(g(0, 1), g(1, 1), u);
        lerp(top, bottom, v)
    }
}

fn grad(hash: u8, x: f64, y: f64) -> f64 {
    // Eight directions; diagonals are scaled by ~1/sqrt(2) so all have similar weight.
    const D: f64 = 0.707_106_781_186_547_5;
    match hash & 7 {
        0 => x,
        1 => -x,
        2 => y,
        3 => -y,
        4 => (x + y) * D,
        5 => (x - y) * D,
        6 => (-x + y) * D,
        _ => (-x - y) * D,
    }
}

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Hermite smoothstep: 0 below `lo`, 1 above `hi`, smooth in between.
pub fn smoothstep(lo: f64, hi: f64, x: f64) -> f64 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Several octaves of gradient noise at doubling frequency and halving amplitude.
#[derive(Debug, Clone)]
pub struct Fbm {
    /// Each octave's noise and its random sampling offset.
    octaves: Vec<(Gradient, f64, f64)>,
}

impl Fbm {
    pub fn new(rng: &mut SimRng, octaves: usize) -> Self {
        use rand::RngExt;
        assert!(octaves > 0, "fBm needs at least one octave");
        let octaves = (0..octaves)
            .map(|_| {
                let g = Gradient::new(rng);
                (g, rng.random_range(0.0..256.0), rng.random_range(0.0..256.0))
            })
            .collect();
        Self { octaves }
    }

    /// Calls `f(octave value, amplitude)` per octave. Each octave is offset and rotated by
    /// ~37 degrees (an exact 3-4-5 rotation, no trig) relative to the previous one; otherwise
    /// all octaves share lattice lines and the sum shows axis-aligned straight edges.
    fn octaves(&self, x: f64, y: f64, mut f: impl FnMut(f64, f64)) -> f64 {
        let (mut x, mut y, mut amp, mut norm) = (x, y, 1.0, 0.0);
        for (g, ox, oy) in &self.octaves {
            f(g.get(x + ox, y + oy), amp);
            norm += amp;
            amp *= 0.5;
            (x, y) = (2.0 * (0.8 * x - 0.6 * y), 2.0 * (0.6 * x + 0.8 * y));
        }
        norm
    }

    /// Fractal sum, normalized by total amplitude so the result stays roughly in `[-1, 1]`.
    pub fn get(&self, x: f64, y: f64) -> f64 {
        let mut sum = 0.0;
        let norm = self.octaves(x, y, |n, amp| sum += amp * n);
        sum / norm
    }

    /// Ridged multifractal: `(1 - |noise|)^2` per octave gives sharp crests along the noise's
    /// zero lines, i.e. long connected ridges instead of round blobs. Result is in `[0, 1]`.
    pub fn ridged(&self, x: f64, y: f64) -> f64 {
        let mut sum = 0.0;
        let norm = self.octaves(x, y, |n, amp| {
            let r = 1.0 - n.abs();
            sum += amp * r * r;
        });
        sum / norm
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng;

    #[test]
    fn zero_at_lattice_points_and_bounded() {
        let n = Gradient::new(&mut rng::stream(1, "test.noise"));
        assert_eq!(n.get(3.0, 7.0), 0.0);
        for i in 0..2000 {
            let v = n.get(i as f64 * 0.137, i as f64 * 0.071 - 40.0);
            assert!((-1.0..=1.0).contains(&v), "{v}");
        }
    }

    #[test]
    fn same_seed_same_values() {
        let a = Fbm::new(&mut rng::stream(9, "test.noise"), 4);
        let b = Fbm::new(&mut rng::stream(9, "test.noise"), 4);
        for i in 0..100 {
            let (x, y) = (i as f64 * 0.31, i as f64 * -0.17);
            assert_eq!(a.get(x, y).to_bits(), b.get(x, y).to_bits());
            assert_eq!(a.ridged(x, y).to_bits(), b.ridged(x, y).to_bits());
        }
    }

    #[test]
    fn noise_is_continuous() {
        let n = Fbm::new(&mut rng::stream(2, "test.noise"), 3);
        for i in 0..500 {
            let x = i as f64 * 0.05;
            assert!((n.get(x, 1.3) - n.get(x + 0.001, 1.3)).abs() < 0.05);
        }
    }
}

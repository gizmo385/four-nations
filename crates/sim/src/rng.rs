//! Deterministic randomness.
//!
//! All randomness in the sim flows from a single world seed. Each subsystem draws from its own
//! named stream so that, e.g., adding a new map-generation pass does not reshuffle unit behaviour
//! for the same seed. Never use `rand::rng()`/thread RNGs in the sim.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

pub type SimRng = ChaCha8Rng;

/// Returns an RNG for the named stream, derived from `seed`.
pub fn stream(seed: u64, name: &str) -> SimRng {
    // FNV-1a over the stream name, mixed with the seed. Stable across platforms and Rust versions.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    SimRng::seed_from_u64(seed ^ h)
}

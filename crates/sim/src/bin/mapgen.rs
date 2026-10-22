//! Prints a generated map as ASCII. Handy for eyeballing generator changes without a GPU.
//!
//!     cargo run -p fn_sim --bin mapgen -- [seed] [width] [height]

use fn_sim::{Defs, MapGenConfig, mapgen};

fn main() {
    let args: Vec<u64> = std::env::args()
        .skip(1)
        .map(|a| a.parse().expect("arguments must be integers: [seed] [width] [height]"))
        .collect();
    let defaults = MapGenConfig::default();
    let cfg = MapGenConfig {
        seed: args.first().copied().unwrap_or(defaults.seed),
        width: args.get(1).map_or(defaults.width, |&w| w as i32),
        height: args.get(2).map_or(defaults.height, |&h| h as i32),
        ..defaults
    };
    let defs = Defs::bundled();
    print!("{}", mapgen::to_ascii(&mapgen::generate(&cfg, &defs), &defs));
}

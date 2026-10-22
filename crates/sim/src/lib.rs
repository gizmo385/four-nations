//! Headless simulation core for The Four Nations.
//!
//! Rules for this crate (see AGENTS.md): no rendering, no windowing, no file or network I/O, no
//! wall-clock time, no thread RNG. Everything is a pure function of the seed, the game data and
//! the commands applied. That keeps it testable without a GPU and reusable from any frontend.

pub mod defs;
pub mod geom;
pub mod mapgen;
pub mod nation;
pub mod rng;
pub mod world;

pub use defs::{Defs, Terrain};
pub use geom::{Grid, Pos};
pub use mapgen::{MapGenConfig, Tile, TileMap};
pub use nation::Nation;
pub use world::World;

//! Headless simulation core for The Four Nations.
//!
//! Rules for this crate (see AGENTS.md): no rendering, no windowing, no file or network I/O, no
//! wall-clock time, no thread RNG. Everything is a pure function of the seed, the game data and
//! the commands applied. That keeps it testable without a GPU and reusable from any frontend.

pub mod autoplay;
pub mod balance;
pub mod beast;
pub mod bending;
pub mod building;
pub mod defs;
pub mod difficulty;
pub mod embark;
pub mod endeavor;
pub mod flood;
pub mod geom;
pub mod harm;
pub mod jobs;
pub mod mapgen;
pub mod nation;
pub mod needs;
pub mod noise;
pub mod path;
pub mod regions;
pub mod research;
pub mod rng;
pub mod save;
pub mod score;
pub mod seasons;
pub mod trade;
pub mod world;

pub use defs::{Defs, Terrain};
pub use geom::{Grid, Pos};
pub use mapgen::{MapGenConfig, Tile, TileMap};
pub use nation::Nation;
pub use path::MoveRules;
pub use regions::Regions;
pub use save::{SAVE_VERSION, SaveError, SaveInfo};
pub use world::World;

//! The pure core of the game. For now: endless, deterministic terrain.
//!
//! The world is an endless grid of tiles. Each tile is a pure function of `(seed, x, y)`: four noise fields
//! (elevation, moisture, temperature, age) sampled at the tile, and a biome looked up from the first three
//! in [`BIOMES`]. Nothing is stored and nothing depends on the order tiles are asked for, so a tile read on
//! its own equals the same tile read as part of a [`Chunk`], on any machine.
//!
//! This crate does no I/O, reads no clock, and starts no threads.
//!
//! ```
//! use blah_core::World;
//!
//! let world = World::new(42);
//! let tile = world.tile(-1_000, 250);
//! println!("{} at elevation {:.2}", tile.biome.biome().name, tile.elevation);
//! ```

mod biome;
mod chunk;
mod fields;
mod hash;
mod noise;
mod world;

pub use biome::{BIOMES, Band, Biome, BiomeId, SEA_LEVEL, classify};
pub use chunk::{CHUNK_AREA, CHUNK_SIZE, Chunk, chunk_of, chunk_origin, local_of};
pub use world::{Tile, World};

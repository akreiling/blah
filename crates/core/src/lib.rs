//! The pure core of the game. For now: endless, deterministic terrain, its regions, and a wanderer walking
//! it.
//!
//! The world is an endless grid of tiles. Each tile is a pure function of `(seed, x, y)`: four noise fields
//! (elevation, moisture, temperature, age) sampled at the tile, and a biome looked up from the first three
//! in [`BIOMES`]. Nothing is stored and nothing depends on the order tiles are asked for, so a tile read on
//! its own equals the same tile read as part of a [`Chunk`], on any machine.
//!
//! Tiles group into [`Region`]s, the units life will live in, which are just as pure: see [`World::region_at`]
//! and, for lookups on every step, [`RegionCache`].
//!
//! This crate does no I/O, reads no clock, and starts no threads.
//!
//! ```
//! use blah_core::World;
//!
//! let world = World::new(42);
//! let tile = world.tile(-1_000, 250);
//! println!("{} at elevation {:.2}", tile.biome.biome().name, tile.elevation);
//! let region = world.region(world.region_at(-1_000, 250)).unwrap();
//! println!("in {}, {} tiles", region.name, region.tiles);
//! ```

mod biome;
mod chunk;
mod fields;
mod game;
mod hash;
mod names;
mod noise;
mod region;
mod world;

pub use biome::{BIOMES, Band, Biome, BiomeId, SEA_LEVEL, classify};
pub use chunk::{CHUNK_AREA, CHUNK_SIZE, Chunk, chunk_of, chunk_origin, local_of};
pub use game::{Dir, Game, LAND_SEARCH_RADIUS, LOG_LIMIT, Log, Step, find_land};
pub use region::{
    Bounds, CELL_SPACING, CORE_TILES, MAX_REGION_TILES, Means, Region, RegionCache, RegionId, SLIVER_TILES, STRAY_TILES,
};
pub use world::{Tile, World};

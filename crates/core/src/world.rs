//! The world: a seed, and the tiles that follow from it.

use crate::biome::{BiomeId, classify};
use crate::chunk::{CHUNK_AREA, CHUNK_SIZE, Chunk, chunk_origin};
use crate::fields::{Seeds, sample};

/// One generated world. Holds nothing but what follows from its seed, so it is cheap to copy and every
/// method is a pure function of the seed and its arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct World {
    seed: u64,
    seeds: Seeds,
}

/// Everything terrain knows about one tile.
///
/// Field values are kept alongside the biome because later systems (fitness, succession) read the
/// gradient within a biome, not just its name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    /// The biome these field values classify to.
    pub biome: BiomeId,
    /// Height, `[0, 1]`. Below [`SEA_LEVEL`](crate::SEA_LEVEL) is water.
    pub elevation: f32,
    /// Dry to wet, `[0, 1]`.
    pub moisture: f32,
    /// Cold to hot, `[0, 1]`, after cooling with height.
    pub temperature: f32,
    /// How long life has existed here, young to old, `[0, 1]`.
    pub age: f32,
}

impl World {
    /// The world for `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> World {
        World { seed, seeds: Seeds::new(seed) }
    }

    /// The seed this world was made from.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// The tile at `(x, y)`. Every `i64` coordinate is valid. Terrain is seamless everywhere except within
    /// 2^24 tiles of the `i64` limits, where the noise's lattice shifts wrap around.
    #[must_use]
    pub fn tile(&self, x: i64, y: i64) -> Tile {
        let s = sample(&self.seeds, x, y);
        // Classify the stored f32 values, not the f64 originals, so a tile's biome always agrees with its
        // own fields even right on a table threshold.
        let elevation = s.elevation as f32;
        let moisture = s.moisture as f32;
        let temperature = s.temperature as f32;
        Tile { biome: classify(elevation, moisture, temperature), elevation, moisture, temperature, age: s.age as f32 }
    }

    /// The chunk at chunk coordinates `(cx, cy)`, covering tiles from [`chunk_origin`] onward. Valid for
    /// any chunk that [`chunk_of`](crate::chunk_of) can return.
    #[must_use]
    pub fn chunk(&self, cx: i64, cy: i64) -> Chunk {
        let (x0, y0) = chunk_origin(cx, cy);
        let mut tiles = Vec::with_capacity(CHUNK_AREA);
        for ly in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                tiles.push(self.tile(x0 + lx as i64, y0 + ly as i64));
            }
        }
        Chunk::new(cx, cy, tiles)
    }
}

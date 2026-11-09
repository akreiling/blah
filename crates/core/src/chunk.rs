//! Square chunks of tiles, the unit the world is generated and cached in.
//!
//! Tile coordinates are global `i64`s. A chunk coordinate is a tile coordinate floor-divided by
//! [`CHUNK_SIZE`], and a local coordinate is the remainder in `0..CHUNK_SIZE`. Floor division (not Rust's
//! truncating `/`) is what keeps tile -1 in chunk -1 rather than chunk 0.

use crate::world::Tile;

/// Tiles along one side of a chunk.
pub const CHUNK_SIZE: usize = 32;
/// Tiles in one chunk.
pub const CHUNK_AREA: usize = CHUNK_SIZE * CHUNK_SIZE;

const SIZE: i64 = CHUNK_SIZE as i64;

/// The chunk containing tile `(x, y)`.
#[must_use]
pub const fn chunk_of(x: i64, y: i64) -> (i64, i64) {
    (x.div_euclid(SIZE), y.div_euclid(SIZE))
}

/// The position of tile `(x, y)` within its chunk, each in `0..CHUNK_SIZE`.
#[must_use]
pub const fn local_of(x: i64, y: i64) -> (usize, usize) {
    (x.rem_euclid(SIZE) as usize, y.rem_euclid(SIZE) as usize)
}

/// The tile coordinates of chunk `(cx, cy)`'s local `(0, 0)` corner.
#[must_use]
pub const fn chunk_origin(cx: i64, cy: i64) -> (i64, i64) {
    (cx * SIZE, cy * SIZE)
}

/// A `CHUNK_SIZE`×`CHUNK_SIZE` block of tiles, stored row-major (`index = ly * CHUNK_SIZE + lx`).
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    cx: i64,
    cy: i64,
    tiles: Box<[Tile]>,
}

impl Chunk {
    pub(crate) fn new(cx: i64, cy: i64, tiles: Vec<Tile>) -> Chunk {
        assert_eq!(tiles.len(), CHUNK_AREA);
        Chunk { cx, cy, tiles: tiles.into_boxed_slice() }
    }

    /// This chunk's chunk coordinates.
    #[must_use]
    pub const fn coords(&self) -> (i64, i64) {
        (self.cx, self.cy)
    }

    /// All tiles, row-major.
    #[must_use]
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    /// The tile at local `(lx, ly)`. Panics if either is outside `0..CHUNK_SIZE`.
    #[must_use]
    pub fn local(&self, lx: usize, ly: usize) -> &Tile {
        assert!(lx < CHUNK_SIZE && ly < CHUNK_SIZE, "local ({lx}, {ly}) outside chunk");
        &self.tiles[ly * CHUNK_SIZE + lx]
    }

    /// The tile at global `(x, y)`, or `None` if it belongs to another chunk.
    #[must_use]
    pub fn get(&self, x: i64, y: i64) -> Option<&Tile> {
        if chunk_of(x, y) != (self.cx, self.cy) {
            return None;
        }
        let (lx, ly) = local_of(x, y);
        Some(self.local(lx, ly))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_division_for_negative_tiles() {
        assert_eq!(chunk_of(0, 31), (0, 0));
        assert_eq!(chunk_of(32, -1), (1, -1));
        assert_eq!(chunk_of(-32, -33), (-1, -2));
        assert_eq!(local_of(-1, -32), (31, 0));
        assert_eq!(local_of(-33, 33), (31, 1));
    }

    #[test]
    fn origin_round_trips() {
        for x in [-1_000_001, -65, -33, -32, -1, 0, 1, 31, 32, 999_999] {
            let (cx, cy) = chunk_of(x, -x);
            let (lx, ly) = local_of(x, -x);
            let (x0, y0) = chunk_origin(cx, cy);
            assert_eq!((x0 + lx as i64, y0 + ly as i64), (x, -x));
        }
    }
}

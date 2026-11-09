//! Properties of the public world API: determinism, chunk and tile agreement, value ranges, and whether
//! the generated geography is plausible as a whole.

use std::collections::HashSet;

use blah_core::{BIOMES, BiomeId, CHUNK_SIZE, Tile, World, chunk_of, chunk_origin, local_of};

const SIZE: i64 = CHUNK_SIZE as i64;

fn fields(t: &Tile) -> [f32; 4] {
    [t.elevation, t.moisture, t.temperature, t.age]
}

/// Tiles over a `span`×`span` square centered on `(cx, cy)`, every `stride` tiles.
fn sample(world: &World, (cx, cy): (i64, i64), span: i64, stride: usize) -> Vec<Tile> {
    let half = span / 2;
    let mut tiles = Vec::new();
    for y in (cy - half..cy + half).step_by(stride) {
        for x in (cx - half..cx + half).step_by(stride) {
            tiles.push(world.tile(x, y));
        }
    }
    tiles
}

fn water_share(tiles: &[Tile]) -> f64 {
    tiles.iter().filter(|t| t.biome.biome().water).count() as f64 / tiles.len() as f64
}

#[test]
fn same_seed_same_tiles() {
    let a = World::new(77);
    let b = World::new(77);
    for (x, y) in [(0, 0), (-1, -1), (12_345, -9_876), (-1_000_000, 1_000_000)] {
        assert_eq!(a.tile(x, y), b.tile(x, y));
    }
    assert_eq!(a.chunk(-3, 4), b.chunk(-3, 4));
}

#[test]
fn different_seed_different_tiles() {
    let a = sample(&World::new(1), (0, 0), 2_000, 50);
    let b = sample(&World::new(2), (0, 0), 2_000, 50);
    let differing = a.iter().zip(&b).filter(|(a, b)| a != b).count();
    assert_eq!(differing, a.len(), "every sampled tile should differ between seeds");
    let biome_differs = a.iter().zip(&b).filter(|(a, b)| a.biome != b.biome).count();
    assert!(biome_differs * 2 > a.len(), "only {biome_differs} of {} biomes differ", a.len());
}

#[test]
fn chunk_matches_tile() {
    let world = World::new(9);
    for (cx, cy) in [(0, 0), (-1, -1), (-1, 0), (0, -1), (3, -7), (31_250, -31_251)] {
        let chunk = world.chunk(cx, cy);
        assert_eq!(chunk.coords(), (cx, cy));
        let (x0, y0) = chunk_origin(cx, cy);
        for ly in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let (x, y) = (x0 + lx as i64, y0 + ly as i64);
                assert_eq!(*chunk.local(lx, ly), world.tile(x, y), "chunk ({cx}, {cy}) local ({lx}, {ly})");
                assert_eq!(chunk.get(x, y), Some(chunk.local(lx, ly)));
                assert_eq!(chunk_of(x, y), (cx, cy));
                assert_eq!(local_of(x, y), (lx, ly));
            }
        }
        assert_eq!(chunk.tiles().len(), CHUNK_SIZE * CHUNK_SIZE);
    }
}

#[test]
fn chunks_meet_at_borders() {
    // Tiles on both sides of the borders around the origin, where truncating division would go wrong.
    let world = World::new(9);
    for (x, y) in [(-1, -1), (0, 0), (-1, 0), (SIZE - 1, SIZE), (-SIZE, -SIZE - 1)] {
        let (cx, cy) = chunk_of(x, y);
        let chunk = world.chunk(cx, cy);
        assert_eq!(chunk.get(x, y), Some(&world.tile(x, y)), "tile ({x}, {y})");
        // The neighbor across each border is in a different chunk, and that chunk agrees too.
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            let other = world.chunk(chunk_of(nx, ny).0, chunk_of(nx, ny).1);
            assert_eq!(other.get(nx, ny), Some(&world.tile(nx, ny)));
        }
    }
    assert_eq!(chunk_of(-1, 0), (-1, 0));
    assert_eq!(world.chunk(0, 0).get(-1, 0), None);
}

#[test]
fn fields_are_in_unit_range() {
    for seed in [0, 1, u64::MAX] {
        let world = World::new(seed);
        for tile in sample(&world, (0, 0), 4_000, 23) {
            for v in fields(&tile) {
                assert!((0.0..=1.0).contains(&v), "{v} out of range in {tile:?}");
            }
        }
    }
}

#[test]
fn large_sample_has_every_biome_and_plausible_water() {
    let world = World::new(4);
    let tiles = sample(&world, (0, 0), 2_000, 8);
    let seen: HashSet<BiomeId> = tiles.iter().map(|t| t.biome).collect();
    for id in BiomeId::all() {
        assert!(seen.contains(&id), "{} never appears", id.biome().name);
    }
    let water = water_share(&tiles);
    assert!((0.25..=0.50).contains(&water), "water share {water:.3}");
}

#[test]
fn water_share_holds_across_seeds() {
    // A single 2000-tile window can land mostly on one ocean or one continent; averaged over seeds and a
    // wider area, the share should sit near sea level's calibration.
    let shares: Vec<f64> = (1..=8).map(|seed| water_share(&sample(&World::new(seed), (0, 0), 6_000, 40))).collect();
    let mean = shares.iter().sum::<f64>() / shares.len() as f64;
    assert!((0.30..=0.50).contains(&mean), "mean water share {mean:.3} over {shares:.3?}");
}

#[test]
fn far_coordinates_work() {
    let world = World::new(5);
    for (cx, cy) in [(1_000_000, 1_000_000), (-1_000_000, -1_000_000), (1_000_000, -1_000_000)] {
        // Neighboring tiles stay close in every field: no precision loss, no seams.
        for y in cy - 40..cy + 40 {
            for x in cx - 40..cx + 40 {
                let here = fields(&world.tile(x, y));
                for v in here {
                    assert!((0.0..=1.0).contains(&v));
                }
                for (nx, ny) in [(x + 1, y), (x, y + 1)] {
                    let there = fields(&world.tile(nx, ny));
                    for (a, b) in here.iter().zip(there) {
                        assert!((a - b).abs() < 0.05, "jump of {} between ({x}, {y}) and ({nx}, {ny})", (a - b).abs());
                    }
                }
            }
        }
        let (kx, ky) = chunk_of(cx, cy);
        assert_eq!(world.chunk(kx, ky).get(cx, cy), Some(&world.tile(cx, cy)));
    }
}

#[test]
fn extreme_coordinates_do_not_panic() {
    let world = World::new(3);
    for (x, y) in [(i64::MAX, i64::MIN), (i64::MIN, i64::MAX), (i64::MAX, i64::MAX)] {
        for v in fields(&world.tile(x, y)) {
            assert!((0.0..=1.0).contains(&v));
        }
    }
    let (cx, cy) = chunk_of(i64::MIN, i64::MAX);
    let _ = world.chunk(cx, cy);
}

#[test]
fn table_has_the_promised_shape() {
    assert!((12..=16).contains(&BIOMES.len()));
    let impassable: Vec<&str> = BIOMES.iter().filter(|b| !b.passable).map(|b| b.name).collect();
    assert_eq!(impassable, ["Deep water", "Snow peaks"]);
}

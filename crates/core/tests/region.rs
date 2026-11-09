//! Properties of regions through the public API: identity that doesn't depend on lookup order or caching,
//! what a region's biome promises, neighbor symmetry, names, and far coordinates.

use std::collections::{BTreeMap, BTreeSet};

use blah_core::{BiomeId, MAX_REGION_TILES, Region, RegionCache, RegionId, World};

/// A tiny deterministic shuffle (xorshift), so tests needn't pull in a random number crate.
fn shuffle<T>(items: &mut [T], mut state: u64) {
    for i in (1..items.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        items.swap(i, (state % (i as u64 + 1)) as usize);
    }
}

/// Every tile of a `side`×`side` square with top-left `(x0, y0)`, row-major.
fn square(x0: i64, y0: i64, side: i64) -> Vec<(i64, i64)> {
    (y0..y0 + side).flat_map(|y| (x0..x0 + side).map(move |x| (x, y))).collect()
}

/// The ids of every tile in `tiles`, looked up in the order given.
fn ids(cache: &mut RegionCache, tiles: &[(i64, i64)]) -> BTreeMap<(i64, i64), RegionId> {
    tiles.iter().map(|&(x, y)| ((x, y), cache.region_at(x, y))).collect()
}

/// Every tile of `region`, found by scanning its bounds.
fn tiles_of(cache: &mut RegionCache, region: &Region) -> Vec<(i64, i64)> {
    let b = region.bounds;
    let mut tiles = Vec::new();
    for y in b.min_y..=b.max_y {
        for x in b.min_x..=b.max_x {
            if cache.region_at(x, y) == region.id {
                tiles.push((x, y));
            }
        }
    }
    tiles
}

/// The distinct regions met in a square, in id order.
fn regions_in(cache: &mut RegionCache, x0: i64, y0: i64, side: i64, stride: usize) -> Vec<Region> {
    let mut seen = BTreeSet::new();
    for y in (y0..y0 + side).step_by(stride) {
        for x in (x0..x0 + side).step_by(stride) {
            seen.insert(cache.region_at(x, y));
        }
    }
    seen.into_iter().map(|id| cache.region(id).expect("region_at gives real regions")).collect()
}

#[test]
fn ids_do_not_depend_on_lookup_order_or_caching() {
    for (seed, x0, y0) in [(4, 2_200, 2_900), (42, -500, 300)] {
        let world = World::new(seed);
        // A block spanning several cells, so lookups cross cell edges in every direction.
        let tiles = square(x0, y0, 130);
        let forward = ids(&mut RegionCache::new(world), &tiles);

        let mut backward_order = tiles.clone();
        backward_order.reverse();
        // A small cache evicts and relabels as it goes.
        assert_eq!(ids(&mut RegionCache::with_capacity(world, 12), &backward_order), forward, "seed {seed} backward");

        let mut shuffled = tiles.clone();
        shuffle(&mut shuffled, seed + 1);
        assert_eq!(ids(&mut RegionCache::new(world), &shuffled), forward, "seed {seed} shuffled");

        // Without any cache, on a shuffled sample.
        for &(x, y) in shuffled.iter().step_by(401) {
            assert_eq!(world.region_at(x, y), forward[&(x, y)], "seed {seed} uncached ({x}, {y})");
        }
        assert!(forward.values().collect::<BTreeSet<_>>().len() > 3, "seed {seed}: block too uniform to test");
    }
}

#[test]
fn cached_descriptions_equal_uncached() {
    let world = World::new(7);
    let mut cache = RegionCache::with_capacity(world, 20);
    for region in regions_in(&mut RegionCache::new(world), 300, -900, 200, 23) {
        assert_eq!(cache.region(region.id).as_ref(), Some(&region));
        assert_eq!(world.region(region.id).as_ref(), Some(&region));
        assert_eq!(cache.region_neighbors(region.id), world.region_neighbors(region.id));
    }
}

#[test]
fn regions_describe_their_tiles() {
    for seed in [0, 4] {
        let world = World::new(seed);
        let mut cache = RegionCache::new(world);
        for region in regions_in(&mut cache, -1_000, 1_000, 400, 31) {
            let tiles = tiles_of(&mut cache, &region);
            let name = &region.name;
            assert_eq!(tiles.len() as u32, region.tiles, "{name}");
            assert!(region.tiles <= MAX_REGION_TILES, "{name}: {} tiles", region.tiles);
            assert!(tiles.contains(&region.id.anchor()), "{name}: anchor outside");
            assert!(tiles.contains(&region.representative), "{name}: representative outside");
            assert_eq!(world.tile(region.representative.0, region.representative.1).biome, region.biome, "{name}");

            let (min_x, max_x) = (tiles.iter().map(|t| t.0).min().unwrap(), tiles.iter().map(|t| t.0).max().unwrap());
            let (min_y, max_y) = (tiles.iter().map(|t| t.1).min().unwrap(), tiles.iter().map(|t| t.1).max().unwrap());
            assert_eq!((min_x, min_y, max_x, max_y), {
                let b = region.bounds;
                (b.min_x, b.min_y, b.max_x, b.max_y)
            });

            let mut by_biome: BTreeMap<BiomeId, usize> = BTreeMap::new();
            let mut mean_age = 0.0;
            for &(x, y) in &tiles {
                let tile = world.tile(x, y);
                // Water and land never share a region.
                assert_eq!(tile.biome.biome().water, region.biome.biome().water, "{name} at ({x}, {y})");
                *by_biome.entry(tile.biome).or_default() += 1;
                mean_age += f64::from(tile.age) / tiles.len() as f64;
            }
            // The region's biome is what most of it is. Not guaranteed (several strays of one other biome
            // could outweigh a small core), but cores of at least 800 tiles and strays under 400 make it so in
            // practice; this catches a rule change that makes it common.
            let most = by_biome.iter().max_by_key(|(_, n)| **n).unwrap();
            assert_eq!(by_biome[&region.biome], *most.1, "{name}: {by_biome:?}");
            assert!((mean_age - f64::from(region.mean.age)).abs() < 1e-4, "{name}");
        }
    }
}

#[test]
fn neighbors_are_symmetric_and_real() {
    let world = World::new(42);
    let mut cache = RegionCache::new(world);
    for region in regions_in(&mut cache, 0, 0, 300, 29) {
        let neighbors = cache.region_neighbors(region.id);
        assert!(!neighbors.is_empty());
        assert!(neighbors.windows(2).all(|w| w[0] < w[1]), "not sorted and unique: {neighbors:?}");
        assert!(!neighbors.contains(&region.id));
        for &other in &neighbors {
            assert!(cache.region_neighbors(other).contains(&region.id), "{other:?} doesn't list {:?}", region.id);
        }
        // Exactly the regions across the region's edges.
        let mut across = BTreeSet::new();
        for (x, y) in tiles_of(&mut cache, &region) {
            for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                let id = cache.region_at(nx, ny);
                if id != region.id {
                    across.insert(id);
                }
            }
        }
        assert_eq!(neighbors, across.into_iter().collect::<Vec<_>>());
    }
}

#[test]
fn names_are_deterministic_and_varied() {
    let world = World::new(9);
    let regions = regions_in(&mut RegionCache::new(world), -500, -500, 1_000, 13);
    let mut names = BTreeSet::new();
    for (i, region) in regions.iter().enumerate() {
        // Same seed and id, fresh everything: same name.
        if i % 16 == 0 {
            assert_eq!(World::new(9).region(region.id).unwrap().name, region.name);
        }
        let name = &region.name;
        assert!(name.len() <= 30, "{name:?} is too long for the panel");
        let words: Vec<&str> = name.split(' ').collect();
        for (i, word) in words.iter().enumerate() {
            let small = (i == 0 && *word == "the") || *word == "of";
            assert!(small || word.starts_with(|c: char| c.is_ascii_uppercase()), "{name:?}");
            assert!(word.chars().all(|c| c.is_ascii_alphabetic()), "{name:?}");
        }
        names.insert(name.clone());
    }
    assert!(regions.len() > 200, "only {} regions", regions.len());
    assert!(names.len() * 10 >= regions.len() * 9, "{} names for {} regions", names.len(), regions.len());
}

#[test]
fn far_coordinates_work() {
    let world = World::new(5);
    let mut cache = RegionCache::new(world);
    for (x, y) in [(1_000_000, 1_000_000), (-1_000_000, -1_000_000), (1_000_000, -1_000_000), (-999_983, 999_979)] {
        let id = cache.region_at(x, y);
        let region = cache.region(id).unwrap();
        assert!(region.bounds.contains(x, y));
        let (ax, ay) = id.anchor();
        assert_eq!(cache.region_at(ax, ay), id);
        assert_eq!(world.region_at(x, y), id);
        for other in cache.region_neighbors(id) {
            assert!(cache.region_neighbors(other).contains(&id));
        }
    }
}

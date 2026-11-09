//! Measures region lookups and prints sample region names, for tuning and for checking that lookups stay
//! cheap enough to run on every step.
//!
//! ```text
//! cargo run --release --example regions -- [seed]
//! ```

use std::time::{Duration, Instant};

use blah_core::{Dir, Game, RegionCache, World};

fn main() {
    let seed: u64 = std::env::args().nth(1).map_or(42, |s| s.parse().expect("seed must be a u64"));
    let world = World::new(seed);

    // Cold: a fresh cache per lookup, at spots far enough apart to share nothing.
    let spots: Vec<(i64, i64)> = (0..40).map(|i| (i * 1_000 - 20_000, (i * 7_919) % 30_000 - 15_000)).collect();
    let mut at = Vec::new();
    let mut describe = Vec::new();
    let mut neighbors = Vec::new();
    let mut again = Vec::new();
    let mut cells = [0; 3];
    for &(x, y) in &spots {
        let mut cache = RegionCache::new(world);
        let t = Instant::now();
        let id = cache.region_at(x, y);
        at.push(t.elapsed());
        cells[0] += cache.len();
        let t = Instant::now();
        let region = cache.region(id).expect("region_at gives a real region");
        describe.push(t.elapsed());
        cells[1] += cache.len();
        let t = Instant::now();
        let n = cache.region_neighbors(region.id).len();
        neighbors.push(t.elapsed());
        cells[2] += cache.len();
        assert!(n > 0);
        let t = Instant::now();
        std::hint::black_box((cache.region(id), cache.region_neighbors(id)));
        again.push(t.elapsed());
    }
    // What labeling a cell mostly costs: sampling its tiles.
    let t = Instant::now();
    let n = 200_000;
    for i in 0..n {
        std::hint::black_box(world.tile(i % 1_000, i / 1_000));
    }
    println!("World::tile: {:.0?} per tile", t.elapsed() / n as u32);
    report("cold region_at", &mut at);
    report("then region (same cache)", &mut describe);
    report("then region_neighbors (same cache)", &mut neighbors);
    report("then both again (warm)", &mut again);
    let per = cells.map(|c| c as f64 / spots.len() as f64);
    println!(
        "cells labeled, on average: {:.1} for region_at, {:.1} after region, {:.1} after neighbors",
        per[0], per[1], per[2]
    );

    // Warm: every tile of a viewport-sized square around one spot, after the first lookup filled the cache.
    let mut cache = RegionCache::new(world);
    let _ = cache.region_at(0, 0);
    let mut warm = Vec::new();
    for y in -8..=8 {
        for x in -8..=8 {
            let t = Instant::now();
            std::hint::black_box(cache.region_at(x, y));
            warm.push(t.elapsed());
        }
    }
    report("warm region_at", &mut warm);

    // A walk: what a game step costs, region lookups and messages included.
    let mut game = Game::new(seed);
    let mut steps = Vec::new();
    for i in 0..20_000usize {
        let dir = Dir::ALL[(i / 60 + i / 7 % 2) % 4];
        let t = Instant::now();
        game.step(dir);
        steps.push(t.elapsed());
    }
    report("Game::step over a 20,000-step walk", &mut steps);

    // Names: the first few regions of each biome met on a coarse grid over a wide area.
    println!("\nsample names:");
    let mut cache = RegionCache::new(world);
    let mut per_biome = std::collections::BTreeMap::<_, Vec<_>>::new();
    for y in (-20_000..20_000).step_by(1_733) {
        for x in (-20_000..20_000).step_by(1_511) {
            let id = cache.region_at(x, y);
            let region = cache.region(id).expect("real region");
            let names = per_biome.entry(region.biome).or_default();
            if names.len() < 2 {
                names.push(region);
            }
        }
    }
    for region in per_biome.values().flatten() {
        println!("  {:<30} {:<14} {:>5} tiles", region.name, region.biome.biome().name, region.tiles);
    }
}

fn report(what: &str, times: &mut [Duration]) {
    times.sort_unstable();
    let n = times.len();
    let mean = times.iter().sum::<Duration>() / n as u32;
    println!(
        "{what:<40} n={n:<6} mean {mean:>10.2?}  median {:>10.2?}  p99 {:>10.2?}  max {:>10.2?}",
        times[n / 2],
        times[(n - 1) * 99 / 100],
        times[n - 1]
    );
}

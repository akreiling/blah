//! Prints a region of the world as a colored glyph map, for eyeballing the terrain while tuning.
//!
//! ```text
//! cargo run --example ascii_map -- [seed] [x] [y] [width] [height] [step] [--plain]
//! ```
//!
//! `(x, y)` is the top-left tile. `step` samples every n-th tile, to see continents at a glance.
//! `--plain` drops the ANSI colors. With no arguments it shows a varied stretch of coast and mountains.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::process::ExitCode;

use blah_core::{BiomeId, World};

struct Args {
    seed: u64,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    step: i64,
    plain: bool,
}

fn parse() -> Result<Args, String> {
    let mut plain = false;
    let mut numbers = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--plain" => plain = true,
            _ => numbers.push(arg.parse::<i64>().map_err(|e| format!("{arg:?}: {e}"))?),
        }
    }
    // Seed 4's origin is open sea; this spot runs from coast through forest to snow peaks.
    let defaults = [4, 2_220, 2_930, 100, 40, 1];
    if numbers.len() > defaults.len() {
        return Err("too many arguments".into());
    }
    let n = |i: usize| numbers.get(i).copied().unwrap_or(defaults[i]);
    let args = Args { seed: n(0) as u64, x: n(1), y: n(2), width: n(3), height: n(4), step: n(5), plain };
    if args.width < 1 || args.height < 1 || args.step < 1 {
        return Err("width, height, and step must be positive".into());
    }
    Ok(args)
}

fn main() -> ExitCode {
    let args = match parse() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("ascii_map: {e}\nusage: ascii_map [seed] [x] [y] [width] [height] [step] [--plain]");
            return ExitCode::FAILURE;
        }
    };
    let world = World::new(args.seed);
    let mut counts: BTreeMap<BiomeId, usize> = BTreeMap::new();
    let mut out = String::new();
    for row in 0..args.height {
        for col in 0..args.width {
            let tile = world.tile(args.x + col * args.step, args.y + row * args.step);
            *counts.entry(tile.biome).or_default() += 1;
            let biome = tile.biome.biome();
            if args.plain {
                out.push(biome.glyph);
            } else {
                // The biome color fills the cell; the glyph is a darker shade of it, so texture shows
                // without fighting the color.
                let [r, g, b] = biome.color;
                let [gr, gg, gb] = biome.color.map(|c| c / 2);
                let _ = write!(out, "\x1b[48;2;{r};{g};{b}m\x1b[38;2;{gr};{gg};{gb}m{}", biome.glyph);
            }
        }
        if !args.plain {
            out.push_str("\x1b[0m");
        }
        out.push('\n');
    }
    let total = (args.width * args.height) as f64;
    for (id, count) in &counts {
        let biome = id.biome();
        let _ = writeln!(out, "{} {:<14} {:5.1}%", biome.glyph, biome.name, 100.0 * *count as f64 / total);
    }
    print!("{out}");
    ExitCode::SUCCESS
}

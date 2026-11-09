//! Writes a contact sheet of every clade evolved in a spread of biomes, for judging the procedural sprites,
//! and measures what growing a region's life costs.
//!
//! ```text
//! cargo run --release -p blah-core --example bestiary_sheet -- out.png [seed]
//! ```
//!
//! Rows are clades and columns are biomes. Each cell is the species that clade becomes in a real region of
//! that biome, scaled ×4 on the biome's map color, with three individuals drawn from its pool below it. A
//! light corner mark means the clade actually settles that region; unmarked cells show what it would become.

use std::io::BufWriter;
use std::time::Instant;

use blah_core::{
    BiomeId, CLADES, GENERATIONS, Phenotype, Region, RegionCache, RegionLife, Rng, SPRITE_SIZE, Sprite, World, clades,
    grow_pool,
};

/// Biomes shown, as columns, hot and dry to cold and wet.
const COLUMNS: [&str; 10] =
    ["Desert", "Badlands", "Savanna", "Jungle", "Swamp", "Grassland", "Forest", "Taiga", "Tundra", "Shallow water"];

const SCALE: usize = 4;
const SMALL: usize = 2;
const CELL_W: usize = SPRITE_SIZE * SCALE + 16;
const CELL_H: usize = SPRITE_SIZE * SCALE + SPRITE_SIZE * SMALL + 20;
const LABEL_W: usize = 120;
const HEADER_H: usize = 28;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().expect("usage: bestiary_sheet <out.png> [seed]");
    let seed: u64 = args.next().map_or(42, |s| s.parse().expect("seed must be a u64"));
    let world = World::new(seed);
    let clades = clades(seed);

    let regions = find_regions(&world);
    let width = LABEL_W + CELL_W * COLUMNS.len();
    let height = HEADER_H + CELL_H * CLADES;
    let mut img = Image::new(width, height, [16, 17, 22]);

    for (col, (name, region)) in COLUMNS.iter().zip(&regions).enumerate() {
        let x0 = LABEL_W + col * CELL_W;
        img.text(x0 + 4, 8, &name.to_uppercase(), 2, [220, 220, 200]);
        let life = RegionLife::grow(seed, &clades, region);
        println!("{} ({}):", region.name, name);
        for (row, clade) in clades.iter().enumerate() {
            let y0 = HEADER_H + row * CELL_H;
            img.rect(x0 + 2, y0 + 2, CELL_W - 4, CELL_H - 4, region.biome.biome().color);
            let resident = life.species_of(clade.id).is_some();
            if resident {
                img.rect(x0 + 2, y0 + 2, 6, 6, [255, 255, 255]);
            }
            let pool = grow_pool(seed, clade, region.biome, region.id, GENERATIONS);
            let species = blah_core::Species::from_pool(seed, clade, region, pool.clone());
            img.sprite(&species.sprite, x0 + 8, y0 + 6, SCALE);
            // Individuals: pool members with the same small variation the game gives a spawn.
            let mut rng = Rng::new(row as u64 * 31 + col as u64);
            for k in 0..3 {
                let genome = pool[rng.below(pool.len())].mutate(&mut rng, 0.5, 0.03);
                let s = Sprite::draw(&Phenotype::of(&genome), &clade.plan, region.biome);
                img.sprite(&s, x0 + 8 + k * (SPRITE_SIZE * SMALL + 2), y0 + 10 + SPRITE_SIZE * SCALE, SMALL);
            }
            println!("  {}{}: {}", species.name, if resident { "" } else { " (absent)" }, species.description);
        }
    }
    for (row, clade) in clades.iter().enumerate() {
        let y0 = HEADER_H + row * CELL_H;
        img.text(6, y0 + CELL_H / 2 - 12, &clade.root.to_uppercase(), 2, [240, 192, 74]);
        img.text(6, y0 + CELL_H / 2 + 2, &clade.plan.family.noun().to_uppercase(), 2, [140, 140, 150]);
    }
    img.save(&out);
    println!("wrote {out}");

    measure(seed, &world);
}

/// One real region per column's biome, found by walking outward from the origin. A biome with no region
/// in reach borrows the nearest region found and only swaps its biome, so the sheet is always complete.
fn find_regions(world: &World) -> Vec<Region> {
    let mut cache = RegionCache::new(*world);
    let mut found: Vec<Option<Region>> = vec![None; COLUMNS.len()];
    let mut first = None;
    'search: for r in 0..60i64 {
        for (dx, dy) in ring(r) {
            let (x, y) = (dx * 97, dy * 97);
            let biome = world.tile(x, y).biome.biome().name;
            let Some(col) = COLUMNS.iter().position(|&c| c == biome) else { continue };
            if found[col].is_some() {
                continue;
            }
            let id = cache.region_at(x, y);
            let region = cache.region(id).expect("real region");
            if region.biome.biome().name == biome {
                first.get_or_insert_with(|| region.clone());
                found[col] = Some(region);
            }
            if found.iter().all(Option::is_some) {
                break 'search;
            }
        }
    }
    let fallback = first.expect("some region found");
    found
        .into_iter()
        .zip(COLUMNS)
        .map(|(r, name)| {
            r.unwrap_or_else(|| {
                let biome = BiomeId::all().find(|b| b.biome().name == name).unwrap();
                Region { biome, ..fallback.clone() }
            })
        })
        .collect()
}

fn ring(r: i64) -> impl Iterator<Item = (i64, i64)> {
    (-r..=r).flat_map(move |dy| (-r..=r).filter(move |dx| dx.abs() == r || dy.abs() == r).map(move |dx| (dx, dy)))
}

/// Times growing the life of real regions, which happens when one is first entered.
fn measure(seed: u64, world: &World) {
    let clades = clades(seed);
    let mut cache = RegionCache::new(*world);
    let regions: Vec<Region> = (0..40)
        .map(|i| {
            let id = cache.region_at(i * 211 - 4_000, (i * 7_919) % 6_000 - 3_000);
            cache.region(id).unwrap()
        })
        .collect();
    let t = Instant::now();
    let mut species = 0;
    for region in &regions {
        species += std::hint::black_box(RegionLife::grow(seed, &clades, region)).species.len();
    }
    let per_region = t.elapsed() / regions.len() as u32;
    let t = Instant::now();
    for region in &regions {
        std::hint::black_box(grow_pool(seed, &clades[0], region.biome, region.id, GENERATIONS));
    }
    let per_pool = t.elapsed() / regions.len() as u32;
    println!(
        "growing a region's life: {per_region:?} per region ({:.1} species each), {per_pool:?} per pool",
        species as f32 / regions.len() as f32
    );
}

/// An RGB image being composed.
struct Image {
    width: usize,
    height: usize,
    pixels: Vec<[u8; 3]>,
}

impl Image {
    fn new(width: usize, height: usize, bg: [u8; 3]) -> Image {
        Image { width, height, pixels: vec![bg; width * height] }
    }

    fn set(&mut self, x: usize, y: usize, c: [u8; 3]) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = c;
        }
    }

    fn rect(&mut self, x0: usize, y0: usize, w: usize, h: usize, c: [u8; 3]) {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                self.set(x, y, c);
            }
        }
    }

    fn sprite(&mut self, s: &Sprite, x0: usize, y0: usize, scale: usize) {
        for y in 0..SPRITE_SIZE {
            for x in 0..SPRITE_SIZE {
                let [r, g, b, a] = s.pixel(x, y);
                if a > 0 {
                    self.rect(x0 + x * scale, y0 + y * scale, scale, scale, [r, g, b]);
                }
            }
        }
    }

    fn text(&mut self, x0: usize, y0: usize, text: &str, scale: usize, c: [u8; 3]) {
        let mut x = x0;
        for ch in text.chars() {
            let glyph = glyph(ch);
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..3 {
                    if bits >> (2 - col) & 1 == 1 {
                        self.rect(x + col * scale, y0 + row * scale, scale, scale, c);
                    }
                }
            }
            x += 4 * scale;
        }
    }

    fn save(&self, path: &str) {
        let file = std::fs::File::create(path).expect("create output file");
        let mut encoder = png::Encoder::new(BufWriter::new(file), self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("png header");
        let data: Vec<u8> = self.pixels.iter().flatten().copied().collect();
        writer.write_image_data(&data).expect("png data");
    }
}

/// A 3×5 font: each row is three bits, high bit on the left.
fn glyph(c: char) -> [u8; 5] {
    match c {
        'A' => [2, 5, 7, 5, 5],
        'B' => [6, 5, 6, 5, 6],
        'C' => [3, 4, 4, 4, 3],
        'D' => [6, 5, 5, 5, 6],
        'E' => [7, 4, 6, 4, 7],
        'F' => [7, 4, 6, 4, 4],
        'G' => [3, 4, 5, 5, 3],
        'H' => [5, 5, 7, 5, 5],
        'I' => [7, 2, 2, 2, 7],
        'J' => [1, 1, 1, 5, 2],
        'K' => [5, 5, 6, 5, 5],
        'L' => [4, 4, 4, 4, 7],
        'M' => [5, 7, 7, 5, 5],
        'N' => [6, 5, 5, 5, 5],
        'O' => [2, 5, 5, 5, 2],
        'P' => [6, 5, 6, 4, 4],
        'Q' => [2, 5, 5, 6, 3],
        'R' => [6, 5, 6, 5, 5],
        'S' => [3, 4, 2, 1, 6],
        'T' => [7, 2, 2, 2, 2],
        'U' => [5, 5, 5, 5, 7],
        'V' => [5, 5, 5, 5, 2],
        'W' => [5, 5, 7, 7, 5],
        'X' => [5, 5, 2, 5, 5],
        'Y' => [5, 5, 2, 2, 2],
        'Z' => [7, 1, 2, 4, 7],
        _ => [0; 5],
    }
}

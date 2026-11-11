//! Embedded 32×32 terrain and object artwork.
//!
//! Terrain comes from corner-matched (Wang) tilesets. Each map cell is made of four quarters, and each
//! quarter comes from the tile whose four corners are the terrains of the 2×2 block of cells meeting at that
//! corner of the cell, so a single cell of one terrain still shows. Every world pixel then samples that layout
//! through smooth world-space noise: textures stop repeating, boundaries wander off the cell grid, and since
//! the warp depends only on world position, neighboring cells agree on every shared edge. Visibility stays per
//! cell. Images are enlarged by integer nearest-neighbor duplication for the display scale and cached only
//! while the visible map uses them.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{RenderImage, Window};
use image::{Frame, RgbaImage};
use isle_core::{Item, Tile};

use blah_core::Biome;

const NATIVE_SIDE: usize = 32;
const QUARTER: usize = NATIVE_SIDE / 2;
const DISPLAY_SIDE: f32 = 32.0;
const BIOME_TINT_PERCENT: u16 = 18;
const CAVE_TINT: [u8; 3] = [76, 150, 101];
const CAVE_TINT_PERCENT: u16 = 22;

/// A ground surface drawn from a tileset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Terrain {
    Water,
    Grass,
    Dirt,
    Sand,
    Rock,
    Floor,
    Wall,
}

/// One embedded tileset joining terrains `a` and `b`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TileSet {
    Shore,
    Path,
    Rocky,
    Dungeon,
    Beach,
    Dunes,
    Highland,
}

pub const ALL_SETS: [TileSet; 7] = [
    TileSet::Shore,
    TileSet::Path,
    TileSet::Rocky,
    TileSet::Dungeon,
    TileSet::Beach,
    TileSet::Dunes,
    TileSet::Highland,
];

impl TileSet {
    /// The two terrains, matching the manifest the art tool writes.
    pub fn terrains(self) -> (Terrain, Terrain) {
        match self {
            TileSet::Shore => (Terrain::Water, Terrain::Grass),
            TileSet::Path => (Terrain::Dirt, Terrain::Grass),
            TileSet::Rocky => (Terrain::Sand, Terrain::Rock),
            TileSet::Dungeon => (Terrain::Floor, Terrain::Wall),
            TileSet::Beach => (Terrain::Water, Terrain::Sand),
            TileSet::Dunes => (Terrain::Sand, Terrain::Grass),
            TileSet::Highland => (Terrain::Grass, Terrain::Rock),
        }
    }

    #[cfg(test)]
    fn name(self) -> &'static str {
        match self {
            TileSet::Shore => "shore",
            TileSet::Path => "path",
            TileSet::Rocky => "rocky",
            TileSet::Dungeon => "dungeon",
            TileSet::Beach => "beach",
            TileSet::Dunes => "dunes",
            TileSet::Highland => "highland",
        }
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            TileSet::Shore => include_bytes!("../assets/world/terrain_shore.png"),
            TileSet::Path => include_bytes!("../assets/world/terrain_path.png"),
            TileSet::Rocky => include_bytes!("../assets/world/terrain_rocky.png"),
            TileSet::Dungeon => include_bytes!("../assets/world/terrain_dungeon.png"),
            TileSet::Beach => include_bytes!("../assets/world/terrain_beach.png"),
            TileSet::Dunes => include_bytes!("../assets/world/terrain_dunes.png"),
            TileSet::Highland => include_bytes!("../assets/world/terrain_highland.png"),
        }
    }
}

impl Terrain {
    /// The set whose full tile draws this terrain alone, and whether it is that set's `b` side.
    fn home(self) -> (TileSet, bool) {
        match self {
            Terrain::Water => (TileSet::Shore, false),
            Terrain::Grass => (TileSet::Shore, true),
            Terrain::Dirt => (TileSet::Path, false),
            Terrain::Sand => (TileSet::Rocky, false),
            Terrain::Rock => (TileSet::Rocky, true),
            Terrain::Floor => (TileSet::Dungeon, false),
            Terrain::Wall => (TileSet::Dungeon, true),
        }
    }
}

/// A set joining two terrains, if one exists, and whether `first` is its `b` side.
fn joining(first: Terrain, second: Terrain) -> Option<(TileSet, bool)> {
    ALL_SETS.into_iter().find_map(|set| match set.terrains() {
        (a, b) if a == first && b == second => Some((set, false)),
        (a, b) if b == first && a == second => Some((set, true)),
        _ => None,
    })
}

/// Objects drawn over terrain, in the embedded sheet's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Object {
    Pine,
    Oak,
    Boulder,
    Door,
    StairsUp,
    StairsDown,
    Counter,
    Altar,
    Coins,
    Arrows,
    Potion,
    Weapon,
    Bow,
    Armor,
    Ring,
    Treasure,
}

pub const ALL_OBJECTS: [Object; 16] = [
    Object::Pine,
    Object::Oak,
    Object::Boulder,
    Object::Door,
    Object::StairsUp,
    Object::StairsDown,
    Object::Counter,
    Object::Altar,
    Object::Coins,
    Object::Arrows,
    Object::Potion,
    Object::Weapon,
    Object::Bow,
    Object::Armor,
    Object::Ring,
    Object::Treasure,
];

const OBJECTS: &[u8] = include_bytes!("../assets/world/objects.png");

impl Object {
    fn index(self) -> usize {
        ALL_OBJECTS.iter().position(|&o| o == self).expect("every object is in the sheet")
    }
}

/// What a cell shows, before its neighbors shape the ground's edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainArt {
    pub ground: Terrain,
    /// A transparent object placed over the ground, if any.
    pub object: Option<Object>,
    /// A gentle biome-color wash applied to water, if any.
    pub tint: Option<[u8; 3]>,
    /// Whether a natural object is drawn mirrored, so repeated trees and boulders vary.
    pub mirrored: bool,
}

/// Natural ground from biome traits, never its name or table index.
fn natural_ground(biome: &Biome) -> Terrain {
    if biome.water {
        Terrain::Water
    } else if !biome.passable || biome.temperature.max <= 0.40 {
        Terrain::Rock
    } else if biome.productivity >= 0.45 {
        Terrain::Grass
    } else {
        Terrain::Sand
    }
}

fn mix(pos: (i32, i32)) -> u32 {
    ((pos.0 as u32).wrapping_mul(0x9e37_79b9) ^ (pos.1 as u32).wrapping_mul(0x85eb_ca6b)).rotate_left(13)
}

/// A stable per-cell choice between two tree shapes, so woods are not uniform.
fn tree_at(pos: (i32, i32)) -> Object {
    if mix(pos) & 4 == 0 { Object::Pine } else { Object::Oak }
}

/// Art layers for a tile. `deep` levels use the dungeon set; towns pave their floors with the dirt path and build
/// their walls of gray stone.
pub fn terrain_art(tile: Tile, biome: &Biome, deep: bool, pos: (i32, i32)) -> TerrainArt {
    let floor = if deep { Terrain::Floor } else { Terrain::Dirt };
    let natural = natural_ground(biome);
    let (ground, object) = match tile {
        Tile::Wall => (if deep { Terrain::Wall } else { Terrain::Rock }, None),
        Tile::Floor => (floor, None),
        Tile::Grass => (Terrain::Grass, None),
        Tile::Water => (Terrain::Water, None),
        Tile::Tree => (if natural == Terrain::Water { Terrain::Grass } else { natural }, Some(tree_at(pos))),
        Tile::Door => (floor, Some(Object::Door)),
        Tile::StairsUp => (floor, Some(Object::StairsUp)),
        Tile::StairsDown => (floor, Some(Object::StairsDown)),
        Tile::Counter => (floor, Some(Object::Counter)),
        Tile::Altar => (floor, Some(Object::Altar)),
        Tile::Wild(_) => (natural, None),
        // Impassable rock is a terrain; a passable biome's scattered rocks are boulders on its ground.
        Tile::Rock if !biome.passable => (Terrain::Rock, None),
        Tile::Rock => (if natural == Terrain::Water { Terrain::Grass } else { natural }, Some(Object::Boulder)),
    };
    // Only water keeps its biome's color, which separates deep from shallow; elsewhere a per-biome wash makes
    // patchy blocks where neighboring biomes share a texture.
    let tint = (ground == Terrain::Water).then_some(biome.color);
    let mirrored = matches!(object, Some(Object::Pine | Object::Oak | Object::Boulder)) && mix(pos) & 16 != 0;
    TerrainArt { ground, object, tint, mirrored }
}

/// The ground-pile icon. Mixed piles use a single treasure mark, and empty piles have no icon.
pub fn item_art(items: &[Item]) -> Option<Object> {
    if items.len() > 1 {
        return Some(Object::Treasure);
    }
    match items.first()? {
        Item::Gold(_) => Some(Object::Coins),
        Item::Arrows(_) => Some(Object::Arrows),
        Item::Potion => Some(Object::Potion),
        Item::Weapon(_) => Some(Object::Weapon),
        Item::Bow(_) => Some(Object::Bow),
        Item::Armor(_) => Some(Object::Armor),
        Item::Ring => Some(Object::Ring),
    }
}

/// One quarter of a cell: a tile chosen by corner mask (NW = 8, NE = 4, SW = 2, SE = 1, set where terrain `b`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Quarter {
    pub set: TileSet,
    pub mask: u8,
}

/// The four quarters (NW, NE, SW, SE) of a cell whose ground is `around[1][1]`. `around` is the 3×3 block of
/// grounds centered on the cell, rows north to south; callers pass the center's own ground for cells the player
/// does not know, so edges never reveal unseen terrain.
pub fn ground_quarters(around: [[Terrain; 3]; 3]) -> [Quarter; 4] {
    let center = around[1][1];
    [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(qx, qy)| {
        // The 2×2 block meeting at this corner of the cell, as tile corners NW, NE, SW, SE.
        let block = [around[qy][qx], around[qy][qx + 1], around[qy + 1][qx], around[qy + 1][qx + 1]];
        let mut partners: Vec<(usize, Terrain)> = Vec::new();
        for &t in block.iter().filter(|&&t| t != center) {
            match partners.iter_mut().find(|(_, p)| *p == t) {
                Some((count, _)) => *count += 1,
                None => partners.push((1, t)),
            }
        }
        partners.sort_by_key(|x| std::cmp::Reverse(x.0));
        let joined = partners.iter().find_map(|&(_, partner)| joining(center, partner).map(|j| (j, partner)));
        match joined {
            Some(((set, center_is_b), partner)) => {
                let mask = block.iter().enumerate().fold(0u8, |mask, (i, &t)| {
                    let is_b = if t == partner { !center_is_b } else { center_is_b };
                    mask | if is_b { 8 >> i } else { 0 }
                });
                Quarter { set, mask }
            }
            None => {
                let (set, is_b) = center.home();
                Quarter { set, mask: if is_b { 15 } else { 0 } }
            }
        }
    })
}

/// The 5×5 block of grounds centered on a cell, rows north to south, and the 3×3 block of water tints. Callers
/// pass the center's own ground and tint for cells the player does not know, so edges never reveal unexplored
/// terrain. A warped sample reaches at most one cell away, and that cell needs its own 3×3 to pick quarters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GroundCell {
    pub pos: (i32, i32),
    pub around: [[Terrain; 5]; 5],
    pub tints: [[Option<[u8; 3]>; 3]; 3],
}

/// Layers of the world-space warp as (period, amplitude) in source pixels. The amplitudes sum to well under a
/// cell, so a warped sample never reaches past a neighboring cell.
const WARP: [(f32, f32); 2] = [(24.0, 5.0), (9.0, 2.0)];
#[cfg(test)]
const WARP_REACH: i64 = 7;
const SHADE_PERIOD: f32 = 72.0;
const SHADE_PERCENT: f32 = 5.0;

fn lattice(seed: u64, channel: u64, x: i64, y: i64) -> f32 {
    let mut h = seed
        ^ channel.wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (x as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9)
        ^ (y as u64).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^= h >> 30;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^= h >> 31;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// Smooth value noise in [-1, 1].
fn value_noise(seed: u64, channel: u64, x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (smooth(x - x0), smooth(y - y0));
    let (ix, iy) = (x0 as i64, y0 as i64);
    let corner = |dx, dy| lattice(seed, channel, ix + dx, iy + dy);
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * sx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * sx;
    (top + (bottom - top) * sy) * 2.0 - 1.0
}

/// The whole-pixel offset at which world pixel `p` samples the terrain. It depends only on world position, so
/// neighboring cells agree on every shared edge.
fn warp(seed: u64, p: (i64, i64)) -> (i64, i64) {
    let mut offset = (0.0, 0.0);
    for (i, &(period, amplitude)) in WARP.iter().enumerate() {
        let (x, y) = (p.0 as f32 / period, p.1 as f32 / period);
        offset.0 += value_noise(seed, 2 * i as u64, x, y) * amplitude;
        offset.1 += value_noise(seed, 2 * i as u64 + 1, x, y) * amplitude;
    }
    (offset.0.round() as i64, offset.1.round() as i64)
}

/// A slow brightness drift across wide fields, in percent.
fn shade(seed: u64, p: (i64, i64)) -> u16 {
    let n = value_noise(seed, 9, p.0 as f32 / SHADE_PERIOD, p.1 as f32 / SHADE_PERIOD);
    (100.0 + n * SHADE_PERCENT).round() as u16
}

fn shaded(mut pixel: [u8; 4], percent: u16) -> [u8; 4] {
    for channel in &mut pixel[..3] {
        *channel = (u16::from(*channel) * percent / 100).min(255) as u8;
    }
    pixel
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Source {
    Ground(GroundCell),
    Object { object: Object, mirrored: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ImageKey {
    source: Source,
    remembered: bool,
    lit: bool,
}

/// Decoded sheets plus the scale- and visibility-bounded images used by the current frame.
pub struct ArtCache {
    seed: u64,
    sets: HashMap<TileSet, RgbaImage>,
    objects: RgbaImage,
    images: HashMap<ImageKey, Arc<RenderImage>>,
    used: HashSet<ImageKey>,
    scale: f32,
    factor: usize,
    device_side: usize,
}
fn decode(bytes: &[u8]) -> RgbaImage {
    let image = image::load_from_memory(bytes).expect("embedded world art must be a PNG").to_rgba8();
    assert_eq!(
        image.dimensions(),
        (4 * NATIVE_SIDE as u32, 4 * NATIVE_SIDE as u32),
        "embedded world art must be a 4×4 sheet of 32-pixel tiles"
    );
    image
}

impl ArtCache {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            sets: ALL_SETS.into_iter().map(|set| (set, decode(set.bytes()))).collect(),
            objects: decode(OBJECTS),
            images: HashMap::new(),
            used: HashSet::new(),
            scale: 1.0,
            factor: 1,
            device_side: DISPLAY_SIDE as usize,
        }
    }

    /// Drop stale GPU images at a display-scale transition and begin tracking this frame's artwork.
    pub fn begin(&mut self, window: &mut Window) {
        for image in self.begin_scale(window.scale_factor()) {
            window.drop_image(image).ok();
        }
    }

    fn begin_scale(&mut self, scale: f32) -> Vec<Arc<RenderImage>> {
        let scale = scale.max(0.5);
        let device_side = (DISPLAY_SIDE * scale).round().max(1.0) as usize;
        let factor = (device_side / NATIVE_SIDE).max(1);
        let stale =
            if self.scale.to_bits() != scale.to_bits() || self.factor != factor || self.device_side != device_side {
                self.images.drain().map(|(_, image)| image).collect()
            } else {
                Vec::new()
            };
        self.scale = scale;
        self.factor = factor;
        self.device_side = device_side;
        self.used.clear();
        stale
    }

    /// Logical image size whose raster maps to exactly one device-pixel cell at this scale.
    pub fn display_size(&self) -> f32 {
        self.device_side as f32 / self.scale
    }

    /// A cell's warped ground.
    pub fn ground(&mut self, cell: GroundCell, remembered: bool, lit: bool) -> Arc<RenderImage> {
        self.get(ImageKey { source: Source::Ground(cell), remembered, lit })
    }

    /// An object icon over the ground, optionally mirrored left to right.
    pub fn object(&mut self, object: Object, mirrored: bool, remembered: bool, lit: bool) -> Arc<RenderImage> {
        self.get(ImageKey { source: Source::Object { object, mirrored }, remembered, lit })
    }

    /// The pixel of a quarter's tile at `local` within the cell. The cell's NW quarter is the SE quarter of the
    /// tile centered on the cell's NW corner.
    fn quarter_pixel(&self, quarter: Quarter, local: (usize, usize)) -> [u8; 4] {
        let (qx, qy) = (local.0 / QUARTER, local.1 / QUARTER);
        let x = (quarter.mask as usize % 4) * NATIVE_SIDE + (1 - qx) * QUARTER + local.0 % QUARTER;
        let y = (quarter.mask as usize / 4) * NATIVE_SIDE + (1 - qy) * QUARTER + local.1 % QUARTER;
        self.sets[&quarter.set].get_pixel(x as u32, y as u32).0
    }

    fn native(&self, source: Source) -> RgbaImage {
        let mut out = RgbaImage::new(NATIVE_SIDE as u32, NATIVE_SIDE as u32);
        match source {
            Source::Ground(cell) => {
                // Quarters of the cell and its eight neighbors, which warped samples may reach.
                let quarters = [0, 1, 2].map(|r| {
                    [0, 1, 2].map(|c| ground_quarters([0, 1, 2].map(|y| [0, 1, 2].map(|x| cell.around[r + y][c + x]))))
                });
                let side = NATIVE_SIDE as i64;
                let (cx, cy) = (i64::from(cell.pos.0), i64::from(cell.pos.1));
                for y in 0..side {
                    for x in 0..side {
                        let p = (cx * side + x, cy * side + y);
                        let (wx, wy) = warp(self.seed, p);
                        let q = (p.0 + wx, p.1 + wy);
                        let r = (q.1.div_euclid(side) - cy + 1) as usize;
                        let c = (q.0.div_euclid(side) - cx + 1) as usize;
                        let local = (q.0.rem_euclid(side) as usize, q.1.rem_euclid(side) as usize);
                        let quarter = quarters[r][c][(local.1 / QUARTER) * 2 + local.0 / QUARTER];
                        let pixel = shaded(self.quarter_pixel(quarter, local), shade(self.seed, p));
                        let pixel = transform_pixel(pixel, cell.tints[r][c], false, false);
                        out.put_pixel(x as u32, y as u32, image::Rgba(pixel));
                    }
                }
            }
            Source::Object { object, mirrored } => {
                let i = object.index();
                let (ox, oy) = ((i % 4) * NATIVE_SIDE, (i / 4) * NATIVE_SIDE);
                for y in 0..NATIVE_SIDE {
                    for x in 0..NATIVE_SIDE {
                        let sx = if mirrored { NATIVE_SIDE - 1 - x } else { x };
                        out.put_pixel(x as u32, y as u32, *self.objects.get_pixel((ox + sx) as u32, (oy + y) as u32));
                    }
                }
            }
        }
        out
    }

    /// The cache key space is bounded by the visible map's cells and presentation variants, and end-of-frame
    /// evicts images that left the visible map.
    fn get(&mut self, key: ImageKey) -> Arc<RenderImage> {
        if !self.images.contains_key(&key) {
            let source = self.native(key.source);
            let opaque = matches!(key.source, Source::Ground(_));
            let pixels = rasterize(&source, opaque, self.device_side, self.factor, None, key.remembered, key.lit);
            let raster =
                RgbaImage::from_raw(self.device_side as u32, self.device_side as u32, pixels).expect("sized raster");
            self.images.insert(key, Arc::new(RenderImage::new([Frame::new(raster)])));
        }
        self.used.insert(key);
        self.images.get(&key).expect("image was inserted").clone()
    }

    /// Release images no longer used by the current map frame from GPUI's texture atlas.
    pub fn end(&mut self, window: &mut Window) {
        for image in self.finish() {
            window.drop_image(image).ok();
        }
    }

    fn finish(&mut self) -> Vec<Arc<RenderImage>> {
        let stale: Vec<ImageKey> = self.images.keys().filter(|key| !self.used.contains(key)).copied().collect();
        stale.into_iter().filter_map(|key| self.images.remove(&key)).collect()
    }
}

/// Integer-only nearest-neighbor enlargement. Fractional display scales get edge padding (or transparent
/// padding for objects), never resampling, so the resulting raster fits the device cell exactly.
fn rasterize(
    source: &RgbaImage,
    opaque: bool,
    target_side: usize,
    factor: usize,
    tint: Option<[u8; 3]>,
    remembered: bool,
    lit: bool,
) -> Vec<u8> {
    let side = source.width() as usize;
    let enlarged = side * factor;
    // Below 1:1 scale, nearest sampling skips source pixels rather than blending them.
    let (enlarged, step) = if target_side < side { (target_side, None) } else { (enlarged, Some(factor)) };
    let pad = target_side.saturating_sub(enlarged) / 2;
    let mut output = Vec::with_capacity(target_side * target_side * 4);
    for y in 0..target_side {
        for x in 0..target_side {
            let inner_x = x as isize - pad as isize;
            let inner_y = y as isize - pad as isize;
            let inside = inner_x >= 0 && inner_y >= 0 && inner_x < enlarged as isize && inner_y < enlarged as isize;
            let pixel = if inside || opaque {
                let cx = inner_x.clamp(0, enlarged as isize - 1) as usize;
                let cy = inner_y.clamp(0, enlarged as isize - 1) as usize;
                let (sx, sy) = match step {
                    Some(factor) => (cx / factor, cy / factor),
                    None => (cx * side / target_side, cy * side / target_side),
                };
                transform_pixel(source.get_pixel(sx as u32, sy as u32).0, tint, remembered, lit)
            } else {
                [0, 0, 0, 0]
            };
            // RenderImage consumes straight BGRA bytes.
            output.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    output
}

fn transform_pixel(mut pixel: [u8; 4], tint: Option<[u8; 3]>, remembered: bool, lit: bool) -> [u8; 4] {
    if pixel[3] == 0 {
        return pixel;
    }
    if let Some(tint) = tint {
        for (channel, target) in pixel[..3].iter_mut().zip(tint) {
            *channel = blend(*channel, target, BIOME_TINT_PERCENT);
        }
    }
    if lit {
        for (channel, target) in pixel[..3].iter_mut().zip(CAVE_TINT) {
            *channel = blend(*channel, target, CAVE_TINT_PERCENT);
        }
    }
    if remembered {
        for channel in &mut pixel[..3] {
            *channel = ((*channel as u16 * 35) / 100) as u8;
        }
    }
    pixel
}

fn blend(original: u8, target: u8, percent: u16) -> u8 {
    ((u16::from(original) * (100 - percent) + u16::from(target) * percent) / 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use blah_core::{BIOMES, BiomeId};
    use isle_core::{Armor, Bow, Weapon};

    fn terrain_named(name: &str) -> Terrain {
        match name {
            "water" => Terrain::Water,
            "grass" => Terrain::Grass,
            "dirt" => Terrain::Dirt,
            "sand" => Terrain::Sand,
            "rock" => Terrain::Rock,
            "floor" => Terrain::Floor,
            "wall" => Terrain::Wall,
            other => panic!("unknown terrain {other}"),
        }
    }

    #[test]
    fn embedded_sheets_decode_and_match_the_manifest() {
        let manifest: serde_json::Value = serde_json::from_str(include_str!("../assets/world/manifest.json")).unwrap();
        assert_eq!(manifest["tile_size"], 32);
        for set in ALL_SETS {
            decode(set.bytes());
            let entry = &manifest["terrain"][set.name()];
            let (a, b) = set.terrains();
            assert_eq!(
                (terrain_named(entry["a"].as_str().unwrap()), terrain_named(entry["b"].as_str().unwrap())),
                (a, b)
            );
        }
        let sheet = decode(OBJECTS);
        let order: Vec<&str> =
            manifest["objects"]["order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(order.len(), ALL_OBJECTS.len());
        for object in ALL_OBJECTS {
            let i = object.index();
            let visible = (0..32)
                .flat_map(|y| (0..32).map(move |x| (x, y)))
                .filter(|&(x, y)| sheet.get_pixel((i % 4 * 32 + x) as u32, (i / 4 * 32 + y) as u32)[3] > 0)
                .count();
            assert!(visible > 40 && visible < 1024, "{object:?} must be a visible object on transparency");
        }
        for set in ALL_SETS {
            let sheet = decode(set.bytes());
            assert!(sheet.pixels().all(|p| p[3] == 255), "{set:?} terrain must be opaque");
        }
    }

    #[test]
    fn every_terrain_has_a_home_tile_of_its_own_side() {
        for terrain in
            [Terrain::Water, Terrain::Grass, Terrain::Dirt, Terrain::Sand, Terrain::Rock, Terrain::Floor, Terrain::Wall]
        {
            let (set, is_b) = terrain.home();
            let (a, b) = set.terrains();
            assert_eq!(if is_b { b } else { a }, terrain);
            assert_eq!(ground_quarters([[terrain; 3]; 3]), [Quarter { set, mask: if is_b { 15 } else { 0 } }; 4]);
        }
    }

    #[test]
    fn quarters_meet_neighbors_on_cell_edges() {
        use Terrain::{Floor, Grass, Sand, Wall, Water};
        // Grass with water to the east: the east quarters use shore tiles whose east corners are water.
        let around = [[Grass, Grass, Water], [Grass, Grass, Water], [Grass, Grass, Water]];
        let q = ground_quarters(around);
        assert_eq!(q[0], Quarter { set: TileSet::Shore, mask: 15 });
        assert_eq!(q[1], Quarter { set: TileSet::Shore, mask: 0b1010 });
        assert_eq!(q[3], Quarter { set: TileSet::Shore, mask: 0b1010 });
        // A lone wall pillar keeps its own corners in every quarter, so it still shows.
        let mut pillar = [[Floor; 3]; 3];
        pillar[1][1] = Wall;
        assert_eq!(ground_quarters(pillar).map(|q| q.mask), [0b0001, 0b0010, 0b0100, 0b1000]);
        // The island's common neighbors all have a joining set.
        use Terrain::{Dirt, Rock};
        for (first, second) in
            [(Grass, Water), (Sand, Water), (Grass, Sand), (Grass, Rock), (Sand, Rock), (Grass, Dirt), (Floor, Wall)]
        {
            assert!(joining(first, second).is_some() && joining(second, first).is_some(), "{first:?}/{second:?}");
        }
        // Terrains no set joins meet with a hard edge: the center's full tile.
        let mut unjoined = [[Sand; 3]; 3];
        unjoined[1][1] = Wall;
        assert_eq!(ground_quarters(unjoined), [Quarter { set: TileSet::Dungeon, mask: 15 }; 4]);
    }

    /// Grounds and tints of a world laid out by `world`, as the view would pass them with everything known.
    fn world_cell(world: &dyn Fn(i32, i32) -> Terrain, pos: (i32, i32)) -> GroundCell {
        let tint =
            |x: i32, y: i32| (world(x, y) == Terrain::Water).then_some([20, 70, 90 + (x.rem_euclid(3) * 30) as u8]);
        GroundCell {
            pos,
            around: [-2, -1, 0, 1, 2].map(|dy| [-2, -1, 0, 1, 2].map(|dx| world(pos.0 + dx, pos.1 + dy))),
            tints: [-1, 0, 1].map(|dy| [-1, 0, 1].map(|dx| tint(pos.0 + dx, pos.1 + dy))),
        }
    }

    fn sample_world(world: &dyn Fn(i32, i32) -> Terrain, cache: &ArtCache, p: (i64, i64)) -> [u8; 4] {
        // Independent of any one cell: find the source cell of the warped point in the world itself.
        let (wx, wy) = warp(cache.seed, p);
        let q = (p.0 + wx, p.1 + wy);
        let source = (q.0.div_euclid(32) as i32, q.1.div_euclid(32) as i32);
        let around = [-1, 0, 1].map(|dy| [-1, 0, 1].map(|dx| world(source.0 + dx, source.1 + dy)));
        let local = (q.0.rem_euclid(32) as usize, q.1.rem_euclid(32) as usize);
        let quarter = ground_quarters(around)[(local.1 / 16) * 2 + local.0 / 16];
        let tint = world_cell(world, source).tints[1][1];
        transform_pixel(shaded(cache.quarter_pixel(quarter, local), shade(cache.seed, p)), tint, false, false)
    }

    fn mixed_world(x: i32, y: i32) -> Terrain {
        match (x.rem_euclid(5), y.rem_euclid(4)) {
            (0, _) => Terrain::Water,
            (1, 0) => Terrain::Sand,
            (2, _) | (3, 1) => Terrain::Dirt,
            (4, 3) => Terrain::Rock,
            _ => Terrain::Grass,
        }
    }

    #[test]
    fn neighboring_cells_agree_with_one_world_wide_sampling() {
        let cache = ArtCache::new(7);
        for cy in -2..3 {
            for cx in -2..3 {
                let image = cache.native(Source::Ground(world_cell(&mixed_world, (cx, cy))));
                for y in 0..32 {
                    for x in 0..32 {
                        let p = (i64::from(cx) * 32 + x, i64::from(cy) * 32 + y);
                        assert_eq!(
                            image.get_pixel(x as u32, y as u32).0,
                            sample_world(&mixed_world, &cache, p),
                            "{p:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn warp_stays_within_a_neighbor_and_moves_pixels() {
        let mut moved = 0;
        for y in -300..300 {
            for x in (-300..300).step_by(7) {
                let (wx, wy) = warp(3, (x, y));
                assert!(wx.abs() <= WARP_REACH && wy.abs() <= WARP_REACH);
                moved += usize::from((wx, wy) != (0, 0));
            }
        }
        assert!(moved > 30_000, "most pixels are displaced: {moved}");
    }

    #[test]
    fn identical_terrain_differs_from_cell_to_cell_and_seed_to_seed() {
        let grass = |_: i32, _: i32| Terrain::Grass;
        let cache = ArtCache::new(7);
        let a = cache.native(Source::Ground(world_cell(&grass, (0, 0))));
        let b = cache.native(Source::Ground(world_cell(&grass, (1, 0))));
        let c = cache.native(Source::Ground(world_cell(&grass, (0, 1))));
        assert!(a != b && a != c && b != c, "a uniform field must not repeat cell to cell");
        let other = ArtCache::new(8).native(Source::Ground(world_cell(&grass, (0, 0))));
        assert_ne!(a, other);
        assert_eq!(a, ArtCache::new(7).native(Source::Ground(world_cell(&grass, (0, 0)))), "deterministic");
    }

    #[test]
    fn mirrored_objects_flip_left_to_right() {
        let cache = ArtCache::new(1);
        let plain = cache.native(Source::Object { object: Object::Pine, mirrored: false });
        let flipped = cache.native(Source::Object { object: Object::Pine, mirrored: true });
        assert_eq!(image::imageops::flip_horizontal(&plain), flipped);
        let mirrored = (0..400).filter(|&x| terrain_art(Tile::Tree, &BIOMES[0], false, (x, 3)).mirrored).count();
        assert!(mirrored > 100 && mirrored < 300, "about half of trees are mirrored: {mirrored}");
    }

    #[test]
    fn ground_cache_reuses_images_and_evicts_old_cells_and_scales() {
        let mut cache = ArtCache::new(7);
        let cell = world_cell(&mixed_world, (0, 0));
        let first = cache.ground(cell, false, false);
        assert!(Arc::ptr_eq(&first, &cache.ground(cell, false, false)));
        let mut changed = cell;
        changed.tints[1][1] = Some([30, 80, 150]);
        assert!(!Arc::ptr_eq(&first, &cache.ground(changed, false, false)));
        assert!(!Arc::ptr_eq(&first, &cache.ground(cell, true, false)));
        assert!(!Arc::ptr_eq(&first, &cache.ground(cell, false, true)));
        assert!(cache.begin_scale(1.0).is_empty());
        cache.ground(cell, false, false);
        assert_eq!(cache.finish().len(), 3);
        assert_eq!(cache.images.len(), 1);
        assert_eq!(cache.begin_scale(2.0).len(), 1);
        assert!(cache.images.is_empty());
        cache.ground(cell, false, false);
        cache.begin_scale(2.0);
        assert_eq!(cache.finish().len(), 1);
        assert!(cache.images.is_empty());
    }

    /// Measure first-frame, cached-frame and scrolling costs at Retina scale, without GPU upload.
    #[test]
    #[ignore]
    fn ground_render_cost() {
        use std::time::Instant;
        let start = Instant::now();
        let mut cache = ArtCache::new(7);
        cache.begin_scale(2.0);
        let radius = isle_core::SIGHT;
        let draw = |cache: &mut ArtCache, offset| {
            for y in -radius..=radius {
                for x in -radius..=radius {
                    std::hint::black_box(cache.ground(world_cell(&mixed_world, (x + offset, y)), false, false));
                }
            }
            cache.finish();
        };
        draw(&mut cache, 0);
        let cold = start.elapsed();
        let start = Instant::now();
        cache.begin_scale(2.0);
        draw(&mut cache, 0);
        let warm = start.elapsed();
        let start = Instant::now();
        cache.begin_scale(2.0);
        draw(&mut cache, 1);
        let scroll = start.elapsed();
        let count = (2 * radius + 1).pow(2) as usize;
        assert_eq!(cache.images.len(), count);
        eprintln!(
            "{count} cells, 2x: cold {cold:?}, cached {warm:?}, scroll {scroll:?}; {} raster bytes",
            count * 64 * 64 * 4
        );
    }

    /// Writes target/isle-world-art/noise-contact.png from the real renderer:
    /// `cargo test -p isle contact_sheet -- --ignored`.
    #[test]
    #[ignore]
    fn contact_sheet() {
        use Terrain::{Dirt, Floor, Grass, Rock, Sand, Wall, Water};
        let (w, h) = (26, 16);
        let world = |x: i32, y: i32| -> Terrain {
            let (fx, fy) = (x as f32 - 11.0, y as f32 - 8.0);
            let island = (fx * fx) / 90.0 + (fy * fy) / 34.0;
            if (20..25).contains(&x) && (3..12).contains(&y) {
                return if (21..24).contains(&x) && (4..11).contains(&y) || (y == 7 && x == 20) { Floor } else { Wall };
            }
            if x >= 19 {
                return Water;
            }
            if island > 1.0 {
                Water
            } else if island > 0.72 {
                Sand
            } else if (fx + 4.0).powi(2) + (fy + 2.0).powi(2) < 7.0 || (x, y) == (14, 10) {
                Rock
            } else if y == 9 && (3..17).contains(&x) || x == 12 && (4..9).contains(&y) {
                Dirt
            } else {
                Grass
            }
        };
        let cache = ArtCache::new(7);
        let scale = 2;
        let mut out = RgbaImage::new((w * 32 * scale) as u32, (h * 32 * scale) as u32);
        for cy in 0..h {
            for cx in 0..w {
                let mut cell = cache.native(Source::Ground(world_cell(&world, (cx, cy))));
                let art = terrain_art(Tile::Grass, &BIOMES[0], false, (cx, cy));
                let object = match (cx, cy) {
                    (6..=9, 11..=12) | (13, 5) | (15, 6) => Some(tree_at((cx, cy))),
                    (16, 11) => Some(Object::Boulder),
                    (22, 5) => Some(Object::StairsDown),
                    (8, 9) => Some(Object::Coins),
                    _ => None,
                };
                if let Some(object) = object {
                    let icon =
                        cache.native(Source::Object { object, mirrored: art.mirrored || mix((cx, cy)) & 32 != 0 });
                    image::imageops::overlay(&mut cell, &icon, 0, 0);
                }
                let big = image::imageops::resize(&cell, 64, 64, image::imageops::FilterType::Nearest);
                image::imageops::replace(&mut out, &big, i64::from(cx * 64), i64::from(cy * 64));
            }
        }
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/isle-world-art/noise-contact.png");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        out.save(&path).unwrap();
        eprintln!("{}", path.display());
    }
    #[test]
    fn fractional_scales_use_integer_pixels_and_pad_without_smoothing() {
        let mut source = RgbaImage::new(32, 32);
        for (x, y, pixel) in source.enumerate_pixels_mut() {
            *pixel = image::Rgba([x as u8, y as u8, (x + y) as u8, 255]);
        }
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0] {
            let mut cache = ArtCache::new(7);
            cache.begin_scale(scale);
            let (device_side, factor) = (cache.device_side, cache.factor);
            assert_eq!(device_side as f32 / scale, DISPLAY_SIDE);
            let bytes = rasterize(&source, true, device_side, factor, None, false, false);
            assert_eq!(bytes.len(), device_side * device_side * 4);
            let enlarged = 32 * factor;
            let pad = (device_side - enlarged) / 2;
            for y in 0..device_side {
                for x in 0..device_side {
                    let sx = ((x as isize - pad as isize).clamp(0, enlarged as isize - 1) as usize) / factor;
                    let sy = ((y as isize - pad as isize).clamp(0, enlarged as isize - 1) as usize) / factor;
                    let at = (y * device_side + x) * 4;
                    assert_eq!(&bytes[at..at + 4], &[(sx + sy) as u8, sy as u8, sx as u8, 255]);
                }
            }
        }
        // Below 1:1 nearest sampling picks source pixels without blending.
        let small = rasterize(&source, true, 16, 1, None, false, false);
        assert_eq!(&small[(16 + 1) * 4..(16 + 1) * 4 + 4], &[4, 2, 2, 255]);
    }

    #[test]
    fn dim_lit_and_tinted_pixels_keep_straight_alpha_and_bgra_order() {
        let source = RgbaImage::from_pixel(32, 32, image::Rgba([100, 80, 40, 128]));
        let plain = rasterize(&source, false, 32, 1, None, false, false);
        assert_eq!(&plain[..4], &[40, 80, 100, 128]);
        let remembered = rasterize(&source, false, 32, 1, None, true, false);
        assert_eq!(&remembered[..4], &[14, 28, 35, 128]);
        assert_eq!(transform_pixel([100, 80, 40, 128], None, false, true), [94, 95, 53, 128]);
        assert_eq!(transform_pixel([100, 80, 40, 0], Some([0, 255, 0]), true, true), [100, 80, 40, 0]);
        assert_eq!(transform_pixel([100, 80, 40, 128], Some([200, 20, 60]), false, false), [118, 69, 43, 128]);
    }

    #[test]
    fn objects_stay_transparent_in_fractional_padding() {
        let source = RgbaImage::from_pixel(32, 32, image::Rgba([30, 40, 50, 255]));
        assert_eq!(&rasterize(&source, false, 40, 1, None, false, true)[..4], &[0, 0, 0, 0]);
        assert_eq!(&rasterize(&source, true, 40, 1, None, false, false)[..4], &[50, 40, 30, 255]);
    }

    #[test]
    fn terrain_selection_uses_biome_properties() {
        let water = BIOMES.iter().find(|b| b.water).unwrap();
        let cold = BIOMES.iter().find(|b| !b.water && b.passable && b.temperature.max <= 0.40).unwrap();
        let fertile = BIOMES
            .iter()
            .find(|b| !b.water && b.passable && b.temperature.max > 0.40 && b.productivity >= 0.45)
            .unwrap();
        let dry = BIOMES
            .iter()
            .find(|b| !b.water && b.passable && b.temperature.max > 0.40 && b.productivity < 0.45)
            .unwrap();
        let at = (3, 4);
        assert_eq!(terrain_art(Tile::Wild(BiomeId(0)), water, false, at).ground, Terrain::Water);
        assert_eq!(terrain_art(Tile::Wild(BiomeId(0)), cold, false, at).ground, Terrain::Rock);
        assert_eq!(terrain_art(Tile::Wild(BiomeId(0)), fertile, false, at).ground, Terrain::Grass);
        assert_eq!(terrain_art(Tile::Wild(BiomeId(0)), dry, false, at).ground, Terrain::Sand);
        let boulder = terrain_art(Tile::Rock, fertile, false, at);
        assert_eq!((boulder.ground, boulder.object), (Terrain::Grass, Some(Object::Boulder)));
        let tree = terrain_art(Tile::Tree, fertile, false, at);
        assert!(matches!(tree.object, Some(Object::Pine | Object::Oak)));
        assert_eq!(tree.ground, Terrain::Grass);
        assert_eq!(terrain_art(Tile::Floor, fertile, false, at).ground, Terrain::Dirt);
        let deep_floor = terrain_art(Tile::Floor, &isle_core::DEEP, true, at);
        assert_eq!((deep_floor.ground, deep_floor.tint), (Terrain::Floor, None));
        let stairs = terrain_art(Tile::StairsDown, &isle_core::DEEP, true, at);
        assert_eq!((stairs.ground, stairs.object), (Terrain::Floor, Some(Object::StairsDown)));
        let woods: HashSet<_> = (0..64).map(|x| tree_at((x, 7))).collect();
        assert_eq!(woods.len(), 2, "woods mix both tree shapes");
    }

    #[test]
    fn every_item_variant_and_mixed_pile_has_an_icon() {
        let variants = [
            (Item::Gold(1), Object::Coins),
            (Item::Arrows(1), Object::Arrows),
            (Item::Potion, Object::Potion),
            (Item::Weapon(Weapon::Dagger), Object::Weapon),
            (Item::Bow(Bow::Shortbow), Object::Bow),
            (Item::Armor(Armor::Leather), Object::Armor),
            (Item::Ring, Object::Ring),
        ];
        for (item, expected) in variants {
            assert_eq!(item_art(&[item]), Some(expected));
        }
        assert_eq!(item_art(&[Item::Gold(1), Item::Potion]), Some(Object::Treasure));
        assert_eq!(item_art(&[]), None);
    }
}

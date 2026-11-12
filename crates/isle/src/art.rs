//! Isle's map art, painted in code.
//!
//! Every ground pixel is computed from its world position, so neighboring cells always agree and a field never
//! repeats. Natural ground (water, grass, sand, snow, rock, and Deep caves) takes each pixel's terrain from the
//! nearby cell centers after a smooth world-space warp, which rounds corners and wobbles shores; built ground
//! (streets, masonry) keeps square edges. Each biome picks a look (see [`style_of`]) and grows its own details
//! (see [`flavor_of`]); where biomes meet, each pixel takes one neighbor's look against a noisy threshold, so
//! borders are ragged rather than blurred. The Deep paints in its stone instead. Weather grades the result (see
//! [`crate::mood`]). Objects, items, and the hero are drawn in code too. Images are enlarged by integer
//! nearest-neighbor duplication for the display scale and cached only while the visible map uses them.

use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;
use std::sync::Arc;

use gpui::{RenderImage, Window};
use image::{Frame, RgbaImage};
use isle_core::{Item, Tile};

use blah_core::Biome;

use crate::hero::{self, Look};
use crate::mood::{self, FRAMES, Mood, Stone};
use crate::paint::{self, HEAD, Palette, Style};

const NATIVE_SIDE: usize = 32;
const SIDE: i64 = NATIVE_SIDE as i64;
const DISPLAY_SIDE: f32 = 32.0;
const BIOME_TINT_PERCENT: u16 = 18;
const CAVE_TINT: [u8; 3] = [76, 150, 101];
const CAVE_TINT_PERCENT: u16 = 22;
const WHITE: [u8; 3] = [255, 255, 255];

/// A kind of ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Terrain {
    Water,
    Grass,
    Dirt,
    Sand,
    Snow,
    Rock,
    Floor,
    Wall,
}

impl Terrain {
    /// Built ground keeps the grid's square edges and straight lines. Underground, floors and walls are cave.
    fn rigid(self, deep: bool) -> bool {
        match self {
            Terrain::Dirt => true,
            Terrain::Floor | Terrain::Wall => !deep,
            _ => false,
        }
    }

    /// Ground that stands above its surroundings and casts a shadow down and to the right.
    fn high(self) -> bool {
        matches!(self, Terrain::Rock | Terrain::Wall)
    }
}

/// What a biome's ground grows beyond its look, from its traits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Flavor {
    Plain,
    /// Wet lowland: dark damp grass, puddles, and reeds.
    Marsh,
    /// The richest land: deep green, thick with growth.
    Lush,
    /// Hot grassland: straw-colored, sparse, with bare earth.
    Dry,
    /// Hot sand: broad dunes with lit and shadowed faces.
    Dunes,
}

/// Objects drawn over terrain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Object {
    Pine,
    Oak,
    /// A flat-topped dry-country tree.
    Acacia,
    /// A tall jungle tree with a heavy crown.
    Canopy,
    /// A marsh tree with hanging strands.
    Willow,
    Boulder,
    Door,
    StairsUp,
    StairsDown,
    Counter,
    Altar,
    Cairn,
    Camp,
    Shrine,
    Watchtower {
        cleared: bool,
    },
    CinderWarden,
    AshSentry,
    Herb(isle_core::Herb),
    Relic(isle_core::Relic),
    Coins,
    Arrows,
    Potion,
    Weapon,
    Bow,
    Armor,
    Ring,
    Treasure,
}

impl Object {
    fn natural(self) -> bool {
        matches!(self, Object::Pine | Object::Oak | Object::Acacia | Object::Canopy | Object::Willow | Object::Boulder)
    }
}

#[cfg(test)]
pub const ALL_OBJECTS: [Object; 32] = [
    Object::Pine,
    Object::Oak,
    Object::Acacia,
    Object::Canopy,
    Object::Willow,
    Object::Boulder,
    Object::Door,
    Object::StairsUp,
    Object::StairsDown,
    Object::Counter,
    Object::Altar,
    Object::Cairn,
    Object::Camp,
    Object::Shrine,
    Object::Watchtower { cleared: false },
    Object::Watchtower { cleared: true },
    Object::CinderWarden,
    Object::AshSentry,
    Object::Herb(isle_core::Herb::Woundleaf),
    Object::Herb(isle_core::Herb::Bogmint),
    Object::Herb(isle_core::Herb::Sunroot),
    Object::Relic(isle_core::Relic::EmberCharm),
    Object::Relic(isle_core::Relic::Stillstone),
    Object::Relic(isle_core::Relic::HuntersToken),
    Object::Coins,
    Object::Arrows,
    Object::Potion,
    Object::Weapon,
    Object::Bow,
    Object::Armor,
    Object::Ring,
    Object::Treasure,
];

/// What a cell shows, before its neighbors shape the ground's edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainArt {
    pub ground: Terrain,
    /// A transparent object placed over the ground, if any.
    pub object: Option<Object>,
    /// The biome's color, washed gently over natural surface ground.
    pub tint: Option<[u8; 3]>,
    /// The biome's look.
    pub style: Style,
    /// The biome's details.
    pub flavor: Flavor,
    /// Whether a natural object is drawn mirrored, so repeated trees and boulders vary.
    pub mirrored: bool,
    /// A natural object's size, 0 to 2; 1 for everything else.
    pub size: u8,
}

/// Natural ground from biome traits, never its name or table index.
fn natural_ground(biome: &Biome) -> Terrain {
    if biome.water {
        Terrain::Water
    } else if biome.temperature.max <= 0.13 || biome.elevation.min >= 0.79 && biome.temperature.max <= 0.35 {
        Terrain::Snow
    } else if !biome.passable || biome.elevation.min >= 0.7 && biome.productivity < 0.3 {
        Terrain::Rock
    } else if biome.productivity >= 0.3 {
        Terrain::Grass
    } else {
        Terrain::Sand
    }
}

/// A biome's look from its climate: warm lowlands are sunlit, cold, wet, or high land is cool and misty, and
/// the temperate rest is mossy.
pub fn style_of(biome: &Biome) -> Style {
    if biome.temperature.min >= 0.45 {
        Style::Fable
    } else if biome.temperature.max <= 0.38 || biome.elevation.min >= 0.72 || biome.moisture.min >= 0.7 {
        Style::Hearth
    } else {
        Style::Moss
    }
}

/// A biome's details from its climate: wet grassland is marsh, the most productive is lush, hot grassland is
/// dry, and hot sand forms dunes.
pub fn flavor_of(biome: &Biome) -> Flavor {
    match natural_ground(biome) {
        Terrain::Sand if biome.temperature.min >= 0.45 => Flavor::Dunes,
        Terrain::Grass if biome.moisture.min >= 0.7 => Flavor::Marsh,
        Terrain::Grass if biome.productivity >= 0.9 => Flavor::Lush,
        Terrain::Grass if biome.temperature.min >= 0.6 => Flavor::Dry,
        _ => Flavor::Plain,
    }
}

fn mix(pos: (i32, i32)) -> u32 {
    ((pos.0 as u32).wrapping_mul(0x9e37_79b9) ^ (pos.1 as u32).wrapping_mul(0x85eb_ca6b)).rotate_left(13)
}

/// A stable per-cell tree: conifers in cold land, broadleaves in warm, both in between, and each biome's own
/// trees where it grows them.
fn tree_at(pos: (i32, i32), style: Style, flavor: Flavor) -> Object {
    let roll = (mix(pos) >> 3) % 8;
    match flavor {
        Flavor::Lush => return if roll < 6 { Object::Canopy } else { Object::Oak },
        Flavor::Dry => return if roll < 6 { Object::Acacia } else { Object::Oak },
        Flavor::Marsh => return if roll < 5 { Object::Willow } else { Object::Oak },
        Flavor::Dunes => return Object::Acacia,
        Flavor::Plain => {}
    }
    let pines = match style {
        Style::Hearth => 7,
        Style::Moss => 4,
        Style::Fable => 1,
    };
    if roll < pines { Object::Pine } else { Object::Oak }
}

/// A natural object's stable size: mostly middling, some small, some big. Jungle trees are mostly big.
fn size_at(pos: (i32, i32), object: Object) -> u8 {
    let size = [0, 0, 1, 1, 1, 1, 2, 2][((mix(pos) >> 9) % 8) as usize];
    match (object, size) {
        (Object::Canopy, 0) => 1,
        (Object::Canopy, _) => 2,
        _ => size,
    }
}

/// Art layers for a tile. `deep` levels draw caves; towns pave their floors with dirt and build their walls of
/// stone.
pub fn terrain_art(tile: Tile, biome: &Biome, deep: bool, pos: (i32, i32)) -> TerrainArt {
    let floor = if deep { Terrain::Floor } else { Terrain::Dirt };
    let natural = natural_ground(biome);
    // Trees and boulders never stand in water.
    let open = if natural == Terrain::Water { Terrain::Grass } else { natural };
    let style = style_of(biome);
    let flavor = if deep { Flavor::Plain } else { flavor_of(biome) };
    let (ground, object) = match tile {
        Tile::Wall => (Terrain::Wall, None),
        Tile::Floor => (floor, None),
        Tile::Grass => (Terrain::Grass, None),
        Tile::Water => (Terrain::Water, None),
        Tile::Tree => (open, Some(tree_at(pos, style, flavor))),
        Tile::Door => (floor, Some(Object::Door)),
        Tile::StairsUp => (floor, Some(Object::StairsUp)),
        Tile::StairsDown => (floor, Some(Object::StairsDown)),
        Tile::Counter => (floor, Some(Object::Counter)),
        Tile::Altar => (floor, Some(Object::Altar)),
        Tile::Wild(_) => (natural, None),
        // Impassable rock is a terrain; a passable biome's scattered rocks are boulders on its ground.
        Tile::Rock if !biome.passable => (Terrain::Rock, None),
        Tile::Rock => (if deep { floor } else { open }, Some(Object::Boulder)),
    };
    // Natural ground keeps a little of its biome's color, which separates deep from shallow water, red
    // badlands from grey peaks, and jungle from savanna. The Deep has its own stone.
    let tint = (!deep && !ground.rigid(false)).then_some(biome.color);
    let natural = object.is_some_and(Object::natural);
    let mirrored = natural && mix(pos) & 16 != 0;
    let size = match object {
        Some(object) if natural => size_at(pos, object),
        _ => 1,
    };
    TerrainArt { ground, object, tint, style, flavor, mirrored, size }
}

/// The ground-pile icon. Mixed piles use a single treasure mark, and empty piles have no icon.
pub fn item_art(items: &[Item]) -> Option<Object> {
    if items.len() > 1 {
        return Some(Object::Treasure);
    }
    match items.first()? {
        Item::Relic(relic) => Some(Object::Relic(*relic)),
        Item::Herb(herb) => Some(Object::Herb(*herb)),
        Item::Gold(_) => Some(Object::Coins),
        Item::Arrows(_) => Some(Object::Arrows),
        Item::Potion => Some(Object::Potion),
        Item::Weapon(_) => Some(Object::Weapon),
        Item::Bow(_) => Some(Object::Bow),
        Item::Armor(_) => Some(Object::Armor),
        Item::Ring => Some(Object::Ring),
    }
}

/// The 5×5 block of grounds centered on a cell, rows north to south, and the 3×3 blocks of tints, looks, and
/// details. Callers pass the center's own values for cells the player does not know, so edges never reveal
/// unexplored terrain. Looks reach one cell away; the outer ring of `around` only tells water how far the
/// shore is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GroundCell {
    pub pos: (i32, i32),
    pub around: [[Terrain; 5]; 5],
    pub tints: [[Option<[u8; 3]>; 3]; 3],
    pub styles: [[Style; 3]; 3],
    pub flavors: [[Flavor; 3]; 3],
}

/// What painting needs to know of one map cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Spot {
    terrain: Terrain,
    style: Style,
    flavor: Flavor,
    tint: Option<[u8; 3]>,
}

impl Spot {
    fn key(self) -> (u8, u8, Option<[u8; 3]>) {
        (self.style as u8, self.flavor as u8, self.tint)
    }
}

impl GroundCell {
    /// The cell at world cell `(x, y)`, up to two away. The outer ring's look is its nearest inner neighbor's,
    /// which painting never reads.
    fn spot(&self, x: i64, y: i64) -> Spot {
        let (dx, dy) = ((x - i64::from(self.pos.0)) as isize, (y - i64::from(self.pos.1)) as isize);
        let (r, c) = ((dy + 1).clamp(0, 2) as usize, (dx + 1).clamp(0, 2) as usize);
        Spot {
            terrain: self.around[(dy + 2) as usize][(dx + 2) as usize],
            style: self.styles[r][c],
            flavor: self.flavors[r][c],
            tint: self.tints[r][c],
        }
    }
}

type World<'a> = &'a dyn Fn(i64, i64) -> Spot;
type Ground<'a> = &'a dyn Fn((i64, i64)) -> Terrain;

/// Layers of the world-space warp as (period, amplitude) in source pixels. The amplitudes sum to well under a
/// cell, so a warped sample never reaches past a neighboring cell.
const WARP: [(f32, f32); 2] = [(24.0, 5.0), (9.0, 2.0)];
#[cfg(test)]
const WARP_REACH: i64 = 7;

pub(crate) fn lattice(seed: u64, channel: u64, x: i64, y: i64) -> f32 {
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
pub(crate) fn value_noise(seed: u64, channel: u64, x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (smooth(x - x0), smooth(y - y0));
    let (ix, iy) = (x0 as i64, y0 as i64);
    let corner = |dx, dy| lattice(seed, channel, ix + dx, iy + dy);
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * sx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * sx;
    (top + (bottom - top) * sy) * 2.0 - 1.0
}

fn noise(seed: u64, channel: u64, period: f32, p: (i64, i64)) -> f32 {
    value_noise(seed, channel, p.0 as f32 / period, p.1 as f32 / period)
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

fn warp_weight(terrain: Terrain, deep: bool) -> f32 {
    match terrain {
        t if t.rigid(deep) => 0.0,
        Terrain::Rock => 0.9,
        _ => 1.0,
    }
}

/// The four cell centers around world pixel `p` and their bilinear weights.
fn corners(p: (i64, i64)) -> [((i64, i64), f32); 4] {
    let (x, y) = ((p.0 - 16).div_euclid(SIDE), (p.1 - 16).div_euclid(SIDE));
    let fx = (p.0 - 16).rem_euclid(SIDE) as f32 / SIDE as f32;
    let fy = (p.1 - 16).rem_euclid(SIDE) as f32 / SIDE as f32;
    [
        ((x, y), (1.0 - fx) * (1.0 - fy)),
        ((x + 1, y), fx * (1.0 - fy)),
        ((x, y + 1), (1.0 - fx) * fy),
        ((x + 1, y + 1), fx * fy),
    ]
}

/// Interpolate rigidity in world space, not separately per output cell. Built ground stays straight, and the
/// natural ground next to it gradually loses its warp rather than tearing at the cell boundary.
fn ground_point(seed: u64, p: (i64, i64), deep: bool, ground: impl Fn(i64, i64) -> Terrain) -> (i64, i64) {
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let (x, y) = ((p.0 - 16).div_euclid(SIDE), (p.1 - 16).div_euclid(SIDE));
    let fx = smooth((p.0 - 16).rem_euclid(SIDE) as f32 / SIDE as f32);
    let fy = smooth((p.1 - 16).rem_euclid(SIDE) as f32 / SIDE as f32);
    let w = |x, y| warp_weight(ground(x, y), deep);
    let top = w(x, y) * (1.0 - fx) + w(x + 1, y) * fx;
    let bottom = w(x, y + 1) * (1.0 - fx) + w(x + 1, y + 1) * fx;
    let strength = ((top * (1.0 - fy) + bottom * fy - 0.5) * 2.0).clamp(0.0, 1.0);
    let (wx, wy) = if strength == 0.0 { (0, 0) } else { warp(seed, p) };
    (p.0 + (wx as f32 * strength).round() as i64, p.1 + (wy as f32 * strength).round() as i64)
}

/// The terrain shown at world pixel `p`. A built cell owns exactly its square; elsewhere the nearest natural
/// cell centers vote by distance at the warped point, so corners round off and lone cells become blobs.
fn terrain_at(seed: u64, p: (i64, i64), deep: bool, world: World) -> Terrain {
    let own = world(p.0.div_euclid(SIDE), p.1.div_euclid(SIDE)).terrain;
    if own.rigid(deep) {
        return own;
    }
    let q = ground_point(seed, p, deep, |x, y| world(x, y).terrain);
    let mut votes: [(Terrain, f32); 4] = [(own, 1e-4); 4];
    let mut n = 1;
    for (at, w) in corners(q) {
        let t = world(at.0, at.1).terrain;
        if t.rigid(deep) {
            continue;
        }
        match votes[..n].iter_mut().find(|(v, _)| *v == t) {
            Some(vote) => vote.1 += w,
            None => {
                votes[n] = (t, w);
                n += 1;
            }
        }
    }
    votes[..n].iter().fold(votes[0], |best, &v| if v.1 > best.1 { v } else { best }).0
}

/// The cell whose look and details paint world pixel `p` (warped to `q`) of terrain `t`: one of the nearest
/// cells of the same terrain, chosen by weight against a noisy threshold. Where looks meet, the border is a
/// ragged, dithered band rather than a blur.
fn pick(seed: u64, p: (i64, i64), q: (i64, i64), t: Terrain, world: World) -> Spot {
    let own = world(p.0.div_euclid(SIDE), p.1.div_euclid(SIDE));
    let mut groups: [(Spot, f32); 4] = [(own, 0.0); 4];
    let mut n = 0;
    for (at, w) in corners(q) {
        let spot = world(at.0, at.1);
        if spot.terrain != t {
            continue;
        }
        match groups[..n].iter_mut().find(|(s, _)| s.key() == spot.key()) {
            Some(group) => group.1 += w,
            None => {
                groups[n] = (spot, w);
                n += 1;
            }
        }
    }
    if n == 0 {
        return own;
    }
    if n == 1 {
        return groups[0].0;
    }
    groups[..n].sort_by_key(|(s, _)| s.key());
    let total: f32 = groups[..n].iter().map(|(_, w)| w).sum();
    let jitter = 0.32 * noise(seed, 58, 7.0, p)
        + 0.14 * noise(seed, 60, 2.5, p)
        + 0.06 * (lattice(seed, 59, p.0, p.1) * 2.0 - 1.0);
    let u = (0.5 + jitter) * total;
    let mut acc = 0.0;
    for &(spot, w) in &groups[..n] {
        acc += w;
        if u < acc {
            return spot;
        }
    }
    groups[n - 1].0
}

/// The smoothly blended biome color of the cells of terrain `t` around `q`, so water deepens gradually.
fn tint_at(q: (i64, i64), t: Terrain, world: World) -> Option<[f32; 3]> {
    let (mut tint, mut weight) = ([0.0; 3], 0.0);
    for (at, w) in corners(q) {
        let spot = world(at.0, at.1);
        if let (true, Some(c)) = (spot.terrain == t, spot.tint) {
            weight += w;
            for (sum, v) in tint.iter_mut().zip(c) {
                *sum += w * f32::from(v);
            }
        }
    }
    (weight > 0.0).then(|| tint.map(|c| c / weight))
}

/// How open the water around `q` is: one far from any shore, about half at the waterline.
fn openness(q: (i64, i64), world: World) -> f32 {
    corners(q)
        .iter()
        .map(|&((x, y), w)| {
            let wet = (-1..=1)
                .flat_map(|dy| (-1..=1).map(move |dx| (x + dx, y + dy)))
                .filter(|&(x, y)| world(x, y).terrain == Terrain::Water)
                .count();
            w * wet as f32 / 9.0
        })
        .sum()
}

/// The nearest different terrain within three pixels along the axes, and how far.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Near(Option<(Terrain, i64)>);

impl Near {
    fn of(p: (i64, i64), here: Terrain, terrain: Ground) -> Near {
        for r in 1..=3 {
            for (dx, dy) in [(r, 0), (-r, 0), (0, r), (0, -r)] {
                let t = terrain((p.0 + dx, p.1 + dy));
                if t != here {
                    return Near(Some((t, r)));
                }
            }
        }
        Near(None)
    }

    fn within(self, reach: i64, of: impl Fn(Terrain) -> bool) -> bool {
        self.0.is_some_and(|(t, d)| d <= reach && of(t))
    }
}

fn mixf(a: [f32; 3], b: [u8; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (f32::from(b[i]) - a[i]) * t)
}

fn f(c: [u8; 3]) -> [f32; 3] {
    c.map(f32::from)
}

fn wash(c: [f32; 3], tint: Option<[f32; 3]>, amount: f32) -> [f32; 3] {
    tint.map_or(c, |tint| [0, 1, 2].map(|i| c[i] + (tint[i] - c[i]) * amount))
}

/// What painting one ground pixel knows.
struct Px<'a> {
    seed: u64,
    /// The world pixel, and the warped point its terrain was sampled at.
    p: (i64, i64),
    q: (i64, i64),
    near: Near,
    tint: Option<[f32; 3]>,
    flavor: Flavor,
    terrain: Ground<'a>,
    world: World<'a>,
}

impl Px<'_> {
    fn n(&self, channel: u64, period: f32, at: (i64, i64)) -> f32 {
        noise(self.seed, channel, period, at)
    }

    fn roll(&self, channel: u64) -> f32 {
        lattice(self.seed, channel, self.p.0, self.p.1)
    }

    /// Pixels straight up (`dir` -1) or down (1) to the first of other terrain than `t`, within `reach`.
    fn run(&self, t: Terrain, dir: i64, reach: i64) -> Option<i64> {
        (1..=reach).find(|&k| (self.terrain)((self.p.0, self.p.1 + dir * k)) != t)
    }

    /// A height field's slope toward the light in the upper left: positive faces it.
    fn facing(&self, channel: u64, period: f32) -> f32 {
        let h = |at: (i64, i64)| self.n(channel, period, at) + 0.45 * self.n(channel + 1, period * 0.4, at);
        h(self.q) - h((self.q.0 + 2, self.q.1 + 2))
    }
}

/// Bright crests of the water's ripples, which [`shimmer_pixel`] makes glitter.
fn crest(seed: u64, q: (i64, i64)) -> f32 {
    ((q.1 as f32 + noise(seed, 6, 30.0, q) * 6.0) / 3.0 + q.0 as f32 / 40.0).sin()
}

/// A marsh's standing water, above a half for puddles.
fn pool(seed: u64, q: (i64, i64)) -> f32 {
    noise(seed, 52, 15.0, q) + noise(seed, 61, 6.0, q) * 0.1
}

/// One material's color at a pixel in palette `pal`.
fn material(t: Terrain, pal: &Palette, stone: Option<Stone>, px: &Px) -> [f32; 3] {
    let (p, q, near) = (px.p, px.q, px.near);
    let deep = stone.is_some();
    match t {
        Terrain::Grass => {
            let patch = px.n(12, 75.0, q);
            let mut c = mixf(f(pal.grass), if patch > 0.0 { pal.grass2 } else { pal.grassdark }, patch.abs() * 0.65);
            c = wash(c, px.tint, 0.16);
            match px.flavor {
                Flavor::Lush => {
                    c = mixf(mixf(wash(c, px.tint, 0.4), pal.leaf, 0.3), pal.ink, 0.22);
                    if px.roll(60) < 0.07 {
                        c = mixf(c, pal.leafhi, 0.35);
                    }
                }
                Flavor::Dry => {
                    c = mixf(c, pal.sand, 0.25);
                    let bare = px.n(53, 21.0, q) + px.n(62, 7.0, q) * 0.3;
                    if bare > 0.3 {
                        c = mixf(c, pal.soil, if bare > 0.45 { 0.5 } else { 0.28 });
                    }
                }
                Flavor::Marsh => {
                    c = mixf(mixf(c, pal.water, 0.1), pal.grassdark, 0.25);
                    let wet = pool(px.seed, q);
                    if near.0.is_none() && wet > 0.66 {
                        let puddle = mixf(mixf(f(pal.water), pal.grassdark, 0.45), pal.grass, 0.1);
                        return if wet < 0.69 || crest(px.seed, q) > 0.9 {
                            mixf(puddle, pal.ripple, 0.35)
                        } else {
                            puddle
                        };
                    }
                    if wet > 0.58 {
                        c = mixf(c, paint::mix(pal.soil, pal.ink, 0.45), 0.4);
                    }
                }
                Flavor::Plain | Flavor::Dunes => {}
            }
            if near.within(2, |t| matches!(t, Terrain::Dirt | Terrain::Floor | Terrain::Wall | Terrain::Rock)) {
                c = mixf(c, pal.grassdark, 0.3);
            } else if near.within(1, |t| matches!(t, Terrain::Sand | Terrain::Water | Terrain::Snow)) {
                c = mixf(c, pal.sand, 0.2);
            }
            if px.roll(17) < 0.045 {
                c = mixf(c, pal.blade, 0.14);
            }
            c
        }
        Terrain::Sand => {
            let mut c = wash(f(pal.sand), px.tint, 0.12);
            if px.flavor == Flavor::Dunes {
                // Long dunes: the face toward the light is pale, the lee darker, and the crest a bright line.
                let phase = (q.0 as f32 * 0.4 + q.1 as f32) / 10.0 + px.n(54, 46.0, q) * 3.0;
                let (s, cos) = phase.sin_cos();
                if s > 0.97 {
                    c = mixf(c, WHITE, 0.22);
                } else if cos > 0.45 {
                    c = mixf(c, pal.snow, 0.16);
                } else if cos < -0.3 {
                    c = mixf(c, pal.soil, 0.32 * -cos);
                }
            } else {
                let ripple = ((q.1 as f32 + px.n(6, 30.0, q) * 6.0) / 3.0).sin().abs();
                if ripple < 0.12 && px.roll(18) > 0.25 {
                    c = mixf(c, pal.soil, 0.2);
                }
            }
            if near.within(2, |t| t == Terrain::Water) {
                c = mixf(c, pal.water, 0.3);
            }
            if px.roll(17) < 0.045 {
                c = mixf(c, pal.stone, 0.14);
            }
            c
        }
        Terrain::Water if stone == Some(Stone::Basalt) => {
            // A dark crust over hot rock, not an orange flood.
            let vein = (px.n(31, 13.0, q) + px.n(32, 6.0, q) * 0.28).abs();
            if vein < 0.025 {
                f(pal.glow)
            } else if vein < 0.12 {
                f(pal.lava)
            } else {
                mixf(f(pal.water), pal.ink, 0.4)
            }
        }
        Terrain::Water => {
            let mut c = match px.tint {
                Some(tint) => wash(f(pal.water), Some(tint), 0.3),
                None if deep => mixf(f(pal.water), pal.ink, 0.25),
                None => f(pal.water),
            };
            // Open water darkens away from the shore; the shallows are lighter.
            let open = openness(q, px.world);
            c = mixf(c, pal.ink, ((open - 0.62) / 0.38).clamp(0.0, 1.0) * 0.32);
            c = mixf(c, pal.ripple, ((0.62 - open) / 0.12).clamp(0.0, 1.0) * 0.2);
            if crest(px.seed, q) > 0.88 {
                c = mixf(c, pal.ripple, 0.2);
            }
            if near.within(2, |t| t != Terrain::Water) {
                c = mixf(f(pal.sand), pal.water, 0.55);
            } else if near.within(3, |t| t != Terrain::Water) {
                c = mixf(c, pal.ripple, 0.25);
            }
            c
        }
        Terrain::Dirt => {
            let mut c = mixf(f(pal.soil), pal.sand, px.n(26, 18.0, p).max(0.0) * 0.3);
            if near.within(2, |t| t == Terrain::Grass) {
                c = mixf(c, pal.grassdark, 0.3);
            }
            if px.roll(19) < 0.03 {
                c = mixf(c, pal.stone, 0.3);
            } else if px.roll(19) > 0.97 {
                c = mixf(c, pal.ink, 0.15);
            }
            c
        }
        Terrain::Snow => {
            let mut c = wash(mixf(f(pal.snow), pal.ripple, px.n(27, 40.0, q).max(0.0) * 0.3), px.tint, 0.05);
            // Wind-packed drifts: a bright lip with a blue shadow in its lee.
            let phase = (q.0 as f32 * 0.6 + q.1 as f32) / 8.0 + px.n(55, 34.0, q) * 2.4;
            let u = (phase / TAU).rem_euclid(1.0);
            if px.n(63, 52.0, q) > -0.25 {
                if u < 0.05 {
                    c = mixf(c, WHITE, 0.4);
                } else if u < 0.2 {
                    c = mixf(c, pal.ripple, 0.3 * (0.2 - u) / 0.15);
                }
            }
            if px.roll(17) < 0.008 {
                c = mixf(c, pal.grassdark, 0.35);
            }
            if near.within(1, |_| true) {
                c = mixf(c, pal.stone, 0.3);
            }
            c
        }
        Terrain::Rock => {
            let base = wash(f(pal.rock), px.tint, 0.3);
            let mut c = mixf(base, pal.stone, px.n(47, 16.0, p).max(0.0) * 0.18);
            // Relief: slopes facing the light are pale, the others shadowed.
            let slope = px.facing(56, 30.0);
            c = if slope > 0.0 {
                mixf(c, pal.stone, (slope * 2.0).min(0.35))
            } else {
                mixf(c, pal.ink, (-slope * 1.6).min(0.28))
            };
            if px.n(48, 19.0, q).abs() < 0.02 {
                c = mixf(c, pal.ink, 0.35);
            }
            if px.roll(17) < 0.05 {
                c = mixf(c, pal.stone, 0.2);
            }
            // A lit rim on top and a dark cliff face below.
            if px.run(t, -1, 1).is_some() {
                c = mixf(mixf(c, pal.stone, 0.5), WHITE, 0.12);
            } else if let Some(k) = px.run(t, 1, 3) {
                c = mixf(c, pal.ink, 0.55 - 0.1 * k as f32);
            } else if near.within(1, |t| t != Terrain::Snow) {
                c = mixf(c, pal.ink, 0.2);
            }
            c
        }
        Terrain::Wall if deep => {
            let mut c = mixf(f(pal.wall), pal.rock, px.n(47, 26.0, q).max(0.0) * 0.18);
            // Broad, soft relief rather than a net of facets.
            let slope = px.facing(56, 34.0);
            c = if slope > 0.0 {
                mixf(c, pal.rock, (slope * 1.2).min(0.22))
            } else {
                mixf(c, pal.ink, (-slope * 0.9).min(0.16))
            };
            // A pale rim keeps known walls distinct from unseen black.
            if near.within(2, |t| t != Terrain::Wall) {
                c = mixf(c, pal.stone, 0.22);
            }
            c.map(|v| v + 9.0)
        }
        Terrain::Wall => {
            let row = p.1.div_euclid(8);
            let x = p.0 + if row % 2 == 0 { 0 } else { 8 };
            let (u, v) = (x.rem_euclid(16), p.1.rem_euclid(8));
            if u == 0 || v == 0 {
                mixf(f(pal.rock), pal.ink, 0.5)
            } else {
                let brick = mixf(f(pal.rock), pal.stone, lattice(px.seed, 28, x.div_euclid(16), row) * 0.45);
                if v == 1 { mixf(brick, pal.stone, 0.25) } else { brick }
            }
        }
        Terrain::Floor => {
            let mut c = mixf(f(pal.floor), pal.stone, px.n(29, 23.0, p).max(0.0) * 0.18);
            if px.n(30, 17.0, q).abs() < 0.03 {
                c = mixf(c, pal.ink, 0.25);
            }
            if stone == Some(Stone::Slate) {
                c = mixf(c, pal.water, px.n(50, 23.0, p).max(0.0) * 0.3);
            }
            if px.roll(17) < 0.04 {
                c = mixf(c, pal.stone, 0.14);
            }
            if near.within(1, |t| t == Terrain::Wall) {
                c = mixf(c, pal.ink, 0.3);
            }
            c
        }
    }
}

/// Loose stones: on rock, and as scree on the open ground below a rock face or cave wall.
fn pebble(px: &Px, t: Terrain, pal: &Palette) -> Option<[f32; 3]> {
    if !matches!(t, Terrain::Rock | Terrain::Grass | Terrain::Sand | Terrain::Snow | Terrain::Floor) {
        return None;
    }
    const GRID: i64 = 9;
    let (seed, p) = (px.seed, px.p);
    let (gx, gy) = (p.0.div_euclid(GRID), p.1.div_euclid(GRID));
    for cy in gy - 1..=gy + 1 {
        for cx in gx - 1..=gx + 1 {
            let roll = lattice(seed, 64, cx, cy);
            if roll > 0.5 {
                continue;
            }
            let center = (
                cx * GRID + 1 + (lattice(seed, 65, cx, cy) * 7.0) as i64,
                cy * GRID + 1 + (lattice(seed, 66, cx, cy) * 7.0) as i64,
            );
            let rx = 2.0 + lattice(seed, 67, cx, cy) * 1.4;
            let ry = rx * 0.7;
            let (dx, dy) = ((p.0 - center.0) as f32, (p.1 - center.1) as f32);
            if (dx / rx).powi(2) + (dy / ry).powi(2) > 1.0 || (px.terrain)(center) != t {
                continue;
            }
            let scree = (1..=5).any(|k| (px.terrain)((center.0, center.1 - k)).high());
            if !(t == Terrain::Rock && roll < 0.08 || t != Terrain::Rock && scree) {
                continue;
            }
            let edge = (dx / rx).powi(2) + (dy / ry).powi(2) > 0.55;
            return Some(if edge && dy >= 0.0 {
                mixf(f(pal.rock), pal.ink, 0.55)
            } else if dy < -ry * 0.2 {
                mixf(f(pal.stone), WHITE, 0.1)
            } else {
                mixf(f(pal.rock), pal.stone, 0.45)
            });
        }
    }
    None
}

/// Rock and walls cast a short shadow down and to the right.
fn shadowed(px: &Px, t: Terrain, pal: &Palette, c: [f32; 3]) -> [f32; 3] {
    if t.high() {
        return c;
    }
    let casts = |k: i64| (px.terrain)((px.p.0, px.p.1 - k)).high() || (px.terrain)((px.p.0 - k, px.p.1 - k)).high();
    match (1..=3).find(|&k| casts(k)) {
        Some(k) => mixf(c, pal.ink, 0.36 - 0.1 * k as f32),
        None => c,
    }
}

/// Lava seams glow on basalt cave floor.
fn ember(seed: u64, mood: Mood, t: Terrain, p: (i64, i64)) -> bool {
    t == Terrain::Floor && mood.stone_at(seed, p) == Some(Stone::Basalt) && mood::seam(seed, p)
}

fn finish(seed: u64, mood: Mood, p: (i64, i64), c: [f32; 3]) -> [u8; 4] {
    // A slow brightness drift across wide fields.
    let drift = 1.0 + noise(seed, 15, 72.0, p) * 0.035;
    let [r, g, b] = c.map(|v| (v * drift).round().clamp(0.0, 255.0) as u8);
    mood::grade(seed, mood, Some(p), [r, g, b, 255])
}

/// The cell that paints `p` and its palette: the Deep's stone, or the picked neighbor's look.
fn look_at(seed: u64, mood: Mood, p: (i64, i64), world: World, terrain: Ground) -> (Spot, &'static Palette) {
    let deep = matches!(mood, Mood::Deep(..));
    let q = ground_point(seed, p, deep, |x, y| world(x, y).terrain);
    let spot = pick(seed, p, q, terrain(p), world);
    (spot, mood.stone_at(seed, p).map_or_else(|| paint::surface(spot.style), paint::stone))
}

/// World-anchored marks: sand ripples, grass tufts, and reeds on a shared 9px lattice, so a tuft can cross a
/// cell edge without being cut in half. `roll` against a biome's density decides whether a mark grows there.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Mark {
    pos: (i64, i64),
    flip: bool,
    roll: f32,
    patch: f32,
}

impl Mark {
    fn grows(self, flavor: Flavor) -> bool {
        let (base, patchy) = match flavor {
            Flavor::Lush => (0.48, 0.12),
            Flavor::Marsh => (0.3, 0.15),
            Flavor::Dry => (0.13, 0.12),
            Flavor::Plain | Flavor::Dunes => (0.22, 0.19),
        };
        self.roll <= base + self.patch * patchy
    }

    fn reed(self, flavor: Flavor) -> bool {
        flavor == Flavor::Marsh && self.roll < 0.17
    }
}

/// The densest marks any biome grows.
const MAX_ROLL: f32 = 0.62;

/// Marks whose shape can reach the `side`-pixel square at `origin`.
fn marks(seed: u64, origin: (i64, i64), side: i64) -> Vec<Mark> {
    let mut out = Vec::new();
    for gy in (origin.1 - 9).div_euclid(9)..=(origin.1 + side + 8).div_euclid(9) {
        for gx in (origin.0 - 10).div_euclid(9)..=(origin.0 + side + 3).div_euclid(9) {
            let roll = lattice(seed, 23, gx, gy);
            if roll > MAX_ROLL {
                continue;
            }
            let pos =
                (gx * 9 + (lattice(seed, 20, gx, gy) * 7.0) as i64, gy * 9 + (lattice(seed, 21, gx, gy) * 7.0) as i64);
            let patch = noise(seed, 22, 48.0, pos);
            out.push(Mark { pos, flip: lattice(seed, 24, gx, gy) < 0.5, roll, patch });
        }
    }
    out
}

/// A mark only grows where its whole footprint shows `kind`, so blades never bleed onto paths.
fn mark_fits(mark: Mark, kind: Terrain, reed: bool, terrain: Ground) -> bool {
    let tall: &[(i64, i64)] = if reed { &[(0, -7), (2, 0)] } else { &[] };
    [(0, 0), (-3, 0), (3, 0), (0, -4), (0, 1)]
        .iter()
        .chain(tall)
        .all(|&(dx, dy)| terrain((mark.pos.0 + dx, mark.pos.1 + dy)) == kind)
}

/// A sand ripple's shade at `p`, if it covers `p`.
fn ripple_shade(mark: Mark, p: (i64, i64)) -> bool {
    let (mut x, y) = (p.0 - mark.pos.0, p.1 - mark.pos.1);
    if mark.flip {
        x = -x;
    }
    matches!((x, y), (-2..=-1, 0) | (0..=2, -1))
}

/// Which part of a tuft covers `p`, with its upper blades leaning by `lean`: 0 dark, 1 blade, 2 tip.
fn tuft_part(mark: Mark, p: (i64, i64), lean: i64) -> Option<u8> {
    let y = p.1 - mark.pos.1;
    let mut x = p.0 - mark.pos.0 - if y <= -2 { lean } else { 0 };
    if mark.flip {
        x = -x;
    }
    match (x, y) {
        (0, 0..=0) | (0, -3..=-1) | (1, -4) => Some(1),
        (-2, -2..=-1) | (-1, 0) | (1, 0) => Some(0),
        (2, -2) => Some(2),
        _ => None,
    }
}

/// Which part of a reed clump covers `p`: two stalks, one with a seed head (3).
fn reed_part(mark: Mark, p: (i64, i64), lean: i64) -> Option<u8> {
    let y = p.1 - mark.pos.1;
    let mut x = p.0 - mark.pos.0 - if y <= -4 { lean } else { 0 };
    if mark.flip {
        x = -x;
    }
    match (x, y) {
        (0, -7..=-6) => Some(3),
        (0, -5..=0) | (2, -3..=0) => Some(1),
        (2, -4) | (-1, -3) => Some(2),
        (-1, 0) | (1, 0) | (3, 0) => Some(0),
        _ => None,
    }
}

/// How far a tuft's blade tips lean, -1 to 1 pixel, at an animation frame. A wave rolls across the world, and
/// slow gusts make some meadows livelier than others. It depends only on the tuft's world position, so a tuft
/// straddling a cell edge leans the same way in both cells.
fn sway(seed: u64, mark: Mark, frame: u8, wind: u8) -> i64 {
    let gust = 1.1 + 0.4 * value_noise(seed, 25, mark.pos.0 as f32 / 160.0, mark.pos.1 as f32 / 160.0);
    let travel = (mark.pos.0 as f32 + 0.5 * mark.pos.1 as f32) / 96.0;
    let wave = TAU * (f32::from(frame) / f32::from(FRAMES) - travel);
    (f32::from(wind) / 10.0 * gust * wave.sin()).round().clamp(-1.0, 1.0) as i64
}

/// A ground pixel. `terrain` answers [`terrain_at`] (the renderer caches it), and `marks` holds every mark that
/// can reach `p`.
fn ground_pixel(seed: u64, mood: Mood, p: (i64, i64), world: World, terrain: Ground, marks: &[Mark]) -> [u8; 4] {
    let t = terrain(p);
    if ember(seed, mood, t, p) {
        let [r, g, b] = mood::EMBER;
        return [r, g, b, 255];
    }
    let stone = mood.stone_at(seed, p);
    let deep = stone.is_some();
    let q = ground_point(seed, p, deep, |x, y| world(x, y).terrain);
    let spot = pick(seed, p, q, t, world);
    if t == Terrain::Sand
        && spot.flavor != Flavor::Dunes
        && let Some(&mark) = marks
            .iter()
            .rev()
            .find(|&&m| m.grows(Flavor::Plain) && ripple_shade(m, p) && mark_fits(m, Terrain::Sand, false, terrain))
    {
        let (_, pal) = look_at(seed, mood, mark.pos, world, terrain);
        return finish(seed, mood, p, f(paint::mix(pal.sand, pal.soil, 0.4)));
    }
    let pal = stone.map_or_else(|| paint::surface(spot.style), paint::stone);
    let tint = match t {
        _ if deep => None,
        Terrain::Water | Terrain::Rock => tint_at(q, t, world),
        _ => spot.tint.map(f),
    };
    let px = Px { seed, p, q, near: Near::of(p, t, terrain), tint, flavor: spot.flavor, terrain, world };
    let c = pebble(&px, t, pal).unwrap_or_else(|| material(t, pal, stone, &px));
    finish(seed, mood, p, shadowed(&px, t, pal, c))
}

/// A pixel of the swaying tuft layer, or None where no tuft covers `p`.
#[expect(clippy::too_many_arguments, reason = "one argument per painting input")]
fn tuft_pixel(
    seed: u64,
    mood: Mood,
    p: (i64, i64),
    frame: u8,
    wind: u8,
    world: World,
    terrain: Ground,
    marks: &[Mark],
) -> Option<[u8; 4]> {
    if terrain(p) != Terrain::Grass {
        return None;
    }
    for &mark in marks.iter().rev() {
        let (dx, dy) = (p.0 - mark.pos.0, p.1 - mark.pos.1);
        if dx.abs() > 4 || !(-7..=0).contains(&dy) || terrain(mark.pos) != Terrain::Grass {
            continue;
        }
        let (spot, pal) = look_at(seed, mood, mark.pos, world, terrain);
        if !mark.grows(spot.flavor) {
            continue;
        }
        let reed = mark.reed(spot.flavor);
        let lean = sway(seed, mark, frame, wind);
        let part = if reed { reed_part(mark, p, lean) } else { tuft_part(mark, p, lean) };
        let Some(part) = part.filter(|_| mark_fits(mark, Terrain::Grass, reed, terrain)) else { continue };
        let (dark, blade, tip) = match spot.flavor {
            Flavor::Lush => (paint::mix(pal.grassdark, pal.ink, 0.3), paint::mix(pal.leaf, pal.blade, 0.4), pal.leafhi),
            Flavor::Dry => (
                paint::mix(pal.grassdark, pal.soil, 0.45),
                paint::mix(pal.blade, pal.sand, 0.45),
                paint::mix(pal.sand, WHITE, 0.15),
            ),
            _ => (pal.grassdark, paint::mix(pal.blade, pal.grass, 0.25), pal.blade),
        };
        let c = match part {
            0 => dark,
            1 => blade,
            2 => tip,
            _ => pal.bark,
        };
        return Some(finish(seed, mood, p, f(c)));
    }
    None
}

/// A pixel of the glittering water layer at an animation frame: ripple crests brighten and fade in slow,
/// uneven waves, and sparse glints twinkle on open water.
fn shimmer_pixel(seed: u64, mood: Mood, p: (i64, i64), frame: u8, world: World, terrain: Ground) -> Option<[u8; 4]> {
    let t = terrain(p);
    if t != Terrain::Water || mood.stone_at(seed, p) == Some(Stone::Basalt) || Near::of(p, t, terrain).0.is_some() {
        return None;
    }
    let deep = matches!(mood, Mood::Deep(..));
    let q = ground_point(seed, p, deep, |x, y| world(x, y).terrain);
    let (_, pal) = look_at(seed, mood, p, world, terrain);
    let phase = TAU * f32::from(frame) / f32::from(FRAMES);
    const GRID: i64 = 11;
    let (gx, gy) = (p.0.div_euclid(GRID), p.1.div_euclid(GRID));
    if lattice(seed, 74, gx, gy) < 0.3 && openness(q, world) > 0.6 {
        let at = (
            gx * GRID + 2 + (lattice(seed, 75, gx, gy) * 7.0) as i64,
            gy * GRID + 2 + (lattice(seed, 76, gx, gy) * 7.0) as i64,
        );
        let on = ((lattice(seed, 77, gx, gy) * f32::from(FRAMES)) as u8 + frame) % FRAMES;
        let (dx, dy) = ((p.0 - at.0).abs(), (p.1 - at.1).abs());
        if on == 0 && dx + dy <= 1 && (dx == 0 || dy == 0) || on == 1 && dx + dy == 0 {
            let [r, g, b] = paint::mix(pal.ripple, WHITE, 0.6);
            return Some(mood::grade(seed, mood, Some(p), [r, g, b, 220]));
        }
    }
    let wave = crest(seed, q);
    if wave <= 0.82 {
        return None;
    }
    let glow = 0.5 + 0.5 * (phase + noise(seed, 69, 26.0, q) * 3.0).cos();
    let alpha = (glow * (wave - 0.82) / 0.18 * 3.0).round() as u8 * 48;
    let [r, g, b] = paint::mix(pal.ripple, WHITE, 0.3);
    (alpha > 0).then(|| mood::grade(seed, mood, Some(p), [r, g, b, alpha]))
}

/// A pixel of drifting mist, `step` drift steps along, for a sky `mist` eighths misty.
fn mist_pixel(seed: u64, p: (i64, i64), step: u32, mist: u8) -> Option<[u8; 4]> {
    let s = step as f32;
    let (x, y) = (p.0 as f32 - s, p.1 as f32 - s * 0.35);
    let wisp = value_noise(seed, 70, x / 44.0, y / 26.0) + 0.5 * value_noise(seed, 71, (x - s * 0.6) / 17.0, y / 11.0);
    let alpha = ((wisp - 0.15) * 0.9).clamp(0.0, 0.6) * f32::from(mist) / 8.0;
    let alpha = (alpha * 8.0).round() as u8 * 16;
    (alpha > 0).then_some([206, 214, 218, alpha])
}

/// How dark each kind of cell is drawn: in sight, remembered, and unknown.
const DARK: [f32; 3] = [0.0, 0.65, 1.0];

/// How opaque the darkness over cell `pos` is at world pixel `p`, given how dark it and its neighbors are
/// (`levels`, 0 in sight, 1 remembered, 2 unknown, rows north to south). Like natural ground, the edge of
/// sight blends the nearest cell centers at a warped point, so a staircase of cells reads as one wobbling
/// curve rather than square steps; it depends only on world position and the cells' levels, so neighboring
/// cells agree. Unknown cells are drawn black, so darkness reaches full at their edges and rounds off
/// their corners, but never lightens them.
fn shade_alpha(seed: u64, p: (i64, i64), pos: (i32, i32), levels: [[u8; 3]; 3]) -> f32 {
    let level = |(cx, cy): (i64, i64)| {
        let (r, c) = ((cy - i64::from(pos.1) + 1).clamp(0, 2), (cx - i64::from(pos.0) + 1).clamp(0, 2));
        levels[r as usize][c as usize]
    };
    let (x, y) = ((p.0 - i64::from(pos.0) * SIDE) as f32 + 0.5, (p.1 - i64::from(pos.1) * SIDE) as f32 + 0.5);
    let rag = 0.7 * noise(seed, 72, 8.0, p) + 0.3 * noise(seed, 73, 3.0, p);
    let smooth = |t: f32| {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let (wx, wy) = warp(seed, p);
    let (mut dim, mut unknown) = (0.0, 0.0);
    for (center, w) in corners((p.0 + wx, p.1 + wy)) {
        match level(center) {
            1 => dim += w,
            2 => {
                dim += w;
                unknown += w;
            }
            _ => {}
        }
    }
    let memory = DARK[1] * smooth((dim - 0.5 + 0.2 * rag) / 0.36 + 0.5);
    // Unknown cells cannot be lightened, so their darkness reaches full a little inside the known side: the
    // inner corner of a step then fills along a curve, rather than the black square's corner showing.
    let mut light = 1.0 - smooth((unknown * (1.0 + 0.4 * rag) - 0.04) / 0.24);
    // Full darkness right at an unknown cell's edge and corner, so its black square never shows a seam. The
    // falloffs from each side compound, which rounds the outer corner of a patch of known land.
    for (r, row) in levels.iter().enumerate() {
        for (c, &l) in row.iter().enumerate() {
            if l != 2 {
                continue;
            }
            let along = |i: usize, v: f32| match i {
                0 => v,
                2 => SIDE as f32 - v,
                _ => 0.0,
            };
            let d = along(c, x).hypot(along(r, y));
            light *= smooth((d - 6.0 * (rag + 1.0)) / 12.0);
        }
    }
    let dark = memory.max(1.0 - light);
    const BAYER: [[f32; 4]; 4] =
        [[0.0, 8.0, 2.0, 10.0], [12.0, 4.0, 14.0, 6.0], [3.0, 11.0, 1.0, 9.0], [15.0, 7.0, 13.0, 5.0]];
    let dither = (BAYER[p.1.rem_euclid(4) as usize][p.0.rem_euclid(4) as usize] + 0.5) / 16.0;
    ((dark * 8.0 + dither - 0.5).round() / 8.0).clamp(0.0, 1.0)
}

/// Terrain over a cell and an eight-pixel margin, which edge shading and marks reach into.
struct Field {
    origin: (i64, i64),
    terrains: Vec<Terrain>,
}

const MARGIN: i64 = 8;
const SPAN: i64 = SIDE + 2 * MARGIN;

impl Field {
    fn new(seed: u64, cell: &GroundCell, deep: bool) -> Field {
        let origin = (i64::from(cell.pos.0) * SIDE - MARGIN, i64::from(cell.pos.1) * SIDE - MARGIN);
        let world = |x, y| cell.spot(x, y);
        let terrains = (0..SPAN * SPAN)
            .map(|i| terrain_at(seed, (origin.0 + i % SPAN, origin.1 + i / SPAN), deep, &world))
            .collect();
        Field { origin, terrains }
    }

    fn at(&self, p: (i64, i64)) -> Terrain {
        let (x, y) = (p.0 - self.origin.0, p.1 - self.origin.1);
        assert!((0..SPAN).contains(&x) && (0..SPAN).contains(&y), "painting stays within the margin");
        self.terrains[(y * SPAN + x) as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Source {
    Ground(GroundCell),
    Object {
        object: Object,
        mirrored: bool,
        size: u8,
        style: Style,
    },
    Hero(Look),
    /// The grass tufts over a cell's ground at one animation frame, in tenths of wind.
    Tufts {
        cell: GroundCell,
        frame: u8,
        wind: u8,
    },
    /// The bright core of a cell's lava seams, faded in and out over the ground's dark embers.
    Heat(GroundCell),
    /// The glitter over a cell's water at one animation frame.
    Shimmer {
        cell: GroundCell,
        frame: u8,
    },
    /// Wisps of mist over a cell, some steps into their drift.
    Mist {
        pos: (i32, i32),
        step: u32,
        mist: u8,
    },
    /// The soft edge of darker cells around a cell; see [`shade_alpha`].
    Shade {
        pos: (i32, i32),
        levels: [[u8; 3]; 3],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ImageKey {
    source: Source,
    remembered: bool,
    lit: bool,
    mood: Mood,
}

/// The scale- and visibility-bounded images used by the current frame.
pub struct ArtCache {
    seed: u64,
    images: HashMap<ImageKey, Arc<RenderImage>>,
    /// Transparent layers; None when a cell has nothing to draw in them.
    overlays: HashMap<ImageKey, Option<Arc<RenderImage>>>,
    used: HashSet<ImageKey>,
    /// Whether each object and size rises above its cell.
    rising: HashMap<(Object, u8), bool>,
    mood: Mood,
    scale: f32,
    factor: usize,
    device_side: usize,
}

impl ArtCache {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            images: HashMap::new(),
            overlays: HashMap::new(),
            used: HashSet::new(),
            rising: HashMap::new(),
            mood: Mood::default(),
            scale: 1.0,
            factor: 1,
            device_side: DISPLAY_SIDE as usize,
        }
    }

    /// Drop stale GPU images at a display-scale transition and begin tracking this frame's artwork under `mood`.
    /// Images of a previous mood fall out of use and are released at the end of the frame.
    pub fn begin(&mut self, window: &mut Window, mood: Mood) {
        self.mood = mood;
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
                let overlays = self.overlays.drain().filter_map(|(_, image)| image);
                self.images.drain().map(|(_, image)| image).chain(overlays).collect()
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

    /// Device pixels of headroom above an object's cell.
    fn head(&self) -> usize {
        head_rows(self.device_side, self.factor)
    }

    /// An object image's logical width, height, and offset from the top of its cell (negative: it rises
    /// above).
    pub fn object_frame(&self) -> (f32, f32, f32) {
        let head = self.head() as f32;
        (self.display_size(), (self.device_side as f32 + head) / self.scale, -head / self.scale)
    }

    /// A cell's painted ground, at full brightness: [`Self::shade`] dims what is only remembered.
    pub fn ground(&mut self, cell: GroundCell, lit: bool) -> Arc<RenderImage> {
        self.get(ImageKey { source: Source::Ground(cell), remembered: false, lit, mood: self.mood })
    }

    /// A terrain object over the ground in its biome's look, optionally mirrored left to right. Place it with
    /// [`Self::object_frame`]. It is never dimmed itself: [`Self::shade`] dims it along with its ground.
    pub fn object(&mut self, art: &TerrainArt, object: Object, lit: bool) -> Arc<RenderImage> {
        let source = Source::Object { object, mirrored: art.mirrored, size: art.size, style: art.style };
        self.get(ImageKey { source, remembered: false, lit, mood: self.mood })
    }

    /// Whether a terrain object's picture rises into the cell to its north.
    pub fn rises(&mut self, art: &TerrainArt, object: Object) -> bool {
        *self.rising.entry((object, art.size)).or_insert_with(|| {
            let image = paint::object(object, paint::surface(Style::Moss), false, false, art.size);
            image.rows().take(HEAD as usize).flatten().any(|p| p[3] != 0)
        })
    }

    /// A ground item's icon, placed like an object. Items keep their colors in every weather so they stay easy
    /// to spot.
    pub fn item(&mut self, object: Object) -> Arc<RenderImage> {
        let source = Source::Object { object, mirrored: false, size: 1, style: Style::Moss };
        self.get(ImageKey { source, remembered: false, lit: false, mood: Mood::default() })
    }

    /// The player's figure. It is always in sight, so it is never remembered.
    pub fn hero(&mut self, look: Look, lit: bool) -> Arc<RenderImage> {
        self.get(ImageKey { source: Source::Hero(look), remembered: false, lit, mood: Mood::default() })
    }

    /// One frame of an animated overlay. `still` (remembered) cells hold still; every frame of a visible cell
    /// stays cached, so the animation cycles through ready images.
    fn animated(&mut self, key: impl Fn(u8) -> ImageKey, frame: u8, still: bool) -> Option<Arc<RenderImage>> {
        let image = self.overlay(key(if still { 0 } else { frame % FRAMES }));
        if !still {
            for other in 0..FRAMES {
                self.used.insert(key(other));
            }
        }
        image
    }

    /// A cell's grass tufts at animation `frame`, or None without any.
    pub fn tufts(&mut self, cell: GroundCell, frame: u8, still: bool, lit: bool) -> Option<Arc<RenderImage>> {
        let wind = self.mood.wind();
        if wind == 0 {
            return None;
        }
        let mood = self.mood;
        let key = |frame| ImageKey { source: Source::Tufts { cell, frame, wind }, remembered: false, lit, mood };
        self.animated(key, frame, still)
    }

    /// The glitter on a cell's water at animation `frame`, or None without water.
    pub fn shimmer(&mut self, cell: GroundCell, frame: u8, still: bool, lit: bool) -> Option<Arc<RenderImage>> {
        if !cell.around[1..4].iter().any(|row| row[1..4].contains(&Terrain::Water)) {
            return None;
        }
        let mood = self.mood;
        let key = |frame| ImageKey { source: Source::Shimmer { cell, frame }, remembered: false, lit, mood };
        self.animated(key, frame, still)
    }

    /// Mist wisps over the cell at `pos`, `step` steps into their drift, or None under a sky without mist.
    pub fn mist(&mut self, pos: (i32, i32), step: u32) -> Option<Arc<RenderImage>> {
        let Mood::Surface(sky) = self.mood else { return None };
        if sky.mist == 0 {
            return None;
        }
        let source = Source::Mist { pos, step, mist: sky.mist };
        self.overlay(ImageKey { source, remembered: false, lit: false, mood: Mood::default() })
    }

    /// The darkness over a known cell at `pos`: none in sight, dim where remembered, with soft, ragged edges
    /// toward cells of other kinds (see [`shade_alpha`]). None for a cell in sight among cells in sight.
    pub fn shade(&mut self, pos: (i32, i32), levels: [[u8; 3]; 3]) -> Option<Arc<RenderImage>> {
        if levels.iter().flatten().all(|&level| level == 0) {
            return None;
        }
        self.overlay(ImageKey {
            source: Source::Shade { pos, levels },
            remembered: false,
            lit: false,
            mood: Mood::default(),
        })
    }

    /// The glowing core of a basalt floor's lava seams, or None where the cell has none.
    pub fn heat(&mut self, cell: GroundCell, lit: bool) -> Option<Arc<RenderImage>> {
        if !matches!(self.mood, Mood::Deep(Stone::Basalt, _) | Mood::Deep(_, Stone::Basalt)) {
            return None;
        }
        self.overlay(ImageKey { source: Source::Heat(cell), remembered: false, lit, mood: self.mood })
    }

    #[cfg(test)]
    fn native(&self, source: Source) -> RgbaImage {
        self.native_in(source, Mood::default())
    }

    fn native_in(&self, source: Source, mood: Mood) -> RgbaImage {
        let mut out = RgbaImage::new(NATIVE_SIDE as u32, NATIVE_SIDE as u32);
        let seed = self.seed;
        let deep = matches!(mood, Mood::Deep(..));
        let pixels = |pos: (i32, i32)| {
            let origin = (i64::from(pos.0) * SIDE, i64::from(pos.1) * SIDE);
            (0..SIDE * SIDE).map(move |i| (i, (origin.0 + i % SIDE, origin.1 + i / SIDE)))
        };
        let put = |out: &mut RgbaImage, i: i64, pixel: [u8; 4]| {
            out.put_pixel((i % SIDE) as u32, (i / SIDE) as u32, image::Rgba(pixel));
        };
        let marks_of =
            |cell: &GroundCell| marks(seed, (i64::from(cell.pos.0) * SIDE, i64::from(cell.pos.1) * SIDE), SIDE);
        match source {
            Source::Ground(cell) => {
                let field = Field::new(seed, &cell, deep);
                let world = |x, y| cell.spot(x, y);
                let marks = marks_of(&cell);
                for (i, p) in pixels(cell.pos) {
                    put(&mut out, i, ground_pixel(seed, mood, p, &world, &|p| field.at(p), &marks));
                }
            }
            Source::Tufts { cell, frame, wind } => {
                let field = Field::new(seed, &cell, deep);
                let world = |x, y| cell.spot(x, y);
                let marks = marks_of(&cell);
                for (i, p) in pixels(cell.pos) {
                    if let Some(pixel) = tuft_pixel(seed, mood, p, frame, wind, &world, &|p| field.at(p), &marks) {
                        put(&mut out, i, pixel);
                    }
                }
            }
            Source::Shimmer { cell, frame } => {
                let field = Field::new(seed, &cell, deep);
                let world = |x, y| cell.spot(x, y);
                for (i, p) in pixels(cell.pos) {
                    if let Some(pixel) = shimmer_pixel(seed, mood, p, frame, &world, &|p| field.at(p)) {
                        put(&mut out, i, pixel);
                    }
                }
            }
            Source::Mist { pos, step, mist } => {
                for (i, p) in pixels(pos) {
                    if let Some(pixel) = mist_pixel(seed, p, step, mist) {
                        put(&mut out, i, pixel);
                    }
                }
            }
            Source::Shade { pos, levels } => {
                for (i, p) in pixels(pos) {
                    let alpha = (shade_alpha(seed, p, pos, levels) * 255.0).round() as u8;
                    if alpha > 0 {
                        put(&mut out, i, [7, 8, 11, alpha]);
                    }
                }
            }
            Source::Heat(cell) => {
                if deep {
                    let field = Field::new(seed, &cell, deep);
                    for (i, p) in pixels(cell.pos) {
                        if ember(seed, mood, field.at(p), p) {
                            let [r, g, b] = mood::HEAT;
                            put(&mut out, i, [r, g, b, 255]);
                        }
                    }
                }
            }
            Source::Hero(look) => out = hero::draw(look),
            Source::Object { object, mirrored, size, style } => {
                let (pal, snowy, hot) = match mood {
                    Mood::Deep(main, _) => (paint::stone(main), false, main == Stone::Basalt),
                    Mood::Surface(_) => (paint::surface(style), style == Style::Hearth, false),
                };
                let mut image = paint::object(object, pal, snowy, hot, size);
                if mirrored {
                    image = image::imageops::flip_horizontal(&image);
                }
                for pixel in image.pixels_mut() {
                    pixel.0 = mood::grade(seed, mood, None, pixel.0);
                }
                out = image;
            }
        }
        out
    }

    /// The cache key space is bounded by the visible map's cells and presentation variants, and end-of-frame
    /// evicts images that left the visible map.
    fn get(&mut self, key: ImageKey) -> Arc<RenderImage> {
        if !self.images.contains_key(&key) {
            let image = self.raster(key, &self.native_in(key.source, key.mood));
            self.images.insert(key, image);
        }
        self.used.insert(key);
        self.images.get(&key).expect("image was inserted").clone()
    }

    fn overlay(&mut self, key: ImageKey) -> Option<Arc<RenderImage>> {
        if !self.overlays.contains_key(&key) {
            let source = self.native_in(key.source, key.mood);
            let image = source.pixels().any(|p| p[3] != 0).then(|| self.raster(key, &source));
            self.overlays.insert(key, image);
        }
        self.used.insert(key);
        self.overlays.get(&key).expect("overlay was inserted").clone()
    }

    fn raster(&self, key: ImageKey, source: &RgbaImage) -> Arc<RenderImage> {
        let opaque = matches!(key.source, Source::Ground(_));
        let pixels = rasterize(source, opaque, self.device_side, self.factor, None, key.remembered, key.lit);
        let height = pixels.len() / (4 * self.device_side);
        let raster = RgbaImage::from_raw(self.device_side as u32, height as u32, pixels).expect("sized raster");
        Arc::new(RenderImage::new([Frame::new(raster)]))
    }

    /// Release images no longer used by the current map frame from GPUI's texture atlas.
    pub fn end(&mut self, window: &mut Window) {
        for image in self.finish() {
            window.drop_image(image).ok();
        }
    }

    fn finish(&mut self) -> Vec<Arc<RenderImage>> {
        let stale: Vec<ImageKey> = self.images.keys().filter(|key| !self.used.contains(key)).copied().collect();
        let mut dropped: Vec<_> = stale.into_iter().filter_map(|key| self.images.remove(&key)).collect();
        let stale: Vec<ImageKey> = self.overlays.keys().filter(|key| !self.used.contains(key)).copied().collect();
        dropped.extend(stale.into_iter().filter_map(|key| self.overlays.remove(&key).flatten()));
        dropped
    }
}

/// Device rows of an object's headroom at a cell of `target_side` device pixels.
fn head_rows(target_side: usize, factor: usize) -> usize {
    if target_side < NATIVE_SIDE { HEAD as usize * target_side / NATIVE_SIDE } else { HEAD as usize * factor }
}

/// Integer-only nearest-neighbor enlargement. Fractional display scales get edge padding (or transparent
/// padding for objects), never resampling, so the resulting raster fits the device cell exactly. A source
/// taller than wide keeps its extra rows above the cell, and the raster grows by them.
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
    let tall = source.height() as usize > side;
    let head = if tall { head_rows(target_side, factor) } else { 0 };
    let enlarged = side * factor;
    // Below 1:1 scale, nearest sampling skips source pixels rather than blending them.
    let (enlarged, step) = if target_side < side { (target_side, None) } else { (enlarged, Some(factor)) };
    let pad = target_side.saturating_sub(enlarged) / 2;
    let mut output = Vec::with_capacity(target_side * (target_side + head) * 4);
    for y in 0..target_side + head {
        for x in 0..target_side {
            let inner_x = x as isize - pad as isize;
            let inner_y = y as isize - head as isize - pad as isize;
            let inside = inner_x >= 0
                && inner_x < enlarged as isize
                && inner_y >= -(head as isize)
                && inner_y < enlarged as isize;
            let pixel = if inside || opaque {
                let cx = inner_x.clamp(0, enlarged as isize - 1) as usize;
                let cy = inner_y.clamp(0, enlarged as isize - 1) as usize;
                let (sx, sy) = match step {
                    Some(factor) => (cx / factor, cy / factor),
                    None => (cx * side / target_side, cy * side / target_side),
                };
                let sy = if tall {
                    // Rows above the cell map back into the headroom, the cell's rows below it.
                    let above = (inner_y + head as isize).max(0) as usize;
                    if inner_y < 0 { above * HEAD as usize / head.max(1) } else { sy + HEAD as usize }
                } else {
                    sy
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
    use crate::paint::STYLES;
    use blah_core::{BIOMES, BiomeId};
    use isle_core::{Armor, Bow, Weapon};

    fn tint_of(terrain: Terrain, x: i32) -> Option<[u8; 3]> {
        (terrain == Terrain::Water).then_some([20, 70, 90 + (x.rem_euclid(3) * 30) as u8])
    }

    type Styled<'a> = &'a dyn Fn(i32, i32) -> (Terrain, Style);

    /// Grounds, tints and looks of a world laid out by `world`, as the view would pass them with everything known.
    fn styled_cell(world: Styled, pos: (i32, i32)) -> GroundCell {
        let at = |dx: i32, dy: i32| world(pos.0 + dx, pos.1 + dy);
        GroundCell {
            pos,
            around: [-2, -1, 0, 1, 2].map(|dy| [-2, -1, 0, 1, 2].map(|dx| at(dx, dy).0)),
            tints: [-1, 0, 1].map(|dy| [-1, 0, 1].map(|dx| tint_of(at(dx, dy).0, pos.0 + dx))),
            styles: [-1, 0, 1].map(|dy| [-1, 0, 1].map(|dx| at(dx, dy).1)),
            flavors: [[Flavor::Plain; 3]; 3],
        }
    }

    fn world_cell(world: &dyn Fn(i32, i32) -> Terrain, pos: (i32, i32)) -> GroundCell {
        styled_cell(&|x, y| (world(x, y), Style::Moss), pos)
    }

    fn spots(world: Styled<'_>) -> impl Fn(i64, i64) -> Spot + '_ {
        move |x, y| {
            let (terrain, style) = world(x as i32, y as i32);
            Spot { terrain, style, flavor: Flavor::Plain, tint: tint_of(terrain, x as i32) }
        }
    }

    /// Independent of any one cell: paint world pixel `p` from the world itself.
    fn sample_world(world: Styled, seed: u64, mood: Mood, p: (i64, i64)) -> [u8; 4] {
        let deep = matches!(mood, Mood::Deep(..));
        let spots = spots(world);
        ground_pixel(seed, mood, p, &spots, &|p| terrain_at(seed, p, deep, &spots), &marks(seed, p, 1))
    }

    fn sample_tufts(world: Styled, seed: u64, mood: Mood, p: (i64, i64), frame: u8) -> [u8; 4] {
        let spots = spots(world);
        let terrain = |p| terrain_at(seed, p, false, &spots);
        tuft_pixel(seed, mood, p, frame, 10, &spots, &terrain, &marks(seed, p, 1)).unwrap_or([0; 4])
    }

    fn mixed_world(x: i32, y: i32) -> Terrain {
        match (x.rem_euclid(5), y.rem_euclid(4)) {
            (0, _) => Terrain::Water,
            (1, 0) => Terrain::Sand,
            (2, _) | (3, 1) => Terrain::Dirt,
            (4, 3) => Terrain::Rock,
            (1, 2) => Terrain::Wall,
            (3, 3) => Terrain::Snow,
            _ => Terrain::Grass,
        }
    }

    fn mixed_styled(x: i32, y: i32) -> (Terrain, Style) {
        (mixed_world(x, y), STYLES[(x.div_euclid(2) + y.div_euclid(3)).rem_euclid(3) as usize])
    }

    fn cave(x: i32, y: i32) -> (Terrain, Style) {
        let open = (x.rem_euclid(7) < 5 && y.rem_euclid(6) < 4) || (x + y).rem_euclid(9) == 0;
        let terrain = if (x, y) == (2, 1) {
            Terrain::Water
        } else if open {
            Terrain::Floor
        } else {
            Terrain::Wall
        };
        (terrain, Style::Moss)
    }

    fn check_agreement(world: Styled, mood: Mood, cells: std::ops::Range<i32>) {
        let cache = ArtCache::new(7);
        for cy in cells.clone() {
            for cx in cells.clone() {
                let image = cache.native_in(Source::Ground(styled_cell(world, (cx, cy))), mood);
                for y in 0..32 {
                    for x in 0..32 {
                        let p = (i64::from(cx) * 32 + x, i64::from(cy) * 32 + y);
                        assert_eq!(image.get_pixel(x as u32, y as u32).0, sample_world(world, 7, mood, p), "{p:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn neighboring_cells_agree_with_one_world_wide_painting() {
        check_agreement(&mixed_styled, Mood::default(), -2..3);
        check_agreement(&mixed_styled, Mood::Surface(mood::Sky { clear: 0, mist: 6 }), -1..2);
        for stone in [Stone::Limestone, Stone::Slate, Stone::Basalt] {
            check_agreement(&cave, Mood::Deep(stone, Stone::Basalt), -1..2);
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
    fn built_ground_keeps_square_edges_and_natural_ground_rounds_off() {
        for terrain in [Terrain::Floor, Terrain::Wall, Terrain::Dirt] {
            for y in -32..32 {
                for x in -32..32 {
                    assert_eq!(ground_point(7, (x, y), false, |_, _| terrain), (x, y));
                }
            }
        }
        // A town wall cell in a meadow owns exactly its square.
        let walled = |x: i64, y: i64| Spot {
            terrain: if (x, y) == (0, 0) { Terrain::Wall } else { Terrain::Grass },
            style: Style::Moss,
            flavor: Flavor::Plain,
            tint: None,
        };
        for y in -32..64 {
            for x in -32..64 {
                let inside = (0..32).contains(&x) && (0..32).contains(&y);
                assert_eq!(terrain_at(7, (x, y), false, &walled) == Terrain::Wall, inside, "{x} {y}");
            }
        }
        // A lone pond becomes a rounded blob: its center is water and its corners are meadow.
        let pond = |x: i64, y: i64| Spot {
            terrain: if (x, y) == (0, 0) { Terrain::Water } else { Terrain::Grass },
            style: Style::Moss,
            flavor: Flavor::Plain,
            tint: None,
        };
        assert_eq!(terrain_at(7, (16, 16), false, &pond), Terrain::Water);
        for corner in [(0, 0), (31, 0), (0, 31), (31, 31)] {
            assert_eq!(terrain_at(7, corner, false, &pond), Terrain::Grass, "{corner:?}");
        }
        let wet = (0..32)
            .flat_map(|y| (0..32).map(move |x| (x, y)))
            .filter(|&p| terrain_at(7, p, false, &pond) == Terrain::Water);
        let wet = wet.count();
        assert!(wet > 200 && wet < 700, "{wet}");
        // A straight coast stays near the cell edge.
        let coast = |x: i64, _: i64| Spot {
            terrain: if x < 0 { Terrain::Water } else { Terrain::Grass },
            style: Style::Moss,
            flavor: Flavor::Plain,
            tint: None,
        };
        for y in -64..64 {
            assert_eq!(terrain_at(7, (-12, y), false, &coast), Terrain::Water);
            assert_eq!(terrain_at(7, (11, y), false, &coast), Terrain::Grass);
        }
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

    fn average(image: &RgbaImage) -> [u32; 3] {
        let n = image.pixels().count() as u32;
        [0, 1, 2].map(|i| image.pixels().map(|p| u32::from(p[i])).sum::<u32>() / n)
    }

    #[test]
    fn looks_follow_the_biome_and_meet_in_ragged_borders() {
        let cache = ArtCache::new(7);
        let field = |style: Style| move |_: i32, _: i32| (Terrain::Grass, style);
        let moss = average(&cache.native(Source::Ground(styled_cell(&field(Style::Moss), (0, 0)))));
        let fable = average(&cache.native(Source::Ground(styled_cell(&field(Style::Fable), (0, 0)))));
        let hearth = average(&cache.native(Source::Ground(styled_cell(&field(Style::Hearth), (0, 0)))));
        assert!(fable[0] > moss[0] + 20 && hearth[2] > moss[2], "{moss:?} {fable:?} {hearth:?}");
        // Halfway across a border between two looks, the color lies between them.
        let border = |x: i32, _: i32| (Terrain::Grass, if x < 0 { Style::Moss } else { Style::Fable });
        let spots = spots(&border);
        let red = |x: i64| {
            (0..64)
                .map(|y| {
                    u32::from(
                        ground_pixel(7, Mood::default(), (x, y), &spots, &|p| terrain_at(7, p, false, &spots), &[])[0],
                    )
                })
                .sum::<u32>()
                / 64
        };
        let (west, edge, east) = (red(-40), red(0), red(40));
        assert!(west < edge && edge < east, "{west} {edge} {east}");
    }

    #[test]
    fn mirrored_objects_flip_left_to_right() {
        let cache = ArtCache::new(1);
        let plain = cache.native(Source::Object { object: Object::Pine, mirrored: false, size: 1, style: Style::Moss });
        let flipped =
            cache.native(Source::Object { object: Object::Pine, mirrored: true, size: 1, style: Style::Moss });
        assert_eq!(image::imageops::flip_horizontal(&plain), flipped);
        let mirrored = (0..400).filter(|&x| terrain_art(Tile::Tree, &BIOMES[0], false, (x, 3)).mirrored).count();
        assert!(mirrored > 100 && mirrored < 300, "about half of trees are mirrored: {mirrored}");
    }

    #[test]
    fn ground_cache_reuses_images_and_evicts_old_cells_and_scales() {
        let mut cache = ArtCache::new(7);
        let cell = world_cell(&mixed_world, (0, 0));
        let first = cache.ground(cell, false);
        assert!(Arc::ptr_eq(&first, &cache.ground(cell, false)));
        let mut changed = cell;
        changed.tints[1][1] = Some([30, 80, 150]);
        assert!(!Arc::ptr_eq(&first, &cache.ground(changed, false)));
        let mut marsh = cell;
        marsh.flavors[1][1] = Flavor::Marsh;
        assert!(!Arc::ptr_eq(&first, &cache.ground(marsh, false)));
        assert!(!Arc::ptr_eq(&first, &cache.ground(cell, true)));
        assert!(cache.begin_scale(1.0).is_empty());
        cache.ground(cell, false);
        assert_eq!(cache.finish().len(), 3);
        assert_eq!(cache.images.len(), 1);
        assert_eq!(cache.begin_scale(2.0).len(), 1);
        assert!(cache.images.is_empty());
        cache.ground(cell, false);
        cache.begin_scale(2.0);
        assert_eq!(cache.finish().len(), 1);
        assert!(cache.images.is_empty());
    }

    /// Measure first-frame, cached-frame and scrolling costs at Retina scale, without GPU upload.
    #[test]
    #[ignore]
    fn ground_render_cost() {
        use std::time::Instant;
        type World = fn(i32, i32) -> (Terrain, Style);
        let worlds: [(&str, World, Mood); 5] = [
            ("mixed", mixed_styled, Mood::default()),
            ("grass", |_, _| (Terrain::Grass, Style::Moss), Mood::default()),
            ("water", |_, _| (Terrain::Water, Style::Moss), Mood::default()),
            ("mist", |_, _| (Terrain::Grass, Style::Hearth), Mood::Surface(mood::Sky { clear: 0, mist: 8 })),
            ("basalt", cave, Mood::Deep(Stone::Basalt, Stone::Slate)),
        ];
        for (name, world, mood) in worlds {
            let start = Instant::now();
            let mut cache = ArtCache::new(7);
            cache.begin_scale(2.0);
            cache.mood = mood;
            let radius = isle_core::SIGHT;
            let draw = |cache: &mut ArtCache, offset, frame| {
                for y in -radius..=radius {
                    for x in -radius..=radius {
                        let cell = styled_cell(&world, (x + offset, y));
                        std::hint::black_box(cache.ground(cell, false));
                        std::hint::black_box(cache.tufts(cell, frame, false, false));
                        std::hint::black_box(cache.heat(cell, false));
                        std::hint::black_box(cache.shimmer(cell, frame, false, false));
                        let edge = if x == radius { [[0, 0, 2]; 3] } else { [[0; 3]; 3] };
                        std::hint::black_box(cache.shade((x + offset, y), edge));
                        std::hint::black_box(cache.mist((x + offset, y), u32::from(frame)));
                    }
                }
                cache.finish();
            };
            draw(&mut cache, 0, 0);
            let cold = start.elapsed();
            let mut fill_max = std::time::Duration::ZERO;
            for frame in 1..FRAMES {
                let start = Instant::now();
                cache.begin_scale(2.0);
                draw(&mut cache, 0, frame);
                fill_max = fill_max.max(start.elapsed());
            }
            let start = Instant::now();
            cache.begin_scale(2.0);
            draw(&mut cache, 0, 0);
            let warm = start.elapsed();
            let start = Instant::now();
            cache.begin_scale(2.0);
            draw(&mut cache, 1, 0);
            let scroll = start.elapsed();
            let count = (2 * radius + 1).pow(2) as usize;
            assert_eq!(cache.images.len(), count);
            let overlays = cache.overlays.values().filter(|image| image.is_some()).count();
            eprintln!(
                "{name}: {count} cells, 2x: cold {cold:?}, sway fill max {fill_max:?}, cached {warm:?}, scroll {scroll:?}; {} raster bytes",
                (count + overlays) * 64 * 64 * 4
            );
        }
    }

    /// Writes target/isle-world-art/hero-contact.png: one row per calling, gear growing left to right.
    /// `cargo test -p isle hero_contact_sheet -- --ignored`.
    #[test]
    #[ignore]
    fn hero_contact_sheet() {
        use crate::hero::Calling;
        let rows = [
            (Calling::Wanderer, [140, 104, 66], Weapon::ShortSword),
            (Calling::Warrior, [164, 48, 44], Weapon::LongSword),
            (Calling::Warrior, [164, 48, 44], Weapon::Mace),
            (Calling::Rogue, [56, 64, 62], Weapon::Dagger),
            (Calling::Ranger, [66, 110, 54], Weapon::ShortSword),
            (Calling::Mage, [204, 90, 36], Weapon::Staff),
            (Calling::Mage, [124, 76, 178], Weapon::Staff),
            (Calling::Cleric, [234, 230, 214], Weapon::Mace),
            (Calling::Cleric, [150, 190, 226], Weapon::Staff),
        ];
        let gear = [
            (None, None, false, false),
            (Some(Armor::Leather), Some(Bow::Shortbow), false, false),
            (Some(Armor::Chainmail), None, false, true),
            (Some(Armor::Plate), Some(Bow::Longbow), true, true),
        ];
        // After the gear columns, the same figure stands on every ground and under every mood.
        let sky = |clear, mist| Mood::Surface(mood::Sky { clear, mist });
        let deep = |stone| Mood::Deep(stone, stone);
        let grounds = [
            (Terrain::Grass, Style::Moss, sky(0, 0)),
            (Terrain::Grass, Style::Fable, sky(8, 0)),
            (Terrain::Grass, Style::Hearth, sky(0, 8)),
            (Terrain::Sand, Style::Fable, sky(0, 0)),
            (Terrain::Dirt, Style::Moss, sky(0, 0)),
            (Terrain::Snow, Style::Hearth, sky(0, 0)),
            (Terrain::Water, Style::Moss, sky(0, 0)),
            (Terrain::Floor, Style::Moss, deep(Stone::Limestone)),
            (Terrain::Floor, Style::Moss, deep(Stone::Slate)),
            (Terrain::Floor, Style::Moss, deep(Stone::Basalt)),
        ];
        let cache = ArtCache::new(7);
        let columns = gear.len() + grounds.len();
        let mut out = RgbaImage::new((columns * 128) as u32, (rows.len() * 128) as u32);
        for (r, &(calling, accent, weapon)) in rows.iter().enumerate() {
            for c in 0..columns {
                let (armor, bow, ring, trim) = gear[if c < gear.len() { c } else { r % gear.len() }];
                let (terrain, style, mood) = grounds[c.saturating_sub(gear.len())];
                let world = move |_: i32, _: i32| (terrain, style);
                let ground = styled_cell(&world, (c as i32 * 3, r as i32 * 3));
                let mut cell = cache.native_in(Source::Ground(ground), mood);
                if mood.wind() > 0 {
                    let tufts = Source::Tufts { cell: ground, frame: 0, wind: mood.wind() };
                    image::imageops::overlay(&mut cell, &cache.native_in(tufts, mood), 0, 0);
                }
                image::imageops::overlay(&mut cell, &cache.native_in(Source::Heat(ground), mood), 0, 0);
                let look = Look { calling, accent, trim, weapon, bow, armor, ring };
                image::imageops::overlay(&mut cell, &cache.native(Source::Hero(look)), 0, 0);
                let big = image::imageops::resize(&cell, 128, 128, image::imageops::FilterType::Nearest);
                image::imageops::replace(&mut out, &big, (c * 128) as i64, (r * 128) as i64);
            }
        }
        save(&out, "hero-contact.png");
    }

    fn save(image: &RgbaImage, name: &str) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/isle-world-art").join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image.save(&path).unwrap();
        eprintln!("{}", path.display());
    }

    /// Preview each wilderness site in the three biome looks.
    #[test]
    #[ignore]
    fn landmark_contact_sheet() {
        let cache = ArtCache::new(7);
        let world = |x: i32, _: i32| match x / 4 {
            0 => (Terrain::Grass, Style::Moss),
            1 => (Terrain::Sand, Style::Fable),
            _ => (Terrain::Snow, Style::Hearth),
        };
        let objects = |x: i32, y: i32| {
            (x.rem_euclid(4) == 1)
                .then_some(match y {
                    1 => Some(Object::Cairn),
                    3 => Some(Object::Camp),
                    5 => Some(Object::Shrine),
                    _ => None,
                })
                .flatten()
        };
        let image = scene(&cache, &world, Mood::Surface(mood::Sky { clear: 4, mist: 0 }), (12, 7), &objects);
        save(&image, "landmark-contact.png");
    }

    /// Preview field supplies in each biome look.
    #[test]
    #[ignore]
    fn herb_contact_sheet() {
        let cache = ArtCache::new(7);
        let world = |x: i32, _: i32| match x / 4 {
            0 => (Terrain::Grass, Style::Moss),
            1 => (Terrain::Grass, Style::Hearth),
            _ => (Terrain::Sand, Style::Fable),
        };
        let objects = |x: i32, y: i32| {
            (x.rem_euclid(4) == 1)
                .then_some(match y {
                    1 => Some(Object::Herb(isle_core::Herb::Woundleaf)),
                    3 => Some(Object::Herb(isle_core::Herb::Bogmint)),
                    5 => Some(Object::Herb(isle_core::Herb::Sunroot)),
                    _ => None,
                })
                .flatten()
        };
        let image = scene(&cache, &world, Mood::Surface(mood::Sky { clear: 4, mist: 0 }), (12, 7), &objects);
        save(&image, "herb-contact.png");
    }

    /// Preview the three relic shapes on grass, sand, and snow.
    #[test]
    #[ignore]
    fn relic_contact_sheet() {
        let cache = ArtCache::new(7);
        let world = |x: i32, _: i32| match x / 4 {
            0 => (Terrain::Grass, Style::Moss),
            1 => (Terrain::Sand, Style::Fable),
            _ => (Terrain::Snow, Style::Hearth),
        };
        let objects = |x: i32, y: i32| {
            let relic = match y {
                1 => Some(isle_core::Relic::EmberCharm),
                3 => Some(isle_core::Relic::Stillstone),
                5 => Some(isle_core::Relic::HuntersToken),
                _ => None,
            };
            (x.rem_euclid(4) == 1).then_some(relic.map(Object::Relic)).flatten()
        };
        let image = scene(&cache, &world, Mood::Surface(mood::Sky { clear: 4, mist: 0 }), (12, 7), &objects);
        save(&image, "relic-contact.png");
    }

    /// Preview the occupied and quiet tower with its two guards.
    #[test]
    #[ignore]
    fn cinder_contact_sheet() {
        let cache = ArtCache::new(7);
        let world = |x: i32, _: i32| match x / 4 {
            0 => (Terrain::Grass, Style::Moss),
            1 => (Terrain::Sand, Style::Fable),
            _ => (Terrain::Snow, Style::Hearth),
        };
        let objects = |x: i32, y: i32| {
            (x.rem_euclid(4) == 1)
                .then_some(match y {
                    1 => Some(Object::Watchtower { cleared: false }),
                    3 => Some(Object::Watchtower { cleared: true }),
                    5 => Some(Object::CinderWarden),
                    7 => Some(Object::AshSentry),
                    _ => None,
                })
                .flatten()
        };
        let image = scene(&cache, &world, Mood::Surface(mood::Sky { clear: 4, mist: 0 }), (12, 9), &objects);
        save(&image, "cinder-contact.png");
    }

    /// Paint a scene of `w`×`h` cells under `mood`, with tufts, heat, and `objects` on top.
    fn scene(
        cache: &ArtCache,
        world: Styled,
        mood: Mood,
        (w, h): (i32, i32),
        objects: &dyn Fn(i32, i32) -> Option<Object>,
    ) -> RgbaImage {
        let mut out = RgbaImage::new((w * 64) as u32, (h * 64) as u32);
        for cy in 0..h {
            for cx in 0..w {
                let cell = styled_cell(world, (cx, cy));
                let mut image = cache.native_in(Source::Ground(cell), mood);
                for layer in [Source::Tufts { cell, frame: 2, wind: mood.wind() }, Source::Heat(cell)] {
                    if !matches!(layer, Source::Tufts { wind: 0, .. }) {
                        image::imageops::overlay(&mut image, &cache.native_in(layer, mood), 0, 0);
                    }
                }
                let big = image::imageops::resize(&image, 64, 64, image::imageops::FilterType::Nearest);
                image::imageops::replace(&mut out, &big, i64::from(cx * 64), i64::from(cy * 64));
            }
        }
        for cy in 0..h {
            for cx in 0..w {
                if let Some(object) = objects(cx, cy) {
                    let mirrored = mix((cx, cy)) & 32 != 0;
                    let size = if object.natural() { size_at((cx, cy), object) } else { 1 };
                    let source = Source::Object { object, mirrored, size, style: world(cx, cy).1 };
                    let image = cache.native_in(source, mood);
                    let big = image::imageops::resize(&image, 64, 88, image::imageops::FilterType::Nearest);
                    image::imageops::overlay(&mut out, &big, i64::from(cx * 64), i64::from(cy * 64 - HEAD * 2));
                }
            }
        }
        out
    }

    /// Three biomes side by side, left to right temperate, warm, and cold, with coast, a road, and a walled
    /// yard in each.
    fn contact_world(x: i32, y: i32) -> (Terrain, Style) {
        use Terrain::{Dirt, Grass, Rock, Sand, Snow, Wall, Water};
        let style = match x {
            ..=9 => Style::Moss,
            10..=19 => Style::Fable,
            _ => Style::Hearth,
        };
        let shore = 13 + ((x as f32 / 3.0).sin() * 1.4).round() as i32;
        let local = x.rem_euclid(10);
        let terrain = if y > shore {
            Water
        } else if y == shore {
            Sand
        } else if (2..6).contains(&local) && (2..5).contains(&y) {
            if (3..5).contains(&local) && y == 3 || (local, y) == (3, 4) { Dirt } else { Wall }
        } else if y == 8 || local == 3 && (5..8).contains(&y) {
            Dirt
        } else if style == Style::Fable && (y < 3 || local >= 7 && y < 7) {
            Sand
        } else if style == Style::Hearth && (y < 2 || local >= 7 && y < 5) {
            Snow
        } else if (local == 8 && (9..11).contains(&y)) || (x, y) == (5, 11) || (x, y) == (26, 6) {
            Rock
        } else if (local, y) == (1, 11) {
            Water
        } else {
            Grass
        };
        (terrain, style)
    }

    fn contact_objects(x: i32, y: i32) -> Option<Object> {
        let local = x.rem_euclid(10);
        let style = contact_world(x, y).1;
        match (local, y) {
            (6..=9, 10..=11) | (0..=1, 5..=6) | (7, 2) => Some(tree_at((x, y), style, Flavor::Plain)),
            (3, 4) => Some(Object::Door),
            (3, 3) => Some(Object::Counter),
            (4, 3) => Some(Object::Altar),
            (5, 6) => Some(Object::Boulder),
            (6, 8) => Some(Object::StairsDown),
            (0, 8) => Some(Object::StairsUp),
            (2, 10) => Some(ALL_OBJECTS[11 + (x / 10) as usize]),
            (4, 10) => Some(ALL_OBJECTS[14 + (x / 10) as usize]),
            (6, 12) => Some(ALL_OBJECTS[17 + (x / 10).min(1) as usize]),
            _ => None,
        }
    }

    /// Writes target/isle-world-art/paint-contact.png from the real renderer:
    /// `cargo test -p isle contact_sheet -- --ignored`.
    #[test]
    #[ignore]
    fn contact_sheet() {
        let cache = ArtCache::new(7);
        let out = scene(&cache, &contact_world, Mood::default(), (30, 16), &contact_objects);
        save(&out, "paint-contact.png");
    }

    /// Writes target/isle-world-art/mood-contact.png: one scene under each sky, then a Deep floor in each stone.
    /// `cargo test -p isle mood_contact_sheet -- --ignored`.
    #[test]
    #[ignore]
    fn mood_contact_sheet() {
        use mood::Sky;
        let cache = ArtCache::new(7);
        let (w, h) = (16, 16);
        let deep = |x: i32, y: i32| {
            let room = (2..9).contains(&x) && (2..9).contains(&y) || (10..15).contains(&x) && (4..14).contains(&y);
            let open = room || y == 6 && (8..11).contains(&x);
            let t = if (x, y) == (12, 10) || (x, y) == (13, 10) {
                Terrain::Water
            } else if open {
                Terrain::Floor
            } else {
                Terrain::Wall
            };
            (t, Style::Moss)
        };
        let deep_objects = |x: i32, y: i32| match (x, y) {
            (3, 3) => Some(Object::StairsUp),
            (12, 12) => Some(Object::StairsDown),
            (6, 6) | (11, 6) => Some(Object::Boulder),
            (7, 3) => Some(Object::Altar),
            _ => None,
        };
        let skies = [Sky::default(), Sky { clear: 8, mist: 0 }, Sky { clear: 0, mist: 8 }];
        let stones = [Stone::Limestone, Stone::Slate, Stone::Basalt];
        let mut out = RgbaImage::new((3 * w * 64) as u32, (2 * h * 64) as u32);
        for (i, sky) in skies.into_iter().enumerate() {
            let shifted = |x: i32, y: i32| contact_world(x + 4 + 10 * i as i32, y);
            let objects = |x: i32, y: i32| contact_objects(x + 4 + 10 * i as i32, y);
            let panel = scene(&cache, &shifted, Mood::Surface(sky), (w, h), &objects);
            image::imageops::replace(&mut out, &panel, i64::from(i as i32 * w * 64), 0);
        }
        for (i, stone) in stones.into_iter().enumerate() {
            let panel = scene(&cache, &deep, Mood::Deep(stone, stone), (w, h), &deep_objects);
            image::imageops::replace(&mut out, &panel, i64::from(i as i32 * w * 64), i64::from(h * 64));
        }
        save(&out, "mood-contact.png");
    }

    /// Writes target/isle-world-art/biome-contact.png: a strip of real biomes with their trees, ponds, and
    /// boulders, and below it the same land under a soft edge of sight.
    /// `cargo test -p isle biome_contact_sheet -- --ignored`.
    #[test]
    #[ignore]
    fn biome_contact_sheet() {
        let names = [
            "Grassland",
            "Forest",
            "Jungle",
            "Savanna",
            "Swamp",
            "Desert",
            "Beach",
            "Badlands",
            "Mountains",
            "Hills",
            "Taiga",
            "Snow peaks",
        ];
        let biomes: Vec<&Biome> = names.iter().map(|n| BIOMES.iter().find(|b| b.name == *n).unwrap()).collect();
        let (band, h) = (5, 10);
        let w = band * names.len() as i32;
        let art = |x: i32, y: i32| {
            let biome = biomes[(x.clamp(0, w - 1) / band) as usize];
            let roll = mix((x, y)) % 23;
            let tile = match (x.rem_euclid(band), y) {
                (2, 7) | (3, 7) => Tile::Water,
                _ if roll < 4 => Tile::Tree,
                _ if roll == 5 => Tile::Rock,
                _ => Tile::Wild(BiomeId(0)),
            };
            terrain_art(tile, biome, false, (x, y))
        };
        let cell = |pos: (i32, i32)| {
            let at = |dx: i32, dy: i32| art(pos.0 + dx, pos.1 + dy);
            GroundCell {
                pos,
                around: [-2, -1, 0, 1, 2].map(|dy| [-2, -1, 0, 1, 2].map(|dx| at(dx, dy).ground)),
                tints: [-1, 0, 1].map(|dy| [-1, 0, 1].map(|dx| at(dx, dy).tint)),
                styles: [-1, 0, 1].map(|dy| [-1, 0, 1].map(|dx| at(dx, dy).style)),
                flavors: [-1, 0, 1].map(|dy| [-1, 0, 1].map(|dx| at(dx, dy).flavor)),
            }
        };
        // Sight in the lower half: a ragged region in view, remembered land around it, and the unknown beyond.
        let level = |x: i32, y: i32| -> u8 {
            if y < h {
                return 0;
            }
            let d = ((x % 12 - 6).pow(2) + (y - h - 5).pow(2)) as f32;
            let blocked = mix((x / 3, y / 2)).is_multiple_of(5);
            if d < 12.0 && !blocked {
                0
            } else if d < 26.0 {
                1
            } else {
                2
            }
        };
        let cache = ArtCache::new(7);
        let mood = Mood::default();
        let mut out = RgbaImage::new((w * 64) as u32, (2 * h * 64) as u32);
        for y in 0..2 * h {
            for x in 0..w {
                let own = level(x, y);
                if own == 2 {
                    let black = RgbaImage::from_pixel(64, 64, image::Rgba([7, 8, 11, 255]));
                    image::imageops::replace(&mut out, &black, i64::from(x * 64), i64::from(y * 64));
                    continue;
                }
                let ground = cell((x, y % h));
                let mut image = cache.native_in(Source::Ground(ground), mood);
                let tufts = Source::Tufts { cell: ground, frame: 2, wind: mood.wind() };
                image::imageops::overlay(&mut image, &cache.native_in(tufts, mood), 0, 0);
                let shimmer = Source::Shimmer { cell: ground, frame: 0 };
                image::imageops::overlay(&mut image, &cache.native_in(shimmer, mood), 0, 0);
                let big = image::imageops::resize(&image, 64, 64, image::imageops::FilterType::Nearest);
                image::imageops::replace(&mut out, &big, i64::from(x * 64), i64::from(y * 64));
            }
        }
        // Objects last, row by row, so tall crowns overlap the cell above as on the map.
        for y in 0..2 * h {
            for x in 0..w {
                let own = level(x, y);
                let terrain = art(x, y % h);
                let Some(object) = terrain.object.filter(|_| own < 2) else { continue };
                let source =
                    Source::Object { object, mirrored: terrain.mirrored, size: terrain.size, style: terrain.style };
                let image = cache.native_in(source, mood);
                let big = image::imageops::resize(&image, 64, 88, image::imageops::FilterType::Nearest);
                image::imageops::overlay(&mut out, &big, i64::from(x * 64), i64::from(y * 64 - 24));
            }
        }
        // The fog over everything, as on the map: crowns rising into unknown land are hidden there too.
        for y in 0..2 * h {
            for x in 0..w {
                let (cx, cy) = (i64::from(x * 64), i64::from(y * 64));
                if level(x, y) == 2 {
                    let black = RgbaImage::from_pixel(64, 64, image::Rgba([7, 8, 11, 255]));
                    image::imageops::replace(&mut out, &black, cx, cy);
                    continue;
                }
                let levels = [-1, 0, 1].map(|dy| [-1, 0, 1].map(|dx| level(x + dx, y + dy)));
                let shade = cache.native_in(Source::Shade { pos: (x, y), levels }, mood);
                let big = image::imageops::resize(&shade, 64, 64, image::imageops::FilterType::Nearest);
                image::imageops::overlay(&mut out, &big, cx, cy);
            }
        }
        save(&out, "biome-contact.png");
    }
    #[test]
    fn tufts_sway_in_the_wind_stay_on_grass_and_agree_across_cell_edges() {
        let cache = ArtCache::new(7);
        let tufts = |world: Styled, pos, frame, wind| {
            cache.native(Source::Tufts { cell: styled_cell(world, pos), frame, wind })
        };
        let grass = |_: i32, _: i32| (Terrain::Grass, Style::Moss);
        let windy: Vec<_> = (0..FRAMES).map(|f| tufts(&grass, (0, 0), f, 10)).collect();
        assert!(windy.iter().any(|f| *f != windy[0]), "wind moves the blades");
        assert!(windy.iter().all(|f| f.pixels().filter(|p| p[3] != 0).count() > 0));
        assert!((0..FRAMES).all(|f| tufts(&grass, (0, 0), f, 0) == tufts(&grass, (0, 0), 0, 0)), "still air");
        for frame in [0, 3] {
            for cy in -1..2 {
                for cx in -1..2 {
                    let image = tufts(&mixed_styled, (cx, cy), frame, 10);
                    for y in 0..32 {
                        for x in 0..32 {
                            let p = (i64::from(cx) * 32 + x, i64::from(cy) * 32 + y);
                            let expected = sample_tufts(&mixed_styled, 7, Mood::default(), p, frame);
                            assert_eq!(image.get_pixel(x as u32, y as u32).0, expected, "{p:?}");
                            if expected[3] != 0 {
                                let spots = spots(&mixed_styled);
                                assert_eq!(terrain_at(7, p, false, &spots), Terrain::Grass);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_sway_frame_of_a_visible_cell_stays_cached_and_the_deep_has_no_tufts() {
        let mut cache = ArtCache::new(7);
        let cell = world_cell(&|_, _| Terrain::Grass, (0, 0));
        cache.begin_scale(1.0);
        let first = cache.tufts(cell, 0, false, false).expect("a meadow has tufts");
        cache.finish();
        cache.begin_scale(1.0);
        cache.tufts(cell, 1, false, false);
        assert!(cache.finish().is_empty(), "other frames of a visible cell are kept");
        cache.begin_scale(1.0);
        assert!(Arc::ptr_eq(&first, &cache.tufts(cell, FRAMES, false, false).unwrap()), "the cycle repeats");
        cache.mood = Mood::Deep(Stone::Slate, Stone::Slate);
        assert!(cache.tufts(cell, 0, false, false).is_none());
        cache.begin_scale(1.0);
        assert!(cache.finish().len() >= 2, "a cell that left the map releases its frames");
    }

    #[test]
    fn fog_is_clear_in_sight_dim_in_memory_and_ragged_only_near_edges() {
        let alpha = |p, levels| shade_alpha(7, p, (0, 0), levels);
        for y in 0..32 {
            for x in 0..32 {
                assert_eq!(alpha((x, y), [[0; 3]; 3]), 0.0);
                assert!((0.6..=0.75).contains(&alpha((x, y), [[1; 3]; 3])));
            }
        }
        let unknown_east = [[0, 0, 2]; 3];
        assert!((0..32).all(|y| alpha((31, y), unknown_east) == 1.0), "black right at the unknown edge");
        let ragged = (12..31).any(|x| {
            let column: Vec<_> = (0..32).map(|y| alpha((x, y), unknown_east)).collect();
            column.iter().any(|&a| a != column[0])
        });
        assert!(ragged, "the edge is ragged, not a straight gradient");
        assert!((0..32).all(|y| alpha((8, y), unknown_east) == 0.0), "the interior stays clear");
        let memory_east = [[0, 0, 1]; 3];
        assert!(
            (0..32).any(|y| {
                let a = alpha((31, y), memory_east);
                a > 0.0 && a < DARK[1]
            }),
            "sight fades into memory rather than ending at the grid"
        );
    }

    #[test]
    fn water_glitters_only_on_water_cycles_and_darkens_away_from_shore() {
        let cache = ArtCache::new(7);
        let ocean = world_cell(&|_, _| Terrain::Water, (0, 0));
        let shore = world_cell(&|x, _| if x < 0 { Terrain::Grass } else { Terrain::Water }, (0, 0));
        let wet: Vec<_> = (0..FRAMES).map(|frame| cache.native(Source::Shimmer { cell: ocean, frame })).collect();
        assert!(wet.iter().any(|a| *a != wet[0]), "water moves");
        assert!(wet.iter().any(|a| a.pixels().any(|p| p[3] > 0)), "crests catch the light");
        let land = world_cell(&|_, _| Terrain::Grass, (0, 0));
        assert!(cache.native(Source::Shimmer { cell: land, frame: 0 }).pixels().all(|p| p[3] == 0));
        let basalt = Mood::Deep(Stone::Basalt, Stone::Basalt);
        assert!(cache.native_in(Source::Shimmer { cell: ocean, frame: 0 }, basalt).pixels().all(|p| p[3] == 0));
        let dark = average(&cache.native(Source::Ground(ocean))).iter().sum::<u32>();
        let shallow = average(&cache.native(Source::Ground(shore))).iter().sum::<u32>();
        assert!(dark < shallow, "open water is darker than shallows: {dark} {shallow}");
        let mut cache = cache;
        let first = cache.shimmer(ocean, 0, false, false).unwrap();
        assert!(Arc::ptr_eq(&first, &cache.shimmer(ocean, FRAMES, false, false).unwrap()));
        assert!(Arc::ptr_eq(&first, &cache.shimmer(ocean, 3, true, false).unwrap()), "memory stays still");
    }

    #[test]
    fn drifting_mist_uses_world_coordinates_and_releases_old_steps() {
        let mut cache = ArtCache::new(7);
        let mist = Mood::Surface(mood::Sky { clear: 0, mist: 8 });
        let pos = (0, 0);
        let image = |step| cache.native(Source::Mist { pos, step, mist: 8 });
        let first = image(0);
        assert_ne!(first, image(30), "wisps drift");
        assert!(cache.native(Source::Mist { pos, step: 0, mist: 0 }).pixels().all(|p| p[3] == 0));
        for pos in [(-1, 0), (0, 0), (1, -1)] {
            let image = cache.native(Source::Mist { pos, step: 30, mist: 8 });
            for (x, y, pixel) in image.enumerate_pixels() {
                let p = (i64::from(pos.0) * SIDE + i64::from(x), i64::from(pos.1) * SIDE + i64::from(y));
                assert_eq!(pixel.0, mist_pixel(7, p, 30, 8).unwrap_or([0; 4]));
            }
        }
        assert!(cache.mist(pos, 0).is_none(), "no mist under an overcast sky");
        cache.mood = mist;
        cache.begin_scale(2.0);
        assert!(cache.mist(pos, 0).is_some());
        cache.finish();
        cache.begin_scale(2.0);
        cache.mist(pos, 30);
        assert_eq!(cache.finish().len(), 1, "the old drift step is released, not accumulated");
        assert_eq!(cache.overlays.len(), 1);
        cache.mood = Mood::Deep(Stone::Slate, Stone::Slate);
        assert!(cache.mist(pos, 0).is_none(), "no sky underground");
    }

    #[test]
    fn tall_objects_keep_their_headroom_and_cell_alignment_at_every_scale() {
        let source = RgbaImage::from_fn(32, 44, |x, y| image::Rgba([x as u8, y as u8, 0, 255]));
        for scale in [0.5, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0] {
            let mut cache = ArtCache::new(7);
            cache.begin_scale(scale);
            let (side, factor, head) = (cache.device_side, cache.factor, cache.head());
            let pixels = rasterize(&source, false, side, factor, None, false, false);
            assert_eq!(pixels.len(), side * (side + head) * 4);
            let pad = side.saturating_sub(32 * factor) / 2;
            let cell_top = ((head + pad) * side + side / 2) * 4;
            assert_eq!(pixels[cell_top + 1], HEAD as u8, "cell top stays at y=0, scale {scale}");
            let (w, h, top) = cache.object_frame();
            assert!((h + top - w).abs() <= w * f32::EPSILON, "object bottom aligns with cell bottom");
        }
    }

    #[test]
    fn big_crowns_rise_into_the_cell_above_and_flat_objects_do_not() {
        let mut cache = ArtCache::new(7);
        let jungle = BIOMES.iter().find(|b| b.name == "Jungle").unwrap();
        let canopy = (0..200)
            .map(|x| terrain_art(Tile::Tree, jungle, false, (x, 2)))
            .find(|art| art.object == Some(Object::Canopy) && art.size == 2)
            .unwrap();
        assert!(cache.rises(&canopy, Object::Canopy));
        let door = terrain_art(Tile::Door, jungle, false, (0, 0));
        assert!(!cache.rises(&door, Object::Door));
        assert!(!cache.rises(&door, Object::Coins));
    }

    #[test]
    fn heat_glows_over_the_embers_of_basalt_floor_only() {
        let floor = |_: i32, _: i32| Terrain::Floor;
        let basalt = Mood::Deep(Stone::Basalt, Stone::Basalt);
        let cache = ArtCache::new(7);
        let mut hot = 0;
        for cy in 0..12 {
            for cx in 0..12 {
                let cell = world_cell(&floor, (cx, cy));
                let heat = cache.native_in(Source::Heat(cell), basalt);
                let ground = cache.native_in(Source::Ground(cell), basalt);
                for (h, g) in heat.pixels().zip(ground.pixels()) {
                    assert_eq!(h[3] != 0, g.0[..3] == mood::EMBER, "glow sits exactly on the embers");
                }
                hot += usize::from(heat.pixels().any(|p| p[3] != 0));
            }
        }
        assert!(hot > 3 && hot < 140, "seams cross some cells, not all: {hot}");
        let slate = Mood::Deep(Stone::Slate, Stone::Limestone);
        let mut cache = ArtCache::new(7);
        cache.mood = slate;
        assert!(cache.heat(world_cell(&floor, (0, 0)), false).is_none());
        let wall = world_cell(&|_, _| Terrain::Wall, (0, 0));
        assert!(cache.native_in(Source::Heat(wall), basalt).pixels().all(|p| p[3] == 0));
        let slate_floor = cache.native_in(Source::Ground(world_cell(&floor, (0, 0))), slate);
        assert!(slate_floor.pixels().all(|p| p.0[..3] != mood::EMBER));
    }

    #[test]
    fn the_deep_paints_in_its_stone() {
        let cache = ArtCache::new(7);
        let floor = world_cell(&|_, _| Terrain::Floor, (0, 0));
        let looks: Vec<[u32; 3]> = [Stone::Limestone, Stone::Slate, Stone::Basalt]
            .into_iter()
            .map(|s| average(&cache.native_in(Source::Ground(floor), Mood::Deep(s, s))))
            .collect();
        let [lime, slate, basalt] = [looks[0], looks[1], looks[2]];
        assert!(lime.iter().sum::<u32>() > basalt.iter().sum::<u32>() + 60, "{lime:?} {basalt:?}");
        assert!(slate[2] > slate[0], "slate is blue: {slate:?}");
        // Known walls stay brighter than the black of unknown cells.
        let wall = cache.native_in(
            Source::Ground(world_cell(&|_, _| Terrain::Wall, (0, 0))),
            Mood::Deep(Stone::Basalt, Stone::Basalt),
        );
        assert!(wall.pixels().all(|p| p[0] > 20 || p[1] > 20 || p[2] > 20));
    }

    #[test]
    fn moods_change_the_ground_but_items_and_the_hero_keep_their_colors() {
        let mut cache = ArtCache::new(7);
        let cell = world_cell(&mixed_world, (0, 0));
        let overcast = cache.ground(cell, false);
        let coins = cache.item(Object::Coins);
        cache.mood = Mood::Surface(mood::Sky { clear: 0, mist: 8 });
        assert!(!Arc::ptr_eq(&overcast, &cache.ground(cell, false)));
        assert!(Arc::ptr_eq(&coins, &cache.item(Object::Coins)));
        let misty = cache.native_in(Source::Ground(cell), cache.mood);
        assert_ne!(misty, cache.native(Source::Ground(cell)));
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
    fn ground_and_look_follow_biome_traits() {
        use Flavor::{Dry, Dunes, Lush, Marsh, Plain};
        use Style::{Fable, Hearth, Moss};
        use Terrain::{Grass, Rock, Sand, Snow, Water};
        let expected = [
            ("Deep water", Water, Moss, Plain),
            ("Shallow water", Water, Moss, Plain),
            ("Snow peaks", Snow, Hearth, Plain),
            ("Mountains", Rock, Hearth, Plain),
            ("Badlands", Rock, Fable, Plain),
            ("Hills", Grass, Hearth, Plain),
            ("Tundra", Snow, Hearth, Plain),
            ("Beach", Sand, Moss, Plain),
            ("Taiga", Grass, Hearth, Plain),
            ("Swamp", Grass, Hearth, Marsh),
            ("Desert", Sand, Fable, Dunes),
            ("Savanna", Grass, Fable, Dry),
            ("Jungle", Grass, Fable, Lush),
            ("Forest", Grass, Moss, Plain),
            ("Grassland", Grass, Moss, Plain),
        ];
        assert_eq!(expected.len(), BIOMES.len());
        for (name, ground, style, flavor) in expected {
            let biome = BIOMES.iter().find(|b| b.name == name).unwrap();
            let art = terrain_art(Tile::Wild(BiomeId(0)), biome, false, (3, 4));
            assert_eq!((art.ground, art.style, art.flavor), (ground, style, flavor), "{name}");
            assert_eq!(art.tint, Some(biome.color), "{name} keeps a wash of its color");
            let trees: HashSet<Object> =
                (0..200).filter_map(|x| terrain_art(Tile::Tree, biome, false, (x, 9)).object).collect();
            let own = match flavor {
                Lush => Object::Canopy,
                Dry | Dunes => Object::Acacia,
                Marsh => Object::Willow,
                Plain => Object::Oak,
            };
            assert!(trees.contains(&own), "{name}: {trees:?}");
        }
        let sizes: HashSet<u8> = (0..200).map(|x| terrain_art(Tile::Tree, &BIOMES[13], false, (x, 9)).size).collect();
        assert_eq!(sizes.len(), 3, "trees come in three sizes");
        let fertile = BIOMES.iter().find(|b| b.name == "Forest").unwrap();
        let at = (3, 4);
        let boulder = terrain_art(Tile::Rock, fertile, false, at);
        assert_eq!((boulder.ground, boulder.object), (Terrain::Grass, Some(Object::Boulder)));
        let tree = terrain_art(Tile::Tree, fertile, false, at);
        assert!(matches!(tree.object, Some(Object::Pine | Object::Oak)));
        assert_eq!(tree.ground, Terrain::Grass);
        assert_eq!(terrain_art(Tile::Floor, fertile, false, at).ground, Terrain::Dirt);
        assert_eq!(terrain_art(Tile::Wall, fertile, false, at).ground, Terrain::Wall);
        let deep_floor = terrain_art(Tile::Floor, &isle_core::DEEP, true, at);
        assert_eq!((deep_floor.ground, deep_floor.tint), (Terrain::Floor, None));
        let stairs = terrain_art(Tile::StairsDown, &isle_core::DEEP, true, at);
        assert_eq!((stairs.ground, stairs.object), (Terrain::Floor, Some(Object::StairsDown)));
        let pines = |style| (0..400).filter(|&x| tree_at((x, 7), style, Flavor::Plain) == Object::Pine).count();
        assert!(pines(Style::Hearth) > 300 && pines(Style::Fable) < 100, "conifers in the cold");
        assert!((100..300).contains(&pines(Style::Moss)), "temperate woods mix both");
    }

    #[test]
    fn every_item_variant_and_mixed_pile_has_an_icon() {
        let variants = [
            (Item::Relic(isle_core::Relic::EmberCharm), Object::Relic(isle_core::Relic::EmberCharm)),
            (Item::Relic(isle_core::Relic::Stillstone), Object::Relic(isle_core::Relic::Stillstone)),
            (Item::Relic(isle_core::Relic::HuntersToken), Object::Relic(isle_core::Relic::HuntersToken)),
            (Item::Herb(isle_core::Herb::Woundleaf), Object::Herb(isle_core::Herb::Woundleaf)),
            (Item::Herb(isle_core::Herb::Bogmint), Object::Herb(isle_core::Herb::Bogmint)),
            (Item::Herb(isle_core::Herb::Sunroot), Object::Herb(isle_core::Herb::Sunroot)),
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

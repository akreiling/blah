//! Isle's map art, painted in code.
//!
//! Every ground pixel is computed from its world position, so neighboring cells always agree and a field never
//! repeats. Natural ground (water, grass, sand, snow, rock, and Deep caves) takes each pixel's terrain from the
//! nearby cell centers after a smooth world-space warp, which rounds corners and wobbles shores; built ground
//! (streets, masonry) keeps square edges. Each biome picks a look (see [`style_of`]), and looks blend across
//! biome borders. The Deep paints in its stone instead. Weather grades the result (see [`crate::mood`]).
//! Objects, items, and the hero are drawn in code too. Images are enlarged by integer nearest-neighbor
//! duplication for the display scale and cached only while the visible map uses them.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{RenderImage, Window};
use image::{Frame, RgbaImage};
use isle_core::{Item, Tile};

use blah_core::Biome;

use crate::hero::{self, Look};
use crate::mood::{self, FRAMES, Mood, Stone};
use crate::paint::{self, Palette, STYLES, Style};

const NATIVE_SIDE: usize = 32;
const SIDE: i64 = NATIVE_SIDE as i64;
const DISPLAY_SIDE: f32 = 32.0;
const BIOME_TINT_PERCENT: u16 = 18;
const CAVE_TINT: [u8; 3] = [76, 150, 101];
const CAVE_TINT_PERCENT: u16 = 22;

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
}

/// Objects drawn over terrain.
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

#[cfg(test)]
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

/// What a cell shows, before its neighbors shape the ground's edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainArt {
    pub ground: Terrain,
    /// A transparent object placed over the ground, if any.
    pub object: Option<Object>,
    /// A gentle biome-color wash applied to water and rock, if any.
    pub tint: Option<[u8; 3]>,
    /// The biome's look.
    pub style: Style,
    /// Whether a natural object is drawn mirrored, so repeated trees and boulders vary.
    pub mirrored: bool,
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

fn mix(pos: (i32, i32)) -> u32 {
    ((pos.0 as u32).wrapping_mul(0x9e37_79b9) ^ (pos.1 as u32).wrapping_mul(0x85eb_ca6b)).rotate_left(13)
}

/// A stable per-cell tree shape: conifers in cold land, broadleaves in warm, both in between.
fn tree_at(pos: (i32, i32), style: Style) -> Object {
    let roll = (mix(pos) >> 3) % 8;
    let pines = match style {
        Style::Hearth => 7,
        Style::Moss => 4,
        Style::Fable => 1,
    };
    if roll < pines { Object::Pine } else { Object::Oak }
}

/// Art layers for a tile. `deep` levels draw caves; towns pave their floors with dirt and build their walls of
/// stone.
pub fn terrain_art(tile: Tile, biome: &Biome, deep: bool, pos: (i32, i32)) -> TerrainArt {
    let floor = if deep { Terrain::Floor } else { Terrain::Dirt };
    let natural = natural_ground(biome);
    // Trees and boulders never stand in water.
    let open = if natural == Terrain::Water { Terrain::Grass } else { natural };
    let style = style_of(biome);
    let (ground, object) = match tile {
        Tile::Wall => (Terrain::Wall, None),
        Tile::Floor => (floor, None),
        Tile::Grass => (Terrain::Grass, None),
        Tile::Water => (Terrain::Water, None),
        Tile::Tree => (open, Some(tree_at(pos, style))),
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
    // Water and rock keep a little of their biome's color, which separates deep from shallow water and red
    // badlands from grey peaks. The Deep has its own stone.
    let tint = (!deep && matches!(ground, Terrain::Water | Terrain::Rock)).then_some(biome.color);
    let mirrored = matches!(object, Some(Object::Pine | Object::Oak | Object::Boulder)) && mix(pos) & 16 != 0;
    TerrainArt { ground, object, tint, style, mirrored }
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

/// The 5×5 block of grounds centered on a cell, rows north to south, and the 3×3 blocks of tints and looks.
/// Callers pass the center's own values for cells the player does not know, so edges never reveal unexplored
/// terrain. Painting reaches at most one cell away; the outer ring of `around` is spare.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GroundCell {
    pub pos: (i32, i32),
    pub around: [[Terrain; 5]; 5],
    pub tints: [[Option<[u8; 3]>; 3]; 3],
    pub styles: [[Style; 3]; 3],
}

/// What painting needs to know of one map cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Spot {
    terrain: Terrain,
    style: Style,
    tint: Option<[u8; 3]>,
}

impl GroundCell {
    fn spot(&self, x: i64, y: i64) -> Spot {
        let (dx, dy) = ((x - i64::from(self.pos.0)) as isize, (y - i64::from(self.pos.1)) as isize);
        let (r, c) = ((dy + 1) as usize, (dx + 1) as usize);
        Spot { terrain: self.around[r + 1][c + 1], style: self.styles[r][c], tint: self.tints[r][c] }
    }
}

type World<'a> = &'a dyn Fn(i64, i64) -> Spot;

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

/// Look weights at world pixel `p`, summing to one, and the blended tint of the water or rock around it.
fn looks(seed: u64, p: (i64, i64), deep: bool, world: World) -> ([f32; 3], Option<[f32; 3]>) {
    let q = ground_point(seed, p, deep, |x, y| world(x, y).terrain);
    let mut styles = [0.0; 3];
    let (mut tint, mut tinted) = ([0.0; 3], 0.0);
    for (at, w) in corners(q) {
        let spot = world(at.0, at.1);
        styles[STYLES.iter().position(|&s| s == spot.style).expect("listed")] += w;
        if let Some(t) = spot.tint {
            tinted += w;
            for (sum, c) in tint.iter_mut().zip(t) {
                *sum += w * f32::from(c);
            }
        }
    }
    (styles, (tinted > 0.0).then(|| tint.map(|c| c / tinted)))
}

/// The nearest different terrain within three pixels along the axes, and how far.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Near(Option<(Terrain, i64)>);

impl Near {
    fn of(p: (i64, i64), here: Terrain, terrain: &dyn Fn((i64, i64)) -> Terrain) -> Near {
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

/// One material's color at world pixel `p` (warped to `q`) in palette `pal`.
#[expect(clippy::too_many_arguments, reason = "one argument per painting input")]
fn material(
    seed: u64,
    t: Terrain,
    pal: &Palette,
    stone: Option<Stone>,
    p: (i64, i64),
    q: (i64, i64),
    near: Near,
    tint: Option<[f32; 3]>,
) -> [f32; 3] {
    let n = |channel, period, at| noise(seed, channel, period, at);
    let roll = |channel| lattice(seed, channel, p.0, p.1);
    let deep = stone.is_some();
    match t {
        Terrain::Grass => {
            let patch = n(12, 75.0, q);
            let mut c = mixf(f(pal.grass), if patch > 0.0 { pal.grass2 } else { pal.grassdark }, patch.abs() * 0.65);
            if near.within(2, |t| matches!(t, Terrain::Dirt | Terrain::Floor | Terrain::Wall | Terrain::Rock)) {
                c = mixf(c, pal.grassdark, 0.3);
            } else if near.within(1, |t| matches!(t, Terrain::Sand | Terrain::Water | Terrain::Snow)) {
                c = mixf(c, pal.sand, 0.2);
            }
            if roll(17) < 0.045 {
                c = mixf(c, pal.blade, 0.14);
            }
            c
        }
        Terrain::Sand => {
            let mut c = f(pal.sand);
            let ripple = ((q.1 as f32 + n(6, 30.0, q) * 6.0) / 3.0).sin().abs();
            if ripple < 0.12 && roll(18) > 0.25 {
                c = mixf(c, pal.soil, 0.2);
            }
            if near.within(2, |t| t == Terrain::Water) {
                c = mixf(c, pal.water, 0.3);
            }
            if roll(17) < 0.045 {
                c = mixf(c, pal.stone, 0.14);
            }
            c
        }
        Terrain::Water if stone == Some(Stone::Basalt) => {
            // A dark crust over hot rock, not an orange flood.
            let vein = (n(31, 13.0, q) + n(32, 6.0, q) * 0.28).abs();
            if vein < 0.025 {
                f(pal.glow)
            } else if vein < 0.12 {
                f(pal.lava)
            } else {
                mixf(f(pal.water), pal.ink, 0.4)
            }
        }
        Terrain::Water => {
            let mut c = match tint {
                Some(tint) if !deep => mixf(f(pal.water), tint.map(|v| v as u8), 0.3),
                _ if deep => mixf(f(pal.water), pal.ink, 0.25),
                _ => f(pal.water),
            };
            if (q.1 as f32 / 4.0 + n(6, 30.0, q) * 2.0).sin().abs() < 0.10 {
                c = mixf(c, pal.ripple, 0.4);
            }
            if near.within(2, |t| t != Terrain::Water) {
                c = mixf(f(pal.sand), pal.water, 0.55);
            } else if near.within(3, |t| t != Terrain::Water) {
                c = mixf(c, pal.ripple, 0.25);
            }
            c
        }
        Terrain::Dirt => {
            let mut c = mixf(f(pal.soil), pal.sand, n(26, 18.0, p).max(0.0) * 0.3);
            if near.within(2, |t| t == Terrain::Grass) {
                c = mixf(c, pal.grassdark, 0.3);
            }
            if roll(19) < 0.03 {
                c = mixf(c, pal.stone, 0.3);
            } else if roll(19) > 0.97 {
                c = mixf(c, pal.ink, 0.15);
            }
            c
        }
        Terrain::Snow => {
            let mut c = mixf(f(pal.snow), pal.ripple, n(27, 40.0, q).max(0.0) * 0.3);
            if roll(17) < 0.008 {
                c = mixf(c, pal.grassdark, 0.35);
            }
            if near.within(1, |_| true) {
                c = mixf(c, pal.stone, 0.3);
            }
            c
        }
        Terrain::Rock => {
            let base = tint.map_or(f(pal.rock), |tint| mixf(f(pal.rock), tint.map(|v| v as u8), 0.3));
            let mut c = mixf(base, pal.stone, n(47, 12.0, p).max(0.0) * 0.5);
            if n(48, 13.0, q).abs() < 0.05 {
                c = mixf(c, pal.ink, 0.4);
            }
            if roll(17) < 0.05 {
                c = mixf(c, pal.stone, 0.2);
            }
            if near.within(1, |t| t != Terrain::Snow) {
                c = mixf(c, pal.ink, 0.25);
            }
            c
        }
        Terrain::Wall if deep => {
            let mut c = mixf(f(pal.wall), pal.rock, n(47, 12.0, p).max(0.0) * 0.34);
            // Facet highlights and a pale rim keep known walls distinct from unseen black.
            if n(48, 13.0, q).abs() < 0.04 {
                c = mixf(c, pal.stone, 0.14);
            }
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
                let brick = mixf(f(pal.rock), pal.stone, lattice(seed, 28, x.div_euclid(16), row) * 0.45);
                if v == 1 { mixf(brick, pal.stone, 0.25) } else { brick }
            }
        }
        Terrain::Floor => {
            let mut c = mixf(f(pal.floor), pal.stone, n(29, 23.0, p).max(0.0) * 0.18);
            if n(30, 17.0, q).abs() < 0.03 {
                c = mixf(c, pal.ink, 0.25);
            }
            if stone == Some(Stone::Slate) {
                c = mixf(c, pal.water, n(50, 23.0, p).max(0.0) * 0.3);
            }
            if roll(17) < 0.04 {
                c = mixf(c, pal.stone, 0.14);
            }
            if near.within(1, |t| t == Terrain::Wall) {
                c = mixf(c, pal.ink, 0.3);
            }
            c
        }
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

/// The palette at `p`: the Deep's stone, or the heaviest surface look.
fn palette_at(seed: u64, mood: Mood, p: (i64, i64), world: World) -> &'static Palette {
    match mood.stone_at(seed, p) {
        Some(stone) => paint::stone(stone),
        None => {
            let (weights, _) = looks(seed, p, false, world);
            let i = (0..3).fold(0, |best, i| if weights[i] > weights[best] { i } else { best });
            paint::surface(STYLES[i])
        }
    }
}

/// World-anchored marks: sand ripples and grass tufts on a shared 9px lattice, so a tuft can cross a cell edge
/// without being cut in half.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Mark {
    pos: (i64, i64),
    flip: bool,
}

/// Marks whose shape can reach the `side`-pixel square at `origin`.
fn marks(seed: u64, origin: (i64, i64), side: i64) -> Vec<Mark> {
    let mut out = Vec::new();
    for gy in (origin.1 - 9).div_euclid(9)..=(origin.1 + side + 4).div_euclid(9) {
        for gx in (origin.0 - 10).div_euclid(9)..=(origin.0 + side + 3).div_euclid(9) {
            let pos =
                (gx * 9 + (lattice(seed, 20, gx, gy) * 7.0) as i64, gy * 9 + (lattice(seed, 21, gx, gy) * 7.0) as i64);
            let patch = noise(seed, 22, 48.0, pos);
            if lattice(seed, 23, gx, gy) <= 0.22 + patch * 0.19 {
                out.push(Mark { pos, flip: lattice(seed, 24, gx, gy) < 0.5 });
            }
        }
    }
    out
}

/// A mark only grows where its whole footprint shows `kind`, so blades never bleed onto paths.
fn mark_fits(mark: Mark, kind: Terrain, terrain: &dyn Fn((i64, i64)) -> Terrain) -> bool {
    [(0, 0), (-3, 0), (3, 0), (0, -4), (0, 1)]
        .iter()
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

/// How far a tuft's blade tips lean, -1 to 1 pixel, at an animation frame. A wave rolls across the world, and
/// slow gusts make some meadows livelier than others. It depends only on the tuft's world position, so a tuft
/// straddling a cell edge leans the same way in both cells.
fn sway(seed: u64, mark: Mark, frame: u8, wind: u8) -> i64 {
    let gust = 1.1 + 0.4 * value_noise(seed, 25, mark.pos.0 as f32 / 160.0, mark.pos.1 as f32 / 160.0);
    let travel = (mark.pos.0 as f32 + 0.5 * mark.pos.1 as f32) / 96.0;
    let wave = std::f32::consts::TAU * (f32::from(frame) / f32::from(FRAMES) - travel);
    (f32::from(wind) / 10.0 * gust * wave.sin()).round().clamp(-1.0, 1.0) as i64
}

/// A ground pixel. `terrain` answers [`terrain_at`] (the renderer caches it), and `marks` holds every mark that
/// can reach `p`.
fn ground_pixel(
    seed: u64,
    mood: Mood,
    p: (i64, i64),
    world: World,
    terrain: &dyn Fn((i64, i64)) -> Terrain,
    marks: &[Mark],
) -> [u8; 4] {
    let t = terrain(p);
    if ember(seed, mood, t, p) {
        let [r, g, b] = mood::EMBER;
        return [r, g, b, 255];
    }
    let stone = mood.stone_at(seed, p);
    let deep = stone.is_some();
    let q = ground_point(seed, p, deep, |x, y| world(x, y).terrain);
    let near = Near::of(p, t, terrain);
    if t == Terrain::Sand
        && let Some(&mark) = marks.iter().rev().find(|&&m| ripple_shade(m, p) && mark_fits(m, Terrain::Sand, terrain))
    {
        let pal = palette_at(seed, mood, mark.pos, world);
        return finish(seed, mood, p, f(paint::mix(pal.sand, pal.soil, 0.4)));
    }
    let c = match stone {
        Some(stone) => material(seed, t, paint::stone(stone), Some(stone), p, q, near, None),
        None => {
            let (weights, tint) = looks(seed, p, false, world);
            let mut c = [0.0; 3];
            for (style, w) in STYLES.into_iter().zip(weights).filter(|&(_, w)| w > 0.0) {
                let one = material(seed, t, paint::surface(style), None, p, q, near, tint);
                for (sum, v) in c.iter_mut().zip(one) {
                    *sum += w * v;
                }
            }
            c
        }
    };
    finish(seed, mood, p, c)
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
    terrain: &dyn Fn((i64, i64)) -> Terrain,
    marks: &[Mark],
) -> Option<[u8; 4]> {
    if terrain(p) != Terrain::Grass {
        return None;
    }
    let (mark, part) = marks
        .iter()
        .rev()
        .filter_map(|&m| tuft_part(m, p, sway(seed, m, frame, wind)).map(|part| (m, part)))
        .find(|&(m, _)| mark_fits(m, Terrain::Grass, terrain))?;
    let pal = palette_at(seed, mood, mark.pos, world);
    let c = match part {
        0 => pal.grassdark,
        1 => paint::mix(pal.blade, pal.grass, 0.25),
        _ => pal.blade,
    };
    Some(finish(seed, mood, p, f(c)))
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

    /// A cell's painted ground.
    pub fn ground(&mut self, cell: GroundCell, remembered: bool, lit: bool) -> Arc<RenderImage> {
        self.get(ImageKey { source: Source::Ground(cell), remembered, lit, mood: self.mood })
    }

    /// A terrain object over the ground in its biome's look, optionally mirrored left to right.
    pub fn object(
        &mut self,
        object: Object,
        mirrored: bool,
        style: Style,
        remembered: bool,
        lit: bool,
    ) -> Arc<RenderImage> {
        self.get(ImageKey { source: Source::Object { object, mirrored, style }, remembered, lit, mood: self.mood })
    }

    /// A ground item's icon. Items keep their colors in every weather so they stay easy to spot.
    pub fn item(&mut self, object: Object) -> Arc<RenderImage> {
        let source = Source::Object { object, mirrored: false, style: Style::Moss };
        self.get(ImageKey { source, remembered: false, lit: false, mood: Mood::default() })
    }

    /// The player's figure. It is always in sight, so it is never remembered.
    pub fn hero(&mut self, look: Look, lit: bool) -> Arc<RenderImage> {
        self.get(ImageKey { source: Source::Hero(look), remembered: false, lit, mood: Mood::default() })
    }

    /// A cell's grass tufts at animation `frame`, or None without any. Remembered cells hold still. Every
    /// frame of a visible cell stays cached, so the sway cycles through ready images.
    pub fn tufts(&mut self, cell: GroundCell, frame: u8, remembered: bool, lit: bool) -> Option<Arc<RenderImage>> {
        let wind = self.mood.wind();
        if wind == 0 {
            return None;
        }
        let mood = self.mood;
        let key = |frame| ImageKey { source: Source::Tufts { cell, frame, wind }, remembered, lit, mood };
        let frame = if remembered { 0 } else { frame % FRAMES };
        let image = self.overlay(key(frame));
        if !remembered {
            for other in 0..FRAMES {
                self.used.insert(key(other));
            }
        }
        image
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
        let pixels = |cell: GroundCell| {
            let origin = (i64::from(cell.pos.0) * SIDE, i64::from(cell.pos.1) * SIDE);
            (0..SIDE * SIDE).map(move |i| (i, (origin.0 + i % SIDE, origin.1 + i / SIDE)))
        };
        let put = |out: &mut RgbaImage, i: i64, pixel: [u8; 4]| {
            out.put_pixel((i % SIDE) as u32, (i / SIDE) as u32, image::Rgba(pixel));
        };
        match source {
            Source::Ground(cell) => {
                let field = Field::new(seed, &cell, deep);
                let world = |x, y| cell.spot(x, y);
                let marks = marks(seed, (i64::from(cell.pos.0) * SIDE, i64::from(cell.pos.1) * SIDE), SIDE);
                for (i, p) in pixels(cell) {
                    put(&mut out, i, ground_pixel(seed, mood, p, &world, &|p| field.at(p), &marks));
                }
            }
            Source::Tufts { cell, frame, wind } => {
                let field = Field::new(seed, &cell, deep);
                let world = |x, y| cell.spot(x, y);
                let marks = marks(seed, (i64::from(cell.pos.0) * SIDE, i64::from(cell.pos.1) * SIDE), SIDE);
                for (i, p) in pixels(cell) {
                    if let Some(pixel) = tuft_pixel(seed, mood, p, frame, wind, &world, &|p| field.at(p), &marks) {
                        put(&mut out, i, pixel);
                    }
                }
            }
            Source::Heat(cell) => {
                if deep {
                    let field = Field::new(seed, &cell, deep);
                    for (i, p) in pixels(cell) {
                        if ember(seed, mood, field.at(p), p) {
                            let [r, g, b] = mood::HEAT;
                            put(&mut out, i, [r, g, b, 255]);
                        }
                    }
                }
            }
            Source::Hero(look) => out = hero::draw(look),
            Source::Object { object, mirrored, style } => {
                let (pal, snowy, hot) = match mood {
                    Mood::Deep(main, _) => (paint::stone(main), false, main == Stone::Basalt),
                    Mood::Surface(_) => (paint::surface(style), style == Style::Hearth, false),
                };
                let mut image = paint::object(object, pal, snowy, hot);
                if mirrored {
                    image = image::imageops::flip_horizontal(&image);
                }
                for (x, y, pixel) in image.enumerate_pixels() {
                    out.put_pixel(x, y, image::Rgba(mood::grade(seed, mood, None, pixel.0)));
                }
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
        let raster =
            RgbaImage::from_raw(self.device_side as u32, self.device_side as u32, pixels).expect("sized raster");
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
        }
    }

    fn world_cell(world: &dyn Fn(i32, i32) -> Terrain, pos: (i32, i32)) -> GroundCell {
        styled_cell(&|x, y| (world(x, y), Style::Moss), pos)
    }

    fn spots(world: Styled<'_>) -> impl Fn(i64, i64) -> Spot + '_ {
        move |x, y| {
            let (terrain, style) = world(x as i32, y as i32);
            Spot { terrain, style, tint: tint_of(terrain, x as i32) }
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
    fn looks_follow_the_biome_and_blend_across_borders() {
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
        let plain = cache.native(Source::Object { object: Object::Pine, mirrored: false, style: Style::Moss });
        let flipped = cache.native(Source::Object { object: Object::Pine, mirrored: true, style: Style::Moss });
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
        type World = fn(i32, i32) -> (Terrain, Style);
        let worlds: [(&str, World, Mood); 4] = [
            ("mixed", mixed_styled, Mood::default()),
            ("grass", |_, _| (Terrain::Grass, Style::Moss), Mood::default()),
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
                        std::hint::black_box(cache.ground(cell, false, false));
                        std::hint::black_box(cache.tufts(cell, frame, false, false));
                        std::hint::black_box(cache.heat(cell, false));
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
                if let Some(object) = objects(cx, cy) {
                    let mirrored = mix((cx, cy)) & 32 != 0;
                    let source = Source::Object { object, mirrored, style: world(cx, cy).1 };
                    image::imageops::overlay(&mut image, &cache.native_in(source, mood), 0, 0);
                }
                let big = image::imageops::resize(&image, 64, 64, image::imageops::FilterType::Nearest);
                image::imageops::replace(&mut out, &big, i64::from(cx * 64), i64::from(cy * 64));
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
            (6..=9, 10..=11) | (0..=1, 5..=6) | (7, 2) => Some(tree_at((x, y), style)),
            (3, 4) => Some(Object::Door),
            (3, 3) => Some(Object::Counter),
            (4, 3) => Some(Object::Altar),
            (5, 6) => Some(Object::Boulder),
            (6, 8) => Some(Object::StairsDown),
            (0, 8) => Some(Object::StairsUp),
            (2, 10) => Some(ALL_OBJECTS[8 + (x / 10) as usize]),
            (4, 10) => Some(ALL_OBJECTS[11 + (x / 10) as usize]),
            (6, 12) => Some(ALL_OBJECTS[14 + (x / 10).min(1) as usize]),
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
        let overcast = cache.ground(cell, false, false);
        let coins = cache.item(Object::Coins);
        cache.mood = Mood::Surface(mood::Sky { clear: 0, mist: 8 });
        assert!(!Arc::ptr_eq(&overcast, &cache.ground(cell, false, false)));
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
        use Style::{Fable, Hearth, Moss};
        use Terrain::{Grass, Rock, Sand, Snow, Water};
        let expected = [
            ("Deep water", Water, Moss),
            ("Shallow water", Water, Moss),
            ("Snow peaks", Snow, Hearth),
            ("Mountains", Rock, Hearth),
            ("Badlands", Rock, Fable),
            ("Hills", Grass, Hearth),
            ("Tundra", Snow, Hearth),
            ("Beach", Sand, Moss),
            ("Taiga", Grass, Hearth),
            ("Swamp", Grass, Hearth),
            ("Desert", Sand, Fable),
            ("Savanna", Grass, Fable),
            ("Jungle", Grass, Fable),
            ("Forest", Grass, Moss),
            ("Grassland", Grass, Moss),
        ];
        assert_eq!(expected.len(), BIOMES.len());
        for (name, ground, style) in expected {
            let biome = BIOMES.iter().find(|b| b.name == name).unwrap();
            let art = terrain_art(Tile::Wild(BiomeId(0)), biome, false, (3, 4));
            assert_eq!((art.ground, art.style), (ground, style), "{name}");
        }
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
        let pines = |style| (0..400).filter(|&x| tree_at((x, 7), style) == Object::Pine).count();
        assert!(pines(Style::Hearth) > 300 && pines(Style::Fable) < 100, "conifers in the cold");
        assert!((100..300).contains(&pines(Style::Moss)), "temperate woods mix both");
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

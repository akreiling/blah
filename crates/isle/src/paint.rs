//! Palettes and a small raster kit for drawing Isle's map objects in code.
//!
//! Everything is drawn at the 32px cell scale with a dark ink outline and a soft shadow, after the offline
//! design studies. Surface palettes follow the biome (see [`crate::art::style_of`]); the Deep uses its stone.

use image::{Rgba, RgbaImage};

use crate::art::Object;
use crate::mood::Stone;

const SIDE: i32 = 32;
/// Rows of headroom above an object's cell.
pub const HEAD: i32 = 12;

const fn hex(v: u32) -> [u8; 3] {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

/// A surface look, chosen per biome with ragged borders where biomes meet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    /// Temperate land: quiet greens, damp earth.
    Moss,
    /// Warm lowlands: golden meadows and pale sand.
    Fable,
    /// Cold, wet, or high land: blue-green shadows.
    Hearth,
}

#[cfg(test)]
pub const STYLES: [Style; 3] = [Style::Moss, Style::Fable, Style::Hearth];

/// The named colors one look draws with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub ink: [u8; 3],
    pub grass: [u8; 3],
    pub grass2: [u8; 3],
    pub grassdark: [u8; 3],
    pub blade: [u8; 3],
    pub soil: [u8; 3],
    pub sand: [u8; 3],
    pub snow: [u8; 3],
    pub water: [u8; 3],
    pub ripple: [u8; 3],
    pub leaf: [u8; 3],
    pub leafhi: [u8; 3],
    pub bark: [u8; 3],
    pub rock: [u8; 3],
    pub stone: [u8; 3],
    pub floor: [u8; 3],
    pub wall: [u8; 3],
    pub gold: [u8; 3],
    pub steel: [u8; 3],
    pub glow: [u8; 3],
    pub red: [u8; 3],
    pub lava: [u8; 3],
}

const MOSS: Palette = Palette {
    ink: hex(0x253533),
    grass: hex(0x65794c),
    grass2: hex(0x828850),
    grassdark: hex(0x536b47),
    blade: hex(0x95a15f),
    soil: hex(0xad9469),
    sand: hex(0xc8b58b),
    snow: hex(0xdfe3da),
    water: hex(0x416e73),
    ripple: hex(0x6d9691),
    leaf: hex(0x4e7352),
    leafhi: hex(0x7f975e),
    bark: hex(0x70523d),
    rock: hex(0x777e75),
    stone: hex(0x93988b),
    floor: hex(0x535c58),
    wall: hex(0x343e3c),
    gold: hex(0xd9b66d),
    steel: hex(0xacb9af),
    glow: hex(0xa4cba0),
    red: hex(0xba6c58),
    lava: hex(0xba6c58),
};

const FABLE: Palette = Palette {
    ink: hex(0x443b38),
    grass: hex(0x8c9a57),
    grass2: hex(0xaca75e),
    grassdark: hex(0x77884f),
    blade: hex(0xc0bd76),
    soil: hex(0xbc9567),
    sand: hex(0xddc48d),
    snow: hex(0xefe9d8),
    water: hex(0x598c91),
    ripple: hex(0x9abcb0),
    leaf: hex(0x658451),
    leafhi: hex(0x9ba85b),
    bark: hex(0x906547),
    rock: hex(0x939183),
    stone: hex(0xbab39e),
    floor: hex(0x706e63),
    wall: hex(0x494c45),
    gold: hex(0xebc478),
    steel: hex(0xc5c8b6),
    glow: hex(0xcbd993),
    red: hex(0xc47863),
    lava: hex(0xc47863),
};

const HEARTH: Palette = Palette {
    ink: hex(0x272f3c),
    grass: hex(0x506e69),
    grass2: hex(0x6d8370),
    grassdark: hex(0x455e62),
    blade: hex(0x8ea18a),
    soil: hex(0x938777),
    sand: hex(0xb6af96),
    snow: hex(0xdbe3ec),
    water: hex(0x374f6c),
    ripple: hex(0x607e95),
    leaf: hex(0x42686b),
    leafhi: hex(0x70918b),
    bark: hex(0x655863),
    rock: hex(0x727b8b),
    stone: hex(0x919bab),
    floor: hex(0x50576a),
    wall: hex(0x333a4e),
    gold: hex(0xd5ad79),
    steel: hex(0xa9baca),
    glow: hex(0xa7d3bf),
    red: hex(0xb8778b),
    lava: hex(0xb8778b),
};

const LIMESTONE: Palette = Palette {
    rock: hex(0x777c73),
    stone: hex(0xaaa996),
    floor: hex(0x696d60),
    wall: hex(0x3c4442),
    leaf: hex(0x777e62),
    leafhi: hex(0xa2aa80),
    ..MOSS
};

const SLATE: Palette = Palette {
    rock: hex(0x475867),
    stone: hex(0x718691),
    floor: hex(0x354950),
    wall: hex(0x23323d),
    leaf: hex(0x4a7773),
    leafhi: hex(0x78a496),
    glow: hex(0xa1e4cb),
    ..HEARTH
};

const BASALT: Palette = Palette {
    ink: hex(0x11151d),
    grass: hex(0x292b31),
    grass2: hex(0x37343a),
    grassdark: hex(0x23262d),
    blade: hex(0x696266),
    soil: hex(0x51464a),
    sand: hex(0x3e3439),
    snow: hex(0x8a8a90),
    water: hex(0x862e25),
    ripple: hex(0xf4a255),
    leaf: hex(0x33343b),
    leafhi: hex(0x62515a),
    bark: hex(0x443138),
    rock: hex(0x373943),
    stone: hex(0x767782),
    floor: hex(0x30323b),
    wall: hex(0x1b1f29),
    gold: hex(0xdeb579),
    steel: hex(0xb0b5bf),
    glow: hex(0xffbd68),
    red: hex(0xd56a43),
    lava: hex(0xd9552c),
};

pub fn surface(style: Style) -> &'static Palette {
    match style {
        Style::Moss => &MOSS,
        Style::Fable => &FABLE,
        Style::Hearth => &HEARTH,
    }
}

pub fn stone(stone: Stone) -> &'static Palette {
    match stone {
        Stone::Limestone => &LIMESTONE,
        Stone::Slate => &SLATE,
        Stone::Basalt => &BASALT,
    }
}

pub fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    [0, 1, 2].map(|i| (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * t).round() as u8)
}

const WHITE: [u8; 3] = [255, 255, 255];
const ASH: [u8; 3] = [46, 42, 48];
const EMBER: [u8; 3] = [255, 94, 34];
const HEART: [u8; 3] = [224, 72, 62];
const WELL_GLOW: [u8; 3] = [124, 220, 190];

/// A 32-wide transparent drawing surface, with [`HEAD`] rows above the cell at negative y. Like the studies'
/// drawing calls, shapes replace pixels rather than compositing, so a shadow drawn first stays translucent
/// only where nothing covers it.
pub struct Canvas(pub RgbaImage);

impl Canvas {
    pub fn new() -> Self {
        Canvas(RgbaImage::new(SIDE as u32, (SIDE + HEAD) as u32))
    }

    fn set(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if (0..SIDE).contains(&x) && (-HEAD..SIDE).contains(&y) {
            self.0.put_pixel(x as u32, (y + HEAD) as u32, Rgba(c));
        }
    }

    pub fn point(&mut self, x: i32, y: i32, c: [u8; 3]) {
        self.set(x, y, [c[0], c[1], c[2], 255]);
    }

    /// A filled rectangle, corners included.
    pub fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: [u8; 3]) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.point(x, y, c);
            }
        }
    }

    /// A polyline through `points`, `width` pixels thick.
    pub fn line(&mut self, points: &[(i32, i32)], c: [u8; 3], width: i32) {
        let lo = -(width - 1) / 2;
        let hi = width / 2;
        let mut brush = |x: i32, y: i32| {
            for dy in lo..=hi {
                for dx in lo..=hi {
                    self.point(x + dx, y + dy, c);
                }
            }
        };
        if points.len() == 1 {
            brush(points[0].0, points[0].1);
        }
        for pair in points.windows(2) {
            let ((mut x, mut y), (x1, y1)) = (pair[0], pair[1]);
            let (dx, dy) = ((x1 - x).abs(), -(y1 - y).abs());
            let (sx, sy) = ((x1 - x).signum(), (y1 - y).signum());
            let mut err = dx + dy;
            loop {
                brush(x, y);
                if (x, y) == (x1, y1) {
                    break;
                }
                let e2 = 2 * err;
                if e2 >= dy {
                    err += dy;
                    x += sx;
                }
                if e2 <= dx {
                    err += dx;
                    y += sy;
                }
            }
        }
    }

    /// A filled polygon, edges included, with an optional outline.
    pub fn poly(&mut self, points: &[(i32, i32)], fill: [u8; 3], outline: Option<[u8; 3]>) {
        let (x0, x1) = points.iter().fold((SIDE, 0), |(a, b), p| (a.min(p.0), b.max(p.0)));
        let (y0, y1) = points.iter().fold((SIDE, 0), |(a, b), p| (a.min(p.1), b.max(p.1)));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let mut inside = false;
                for i in 0..points.len() {
                    let (a, b) = (points[i], points[(i + 1) % points.len()]);
                    let (ax, ay, bx, by) = (a.0 as f32, a.1 as f32, b.0 as f32, b.1 as f32);
                    if (ay > py) != (by > py) && px < ax + (py - ay) / (by - ay) * (bx - ax) {
                        inside = !inside;
                    }
                }
                if inside {
                    self.point(x, y, fill);
                }
            }
        }
        let mut ring = points.to_vec();
        ring.push(points[0]);
        self.line(&ring, outline.unwrap_or(fill), 1);
    }

    /// An ellipse in the inclusive box, filled with `fill` (which may be translucent or clear).
    pub fn ellipse(&mut self, (x0, y0, x1, y1): (i32, i32, i32, i32), fill: [u8; 4], outline: Option<[u8; 3]>) {
        let (cx, cy) = ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
        let (rx, ry) = (((x1 - x0) as f32 / 2.0).max(0.5), ((y1 - y0) as f32 / 2.0).max(0.5));
        let inside = |x: i32, y: i32| ((x as f32 - cx) / rx).powi(2) + ((y as f32 - cy) / ry).powi(2) <= 1.05;
        for y in y0..=y1 {
            for x in x0..=x1 {
                if !inside(x, y) {
                    continue;
                }
                let edge = !(inside(x - 1, y) && inside(x + 1, y) && inside(x, y - 1) && inside(x, y + 1));
                match outline {
                    Some(ink) if edge => self.point(x, y, ink),
                    _ => self.set(x, y, fill),
                }
            }
        }
    }

    fn shadow(&mut self, p: &Palette) {
        self.shadow_in(p, &Grow::new(1, (16, 28)));
    }

    fn shadow_in(&mut self, p: &Palette, grow: &Grow) {
        let [r, g, b] = p.ink;
        let (x0, y0, x1, y1) = grow.rect(5, 25, 27, 29);
        self.ellipse((x0, y0, x1, y1), [r, g, b, 80], None);
    }
}

/// Natural objects come in three sizes, scaled about the foot of the trunk.
const SIZES: [f32; 3] = [0.8, 1.0, 1.18];

struct Grow {
    scale: f32,
    foot: (i32, i32),
}

impl Grow {
    fn new(size: u8, foot: (i32, i32)) -> Self {
        Grow { scale: SIZES[usize::from(size.min(2))], foot }
    }

    fn at(&self, (x, y): (i32, i32)) -> (i32, i32) {
        let f = |v: i32, o: i32| o + ((v - o) as f32 * self.scale).round() as i32;
        (f(x, self.foot.0), f(y, self.foot.1))
    }

    fn all(&self, points: &[(i32, i32)]) -> Vec<(i32, i32)> {
        points.iter().map(|&p| self.at(p)).collect()
    }

    fn rect(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> (i32, i32, i32, i32) {
        let ((a, b), (c, d)) = (self.at((x0, y0)), self.at((x1, y1)));
        (a, b, c, d)
    }
}

fn solid(c: [u8; 3]) -> [u8; 4] {
    [c[0], c[1], c[2], 255]
}

/// Draw an object in `p` at `size` (0 to 2; only natural objects vary). `snowy` dusts conifers; `hot` threads
/// boulders with lava. The image is [`HEAD`] pixels taller than a cell, so big crowns can rise over the cell
/// above; the cell's own square is its lower 32 rows.
pub fn object(object: Object, p: &Palette, snowy: bool, hot: bool, size: u8) -> RgbaImage {
    let mut d = Canvas::new();
    let ink = Some(p.ink);
    match object {
        Object::Oak => {
            let g = Grow::new(size, (16, 28));
            d.shadow_in(p, &g);
            let (x0, y0, x1, y1) = g.rect(14, 19, 17, 27);
            d.rect(x0, y0, x1, y1, p.ink);
            d.rect(x0 + 1, y0, x1 - 1, y1 - 1, p.bark);
            d.poly(
                &g.all(&[(15, 2), (24, 8), (23, 12), (29, 17), (25, 23), (17, 25), (7, 23), (3, 18), (7, 12), (6, 8)]),
                p.leaf,
                ink,
            );
            d.poly(&g.all(&[(15, 4), (22, 8), (20, 12), (11, 14), (6, 17), (8, 10)]), p.leafhi, None);
            d.line(&g.all(&[(11, 19), (16, 16), (22, 17)]), mix(p.leaf, p.ink, 0.25), 1);
            d.line(&g.all(&[(20, 20), (25, 19)]), mix(p.leaf, p.ink, 0.25), 1);
            for at in [(18, 6), (10, 9), (13, 12), (19, 10)] {
                let (x, y) = g.at(at);
                d.point(x, y, mix(p.leafhi, WHITE, 0.25));
            }
        }
        Object::Pine => {
            let g = Grow::new(size, (16, 28));
            d.shadow_in(p, &g);
            let (x0, y0, x1, y1) = g.rect(14, 22, 17, 28);
            d.rect(x0, y0, x1, y1, p.ink);
            d.rect(x0 + 1, y0, x1 - 1, y1 - 1, p.bark);
            let tiers =
                [(16, 1), (22, 9), (19, 9), (25, 16), (21, 16), (27, 23), (5, 23), (11, 16), (7, 16), (13, 9), (10, 9)];
            d.poly(&g.all(&tiers), p.leaf, ink);
            let dark = mix(p.leaf, p.ink, 0.35);
            d.poly(&g.all(&[(17, 4), (21, 9), (18, 9)]), dark, None);
            d.poly(&g.all(&[(18, 11), (24, 16), (20, 16)]), dark, None);
            d.poly(&g.all(&[(20, 18), (26, 22), (17, 22)]), dark, None);
            for line in [[(15, 3), (11, 8)], [(13, 10), (8, 15)], [(11, 17), (6, 22)]] {
                d.line(&g.all(&line), p.leafhi, 1);
            }
            if snowy {
                let snow = mix(p.snow, WHITE, 0.3);
                for line in [
                    [(14, 3), (17, 3)],
                    [(11, 9), (14, 9)],
                    [(8, 16), (12, 16)],
                    [(18, 16), (21, 16)],
                    [(6, 23), (10, 23)],
                ] {
                    d.line(&g.all(&line), snow, 1);
                }
            }
        }
        Object::Acacia => {
            let g = Grow::new(size, (16, 28));
            let [r, gr, b] = p.ink;
            let (x0, y0, x1, y1) = g.rect(2, 25, 30, 29);
            d.ellipse((x0, y0, x1, y1), [r, gr, b, 80], None);
            let trunk = g.all(&[(16, 28), (16, 20), (12, 13)]);
            let branch = g.all(&[(16, 21), (21, 12)]);
            for path in [&trunk, &branch] {
                d.line(path, p.ink, 3);
            }
            for path in [&trunk, &branch] {
                d.line(path, p.bark, 1);
            }
            let crown = [(2, 12), (6, 8), (12, 6), (20, 6), (26, 8), (30, 12), (25, 14), (16, 15), (7, 14)];
            d.poly(&g.all(&crown), p.leaf, ink);
            d.poly(&g.all(&[(6, 9), (12, 7), (20, 7), (25, 9), (21, 11), (10, 11)]), p.leafhi, None);
            d.line(&g.all(&[(6, 13), (16, 14), (25, 13)]), mix(p.leaf, p.ink, 0.3), 1);
        }
        Object::Canopy => {
            let g = Grow::new(size, (16, 28));
            let [r, gr, b] = p.ink;
            let (x0, y0, x1, y1) = g.rect(2, 24, 30, 30);
            d.ellipse((x0, y0, x1, y1), [r, gr, b, 90], None);
            d.poly(&g.all(&[(13, 28), (14, 17), (18, 17), (19, 28), (22, 29), (10, 29)]), p.bark, ink);
            let dark = mix(p.leaf, p.ink, 0.5);
            let crown = [(16, -4), (24, -1), (29, 5), (30, 13), (26, 20), (18, 22), (10, 21), (4, 17), (2, 9), (6, 1)];
            d.poly(&g.all(&crown), dark, ink);
            for (x0, y0, x1, y1) in [(5, 3, 16, 12), (16, 0, 27, 10), (10, 10, 23, 19)] {
                let (a, b, c, e) = g.rect(x0, y0, x1, y1);
                d.ellipse((a, b, c, e), solid(mix(p.leaf, p.ink, 0.25)), None);
            }
            for (x0, y0, x1, y1) in [(8, 4, 12, 7), (19, 1, 23, 4), (14, 11, 18, 14)] {
                let (a, b, c, e) = g.rect(x0, y0, x1, y1);
                d.ellipse((a, b, c, e), solid(p.leaf), None);
            }
            for vine in [[(7, 17), (7, 25)], [(24, 18), (24, 24)], [(12, 20), (11, 26)]] {
                d.line(&g.all(&vine), mix(p.leaf, p.ink, 0.1), 1);
            }
        }
        Object::Willow => {
            let g = Grow::new(size, (16, 28));
            d.shadow_in(p, &g);
            let (x0, y0, x1, y1) = g.rect(14, 15, 17, 28);
            d.rect(x0, y0, x1, y1, p.ink);
            d.rect(x0 + 1, y0, x1 - 1, y1 - 1, p.bark);
            let crown = [
                (16, 1),
                (24, 4),
                (28, 10),
                (29, 21),
                (25, 17),
                (22, 23),
                (19, 18),
                (16, 23),
                (13, 18),
                (10, 23),
                (7, 17),
                (3, 21),
                (4, 10),
                (8, 4),
            ];
            d.poly(&g.all(&crown), p.leaf, ink);
            d.poly(&g.all(&[(10, 5), (16, 3), (22, 5), (18, 8), (12, 8)]), p.leafhi, None);
            for (x, end) in [(7, 18), (11, 21), (15, 20), (19, 21), (23, 19), (26, 17)] {
                d.line(&g.all(&[(x, 9), (x, end)]), mix(p.leafhi, p.leaf, 0.35), 1);
            }
        }
        Object::Boulder => {
            let g = Grow::new(size, (16, 27));
            d.shadow_in(p, &g);
            d.poly(&g.all(&[(4, 23), (5, 15), (11, 10), (22, 11), (28, 19), (26, 26), (10, 27)]), p.rock, ink);
            d.poly(&g.all(&[(6, 16), (12, 12), (21, 12), (17, 20), (8, 22)]), p.stone, None);
            d.line(&g.all(&[(20, 15), (17, 20), (22, 24)]), mix(p.rock, p.ink, 0.3), 1);
            if hot {
                d.line(&g.all(&[(19, 14), (17, 19), (20, 22)]), p.lava, 1);
                let (x, y) = g.at((18, 19));
                d.point(x, y, p.glow);
            }
        }
        Object::Door => {
            d.rect(5, 2, 26, 31, p.ink);
            d.rect(6, 3, 25, 31, mix(p.rock, p.stone, 0.5));
            d.ellipse((9, 5, 22, 16), solid(p.ink), None);
            d.rect(9, 10, 22, 31, p.ink);
            d.ellipse((10, 6, 21, 16), solid(p.bark), None);
            d.rect(10, 11, 21, 31, p.bark);
            for x in [13, 18] {
                d.line(&[(x, 8), (x, 31)], mix(p.bark, p.ink, 0.4), 1);
            }
            for y in [13, 25] {
                d.line(&[(10, y), (15, y)], p.ink, 1);
            }
            d.rect(19, 19, 20, 20, p.gold);
        }
        Object::StairsDown => {
            d.poly(&[(5, 12), (22, 8), (28, 15), (26, 27), (6, 27)], p.ink, None);
            for i in 0..4 {
                let y = 13 + i * 3;
                d.line(&[(8 + i * 2, y), (24, y)], mix(p.stone, p.ink, i as f32 * 0.2), 2);
            }
        }
        Object::StairsUp => {
            d.shadow(p);
            for i in 0..4 {
                let (x0, y0) = (5 + i * 4, 21 - i * 5);
                d.rect(x0, y0, 27, y0 + 6, p.ink);
                d.rect(x0 + 1, y0 + 1, 26, y0 + 5, mix(p.rock, p.stone, i as f32 * 0.3));
                d.line(&[(x0 + 1, y0 + 1), (26, y0 + 1)], mix(p.stone, WHITE, 0.3), 1);
            }
        }
        Object::Counter => {
            d.shadow(p);
            d.rect(4, 12, 27, 23, p.ink);
            d.rect(5, 15, 26, 22, p.bark);
            d.line(&[(5, 19), (26, 19)], mix(p.bark, p.ink, 0.3), 1);
            d.rect(5, 13, 26, 14, mix(p.bark, p.gold, 0.35));
            d.rect(7, 24, 9, 27, p.ink);
            d.rect(22, 24, 24, 27, p.ink);
            d.ellipse((9, 8, 13, 11), solid(p.gold), ink);
            d.rect(18, 6, 22, 11, p.ink);
            d.rect(19, 7, 21, 11, p.steel);
        }
        Object::Altar => {
            d.shadow(p);
            d.poly(&[(6, 23), (25, 23), (27, 27), (4, 27)], p.rock, ink);
            d.rect(9, 15, 23, 23, p.ink);
            d.rect(10, 16, 22, 22, p.stone);
            d.poly(&[(6, 13), (22, 11), (26, 14), (10, 17), (6, 15)], p.steel, ink);
            d.line(&[(15, 13), (15, 6)], p.gold, 2);
            d.ellipse((13, 2, 18, 7), solid(p.glow), ink);
        }
        Object::Watchtower { cleared } => {
            d.shadow(p);
            d.poly(&[(5, 24), (26, 24), (29, 30), (3, 30)], p.rock, ink);
            d.rect(8, -3, 24, 26, p.ink);
            d.rect(9, -2, 23, 25, p.stone);
            for y in [3, 10, 17, 24] {
                d.line(&[(9, y), (23, y)], p.rock, 1);
            }
            for (x, y) in [(12, -2), (19, 4), (12, 11), (20, 18)] {
                d.line(&[(x, y), (x, y + 5)], p.rock, 1);
            }
            for (x, top) in [(7, -6), (14, -4), (21, -8)] {
                d.rect(x, top, x + 4, 0, p.ink);
                d.rect(x + 1, top + 1, x + 3, -1, p.rock);
            }
            d.rect(14, 5, 17, 11, p.ink);
            d.poly(&[(12, 26), (12, 20), (16, 16), (20, 20), (20, 26)], p.ink, None);
            d.line(&[(11, 26), (21, 26)], p.steel, 1);
            if cleared {
                d.rect(14, 6, 17, 8, p.steel);
                d.line(&[(15, 7), (15, 12)], p.glow, 1);
            } else {
                d.poly(&[(14, 8), (15, 1), (17, 5), (19, 3), (19, 9)], p.red, None);
                d.line(&[(16, 5), (17, 8)], p.glow, 1);
            }
        }
        Object::CinderWarden | Object::AshSentry => {
            let warden = object == Object::CinderWarden;
            d.shadow(p);
            d.line(&[(12, 23), (11, 29)], p.ink, 3);
            d.line(&[(19, 23), (20, 29)], p.ink, 3);
            d.poly(&[(10, 12), (20, 12), (24, 25), (7, 25)], if warden { p.red } else { p.bark }, ink);
            d.rect(11, 12, 20, 21, if warden { p.steel } else { p.rock });
            d.ellipse((11, 3, 20, 12), solid(if warden { p.steel } else { p.bark }), ink);
            d.rect(12, 7, 19, 9, p.ink);
            d.rect(14, 8, 17, 8, p.glow);
            if warden {
                d.line(&[(16, 0), (16, 4)], p.red, 3);
                d.poly(&[(3, 13), (10, 13), (10, 22), (6, 26), (3, 22)], p.steel, ink);
                d.line(&[(25, 7), (25, 28)], p.bark, 2);
                d.poly(&[(25, 2), (28, 8), (23, 8)], p.steel, ink);
            } else {
                d.line(&[(24, 9), (28, 13), (29, 19), (27, 25), (24, 28)], p.bark, 2);
                d.line(&[(24, 9), (24, 28)], p.steel, 1);
                d.line(&[(19, 17), (31, 17)], p.gold, 1);
            }
        }
        Object::Cairn => {
            d.shadow(p);
            for (x, y, w) in [(4, 22, 12), (16, 22, 12), (7, 16, 11), (17, 17, 8), (11, 10, 10), (13, 6, 6)] {
                d.ellipse((x, y, x + w, y + 6), solid(p.rock), ink);
                d.line(&[(x + 2, y + 1), (x + w - 2, y + 1)], p.stone, 1);
            }
            d.rect(13, 23, 19, 27, p.ink);
            d.rect(14, 24, 18, 26, p.bark);
        }
        Object::Camp => {
            d.shadow(p);
            d.poly(&[(4, 21), (12, 5), (23, 10), (28, 25)], mix(p.bark, p.gold, 0.25), ink);
            d.poly(&[(4, 21), (12, 5), (15, 23)], p.bark, ink);
            d.poly(&[(7, 21), (12, 11), (13, 22)], p.ink, None);
            for (x, y) in [(17, 25), (21, 23), (25, 26)] {
                d.ellipse((x, y, x + 4, y + 3), solid(p.rock), ink);
            }
            d.line(&[(19, 26), (24, 25)], p.ink, 2);
            d.rect(4, 24, 10, 28, p.ink);
            d.rect(5, 25, 9, 27, p.steel);
            d.line(&[(7, 25), (7, 27)], p.bark, 1);
        }
        Object::Shrine => {
            d.shadow(p);
            d.poly(&[(3, 24), (26, 23), (29, 28), (4, 29)], p.rock, ink);
            d.rect(5, 7, 10, 24, p.ink);
            d.rect(6, 8, 9, 23, p.stone);
            d.rect(22, 14, 27, 25, p.ink);
            d.rect(23, 15, 26, 24, p.stone);
            d.poly(&[(4, 5), (14, 5), (12, 9), (4, 10)], p.rock, ink);
            d.rect(12, 18, 22, 25, p.ink);
            d.rect(13, 19, 21, 24, p.stone);
            d.line(&[(13, 19), (21, 19)], p.steel, 1);
            d.rect(15, 25, 22, 28, p.bark);
            d.rect(18, 25, 19, 27, p.gold);
        }
        Object::AshenForge => {
            d.shadow(p);
            d.poly(&[(3, 24), (28, 23), (30, 29), (2, 29)], p.ink, None);
            d.poly(&[(5, 19), (7, 12), (12, 8), (21, 8), (26, 14), (27, 23), (23, 27), (8, 26)], ASH, ink);
            d.rect(8, 16, 24, 23, p.rock);
            d.rect(10, 17, 22, 24, p.ink);
            d.ellipse((12, 17, 20, 24), solid([34, 20, 20]), ink);
            d.poly(&[(14, 23), (15, 19), (17, 17), (17, 20), (19, 22), (17, 25)], EMBER, None);
            d.line(&[(9, 15), (12, 12), (17, 12), (20, 14)], p.stone, 1);
            d.line(&[(7, 20), (9, 18), (9, 16)], EMBER, 1);
            d.line(&[(23, 14), (25, 17), (24, 19)], EMBER, 1);
            d.rect(12, 25, 18, 27, p.steel);
            d.rect(14, 24, 16, 25, p.gold);
        }
        Object::WellOfNames => {
            d.shadow(p);
            d.ellipse((4, 15, 27, 30), solid(p.ink), ink);
            d.ellipse((6, 12, 25, 26), solid(p.rock), ink);
            d.ellipse((9, 13, 22, 22), solid(ASH), Some(p.ink));
            d.ellipse((10, 13, 21, 20), [0, 0, 0, 0], Some(mix(ASH, p.stone, 0.45)));
            d.rect(7, 21, 24, 25, p.stone);
            d.line(&[(8, 21), (13, 23), (18, 22), (23, 24)], p.ink, 2);
            d.rect(9, 19, 10, 20, WELL_GLOW);
            d.rect(14, 16, 15, 18, WELL_GLOW);
            d.rect(18, 11, 19, 13, WELL_GLOW);
            d.line(&[(10, 18), (11, 16), (11, 14)], mix(WELL_GLOW, WHITE, 0.25), 1);
            d.line(&[(21, 16), (22, 14), (21, 12)], mix(WELL_GLOW, WHITE, 0.25), 1);
        }
        Object::Herb(herb) => {
            let bloom = match herb {
                isle_core::Herb::Woundleaf => [236, 220, 184],
                isle_core::Herb::Bogmint => [150, 150, 226],
                isle_core::Herb::Sunroot => [236, 172, 68],
            };
            d.shadow(p);
            d.line(&[(16, 27), (16, 12)], p.ink, 3);
            d.line(&[(16, 26), (16, 13)], p.leaf, 1);
            for (x, y, side) in [(15, 23, -1), (17, 20, 1), (15, 17, -1)] {
                d.poly(&[(x, y), (x + side * 7, y - 5), (x + side * 6, y), (x, y + 2)], p.leaf, ink);
                d.line(&[(x + side * 2, y), (x + side * 5, y - 2)], p.leafhi, 1);
            }
            if herb == isle_core::Herb::Sunroot {
                d.ellipse((12, 24, 20, 29), solid(p.bark), ink);
            }
            for (x, y) in [(12, 10), (17, 8), (19, 12)] {
                d.ellipse((x, y, x + 4, y + 4), solid(bloom), ink);
            }
        }
        Object::Coins => {
            for (x, y) in [(8, 20), (15, 18), (11, 14)] {
                d.ellipse((x, y, x + 9, y + 5), solid(p.gold), ink);
                d.line(&[(x + 3, y + 2), (x + 5, y + 2)], mix(p.gold, WHITE, 0.45), 1);
            }
        }
        Object::Arrows => {
            for (ox, oy) in [(0, 0), (4, 3)] {
                d.line(&[(5 + ox, 24 + oy), (21 + ox, 8 + oy)], p.ink, 2);
                d.line(&[(6 + ox, 23 + oy), (21 + ox, 8 + oy)], p.bark, 1);
                d.poly(&[(20 + ox, 6 + oy), (25 + ox, 4 + oy), (23 + ox, 9 + oy)], p.steel, ink);
                d.line(&[(6 + ox, 23 + oy), (4 + ox, 20 + oy)], p.red, 1);
                d.line(&[(6 + ox, 23 + oy), (9 + ox, 25 + oy)], p.red, 1);
            }
        }
        Object::Potion => {
            d.rect(13, 7, 18, 11, p.ink);
            d.rect(14, 8, 17, 10, p.bark);
            d.poly(&[(13, 11), (18, 11), (19, 16), (23, 20), (22, 26), (10, 26), (9, 20), (13, 16)], p.steel, ink);
            d.poly(&[(11, 20), (21, 20), (20, 25), (12, 25)], p.red, None);
            d.line(&[(12, 17), (11, 20)], mix(p.steel, WHITE, 0.4), 1);
        }
        Object::Weapon => {
            d.poly(&[(23, 4), (24, 10), (14, 22), (11, 19)], p.steel, ink);
            d.line(&[(21, 9), (13, 18)], mix(p.steel, WHITE, 0.35), 1);
            d.line(&[(8, 18), (17, 25)], p.gold, 2);
            d.line(&[(11, 23), (7, 28)], p.bark, 2);
        }
        Object::Oathfang => {
            d.line(&[(23, 4), (25, 7), (25, 11), (23, 15), (19, 19), (15, 22)], p.ink, 5);
            d.line(&[(23, 4), (24, 7), (24, 10), (22, 14), (19, 17)], ASH, 3);
            d.line(&[(23, 5), (23, 9), (21, 13), (18, 16)], EMBER, 1);
            d.line(&[(10, 19), (18, 25)], p.gold, 2);
            d.line(&[(13, 23), (9, 28)], p.bark, 2);
        }
        Object::Bow => {
            let arc: Vec<(i32, i32)> = (0..=24)
                .map(|t| {
                    let u = (t as f32 - 12.0) / 12.0;
                    (9 + (12.0 * (1.0 - u * u)).round() as i32, 4 + t)
                })
                .collect();
            d.line(&arc, p.ink, 3);
            d.line(&arc, p.bark, 1);
            d.line(&[(9, 4), (9, 28)], mix(p.steel, WHITE, 0.3), 1);
            d.rect(19, 15, 22, 17, p.red);
        }
        Object::Armor => {
            d.poly(
                &[(10, 8), (13, 11), (18, 11), (22, 8), (27, 13), (23, 17), (22, 26), (10, 26), (9, 17), (5, 13)],
                p.steel,
                ink,
            );
            d.poly(&[(11, 13), (16, 15), (16, 24), (12, 24)], mix(p.steel, WHITE, 0.3), None);
            d.line(&[(10, 24), (22, 24)], p.bark, 1);
        }
        Object::AshenCarapace => {
            d.poly(
                &[(10, 8), (13, 11), (18, 11), (22, 8), (27, 13), (23, 17), (22, 26), (10, 26), (9, 17), (5, 13)],
                ASH,
                ink,
            );
            d.poly(&[(11, 12), (15, 14), (15, 23), (12, 24)], [90, 82, 84], None);
            d.line(&[(8, 13), (11, 15), (10, 18)], p.stone, 1);
            d.line(&[(24, 13), (21, 15), (22, 18)], p.stone, 1);
            d.line(&[(14, 13), (16, 16), (14, 18), (17, 21), (16, 24)], EMBER, 1);
            d.rect(17, 15, 18, 17, EMBER);
            d.line(&[(10, 24), (22, 24)], p.ink, 1);
        }
        Object::Ring => {
            d.ellipse((8, 12, 23, 27), solid(p.gold), ink);
            d.ellipse((12, 16, 19, 23), [0, 0, 0, 0], ink);
            d.line(&[(10, 17), (11, 15)], mix(p.gold, WHITE, 0.45), 1);
            d.ellipse((13, 7, 18, 12), solid(p.glow), ink);
        }
        Object::Relic(relic) => match relic {
            isle_core::Relic::EmberCharm => {
                d.line(&[(9, 6), (11, 13), (21, 13), (23, 6)], p.gold, 2);
                d.poly(&[(16, 12), (23, 20), (16, 29), (9, 20)], p.gold, ink);
                d.poly(&[(16, 15), (20, 20), (16, 26), (12, 20)], p.red, ink);
                d.line(&[(16, 17), (14, 20)], p.glow, 1);
            }
            isle_core::Relic::Stillstone => {
                d.poly(&[(12, 6), (22, 10), (25, 24), (17, 29), (7, 23), (8, 12)], p.steel, ink);
                d.poly(&[(12, 8), (20, 11), (21, 16), (9, 20)], p.stone, None);
                d.line(&[(12, 14), (20, 14), (12, 22), (20, 22)], p.ink, 2);
                d.line(&[(16, 12), (16, 24)], p.glow, 1);
            }
            isle_core::Relic::HuntersToken => {
                d.ellipse((6, 8, 26, 29), solid(p.bark), ink);
                d.ellipse((8, 10, 24, 27), solid(p.gold), ink);
                d.line(&[(12, 24), (21, 13)], p.ink, 2);
                d.poly(&[(17, 13), (23, 11), (22, 17)], p.steel, ink);
                d.line(&[(10, 20), (15, 24)], p.bark, 2);
            }
            isle_core::Relic::ExileHeart => {
                d.shadow(p);
                d.poly(
                    &[(16, 26), (7, 18), (6, 12), (9, 8), (13, 8), (16, 12), (19, 8), (23, 8), (26, 12), (25, 18)],
                    HEART,
                    ink,
                );
                d.poly(&[(16, 23), (10, 17), (9, 12), (12, 10), (16, 15), (20, 10), (23, 12), (22, 17)], p.red, None);
                d.line(&[(7, 5), (7, 15), (11, 22), (16, 28), (21, 22), (25, 15), (25, 5)], p.gold, 2);
                d.line(&[(11, 7), (11, 17), (16, 24)], p.steel, 1);
                d.line(&[(21, 7), (21, 17), (16, 24)], p.steel, 1);
                d.line(&[(8, 10), (24, 10)], p.gold, 1);
                d.line(&[(8, 17), (24, 17)], p.gold, 1);
                d.line(&[(16, 15), (15, 18), (17, 20)], p.glow, 2);
            }
            relic => {
                let depth = isle_core::Relic::CONQUEST.iter().position(|&prize| prize == relic).unwrap() + 1;
                let colors = [p.bark, p.glow, p.steel, p.gold, p.red, p.steel, p.glow, p.stone, p.gold, p.red];
                let color = colors[depth - 1];
                d.line(&[(10, 5), (12, 12), (20, 12), (22, 5)], p.steel, 1);
                let shoulder = 6 + (depth % 3) as i32;
                d.poly(&[(16, 10), (26 - shoulder, 13), (25, 21), (16, 29), (7, 21), (shoulder + 6, 13)], color, ink);
                d.poly(&[(16, 13), (21, 20), (16, 26), (11, 20)], p.ink, None);
                // A distinct four-stroke rune identifies the floor even without color.
                for bit in 0..4 {
                    if depth & (1 << bit) != 0 {
                        let y = 16 + bit * 2;
                        d.line(&[(14, y), (18, y)], p.glow, 1);
                    }
                }
            }
        },
        Object::Treasure => {
            d.shadow(p);
            d.poly(&[(7, 13), (23, 13), (26, 18), (25, 26), (6, 26), (6, 18)], p.bark, ink);
            d.rect(7, 19, 24, 20, p.ink);
            d.rect(9, 14, 10, 25, p.gold);
            d.rect(21, 14, 22, 25, p.gold);
            d.rect(15, 18, 17, 22, p.gold);
            d.line(&[(11, 15), (20, 15)], mix(p.bark, p.gold, 0.35), 1);
        }
    }
    d.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::ALL_OBJECTS;

    #[test]
    fn every_object_is_a_visible_outlined_shape_on_transparency() {
        for object in ALL_OBJECTS {
            for style in STYLES {
                let image = super::object(object, surface(style), false, false, 1);
                let visible = image.pixels().filter(|p| p[3] == 255).count();
                assert!(visible > 40 && visible < 900, "{object:?} {style:?}: {visible}");
                assert!(image.pixels().any(|p| p.0[..3] == surface(style).ink), "{object:?} has an ink outline");
            }
        }
    }

    #[test]
    fn mythic_equipment_and_sites_have_distinct_silhouettes() {
        for (mythic, ordinary) in [
            (Object::Oathfang, Object::Weapon),
            (Object::AshenCarapace, Object::Armor),
            (Object::Relic(isle_core::Relic::ExileHeart), Object::Relic(isle_core::Relic::EmberCharm)),
            (Object::AshenForge, Object::Shrine),
            (Object::WellOfNames, Object::Cairn),
        ] {
            assert_ne!(super::object(mythic, &MOSS, false, false, 1), super::object(ordinary, &MOSS, false, false, 1));
        }
    }

    #[test]
    fn conquest_relics_have_distinct_visible_art() {
        let images: Vec<_> = isle_core::Relic::CONQUEST
            .into_iter()
            .map(|relic| super::object(Object::Relic(relic), &MOSS, false, false, 1))
            .collect();
        for (index, image) in images.iter().enumerate() {
            assert!(image.pixels().filter(|pixel| pixel[3] == 255).count() > 40);
            for other in &images[..index] {
                assert_ne!(image, other);
            }
        }
    }

    #[test]
    fn styles_share_silhouettes_and_change_only_colors() {
        for object in ALL_OBJECTS {
            let masks: Vec<Vec<u8>> = STYLES
                .iter()
                .map(|&s| super::object(object, surface(s), false, false, 1).pixels().map(|p| p[3]).collect())
                .collect();
            assert!(masks.windows(2).all(|w| w[0] == w[1]), "{object:?}");
        }
    }

    #[test]
    fn natural_objects_grow_in_sizes_and_big_crowns_rise_into_the_headroom() {
        let area =
            |object, size| super::object(object, &MOSS, false, false, size).pixels().filter(|p| p[3] == 255).count();
        for object in [Object::Pine, Object::Oak, Object::Acacia, Object::Canopy, Object::Willow, Object::Boulder] {
            assert!(area(object, 0) < area(object, 1) && area(object, 1) < area(object, 2), "{object:?}");
        }
        let above = |object, size| {
            let image = super::object(object, &MOSS, false, false, size);
            (0..HEAD as u32).any(|y| (0..32).any(|x| image.get_pixel(x, y)[3] != 0))
        };
        assert!(above(Object::Canopy, 1) && above(Object::Oak, 2));
        assert!(!above(Object::Oak, 1) && !above(Object::Coins, 1), "only big crowns leave the cell");
    }
}

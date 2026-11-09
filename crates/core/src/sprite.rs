//! Procedural sprites: a 16×16 picture of a creature, drawn from its phenotype and its clade's body plan.
//!
//! The body plan picks the silhouette and its proportions. Genes change what's drawn on top of it:
//!
//! - size scales the whole silhouette, from about 60% of the tile to all of it
//! - limbs sets how many legs, tentacles, or feet are drawn
//! - covering sets the texture and outline: fur tufts, feather fringes, chitin plates and spikes,
//!   scale dither, slime drips and shine
//! - hue sets the palette, which the biome then tints
//!
//! Every creature is drawn on one half and mirrored, on a transparent background, with a dark outline and
//! a small palette: a four-step ramp of the body color plus belly, markings, eyes, and outline. Shading is
//! by position alone (lit from above), so the mirror stays exact.
//!
//! [`Sprite::draw`] is a pure function, built only from exactly rounded arithmetic like the rest of the sim,
//! so a species looks the same on every platform. Drawing is cheap, about a microsecond per pixel at most.

use crate::biome::BiomeId;
use crate::clade::{BodyPlan, Family, Pattern};
use crate::phenotype::{Covering, Phenotype};

/// Width and height of a sprite, in pixels.
pub const SPRITE_SIZE: usize = 16;
const N: usize = SPRITE_SIZE;

/// A 16×16 RGBA image, row-major from the top left, with straight (not premultiplied) alpha.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sprite([u8; N * N * 4]);

impl Sprite {
    /// The pixels as RGBA bytes, row by row from the top.
    #[must_use]
    pub const fn rgba(&self) -> &[u8; N * N * 4] {
        &self.0
    }

    /// The pixel at `(x, y)`, from the top left. Both must be below [`SPRITE_SIZE`].
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let i = (y * N + x) * 4;
        [self.0[i], self.0[i + 1], self.0[i + 2], self.0[i + 3]]
    }

    /// Draws a creature: its phenotype in its clade's body plan, tinted by the biome it lives in.
    #[must_use]
    pub fn draw(phenotype: &Phenotype, plan: &BodyPlan, biome: BiomeId) -> Sprite {
        let mut c = Canvas::new(phenotype.size);
        let pairs = phenotype.legs / 2;
        match plan.family {
            Family::Crawler => crawler(&mut c, plan, pairs),
            Family::Serpent => serpent(&mut c, plan, pairs),
            Family::Beast => beast(&mut c, plan, pairs),
            Family::Flier => flier(&mut c, plan, pairs),
            Family::Blob => blob(&mut c, plan, pairs),
            Family::Crab => crab(&mut c, plan, pairs),
            Family::Toad => toad(&mut c, plan, pairs),
            Family::Ray => ray(&mut c, plan, pairs),
        }
        c.covering_silhouette(phenotype.covering);
        c.outline();
        if phenotype.covering == Covering::Chitin {
            c.spikes();
        }
        let palette = Palette::new(phenotype, plan, biome);
        c.paint(&palette, phenotype.covering, plan.pattern)
    }
}

/// What a pixel is made of, before it has a color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mat {
    Empty,
    /// The body color, shaded.
    Body,
    /// The lighter underside.
    Belly,
    /// A thick limb, shaded a step darker than the body.
    Stout,
    /// A wing or fin, shaded a step darker than the body.
    Wing,
    Eye,
    /// Horn, beak, claw, or tooth.
    Claw,
    /// A dark opening: mouth or nose.
    Mouth,
    /// A white glint.
    Shine,
    /// A red tongue.
    Tongue,
    /// A thin limb, drawn as a dark line with no outline of its own.
    Limb,
    Outline,
    /// An outline-colored point sticking out of the outline.
    Spike,
}

impl Mat {
    /// Whether the pixel is part of the creature's mass, which gets an outline.
    const fn solid(self) -> bool {
        matches!(
            self,
            Mat::Body
                | Mat::Belly
                | Mat::Stout
                | Mat::Wing
                | Mat::Eye
                | Mat::Claw
                | Mat::Mouth
                | Mat::Shine
                | Mat::Tongue
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    mat: Mat,
    /// Paint order of the part this pixel belongs to. A part's edge against a part painted earlier is
    /// shaded like an edge against the background, which is what separates a head from the body behind it.
    layer: u8,
}

/// A sprite being drawn. Shapes are given in local units on the right half (`u ≥ 0` from the midline, `v`
/// down from the center), scaled by size, and always painted mirrored.
struct Canvas {
    cells: [[Cell; N]; N],
    /// Pixels per local unit.
    k: f32,
    /// The layer subsequent shapes paint on.
    layer: u8,
    /// The layer markings go on.
    body_layer: u8,
}

/// Center line: pixels `0..8` mirror `8..16`.
const MID: f32 = N as f32 / 2.0;

impl Canvas {
    fn new(size: f32) -> Canvas {
        let empty = Cell { mat: Mat::Empty, layer: 0 };
        Canvas { cells: [[empty; N]; N], k: 0.58 + 0.42 * size, layer: 1, body_layer: 1 }
    }

    fn at(&self, x: i32, y: i32) -> Mat {
        if (0..N as i32).contains(&x) && (0..N as i32).contains(&y) {
            self.cells[y as usize][x as usize].mat
        } else {
            Mat::Empty
        }
    }

    /// Sets pixel `(x, y)` and its mirror. Solid pixels keep a one-pixel margin free for the outline.
    fn put(&mut self, x: i32, y: i32, mat: Mat) {
        let (lo, hi) = if mat.solid() { (1, N as i32 - 2) } else { (0, N as i32 - 1) };
        if !(lo..=hi).contains(&x) || !(lo..=hi).contains(&y) {
            return;
        }
        let cell = Cell { mat, layer: self.layer };
        self.cells[y as usize][x as usize] = cell;
        self.cells[y as usize][N - 1 - x as usize] = cell;
    }

    /// Local coordinates of the center of right-half pixel `(x, y)`.
    fn local(&self, x: usize, y: usize) -> (f32, f32) {
        ((x as f32 + 0.5 - MID) / self.k, (y as f32 + 0.5 - MID) / self.k)
    }

    /// Pixel coordinates of a local point.
    fn pixel(&self, u: f32, v: f32) -> (i32, i32) {
        ((MID + u * self.k).floor() as i32, (MID + v * self.k).floor() as i32)
    }

    /// Paints every right-half pixel whose center passes `inside`, and its mirror. `inside` sees `|u|`, so a
    /// shape off the midline is mirrored onto itself where it crosses it.
    fn fill(&mut self, mat: Mat, inside: impl Fn(f32, f32) -> bool) {
        for y in 0..N {
            for x in N / 2..N {
                let (u, v) = self.local(x, y);
                if inside(u, v) || inside(-u, v) {
                    self.put(x as i32, y as i32, mat);
                }
            }
        }
    }

    fn ellipse(&mut self, cu: f32, cv: f32, ru: f32, rv: f32, mat: Mat) {
        self.fill(mat, |u, v| in_ellipse(u, v, cu, cv, ru, rv));
    }

    /// An ellipse cut off below `v_max`.
    fn ellipse_above(&mut self, cu: f32, cv: f32, ru: f32, rv: f32, v_max: f32, mat: Mat) {
        self.fill(mat, |u, v| v <= v_max && in_ellipse(u, v, cu, cv, ru, rv));
    }

    /// A filled polygon with vertices on the right half.
    fn polygon(&mut self, points: &[(f32, f32)], mat: Mat) {
        self.fill(mat, |u, v| {
            let mut inside = false;
            let mut j = points.len() - 1;
            for i in 0..points.len() {
                let ((ui, vi), (uj, vj)) = (points[i], points[j]);
                if (vi > v) != (vj > v) && u < (uj - ui) * (v - vi) / (vj - vi) + ui {
                    inside = !inside;
                }
                j = i;
            }
            inside
        });
    }

    /// A one-pixel line.
    fn line(&mut self, (u0, v0): (f32, f32), (u1, v1): (f32, f32), mat: Mat) {
        let (du, dv) = (u1 - u0, v1 - v0);
        let steps = ((du * du + dv * dv).sqrt() * self.k * 3.0).ceil().max(1.0) as usize;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let (x, y) = self.pixel(u0 + (u1 - u0) * t, v0 + (v1 - v0) * t);
            self.put(x, y, mat);
        }
    }

    /// A line `thick` local units from its spine, at least a pixel wide.
    fn thick(&mut self, (u0, v0): (f32, f32), (u1, v1): (f32, f32), thick: f32, mat: Mat) {
        let r = thick.max(0.5 / self.k);
        self.fill(mat, |u, v| {
            let (du, dv) = (u1 - u0, v1 - v0);
            let t = (((u - u0) * du + (v - v0) * dv) / (du * du + dv * dv).max(1e-6)).clamp(0.0, 1.0);
            let (eu, ev) = (u - u0 - du * t, v - v0 - dv * t);
            eu * eu + ev * ev <= r * r
        });
    }

    fn dot(&mut self, u: f32, v: f32, mat: Mat) {
        let (x, y) = self.pixel(u, v);
        self.put(x, y, mat);
    }

    /// A pair of eyes, kept at least a column off the midline so they never merge into one.
    fn eyes(&mut self, u: f32, v: f32) {
        let (x, y) = self.pixel(u, v);
        self.put(x.max(N as i32 / 2 + 1), y, Mat::Eye);
    }

    /// Clears a pixel, for notches.
    fn carve(&mut self, u: f32, v: f32) {
        let (x, y) = self.pixel(u, v);
        if (0..N as i32).contains(&x) && (0..N as i32).contains(&y) {
            let empty = Cell { mat: Mat::Empty, layer: 0 };
            self.cells[y as usize][x as usize] = empty;
            self.cells[y as usize][N - 1 - x as usize] = empty;
        }
    }

    /// Topmost and bottommost solid rows of right-half column `x`.
    fn column_span(&self, x: usize) -> Option<(usize, usize)> {
        let top = (0..N).find(|&y| self.cells[y][x].mat.solid())?;
        let bottom = (0..N).rev().find(|&y| self.cells[y][x].mat.solid())?;
        Some((top, bottom))
    }

    /// The outermost solid pixel of row `y` on the right half.
    fn row_edge(&self, y: usize) -> Option<usize> {
        (N / 2..N).rev().find(|&x| self.cells[y][x].mat.solid())
    }

    /// Grows a pixel of `mat` at `(x, y)` (right half) if it is empty, on the layer of `from`.
    fn grow(&mut self, x: usize, y: usize, from: (usize, usize), mat: Mat) {
        if self.at(x as i32, y as i32) == Mat::Empty {
            let layer = self.layer;
            self.layer = self.cells[from.1][from.0].layer;
            self.put(x as i32, y as i32, mat);
            self.layer = layer;
        }
    }

    /// Changes to the silhouette that come from the covering, before outlining.
    fn covering_silhouette(&mut self, covering: Covering) {
        match covering {
            // Tufts sticking out from the sides and the top.
            Covering::Fur => {
                for y in (1..N - 1).step_by(2) {
                    if let Some(x) = self.row_edge(y).filter(|&x| x < N - 2 && self.cells[y][x].mat == Mat::Body) {
                        self.grow(x + 1, y, (x, y), Mat::Body);
                    }
                }
                for x in (N / 2 + 1..N - 1).step_by(2) {
                    if let Some((top, _)) = self.column_span(x).filter(|&(t, _)| t > 1)
                        && self.cells[top][x].mat == Mat::Body
                    {
                        self.grow(x, top - 1, (x, top), Mat::Body);
                    }
                }
            }
            // A crest on the head and a fringe along the bottom of wings and body.
            Covering::Feather => {
                if let Some((top, _)) = self.column_span(N / 2).filter(|&(t, _)| t > 1) {
                    self.grow(N / 2, top - 1, (N / 2, top), Mat::Body);
                }
                for x in (N / 2..N - 1).step_by(2) {
                    if let Some((_, bottom)) = self.column_span(x).filter(|&(_, b)| b < N - 2) {
                        let mat = self.cells[bottom][x].mat;
                        if matches!(mat, Mat::Body | Mat::Wing) {
                            self.grow(x, bottom + 1, (x, bottom), mat);
                        }
                    }
                }
            }
            // Drips hanging from the underside.
            Covering::Slime => {
                for x in (N / 2 + 1..N - 1).step_by(3) {
                    if let Some((_, bottom)) = self.column_span(x).filter(|&(_, b)| b < N - 2)
                        && self.cells[bottom][x].mat == Mat::Body
                    {
                        self.grow(x, bottom + 1, (x, bottom), Mat::Body);
                    }
                }
            }
            Covering::Chitin | Covering::Scale => {}
        }
    }

    /// Outlines every solid pixel on its four sides.
    fn outline(&mut self) {
        let solid = |c: &Canvas, x: i32, y: i32| c.at(x, y).solid();
        let mut marks = Vec::new();
        for y in 0..N as i32 {
            for x in 0..N as i32 {
                if self.at(x, y) == Mat::Empty
                    && (solid(self, x - 1, y)
                        || solid(self, x + 1, y)
                        || solid(self, x, y - 1)
                        || solid(self, x, y + 1))
                {
                    marks.push((x as usize, y as usize));
                }
            }
        }
        for (x, y) in marks {
            self.cells[y][x] = Cell { mat: Mat::Outline, layer: 0 };
        }
    }

    /// Spines along the sides of a shell, every third row, past the outline.
    fn spikes(&mut self) {
        for y in (2..N - 2).step_by(3) {
            let Some(x) = (N / 2..N).rev().find(|&x| self.cells[y][x].mat == Mat::Outline) else { continue };
            if x + 1 < N && self.cells[y][x - 1].mat == Mat::Body && self.cells[y][x + 1].mat == Mat::Empty {
                self.cells[y][x + 1] = Cell { mat: Mat::Spike, layer: 0 };
                self.cells[y][N - 2 - x] = Cell { mat: Mat::Spike, layer: 0 };
            }
        }
    }

    /// Whether the neighbor of `(x, y)` at `(nx, ny)` counts as outside the pixel's part: empty, not solid,
    /// or a part painted behind it.
    fn outside(&self, x: usize, y: usize, nx: i32, ny: i32) -> bool {
        if !(0..N as i32).contains(&nx) || !(0..N as i32).contains(&ny) {
            return true;
        }
        let n = self.cells[ny as usize][nx as usize];
        !n.mat.solid() || n.layer < self.cells[y][x].layer
    }

    /// Colors every pixel.
    fn paint(&self, p: &Palette, covering: Covering, pattern: Pattern) -> Sprite {
        let mut out = [0u8; N * N * 4];
        let body_top = (0..N).find(|&y| self.cells[y].iter().any(|c| c.layer == self.body_layer && c.mat == Mat::Body));
        let top_of = |x: usize| self.column_span(x).map_or(0, |(t, _)| t);
        for y in 0..N {
            for x in 0..N {
                let cell = self.cells[y][x];
                // Distance from the midline in columns, the same for a pixel and its mirror.
                let i = if x >= N / 2 { x - N / 2 } else { N / 2 - 1 - x };
                let (xi, yi) = (x as i32, y as i32);
                let top = self.outside(x, y, xi, yi - 1);
                let bottom = self.outside(x, y, xi, yi + 1);
                let side = self.outside(x, y, xi - 1, yi) || self.outside(x, y, xi + 1, yi);
                let shade = if top {
                    3
                } else if bottom {
                    0
                } else if side {
                    1
                } else {
                    2
                };
                let rgb = match cell.mat {
                    Mat::Empty => continue,
                    Mat::Body => {
                        let mut s = texture(covering, shade, i, y, top_of(x));
                        let marked = cell.layer == self.body_layer && !side && !bottom && s >= 1;
                        let row = body_top.map_or(0, |t| y.saturating_sub(t));
                        let accent = marked
                            && match pattern {
                                Pattern::Plain => false,
                                Pattern::Stripes => row % 3 == 2,
                                Pattern::Spots => (i + 2 * y) % 5 == 1 && y % 2 == 0,
                                Pattern::Dorsal => i == 0 && !top,
                            };
                        if accent {
                            if shade == 3 { p.accent_light } else { p.accent }
                        } else {
                            if covering == Covering::Slime && shade == 3 && i == 2 {
                                s = 4;
                            }
                            p.ramp(s)
                        }
                    }
                    Mat::Belly => {
                        if side || bottom {
                            p.ramp(2)
                        } else {
                            p.belly
                        }
                    }
                    Mat::Stout | Mat::Wing => {
                        let s = texture(covering, shade, i, y, top_of(x));
                        p.ramp(s.saturating_sub(1))
                    }
                    Mat::Eye => p.eye,
                    Mat::Claw => {
                        if top {
                            p.claw_light
                        } else {
                            p.claw
                        }
                    }
                    Mat::Mouth => p.outline,
                    Mat::Shine => p.shine,
                    Mat::Tongue => TONGUE,
                    Mat::Limb => p.limb,
                    Mat::Outline | Mat::Spike => p.outline,
                };
                let o = (y * N + x) * 4;
                out[o..o + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        Sprite(out)
    }
}

/// Whether `(u, v)` lies in the ellipse centered at `(cu, cv)` with radii `ru` and `rv`.
fn in_ellipse(u: f32, v: f32, cu: f32, cv: f32, ru: f32, rv: f32) -> bool {
    let (a, b) = ((u - cu) / ru, (v - cv) / rv);
    a * a + b * b <= 1.0
}

/// The shade a covering gives an interior pixel, from the position shade (0 dark to 3 lit). Slime returns
/// 4 for a glint.
fn texture(covering: Covering, shade: u8, i: usize, y: usize, top: usize) -> u8 {
    if shade != 2 {
        return shade;
    }
    match covering {
        // Strokes that mirror into chevrons.
        Covering::Fur => {
            if (i + y).is_multiple_of(3) {
                1
            } else {
                2
            }
        }
        // Scallops: a short dark arc every three rows.
        Covering::Feather => {
            if (y + 3 - top % 3).is_multiple_of(3) && i.is_multiple_of(2) {
                1
            } else {
                2
            }
        }
        // Plates with a glossy streak.
        Covering::Chitin => {
            if (y + 3 - top % 3).is_multiple_of(3) {
                1
            } else if i == 1 {
                3
            } else {
                2
            }
        }
        // A dither of dark scales.
        Covering::Scale => {
            if (i + y).is_multiple_of(2) && y.is_multiple_of(2) {
                1
            } else {
                2
            }
        }
        Covering::Slime => 2,
    }
}

/// A creature's colors.
struct Palette {
    /// Body, dark to light, then a glint.
    ramp: [[u8; 3]; 5],
    belly: [u8; 3],
    accent: [u8; 3],
    accent_light: [u8; 3],
    eye: [u8; 3],
    claw: [u8; 3],
    claw_light: [u8; 3],
    shine: [u8; 3],
    limb: [u8; 3],
    outline: [u8; 3],
}

/// How far toward the biome's own color a creature's colors are pulled.
const TINT: f32 = 0.12;
/// A forked tongue's red.
const TONGUE: [u8; 3] = [214, 52, 64];

impl Palette {
    fn new(p: &Phenotype, plan: &BodyPlan, biome: BiomeId) -> Palette {
        let h = p.hue;
        // Saturation and value by covering: fur is soft, slime vivid, chitin deep.
        let (s, v) = match p.covering {
            Covering::Fur => (0.42, 0.72),
            Covering::Feather => (0.55, 0.80),
            Covering::Chitin => (0.62, 0.66),
            Covering::Scale => (0.52, 0.70),
            Covering::Slime => (0.70, 0.82),
        };
        // Pigment follows the genes that climate selects: creatures hardened to heat or cold bleach toward
        // pale (sand-pale in desert, near-white in tundra), and water-bound ones run dark and rich.
        let bleach = ((p.heat - 0.7).max(0.0) * 2.0 + (p.cold - 0.6).max(0.0) * 1.5).min(0.65);
        let wet = ((p.water - 0.5) * 2.0).max(0.0);
        let s = s * (1.0 - bleach) * (1.0 + 0.3 * wet);
        let v = (v + (1.0 - v) * bleach * 0.6) * (1.0 - 0.22 * wet);
        let tint = biome.biome().color;
        let t = |c: [u8; 3]| mix(c, tint, TINT);
        // Shadows lean toward blue and lights toward yellow, the usual pixel-art ramp.
        let step = |shift: f32, ds: f32, dv: f32| t(hsv(toward(h, shift), (s * ds).min(1.0), (v * dv).min(1.0)));
        let ramp = [
            step(-0.06, 1.1, 0.42),
            step(-0.03, 1.05, 0.66),
            step(0.0, 1.0, 0.88),
            step(0.03, 0.8, 1.08),
            t(hsv(toward(h, 0.06), s * 0.3, 1.0)),
        ];
        // Markings stay readable on any body: dark if the hues are close, else mid.
        let close = crate::genome::gene_delta(crate::genome::Gene::Hue, h, plan.accent).abs() < 0.12;
        let accent_v = if close { 0.38 } else { 0.62 };
        let outline_s = if p.covering == Covering::Slime { 0.75 } else { 0.5 };
        let outline = hsv(toward(h, -0.08), outline_s, if p.covering == Covering::Slime { 0.28 } else { 0.15 });
        Palette {
            ramp,
            belly: t(hsv(toward(h, 0.05), s * 0.35, (v * 1.15).min(1.0))),
            accent: t(hsv(plan.accent, 0.7, accent_v)),
            accent_light: t(hsv(plan.accent, 0.55, accent_v + 0.2)),
            eye: plan.eye,
            claw: t([200, 190, 160]),
            claw_light: t([240, 232, 205]),
            shine: [255, 255, 245],
            limb: mix(ramp[0], outline, 0.5),
            outline,
        }
    }

    fn ramp(&self, shade: u8) -> [u8; 3] {
        self.ramp[usize::from(shade.min(4))]
    }
}

/// Hue `h` nudged by `amount` toward yellow (positive) or blue (negative), the short way round.
fn toward(h: f32, amount: f32) -> f32 {
    let target = if amount >= 0.0 { 1.0 / 6.0 } else { 2.0 / 3.0 };
    let d = crate::genome::gene_delta(crate::genome::Gene::Hue, h, target);
    (h + d.signum() * amount.abs().min(d.abs())).rem_euclid(1.0)
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let m = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// An RGB color from hue `[0, 1)`, saturation, and value.
pub(crate) fn hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h6 = h.rem_euclid(1.0) * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h6 % 2.0 - 1.0).abs());
    let (r, g, b) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let byte = |f: f32| ((f + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(r), byte(g), byte(b)]
}

// The families. Each paints back to front, raising the layer for each part that should read as separate.
// Local units: at full size one unit is one pixel, and the right half spans u = 0 to 7.

fn crawler(c: &mut Canvas, p: &BodyPlan, pairs: u8) {
    let (w, l, h) = (p.width, p.length, p.head);
    // Legs at least two pixels apart, so small crawlers keep separate legs instead of a dark smear.
    let spacing = (2.4 * l).max(2.1 / c.k);
    for j in 0..pairs {
        let va = 0.2 + (f32::from(j) - f32::from(pairs.saturating_sub(1)) / 2.0) * spacing;
        // Reach is set in pixels, not body units, so legs clear the outline at any size.
        let knee = (2.0 * w + 2.4 / c.k, va - 0.7);
        c.line((2.0 * w, va), knee, Mat::Limb);
        c.line(knee, (knee.0 + 1.6 / c.k, va + 1.0 / c.k), Mat::Limb);
    }
    c.line((0.7, -5.2 * l), (2.6, -7.2), Mat::Limb);
    c.layer = 2;
    c.body_layer = 2;
    c.ellipse(0.0, 1.8 * l, 3.3 * w, 3.9 * l, Mat::Body);
    c.layer = 3;
    c.ellipse(0.0, -2.6 * l, 2.4 * w, 1.5 * l, Mat::Body);
    c.layer = 4;
    c.ellipse(0.0, -4.5 * l, 1.5 * h, 1.2 * h, Mat::Body);
    c.eyes(1.0 * h, -4.8 * l);
    if p.feature > 0.5 {
        c.dot(0.6, -5.9 * l, Mat::Claw);
    }
}

fn serpent(c: &mut Canvas, p: &BodyPlan, pairs: u8) {
    let (w, h) = (p.width, p.head);
    for j in 0..pairs {
        let dv = 0.8 * f32::from(j);
        c.line((4.4 * w, 4.0 + dv), (6.6 * w, 5.4 + dv), Mat::Limb);
    }
    // Three coils stacked into a cone; each one's lower edge shades dark against the one behind it.
    c.layer = 2;
    c.ellipse(0.0, 5.0, 5.4 * w, 1.8, Mat::Body);
    c.layer = 3;
    c.body_layer = 3;
    c.ellipse(0.0, 3.4, 4.0 * w, 1.5, Mat::Body);
    c.layer = 4;
    c.ellipse(0.0, 2.0, 2.6 * w, 1.2, Mat::Body);
    // The neck rises from the top coil, with a small hood if the plan has one.
    c.layer = 5;
    c.ellipse(0.0, -0.4, 1.1 * w, 2.6, Mat::Body);
    c.ellipse(0.0, 0.2, 0.5, 1.6, Mat::Belly);
    if p.feature > 0.55 {
        c.ellipse(0.0, -1.4, (1.6 + 1.6 * (p.feature - 0.55)) * w, 1.5, Mat::Body);
    }
    c.layer = 6;
    c.ellipse(0.0, -3.4, 1.9 * h, 1.3 * h, Mat::Body);
    c.eyes(1.0 * h, -3.6);
    c.layer = 7;
    c.line((0.0, -2.0), (0.0, -1.4), Mat::Tongue);
}

fn beast(c: &mut Canvas, p: &BodyPlan, pairs: u8) {
    let (w, l, h) = (p.width, p.length, p.head);
    for j in 0..pairs {
        let u = (1.7 + 1.5 * f32::from(j)) * w;
        c.thick((u, 3.0), (u + 0.3 * f32::from(j), 6.6 - 0.5 * f32::from(j)), 0.9, Mat::Stout);
    }
    if p.feature < 0.5 {
        c.ellipse(2.3 * h, -5.0, 1.0, 1.3, Mat::Body);
    }
    c.layer = 2;
    c.body_layer = 2;
    c.ellipse(0.0, 1.6 * l, 4.3 * w, 3.5 * l, Mat::Body);
    c.ellipse(0.0, 2.4 * l, 2.0 * w, 2.2 * l, Mat::Belly);
    c.layer = 3;
    c.ellipse(0.0, -2.6, 2.9 * h, 2.4 * h, Mat::Body);
    c.layer = 4;
    c.ellipse(0.0, -1.7, 1.3 * h, 0.9 * h, Mat::Belly);
    c.dot(0.0, -2.2, Mat::Mouth);
    c.eyes(1.3 * h, -3.2);
    if p.feature >= 0.5 {
        c.layer = 5;
        c.thick((1.9 * h, -4.4), (3.4 * h, -6.6), 0.4, Mat::Claw);
    }
}

fn flier(c: &mut Canvas, p: &BodyPlan, pairs: u8) {
    let (w, l, h) = (p.width, p.length, p.head);
    let span = 5.6 + 1.3 * p.feature;
    c.polygon(
        &[(0.8, -1.8), (3.2, -3.6), (span, -3.4), (span - 0.6, -1.0), (span - 1.4, 0.6), (2.6, 1.8), (0.8, 1.4)],
        Mat::Wing,
    );
    c.ellipse(0.0, 4.4, 1.5, 1.6, Mat::Wing);
    for j in 0..pairs {
        let j = f32::from(j);
        c.line((0.6 + 0.7 * j, 3.4), (0.9 + 0.9 * j, 5.6), Mat::Limb);
    }
    if p.feature >= 0.5 {
        c.ellipse(1.4 * h, -4.6, 0.6, 1.0, Mat::Body);
    }
    c.layer = 2;
    c.body_layer = 2;
    c.ellipse(0.0, 0.6 * l, 1.9 * w, 3.0 * l, Mat::Body);
    c.ellipse(0.0, 1.2, 0.9, 1.8, Mat::Belly);
    c.layer = 3;
    c.ellipse(0.0, -2.9, 1.7 * h, 1.5 * h, Mat::Body);
    c.eyes(0.8 * h, -3.2);
    if p.feature < 0.5 {
        c.layer = 4;
        c.dot(0.0, -2.0, Mat::Claw);
    }
}

fn blob(c: &mut Canvas, p: &BodyPlan, pairs: u8) {
    let (w, l) = (p.width, p.length);
    for j in 0..pairs {
        let u = 0.8 + 1.4 * f32::from(j);
        c.line((u, 4.0), (u + 0.6, 7.0), Mat::Limb);
    }
    let stalks = p.feature >= 0.4;
    let tip = (1.8 + 1.2 * p.feature, -5.2 - 0.8 * p.feature);
    if stalks {
        c.line((1.2, -2.5), (tip.0, tip.1 + 0.6), Mat::Limb);
    }
    c.layer = 2;
    c.body_layer = 2;
    c.ellipse_above(0.0, 1.0 * l, 5.0 * w, 4.6 * l, 4.4, Mat::Body);
    for u in [0.6, 2.6, 4.4] {
        c.ellipse(u * w, 4.4, 0.8, 0.9, Mat::Body);
    }
    c.layer = 3;
    if stalks {
        c.eyes(tip.0, tip.1);
    } else {
        c.eyes(1.7 * w, -0.4);
    }
    c.dot(2.6 * w, -1.8 * l, Mat::Shine);
    c.line((0.0, 1.6), (1.2, 1.4), Mat::Mouth);
}

fn crab(c: &mut Canvas, p: &BodyPlan, pairs: u8) {
    let (w, l) = (p.width, p.length);
    let spacing = 1.1_f32.max(1.6 / c.k);
    for j in 0..pairs {
        let t = f32::from(j);
        let va = -0.4 + spacing * t;
        let knee = (3.4 * w + 2.0 / c.k, va - 0.8 + 0.3 * t);
        c.line((3.4 * w, va), knee, Mat::Limb);
        c.line(knee, (knee.0 + 1.4 / c.k, va + 1.4 + 0.3 * t), Mat::Limb);
    }
    let reach = 3.8 + 0.6 * p.feature;
    c.thick((2.6, -2.0), (reach, -3.8), 0.5, Mat::Stout);
    c.layer = 2;
    c.body_layer = 2;
    c.ellipse(0.0, 0.8 * l, 4.4 * w, 3.1 * l, Mat::Body);
    c.layer = 3;
    let size = 1.1 + 0.6 * p.feature;
    let claw = (reach + 0.4, -5.0);
    c.ellipse(claw.0, claw.1, size, size * 0.95, Mat::Body);
    c.carve(claw.0 - 0.3, claw.1 - size * 0.75);
    c.line((0.9, -2.0), (1.2, -3.2), Mat::Limb);
    c.eyes(1.2, -3.6);
}

fn toad(c: &mut Canvas, p: &BodyPlan, pairs: u8) {
    let (w, l, h) = (p.width, p.length, p.head);
    if pairs >= 2 {
        c.ellipse(4.4 * w, 3.4, 1.8, 2.1, Mat::Body);
        c.thick((3.4 * w, 5.8), (6.2 * w, 6.4), 0.5, Mat::Stout);
    }
    for j in 2..pairs {
        let dv = 1.2 * f32::from(j - 2);
        c.line((4.6 * w, 0.6 + dv), (6.6 * w, 1.6 + dv), Mat::Limb);
    }
    c.layer = 2;
    c.body_layer = 2;
    c.ellipse(0.0, 2.0 * l, 4.6 * w, 3.4 * l, Mat::Body);
    c.ellipse(0.0, 2.8, 2.6 * w, 2.0, Mat::Belly);
    if pairs >= 1 {
        c.layer = 3;
        c.thick((1.9 * w, 3.8), (2.2 * w, 6.3), 0.6, Mat::Stout);
    }
    c.layer = 4;
    c.ellipse(0.0, -1.4, 3.9 * h, 2.3 * h, Mat::Body);
    c.layer = 5;
    c.ellipse(2.5 * h, -3.3, 1.3, 1.2, Mat::Body);
    c.eyes(2.5 * h, -3.5);
    c.line((0.0, -0.5), (2.8 * h, -1.0), Mat::Mouth);
}

fn ray(c: &mut Canvas, p: &BodyPlan, pairs: u8) {
    let (w, l) = (p.width, p.length);
    c.line((0.4, 3.0), (0.4, 6.4 * l), Mat::Limb);
    for j in 0..pairs {
        let dv = 0.8 * f32::from(j);
        c.line((1.6, 2.4 + dv), (2.6, 3.6 + dv), Mat::Limb);
    }
    // Wings swept back like a manta's, so the outline is a chevron rather than a cap.
    c.layer = 2;
    c.body_layer = 2;
    let span = 7.0 * w * (0.85 + 0.15 * p.feature);
    c.polygon(
        &[(0.0, -3.6 * l), (2.0, -3.2 * l), (span, 1.2), (span - 0.8, 2.4), (3.0, 1.6), (1.4, 3.4), (0.0, 3.4)],
        Mat::Body,
    );
    c.layer = 3;
    c.ellipse(0.0, -0.4, 1.7, 3.4 * l, Mat::Body);
    c.layer = 4;
    // Head lobes either side of the mouth.
    c.ellipse(1.6, -3.9 * l, 0.6, 1.0 + 0.4 * p.feature, Mat::Body);
    c.eyes(1.0, -2.6 * l);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clade::clades;
    use crate::genome::{Gene, Genome};

    fn sprites() -> Vec<Sprite> {
        let mut out = Vec::new();
        for clade in clades(7) {
            for (i, size) in [0.0, 0.5, 1.0].into_iter().enumerate() {
                let mut genes = clade.base.genes();
                genes[0] = size;
                genes[1] = i as f32 * 0.45;
                genes[2] = (i as f32 * 0.37 + 0.1) % 1.0;
                let p = Phenotype::of(&Genome::new(genes));
                out.push(Sprite::draw(&p, &clade.plan, BiomeId(i as u8 * 4)));
            }
        }
        out
    }

    #[test]
    fn drawing_is_deterministic() {
        assert_eq!(sprites(), sprites());
    }

    #[test]
    fn sprites_are_mirrored_with_a_transparent_background() {
        for (n, s) in sprites().iter().enumerate() {
            for y in 0..N {
                for x in 0..N {
                    assert_eq!(s.pixel(x, y), s.pixel(N - 1 - x, y), "sprite {n} at ({x}, {y})");
                }
            }
            for (x, y) in [(0, 0), (N - 1, 0), (0, N - 1), (N - 1, N - 1)] {
                assert_eq!(s.pixel(x, y)[3], 0, "sprite {n} corner ({x}, {y})");
            }
            let opaque = s.rgba().chunks(4).filter(|p| p[3] == 255).count();
            assert!((20..N * N * 3 / 4).contains(&opaque), "sprite {n}: {opaque} opaque pixels");
            assert!(s.rgba().chunks(4).all(|p| p[3] == 0 || p[3] == 255));
        }
    }

    #[test]
    fn genes_change_the_picture() {
        // Each gene the module doc says shows, moved between two values that differ in what is drawn, changes
        // the sprite for every family. Heat and cold only bleach past a threshold, and water only darkens past
        // wading, so their pairs straddle those.
        let pairs = [
            (Gene::Size, 0.1, 0.9),
            (Gene::Limbs, 0.0, 1.0),
            (Gene::Covering, 0.05, 0.95),
            (Gene::Heat, 0.2, 1.0),
            (Gene::Cold, 0.2, 1.0),
            (Gene::Water, 0.3, 1.0),
            (Gene::Hue, 0.1, 0.6),
        ];
        let biome = BiomeId(14);
        for clade in clades(7) {
            for (gene, a, b) in pairs {
                let draw = |v: f32| {
                    let mut g = clade.base;
                    g.set(gene, v);
                    Sprite::draw(&Phenotype::of(&g), &clade.plan, biome)
                };
                assert_ne!(draw(a), draw(b), "{:?}: {gene:?} {a} vs {b} looks the same", clade.plan.family);
            }
        }
    }

    /// Hue `[0, 1)`, saturation, and value of an RGB color: the inverse of [`hsv`], to check it against.
    fn rgb_to_hsv([r, g, b]: [u8; 3]) -> (f32, f32, f32) {
        let (r, g, b) = (f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let h = if d == 0.0 {
            0.0
        } else if max == r {
            ((g - b) / d).rem_euclid(6.0) / 6.0
        } else if max == g {
            ((b - r) / d + 2.0) / 6.0
        } else {
            ((r - g) / d + 4.0) / 6.0
        };
        let s = if max == 0.0 { 0.0 } else { d / max };
        (h, s, max)
    }

    #[test]
    fn color_conversions_round_trip() {
        for rgb in [[255, 0, 0], [12, 200, 90], [222, 196, 120], [10, 10, 10]] {
            let (h, s, v) = rgb_to_hsv(rgb);
            let back = hsv(h, s, v);
            for i in 0..3 {
                assert!((i32::from(back[i]) - i32::from(rgb[i])).abs() <= 1, "{rgb:?} -> {back:?}");
            }
        }
    }
}

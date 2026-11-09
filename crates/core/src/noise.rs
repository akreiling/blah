//! Gradient noise and fractal layering on an integer tile grid.
//!
//! This is hand-rolled rather than taken from a crate for two reasons. Positions stay split into an `i64`
//! lattice cell plus a small `f64` offset inside it, so precision is the same at tile 10^12 as at tile 0;
//! crates that take `f64` coordinates lose fractional bits far from the origin. And gradients come from
//! hashing the cell, not from a 256-entry permutation table, so the noise never repeats.
//!
//! Only `+ - * /`, `sqrt`, and `floor` are used. Those are exactly rounded under IEEE 754, and Rust never
//! fuses them into FMAs, so the same seed gives bit-identical terrain on every platform we target. The
//! platform's `exp` or `sin` would not guarantee that.

use crate::hash::{combine, hash_point, mix};

/// One layer of fractal noise.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Octave {
    /// Lattice spacing in tiles. An integer, so a tile's cell and offset are exact.
    pub wavelength: i64,
    /// Relative weight in the sum.
    pub amplitude: f64,
}

/// Standard deviation of [`gradient`] over uniformly random points, measured over 4M samples. It turns the
/// fractal sum into a z-score; `fields::tests::fields_are_roughly_uniform` catches it drifting.
const GRADIENT_STD: f64 = 0.3051;

/// Sixteen unit gradients, offset half a step from the axes so no gradient lines up with the grid.
const GRADIENTS: [(f64, f64); 16] = [
    (0.980_785_3, 0.195_090_3),
    (0.831_469_6, 0.555_570_2),
    (0.555_570_2, 0.831_469_6),
    (0.195_090_3, 0.980_785_3),
    (-0.195_090_3, 0.980_785_3),
    (-0.555_570_2, 0.831_469_6),
    (-0.831_469_6, 0.555_570_2),
    (-0.980_785_3, 0.195_090_3),
    (-0.980_785_3, -0.195_090_3),
    (-0.831_469_6, -0.555_570_2),
    (-0.555_570_2, -0.831_469_6),
    (-0.195_090_3, -0.980_785_3),
    (0.195_090_3, -0.980_785_3),
    (0.555_570_2, -0.831_469_6),
    (0.831_469_6, -0.555_570_2),
    (0.980_785_3, -0.195_090_3),
];

/// Perlin-style gradient noise at offset `(fx, fy)` in `[0, 1)²` within lattice cell `(cx, cy)`.
/// Roughly in `[-1, 1]`, zero at lattice points.
fn gradient(seed: u64, cx: i64, cy: i64, fx: f64, fy: f64) -> f64 {
    let corner = |dx: i64, dy: i64| {
        let (gx, gy) = GRADIENTS[(hash_point(seed, cx.wrapping_add(dx), cy.wrapping_add(dy)) >> 60) as usize];
        gx * (fx - dx as f64) + gy * (fy - dy as f64)
    };
    let u = fade(fx);
    let v = fade(fy);
    let bottom = lerp(corner(0, 0), corner(1, 0), u);
    let top = lerp(corner(0, 1), corner(1, 1), u);
    // Unit gradients peak at sqrt(2)/2; scale toward [-1, 1].
    lerp(bottom, top, v) * std::f64::consts::SQRT_2
}

/// Quintic smoothstep, so the noise has a continuous second derivative and no creases along cell edges.
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Splits tile coordinate `x`, nudged by a sub-tile `offset`, into a lattice cell and a position in it.
fn locate(x: i64, offset: f64, wavelength: i64) -> (i64, f64) {
    let t = (x.rem_euclid(wavelength) as f64 + offset) / wavelength as f64;
    let whole = t.floor();
    (x.div_euclid(wavelength).wrapping_add(whole as i64), t - whole)
}

/// Fractal sum of `octaves` at tile `(x, y)` displaced by `(dx, dy)` tiles, as an approximate z-score:
/// mean 0, standard deviation 1, rarely beyond ±3.
///
/// Each octave gets its own seed and a hashed lattice shift. Without the shift every octave would have a
/// lattice point (and so a zero) at the origin, leaving a visible dimple there.
pub(crate) fn fbm(seed: u64, octaves: &[Octave], x: i64, y: i64, dx: f64, dy: f64) -> f64 {
    let mut sum = 0.0;
    let mut power = 0.0;
    for (i, octave) in octaves.iter().enumerate() {
        let octave_seed = combine(seed, i as u64);
        let shift = mix(octave_seed);
        let sx = x.wrapping_add((shift & 0xff_ffff) as i64);
        let sy = y.wrapping_add((shift >> 40) as i64);
        let (cx, fx) = locate(sx, dx, octave.wavelength);
        let (cy, fy) = locate(sy, dy, octave.wavelength);
        sum += octave.amplitude * gradient(octave_seed, cx, cy, fx, fy);
        power += octave.amplitude * octave.amplitude;
    }
    // Octaves are independent, so their variances add.
    sum / (power.sqrt() * GRADIENT_STD)
}

/// Maps a z-score to `[0, 1]` through an approximate normal CDF, so field values are spread evenly and a
/// threshold in the biome table reads as a share of the land: "elevation below 0.4" is about 40% of tiles.
pub(crate) fn to_unit(z: f64) -> f64 {
    // Phi(z) = (1 + tanh(0.8 z + 0.036 z^3)) / 2 to within 0.03%, with tanh replaced by its [3/2] Padé
    // approximant (monotone, exactly 1 at 3). The result is within 0.012 of the true CDF, plenty for a remap.
    let a = 0.797_884_6 * (z + 0.044_715 * z * z * z);
    let a = a.clamp(-3.0, 3.0);
    let tanh = a * (27.0 + a * a) / (27.0 + 9.0 * a * a);
    (0.5 + 0.5 * tanh).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: [Octave; 1] = [Octave { wavelength: 64, amplitude: 1.0 }];

    #[test]
    fn locate_floors_negative_coordinates() {
        assert_eq!(locate(-1, 0.0, 64), (-1, 63.0 / 64.0));
        assert_eq!(locate(-64, 0.0, 64), (-1, 0.0));
        assert_eq!(locate(63, 2.0, 64), (1, 1.0 / 64.0));
        assert_eq!(locate(0, -0.5, 64), (-1, 63.5 / 64.0));
    }

    #[test]
    fn continuous_across_cell_edges() {
        // Tiles on either side of a lattice line, including the line through zero.
        for x in [-65, -1, 63, 1_000_063] {
            let a = fbm(3, &ONE, x, 5, 0.0, 0.0);
            let b = fbm(3, &ONE, x + 1, 5, 0.0, 0.0);
            assert!((a - b).abs() < 0.2, "jump of {} at x={x}", (a - b).abs());
        }
    }

    #[test]
    fn displacement_matches_neighbor() {
        // Shifting by a whole tile must land exactly on the neighboring tile.
        let a = fbm(9, &ONE, 10, -20, 1.0, -1.0);
        let b = fbm(9, &ONE, 11, -21, 0.0, 0.0);
        assert!((a - b).abs() < 1e-12);
    }

    #[test]
    fn to_unit_is_monotone_and_bounded() {
        let mut last = 0.0;
        for i in -800..=800 {
            let u = to_unit(f64::from(i) / 100.0);
            assert!((0.0..=1.0).contains(&u));
            assert!(u >= last);
            last = u;
        }
        assert!((to_unit(0.0) - 0.5).abs() < 1e-12);
        assert!((to_unit(1.0) - 0.8413).abs() < 0.012);
    }
}

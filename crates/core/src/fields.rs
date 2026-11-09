//! The four climate fields every tile samples, and the scales that shape them.
//!
//! Wavelengths are in tiles and chosen against what the player sees: the finest octaves are about the size
//! of the 15×15 viewport and weak, so a screen reads as one place with ragged edges rather than speckle;
//! moisture and temperature turn over every few hundred tiles, so a walk crosses biomes; and elevation's
//! base octave is continent-sized.
//! Wavelengths are deliberately not powers of two, so octave lattices don't line up into a grid.

use crate::biome::SEA_LEVEL;
use crate::hash::derive;
use crate::noise::{Octave, fbm, to_unit};

const fn octave(wavelength: i64, amplitude: f64) -> Octave {
    Octave { wavelength, amplitude }
}

/// Continents and coasts: where land is, and its broad height.
const CONTINENTS: &[Octave] = &[
    octave(1_097, 1.00),
    octave(521, 0.55),
    octave(257, 0.32),
    octave(127, 0.18),
    octave(61, 0.12),
    octave(31, 0.08),
    octave(17, 0.05),
];
/// Mountain ranges, as the zero-crossing lines of this field (see [`elevation`]).
const RIDGES: &[Octave] = &[octave(353, 1.00), octave(167, 0.45), octave(83, 0.20), octave(41, 0.10)];
/// Climate octaves fall off more slowly than continents do (0.6 per octave rather than 0.5). With less
/// fine detail, biome borders are smooth arcs that read as ruled lines at walking scale.
const MOISTURE: &[Octave] =
    &[octave(613, 1.00), octave(293, 0.60), octave(139, 0.36), octave(67, 0.22), octave(31, 0.13), octave(15, 0.06)];
const TEMPERATURE: &[Octave] =
    &[octave(1_123, 1.00), octave(541, 0.60), octave(257, 0.36), octave(127, 0.22), octave(61, 0.13), octave(29, 0.07)];
const AGE: &[Octave] = &[octave(877, 1.00), octave(421, 0.50), octave(199, 0.25)];

/// Low-frequency displacement of elevation's input, which bends round noise contours into peninsulas and
/// inlets.
const WARP: &[Octave] = &[octave(503, 1.00), octave(241, 0.40)];
/// How far the warp moves a sample, in tiles per standard deviation. The warp must change by well under a
/// tile per tile, or the map folds over itself and smears into straight streaks; `warp_never_folds` checks.
const WARP_TILES: f64 = 15.0;

/// Share of land height that comes from the continent; the rest is reserved for ridges. Keeping it well
/// under 1 stops continent cores from becoming one vast plateau of peaks.
const PLAINS: f64 = 0.55;
/// Width of a range across its crest, in standard deviations of the ridge field. Smaller is narrower.
const RIDGE_WIDTH: f64 = 0.7;
/// How far inland (as a share of land height) ranges reach full height, so coasts stay low.
const RANGE_INLAND: f64 = 0.35;

/// Temperature lost between sea level and the highest peak, on the `[0, 1]` scale. It grows with the
/// square of height, because elevation isn't linear in altitude: low plains barely cool, peaks freeze.
const LAPSE: f64 = 0.55;

/// Per-world sub-seeds, one per field, so the fields don't correlate with each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Seeds {
    continents: u64,
    ridges: u64,
    warp_x: u64,
    warp_y: u64,
    moisture: u64,
    temperature: u64,
    age: u64,
}

impl Seeds {
    pub(crate) const fn new(seed: u64) -> Seeds {
        Seeds {
            continents: derive(seed, b"continen"),
            ridges: derive(seed, b"ridges__"),
            warp_x: derive(seed, b"warp_x__"),
            warp_y: derive(seed, b"warp_y__"),
            moisture: derive(seed, b"moisture"),
            temperature: derive(seed, b"temperat"),
            age: derive(seed, b"age_____"),
        }
    }
}

/// All four fields at one tile, each in `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Sample {
    pub elevation: f64,
    pub moisture: f64,
    pub temperature: f64,
    pub age: f64,
}

/// Height in `[0, 1]`, built in two parts so that coastlines and mountain ranges can each look right.
///
/// The continent field alone decides land and sea, so the water share is exactly the share of continent
/// values below [`SEA_LEVEL`]. On land, the continent supplies a low base and ridged noise raises ranges
/// along the lines where a second field crosses zero. Those lines are long and branching, which is what
/// real ranges look like, where a plain fractal's high values would be round blobs.
pub(crate) fn elevation(seeds: &Seeds, x: i64, y: i64) -> f64 {
    let dx = WARP_TILES * fbm(seeds.warp_x, WARP, x, y, 0.0, 0.0);
    let dy = WARP_TILES * fbm(seeds.warp_y, WARP, x, y, 0.0, 0.0);
    let continent = to_unit(fbm(seeds.continents, CONTINENTS, x, y, dx, dy));
    let sea = f64::from(SEA_LEVEL);
    if continent <= sea {
        return continent;
    }
    let land = (continent - sea) / (1.0 - sea);
    let ridge = 1.0 - (fbm(seeds.ridges, RIDGES, x, y, dx, dy).abs() / RIDGE_WIDTH).min(1.0);
    // Cubing the profile narrows ranges from broad swells to spines. Plain multiplication, not `powi`,
    // whose precision is unspecified.
    let rise = ridge * ridge * ridge * smoothstep(land / RANGE_INLAND);
    sea + (1.0 - sea) * (PLAINS * land + (1.0 - PLAINS) * rise)
}

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(crate) fn moisture(seeds: &Seeds, x: i64, y: i64) -> f64 {
    to_unit(fbm(seeds.moisture, MOISTURE, x, y, 0.0, 0.0))
}

/// Temperature takes elevation as an input because it falls with height above the sea. Sea surface keeps
/// the base value, so cold water and warm water both exist.
pub(crate) fn temperature(seeds: &Seeds, x: i64, y: i64, elevation: f64) -> f64 {
    let base = to_unit(fbm(seeds.temperature, TEMPERATURE, x, y, 0.0, 0.0));
    let sea = f64::from(SEA_LEVEL);
    let height = ((elevation - sea) / (1.0 - sea)).max(0.0);
    (base - LAPSE * height * height).clamp(0.0, 1.0)
}

pub(crate) fn age(seeds: &Seeds, x: i64, y: i64) -> f64 {
    to_unit(fbm(seeds.age, AGE, x, y, 0.0, 0.0))
}

pub(crate) fn sample(seeds: &Seeds, x: i64, y: i64) -> Sample {
    let elevation = elevation(seeds, x, y);
    Sample {
        elevation,
        moisture: moisture(seeds, x, y),
        temperature: temperature(seeds, x, y, elevation),
        age: age(seeds, x, y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Share of samples in each tenth of `[0, 1]`, over a wide strided area.
    fn deciles(f: impl Fn(i64, i64) -> f64) -> [f64; 10] {
        let mut counts = [0usize; 10];
        let mut n = 0;
        for y in (-20_000..20_000).step_by(97) {
            for x in (-20_000..20_000).step_by(97) {
                counts[((f(x, y) * 10.0) as usize).min(9)] += 1;
                n += 1;
            }
        }
        counts.map(|c| c as f64 / f64::from(n))
    }

    /// The biome table's moisture and temperature thresholds assume evenly spread fields; this fails if the
    /// noise's normalization (`GRADIENT_STD`) drifts. Elevation is deliberately uneven on land.
    #[test]
    fn fields_are_roughly_uniform() {
        let seeds = Seeds::new(11);
        for (name, shares) in
            [("moisture", deciles(|x, y| moisture(&seeds, x, y))), ("age", deciles(|x, y| age(&seeds, x, y)))]
        {
            for (i, share) in shares.iter().enumerate() {
                assert!((0.05..0.16).contains(share), "{name} decile {i} holds {share:.3}: {shares:.3?}");
            }
        }
    }

    #[test]
    fn warp_never_folds() {
        let seeds = Seeds::new(3);
        let mut steepest: f64 = 0.0;
        for y in (-20_000..20_000).step_by(37) {
            for x in (-20_000..20_000).step_by(37) {
                for warp in [seeds.warp_x, seeds.warp_y] {
                    let here = fbm(warp, WARP, x, y, 0.0, 0.0);
                    let right = fbm(warp, WARP, x + 1, y, 0.0, 0.0);
                    let down = fbm(warp, WARP, x, y + 1, 0.0, 0.0);
                    steepest = steepest.max((right - here).abs()).max((down - here).abs());
                }
            }
        }
        assert!(steepest * WARP_TILES < 0.5, "warp moves {:.2} tiles per tile", steepest * WARP_TILES);
    }

    #[test]
    fn temperature_falls_with_elevation() {
        let seeds = Seeds::new(5);
        let low = temperature(&seeds, 100, 100, 0.45);
        let high = temperature(&seeds, 100, 100, 0.95);
        assert!(high < low);
    }

    #[test]
    fn fields_are_uncorrelated() {
        let seeds = Seeds::new(23);
        let mut pairs = Vec::new();
        for y in (-10_000..10_000).step_by(131) {
            for x in (-10_000..10_000).step_by(131) {
                pairs.push((moisture(&seeds, x, y), age(&seeds, x, y)));
            }
        }
        let n = pairs.len() as f64;
        let (ma, mb) = pairs.iter().fold((0.0, 0.0), |(a, b), p| (a + p.0 / n, b + p.1 / n));
        let cov = pairs.iter().map(|p| (p.0 - ma) * (p.1 - mb)).sum::<f64>() / n;
        let va = pairs.iter().map(|p| (p.0 - ma).powi(2)).sum::<f64>() / n;
        let vb = pairs.iter().map(|p| (p.1 - mb).powi(2)).sum::<f64>() / n;
        let r = cov / (va * vb).sqrt();
        assert!(r.abs() < 0.15, "moisture and age correlate: r = {r:.3}");
    }
}

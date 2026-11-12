//! Presentation-only weather and stone: how the same world looks under a changing sky, or in the Deep.
//!
//! Nothing here touches the rules or the save. The sky drifts between overcast, clear, and mist with the round
//! number, and its strength varies in broad world-space patches, so mist pools and sunlight breaks through
//! unevenly. Each Deep floor has a main stone and a vein stone that surfaces in broad regions; deeper bands
//! favor basalt. Grading is per world pixel, so neighboring cells always agree.

use std::hash::{Hash, Hasher};

use isle_core::{Game, LevelId};

use crate::art::{lattice, value_noise};

/// Milliseconds per animation frame, and frames per grass-sway cycle.
pub const FRAME_MS: u64 = 180;
pub const FRAMES: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stone {
    /// Worn pale limestone, warm and dusty.
    Limestone,
    /// Wet blue slate with damp patches.
    Slate,
    /// Near-black basalt with narrow lava seams.
    Basalt,
}

/// Island-wide sky, in eighths. At most one of the two is nonzero; neither means overcast.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Sky {
    pub clear: u8,
    pub mist: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mood {
    Surface(Sky),
    /// A floor's main stone and the stone of its veins.
    Deep(Stone, Stone),
}

impl Default for Mood {
    fn default() -> Self {
        Mood::Surface(Sky::default())
    }
}

/// Launch-time overrides for previewing a look.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Override {
    pub sky: Option<Sky>,
    pub stone: Option<Stone>,
}

pub fn parse_sky(name: &str) -> Option<Sky> {
    match name {
        "overcast" => Some(Sky::default()),
        "clear" => Some(Sky { clear: 8, mist: 0 }),
        "mist" => Some(Sky { clear: 0, mist: 8 }),
        _ => None,
    }
}

pub fn parse_stone(name: &str) -> Option<Stone> {
    match name {
        "limestone" => Some(Stone::Limestone),
        "slate" => Some(Stone::Slate),
        "basalt" => Some(Stone::Basalt),
        _ => None,
    }
}

/// The sky after `round` rounds: mostly overcast, with clear spells and mists that build and fade over tens of
/// rounds.
pub fn sky(seed: u64, round: u64) -> Sky {
    let r = round as f32;
    let t = value_noise(seed, 40, r / 140.0, 0.5) + 0.45 * value_noise(seed, 41, r / 37.0, 0.5);
    let eighths = |v: f32| (v.clamp(0.0, 1.0) * 8.0).round() as u8;
    if t > 0.0 {
        Sky { clear: eighths((t - 0.15) / 0.45), mist: 0 }
    } else {
        Sky { clear: 0, mist: eighths((-t - 0.15) / 0.45) }
    }
}

/// A Deep floor's stones. Floors come in bands of three, so geology changes now and then rather than every
/// floor; deeper bands favor basalt.
pub fn stones(seed: u64, level: LevelId) -> (Stone, Stone) {
    let LevelId::Deep(town, depth) = level else { return (Stone::Limestone, Stone::Limestone) };
    let mut hasher = std::hash::DefaultHasher::new();
    town.hash(&mut hasher);
    let town = hasher.finish() as i64;
    let band = i64::from(depth.saturating_sub(1) / 3);
    let weights: [f32; 3] = match band {
        0 => [0.5, 0.35, 0.15],
        1 => [0.25, 0.45, 0.30],
        _ => [0.15, 0.35, 0.50],
    };
    let pick = |roll: f32| {
        let mut acc = 0.0;
        for (stone, w) in [Stone::Limestone, Stone::Slate, Stone::Basalt].into_iter().zip(weights) {
            acc += w;
            if roll < acc {
                return stone;
            }
        }
        Stone::Basalt
    };
    let main = pick(lattice(seed, 50, town, band));
    let vein = pick(lattice(seed, 51, town, band));
    (main, vein)
}

/// What the character's surroundings look like now.
pub fn of(game: &Game, forced: Override) -> Mood {
    match game.player().at {
        LevelId::Surface => Mood::Surface(forced.sky.unwrap_or_else(|| sky(game.seed(), game.round_number()))),
        level => match forced.stone {
            Some(stone) => Mood::Deep(stone, stone),
            None => {
                let (main, vein) = stones(game.seed(), level);
                Mood::Deep(main, vein)
            }
        },
    }
}

impl Mood {
    /// Wind strength in tenths: breezy when overcast, gentle when clear, nearly still in mist. None underground.
    pub fn wind(self) -> u8 {
        match self {
            Mood::Surface(sky) => (11 - u16::from(sky.mist) - u16::from(sky.clear) / 2).max(3) as u8,
            Mood::Deep(..) => 0,
        }
    }

    /// The stone at world pixel `p`: veins surface in broad regions with a slightly ragged edge.
    pub fn stone_at(self, seed: u64, p: (i64, i64)) -> Option<Stone> {
        let Mood::Deep(main, vein) = self else { return None };
        let region = value_noise(seed, 44, p.0 as f32 / 190.0, p.1 as f32 / 190.0);
        let ragged = (lattice(seed, 45, p.0, p.1) - 0.5) * 0.08;
        Some(if region + ragged > 0.3 { vein } else { main })
    }
}

/// Lava seams: narrow, branching cracks, only in some regions of basalt floor.
pub fn seam(seed: u64, p: (i64, i64)) -> bool {
    let (x, y) = (p.0 as f32, p.1 as f32);
    value_noise(seed, 46, x / 21.0, y / 21.0).abs() < 0.028 && value_noise(seed, 47, x / 40.0, y / 40.0) > 0.25
}

pub const EMBER: [u8; 3] = [118, 36, 22];
pub const HEAT: [u8; 3] = [255, 156, 72];

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Grade one pixel under the sky. `p` is its world pixel when known (ground), or None for objects, which take
/// the island-wide amount. Overcast is the art as drawn, and the Deep paints in its own stone instead.
pub fn grade(seed: u64, mood: Mood, p: Option<(i64, i64)>, pixel: [u8; 4]) -> [u8; 4] {
    let Mood::Surface(sky) = mood else { return pixel };
    if pixel[3] == 0 || sky == Sky::default() {
        return pixel;
    }
    let local = |channel: u64, period: f32| {
        p.map_or(0.0, |p| value_noise(seed, channel, p.0 as f32 / period, p.1 as f32 / period))
    };
    let mut c = [f32::from(pixel[0]), f32::from(pixel[1]), f32::from(pixel[2])];
    let lum = 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2];
    let clear = (f32::from(sky.clear) / 8.0 * (0.75 + 0.45 * local(42, 150.0))).clamp(0.0, 1.0);
    let mist = (f32::from(sky.mist) / 8.0 * (0.7 + 0.5 * local(43, 110.0))).clamp(0.0, 1.0);
    if clear > 0.0 {
        let warm = [c[0] * 1.06 + 5.0, c[1] * 1.03 + 3.0, c[2] * 0.93];
        let lum = 0.3 * warm[0] + 0.59 * warm[1] + 0.11 * warm[2];
        let vivid = warm.map(|v| lum + (v - lum) * 1.1);
        c = mix(c, vivid, clear);
    }
    if mist > 0.0 {
        let grey = mix(c, [lum; 3], 0.45);
        c = mix(c, mix(grey, [156.0, 171.0, 176.0], 0.3), mist);
    }
    let [r, g, b] = c.map(|v| v.round().clamp(0.0, 255.0) as u8);
    [r, g, b, pixel[3]]
}

/// A shared brightness pulse in [0, 1], `period` seconds long. Every cell uses the same phase, so a glow
/// spanning several cells never shows seams.
pub fn pulse(elapsed: f32, period: f32) -> f32 {
    0.5 + 0.5 * (elapsed / period * std::f32::consts::TAU).sin()
}

#[cfg(test)]
mod tests {
    use super::*;
    use isle_core::Town;

    #[test]
    fn the_sky_is_deterministic_mostly_overcast_and_changes_gradually() {
        let skies: Vec<Sky> = (0..20_000).map(|r| sky(7, r)).collect();
        assert_eq!(skies, (0..20_000).map(|r| sky(7, r)).collect::<Vec<_>>());
        assert!(skies.iter().all(|s| s.clear <= 8 && s.mist <= 8 && (s.clear == 0 || s.mist == 0)));
        let overcast = skies.iter().filter(|s| **s == Sky::default()).count();
        let clear = skies.iter().filter(|s| s.clear == 8).count();
        let mist = skies.iter().filter(|s| s.mist == 8).count();
        assert!(overcast > 5_000 && clear > 300 && mist > 300, "{overcast} {clear} {mist}");
        // No jumps: neighboring rounds differ by at most one eighth on each side.
        for pair in skies.windows(2) {
            let step = |s: Sky| i16::from(s.clear) - i16::from(s.mist);
            assert!((step(pair[0]) - step(pair[1])).abs() <= 1, "{pair:?}");
        }
        assert_ne!((0..2000).map(|r| sky(7, r)).collect::<Vec<_>>(), (0..2000).map(|r| sky(8, r)).collect::<Vec<_>>());
    }

    #[test]
    fn stone_comes_in_bands_and_every_kind_occurs() {
        let town = |variant: u64| -> Town {
            serde_json::from_value(serde_json::json!({ "origin": [0, 0], "variant": variant })).unwrap()
        };
        let band = |d: u32| stones(7, LevelId::Deep(town(3), d));
        assert_eq!(band(1), band(3));
        assert_eq!(band(4), band(6));
        let mut seen = std::collections::HashSet::new();
        for variant in 0..60 {
            for depth in [1, 4, 7, 10] {
                let (a, b) = stones(7, LevelId::Deep(town(variant), depth));
                seen.insert(a);
                seen.insert(b);
            }
        }
        assert_eq!(seen.len(), 3);
        let mood = Mood::Deep(Stone::Limestone, Stone::Basalt);
        let veined = (-40..40).flat_map(|y| (-40..40).map(move |x| (x * 16, y * 16)));
        let basalt = veined.filter(|&p| mood.stone_at(7, p) == Some(Stone::Basalt)).count();
        assert!(basalt > 100 && basalt < 3_200, "veins are regions, not the whole floor: {basalt}");
    }

    #[test]
    fn overcast_is_the_art_as_drawn_and_other_moods_shift_it_without_touching_alpha() {
        let px = [90, 130, 70, 255];
        assert_eq!(grade(7, Mood::default(), Some((3, 4)), px), px);
        assert_eq!(grade(7, Mood::Surface(Sky { clear: 8, mist: 0 }), Some((3, 4)), [1, 2, 3, 0]), [1, 2, 3, 0]);
        let clear = grade(7, Mood::Surface(Sky { clear: 8, mist: 0 }), Some((3, 4)), px);
        let mist = grade(7, Mood::Surface(Sky { clear: 0, mist: 8 }), Some((3, 4)), px);
        assert!(clear[0] > px[0] && clear[2] < px[2], "clear skies warm: {clear:?}");
        assert!((i16::from(mist[0]) - i16::from(mist[1])).abs() < 40, "mist greys: {mist:?}");
        // Mist strength varies across the land.
        let strengths: std::collections::HashSet<_> =
            (0..40).map(|i| grade(7, Mood::Surface(Sky { clear: 0, mist: 4 }), Some((i * 97, i * 41)), px)).collect();
        assert!(strengths.len() > 10);
    }

    #[test]
    fn lava_seams_are_narrow_and_the_deep_is_not_graded_by_the_sky() {
        let points = (0..400).flat_map(|y| (0..400).map(move |x| (x, y))).filter(|&p| seam(7, p)).count();
        assert!(points > 200 && points < 400 * 400 / 10, "{points}");
        let px = [120, 120, 120, 255];
        assert_eq!(grade(7, Mood::Deep(Stone::Basalt, Stone::Slate), Some((3, 4)), px), px);
    }
}

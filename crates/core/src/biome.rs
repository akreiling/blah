//! The biome table: the one place that decides what a patch of climate looks like.
//!
//! Every biome is one row with its look, its game properties, the climate envelope it claims, and the
//! creature it rewards. Retuning or adding a biome is an edit to [`BIOMES`] and nothing else; code refers
//! to biomes by their properties (`passable`, `water`) rather than by name.

/// An inclusive range of a field value in `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// Lowest value in the band.
    pub min: f32,
    /// Highest value in the band.
    pub max: f32,
}

impl Band {
    /// The whole range, for a field the biome doesn't care about.
    pub const ANY: Band = Band { min: 0.0, max: 1.0 };

    const fn new(min: f32, max: f32) -> Band {
        Band { min, max }
    }

    /// Whether `v` lies in the band, ends included.
    #[must_use]
    pub fn contains(self, v: f32) -> bool {
        self.min <= v && v <= self.max
    }
}

/// One row of the biome table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Biome {
    /// Display name.
    pub name: &'static str,
    /// Glyph for text renderings. Unique across the table.
    pub glyph: char,
    /// Display color, `[r, g, b]`.
    pub color: [u8; 3],
    /// Whether the wanderer can walk here. A stand-in until movement comes from Survival and genes.
    pub passable: bool,
    /// Whether this is open water, for coastline and swimmer logic.
    pub water: bool,
    /// Placeholder for the food web: plant biomass relative to the richest biome. Not tuned yet.
    pub productivity: f32,
    /// Elevation this biome claims.
    pub elevation: Band,
    /// Moisture this biome claims.
    pub moisture: Band,
    /// Temperature this biome claims.
    pub temperature: Band,
    /// What living here rewards; see [`Fitness`].
    pub fitness: Fitness,
}

/// What a biome rewards in a creature: the fitness function, as data.
///
/// Limbs are a pressure, one way or the other. Everything else is a target the creature is scored against
/// by squared distance, so tolerating heat that never comes costs as much as lacking it. The weights that
/// turn these into one score live with the scoring, in `life.rs`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fitness {
    /// Preferred heat tolerance: how hot it gets.
    pub heat: f32,
    /// Preferred cold tolerance: how cold it gets.
    pub cold: f32,
    /// Preferred water affinity.
    pub wet: f32,
    /// Preferred covering insulation (fur 1.0 down to slime 0.0), give or take a slack, which is how heavy
    /// coats are punished in heat and bare skin in cold.
    pub insulation: f32,
    /// Preferred size: big in the cold (Bergmann's rule), small where food is scarce.
    pub size: f32,
    /// Pressure on the limbs gene, `[-1, 1]`: negative pays fewer legs (water), positive more (rough
    /// ground), zero leaves leg count to drift.
    pub limbs: f32,
    /// Preferred aggression: high where food is scarce, low on open plains where prey survives by running.
    pub aggression: f32,
}
impl Biome {
    /// Whether this biome's envelope contains the given climate.
    #[must_use]
    pub fn claims(&self, elevation: f32, moisture: f32, temperature: f32) -> bool {
        self.elevation.contains(elevation) && self.moisture.contains(moisture) && self.temperature.contains(temperature)
    }
}

/// Index of a row in [`BIOMES`].
///
/// Only meaningful for the table it came from. Terrain is never saved, so ids needn't survive table edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BiomeId(pub u8);

impl BiomeId {
    /// The table row for this id.
    #[must_use]
    pub fn biome(self) -> &'static Biome {
        &BIOMES[self.0 as usize]
    }

    /// Every id in the table, in table order.
    pub fn all() -> impl Iterator<Item = BiomeId> {
        (0..BIOMES.len()).map(|i| BiomeId(i as u8))
    }
}

/// Picks the biome for a climate: the first row, in table order, whose envelope contains it.
///
/// Order is the tie-breaker, so specific rows (beaches, peaks, swamps) sit above the broad ones they carve
/// out of, and the last row claims everything so no climate is left without a biome.
#[must_use]
pub fn classify(elevation: f32, moisture: f32, temperature: f32) -> BiomeId {
    let index = BIOMES.iter().position(|b| b.claims(elevation, moisture, temperature)).unwrap_or(BIOMES.len() - 1);
    BiomeId(index as u8)
}

/// Sea level on the elevation scale. Elevation is spread evenly over `[0, 1]`, so this is also roughly the
/// share of the world under water.
pub const SEA_LEVEL: f32 = 0.40;

#[expect(clippy::too_many_arguments, reason = "one argument per table column")]
const fn row(
    name: &'static str,
    glyph: char,
    color: [u8; 3],
    passable: bool,
    water: bool,
    productivity: f32,
    bands: [Band; 3],
    fitness: Fitness,
) -> Biome {
    let [elevation, moisture, temperature] = bands;
    Biome { name, glyph, color, passable, water, productivity, elevation, moisture, temperature, fitness }
}

const fn fit(heat: f32, cold: f32, wet: f32, insulation: f32, size: f32, limbs: f32, aggression: f32) -> Fitness {
    Fitness { heat, cold, wet, insulation, size, limbs, aggression }
}

const ANY: Band = Band::ANY;
const SEA: f32 = SEA_LEVEL;
const fn b(min: f32, max: f32) -> Band {
    Band::new(min, max)
}

/// The biome table, Whittaker-style: elevation sorts water, coast, lowland, and highland, then moisture
/// and temperature pick the lowland biome. First match wins; see [`classify`].
///
/// Elevation is spread evenly below sea level but bunched on land (see `fields::elevation`), so land
/// thresholds were tuned by eye with the `ascii_map` example. Cold coasts are tundra rather than beach
/// because Tundra sits above Beach.
///
/// Each row's second line is its [`Fitness`], tuned by eye with the `bestiary_sheet` example.
#[rustfmt::skip]
pub const BIOMES: &[Biome] = &[
    //  name             glyph  color             pass   water  prod  elevation        moisture         temperature
    //  fit( heat,  cold,   wet, insul,  size, limbs,  aggr)
    row("Deep water",    '~', [ 22,  54, 112], false, true,  0.20, [b(0.000, 0.300), ANY,             ANY            ],
        fit( 0.10,  0.50,  1.00,  0.05,  0.75, -1.00,  0.70)),
    row("Shallow water", '-', [ 48, 104, 170], true,  true,  0.50, [b(0.300, SEA),   ANY,             ANY            ],
        fit( 0.30,  0.20,  0.85,  0.10,  0.45, -0.70,  0.40)),
    row("Snow peaks",    'A', [236, 240, 246], false, false, 0.02, [b(0.860, 1.000), ANY,             b(0.000, 0.320)],
        fit( 0.00,  1.00,  0.15,  1.00,  0.70,  0.40,  0.60)),
    row("Mountains",     '^', [128, 118, 110], true,  false, 0.15, [b(0.790, 1.000), ANY,             ANY            ],
        fit( 0.10,  0.70,  0.20,  0.70,  0.55,  0.60,  0.60)),
    row("Badlands",      '#', [168,  84,  52], true,  false, 0.08, [b(0.700, 1.000), b(0.000, 0.250), b(0.600, 1.000)],
        fit( 0.90,  0.15,  0.05,  0.25,  0.35,  0.50,  0.80)),
    row("Hills",         'n', [118, 132,  82], true,  false, 0.40, [b(0.720, 1.000), ANY,             ANY            ],
        fit( 0.20,  0.40,  0.25,  0.65,  0.50,  0.30,  0.45)),
    row("Tundra",        '_', [168, 178, 168], true,  false, 0.15, [ANY,             ANY,             b(0.000, 0.130)],
        fit( 0.00,  1.00,  0.25,  1.00,  0.80,  0.00,  0.55)),
    row("Beach",         '.', [226, 210, 150], true,  false, 0.20, [b(SEA,   0.407), ANY,             ANY            ],
        fit( 0.50,  0.10,  0.55,  0.25,  0.35,  0.00,  0.30)),
    row("Taiga",         't', [ 52,  98,  82], true,  false, 0.40, [ANY,             b(0.400, 1.000), b(0.000, 0.380)],
        fit( 0.05,  0.80,  0.35,  0.90,  0.65,  0.00,  0.50)),
    row("Swamp",         ',', [ 74,  96,  60], true,  false, 0.80, [b(SEA,   0.520), b(0.700, 1.000), b(0.380, 1.000)],
        fit( 0.50,  0.10,  0.75,  0.00,  0.50, -0.40,  0.65)),
    row("Desert",        ':', [222, 196, 120], true,  false, 0.10, [ANY,             b(0.000, 0.280), b(0.450, 1.000)],
        fit( 1.00,  0.10,  0.00,  0.30,  0.25,  0.30,  0.75)),
    row("Savanna",       ';', [184, 170,  86], true,  false, 0.50, [ANY,             b(0.000, 0.520), b(0.640, 1.000)],
        fit( 0.75,  0.05,  0.15,  0.20,  0.75,  0.20,  0.45)),
    row("Jungle",        '%', [ 32, 120,  48], true,  false, 1.00, [ANY,             ANY,             b(0.640, 1.000)],
        fit( 0.70,  0.00,  0.50,  0.10,  0.60,  0.00,  0.60)),
    row("Forest",        '&', [ 46, 110,  58], true,  false, 0.70, [ANY,             b(0.480, 1.000), ANY            ],
        fit( 0.20,  0.30,  0.40,  0.60,  0.50,  0.00,  0.45)),
    row("Grassland",     '"', [118, 168,  78], true,  false, 0.60, [ANY,             ANY,             ANY            ],
        fit( 0.30,  0.25,  0.30,  0.55,  0.55,  0.00,  0.20)),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn glyphs_and_names_are_unique() {
        let glyphs: HashSet<char> = BIOMES.iter().map(|b| b.glyph).collect();
        let names: HashSet<&str> = BIOMES.iter().map(|b| b.name).collect();
        assert_eq!(glyphs.len(), BIOMES.len());
        assert_eq!(names.len(), BIOMES.len());
    }

    #[test]
    fn last_row_claims_everything() {
        let last = BIOMES.last().unwrap();
        assert_eq!((last.elevation, last.moisture, last.temperature), (ANY, ANY, ANY));
    }

    #[test]
    fn water_is_exactly_below_sea_level() {
        for i in 0..=1000 {
            let e = i as f32 / 1000.0;
            for (m, t) in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0), (0.1, 0.9)] {
                let water = classify(e, m, t).biome().water;
                assert_eq!(water, e <= SEA_LEVEL, "e={e} m={m} t={t}");
            }
        }
    }

    #[test]
    fn every_row_is_reachable() {
        // Search each row's own envelope for a climate it wins, so thin bands like beaches are found too.
        let steps = 24;
        let points = |band: Band| (0..=steps).map(move |i| band.min + (band.max - band.min) * i as f32 / steps as f32);
        for id in BiomeId::all() {
            let row = id.biome();
            let wins = points(row.elevation)
                .any(|e| points(row.moisture).any(|m| points(row.temperature).any(|t| classify(e, m, t) == id)));
            assert!(wins, "{} is shadowed by rows above it", row.name);
        }
    }
}

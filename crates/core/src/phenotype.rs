//! Phenotype: what a genome amounts to. Stats, temperament, where it can go, and a description in words.
//!
//! [`Phenotype::of`] is a pure function of the genome. Every threshold and formula lives here, so retuning
//! how genes show is an edit to this file alone.

use crate::biome::Biome;
use crate::genome::{Gene, Genome};

/// What a creature is covered in. The variants are in gene order, from insulated and dry to bare and wet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Covering {
    /// Fur: the warmest covering.
    Fur,
    /// Feathers: warm and light.
    Feather,
    /// Chitin: a hard shell, the best armor.
    Chitin,
    /// Scales: armored and good at keeping water in.
    Scale,
    /// Slime: bare, wet skin.
    Slime,
}

/// One row of [`COVERINGS`]: what a covering does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoveringTraits {
    /// The covering.
    pub covering: Covering,
    /// Adjective for descriptions.
    pub word: &'static str,
    /// Warmth, `[0, 1]`. Biomes prefer an amount of it; see [`Fitness::insulation`](crate::Fitness).
    pub insulation: f32,
    /// Defense it adds.
    pub armor: i32,
    /// How wet a biome must be (its preferred water affinity) for this covering not to dry out.
    pub needs_wet: f32,
}

/// What each covering does, in gene order.
#[rustfmt::skip]
pub const COVERINGS: [CoveringTraits; 5] = [
    CoveringTraits { covering: Covering::Fur,     word: "furred",    insulation: 1.00, armor: 1, needs_wet: 0.0 },
    CoveringTraits { covering: Covering::Feather, word: "feathered", insulation: 0.75, armor: 0, needs_wet: 0.0 },
    CoveringTraits { covering: Covering::Chitin,  word: "chitinous", insulation: 0.35, armor: 2, needs_wet: 0.0 },
    CoveringTraits { covering: Covering::Scale,   word: "scaled",    insulation: 0.20, armor: 1, needs_wet: 0.0 },
    CoveringTraits { covering: Covering::Slime,   word: "slimy",     insulation: 0.00, armor: 0, needs_wet: 0.4 },
];

impl Covering {
    /// What this covering does.
    #[must_use]
    pub const fn traits(self) -> &'static CoveringTraits {
        &COVERINGS[self as usize]
    }
}

/// How a creature reacts to the wanderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Temperament {
    /// Approaches whatever it senses and attacks.
    Aggressive,
    /// Attacks what comes near its home, but doesn't chase far.
    Territorial,
    /// Flees what it senses, and fights only when cornered.
    Skittish,
}

impl Temperament {
    /// The word for descriptions.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Temperament::Aggressive => "aggressive",
            Temperament::Territorial => "territorial",
            Temperament::Skittish => "skittish",
        }
    }
}

/// Which kinds of terrain a creature can enter. Impassable land (snow peaks) is closed to everything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Access {
    /// Passable dry land.
    pub land: bool,
    /// Shallow water.
    pub shallow: bool,
    /// Deep water.
    pub deep: bool,
}

impl Access {
    /// Whether a creature with this access can stand on `biome`.
    #[must_use]
    pub const fn allows(self, biome: &Biome) -> bool {
        match (biome.water, biome.passable) {
            (false, true) => self.land,
            (false, false) => false,
            (true, true) => self.shallow,
            (true, false) => self.deep,
        }
    }
}

/// Combat numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stats {
    /// Hit points when unhurt.
    pub hp: i32,
    /// Added to damage dealt.
    pub attack: i32,
    /// Taken off damage received.
    pub defense: i32,
    /// Actions earned per turn, in tenths: 10 acts once a turn, as the wanderer does; 15 acts three times in
    /// two turns.
    pub speed: i32,
}

/// Water affinity at which a creature can wade into shallow water.
pub const WADE_AT: f32 = 0.5;
/// Water affinity at which a creature can swim in deep water.
pub const SWIM_AT: f32 = 0.78;
/// Water affinity above which a creature can no longer leave water.
pub const AQUATIC_AT: f32 = 0.9;
/// Tolerance above which a description calls a creature heat- or cold-hardy.
pub const HARDY_AT: f32 = 0.6;
/// Aggression at or above which a creature is aggressive.
pub const AGGRESSIVE_AT: f32 = 0.62;
/// Aggression below which a creature is skittish. Between the two, it is territorial.
pub const SKITTISH_BELOW: f32 = 0.38;

/// What a genome amounts to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Phenotype {
    /// The genome this was read from.
    pub genome: Genome,
    /// Size gene, `[0, 1]`, kept for drawing.
    pub size: f32,
    /// Legs: 0, 2, 4, 6, or 8.
    pub legs: u8,
    /// Covering.
    pub covering: Covering,
    /// Heat tolerance, `[0, 1]`.
    pub heat: f32,
    /// Cold tolerance, `[0, 1]`.
    pub cold: f32,
    /// Water affinity, `[0, 1]`.
    pub water: f32,
    /// Hue in `[0, 1)`, around the color wheel from red.
    pub hue: f32,
    /// Combat numbers.
    pub stats: Stats,
    /// How it reacts to the wanderer.
    pub temperament: Temperament,
    /// How far it notices the wanderer, in tiles (Chebyshev distance).
    pub sense: i64,
    /// Where it can go.
    pub access: Access,
}

impl Phenotype {
    /// Reads a genome.
    #[must_use]
    pub fn of(genome: &Genome) -> Phenotype {
        let size = genome.get(Gene::Size);
        let legs = band(genome.get(Gene::Limbs), 5) as u8 * 2;
        let covering = COVERINGS[band(genome.get(Gene::Covering), COVERINGS.len())].covering;
        let water = genome.get(Gene::Water);
        let aggression = genome.get(Gene::Aggression);

        let temperament = if aggression >= AGGRESSIVE_AT {
            Temperament::Aggressive
        } else if aggression < SKITTISH_BELOW {
            Temperament::Skittish
        } else {
            Temperament::Territorial
        };
        // Big creatures hit harder and last longer; small, many-legged ones are quick and alert.
        let stats = Stats {
            hp: (4.0 + 14.0 * size).round() as i32,
            attack: (1.0 + 3.0 * size + 2.0 * aggression).round() as i32,
            defense: covering.traits().armor + i32::from(size > 0.7),
            speed: (6.0 + 4.0 * (1.0 - size) + 3.0 * f32::from(legs.min(6)) / 6.0).round() as i32
                + i32::from(covering == Covering::Feather),
        };
        let sense = (3.0 + 3.0 * aggression + 2.0 * (1.0 - size)).round() as i64;
        let access = Access { land: water <= AQUATIC_AT, shallow: water >= WADE_AT, deep: water >= SWIM_AT };

        Phenotype {
            genome: *genome,
            size,
            legs,
            covering,
            heat: genome.get(Gene::Heat),
            cold: genome.get(Gene::Cold),
            water,
            hue: genome.get(Gene::Hue),
            stats,
            temperament,
            sense,
            access,
        }
    }

    /// The size in a word.
    #[must_use]
    pub fn size_word(&self) -> &'static str {
        ["tiny", "small", "mid-sized", "large", "huge"][band(self.size, 5)]
    }

    /// The legs in a word: "legless", "four-legged".
    #[must_use]
    pub fn legs_word(&self) -> &'static str {
        ["legless", "two-legged", "four-legged", "six-legged", "eight-legged"][usize::from(self.legs / 2)]
    }

    /// A short description, such as "small six-legged chitinous creature, heat-hardy, aggressive". The
    /// body is called a "creature"; [`Phenotype::describe_as`] names it.
    #[must_use]
    pub fn describe(&self) -> String {
        self.describe_as("creature")
    }

    /// [`Phenotype::describe`] with `noun` for the body ("crawler", "serpent").
    #[must_use]
    pub fn describe_as(&self, noun: &str) -> String {
        let mut words =
            vec![format!("{} {} {} {noun}", self.size_word(), self.legs_word(), self.covering.traits().word)];
        if self.heat >= HARDY_AT {
            words.push("heat-hardy".into());
        }
        if self.cold >= HARDY_AT {
            words.push("cold-hardy".into());
        }
        let water = if !self.access.land {
            Some("aquatic")
        } else if self.access.deep {
            Some("amphibious")
        } else if self.access.shallow {
            Some("wading")
        } else {
            None
        };
        words.extend(water.map(String::from));
        words.push(self.temperament.word().into());
        words.join(", ")
    }
}

/// Which of `n` equal bands of `[0, 1]` a value falls in. 1.0 is in the last band.
fn band(v: f32, n: usize) -> usize {
    ((v * n as f32) as usize).min(n - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::BIOMES;

    fn genome(size: f32, limbs: f32, covering: f32, water: f32, aggression: f32) -> Genome {
        Genome::new([size, limbs, covering, 0.8, 0.1, water, aggression, 0.3])
    }

    #[test]
    fn same_genome_same_phenotype() {
        let g = genome(0.3, 0.7, 0.5, 0.2, 0.8);
        assert_eq!(Phenotype::of(&g), Phenotype::of(&g));
    }

    #[test]
    fn bands_and_words() {
        let p = Phenotype::of(&genome(0.3, 0.7, 0.5, 0.2, 0.8));
        assert_eq!((p.legs, p.covering, p.temperament), (6, Covering::Chitin, Temperament::Aggressive));
        assert_eq!(p.describe(), "small six-legged chitinous creature, heat-hardy, aggressive");
        let q = Phenotype::of(&genome(1.0, 0.0, 1.0, 0.95, 0.1));
        assert_eq!((q.legs, q.covering, q.temperament), (0, Covering::Slime, Temperament::Skittish));
        assert_eq!(q.describe_as("blob"), "huge legless slimy blob, heat-hardy, aquatic, skittish");
    }

    #[test]
    fn bigger_is_tougher_and_slower() {
        let small = Phenotype::of(&genome(0.1, 0.5, 0.1, 0.2, 0.5)).stats;
        let big = Phenotype::of(&genome(0.9, 0.5, 0.1, 0.2, 0.5)).stats;
        assert!(big.hp > small.hp && big.attack > small.attack && big.speed < small.speed);
    }

    #[test]
    fn water_affinity_opens_water_and_closes_land() {
        let biome = |name: &str| BIOMES.iter().find(|b| b.name == name).unwrap();
        let (land, shallow, deep) = (biome("Grassland"), biome("Shallow water"), biome("Deep water"));
        let access = |w| Phenotype::of(&genome(0.5, 0.5, 0.5, w, 0.5)).access;
        assert!(access(0.1).allows(land) && !access(0.1).allows(shallow));
        assert!(access(0.6).allows(shallow) && !access(0.6).allows(deep));
        assert!(access(0.85).allows(land) && access(0.85).allows(deep));
        assert!(!access(0.95).allows(land) && access(0.95).allows(deep));
        assert!(!access(0.5).allows(biome("Snow peaks")));
    }
}

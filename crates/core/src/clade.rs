//! Clades: the handful of ancestral lineages a world's life descends from.
//!
//! A clade is a body plan and a base genome. The body plan is the part that never evolves: the silhouette
//! family, its proportions, its markings, its eye color. Genes stretch and recolor that silhouette from
//! region to region, but they can't swap it, so a desert crawler and a fen crawler still read as cousins.
//! The base genome is where every region's pool of the clade starts from.
//!
//! Clades are a pure function of the world seed.

use crate::genome::{GENES, Gene, Genome};
use crate::hash::Rng;
use crate::names;

/// Clades in every world: one per [`Family`].
pub const CLADES: usize = Family::ALL.len();

/// A silhouette family. Each draws differently (see [`sprite`](crate::Sprite)) and starts from its own
/// typical genome, so families differ in body before any evolution happens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Family {
    /// Beetle-like, seen from above: segmented shell, jointed legs, antennae.
    Crawler,
    /// A rearing, hooded snake on its coils.
    Serpent,
    /// A sturdy mammal-like beast facing you, with ears or horns.
    Beast,
    /// Something with spread wings: a bird or a bat.
    Flier,
    /// A domed jelly with eyestalks and tentacles.
    Blob,
    /// A wide shell with pincers and many legs, seen from above.
    Crab,
    /// A squat, wide-mouthed hopper with bulging eyes.
    Toad,
    /// A flat, diamond-finned glider with a whip tail, seen from above.
    Ray,
}

impl Family {
    /// Every family, in clade order.
    pub const ALL: [Family; 8] = [
        Family::Crawler,
        Family::Serpent,
        Family::Beast,
        Family::Flier,
        Family::Blob,
        Family::Crab,
        Family::Toad,
        Family::Ray,
    ];

    /// What to call the body in descriptions.
    #[must_use]
    pub const fn noun(self) -> &'static str {
        match self {
            Family::Crawler => "crawler",
            Family::Serpent => "serpent",
            Family::Beast => "beast",
            Family::Flier => "flier",
            Family::Blob => "blob",
            Family::Crab => "crab",
            Family::Toad => "toad",
            Family::Ray => "ray",
        }
    }

    /// The verb for its attack, third person: "The Fen Skitter stings you for 2."
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Family::Crawler => "stings",
            Family::Serpent => "bites",
            Family::Beast => "mauls",
            Family::Flier => "rakes",
            Family::Blob => "lashes",
            Family::Crab => "pinches",
            Family::Toad => "bites",
            Family::Ray => "stings",
        }
    }

    /// The genome the family's clade starts near, before the seed's jitter. Hue is left to the seed.
    #[rustfmt::skip]
    const fn typical(self) -> [f32; GENES] {
        //                  size  limbs cover heat  cold  water aggr  hue
        match self {
            Family::Crawler => [0.30, 0.72, 0.50, 0.50, 0.30, 0.30, 0.60, 0.0],
            Family::Serpent => [0.50, 0.05, 0.70, 0.60, 0.30, 0.40, 0.55, 0.0],
            Family::Beast =>   [0.65, 0.50, 0.10, 0.30, 0.60, 0.25, 0.50, 0.0],
            Family::Flier =>   [0.30, 0.30, 0.30, 0.40, 0.50, 0.30, 0.40, 0.0],
            Family::Blob =>    [0.45, 0.05, 0.90, 0.40, 0.30, 0.65, 0.45, 0.0],
            Family::Crab =>    [0.45, 0.95, 0.50, 0.40, 0.30, 0.70, 0.55, 0.0],
            Family::Toad =>    [0.40, 0.50, 0.85, 0.50, 0.30, 0.65, 0.40, 0.0],
            Family::Ray =>     [0.55, 0.05, 0.70, 0.40, 0.40, 0.88, 0.50, 0.0],
        }
    }
}

/// A clade's markings, drawn in its accent color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pattern {
    /// No markings.
    Plain,
    /// Bands across the body.
    Stripes,
    /// Scattered spots.
    Spots,
    /// One stripe down the back.
    Dorsal,
}

/// The fixed half of a clade: everything about its look that genes don't touch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyPlan {
    /// The silhouette family.
    pub family: Family,
    /// Body width, as a factor around 1.
    pub width: f32,
    /// Body length, as a factor around 1.
    pub length: f32,
    /// Head size, as a factor around 1.
    pub head: f32,
    /// One family-specific feature in `[0, 1]`: horns rather than ears, hood size, wingspan, pincer size,
    /// eyestalk length.
    pub feature: f32,
    /// Markings.
    pub pattern: Pattern,
    /// Hue of the markings, `[0, 1)`.
    pub accent: f32,
    /// Eye color.
    pub eye: [u8; 3],
}

/// Index of a clade in [`clades`]' list, which is also its [`Family`]'s index in [`Family::ALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CladeId(pub u8);

/// One ancestral lineage.
#[derive(Clone, Debug, PartialEq)]
pub struct Clade {
    /// Its index.
    pub id: CladeId,
    /// The root of every species name in it: "Skitter" in "Dune Skitter".
    pub root: &'static str,
    /// What it looks like, whatever its genes.
    pub plan: BodyPlan,
    /// Where its pools start.
    pub base: Genome,
}

/// Eye colors a clade can have.
const EYES: [[u8; 3]; 6] =
    [[255, 220, 60], [235, 60, 40], [245, 245, 235], [130, 235, 90], [255, 140, 30], [90, 220, 235]];

/// The clades of the world for `seed`, one per family, in [`Family::ALL`] order.
#[must_use]
pub fn clades(seed: u64) -> Vec<Clade> {
    Family::ALL
        .iter()
        .enumerate()
        .map(|(i, &family)| {
            let mut rng = Rng::keyed(seed, b"cladesss", &[i as u64]);
            let mut jitter = |amount: f32| 1.0 + (rng.unit() * 2.0 - 1.0) * amount;
            let (width, length, head) = (jitter(0.12), jitter(0.1), jitter(0.15));
            let feature = rng.unit();
            let pattern = [Pattern::Plain, Pattern::Stripes, Pattern::Spots, Pattern::Dorsal][rng.below(4)];
            let accent = rng.unit();
            let eye = EYES[rng.below(EYES.len())];
            let mut genes = family.typical();
            for gene in Gene::ALL {
                genes[gene as usize] += (rng.unit() * 2.0 - 1.0) * 0.08;
            }
            genes[Gene::Hue as usize] = rng.unit();
            Clade {
                id: CladeId(i as u8),
                root: names::clade_root(seed, family),
                plan: BodyPlan { family, width, length, head, feature, pattern, accent, eye },
                base: Genome::new(genes),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clades_follow_the_seed() {
        assert_eq!(clades(5), clades(5));
        assert_ne!(clades(5), clades(6));
        let c = clades(5);
        assert_eq!(c.len(), CLADES);
        for (i, clade) in c.iter().enumerate() {
            assert_eq!((clade.id.0 as usize, clade.plan.family), (i, Family::ALL[i]));
        }
        let roots: std::collections::HashSet<_> = c.iter().map(|c| c.root).collect();
        assert_eq!(roots.len(), CLADES, "root names must be unique within a world");
    }
}

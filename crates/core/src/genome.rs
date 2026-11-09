//! Genomes: what a creature inherits, as a short vector of genes in `[0, 1]`, and the operators that make
//! new ones.
//!
//! A genome means nothing on its own; [`Phenotype::of`](crate::Phenotype::of) reads it. The operators here
//! are pure given their [`Rng`] stream, so a pool evolved from the same key always comes out the same.

use crate::hash::Rng;

/// Genes in a genome.
pub const GENES: usize = 8;

/// One gene: an index into a [`Genome`], with a name for display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Gene {
    /// Body size: how much of its tile a creature fills, and with it hit points and attack.
    Size,
    /// Limb count, in bands of 0, 2, 4, 6, and 8 legs.
    Limbs,
    /// Covering, in bands: fur, feather, chitin, scale, slime. The bands run from insulated and dry to bare
    /// and wet, so a small step is a plausible change.
    Covering,
    /// Heat tolerance.
    Heat,
    /// Cold tolerance.
    Cold,
    /// Water affinity: high values wade, then swim, then can't leave water.
    Water,
    /// Aggression: skittish, territorial, aggressive.
    Aggression,
    /// Body hue, around the color wheel. The one gene that wraps: 0 and 1 are the same red.
    Hue,
}

impl Gene {
    /// Every gene, in genome order.
    pub const ALL: [Gene; GENES] =
        [Gene::Size, Gene::Limbs, Gene::Covering, Gene::Heat, Gene::Cold, Gene::Water, Gene::Aggression, Gene::Hue];

    /// A short lowercase name, for trait lists.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Gene::Size => "size",
            Gene::Limbs => "limbs",
            Gene::Covering => "covering",
            Gene::Heat => "heat tolerance",
            Gene::Cold => "cold tolerance",
            Gene::Water => "water affinity",
            Gene::Aggression => "aggression",
            Gene::Hue => "hue",
        }
    }

    /// Whether the gene is circular, so values wrap around `[0, 1)` instead of clamping.
    #[must_use]
    pub const fn wraps(self) -> bool {
        matches!(self, Gene::Hue)
    }

    /// How large this gene's mutation steps are relative to the others. The genes that draw a lineage's
    /// look move slowly, so it keeps its colors, legs, and covering where nothing selects on them and stays
    /// recognizable from region to region.
    #[must_use]
    pub const fn step(self) -> f32 {
        match self {
            Gene::Hue => 0.35,
            Gene::Limbs => 0.5,
            Gene::Covering => 0.6,
            _ => 1.0,
        }
    }
}

/// A creature's genes, each in `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Genome([f32; GENES]);

impl Genome {
    /// A genome from raw values, in [`Gene::ALL`] order. Values are clamped into `[0, 1]`, or wrapped for
    /// [`Gene::Hue`].
    #[must_use]
    pub fn new(genes: [f32; GENES]) -> Genome {
        let mut g = Genome(genes);
        for gene in Gene::ALL {
            g.set(gene, genes[gene as usize]);
        }
        g
    }

    /// One gene's value.
    #[must_use]
    pub const fn get(&self, gene: Gene) -> f32 {
        self.0[gene as usize]
    }

    /// Sets one gene, clamped or wrapped into range.
    pub fn set(&mut self, gene: Gene, value: f32) {
        let value = if value.is_nan() { 0.5 } else { value };
        self.0[gene as usize] = if gene.wraps() { value.rem_euclid(1.0) } else { value.clamp(0.0, 1.0) };
    }

    /// The raw values, in [`Gene::ALL`] order.
    #[must_use]
    pub const fn genes(&self) -> [f32; GENES] {
        self.0
    }

    /// A copy where each gene, with probability `rate`, moves by a normal step of standard deviation `sigma`
    /// times the gene's [`Gene::step`].
    #[must_use]
    pub fn mutate(&self, rng: &mut Rng, rate: f32, sigma: f32) -> Genome {
        let mut child = *self;
        for gene in Gene::ALL {
            if rng.chance(rate) {
                child.set(gene, self.get(gene) + rng.normal() * sigma * gene.step());
            }
        }
        child
    }

    /// A child taking each gene from one parent or the other, evenly. Genes are taken whole, not blended,
    /// so a fur parent and a slime parent never average out to a scaly child.
    #[must_use]
    pub fn recombine(&self, other: &Genome, rng: &mut Rng) -> Genome {
        let bits = rng.next_u64();
        let mut child = *self;
        for gene in Gene::ALL {
            if bits >> gene as u32 & 1 == 1 {
                child.0[gene as usize] = other.0[gene as usize];
            }
        }
        child
    }

    /// How far apart two genomes are: the root mean square of their per-gene differences, with hue measured
    /// the short way around. In `[0, 1]`.
    #[must_use]
    pub fn distance(&self, other: &Genome) -> f32 {
        let sum: f32 = Gene::ALL.iter().map(|&g| gene_delta(g, self.get(g), other.get(g)).powi(2)).sum();
        (sum / GENES as f32).sqrt()
    }

    /// The mean of a set of genomes, gene by gene; hue is a circular mean. `None` for an empty set.
    #[must_use]
    pub fn mean(genomes: &[Genome]) -> Option<Genome> {
        if genomes.is_empty() {
            return None;
        }
        let n = genomes.len() as f32;
        let mut mean = [0.0; GENES];
        for gene in Gene::ALL {
            let values = genomes.iter().map(|g| g.get(gene));
            mean[gene as usize] = if gene.wraps() {
                let (s, c) = values.fold((0.0, 0.0), |(s, c), v| {
                    let a = v * std::f32::consts::TAU;
                    (s + a.sin(), c + a.cos())
                });
                f32::atan2(s, c) / std::f32::consts::TAU
            } else {
                values.sum::<f32>() / n
            };
        }
        Some(Genome::new(mean))
    }
}

/// `b - a` for one gene, the short way around for a circular gene, so it lies in `[-0.5, 0.5]` there.
#[must_use]
pub fn gene_delta(gene: Gene, a: f32, b: f32) -> f32 {
    let d = b - a;
    if gene.wraps() { (d + 0.5).rem_euclid(1.0) - 0.5 } else { d }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Genome {
        Genome::new([0.2, 0.5, 0.9, 0.1, 0.7, 0.3, 0.6, 0.95])
    }

    #[test]
    fn values_stay_in_range() {
        let g = Genome::new([-1.0, 2.0, 0.5, f32::NAN, 0.0, 1.0, 0.5, 1.25]);
        assert_eq!(g.get(Gene::Size), 0.0);
        assert_eq!(g.get(Gene::Limbs), 1.0);
        assert_eq!(g.get(Gene::Heat), 0.5);
        assert!((g.get(Gene::Hue) - 0.25).abs() < 1e-6);
        let mut rng = Rng::new(3);
        let mut m = g;
        for _ in 0..200 {
            m = m.mutate(&mut rng, 1.0, 0.5);
            assert!(m.genes().iter().all(|v| (0.0..=1.0).contains(v)));
        }
    }

    #[test]
    fn operators_are_deterministic() {
        let (a, b) = (sample(), Genome::new([0.8; GENES]));
        let run = || {
            let mut rng = Rng::keyed(1, b"genetest", &[2]);
            (a.mutate(&mut rng, 0.5, 0.1), a.recombine(&b, &mut rng))
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn recombination_takes_whole_genes() {
        let (a, b) = (sample(), Genome::new([0.8; GENES]));
        let mut rng = Rng::new(11);
        for _ in 0..50 {
            let c = a.recombine(&b, &mut rng);
            for gene in Gene::ALL {
                assert!(c.get(gene) == a.get(gene) || c.get(gene) == b.get(gene));
            }
        }
    }

    #[test]
    fn hue_means_and_distances_wrap() {
        let a = Genome::new([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.95]);
        let mut b = a;
        b.set(Gene::Hue, 0.05);
        let mean = Genome::mean(&[a, b]).unwrap();
        assert!(mean.get(Gene::Hue) < 0.01 || mean.get(Gene::Hue) > 0.99, "{}", mean.get(Gene::Hue));
        assert!((gene_delta(Gene::Hue, 0.95, 0.05) - 0.1).abs() < 1e-6);
        assert!(a.distance(&b) < 0.04);
    }
}

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
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
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
        let sum: f32 = Gene::ALL
            .iter()
            .map(|&g| {
                let d = gene_delta(g, self.get(g), other.get(g));
                d * d
            })
            .sum();
        (sum / GENES as f32).sqrt()
    }

    /// The mean of a set of genomes, gene by gene. Hue is a circular mean: the point on the wheel nearest all
    /// the hues, found without trigonometry so it is the same on every platform. `None` for an empty set.
    #[must_use]
    pub fn mean(genomes: &[Genome]) -> Option<Genome> {
        if genomes.is_empty() {
            return None;
        }
        let n = genomes.len() as f32;
        let mut mean = [0.0; GENES];
        for gene in Gene::ALL {
            let values = genomes.iter().map(|g| g.get(gene));
            mean[gene as usize] =
                if gene.wraps() { circular_mean(&values.collect::<Vec<_>>()) } else { values.sum::<f32>() / n };
        }
        Some(Genome::new(mean))
    }
}

/// The mean of values on a wheel of circumference 1, such as hues: the point with the least sum of squared
/// distances to them, each measured the short way around (their Fréchet mean). In `[0, 1)`. Uses only
/// exactly rounded arithmetic, no trigonometry, so it is the same on every platform.
///
/// Measured that way, the sum is a quadratic in the candidate point between the places where the point's
/// opposite crosses one of the values, and each quadratic piece is least at `(S + k) / n`, wrapped, for some
/// integer `k`, where `S` is the sum of the values and `n` their count. So the minimum is one of the `n`
/// points `(S + k) / n` for `k` in `0..n`. Each is scored by its sum of squared distances, and the lowest
/// wins, the smallest `k` on a tie.
///
/// When the values have no single nearest point, as for an opposite pair (0 and 0.5 give 0.25 and 0.75 equal
/// scores) or values spread evenly around the wheel, the candidates tie or differ only by rounding. The result
/// is then whichever scores lowest after rounding, or the smallest `k` on an exact tie (0.25 for that pair): a
/// fixed function of the values and their order, but one of several equally good answers. For the clustered
/// hues of a pool it agrees closely with the mean direction of the hues as angles. `0.0` for no values.
fn circular_mean(values: &[f32]) -> f32 {
    let n = values.len() as f32;
    let sum: f32 = values.iter().sum();
    let spread = |c: f32| -> f32 {
        values
            .iter()
            .map(|&v| {
                let d = gene_delta(Gene::Hue, c, v);
                d * d
            })
            .sum()
    };
    let mut best = (f32::INFINITY, 0.0);
    for k in 0..values.len() {
        let c = ((sum + k as f32) / n).rem_euclid(1.0);
        let s = spread(c);
        if s < best.0 {
            best = (s, c);
        }
    }
    best.1
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

    #[test]
    fn circular_mean_wraps_through_zero() {
        for (values, expect) in
            [(&[0.95, 0.05][..], 0.0), (&[0.9, 0.98, 0.04], 0.973_333), (&[0.02, 0.97, 0.99], 0.993_333)]
        {
            let mean = circular_mean(values);
            assert!((0.0..1.0).contains(&mean), "{values:?}: {mean}");
            assert!(gene_delta(Gene::Hue, mean, expect).abs() < 1e-5, "{values:?}: {mean}, not {expect}");
        }
    }

    #[test]
    #[allow(clippy::disallowed_methods)] // The mean direction of the hues as angles is the reference here.
    fn circular_mean_agrees_with_the_angle_mean_on_clusters() {
        let angle_mean = |values: &[f32]| {
            let (s, c) = values.iter().fold((0.0, 0.0), |(s, c), &v| {
                let a = f64::from(v) * std::f64::consts::TAU;
                (s + a.sin(), c + a.cos())
            });
            (f64::atan2(s, c) / std::f64::consts::TAU) as f32
        };
        // Clusters about as wide as an evolved pool's hues usually are, centered all around the wheel.
        let mut rng = Rng::new(1);
        for _ in 0..5_000 {
            let center = rng.unit();
            let values: Vec<f32> =
                (0..2 + rng.below(23)).map(|_| (center + (rng.unit() * 2.0 - 1.0) * 0.1).rem_euclid(1.0)).collect();
            let (exact, angles) = (circular_mean(&values), angle_mean(&values));
            assert!(gene_delta(Gene::Hue, exact, angles).abs() < 0.01, "{values:?}: {exact} against {angles}");
        }
    }

    #[test]
    fn circular_mean_settles_ties_by_the_first_candidate() {
        // Opposite hues have two equally near points, a quarter turn either side; the first candidate wins
        // whichever order the hues come in.
        assert_eq!(circular_mean(&[0.0, 0.5]), 0.25);
        assert_eq!(circular_mean(&[0.5, 0.0]), 0.25);
        assert_eq!(circular_mean(&[0.25, 0.75]), 0.5);
        // Four hues evenly spaced have four equally near points, between each pair.
        assert_eq!(circular_mean(&[0.0, 0.25, 0.5, 0.75]), 0.375);
        assert_eq!(circular_mean(&[]), 0.0);
    }
}

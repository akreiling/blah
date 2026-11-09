//! Life in a region: which clades live there, and what the region's biome has made of them.
//!
//! Fitness is a pure function of a phenotype and a biome, read from the biome's [`Fitness`] row. A region's
//! life is a pure function of `(seed, region)`. It picks two to four clades by how well each could come to
//! fit the biome, seeds each from its clade's base genome, and runs a short evolution under the biome:
//! [`POOL_SIZE`] genomes for [`GENERATIONS`] generations of fitness-proportional selection,
//! recombination, and mutation. What comes out is one [`Species`] per clade, with its pool and the look and name that follow.
//!
//! Growth is first-reveal only for now. Two seams are left for what the design calls growth at the
//! frontier: [`founders`] is where neighboring regions' pools will seed a new region instead of the bare
//! clade base, and [`generations`] is where region age will set how much history a region gets.

use crate::biome::{
    Biome, BiomeId, Fitness, INSULATION_SLACK, STRANDED, W_AGGRESSION, W_CAMO, W_DRY, W_INSULATION, W_LIMBS, W_SIZE,
    W_TOLERANCE, W_WET,
};
use crate::clade::{Clade, CladeId};
use crate::genome::{Gene, Genome, gene_delta};
use crate::hash::Rng;
use crate::names;
use crate::phenotype::{COVERINGS, Phenotype};
use crate::region::{Region, RegionId};
use crate::sprite::Sprite;

/// Genomes in a population's pool.
pub const POOL_SIZE: usize = 24;
/// Generations a pool is evolved for when its region is first grown.
pub const GENERATIONS: u32 = 32;
/// Fewest clades living in a region.
pub const MIN_CLADES: usize = 2;
/// Most clades living in a region.
pub const MAX_CLADES: usize = 4;

/// Spread of founders around their clade's base genome.
const FOUNDER_SIGMA: f32 = 0.10;
/// Chance each gene mutates in a child.
const MUTATION_RATE: f32 = 0.3;
/// Size of a mutation step.
const MUTATION_SIGMA: f32 = 0.06;
/// The fittest genomes, copied unchanged into the next generation.
const ELITE: usize = 2;
/// How far each gene can move toward a biome's ideal when judging how fit a clade could become.
const REACH: f32 = 0.35;

/// How well a phenotype suits a biome, in `(0, 1]`: `exp(−penalty)`, where the penalty sums the terms
/// described on [`Fitness`] with the weights beside [`BIOMES`](crate::BIOMES). Every term is zero or more,
/// and all of them are zero only for a creature that is exactly what the biome wants.
///
/// The exponential is computed from exactly rounded operations rather than the platform's `exp`, so the same
/// genome scores the same bits everywhere; see the crate docs.
#[must_use]
pub fn fitness(p: &Phenotype, biome: &Biome) -> f32 {
    let f: &Fitness = &biome.fitness;
    let g = &p.genome;
    let limbs = g.get(Gene::Limbs);
    let sq = |x: f32| x * x;
    // Limb pressure costs the shortfall from the end it pays for: all legs when positive, none when negative.
    let limb_shortfall = f.limbs.max(0.0) * (1.0 - limbs) + (-f.limbs).max(0.0) * limbs;
    let mut penalty = W_TOLERANCE * (sq(p.heat - f.heat) + sq(p.cold - f.cold))
        + W_WET * sq(p.water - f.wet)
        + W_INSULATION * sq(((p.covering.traits().insulation - f.insulation).abs() - INSULATION_SLACK).max(0.0))
        + W_DRY * sq((p.covering.traits().needs_wet - f.wet).max(0.0))
        + W_SIZE * sq(p.size - f.size)
        + W_LIMBS * limb_shortfall
        + W_AGGRESSION * sq(g.get(Gene::Aggression) - f.aggression)
        + W_CAMO * f.camo_strength * sq(2.0 * gene_delta(Gene::Hue, p.hue, f.camo_hue));
    if !p.access.allows(biome) {
        penalty += STRANDED;
    }
    exp_neg(penalty)
}

/// Inputs above this are scored as this. `e^(−80)` is about 1.8e-35, still a normal `f32` (the smallest is
/// about 1.2e-38), so [`exp_neg`] never returns zero or a subnormal, which proportional selection would divide
/// by. Fitness penalties stay far below it: every term at its worst plus [`STRANDED`] sums to at most 16.25.
const EXP_NEG_MAX: f64 = 80.0;
/// Times [`exp_neg`] halves its input before the series, and so squares the series after it.
const EXP_NEG_HALVINGS: u32 = 8;
/// Terms of the Taylor series in [`exp_neg`] after the constant 1.
const EXP_NEG_TERMS: u32 = 8;

/// `e^(−p)` for `p` in `[0, 80]` (clamped into it), built only from operations IEEE 754 rounds exactly, in a
/// fixed order, so it gives the same bits on every platform; the platform's `exp` comes from its math library,
/// which can differ in the last bit, and a last bit is enough to flip a fitness-weighted draw.
///
/// It computes `e^p = (e^(p / 2^8))^(2^8)` in `f64`: `x = p / 256` is at most 0.3125, where eight Taylor terms,
/// in the nested form `1 + x(1 + x/2(1 + x/3(…)))`, leave a relative error below 1e-10; eight squarings raise
/// that to the 256th power, growing the error 256-fold; and the reciprocal, rounded to `f32`, is the answer.
/// Over the penalties fitness produces (`x` below 0.07) the series error is negligible and the result is within
/// rounding of the true value, so it agrees with a good `f32::exp` to an ulp.
///
/// Every step is a non-decreasing function of its input under round-to-nearest (the series has only
/// non-negative terms, squaring a positive number is increasing, and the reciprocal reverses the order), so
/// the result never increases as `p` grows. At `p = 0` every step is exact and the result is exactly 1.
fn exp_neg(p: f32) -> f32 {
    let x = f64::from(p).clamp(0.0, EXP_NEG_MAX) / f64::from(1u32 << EXP_NEG_HALVINGS);
    let mut e = 1.0;
    for k in (1..=EXP_NEG_TERMS).rev() {
        e = 1.0 + x / f64::from(k) * e;
    }
    for _ in 0..EXP_NEG_HALVINGS {
        e *= e;
    }
    (1.0 / e) as f32
}

/// The genome `base` would become if every gene moved straight toward what `biome` rewards, by at most
/// `reach`. Used to judge which clades could thrive in a region before evolving any of them.
#[must_use]
pub fn reach_toward(base: &Genome, biome: &Biome, reach: f32) -> Genome {
    let f = &biome.fitness;
    // The center of the covering band nearest the preferred insulation.
    let covering = COVERINGS
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1.insulation - f.insulation).abs().total_cmp(&(b.1.insulation - f.insulation).abs()))
        .map_or(0.5, |(i, _)| (i as f32 + 0.5) / COVERINGS.len() as f32);
    let limbs = if f.limbs > 0.0 {
        1.0
    } else if f.limbs < 0.0 {
        0.0
    } else {
        base.get(Gene::Limbs)
    };
    let targets = [
        (Gene::Size, f.size),
        (Gene::Limbs, limbs),
        (Gene::Covering, covering),
        (Gene::Heat, f.heat),
        (Gene::Cold, f.cold),
        (Gene::Water, f.wet),
        (Gene::Aggression, f.aggression),
        (Gene::Hue, f.camo_hue),
    ];
    let mut out = *base;
    for (gene, target) in targets {
        let d = gene_delta(gene, base.get(gene), target).clamp(-reach, reach);
        out.set(gene, base.get(gene) + d);
    }
    out
}

/// How fit `clade` could become in `biome`: the fitness of its base genome moved a fixed reach toward the
/// biome's ideal. It decides which clades settle a region.
#[must_use]
pub fn potential(clade: &Clade, biome: &Biome) -> f32 {
    fitness(&Phenotype::of(&reach_toward(&clade.base, biome, REACH)), biome)
}

/// The founding genomes of `clade` in region `region`: the clade's base genome, spread by mutation.
///
/// This is the seam for growth at the frontier: once neighbors matter, founders will be drawn from the
/// pools of already-grown neighboring regions where the clade lives, falling back to this.
#[must_use]
pub fn founders(seed: u64, clade: &Clade, region: RegionId) -> Vec<Genome> {
    let mut rng = region_rng(seed, b"founders", region, clade.id);
    (0..POOL_SIZE).map(|_| clade.base.mutate(&mut rng, 1.0, FOUNDER_SIGMA)).collect()
}

/// Generations of history a region gets when first grown.
///
/// The seam for succession: region age will scale this, so old regions arrive deeply evolved and young
/// ones barely changed from their founders.
#[must_use]
pub const fn generations(_region: &Region) -> u32 {
    GENERATIONS
}

/// Runs `generations` generations of selection under `biome` on `pool`, in place. Each generation keeps the
/// fittest few unchanged and fills the rest with mutated children of parents drawn in proportion to fitness.
/// Pure given `rng`.
///
/// Selection is proportional rather than by rank so that pressure scales with how much a trait matters: a
/// slight camouflage edge stays slight, instead of counting as much as surviving the heat once everything
/// else has settled. That keeps lineages from converging on one look per biome.
pub fn evolve(pool: &mut Vec<Genome>, biome: &Biome, generations: u32, rng: &mut Rng) {
    let n = pool.len();
    if n == 0 {
        return;
    }
    for _ in 0..generations {
        let scores: Vec<f32> = pool.iter().map(|g| fitness(&Phenotype::of(g), biome)).collect();
        let mut order: Vec<usize> = (0..n).collect();
        // Index breaks ties, so the order never depends on the sort's stability.
        order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]).then(a.cmp(&b)));
        let mut next: Vec<Genome> = order.iter().take(ELITE.min(n)).map(|&i| pool[i]).collect();
        while next.len() < n {
            let (a, b) = (rng.weighted(&scores), rng.weighted(&scores));
            next.push(pool[a].recombine(&pool[b], rng).mutate(rng, MUTATION_RATE, MUTATION_SIGMA));
        }
        *pool = next;
    }
}

/// `clade`'s pool in region `region` of `biome`, grown from its founders for `generations` generations.
#[must_use]
pub fn grow_pool(seed: u64, clade: &Clade, biome: BiomeId, region: RegionId, generations: u32) -> Vec<Genome> {
    let mut pool = founders(seed, clade, region);
    let mut rng = region_rng(seed, b"evolving", region, clade.id);
    evolve(&mut pool, biome.biome(), generations, &mut rng);
    pool
}

fn region_rng(seed: u64, label: &[u8; 8], region: RegionId, clade: CladeId) -> Rng {
    let (x, y) = region.anchor();
    Rng::keyed(seed, label, &[x as u64, y as u64, u64::from(clade.0)])
}

/// A species: for now, one clade in one region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpeciesId {
    /// Where it lives.
    pub region: RegionId,
    /// What it descends from.
    pub clade: CladeId,
}

/// One species' population and everything shown about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Species {
    /// Its id.
    pub id: SpeciesId,
    /// Placeholder name, such as "Dune Skitter".
    pub name: String,
    /// The biome of its region, which shaped it.
    pub biome: BiomeId,
    /// Representative genomes. Individuals are drawn from these.
    pub pool: Vec<Genome>,
    /// Mean of the pool, gene by gene.
    pub mean: Genome,
    /// The phenotype of the pool member nearest the mean: what the species looks like.
    pub typical: Phenotype,
    /// The typical phenotype in words, with the body named: "small six-legged chitinous crawler, …".
    pub description: String,
    /// The typical phenotype's sprite.
    pub sprite: Sprite,
    /// Its share of the region's creatures; the shares in a region sum to 1.
    pub abundance: f32,
}

/// Everything alive in one region.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionLife {
    /// The region.
    pub region: RegionId,
    /// The region's biome.
    pub biome: BiomeId,
    /// Its species, in clade order.
    pub species: Vec<Species>,
}

impl RegionLife {
    /// Grows the life of `region` in the world for `seed`, whose clades are `clades`.
    #[must_use]
    pub fn grow(seed: u64, clades: &[Clade], region: &Region) -> RegionLife {
        let biome = region.biome.biome();
        let mut rng = region_rng(seed, b"settlers", region.id, CladeId(0));
        // Clades that could come to fit are likelier to settle; cubing sharpens that preference.
        let mut weights: Vec<f32> = clades.iter().map(|c| potential(c, biome)).map(|p| p * p * p).collect();
        let count = (MIN_CLADES + rng.below(MAX_CLADES - MIN_CLADES + 1)).min(clades.len());
        let mut chosen = Vec::with_capacity(count);
        for _ in 0..count {
            let i = rng.weighted(&weights);
            weights[i] = 0.0;
            chosen.push(i);
        }
        chosen.sort_unstable();

        let generations = generations(region);
        let mut species: Vec<Species> = chosen
            .into_iter()
            .map(|i| {
                let clade = &clades[i];
                let pool = grow_pool(seed, clade, region.biome, region.id, generations);
                Species::from_pool(seed, clade, region, pool)
            })
            .collect();
        let total: f32 = species.iter().map(|s| s.abundance).sum();
        for s in &mut species {
            s.abundance = if total > 0.0 { s.abundance / total } else { 1.0 / count as f32 };
        }
        RegionLife { region: region.id, biome: region.biome, species }
    }

    /// The species of `clade` here, if that clade lives here.
    #[must_use]
    pub fn species_of(&self, clade: CladeId) -> Option<&Species> {
        self.species.iter().find(|s| s.id.clade == clade)
    }

    /// The region's biome productivity, which sets how many creatures it supports.
    #[must_use]
    pub fn productivity(&self) -> f32 {
        self.biome.biome().productivity
    }
}

impl Species {
    /// The species `clade` becomes in `region` with this pool. Its abundance is the pool's mean fitness,
    /// which [`RegionLife::grow`] normalizes into a share.
    #[must_use]
    pub fn from_pool(seed: u64, clade: &Clade, region: &Region, pool: Vec<Genome>) -> Species {
        let biome = region.biome.biome();
        let mean = Genome::mean(&pool).unwrap_or(clade.base);
        let typical = pool.iter().copied().min_by(|a, b| a.distance(&mean).total_cmp(&b.distance(&mean)));
        let typical = Phenotype::of(&typical.unwrap_or(mean));
        let abundance = pool.iter().map(|g| fitness(&Phenotype::of(g), biome)).sum::<f32>() / pool.len().max(1) as f32;
        Species {
            id: SpeciesId { region: region.id, clade: clade.id },
            name: names::species_name(seed, region.id, region.biome, clade.root),
            biome: region.biome,
            description: typical.describe_as(clade.plan.family.noun()),
            sprite: Sprite::draw(&typical, &clade.plan, region.biome),
            mean,
            typical,
            pool,
            abundance,
        }
    }
}

/// How one trait of a species differs from the same clade elsewhere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraitDelta {
    /// The gene compared.
    pub gene: Gene,
    /// Difference of pool means, this species minus the other.
    pub delta: f32,
    /// The difference in arrows, −3 to 3: ▲▲ is `2`. Zero when the difference is too small to mention.
    pub marks: i8,
}

/// Genes compared between populations: the continuous ones. Covering and hue show in the sprite instead.
pub const COMPARED: [Gene; 6] = [Gene::Size, Gene::Limbs, Gene::Heat, Gene::Cold, Gene::Water, Gene::Aggression];

/// Differences in pool means at which a delta earns one, two, and three arrows.
pub const MARKS_AT: [f32; 3] = [0.08, 0.2, 0.35];

/// How `species` differs from `other`, gene by gene over [`COMPARED`].
#[must_use]
pub fn compare(species: &Species, other: &Species) -> Vec<TraitDelta> {
    COMPARED
        .iter()
        .map(|&gene| {
            let delta = gene_delta(gene, other.mean.get(gene), species.mean.get(gene));
            let marks = MARKS_AT.iter().filter(|&&at| delta.abs() >= at).count() as i8 * delta.signum() as i8;
            TraitDelta { gene, delta, marks }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clade::clades;
    use crate::genome::GENES;
    use crate::hash::combine;
    use crate::world::World;

    fn biome(name: &str) -> BiomeId {
        BiomeId::all().find(|b| b.biome().name == name).unwrap()
    }

    /// The mean of `gene` in `clade`'s pool grown under `biome`, keyed by region `id`. The region only
    /// keys the random streams here; the biome is what's being tested.
    fn mean_gene(seed: u64, id: RegionId, clade: &Clade, biome: BiomeId, gene: Gene) -> f32 {
        let pool = grow_pool(seed, clade, biome, id, GENERATIONS);
        Genome::mean(&pool).unwrap().get(gene)
    }

    #[test]
    fn pools_are_deterministic() {
        let c = clades(4);
        let id = World::new(4).region_at(0, 0);
        assert_eq!(grow_pool(4, &c[2], biome("Desert"), id, 10), grow_pool(4, &c[2], biome("Desert"), id, 10));
    }

    #[test]
    fn desert_breeds_heat_tolerance_and_tundra_cold() {
        for seed in [1, 2] {
            let id = World::new(seed).region_at(0, 0);
            for clade in &clades(seed) {
                let (desert, tundra) = (biome("Desert"), biome("Tundra"));
                let mean = |biome, gene| mean_gene(seed, id, clade, biome, gene);
                let heat = (mean(desert, Gene::Heat), mean(tundra, Gene::Heat));
                assert!(heat.0 > heat.1 + 0.2, "seed {seed} {:?}: heat {heat:?}", clade.plan.family);
                let cold = (mean(desert, Gene::Cold), mean(tundra, Gene::Cold));
                assert!(cold.1 > cold.0 + 0.2, "seed {seed} {:?}: cold {cold:?}", clade.plan.family);
            }
        }
    }

    #[test]
    fn wet_biomes_breed_water_affinity() {
        for seed in [1, 2] {
            let id = World::new(seed).region_at(0, 0);
            for clade in &clades(seed) {
                let swamp = mean_gene(seed, id, clade, biome("Swamp"), Gene::Water);
                let desert = mean_gene(seed, id, clade, biome("Desert"), Gene::Water);
                let sea = mean_gene(seed, id, clade, biome("Deep water"), Gene::Water);
                assert!(swamp > desert + 0.2, "seed {seed} {:?}: swamp {swamp}, desert {desert}", clade.plan.family);
                assert!(sea > swamp, "seed {seed} {:?}: sea {sea}, swamp {swamp}", clade.plan.family);
            }
        }
    }

    #[test]
    fn fitter_genomes_score_higher() {
        let desert = biome("Desert").biome();
        let base = Genome::new([0.25, 0.7, 0.7, 0.2, 0.2, 0.0, 0.75, 0.12]);
        let mut hardy = base;
        hardy.set(Gene::Heat, 1.0);
        assert!(fitness(&Phenotype::of(&hardy), desert) > fitness(&Phenotype::of(&base), desert));
        let mut furred = hardy;
        furred.set(Gene::Covering, 0.05);
        assert!(fitness(&Phenotype::of(&furred), desert) < fitness(&Phenotype::of(&hardy), desert));
    }

    #[test]
    fn fitness_stays_in_its_range() {
        // Every corner of the genome cube and a spread inside it, scored in every biome, along with each
        // biome's own ideal reached from every corner: near the ideal is where a bonus would overshoot 1.
        let corners: Vec<Genome> = (0..1u32 << GENES)
            .map(|bits| Genome::new(std::array::from_fn(|i| if bits >> i & 1 == 1 { 1.0 } else { 0.0 })))
            .collect();
        let mut rng = Rng::new(17);
        let spread: Vec<Genome> = (0..500).map(|_| Genome::new(std::array::from_fn(|_| rng.unit()))).collect();
        for id in BiomeId::all() {
            let biome = id.biome();
            let ideals = corners.iter().map(|g| reach_toward(g, biome, 1.0));
            for genome in corners.iter().copied().chain(spread.iter().copied()).chain(ideals) {
                let f = fitness(&Phenotype::of(&genome), biome);
                assert!(f > 0.0 && f <= 1.0, "{}: fitness {f} for {genome:?}", biome.name);
            }
        }
    }

    #[test]
    #[allow(clippy::disallowed_methods)] // The platform's exp is the reference here, compared within a bound.
    fn exact_exp_tracks_the_platform_exp() {
        // Every penalty fitness can produce, and on to the clamp, in steps of 0.001.
        for i in 0..=80_000 {
            let p = i as f32 / 1000.0;
            let (exact, platform) = (exp_neg(p), (-p).exp());
            let error = ((exact - platform) / platform).abs();
            assert!(error < 1e-6, "e^-{p}: {exact} against {platform}, relative error {error}");
        }
    }

    #[test]
    fn exact_exp_is_one_at_zero_then_falls_and_never_reaches_zero() {
        assert_eq!(exp_neg(0.0).to_bits(), 1.0f32.to_bits());
        let mut last = exp_neg(0.0);
        for i in 1..=1_000_000 {
            let p = i as f32 / 10_000.0;
            let e = exp_neg(p);
            assert!(e > 0.0 && e <= 1.0 && e.is_normal(), "e^-{p} = {e}");
            if f64::from(p) <= EXP_NEG_MAX {
                assert!(e < last, "e^-{p} = {e} does not fall below {last}");
            } else {
                assert_eq!(e, last, "past the clamp, e^-{p} = {e} should hold at {last}");
            }
            last = e;
        }
    }

    /// Hashes the exact bits of everything grown in a region: each species' clade, pool, mean, abundance,
    /// and sprite.
    fn life_bits(life: &RegionLife) -> u64 {
        let mut h = combine(life.region.anchor().0 as u64, life.region.anchor().1 as u64);
        for s in &life.species {
            h = combine(h, u64::from(s.id.clade.0));
            let genes = s.pool.iter().chain([&s.mean]).flat_map(|g| g.genes());
            for v in genes.chain([s.abundance]) {
                h = combine(h, u64::from(v.to_bits()));
            }
            h = s.sprite.rgba().iter().fold(h, |h, &b| combine(h, u64::from(b)));
        }
        h
    }

    /// Evolution's exact output, pinned. A region's life is meant to be the same bits for the same seed on
    /// every machine; the sim uses only exactly rounded float operations so that it can be.
    ///
    /// If this fails and nobody meant to change what evolution produces, the sim's arithmetic came out
    /// differently on this platform or toolchain: something inexact (a math-library call, a fused
    /// multiply-add, an unordered sum) has crept in, and a fitness-weighted draw went the other way. Find it;
    /// don't update the constant. Update the constant only when the sim was changed on purpose (new fitness
    /// terms, retuned weights, a different selection rule), and say so in the commit.
    #[test]
    fn evolution_is_pinned_to_exact_bits() {
        let mut h = 0;
        for seed in [1, 2, 3] {
            let world = World::new(seed);
            for (x, y) in [(0, 0), (400, -250)] {
                let region = world.region(world.region_at(x, y)).unwrap();
                h = combine(h, life_bits(&RegionLife::grow(seed, &clades(seed), &region)));
            }
        }
        assert_eq!(
            h, 0x88c9_037c_d476_d608,
            "the sim's arithmetic changed; see this test's doc before touching the constant"
        );
    }

    #[test]
    fn limb_pressure_pays_toward_its_end() {
        let fit_with = |name: &str, limbs: f32| {
            let biome = biome(name).biome();
            let mut g = reach_toward(&Genome::new([0.5; GENES]), biome, 1.0);
            g.set(Gene::Limbs, limbs);
            fitness(&Phenotype::of(&g), biome)
        };
        // Rough ground pays for legs, water for none, and the end paid for costs nothing.
        assert!(fit_with("Mountains", 1.0) > fit_with("Mountains", 0.5));
        assert!(fit_with("Deep water", 0.0) > fit_with("Deep water", 0.5));
        assert!(fit_with("Mountains", 1.0) <= 1.0 && fit_with("Deep water", 0.0) <= 1.0);
        // Where there is no pressure, legs don't matter.
        assert_eq!(fit_with("Grassland", 0.0), fit_with("Grassland", 1.0));
    }

    #[test]
    fn region_life_picks_a_few_clades_and_shares_add_up() {
        let world = World::new(9);
        let c = clades(9);
        let region = world.region(world.region_at(0, 0)).unwrap();
        let life = RegionLife::grow(9, &c, &region);
        assert_eq!(life, RegionLife::grow(9, &c, &region));
        assert!((MIN_CLADES..=MAX_CLADES).contains(&life.species.len()));
        let total: f32 = life.species.iter().map(|s| s.abundance).sum();
        assert!((total - 1.0).abs() < 1e-4);
        for s in &life.species {
            assert_eq!(s.pool.len(), POOL_SIZE);
            assert!(s.name.ends_with(c[s.id.clade.0 as usize].root));
        }
        // Across many regions, each holds distinct clades, in clade order.
        for seed in [9, 10, 11] {
            let world = World::new(seed);
            let c = clades(seed);
            for i in 0..12 {
                let region = world.region(world.region_at(i * 150 - 900, (i * 97) % 600 - 300)).unwrap();
                let life = RegionLife::grow(seed, &c, &region);
                let ids: Vec<CladeId> = life.species.iter().map(|s| s.id.clade).collect();
                assert!(ids.windows(2).all(|w| w[0] < w[1]), "seed {seed} {}: clades {ids:?}", region.name);
                assert!((MIN_CLADES..=MAX_CLADES).contains(&ids.len()), "seed {seed} {}: {ids:?}", region.name);
            }
        }
    }

    #[test]
    fn comparison_marks_scale_with_difference() {
        let c = clades(3);
        let world = World::new(3);
        let region = world.region(world.region_at(0, 0)).unwrap();
        let make = |biome: BiomeId| {
            let pool = grow_pool(3, &c[0], biome, region.id, GENERATIONS);
            Species::from_pool(3, &c[0], &Region { biome, ..region.clone() }, pool)
        };
        let (desert, tundra) = (make(biome("Desert")), make(biome("Tundra")));
        let deltas = compare(&desert, &tundra);
        let heat = deltas.iter().find(|d| d.gene == Gene::Heat).unwrap();
        assert!(heat.marks >= 2, "{heat:?}");
        assert!(compare(&desert, &desert).iter().all(|d| d.marks == 0));
    }
}

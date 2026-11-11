//! Regional and Deep life are regenerated from the seed. Only individual identities and genomes belong in saves.

use std::collections::VecDeque;

use blah_core::{
    Band, Biome, Clade, CladeId, Fitness, GENERATIONS, Genome, POOL_SIZE, Phenotype, RegionCache, RegionLife, Rng,
    SpeciesId, World, clades, evolve, fitness, individual, potential,
};
use serde::{Deserialize, Serialize};

use crate::data::{Kind, Stats};
use crate::town::Town;

/// Deep-only adaptations expressed by an evolved creature's existing body genes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaveTraits {
    /// Six-or-more-legged chitinous or scaled life can sense vibrations through stone.
    pub tremorsense: bool,
    /// Cold-tolerant slime life gives off a dim glow.
    pub luminous: bool,
}

/// Tremorsense requires six legs, enough to read ground-borne vibrations.
const TREMORSENSE_MIN_LEGS: u8 = 6;
/// Only cold tolerance of at least one half yields a glow in the Deep.
const LUMINOUS_MIN_COLD: f32 = 0.5;

/// What life under a town selects for: cool, damp, unchanging stone with little to eat. Scarce food favors small
/// bodies and boldness, broken rock favors legs, and the dark makes coloring nearly irrelevant.
pub const DEEP: Biome = Biome {
    name: "Deep",
    glyph: '.',
    color: [96, 90, 84],
    passable: true,
    water: false,
    deep: false,
    productivity: 0.12,
    elevation: Band::ANY,
    moisture: Band::ANY,
    temperature: Band::ANY,
    fitness: Fitness {
        heat: 0.05,
        cold: 0.45,
        wet: 0.55,
        insulation: 0.4,
        size: 0.3,
        limbs: 0.5,
        aggression: 0.75,
        camo_hue: 0.08,
        camo_strength: 0.1,
    },
};

/// The Deep at `depth`: deeper floors are colder and hungrier still, which selects bigger, bolder bodies.
pub fn deep_biome(depth: u32) -> Biome {
    let f = depth.clamp(1, 10) as f32 - 1.0;
    Biome {
        fitness: Fitness { cold: 0.45 + 0.03 * f, size: 0.3 + 0.05 * f, aggression: 0.75 + 0.025 * f, ..DEEP.fitness },
        productivity: DEEP.productivity * (1.0 - 0.05 * f),
        ..DEEP
    }
}

/// Words for life shaped by the Deep. They name where it lives, not senses the genome does not model.
const DEEP_EPITHETS: [&str; 12] =
    ["Cave", "Gloom", "Stone", "Hollow", "Drip", "Burrow", "Shale", "Grotto", "Crag", "Silt", "Under", "Echo"];

/// Where a creature's population lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lineage {
    /// A surface region's species.
    Region(SpeciesId),
    /// A clade evolved on one floor of the Deep under one town.
    Deep {
        /// The town above.
        town: Town,
        /// The floor, from 1.
        depth: u32,
        /// What it descends from.
        clade: CladeId,
    },
}

/// The evolved identity of one creature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wildlife {
    /// The population it descends from.
    pub lineage: Lineage,
    /// Its species name.
    pub name: String,
    /// Its individual genome, including variation from the species pool.
    pub genome: Genome,
}

impl Wildlife {
    /// Its genome read into behavior, access, and physical traits.
    pub fn phenotype(&self) -> Phenotype {
        Phenotype::of(&self.genome)
    }

    /// Cave adaptations read from existing body genes; surface lineages never gain cave-only traits.
    pub fn cave_traits(&self) -> CaveTraits {
        if !matches!(self.lineage, Lineage::Deep { .. }) {
            return CaveTraits::default();
        }
        let phenotype = self.phenotype();
        CaveTraits {
            tremorsense: phenotype.legs >= TREMORSENSE_MIN_LEGS
                && matches!(phenotype.covering, blah_core::Covering::Chitin | blah_core::Covering::Scale),
            luminous: phenotype.covering == blah_core::Covering::Slime && phenotype.cold >= LUMINOUS_MIN_COLD,
        }
    }

    /// Floors below the surface it lives on; zero for surface life.
    pub fn depth(&self) -> u32 {
        match self.lineage {
            Lineage::Region(_) => 0,
            Lineage::Deep { depth, .. } => depth,
        }
    }

    /// Its genome's numbers, hardened by every floor below the first: deeper life has survived worse.
    pub(crate) fn stats(&self) -> Stats {
        let p = self.phenotype().stats;
        let f = self.depth().saturating_sub(1) as i32;
        let hp = p.hp + p.hp * f / 3;
        let attack = p.attack + f;
        let defense = p.defense + f / 2;
        let xp = (hp + 2 * attack + defense) as u32;
        Stats {
            name: self.name.clone(),
            hp,
            ac: 9 + defense,
            hit: attack - 1,
            damage: (1.max(p.attack - 2) + f / 2, attack),
            xp: xp * (2 + f as u32) / 2,
            gold: (3 * f, attack + 6 * f),
            ..Kind::Wild.stats()
        }
    }
}

/// One clade's population in one Deep.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DeepSpecies {
    pub(crate) clade: CladeId,
    pub(crate) name: String,
    pub(crate) pool: Vec<Genome>,
    pub(crate) abundance: f32,
}

/// Grows the life on one floor beneath `town`: two to four clades chosen by how well they could come to fit
/// that depth, each evolved there from its clade's base genome.
pub(crate) fn grow_deep(seed: u64, clades: &[Clade], town: Town, depth: u32) -> Vec<DeepSpecies> {
    let biome = deep_biome(depth);
    let mut rng = Rng::keyed(seed, b"deeplife", &[town.origin.0 as u64, town.origin.1 as u64, u64::from(depth)]);
    let mut weights: Vec<f32> = clades.iter().map(|c| potential(c, &biome)).map(|p| p * p * p).collect();
    let count = (2 + rng.below(3)).min(clades.len());
    let mut chosen = Vec::with_capacity(count);
    for _ in 0..count {
        let i = rng.weighted(&weights);
        weights[i] = 0.0;
        chosen.push(i);
    }
    chosen.sort_unstable();
    let mut species: Vec<DeepSpecies> = chosen
        .into_iter()
        .map(|i| {
            let clade = &clades[i];
            let mut pool: Vec<Genome> = (0..POOL_SIZE).map(|_| clade.base.mutate(&mut rng, 1.0, 0.10)).collect();
            evolve(&mut pool, &biome, GENERATIONS, &mut rng);
            let abundance = pool.iter().map(|g| fitness(&Phenotype::of(g), &biome)).sum::<f32>() / pool.len() as f32;
            let name = format!("{} {}", DEEP_EPITHETS[rng.below(DEEP_EPITHETS.len())], clade.root);
            DeepSpecies { clade: clade.id, name, pool, abundance }
        })
        .collect();
    let total: f32 = species.iter().map(|s| s.abundance).sum();
    for s in &mut species {
        s.abundance = if total > 0.0 { s.abundance / total } else { 1.0 / count as f32 };
    }
    species
}

/// Disposable, bounded caches. Equality ignores them because warming them cannot affect game state.
#[derive(Clone, Default)]
pub(crate) struct LifeCache {
    inner: Option<Cache>,
}

#[derive(Clone)]
struct Cache {
    seed: u64,
    regions: RegionCache,
    clades: Vec<Clade>,
    lives: VecDeque<RegionLife>,
    deeps: VecDeque<((Town, u32), Vec<DeepSpecies>)>,
}

impl PartialEq for LifeCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for LifeCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LifeCache")
    }
}

impl LifeCache {
    #[cfg(test)]
    pub(crate) fn inner_is_empty(&self) -> bool {
        self.inner.is_none()
    }

    fn cache(&mut self, seed: u64) -> &mut Cache {
        if self.inner.as_ref().is_none_or(|cache| cache.seed != seed) {
            self.inner = Some(Cache {
                seed,
                regions: RegionCache::new(World::new(seed)),
                clades: clades(seed),
                lives: VecDeque::new(),
                deeps: VecDeque::new(),
            });
        }
        self.inner.as_mut().expect("initialized above")
    }

    pub(crate) fn draw(&mut self, seed: u64, at: (i64, i64), rng: &mut Rng) -> Wildlife {
        let cache = self.cache(seed);
        let id = cache.regions.region_at(at.0, at.1);
        let index = if let Some(index) = cache.lives.iter().position(|life| life.region == id) {
            index
        } else {
            let region = cache.regions.region(id).expect("region_at returns a real region");
            if cache.lives.len() == 16 {
                cache.lives.pop_front();
            }
            cache.lives.push_back(RegionLife::grow(seed, &cache.clades, &region));
            cache.lives.len() - 1
        };
        let life = &cache.lives[index];
        let weights: Vec<f32> = life.species.iter().map(|s| s.abundance).collect();
        let species = &life.species[rng.weighted(&weights)];
        Wildlife {
            lineage: Lineage::Region(species.id),
            name: species.name.clone(),
            genome: individual(&species.pool[rng.below(species.pool.len())], rng),
        }
    }

    pub(crate) fn deep_species(&mut self, seed: u64, town: Town, depth: u32) -> &[DeepSpecies] {
        let cache = self.cache(seed);
        let index = if let Some(index) = cache.deeps.iter().position(|(key, _)| *key == (town, depth)) {
            index
        } else {
            if cache.deeps.len() == 8 {
                cache.deeps.pop_front();
            }
            let species = grow_deep(seed, &cache.clades, town, depth);
            cache.deeps.push_back(((town, depth), species));
            cache.deeps.len() - 1
        };
        &cache.deeps[index].1
    }

    pub(crate) fn draw_deep(&mut self, seed: u64, town: Town, depth: u32, rng: &mut Rng) -> Wildlife {
        let life = self.deep_species(seed, town, depth);
        let weights: Vec<f32> = life.iter().map(|s| s.abundance).collect();
        let species = &life[rng.weighted(&weights)];
        Wildlife {
            lineage: Lineage::Deep { town, depth, clade: species.clade },
            name: species.name.clone(),
            genome: individual(&species.pool[rng.below(species.pool.len())], rng),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn individuals_come_from_the_original_regional_pools() {
        let seed = 7;
        let world = World::new(seed);
        let at = (-100, 80);
        let id = world.region_at(at.0, at.1);
        let region = world.region(id).unwrap();
        let life = RegionLife::grow(seed, &clades(seed), &region);
        let mut rng = Rng::new(42);
        let weights: Vec<f32> = life.species.iter().map(|s| s.abundance).collect();
        let species = &life.species[rng.weighted(&weights)];
        let genome = individual(&species.pool[rng.below(species.pool.len())], &mut rng);
        let actual = LifeCache::default().draw(seed, at, &mut Rng::new(42));
        assert_eq!(actual.lineage, Lineage::Region(species.id));
        assert_eq!(actual.name, species.name);
        assert_eq!(actual.genome, genome);
    }

    #[test]
    fn life_cache_is_bounded_and_eviction_changes_no_individuals() {
        let mut cache = LifeCache::default();
        let first = cache.draw(7, (0, 0), &mut Rng::new(99));
        let Lineage::Region(species) = first.lineage else { panic!("surface life has a region") };
        for x in 1..=17 {
            cache.draw(7, (x * 500, 0), &mut Rng::new(99));
            assert!(cache.inner.as_ref().unwrap().lives.len() <= 16);
        }
        assert!(!cache.inner.as_ref().unwrap().lives.iter().any(|l| l.region == species.region));
        assert_eq!(cache.draw(7, (0, 0), &mut Rng::new(99)), first);
        let changed = cache.draw(8, (0, 0), &mut Rng::new(99));
        assert_eq!(changed, LifeCache::default().draw(8, (0, 0), &mut Rng::new(99)));
    }

    fn towns(seed: u64) -> Vec<Town> {
        let mut game = crate::Game::new(seed);
        game.answer("Ash");
        game.level().towns().collect()
    }

    #[test]
    fn deep_life_is_evolved_for_the_deep_and_walks_its_floors() {
        let mut fitter = 0;
        let mut total = 0;
        for seed in [3, 7, 11] {
            for town in towns(seed) {
                let deep = grow_deep(seed, &clades(seed), town, 1);
                assert!((2..=4).contains(&deep.len()));
                assert!((deep.iter().map(|s| s.abundance).sum::<f32>() - 1.0).abs() < 1e-4);
                for species in &deep {
                    let clade = &clades(seed)[species.clade.0 as usize];
                    let evolved = species.pool.iter().map(|g| fitness(&Phenotype::of(g), &DEEP)).sum::<f32>();
                    let base = fitness(&Phenotype::of(&clade.base), &DEEP) * species.pool.len() as f32;
                    total += 1;
                    fitter += usize::from(evolved > base);
                    let walkers = species.pool.iter().filter(|g| Phenotype::of(g).access.allows(&DEEP)).count();
                    assert!(walkers * 2 > species.pool.len(), "{} cannot walk the Deep", species.name);
                    assert!(DEEP_EPITHETS.iter().any(|e| species.name == format!("{e} {}", clade.root)));
                }
            }
        }
        assert!(fitter * 10 >= total * 9, "evolution under the Deep makes pools fitter there: {fitter}/{total}");
    }

    #[test]
    fn deep_draws_are_reproducible_after_eviction_and_differ_between_towns() {
        let all = towns(7);
        let mut cache = LifeCache::default();
        let first = cache.draw_deep(7, all[0], 1, &mut Rng::new(5));
        assert!(matches!(first.lineage, Lineage::Deep { town, .. } if town == all[0]));
        for &town in all.iter().cycle().skip(1).take(9) {
            cache.draw_deep(7, town, 1, &mut Rng::new(5));
            assert!(cache.inner.as_ref().unwrap().deeps.len() <= 8);
        }
        assert_eq!(cache.draw_deep(7, all[0], 1, &mut Rng::new(5)), first);
        let names: std::collections::HashSet<Vec<String>> =
            all.iter().map(|&t| cache.deep_species(7, t, 1).iter().map(|s| s.name.clone()).collect()).collect();
        assert!(names.len() > 1, "each town's Deep has its own life");
    }

    #[test]
    fn deeper_floors_evolve_bigger_bolder_life_that_is_much_harder_to_kill() {
        let town = towns(7)[0];
        let mut cache = LifeCache::default();
        let mean = |cache: &mut LifeCache, depth: u32| {
            let mut rng = Rng::new(11);
            let draws: Vec<Wildlife> = (0..200).map(|_| cache.draw_deep(7, town, depth, &mut rng)).collect();
            let n = draws.len() as f32;
            let size = draws.iter().map(|w| w.phenotype().size).sum::<f32>() / n;
            let stats: Vec<Stats> = draws.iter().map(Wildlife::stats).collect();
            let hp = stats.iter().map(|s| s.hp).sum::<i32>() as f32 / n;
            let hit = stats.iter().map(|s| s.hit).sum::<i32>() as f32 / n;
            let xp = stats.iter().map(|s| s.xp).sum::<u32>() as f32 / n;
            (size, hp, hit, xp)
        };
        let (one, five, ten) = (mean(&mut cache, 1), mean(&mut cache, 5), mean(&mut cache, 10));
        assert!(one.0 < ten.0, "deeper selection favors bigger bodies: {one:?} {ten:?}");
        assert!(five.1 > one.1 * 1.8 && ten.1 > one.1 * 3.0, "health: {one:?} {five:?} {ten:?}");
        assert!(five.2 >= one.2 + 4.0 && ten.2 >= one.2 + 9.0, "accuracy: {one:?} {five:?} {ten:?}");
        assert!(ten.3 > one.3 * 10.0, "deep kills are worth far more: {one:?} {ten:?}");
    }
}

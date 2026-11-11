//! Regional life is regenerated from the seed. Only individual identities and genomes belong in saves.

use std::collections::VecDeque;

use blah_core::{Clade, Genome, Phenotype, RegionCache, RegionLife, Rng, SpeciesId, World, clades, individual};
use serde::{Deserialize, Serialize};

use crate::data::{Kind, Stats};

/// The evolved identity of one surface creature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wildlife {
    /// The region and clade it descends from.
    pub species: SpeciesId,
    /// Its regional species name.
    pub name: String,
    /// Its individual genome, including variation from the species pool.
    pub genome: Genome,
}

impl Wildlife {
    /// Its genome read into behavior, access, and physical traits.
    pub fn phenotype(&self) -> Phenotype {
        Phenotype::of(&self.genome)
    }

    pub(crate) fn stats(&self) -> Stats {
        let p = self.phenotype().stats;
        Stats {
            name: self.name.clone(),
            hp: p.hp,
            ac: 9 + p.defense,
            hit: p.attack - 1,
            damage: (1.max(p.attack - 2), p.attack),
            xp: (p.hp + 2 * p.attack + p.defense) as u32,
            gold: (0, p.attack),
            ..Kind::Wild.stats()
        }
    }
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

    pub(crate) fn draw(&mut self, seed: u64, at: (i64, i64), rng: &mut Rng) -> Wildlife {
        if self.inner.as_ref().is_none_or(|cache| cache.seed != seed) {
            self.inner = Some(Cache {
                seed,
                regions: RegionCache::new(World::new(seed)),
                clades: clades(seed),
                lives: VecDeque::new(),
            });
        }
        let cache = self.inner.as_mut().expect("initialized above");
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
            species: species.id,
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
        assert_eq!(actual.species, species.id);
        assert_eq!(actual.name, species.name);
        assert_eq!(actual.genome, genome);
    }

    #[test]
    fn life_cache_is_bounded_and_eviction_changes_no_individuals() {
        let mut cache = LifeCache::default();
        let first = cache.draw(7, (0, 0), &mut Rng::new(99));
        for x in 1..=17 {
            cache.draw(7, (x * 500, 0), &mut Rng::new(99));
            assert!(cache.inner.as_ref().unwrap().lives.len() <= 16);
        }
        assert!(!cache.inner.as_ref().unwrap().lives.iter().any(|l| l.region == first.species.region));
        assert_eq!(cache.draw(7, (0, 0), &mut Rng::new(99)), first);
        let changed = cache.draw(8, (0, 0), &mut Rng::new(99));
        assert_eq!(changed, LifeCache::default().draw(8, (0, 0), &mut Rng::new(99)));
    }
}

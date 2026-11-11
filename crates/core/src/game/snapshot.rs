//! Versioned state, without terrain, life, or render caches.

use super::*;
use serde::{Deserialize, Serialize};

/// Snapshot layout. Bump when serialized fields or their meaning change.
pub const SNAPSHOT_VERSION: u32 = 1;
/// Terrain, biome table, region, and naming compatibility. Bump for incompatible generation changes.
pub const TERRAIN_VERSION: u32 = 1;
/// Simulation, evolution, clade, and technique-catalog compatibility.
pub const RULES_VERSION: u32 = 1;

/// A complete action-boundary state. Serialization is pure; storage belongs to the app.
/// Fields are private so callers restore through the version and invariant checks.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameSnapshot {
    snapshot_version: u32,
    terrain_version: u32,
    rules_version: u32,
    seed: u64,
    position: (i64, i64),
    home: (i64, i64),
    visited: Vec<RegionId>,
    turn: u64,
    log: Log,
    wanderer: Wanderer,
    monsters: Vec<CreatureSnapshot>,
    next_monster: u64,
    deaths: u32,
    maneuver: Maneuver,
    sneaking: bool,
    last_attack: Option<AttackResult>,
    sneak_moves: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatureSnapshot {
    id: MonsterId,
    species: SpeciesId,
    name: String,
    genome: Genome,
    phenotype: Phenotype,
    position: (i64, i64),
    hp: i32,
    home: (i64, i64),
    alerted: bool,
    energy: i32,
    practice_left: i32,
    stealth_left: i32,
    tracking: bool,
}

impl From<&Monster> for CreatureSnapshot {
    fn from(m: &Monster) -> Self {
        Self {
            id: m.id,
            species: m.species,
            name: m.name.clone(),
            genome: m.genome,
            phenotype: m.phenotype,
            position: m.position,
            hp: m.hp,
            home: m.home,
            alerted: m.alerted,
            energy: m.energy,
            practice_left: m.practice_left,
            stealth_left: m.stealth_left,
            tracking: m.tracking,
        }
    }
}

impl GameSnapshot {
    /// The world's seed.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Turns passed.
    #[must_use]
    pub const fn turn(&self) -> u64 {
        self.turn
    }

    /// Times the wanderer has died.
    #[must_use]
    pub const fn deaths(&self) -> u32 {
        self.deaths
    }
}

impl Game {
    /// Copy authoritative state at an action boundary. No caches or biome-table indices are saved.
    #[must_use]
    pub fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            snapshot_version: SNAPSHOT_VERSION,
            terrain_version: TERRAIN_VERSION,
            rules_version: RULES_VERSION,
            seed: self.world.seed(),
            position: self.position,
            home: self.home,
            visited: self.visited.clone(),
            turn: self.turn,
            log: self.log.clone(),
            wanderer: self.wanderer,
            monsters: self.monsters.iter().map(CreatureSnapshot::from).collect(),
            next_monster: self.next_monster,
            deaths: self.deaths,
            maneuver: self.maneuver.clone(),
            sneaking: self.sneaking,
            last_attack: self.last_attack,
            sneak_moves: self.sneak_moves,
        }
    }

    /// Restore without taking an action or granting practice. Derived state is regenerated.
    /// Unsupported versions and invalid state are rejected, never repaired silently.
    pub fn from_snapshot(s: GameSnapshot) -> Result<Self, &'static str> {
        if s.snapshot_version != SNAPSHOT_VERSION {
            return Err("Unsupported core snapshot version; this save needs a compatible game or migration.");
        }
        if s.terrain_version != TERRAIN_VERSION {
            return Err("Incompatible terrain generator version; the original save has not been changed.");
        }
        if s.rules_version != RULES_VERSION {
            return Err("Incompatible simulation rules version; the original save has not been changed.");
        }
        s.maneuver.validate()?;
        let coordinate = |(x, y): (i64, i64)| x.unsigned_abs() <= (1 << 61) && y.unsigned_abs() <= (1 << 61);
        let w = &s.wanderer;
        if !coordinate(s.position)
            || !coordinate(s.home)
            || s.visited.is_empty()
            || s.visited.len() > VISITED_LIMIT
            || s.visited.iter().any(|id| !coordinate(id.anchor()))
            || s.visited.iter().collect::<HashSet<_>>().len() != s.visited.len()
            || w.stats != crate::WANDERER_STATS
            || !(1..=w.max_hp()).contains(&w.hp)
            || !(0..=w.max_focus()).contains(&w.focus)
            || s.log.len() > LOG_LIMIT
            || s.log.total < s.log.len() as u64
            || s.turn > u64::MAX - DEATH_TURNS - 2
            || s.monsters.len() > 128
            || s.monsters.iter().map(|m| m.id).collect::<HashSet<_>>().len() != s.monsters.len()
        {
            return Err("Invalid character or world state in save.");
        }
        let world = World::new(s.seed);
        let mut regions = RegionCache::new(world);
        let region = region_of(&mut regions, s.position);
        if s.visited.last() != Some(&region.id) {
            return Err("Saved region history does not match the character's position.");
        }
        let mut game = Game {
            world,
            regions,
            clades: clades(s.seed),
            lives: HashMap::new(),
            position: s.position,
            home: s.home,
            region,
            visited: s.visited,
            turn: s.turn,
            log: s.log,
            wanderer: s.wanderer,
            monsters: Vec::new(),
            next_monster: s.next_monster,
            deaths: s.deaths,
            maneuver: s.maneuver,
            sneaking: s.sneaking,
            last_attack: s.last_attack,
            sneak_moves: s.sneak_moves,
            crept: false,
        };
        for &id in &game.visited.clone() {
            if game.regions.region(id).is_none() {
                return Err("Saved history names an unknown region.");
            }
            game.life(id);
        }
        for m in s.monsters {
            let stats = m.phenotype.stats;
            let genes_valid = |g: &Genome| g.genes().iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v));
            if !coordinate(m.position)
                || !coordinate(m.home)
                || !coordinate(m.species.region.anchor())
                || usize::from(m.species.clade.0) >= game.clades.len()
                || m.id.0 >= game.next_monster
                || !(1..=10_000).contains(&stats.hp)
                || !(1..=stats.hp).contains(&m.hp)
                || !(0..=10_000).contains(&stats.attack)
                || !(0..=10_000).contains(&stats.defense)
                || !(0..=100).contains(&stats.speed)
                || !(0..=10_000).contains(&m.energy)
                || !(0..=stats.hp).contains(&m.practice_left)
                || !(0..=STEALTH_LESSONS).contains(&m.stealth_left)
                || !genes_valid(&m.genome)
                || !genes_valid(&m.phenotype.genome)
                || ![m.phenotype.size, m.phenotype.heat, m.phenotype.cold, m.phenotype.water, m.phenotype.hue]
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                || ![0, 2, 4, 6, 8].contains(&m.phenotype.legs)
                || !(0..=100).contains(&m.phenotype.sense)
                || game.regions.region(m.species.region).is_none()
            {
                return Err("Invalid creature state in save.");
            }
            let species = game
                .life(m.species.region)
                .species_of(m.species.clade)
                .ok_or("Saved creature names an unknown species.")?;
            let biome = species.biome;
            let sprite = Sprite::draw(&m.phenotype, &game.clade(m.species.clade).plan, biome);
            game.monsters.push(Monster {
                id: m.id,
                species: m.species,
                name: m.name,
                genome: m.genome,
                phenotype: m.phenotype,
                sprite,
                position: m.position,
                hp: m.hp,
                home: m.home,
                alerted: m.alerted,
                energy: m.energy,
                practice_left: m.practice_left,
                stealth_left: m.stealth_left,
                tracking: m.tracking,
            });
        }
        Ok(game)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_full_state_and_regenerates_all_referenced_life() {
        for mut game in [
            Game::new(7),
            Game::composition_trial(42, Maneuver::backstab_burst()).unwrap(),
            Game::inferno_trial(42).unwrap(),
        ] {
            for i in 0..80 {
                game.step(Dir::ALL[(i / 9) % 4]);
                let loaded = Game::from_snapshot(game.snapshot()).unwrap();
                assert_eq!(loaded, game);
                assert_eq!(loaded.local_life(), game.local_life());
                for m in loaded.monsters() {
                    assert_eq!(loaded.species(m.species), game.species(m.species));
                }
            }
        }
    }

    #[test]
    fn rejects_incompatible_versions_and_invalid_state() {
        for version in 0..3 {
            let mut s = Game::new(42).snapshot();
            match version {
                0 => s.snapshot_version += 1,
                1 => s.terrain_version += 1,
                _ => s.rules_version += 1,
            }
            assert!(Game::from_snapshot(s).is_err());
        }
        let mut s = Game::new(42).snapshot();
        s.wanderer.hp = 0;
        assert!(Game::from_snapshot(s).is_err());
    }
}

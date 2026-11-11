//! Versioned state, without terrain, life, or render caches.

use super::*;
use serde::{Deserialize, Serialize};

/// Snapshot layout. Bump when serialized fields or their meaning change.
pub const SNAPSHOT_VERSION: u32 = 4;
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
    #[serde(default = "starting_inventory")]
    inventory: Vec<crate::WeaponItem>,
    #[serde(default)]
    equipped: crate::WeaponItem,
    #[serde(default = "starting_salves")]
    salves: u8,
    #[serde(default)]
    caches: Vec<CacheRecord>,
    #[serde(default)]
    herbs: [u8; 3],
    #[serde(default)]
    draughts: u8,
    /// Patches picked recently enough that they have not regrown.
    #[serde(default)]
    picked: Vec<PickedPatch>,
    #[serde(default = "journey_enabled")]
    journey: bool,
    sneaking: bool,
    last_attack: Option<AttackResult>,
    sneak_moves: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PickedPatch {
    position: (i64, i64),
    turn: u64,
}

fn starting_inventory() -> Vec<crate::WeaponItem> {
    crate::WeaponItem::STARTING.to_vec()
}

const fn starting_salves() -> u8 {
    crate::SALVE_CAPACITY
}

const fn journey_enabled() -> bool {
    true
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
            inventory: self.inventory.clone(),
            equipped: self.equipped,
            salves: self.salves,
            caches: self.caches.values().cloned().collect(),
            herbs: self.herbs,
            draughts: self.draughts,
            picked: self.active_picks().map(|(position, turn)| PickedPatch { position, turn }).collect(),
            journey: self.journey,
            sneaking: self.sneaking,
            last_attack: self.last_attack,
            sneak_moves: self.sneak_moves,
        }
    }

    /// Restore without taking an action or granting practice. Derived state is regenerated.
    /// Unsupported versions and invalid state are rejected, never repaired silently.
    pub fn from_snapshot(mut s: GameSnapshot) -> Result<Self, &'static str> {
        // Version 1 predates commitments; version 2 also predates equipment, salves, and caches; version 3
        // predates herbs and draughts.
        if (1..=3).contains(&s.snapshot_version) {
            s.herbs = [0; 3];
            s.draughts = 0;
            s.picked.clear();
        }
        match s.snapshot_version {
            1 => {
                s.wanderer.adaptations = crate::Adaptations::default();
                s.inventory = starting_inventory();
                s.equipped = crate::WeaponItem::plain(s.maneuver.weapon);
                s.salves = starting_salves();
                s.caches.clear();
                s.journey = true;
                s.snapshot_version = SNAPSHOT_VERSION;
            }
            2 => {
                s.inventory = starting_inventory();
                s.equipped = crate::WeaponItem::plain(s.maneuver.weapon);
                s.salves = starting_salves();
                s.caches.clear();
                s.journey = true;
                s.snapshot_version = SNAPSHOT_VERSION;
            }
            3 => s.snapshot_version = SNAPSHOT_VERSION,
            _ => {}
        }
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
            || !w.adaptations.consistent()
            || w.adaptations.held().any(|a| !a.earned(w))
            || s.log.len() > LOG_LIMIT
            || s.log.total < s.log.len() as u64
            || s.turn > u64::MAX - DEATH_TURNS - 2
            || s.monsters.len() > 65_536
            || s.monsters.iter().map(|m| m.id).collect::<HashSet<_>>().len() != s.monsters.len()
            || s.inventory.len() > 8
            || s.inventory.iter().copied().collect::<HashSet<_>>().len() != s.inventory.len()
            || crate::WeaponItem::STARTING.iter().any(|item| !s.inventory.contains(item))
            || !s.inventory.contains(&s.equipped)
            || s.equipped.weapon() != s.maneuver.weapon
            || s.salves > crate::SALVE_CAPACITY
            || s.caches.len() > 65_536
            || s.caches.iter().map(|cache| cache.region).collect::<HashSet<_>>().len() != s.caches.len()
            || (!s.journey && !s.caches.is_empty())
            || s.herbs.iter().any(|&count| count > crate::HERB_CAPACITY)
            || s.draughts > crate::DRAUGHT_CAPACITY
            || s.picked.len() > 4_096
            || (!s.journey && !s.picked.is_empty())
            || s.picked.iter().map(|pick| pick.position).collect::<HashSet<_>>().len() != s.picked.len()
            || s.picked.iter().any(|pick| {
                !coordinate(pick.position) || pick.turn > s.turn || s.turn - pick.turn >= crate::HERB_REGROW_TURNS
            })
        {
            return Err("Invalid character or world state in save.");
        }
        let world = World::new(s.seed);
        let mut regions = RegionCache::new(world);
        for cache in &mut s.caches {
            let Some(cache_region) = regions.region(cache.region) else {
                return Err("Saved cache names an unknown region.");
            };
            let Some(position) = Self::cache_site(&world, &mut regions, &cache_region, s.home) else {
                return Err("Saved cache is not in a land region.");
            };
            cache.position = position;
            cache.treasure = Self::cache_reward(s.seed, cache.region);
        }
        let region = region_of(&mut regions, s.position);
        if s.visited.last() != Some(&region.id) {
            return Err("Saved region history does not match the character's position.");
        }
        let mut cache_guard_ids = HashSet::new();
        for cache in &s.caches {
            if !coordinate(cache.position) {
                return Err("Invalid cache state in save.");
            }
            let Some(cache_region) = regions.region(cache.region) else {
                return Err("Saved cache names an unknown region.");
            };
            let Some(site) = Self::cache_site(&world, &mut regions, &cache_region, s.home) else {
                return Err("Saved cache is not in a land region.");
            };
            if regions.region_at(site.0, site.1) != cache.region
                || !is_land(world.tile(site.0, site.1).biome.biome())
                || !(2..=3).contains(&cache.guards.len())
                || (cache.claimed && !s.inventory.contains(&cache.treasure))
            {
                return Err("Invalid cache state in save.");
            }
            for guard in &cache.guards {
                let stats = guard.phenotype.stats;
                if !cache_guard_ids.insert(guard.id)
                    || guard.id.0 >= s.next_monster
                    || guard.species.region != cache.region
                    || usize::from(guard.species.clade.0) >= clades(s.seed).len()
                    || !coordinate(guard.position)
                    || chebyshev(guard.position, cache.position) > TERRITORY
                    || guard.phenotype.temperament != crate::Temperament::Territorial
                    || !(1..=10_000).contains(&stats.hp)
                    || !(0..=stats.hp).contains(&guard.hp)
                    || !(0..=10_000).contains(&stats.attack)
                    || !(0..=10_000).contains(&stats.defense)
                    || !(0..=100).contains(&stats.speed)
                    || !(0..=10_000).contains(&guard.energy)
                    || !(0..=stats.hp).contains(&guard.practice_left)
                    || !(0..=STEALTH_LESSONS).contains(&guard.stealth_left)
                    || !guard
                        .genome
                        .genes()
                        .iter()
                        .chain(guard.phenotype.genome.genes().iter())
                        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
                {
                    return Err("Invalid cache guard state in save.");
                }
                let monster = s.monsters.iter().find(|m| m.id == guard.id);
                if guard.hp > 0 {
                    if let Some(monster) = monster {
                        if monster.hp != guard.hp
                            || monster.position != guard.position
                            || monster.home != cache.position
                            || monster.species != guard.species
                            || monster.name != guard.name
                            || monster.genome != guard.genome
                            || monster.phenotype != guard.phenotype
                            || monster.alerted != guard.alerted
                            || monster.energy != guard.energy
                            || monster.practice_left != guard.practice_left
                            || monster.stealth_left != guard.stealth_left
                            || monster.tracking != guard.tracking
                        {
                            return Err("Cache guard does not match its saved cache record.");
                        }
                    } else if chebyshev(guard.position, s.position) <= DESPAWN_RADIUS {
                        return Err("A nearby living cache guard is missing from the world state.");
                    }
                } else if monster.is_some() {
                    return Err("A dead cache guard is still present in the world state.");
                }
            }
        }
        for pick in &s.picked {
            let id = regions.region_at(pick.position.0, pick.position.1);
            let region = regions.region(id).ok_or("Saved herb patch names an unknown region.")?;
            if !Self::herb_patches(&world, &mut regions, &region, s.home)
                .iter()
                .any(|patch| patch.position == pick.position)
            {
                return Err("Saved herb patch is not a patch of this world.");
            }
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
            inventory: s.inventory,
            equipped: s.equipped,
            salves: s.salves,
            caches: s.caches.into_iter().map(|cache| (cache.region, cache)).collect(),
            herbs: s.herbs,
            draughts: s.draughts,
            picked: s.picked.iter().map(|pick| (pick.position, pick.turn)).collect(),
            herb_sites: BTreeMap::new(),
            journey: s.journey,
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
        game.update_visible_herbs();
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

    #[test]
    fn migrates_serialized_v1_without_commitments_and_writes_current_version() {
        let mut game = Game::new(42);
        game.wanderer.focus_skill.learn(4_000, Rank::Expert);
        let mut legacy = serde_json::to_value(game.snapshot()).unwrap();
        legacy["snapshot_version"] = 1.into();
        legacy["wanderer"].as_object_mut().unwrap().remove("adaptations");
        let object = legacy.as_object_mut().unwrap();
        for field in ["inventory", "equipped", "salves", "caches", "journey"] {
            object.remove(field);
        }
        let bytes = serde_json::to_vec(&legacy).unwrap();
        let snapshot = serde_json::from_slice(&bytes).unwrap();
        let mut loaded = Game::from_snapshot(snapshot).unwrap();
        assert_eq!(loaded.position, game.position);
        assert_eq!(loaded.turn, game.turn);
        assert_eq!(loaded.wanderer.focus_skill, game.wanderer.focus_skill);
        assert_eq!(loaded.wanderer.adaptations, crate::Adaptations::default());
        assert_eq!(loaded.inventory, crate::WeaponItem::STARTING);
        assert_eq!(loaded.salves, crate::SALVE_CAPACITY);
        assert!(loaded.caches.is_empty());
        loaded.monsters.clear();
        loaded.commit_adaptation(crate::Adaptation::DeepReserves).unwrap();
        let saved = serde_json::to_value(loaded.snapshot()).unwrap();
        assert_eq!(saved["snapshot_version"], SNAPSHOT_VERSION);
        assert_eq!(saved["wanderer"]["adaptations"]["taken"][0], "DeepReserves");
        let round_trip = Game::from_snapshot(serde_json::from_value(saved).unwrap()).unwrap();
        assert_eq!(round_trip, loaded);
    }

    #[test]
    fn migrates_serialized_v2_to_the_starting_kit_without_claims() {
        let game = Game::new(42);
        let mut legacy = serde_json::to_value(game.snapshot()).unwrap();
        legacy["snapshot_version"] = 2.into();
        let object = legacy.as_object_mut().unwrap();
        for field in ["inventory", "equipped", "salves", "caches", "journey"] {
            object.remove(field);
        }
        let snapshot = serde_json::from_value(legacy).unwrap();
        let loaded = Game::from_snapshot(snapshot).unwrap();
        assert_eq!(loaded.inventory, crate::WeaponItem::STARTING.to_vec());
        assert_eq!(loaded.equipped.weapon(), loaded.maneuver.weapon);
        assert_eq!(loaded.salves, crate::SALVE_CAPACITY);
        assert!(loaded.caches.is_empty());
        assert!(loaded.journey);
        assert_eq!(loaded.snapshot().snapshot_version, SNAPSHOT_VERSION);
    }

    #[test]
    fn migrates_serialized_v3_without_changing_existing_items_or_claims() {
        let mut game = Game::new(42);
        let region = game.region.clone();
        let site = Game::cache_site(&game.world, &mut game.regions, &region, game.home).unwrap();
        game.position = site;
        game.update_region(false);
        assert_eq!(game.claim_cache(), Step::Waited);
        assert!(game.cache_at(site.0, site.1).unwrap().claimed);
        let mut legacy = serde_json::to_value(game.snapshot()).unwrap();
        legacy["snapshot_version"] = 3.into();
        for field in ["herbs", "draughts", "picked"] {
            legacy.as_object_mut().unwrap().remove(field);
        }
        let loaded = Game::from_snapshot(serde_json::from_value(legacy).unwrap()).unwrap();
        assert_eq!(loaded, game);
        assert_eq!(loaded.herbs, [0; 3]);
        assert_eq!(loaded.draughts, 0);
        assert!(loaded.picked.is_empty());
        assert_eq!(loaded.snapshot().snapshot_version, SNAPSHOT_VERSION);
    }

    #[test]
    fn rejects_inconsistent_or_unearned_commitments_in_v2() {
        for taken in [
            serde_json::json!(["DeepReserves", "ConcentratedChanneling"]),
            serde_json::json!(["DeepReserves", "DeepReserves"]),
            serde_json::json!([null, "DeepReserves"]),
            serde_json::json!(["VeiledMovement", null]),
        ] {
            let mut state = serde_json::to_value(Game::new(42).snapshot()).unwrap();
            state["wanderer"]["adaptations"]["taken"] = taken;
            let snapshot = serde_json::from_value(state).unwrap();
            assert!(Game::from_snapshot(snapshot).is_err());
        }
    }
}

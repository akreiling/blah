//! The pure core of the game. For now: endless, deterministic terrain, its regions, the life evolved in
//! them, and a wanderer walking it and fighting what it meets.
//!
//! The world is an endless grid of tiles. Each tile is a pure function of `(seed, x, y)`: four noise fields
//! (elevation, moisture, temperature, age) sampled at the tile, and a biome looked up from the first three
//! in [`BIOMES`]. Nothing is stored and nothing depends on the order tiles are asked for, so a tile read on
//! its own equals the same tile read as part of a [`Chunk`], on any machine.
//!
//! Tiles group into [`Region`]s, the units life lives in, which are just as pure: see [`World::region_at`]
//! and, for lookups on every step, [`RegionCache`].
//!
//! Life descends from a few [`Clade`]s per world. A creature is a [`Genome`], read into a [`Phenotype`]
//! (stats, temperament, terrain, a description) and drawn as a [`Sprite`]. Each region's [`RegionLife`] is
//! its clades evolved under its biome's [`Fitness`] row, so the same lineage looks and acts differently from
//! biome to biome. A [`Game`] spawns individuals from those pools around the wanderer.
//!
//! This crate does no I/O, reads no clock, and starts no threads.
//!
//! The same seed gives the same bits on every platform, terrain and life alike, because floats go only through
//! operations IEEE 754 rounds exactly: `+ − × ÷`, `sqrt`, comparisons, `floor` and `round`, remainders, and
//! casts. Rust never fuses them into multiply-adds, and sums run over slices in a fixed order. The math
//! library (`exp`, `sin`, `powf`, `powi`, `hypot`, …) is never used: its results can differ in the last bit
//! between platforms and Rust versions, and a last bit can flip a fitness-weighted draw and with it a whole gene
//! pool. Where the sim needs an exponential or a circular mean, it builds one from exact operations. This
//! crate's `clippy.toml` disallows the math library (tests that compare against it opt out), and two tests
//! pin the exact bits that evolution and a scripted game produce.
//!
//! ```
//! use blah_core::World;
//!
//! let world = World::new(42);
//! let tile = world.tile(-1_000, 250);
//! println!("{} at elevation {:.2}", tile.biome.biome().name, tile.elevation);
//! let region = world.region(world.region_at(-1_000, 250)).unwrap();
//! println!("in {}, {} tiles", region.name, region.tiles);
//! ```

mod adaptation;
mod biome;
mod chunk;
mod clade;
mod combat;
mod fields;
mod game;
mod genome;
mod hash;
mod life;
mod monster;
mod moves;
mod names;
mod noise;
mod phenotype;
mod region;
mod skills;
mod sprite;
mod world;

pub use adaptation::{ADAPTATION_CAPACITY, Adaptation, Adaptations, BREAKTHROUGH_RANK};
pub use biome::{BIOMES, Band, Biome, BiomeId, Fitness, SEA_LEVEL, classify};
pub use chunk::{CHUNK_AREA, CHUNK_SIZE, Chunk, chunk_of, chunk_origin, local_of};
pub use clade::{BodyPlan, CLADES, Clade, CladeId, Family, Pattern, clades};
pub use combat::{
    Aim, AreaPreview, AttackContext, AttackPreview, AttackResult, Burst, Delivery, Element, MAX_AREA_RADIUS,
    MAX_BURST_POWER, MAX_FOCUS, Maneuver, SNEAK_REDUCTION, Shape, Technique, Trigger, Weapon,
};
pub use game::{
    Comparison, DEATH_TURNS, DESPAWN_RADIUS, Dir, Game, LAND_SEARCH_RADIUS, LOG_LIMIT, Log, LogEntry, LogKind,
    REGEN_TURNS, REST_FOCUS, REST_HEALTH, SPAWN_MAX, SPAWN_MIN, Step, TERRITORY, VIEW_RADIUS, WANDER_CHANCE, capacity,
    find_land, individual, ring, spawn_chance,
};
pub use game::{GameSnapshot, RULES_VERSION, SNAPSHOT_VERSION, TERRAIN_VERSION};
pub use genome::{GENES, Gene, Genome, gene_delta};
pub use hash::{Rng, hash_point};
pub use life::{
    COMPARED, GENERATIONS, MARKS_AT, MAX_CLADES, MIN_CLADES, POOL_SIZE, RegionLife, Species, SpeciesId, TraitDelta,
    compare, evolve, fitness, founders, generations, grow_pool, potential, reach_toward,
};
pub use monster::{ACTION_COST, MAX_ACTIONS, MISS_CHANCE, Monster, MonsterId, WANDERER_STATS, Wanderer, damage};
pub use moves::{LearnedMove, MovePreview};
pub use phenotype::{
    AGGRESSIVE_AT, AQUATIC_AT, Access, COVERINGS, Covering, CoveringTraits, HARDY_AT, Phenotype, SKITTISH_BELOW,
    SWIM_AT, Stats, Temperament, WADE_AT,
};
pub use region::{
    Bounds, CELL_SPACING, CORE_TILES, MAX_REGION_TILES, Means, Region, RegionCache, RegionId, SLIVER_TILES, STRAY_TILES,
};
pub use skills::{Rank, Skill, Training, WeaponSkills};
pub use sprite::{SPRITE_SIZE, Sprite};
pub use world::{Tile, World};

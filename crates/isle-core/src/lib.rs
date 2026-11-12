//! Isle: a solo island adventure in the manner of *Island of Kesmai*. You type a command, and the round
//! resolves when the clock runs out: your command first, then every creature in turn.
//!
//! The core is pure and deterministic. A seed and the same commands give the same island, and the whole
//! [`Game`] serializes, dice included, so a saved game resumes exactly. The window supplies the clock.

mod cinder;
mod command;
mod data;
mod deep;
mod game;
mod herb;
mod landmark;
mod map;
mod practice;
mod relic;
mod rng;
mod town;
mod wildlife;

#[cfg(test)]
mod tests;

pub use cinder::{CinderWatch, WatchPhase};
pub use command::{Command, Dir, HELP, Instant, MAX_STEPS, Parsed, Target, parse};
pub use data::{Armor, Bow, Item, Kind, SHOP, Spell, Stats, Weapon, sale_price};
pub use deep::Deep;
pub use game::{Creature, Game, Line, Phase, Pile, Player, SACK, SIGHT, Sighted, Tone, modifier};
pub use herb::{Herb, HerbPatch};
pub use landmark::{Landmark, LandmarkKind};
pub use map::{LevelId, Place, Pos, Terrain, Tile, Zone, distance, line};
pub use practice::{Practice, Practices};
pub use relic::Relic;
pub use town::Town;
pub use wildlife::{CaveTraits, DEEP, Lineage, Wildlife};

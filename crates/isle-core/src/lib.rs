//! Isle: a solo island adventure in the manner of *Island of Kesmai*. You type a command, and the round
//! resolves when the clock runs out: your command first, then every creature in turn.
//!
//! The core is pure and deterministic. A seed and the same commands give the same island, and the whole
//! [`Game`] serializes, dice included, so a saved game resumes exactly. The window supplies the clock.

mod command;
mod data;
mod game;
mod map;
mod rng;

#[cfg(test)]
mod tests;

pub use command::{Command, Dir, HELP, Instant, MAX_STEPS, Parsed, Target, parse};
pub use data::{Armor, Bow, Class, Item, Kind, LEVELS, SHOP, Spell, Stats, Weapon, sale_price};
pub use game::{Creature, Game, Line, Phase, Pile, Player, SACK, SIGHT, Sighted, Tone, modifier};
pub use map::{Level, LevelId, Place, Pos, SpawnKind, Terrain, Tile, Zone, distance, line};

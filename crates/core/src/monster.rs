//! Monsters and the wanderer as combatants: who they are, and the rules for hitting each other.
//!
//! The game loop that moves them lives in [`Game`](crate::Game); this module holds the pieces it uses.

use crate::combat::{MAX_FOCUS, SNEAK_REDUCTION};
use crate::genome::Genome;
use crate::hash::Rng;
use crate::life::SpeciesId;
use crate::phenotype::{Phenotype, Stats};
use crate::sprite::Sprite;

/// A monster's stable id within one game, in order of appearance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MonsterId(pub u64);

/// One creature near the wanderer: an individual drawn from its species' pool.
#[derive(Clone, Debug, PartialEq)]
pub struct Monster {
    /// Its id.
    pub id: MonsterId,
    /// Its species.
    pub species: SpeciesId,
    /// Its species' name, for messages: "Dune Skitter".
    pub name: String,
    /// Its own genome, a pool member with a little individual variation.
    pub genome: Genome,
    /// What its genome amounts to.
    pub phenotype: Phenotype,
    /// Its own sprite, which differs slightly from the species sprite.
    pub sprite: Sprite,
    /// Tile coordinates.
    pub position: (i64, i64),
    /// Hit points left.
    pub hp: i32,
    /// Where it spawned. Territorial monsters guard the ground around it.
    pub home: (i64, i64),
    /// Whether this individual has located the wanderer. Sneaking does not erase existing tracking.
    pub alerted: bool,
    /// Speed banked toward its next action, in tenths of an action.
    pub(crate) energy: i32,
}

impl Monster {
    /// Heat tolerance becomes up to 75 percent fire resistance in the combat prototype.
    #[must_use]
    pub fn fire_resistance(&self) -> u8 {
        (self.phenotype.heat.clamp(0.0, 1.0) * 75.0).round() as u8
    }

    /// Whether its senses locate the actor now. Break normal sensing range to lose tracking.
    #[must_use]
    pub fn detects(&self, position: (i64, i64), sneaking: bool) -> bool {
        let distance = (self.position.0 - position.0).abs().max((self.position.1 - position.1).abs());
        let range = if sneaking && !self.alerted {
            self.phenotype.sense.saturating_sub(SNEAK_REDUCTION).max(0)
        } else {
            self.phenotype.sense
        };
        distance <= range
    }

    /// Hit points when unhurt.
    #[must_use]
    pub const fn max_hp(&self) -> i32 {
        self.phenotype.stats.hp
    }
}

/// The wanderer's state in a fight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wanderer {
    /// Hit points left.
    pub hp: i32,
    /// Combat numbers. Speed is always [`ACTION_COST`]: one action a turn.
    pub stats: Stats,
    /// Focus left for special techniques and careful movement; no passive regeneration.
    pub focus: i32,
}

impl Wanderer {
    /// A fresh wanderer.
    #[must_use]
    pub const fn new() -> Wanderer {
        Wanderer { hp: WANDERER_STATS.hp, stats: WANDERER_STATS, focus: MAX_FOCUS }
    }

    /// Hit points when unhurt.
    #[must_use]
    pub const fn max_hp(&self) -> i32 {
        self.stats.hp
    }
}

impl Default for Wanderer {
    fn default() -> Wanderer {
        Wanderer::new()
    }
}

/// The wanderer's numbers until levels and abilities exist.
pub const WANDERER_STATS: Stats = Stats { hp: 30, attack: 4, defense: 1, speed: ACTION_COST };

/// Energy one action costs. A creature banks its speed each turn and acts once per this much.
pub const ACTION_COST: i32 = 10;

/// Most actions a monster takes in one turn, however fast it is.
pub const MAX_ACTIONS: u32 = 2;

/// Chance that any attack misses outright.
pub const MISS_CHANCE: f32 = 0.1;

/// Damage from one attack, or `None` for a miss: attack minus defense, plus a roll of −1 to +1, at least 1.
pub fn damage(attack: i32, defense: i32, rng: &mut Rng) -> Option<i32> {
    if rng.chance(MISS_CHANCE) {
        return None;
    }
    let roll = rng.below(3) as i32 - 1;
    Some((attack - defense + roll).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_is_at_least_one_and_tracks_the_gap() {
        let mut rng = Rng::new(1);
        let mut hits = 0;
        for _ in 0..500 {
            if let Some(d) = damage(2, 9, &mut rng) {
                assert_eq!(d, 1);
                hits += 1;
            }
            if let Some(d) = damage(8, 2, &mut rng) {
                assert!((5..=7).contains(&d));
            }
        }
        assert!((380..500).contains(&hits), "{hits} hits");
    }
}

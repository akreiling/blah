//! Monsters and the wanderer as combatants: who they are, and the rules for hitting each other.
//!
//! The game loop that moves them lives in [`Game`](crate::Game); this module holds the pieces it uses.

use crate::combat::MAX_FOCUS;
use crate::genome::Genome;
use crate::hash::Rng;
use crate::life::SpeciesId;
use crate::phenotype::{Phenotype, Stats};
use crate::sprite::Sprite;
use crate::{Adaptation, Adaptations, Skill, Temperament, Training, Weapon, WeaponSkills};

/// A monster's stable id within one game, in order of appearance.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    /// Finite damage evidence left for this individual. Healing after death never refills it.
    pub(crate) practice_left: i32,
    /// Finite stealth lessons left for this individual, shared by every way of evading it.
    pub(crate) stealth_left: i32,
    /// Whether it located the wanderer at the last turn's observation. Losing that is an escape.
    pub(crate) tracking: bool,
}

impl Monster {
    /// Approximate percent of full Health threatened over an unskilled, guarded weapon fight.
    /// Uses full Health and base competence, never current injury, Focus, or earned weapon bonuses.
    pub(crate) fn weapon_threat(&self, wanderer: &Wanderer, weapon: Weapon) -> u32 {
        if self.phenotype.temperament == Temperament::Skittish {
            return 0;
        }
        let stats = self.phenotype.stats;
        let incoming = (i128::from(stats.attack) - i128::from(wanderer.stats.defense)).max(1);
        let outgoing =
            (i128::from(wanderer.stats.attack) + i128::from(weapon.force()) - i128::from(stats.defense)).max(1);
        let speed = i128::from(stats.speed.clamp(0, ACTION_COST * MAX_ACTIONS as i32));
        let threat = 100 * incoming * i128::from(stats.hp.max(0)) * speed
            / (outgoing * i128::from(ACTION_COST) * i128::from(wanderer.max_hp().max(1)));
        threat.min(200) as u32
    }

    /// Consume a shared, finite damage opportunity, including damage delivered by magic.
    pub(crate) fn spend_practice(&mut self, damage: i32) -> u32 {
        let spent = damage.max(0).min(self.hp.max(0)).min(self.practice_left.max(0));
        self.practice_left -= spent;
        spent as u32
    }

    /// Consume up to `units` of this individual's stealth lessons, returning how many were left to take.
    pub(crate) fn spend_stealth(&mut self, units: i32) -> u32 {
        let spent = units.max(0).min(self.stealth_left.max(0));
        self.stealth_left -= spent;
        spent as u32
    }

    /// Heat tolerance becomes up to 75 percent fire resistance in the combat prototype.
    #[must_use]
    pub fn fire_resistance(&self) -> u8 {
        (self.phenotype.heat.clamp(0.0, 1.0) * 75.0).round() as u8
    }

    /// Whether its senses locate the actor now. Concealment shortens only an unalerted individual's
    /// initial sensing range, by `concealment` tiles (0 when not sneaking). Break normal range to lose tracking.
    #[must_use]
    pub fn detects(&self, position: (i64, i64), concealment: i64) -> bool {
        let distance = (self.position.0 - position.0).abs().max((self.position.1 - position.1).abs());
        let range = if self.alerted {
            self.phenotype.sense
        } else {
            self.phenotype.sense.saturating_sub(concealment.max(0)).max(0)
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
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wanderer {
    /// Hit points left.
    pub hp: i32,
    /// Combat numbers. Speed is always [`ACTION_COST`]: one action a turn.
    pub stats: Stats,
    /// Focus left for special techniques and careful movement; no passive regeneration.
    pub focus: i32,
    /// Earned weapon competence belongs to the character, not to the equipped item.
    pub weapons: WeaponSkills,
    /// Practice paying for techniques that work, which enlarges the Focus pool.
    pub focus_skill: Skill,
    /// Practice evading real threats, which conceals better, sneaks cheaper, and strikes openings harder.
    pub stealth: Skill,
    /// Permanent developments and the history that earns their offers.
    #[serde(default)]
    pub adaptations: Adaptations,
}

impl Wanderer {
    /// A fresh wanderer.
    #[must_use]
    pub fn new() -> Wanderer {
        Wanderer {
            hp: WANDERER_STATS.hp,
            stats: WANDERER_STATS,
            focus: MAX_FOCUS,
            weapons: WeaponSkills::default(),
            focus_skill: Skill::default(),
            stealth: Skill::default(),
            adaptations: Adaptations::default(),
        }
    }

    /// Most Focus the wanderer can hold: [`MAX_FOCUS`] plus earned Focus practice.
    #[must_use]
    pub fn max_focus(&self) -> i32 {
        self.focus_skill.focus_capacity() + if self.adaptations.has(Adaptation::DeepReserves) { 6 } else { 0 }
    }

    /// Weapon and stealth practice for an attack with `weapon`.
    #[must_use]
    pub fn training(&self, weapon: Weapon) -> Training {
        Training {
            weapon: self.weapons.get(weapon),
            stealth: self.stealth,
            disciplined: self.adaptations.has(Adaptation::Discipline(weapon)),
            veiled: self.adaptations.has(Adaptation::VeiledMovement),
            concentrated: self.adaptations.has(Adaptation::ConcentratedChanneling),
        }
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

/// The wanderer's base numbers, before practice.
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

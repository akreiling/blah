//! Adaptations: lasting structural developments a character commits to after earning them.
//!
//! Practice makes an adaptation available; nothing is taken automatically. A character holds at most
//! [`ADAPTATION_CAPACITY`] of them, some exclude each other, and none can be removed. Each one's benefit and
//! cost are applied where the rule they change already lives, so this module only says what exists, what a
//! character has earned, and what fits together.

use serde::{Deserialize, Serialize};

use crate::{Rank, Wanderer, Weapon};

/// Most adaptations one character can hold. A prototype number, deliberately below the catalog's size.
pub const ADAPTATION_CAPACITY: usize = 2;

/// Rank a related skill must reach before its breakthrough is offered.
pub const BREAKTHROUGH_RANK: Rank = Rank::Skilled;

/// One lasting structural development.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Adaptation {
    /// Committed training with one weapon family.
    Discipline(Weapon),
    /// Movement shaped around not being noticed.
    VeiledMovement,
    /// A larger store of Focus that refills more slowly.
    DeepReserves,
    /// Magic pressed into fewer, harder releases.
    ConcentratedChanneling,
}

impl Adaptation {
    /// Every adaptation, in display order.
    pub const ALL: [Adaptation; 7] = [
        Adaptation::Discipline(Weapon::Dagger),
        Adaptation::Discipline(Weapon::Spear),
        Adaptation::Discipline(Weapon::Club),
        Adaptation::Discipline(Weapon::Staff),
        Adaptation::VeiledMovement,
        Adaptation::DeepReserves,
        Adaptation::ConcentratedChanneling,
    ];

    /// The name shown to the player.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Adaptation::Discipline(Weapon::Dagger) => "Dagger discipline",
            Adaptation::Discipline(Weapon::Spear) => "Spear discipline",
            Adaptation::Discipline(Weapon::Club) => "Club discipline",
            Adaptation::Discipline(Weapon::Staff) => "Staff discipline",
            Adaptation::VeiledMovement => "Veiled movement",
            Adaptation::DeepReserves => "Deep reserves",
            Adaptation::ConcentratedChanneling => "Concentrated channeling",
        }
    }

    /// What practice earns it.
    #[must_use]
    pub const fn earned_by(self) -> &'static str {
        match self {
            Adaptation::Discipline(Weapon::Dagger) => "skilled dagger practice",
            Adaptation::Discipline(Weapon::Spear) => "skilled spear practice",
            Adaptation::Discipline(Weapon::Club) => "skilled club practice",
            Adaptation::Discipline(Weapon::Staff) => "skilled staff practice",
            Adaptation::VeiledMovement => "skilled stealth",
            Adaptation::DeepReserves => "skilled Focus",
            Adaptation::ConcentratedChanneling => "skilled Focus and a released burst",
        }
    }

    /// What it gives.
    #[must_use]
    pub const fn benefit(self) -> &'static str {
        match self {
            Adaptation::Discipline(_) => "+1 damage and 2% fewer misses with this weapon family",
            Adaptation::VeiledMovement => "sneaking conceals 2 tiles more; every other sneaking move is free",
            Adaptation::DeepReserves => "+6 maximum Focus",
            Adaptation::ConcentratedChanneling => "every released burst deals 2 more damage, yours included",
        }
    }

    /// What it costs.
    #[must_use]
    pub const fn cost(self) -> &'static str {
        match self {
            Adaptation::Discipline(_) => "other weapon families cannot train past practiced",
            Adaptation::VeiledMovement => "free strikes deal 1 less damage",
            Adaptation::DeepReserves => "resting restores 1 less Focus per turn",
            Adaptation::ConcentratedChanneling => "ground areas are limited to radius 2",
        }
    }

    /// What it rules out, for display.
    #[must_use]
    pub const fn rules_out(self) -> &'static str {
        match self {
            Adaptation::Discipline(_) => "any other weapon discipline",
            Adaptation::VeiledMovement => "nothing",
            Adaptation::DeepReserves => "Concentrated channeling",
            Adaptation::ConcentratedChanneling => "Deep reserves",
        }
    }

    /// Whether a character can hold both.
    #[must_use]
    pub const fn excludes(self, other: Adaptation) -> bool {
        matches!(
            (self, other),
            (Adaptation::Discipline(_), Adaptation::Discipline(_))
                | (Adaptation::DeepReserves, Adaptation::ConcentratedChanneling)
                | (Adaptation::ConcentratedChanneling, Adaptation::DeepReserves)
        )
    }

    /// Whether this wanderer's practice has earned it.
    #[must_use]
    pub fn earned(self, wanderer: &Wanderer) -> bool {
        let at = |rank: Rank| rank >= BREAKTHROUGH_RANK;
        match self {
            Adaptation::Discipline(weapon) => at(wanderer.weapons.get(weapon).rank()),
            Adaptation::VeiledMovement => at(wanderer.stealth.rank()),
            Adaptation::DeepReserves => at(wanderer.focus_skill.rank()),
            Adaptation::ConcentratedChanneling => at(wanderer.focus_skill.rank()) && wanderer.adaptations.channeled,
        }
    }
}

/// The adaptations a character holds, and the history some of them need.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adaptations {
    taken: [Option<Adaptation>; ADAPTATION_CAPACITY],
    /// Whether a burst has ever been released into a creature, which concentration builds on.
    pub(crate) channeled: bool,
}

impl Adaptations {
    /// Whether `adaptation` is held.
    #[must_use]
    pub fn has(&self, adaptation: Adaptation) -> bool {
        self.taken.contains(&Some(adaptation))
    }

    /// The adaptations held, in the order committed.
    pub fn held(&self) -> impl Iterator<Item = Adaptation> + '_ {
        self.taken.iter().flatten().copied()
    }

    /// Structural capacity not yet committed.
    #[must_use]
    pub fn free(&self) -> usize {
        self.taken.iter().filter(|slot| slot.is_none()).count()
    }

    /// Whether `adaptation` fits beside everything held: room left, not held, and excluded by nothing held.
    #[must_use]
    pub fn fits(&self, adaptation: Adaptation) -> bool {
        self.free() > 0 && !self.has(adaptation) && self.held().all(|held| !held.excludes(adaptation))
    }

    /// Whether the held set is one a character could have reached: no repeats, no exclusions, and filled in
    /// order.
    pub(crate) fn consistent(&self) -> bool {
        let held: Vec<_> = self.held().collect();
        let in_order = self.taken.iter().skip_while(|slot| slot.is_some()).all(Option::is_none);
        in_order && held.iter().enumerate().all(|(i, a)| held[..i].iter().all(|b| b != a && !b.excludes(*a)))
    }

    pub(crate) fn add(&mut self, adaptation: Adaptation) {
        debug_assert!(self.fits(adaptation));
        if let Some(slot) = self.taken.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(adaptation);
        }
    }

    /// The highest rank weapon practice with `weapon` can reach.
    pub(crate) fn weapon_ceiling(&self, weapon: Weapon) -> Rank {
        let other = self.held().any(|a| matches!(a, Adaptation::Discipline(w) if w != weapon));
        if other { Rank::Practiced } else { Rank::Expert }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusions_are_symmetric_and_capacity_is_below_the_catalog() {
        for a in Adaptation::ALL {
            for b in Adaptation::ALL {
                assert_eq!(a.excludes(b), b.excludes(a), "{a:?} {b:?}");
            }
        }
        assert!(ADAPTATION_CAPACITY < Adaptation::ALL.len());
    }

    #[test]
    fn fits_respects_room_repeats_and_exclusions() {
        let mut held = Adaptations::default();
        held.add(Adaptation::Discipline(Weapon::Dagger));
        assert!(!held.fits(Adaptation::Discipline(Weapon::Dagger)));
        assert!(!held.fits(Adaptation::Discipline(Weapon::Spear)));
        assert!(held.fits(Adaptation::DeepReserves));
        held.add(Adaptation::DeepReserves);
        assert_eq!(held.free(), 0);
        assert!(!held.fits(Adaptation::VeiledMovement), "no room left");
        assert!(held.consistent());
        assert_eq!(held.weapon_ceiling(Weapon::Dagger), Rank::Expert);
        assert_eq!(held.weapon_ceiling(Weapon::Club), Rank::Practiced);
    }
}

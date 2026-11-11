//! Character-owned practice and the threat ceiling that limits what an individual can teach.

use blah_core::{Rank, Skill};
use serde::{Deserialize, Serialize};

use crate::{Spell, Weapon};

/// One discipline that can improve through useful play.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Practice {
    /// Small blades.
    Dagger,
    /// Short and long swords.
    Blades,
    /// The mace.
    Blunt,
    /// The quarterstaff.
    Staff,
    /// The shortbow and longbow.
    Archery,
    /// Spark.
    Evocation,
    /// Shield.
    Warding,
    /// Heal.
    Restoration,
    /// Stun.
    Binding,
    /// Hiding, unseen movement, openings, and escape.
    Stealth,
    /// Health capacity.
    Vitality,
    /// Focus capacity.
    Focus,
}

impl Practice {
    /// Every discipline, in the order shown by `skills`.
    pub const ALL: [Practice; 12] = [
        Practice::Dagger,
        Practice::Blades,
        Practice::Blunt,
        Practice::Staff,
        Practice::Archery,
        Practice::Evocation,
        Practice::Warding,
        Practice::Restoration,
        Practice::Binding,
        Practice::Stealth,
        Practice::Vitality,
        Practice::Focus,
    ];

    /// The name shown to the player.
    pub const fn name(self) -> &'static str {
        match self {
            Practice::Dagger => "dagger",
            Practice::Blades => "blades",
            Practice::Blunt => "blunt",
            Practice::Staff => "staff",
            Practice::Archery => "archery",
            Practice::Evocation => "evocation",
            Practice::Warding => "warding",
            Practice::Restoration => "restoration",
            Practice::Binding => "binding",
            Practice::Stealth => "stealth",
            Practice::Vitality => "vitality",
            Practice::Focus => "focus",
        }
    }

    /// The discipline trained by a weapon.
    pub const fn weapon(weapon: Weapon) -> Practice {
        match weapon {
            Weapon::Dagger => Practice::Dagger,
            Weapon::ShortSword | Weapon::LongSword => Practice::Blades,
            Weapon::Mace => Practice::Blunt,
            Weapon::Staff => Practice::Staff,
        }
    }

    /// The discipline trained by a spell.
    pub const fn spell(spell: Spell) -> Practice {
        match spell {
            Spell::Spark => Practice::Evocation,
            Spell::Shield => Practice::Warding,
            Spell::Heal => Practice::Restoration,
            Spell::Stun => Practice::Binding,
        }
    }

    const fn index(self) -> usize {
        match self {
            Practice::Dagger => 0,
            Practice::Blades => 1,
            Practice::Blunt => 2,
            Practice::Staff => 3,
            Practice::Archery => 4,
            Practice::Evocation => 5,
            Practice::Warding => 6,
            Practice::Restoration => 7,
            Practice::Binding => 8,
            Practice::Stealth => 9,
            Practice::Vitality => 10,
            Practice::Focus => 11,
        }
    }
}

/// All of a character's earned practice, independent of current equipment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Practices {
    skills: [Skill; Practice::ALL.len()],
}

impl Practices {
    /// The practice rank for one discipline.
    #[must_use]
    pub const fn get(&self, practice: Practice) -> Skill {
        self.skills[practice.index()]
    }

    /// Apply evidence up to the rank taught by this threat. A lower ceiling never erases prior learning.
    pub(crate) fn learn(&mut self, practice: Practice, evidence: u32, threat: u32) -> Option<Rank> {
        let skill = &mut self.skills[practice.index()];
        let before = skill.rank();
        skill.learn(evidence, ceiling(threat));
        let after = skill.rank();
        (after > before).then_some(after)
    }
}

/// Threat below ten is too weak to teach; higher thresholds match the progression design.
pub(crate) const fn ceiling(threat: u32) -> Rank {
    match threat {
        0..10 => Rank::Novice,
        10..25 => Rank::Practiced,
        25..50 => Rank::Skilled,
        50..100 => Rank::Adept,
        _ => Rank::Expert,
    }
}

/// The ordinal rank used for small, bounded effects such as accuracy or capacity.
pub(crate) const fn rank_steps(rank: Rank) -> i32 {
    match rank {
        Rank::Novice => 0,
        Rank::Practiced => 1,
        Rank::Skilled => 2,
        Rank::Adept => 3,
        Rank::Expert => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mappings_and_public_names_are_stable() {
        assert_eq!(Practice::ALL.len(), 12);
        assert_eq!(Practice::weapon(Weapon::Dagger), Practice::Dagger);
        assert_eq!(Practice::weapon(Weapon::ShortSword), Practice::Blades);
        assert_eq!(Practice::weapon(Weapon::LongSword), Practice::Blades);
        assert_eq!(Practice::weapon(Weapon::Mace), Practice::Blunt);
        assert_eq!(Practice::weapon(Weapon::Staff), Practice::Staff);
        assert_eq!(Practice::spell(Spell::Spark), Practice::Evocation);
        assert_eq!(Practice::spell(Spell::Shield), Practice::Warding);
        assert_eq!(Practice::spell(Spell::Heal), Practice::Restoration);
        assert_eq!(Practice::spell(Spell::Stun), Practice::Binding);
        assert_eq!(
            Practice::ALL.map(Practice::name),
            [
                "dagger",
                "blades",
                "blunt",
                "staff",
                "archery",
                "evocation",
                "warding",
                "restoration",
                "binding",
                "stealth",
                "vitality",
                "focus",
            ]
        );
    }

    #[test]
    fn teaching_ceiling_and_threshold_boundaries_are_exact() {
        for (threat, expected) in [
            (0, Rank::Novice),
            (9, Rank::Novice),
            (10, Rank::Practiced),
            (24, Rank::Practiced),
            (25, Rank::Skilled),
            (49, Rank::Skilled),
            (50, Rank::Adept),
            (99, Rank::Adept),
            (100, Rank::Expert),
        ] {
            let mut practices = Practices::default();
            practices.learn(Practice::Dagger, u32::MAX, threat);
            assert_eq!(practices.get(Practice::Dagger).rank(), expected, "threat {threat}");
        }

        let mut skill = Practices::default();
        assert_eq!(skill.learn(Practice::Dagger, 999, 100), None);
        assert_eq!(skill.get(Practice::Dagger).rank(), Rank::Novice);
        assert_eq!(skill.learn(Practice::Dagger, 1, 100), Some(Rank::Practiced));
        skill.learn(Practice::Dagger, u32::MAX, 10);
        assert_eq!(
            skill.get(Practice::Dagger).rank(),
            Rank::Practiced,
            "weak threats cannot erase or grow past their ceiling"
        );
    }
}

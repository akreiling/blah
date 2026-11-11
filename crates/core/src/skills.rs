//! Bounded, doing-based practice. Activities supply earned evidence and a teaching ceiling.

use crate::Weapon;

/// A word-only proficiency band. The thresholds are prototype pacing, not a final progression curve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    /// No established practice yet.
    #[default]
    Novice,
    /// Familiar with the basics.
    Practiced,
    /// Reliable execution.
    Skilled,
    /// Substantial experience.
    Adept,
    /// The current prototype's highest band.
    Expert,
}

impl Rank {
    /// The word shown to the player.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Novice => "novice",
            Self::Practiced => "practiced",
            Self::Skilled => "skilled",
            Self::Adept => "adept",
            Self::Expert => "expert",
        }
    }

    const fn threshold(self) -> u32 {
        match self {
            Self::Novice => 0,
            Self::Practiced => 1_000,
            Self::Skilled => 4_000,
            Self::Adept => 12_000,
            Self::Expert => 30_000,
        }
    }

    /// Weak opponents teach only the basics; harmless ones teach nothing.
    pub(crate) const fn ceiling(threat: u32) -> Self {
        match threat {
            0..10 => Self::Novice,
            10..25 => Self::Practiced,
            25..50 => Self::Skilled,
            50..100 => Self::Adept,
            _ => Self::Expert,
        }
    }
}

/// Accumulated evidence for one proficiency, independent of equipment ownership.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Skill {
    practice: u32,
}

impl Skill {
    /// Current word-only proficiency.
    #[must_use]
    pub const fn rank(self) -> Rank {
        match self.practice {
            0..1_000 => Rank::Novice,
            1_000..4_000 => Rank::Practiced,
            4_000..12_000 => Rank::Skilled,
            12_000..30_000 => Rank::Adept,
            _ => Rank::Expert,
        }
    }

    /// Apply earned evidence without crossing the activity's teaching ceiling.
    /// Returns whether the visible rank increased. A lower ceiling never removes existing practice.
    pub fn learn(&mut self, evidence: u32, ceiling: Rank) -> bool {
        let before = self.rank();
        self.practice = self.practice.max(self.practice.saturating_add(evidence).min(ceiling.threshold()));
        self.rank() > before
    }

    /// The small step bonus shared by weapon damage, opening strikes, and concealment range.
    #[must_use]
    pub const fn bonus(self) -> i32 {
        match self.rank() {
            Rank::Novice => 0,
            Rank::Practiced | Rank::Skilled => 1,
            Rank::Adept | Rank::Expert => 2,
        }
    }

    /// Focus capacity with this Focus practice: two more per rank, from 24 to 32.
    #[must_use]
    pub const fn focus_capacity(self) -> i32 {
        crate::MAX_FOCUS + 2 * self.rank() as i32
    }

    /// How much sneaking shortens an unalerted observer's initial sensing range, from 6 to 8 tiles.
    #[must_use]
    pub const fn sneak_reduction(self) -> i64 {
        crate::SNEAK_REDUCTION + self.bonus() as i64
    }

    /// Every this-many sneaking moves, one costs no Focus. Novices pay for every move.
    #[must_use]
    pub const fn free_sneak_every(self) -> Option<u32> {
        match self.rank() {
            Rank::Novice => None,
            Rank::Practiced => Some(4),
            Rank::Skilled => Some(3),
            Rank::Adept | Rank::Expert => Some(2),
        }
    }

    /// Weapon miss probability, from ten percent down to six percent.
    #[must_use]
    pub const fn miss_chance(self) -> f32 {
        match self.rank() {
            Rank::Novice => crate::MISS_CHANCE,
            Rank::Practiced => 0.09,
            Rank::Skilled => 0.08,
            Rank::Adept => 0.07,
            Rank::Expert => 0.06,
        }
    }
}

/// The practice that shapes one attack: the weapon used and, at an actual opening, stealth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Training {
    /// Practice with the weapon being used.
    pub weapon: Skill,
    /// Stealth practice, which strengthens a strike only at a real unguarded opening.
    pub stealth: Skill,
    /// A committed discipline with the weapon being used.
    pub disciplined: bool,
    /// Veiled movement's cost to free strikes.
    pub veiled: bool,
    /// Concentrated channeling's bonus to released bursts.
    pub concentrated: bool,
}

/// Character-owned practice for the four current weapon families.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponSkills {
    skills: [Skill; 4],
}

impl WeaponSkills {
    const fn index(weapon: Weapon) -> usize {
        match weapon {
            Weapon::Dagger => 0,
            Weapon::Spear => 1,
            Weapon::Club => 2,
            Weapon::Staff => 3,
        }
    }

    /// Practice with this family, including when another weapon is equipped.
    #[must_use]
    pub const fn get(self, weapon: Weapon) -> Skill {
        self.skills[Self::index(weapon)]
    }

    pub(crate) fn learn(&mut self, weapon: Weapon, damage: u32, threat: u32) -> bool {
        self.learn_capped(weapon, damage, threat, Rank::Expert)
    }

    pub(crate) fn learn_capped(&mut self, weapon: Weapon, damage: u32, threat: u32, ceiling: Rank) -> bool {
        self.skills[Self::index(weapon)].learn(damage.saturating_mul(threat), Rank::ceiling(threat).min(ceiling))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn ranks_have_bounded_effects_and_exact_thresholds() {
        for (points, rank, damage, miss) in [
            (0, Rank::Novice, 0, 0.10),
            (1_000, Rank::Practiced, 1, 0.09),
            (4_000, Rank::Skilled, 1, 0.08),
            (12_000, Rank::Adept, 2, 0.07),
            (30_000, Rank::Expert, 2, 0.06),
        ] {
            let mut skill = Skill::default();
            if points > 0 {
                skill.learn(points - 1, Rank::Expert);
                assert!(skill.rank() < rank);
                assert!(skill.learn(1, Rank::Expert));
            }
            assert_eq!(skill.rank(), rank);
            assert_eq!(skill.bonus(), damage);
            assert_eq!(skill.miss_chance(), miss);
        }
    }

    #[test]
    fn threat_caps_learning_without_erasing_harder_lessons() {
        for (threat, rank) in [
            (0, Rank::Novice),
            (9, Rank::Novice),
            (10, Rank::Practiced),
            (24, Rank::Practiced),
            (25, Rank::Skilled),
            (49, Rank::Skilled),
            (50, Rank::Adept),
            (99, Rank::Adept),
            (100, Rank::Expert),
            (200, Rank::Expert),
        ] {
            let mut weapons = WeaponSkills::default();
            weapons.learn(Weapon::Dagger, u32::MAX, threat);
            assert_eq!(weapons.get(Weapon::Dagger).rank(), rank);
            let before = weapons;
            for _ in 0..100 {
                weapons.learn(Weapon::Dagger, u32::MAX, threat);
                weapons.learn(Weapon::Dagger, u32::MAX, 0);
            }
            assert_eq!(weapons, before);
            assert_eq!(weapons.get(Weapon::Spear), Skill::default());
        }
        let mut slow = Skill::default();
        let mut fast = Skill::default();
        slow.learn(10 * 10, Rank::Expert);
        fast.learn(10 * 100, Rank::Expert);
        assert!(slow.practice < fast.practice);
    }

    proptest! {
        #[test]
        fn practice_is_monotone_bounded_and_split_independent(
            initial in any::<u32>(), a in any::<u32>(), b in any::<u32>(), threat in 0u32..=200
        ) {
            let mut skill = Skill::default();
            skill.learn(initial, Rank::Expert);
            let before = skill;
            let mut split = skill;
            let ceiling = Rank::ceiling(threat);
            skill.learn(a.saturating_add(b), ceiling);
            split.learn(a, ceiling);
            split.learn(b, ceiling);
            prop_assert_eq!(skill, split);
            prop_assert!(skill.practice >= before.practice);
            prop_assert!(skill.practice <= Rank::Expert.threshold());
            prop_assert!(skill.practice <= before.practice.max(ceiling.threshold()));
            let unchanged = skill;
            prop_assert!(!skill.learn(0, Rank::Expert));
            prop_assert_eq!(skill, unchanged);
        }
    }
}

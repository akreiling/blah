//! Small earned actions. Availability is derived from practice, never a second unlock ledger.

use crate::{Wanderer, Weapon};

/// The first active choices earned at practiced rank.
/// These actions leave the prepared composition unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LearnedMove {
    /// Dagger strike followed by one step directly away, even on a miss.
    Withdraw,
    /// Spear strike exactly two tiles away through an empty, passable tile.
    Reach,
    /// Club strike that pushes a surviving target on a hit, when there is room.
    Shove,
    /// One staff swing at every cardinally adjacent target.
    Sweep,
    /// Two quiet steps in one turn, without magically erasing tracking.
    SlipAway,
    /// A free adjacent magic pulse: one damage on a hit, without weapon or Focus practice.
    Pulse,
}

impl LearnedMove {
    /// Active move for the equipped weapon family.
    #[must_use]
    pub const fn for_weapon(weapon: Weapon) -> Self {
        match weapon {
            Weapon::Dagger => Self::Withdraw,
            Weapon::Spear => Self::Reach,
            Weapon::Club => Self::Shove,
            Weapon::Staff => Self::Sweep,
        }
    }

    /// Player-facing name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Withdraw => "Withdraw",
            Self::Reach => "Reach",
            Self::Shove => "Shove",
            Self::Sweep => "Sweep",
            Self::SlipAway => "Slip away",
            Self::Pulse => "Pulse",
        }
    }

    /// Focus paid once per accepted action, including a miss.
    #[must_use]
    pub const fn focus_cost(self) -> i32 {
        match self {
            Self::Withdraw | Self::Shove => 3,
            Self::Reach => 2,
            Self::Sweep | Self::SlipAway => 4,
            Self::Pulse => 0,
        }
    }

    /// Required proficiency, displayed even before the action is earned.
    #[must_use]
    pub const fn requirement(self) -> &'static str {
        match self {
            Self::Withdraw => "practiced dagger",
            Self::Reach => "practiced spear",
            Self::Shove => "practiced club",
            Self::Sweep => "practiced staff",
            Self::SlipAway => "practiced stealth",
            Self::Pulse => "practiced Focus",
        }
    }

    /// Which family must be equipped, if this is a weapon move.
    #[must_use]
    pub const fn weapon(self) -> Option<Weapon> {
        match self {
            Self::Withdraw => Some(Weapon::Dagger),
            Self::Reach => Some(Weapon::Spear),
            Self::Shove => Some(Weapon::Club),
            Self::Sweep => Some(Weapon::Staff),
            Self::SlipAway | Self::Pulse => None,
        }
    }

    /// Whether character-owned practice has earned the action, independently of equipment.
    #[must_use]
    pub fn unlocked(self, wanderer: &Wanderer) -> bool {
        let skill = match self.weapon() {
            Some(weapon) => wanderer.weapons.get(weapon),
            None if self == Self::SlipAway => wanderer.stealth,
            None => wanderer.focus_skill,
        };
        skill.rank() != crate::Rank::Novice
    }
}

/// Immediate learned-action effects before misses, creature reactions, or regeneration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovePreview {
    /// Focus paid once, not per target or tile.
    pub focus: i32,
    /// Number of creatures struck.
    pub targets: usize,
    /// Sum of minimum damage if every strike lands. Misses can reduce this to zero.
    pub min_hit: i32,
    /// Sum of maximum damage if every strike lands.
    pub max_hit: i32,
    /// Where the wanderer moves before retaliation, if applicable.
    pub destination: Option<(i64, i64)>,
    /// Where a surviving target is pushed on a hit, if applicable.
    pub push: Option<(i64, i64)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rank;

    #[test]
    fn first_rank_unlocks_only_its_own_actions_without_an_unlock_ledger() {
        let actions = [
            LearnedMove::Withdraw,
            LearnedMove::Reach,
            LearnedMove::Shove,
            LearnedMove::Sweep,
            LearnedMove::SlipAway,
            LearnedMove::Pulse,
        ];
        let mut w = Wanderer::new();
        assert!(actions.into_iter().all(|m| !m.unlocked(&w)));
        for weapon in [Weapon::Dagger, Weapon::Spear, Weapon::Club, Weapon::Staff] {
            let mut trained = w;
            trained.weapons.learn(weapon, 10, 100);
            for action in actions {
                assert_eq!(action.unlocked(&trained), action == LearnedMove::for_weapon(weapon));
            }
        }
        w.focus_skill.learn(1_000, Rank::Expert);
        assert!(LearnedMove::Pulse.unlocked(&w));
        assert!(!LearnedMove::SlipAway.unlocked(&w));
        w.stealth.learn(1_000, Rank::Expert);
        assert!(LearnedMove::SlipAway.unlocked(&w));
    }
}

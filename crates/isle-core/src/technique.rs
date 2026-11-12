//! Weapon techniques that skilled practice unlocks. They fire on their own when the moment comes and
//! Focus allows, and never against a foe too weak to teach anything; none is a command to remember.

use blah_core::Rank;
use serde::{Deserialize, Serialize};

use crate::{Practice, Weapon};

/// The rank at which a weapon family's technique unlocks.
pub const TECHNIQUE_RANK: Rank = Rank::Skilled;

/// One automatic technique per weapon family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Technique {
    /// Dagger: a missed thrust draws the target's guard.
    Feint,
    /// Blades: turn one close blow a round in single combat, then riposte.
    Parry,
    /// Blunt: a solid hit costs the target its next action.
    Stagger,
    /// Staff: each attack carries on to a second creature beside you.
    Sweep,
    /// Archery: unhurried shots from a distance fly truer.
    SteadyShot,
}

impl Technique {
    /// Every technique, in skills order.
    pub const ALL: [Self; 5] = [Self::Feint, Self::Parry, Self::Stagger, Self::Sweep, Self::SteadyShot];

    /// Its name in `skills` and the log.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Feint => "feint",
            Self::Parry => "parry",
            Self::Stagger => "stagger",
            Self::Sweep => "sweep",
            Self::SteadyShot => "steady shot",
        }
    }

    /// The practice that unlocks it.
    pub const fn practice(self) -> Practice {
        match self {
            Self::Feint => Practice::Dagger,
            Self::Parry => Practice::Blades,
            Self::Stagger => Practice::Blunt,
            Self::Sweep => Practice::Staff,
            Self::SteadyShot => Practice::Archery,
        }
    }

    /// The technique a practice unlocks, if any.
    pub fn of(practice: Practice) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.practice() == practice)
    }

    /// Whether `weapon` in hand performs it. Steady shot needs only the bow.
    pub(crate) const fn suits(self, weapon: Weapon) -> bool {
        match self {
            Self::Feint => matches!(weapon, Weapon::Dagger),
            Self::Parry => matches!(weapon, Weapon::ShortSword | Weapon::LongSword | Weapon::Oathfang),
            Self::Stagger => matches!(weapon, Weapon::Mace),
            Self::Sweep => matches!(weapon, Weapon::Staff),
            Self::SteadyShot => true,
        }
    }

    /// Focus spent each time it fires.
    pub const fn cost(self) -> i32 {
        match self {
            Self::SteadyShot => 1,
            _ => 2,
        }
    }

    /// When it fires and what it does.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Feint => "a missed thrust gives your next attack on that foe +4 to hit and double damage",
            Self::Parry => {
                "once a round in single combat, turns a blow that beats your armor by under 4; next attack ripostes (+4 hit, +2 damage)"
            }
            Self::Stagger => "a hit 5 or more over the foe's armor costs it its next action",
            Self::Sweep => "each attack also swings at another foe beside you (-2 damage)",
            Self::SteadyShot => "+2 to hit and damage at 3 or more cells with nothing hunting you beside",
        }
    }
}

/// An advantage a technique left for the very next command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Readied {
    /// A parried blow: the next attack is a riposte.
    Riposte,
    /// A missed dagger thrust drew this creature's guard.
    Feint(u32),
}

impl Readied {
    /// The short panel state.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Riposte => "riposte ready",
            Self::Feint(_) => "guard drawn",
        }
    }
}

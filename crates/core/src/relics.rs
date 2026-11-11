//! A small set of relics, each changing one bounded rule while attuned.

/// The number of relics that can be attuned at once.
pub const RELIC_SLOTS: usize = 2;

/// A unique relic found at a shrine or guarded ruin.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Relic {
    /// Camps restore two additional Health.
    HearthEmber,
    /// Entering careful movement costs no Focus.
    Hushstone,
    /// Springs recover after half their usual cooldown.
    SpringVial,
    /// Waking after death restores three Health instead of one.
    ReturningLantern,
}

impl Relic {
    /// Every relic, in stable inventory and reward order.
    pub const ALL: [Self; 4] = [Self::HearthEmber, Self::Hushstone, Self::SpringVial, Self::ReturningLantern];

    /// The relic's display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::HearthEmber => "Hearth ember",
            Self::Hushstone => "Hushstone",
            Self::SpringVial => "Spring vial",
            Self::ReturningLantern => "Returning lantern",
        }
    }

    /// The one bounded rule this relic changes while attuned.
    #[must_use]
    pub const fn effect(self) -> &'static str {
        match self {
            Self::HearthEmber => "Camps restore +2 Health.",
            Self::Hushstone => "Entering sneak costs no Focus.",
            Self::SpringVial => "Springs recover in 150 turns, not 300.",
            Self::ReturningLantern => "Death wakes you with 3 Health, not 1.",
        }
    }
}

/// Additional Health restored by an attuned Hearth ember at camp.
pub const HEARTH_EMBER_HEAL: i32 = 2;
/// Health restored by death when the Returning lantern is attuned.
pub const RETURNING_LANTERN_HEALTH: i32 = 3;

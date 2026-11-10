//! A small composition grammar, not a catalog of named combos.
//!
//! The prototype supports weapon, touch, or ground delivery, direct or opening-seeking aim, and force or
//! fire bursts on contact or penetration. Names and advisory reviews never enter resolution. All coefficients
//! below are prototype rules; no skill learning, anatomical simulation, or remote evaluator is implied.

use crate::{MISS_CHANCE, Rng};

/// Largest supported burst investment in this prototype.
pub const MAX_BURST_POWER: u8 = 6;
/// Largest square area radius admitted by the prototype.
pub const MAX_AREA_RADIUS: u8 = 3;
/// Focus available on a fresh expedition.
pub const MAX_FOCUS: i32 = 24;
/// Reduction in initial sensing range while sneaking. Tracking an already located actor is unaffected.
pub const SNEAK_REDUCTION: i64 = 6;

/// Prototype equipment catalog. Compositions use capabilities, not these names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Weapon {
    /// A precise, piercing weapon with no added base force.
    #[default]
    Dagger,
    /// A stronger piercing weapon with less precise placement.
    Spear,
    /// A forceful, non-piercing weapon.
    Club,
    /// A blunt channeling instrument that can strike the ground.
    Staff,
}

impl Weapon {
    /// The equipment's display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dagger => "Dagger",
            Self::Spear => "Spear",
            Self::Club => "Club",
            Self::Staff => "Staff",
        }
    }

    /// Next piece of developer equipment.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Dagger => Self::Spear,
            Self::Spear => Self::Club,
            Self::Club => Self::Staff,
            Self::Staff => Self::Dagger,
        }
    }

    /// Whether a strike can establish a penetrating delivery.
    #[must_use]
    pub const fn pierces(self) -> bool {
        matches!(self, Self::Dagger | Self::Spear)
    }

    /// Whether this instrument can deliver a ground impact.
    #[must_use]
    pub const fn grounds(self) -> bool {
        !matches!(self, Self::Dagger)
    }

    const fn force(self) -> i32 {
        match self {
            Self::Dagger => 0,
            Self::Spear => 2,
            Self::Club => 3,
            Self::Staff => 1,
        }
    }

    const fn precision(self) -> i32 {
        match self {
            Self::Dagger => 3,
            Self::Spear => 1,
            Self::Club | Self::Staff => 0,
        }
    }
}

/// How an effect reaches an adjacent target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// Strike using the equipped weapon.
    Weapon,
    /// Touch without a physical strike.
    Touch,
    /// Strike the ground to release a caster-centered area payload.
    Ground,
}

/// How the actor directs the delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aim {
    /// An ordinary attack.
    Direct,
    /// Spend attention exploiting an unguarded opening, if one actually exists.
    Opening,
}

/// The resolved event that releases an attached payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Release on a successful contact, including a glancing strike.
    Contact,
    /// Release only after a piercing delivery overcomes defense.
    Penetration,
}

/// What an attached burst does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    /// Mechanical force; surface delivery contends with armor.
    Force,
    /// Heat; the creature's heat tolerance reduces damage instead.
    Fire,
}

/// The spatial extent of a payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// One contacted target.
    Point,
    /// Everyone in a square around the caster, including the caster. No faction filtering.
    Area(u8),
}

/// A bounded magical payload. This is the example's thaumaturgy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Burst {
    /// Investment from one through [`MAX_BURST_POWER`].
    pub power: u8,
    /// When the prepared energy is released.
    pub trigger: Trigger,
    /// Which material interaction to resolve.
    pub element: Element,
    /// Which supported locations receive the payload.
    pub shape: Shape,
}

impl Burst {
    /// Local damage under the payload's laws, shared by contact attacks, area previews, and execution.
    ///
    /// Fire resistance is a percentage, capped at 75 in this prototype. Force on the surface meets armor;
    /// an internal release does not. Opening amplification is possible only at a precise penetration.
    #[must_use]
    pub fn damage(self, defense: i32, fire_resistance: u8, internal: bool, amplified: bool) -> i32 {
        let power = i32::from(self.power) * 2 * if amplified { 2 } else { 1 };
        match self.element {
            Element::Force if !internal => (power - defense.max(0) / 2).max(0),
            Element::Force => power,
            Element::Fire => power * i32::from(100 - fire_resistance.min(75)) / 100,
        }
    }
}

/// Executable structure. It deliberately has no name or model-supplied damage number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Technique {
    /// The delivery mechanism.
    pub delivery: Delivery,
    /// The practiced placement being attempted.
    pub aim: Aim,
    /// An optional magical payload.
    pub burst: Option<Burst>,
}

impl Default for Technique {
    fn default() -> Self {
        Self { delivery: Delivery::Weapon, aim: Aim::Direct, burst: None }
    }
}

impl Technique {
    /// Structural legality, independent of any particular target.
    ///
    /// Nothing checks whether the exact combination appears in a recipe list.
    pub fn validate(self, weapon: Weapon) -> Result<(), &'static str> {
        if self.delivery == Delivery::Touch && self.burst.is_none() {
            return Err("Touch needs a payload.");
        }
        if self.delivery == Delivery::Ground {
            if !weapon.grounds() {
                return Err("Ground delivery needs an impact-capable instrument.");
            }
            if self.aim != Aim::Direct {
                return Err("A ground area cannot target a precise opening.");
            }
            if !matches!(self.burst, Some(Burst { shape: Shape::Area(_), .. })) {
                return Err("Ground delivery needs an area payload.");
            }
        }
        if let Some(burst) = self.burst {
            if !(1..=MAX_BURST_POWER).contains(&burst.power) {
                return Err("Burst power must be between 1 and 6.");
            }
            if burst.trigger == Trigger::Penetration && (self.delivery != Delivery::Weapon || !weapon.pierces()) {
                return Err("Penetration needs a piercing weapon delivery.");
            }
            if let Shape::Area(radius) = burst.shape {
                if !(1..=MAX_AREA_RADIUS).contains(&radius) {
                    return Err("Area radius must be between 1 and 3.");
                }
                if self.delivery != Delivery::Ground {
                    return Err("Area payloads need ground delivery.");
                }
            }
        }
        Ok(())
    }

    /// Focus paid before an attempted execution, even if it misses or its trigger fails.
    #[must_use]
    pub fn focus_cost(self) -> i32 {
        i32::from(self.aim == Aim::Opening)
            + self.burst.map_or(0, |b| match b.shape {
                Shape::Point => 2 + i32::from(b.power) + i32::from(b.trigger == Trigger::Penetration),
                Shape::Area(radius) => 2 + 2 * i32::from(b.power) + i32::from(radius) * i32::from(radius),
            })
    }
}

/// A player's named technique and the equipment prepared to execute it.
///
/// An AI can propose a name and a reviewer can discuss this structure, but neither owns its behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Maneuver {
    /// Cosmetic name, at most 48 characters and without control characters.
    pub name: String,
    /// Equipment for the prototype, not a separate inventory system.
    pub weapon: Weapon,
    /// The authoritative mechanical definition.
    pub technique: Technique,
}

impl Default for Maneuver {
    fn default() -> Self {
        Self { name: "Strike".into(), weapon: Weapon::Dagger, technique: Technique::default() }
    }
}

impl Maneuver {
    /// A free ordinary strike with the same equipped weapon, without aim or payload.
    #[must_use]
    pub fn basic_strike(&self) -> Self {
        Self { name: "Strike".into(), weapon: self.weapon, technique: Technique::default() }
    }

    /// The requested example, expressed using exactly the same grammar as any other combination.
    #[must_use]
    pub fn backstab_burst() -> Self {
        Self {
            name: "Backstab Burst Spine".into(),
            weapon: Weapon::Dagger,
            technique: Technique {
                delivery: Delivery::Weapon,
                aim: Aim::Opening,
                burst: Some(Burst {
                    power: 3,
                    trigger: Trigger::Penetration,
                    element: Element::Force,
                    shape: Shape::Point,
                }),
            },
        }
    }

    /// A staff-grounded fire burst. Its name grants no exception to the area or self-hit rules.
    #[must_use]
    pub fn staff_inferno() -> Self {
        Self {
            name: "Cinder Reckoning".into(),
            weapon: Weapon::Staff,
            technique: Technique {
                delivery: Delivery::Ground,
                aim: Aim::Direct,
                burst: Some(Burst {
                    power: 6,
                    trigger: Trigger::Contact,
                    element: Element::Fire,
                    shape: Shape::Area(2),
                }),
            },
        }
    }

    /// Validated area shape and payload, or a reason this maneuver cannot be discharged into the ground.
    pub fn area_payload(&self) -> Result<(u8, Burst), &'static str> {
        self.validate()?;
        match self.technique.burst {
            Some(burst @ Burst { shape: Shape::Area(radius), .. }) => Ok((radius, burst)),
            _ => Err("Prepare a ground-area maneuver before discharging."),
        }
    }

    /// Guaranteed damage to the unprotected caster, for a draft's safety preview.
    pub fn backlash(&self) -> Result<i32, &'static str> {
        let (_, burst) = self.area_payload()?;
        Ok(burst.damage(0, 0, false, false))
    }

    /// Validate metadata and executable structure before preparation or resolution.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.name.trim().is_empty() || self.name.chars().count() > 48 || self.name.chars().any(char::is_control) {
            return Err("Use a nonempty name of at most 48 characters, without control characters.");
        }
        self.technique.validate(self.weapon)
    }

    /// Resolve one paid attempt. The caller owns turn advancement, resources, and applying this result.
    pub fn resolve(&self, context: AttackContext, rng: &mut Rng) -> Result<AttackResult, &'static str> {
        self.validate()?;
        if self.technique.delivery == Delivery::Ground {
            return Err("Ground areas use area resolution, not a single-target attack.");
        }
        if rng.chance(MISS_CHANCE) {
            return Ok(AttackResult::default());
        }
        Ok(self.on_hit(context, rng.below(3) as i32 - 1))
    }

    /// Exact range across successful-hit rolls; a miss can still do zero damage.
    pub fn preview(&self, context: AttackContext) -> Result<AttackPreview, &'static str> {
        self.validate()?;
        if self.technique.delivery == Delivery::Ground {
            return Err("Use the area preview for a ground delivery.");
        }
        let low = self.on_hit(context, -1);
        let high = self.on_hit(context, 1);
        Ok(AttackPreview {
            focus: self.technique.focus_cost(),
            min_hit: low.total(),
            max_hit: high.total(),
            opening: high.opening,
            penetration_possible: high.penetrated,
        })
    }

    fn on_hit(&self, context: AttackContext, roll: i32) -> AttackResult {
        let weapon_contact = self.technique.delivery == Delivery::Weapon;
        let precision = if weapon_contact { self.weapon.precision() } else { 0 };
        let opening = self.technique.aim == Aim::Opening && context.unguarded && precision > 0;
        let defense = context.defense.max(0).saturating_sub(if opening { precision } else { 0 }).max(0);
        let impact = context.attack.saturating_add(self.weapon.force()).saturating_sub(defense).saturating_add(roll);
        let penetrated = weapon_contact && self.weapon.pierces() && impact > 0;
        let physical = if weapon_contact { impact.max(1) } else { 0 };
        let mut result = AttackResult { hit: true, physical, penetrated, opening, ..AttackResult::default() };
        if let Some(burst) = self.technique.burst {
            result.burst_released = burst.trigger == Trigger::Contact || penetrated;
            if result.burst_released {
                // Concentration at an unguarded penetrating contact increases local effect, not the
                // character's generic magic score. No weapon or maneuver name is involved.
                result.amplified = opening && burst.trigger == Trigger::Penetration;
                result.burst = burst.damage(
                    context.defense,
                    context.fire_resistance,
                    burst.trigger == Trigger::Penetration,
                    result.amplified,
                );
            }
        }
        result
    }
}

/// Authoritative facts supplied by the current encounter, not by the composition author.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttackContext {
    /// The actor's base attack.
    pub attack: i32,
    /// The target's defense.
    pub defense: i32,
    /// Whether this target has an actual unguarded opening.
    pub unguarded: bool,
    /// Target-specific heat protection derived from its phenotype, not supplied by the technique author.
    pub fire_resistance: u8,
}

/// The exact immediate consequences of an area discharge, before enemy retaliation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AreaPreview {
    /// Square radius around the caster.
    pub radius: u8,
    /// Focus paid once for the discharge.
    pub focus: i32,
    /// Creatures reached, excluding the caster.
    pub targets: usize,
    /// Sum of immediate damage to those creatures, before limiting to their remaining Health.
    pub damage: i32,
    /// Damage to the caster. A discharge can be lethal.
    pub self_damage: i32,
}

/// A resolved attempt and the provenance of its effects. These are not yet learning rewards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AttackResult {
    /// Whether the delivery contacted the target.
    pub hit: bool,
    /// Physical damage, including a minimum-one glancing weapon hit.
    pub physical: i32,
    /// Magical damage.
    pub burst: i32,
    /// Whether precise placement exploited an actual opening.
    pub opening: bool,
    /// Whether the weapon overcame defense.
    pub penetrated: bool,
    /// Whether the payload's trigger fired.
    pub burst_released: bool,
    /// Whether concentration at the opening amplified the payload.
    pub amplified: bool,
}

impl AttackResult {
    /// Combined damage, without overflow for extreme caller-supplied stats.
    #[must_use]
    pub const fn total(self) -> i32 {
        self.physical.saturating_add(self.burst)
    }
}

/// A nonmutating, nonrandom query using the same rules as execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttackPreview {
    /// Focus required to attempt the maneuver.
    pub focus: i32,
    /// Lowest damage on a successful hit; a miss does zero.
    pub min_hit: i32,
    /// Highest damage on a successful hit.
    pub max_hit: i32,
    /// Whether precise placement can exploit an opening now.
    pub opening: bool,
    /// Whether any hit roll can penetrate.
    pub penetration_possible: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUARDED: AttackContext = AttackContext { attack: 4, defense: 3, unguarded: false, fire_resistance: 0 };

    #[test]
    fn three_parts_amplify_through_placement_not_recipe_names() {
        let combined = Maneuver::backstab_burst();
        let context = AttackContext { unguarded: true, ..GUARDED };
        let result = combined.on_hit(context, 0);
        // Independent arithmetic: 4 - max(3 - 3, 0) physical, plus (3 * 2) * 2 placed burst.
        assert_eq!((result.physical, result.burst, result.total()), (4, 12, 16));
        assert!(result.opening && result.penetrated && result.amplified);
        assert_eq!(combined.on_hit(GUARDED, 0).total(), 7);
        let mut dagger = combined.clone();
        dagger.technique.burst = None;
        let touch = Maneuver {
            name: "Palm".into(),
            technique: Technique {
                delivery: Delivery::Touch,
                aim: Aim::Direct,
                burst: Some(Burst {
                    power: 3,
                    trigger: Trigger::Contact,
                    element: Element::Force,
                    shape: Shape::Point,
                }),
            },
            ..Maneuver::default()
        };
        assert_eq!(dagger.on_hit(context, 0).total(), 4);
        assert_eq!(touch.on_hit(context, 0).total(), 5);
        assert!(result.total() > dagger.on_hit(context, 0).total() + touch.on_hit(context, 0).total());
    }

    #[test]
    fn names_do_not_change_rules_and_unlisted_equipment_composes() {
        let mut original = Maneuver::backstab_burst();
        original.weapon = Weapon::Spear;
        let mut renamed = original.clone();
        renamed.name = "An unlisted player's idea".into();
        for seed in 0..100 {
            assert_eq!(original.resolve(GUARDED, &mut Rng::new(seed)), renamed.resolve(GUARDED, &mut Rng::new(seed)));
        }
        let hit = renamed.on_hit(AttackContext { unguarded: true, ..GUARDED }, 0);
        assert_eq!((hit.physical, hit.burst), (4, 12));
        assert!(hit.amplified);
    }

    #[test]
    fn blocking_and_misses_do_not_trigger_penetration_payloads() {
        let maneuver = Maneuver::backstab_burst();
        let blocked = AttackContext { defense: 100, ..GUARDED };
        let hit = maneuver.on_hit(blocked, 0);
        assert_eq!((hit.physical, hit.burst), (1, 0));
        assert!(!hit.penetrated && !hit.burst_released && !hit.amplified);
        let miss = (0..100).map(|s| maneuver.resolve(GUARDED, &mut Rng::new(s)).unwrap()).find(|r| !r.hit).unwrap();
        assert_eq!(miss, AttackResult::default());
    }

    #[test]
    fn generic_contact_payload_works_on_a_nonpiercing_weapon() {
        let mut maneuver = Maneuver::backstab_burst();
        maneuver.weapon = Weapon::Club;
        assert!(maneuver.validate().is_err());
        maneuver.technique.burst.as_mut().unwrap().trigger = Trigger::Contact;
        assert!(maneuver.validate().is_ok());
        let hit = maneuver.on_hit(AttackContext { unguarded: true, ..GUARDED }, 0);
        assert_eq!((hit.physical, hit.burst), (4, 5));
        assert!(!hit.penetrated && !hit.amplified);
    }

    #[test]
    fn all_supported_combinations_have_matching_previews_and_bounded_effects() {
        let mut accepted = 0;
        for weapon in [Weapon::Dagger, Weapon::Spear, Weapon::Club] {
            for delivery in [Delivery::Weapon, Delivery::Touch] {
                for aim in [Aim::Direct, Aim::Opening] {
                    for trigger in [Trigger::Contact, Trigger::Penetration] {
                        for power in [0, 1, MAX_BURST_POWER, MAX_BURST_POWER + 1, u8::MAX] {
                            let m = Maneuver {
                                name: "Generated structure".into(),
                                weapon,
                                technique: Technique {
                                    delivery,
                                    aim,
                                    burst: Some(Burst { power, trigger, element: Element::Force, shape: Shape::Point }),
                                },
                            };
                            let valid = (1..=6).contains(&power)
                                && (trigger == Trigger::Contact || delivery == Delivery::Weapon && weapon.pierces());
                            assert_eq!(m.validate().is_ok(), valid);
                            if !valid {
                                assert!(m.resolve(GUARDED, &mut Rng::new(0)).is_err());
                                continue;
                            }
                            accepted += 1;
                            for defense in [0, 3, 100] {
                                for unguarded in [false, true] {
                                    let context = AttackContext { attack: 4, defense, unguarded, fire_resistance: 0 };
                                    let preview = m.preview(context).unwrap();
                                    for seed in 0..40 {
                                        let result = m.resolve(context, &mut Rng::new(seed)).unwrap();
                                        assert!(
                                            !result.hit
                                                || (preview.min_hit..=preview.max_hit).contains(&result.total())
                                        );
                                        assert!((0..=24).contains(&result.burst));
                                        assert!(result.total() <= 32);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(accepted, 32);
    }

    #[test]
    fn empty_delivery_and_invalid_metadata_are_rejected() {
        let mut m = Maneuver::default();
        m.technique.delivery = Delivery::Touch;
        assert!(m.validate().is_err());
        m.technique = Technique::default();
        for name in ["", "   ", "\n", "bad\tname"] {
            m.name = name.into();
            assert!(m.validate().is_err());
        }
        m.name = "x".repeat(49);
        assert!(m.validate().is_err());
    }

    #[test]
    fn basic_weapon_preserves_existing_damage_and_random_stream() {
        let m = Maneuver::default();
        for seed in 0..200 {
            let mut a = Rng::new(seed);
            let mut b = a.clone();
            let expected = crate::damage(4, 3, &mut a);
            let result = m.resolve(GUARDED, &mut b).unwrap();
            assert_eq!(result.hit.then_some(result.total()), expected);
            assert_eq!(a.below(1000), b.below(1000));
        }
    }

    #[test]
    fn fire_and_force_obey_different_defenses() {
        let force = Burst { power: 6, trigger: Trigger::Contact, element: Element::Force, shape: Shape::Point };
        let fire = Burst { element: Element::Fire, ..force };
        assert_eq!(force.damage(10, 75, false, false), 7);
        assert_eq!(fire.damage(10, 75, false, false), 3);
        assert_eq!(fire.damage(100, 0, false, false), 12);
        assert_eq!(fire.damage(0, 50, true, true), 12);
    }

    #[test]
    fn ground_shapes_are_bounded_generic_and_cannot_use_point_resolution() {
        for weapon in [Weapon::Dagger, Weapon::Spear, Weapon::Club, Weapon::Staff] {
            for aim in [Aim::Direct, Aim::Opening] {
                for element in [Element::Force, Element::Fire] {
                    for trigger in [Trigger::Contact, Trigger::Penetration] {
                        for radius in [0, 1, 2, MAX_AREA_RADIUS, MAX_AREA_RADIUS + 1, u8::MAX] {
                            let m = Maneuver {
                                name: "Not an authored recipe".into(),
                                weapon,
                                technique: Technique {
                                    delivery: Delivery::Ground,
                                    aim,
                                    burst: Some(Burst { power: 6, trigger, element, shape: Shape::Area(radius) }),
                                },
                            };
                            let valid = weapon != Weapon::Dagger
                                && aim == Aim::Direct
                                && trigger == Trigger::Contact
                                && (1..=3).contains(&radius);
                            assert_eq!(m.validate().is_ok(), valid);
                            assert!(m.preview(GUARDED).is_err());
                            assert!(m.resolve(GUARDED, &mut Rng::new(0)).is_err());
                            if valid {
                                assert!(m.technique.focus_cost() <= MAX_FOCUS);
                                assert_eq!(m.backlash(), Ok(12));
                            }
                        }
                    }
                }
            }
        }
        let staff = Maneuver::staff_inferno();
        assert_eq!(staff.technique.focus_cost(), 18);
        assert_eq!(staff.backlash(), Ok(12));
        let mut point_area = staff.clone();
        point_area.technique.delivery = Delivery::Touch;
        assert!(point_area.validate().is_err());
        let mut empty_ground = staff;
        empty_ground.technique.burst = None;
        assert!(empty_ground.validate().is_err());
    }
}

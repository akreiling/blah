//! The keyboard-first, local composition draft used by the GPUI workshop.
//!
//! Draft changes are kept separate from the live game loadout until `prepare` succeeds. This is only
//! an editor for the deterministic core grammar; it does not call an AI service or invent rule effects.

use blah_core::{Aim, Burst, Delivery, Element, Game, MAX_AREA_RADIUS, MAX_BURST_POWER, Maneuver, Shape, Trigger};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Edit {
    Weapon,
    Delivery,
    Aim,
    Payload,
    Element,
    Trigger,
    Shape,
    PowerUp,
    PowerDown,
    DaggerExample,
    StaffExample,
}

/// The unprepared maneuver and the latest preparation error, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Workshop {
    draft: Maneuver,
    prepare_error: Option<&'static str>,
}

impl Workshop {
    pub(crate) fn new(maneuver: Maneuver) -> Self {
        Self { draft: maneuver, prepare_error: None }
    }

    pub(crate) fn draft(&self) -> &Maneuver {
        &self.draft
    }

    /// The authoritative structural reason, or a safe-preparation error from the last Enter.
    pub(crate) fn reason(&self) -> Option<&'static str> {
        self.draft.validate().err().or(self.prepare_error)
    }

    pub(crate) fn apply(&mut self, edit: Edit) {
        if matches!(edit, Edit::DaggerExample | Edit::StaffExample) {
            self.draft = match edit {
                Edit::DaggerExample => Maneuver::backstab_burst(),
                Edit::StaffExample => Maneuver::staff_inferno(),
                _ => unreachable!(),
            };
            self.prepare_error = None;
            return;
        }

        if matches!(edit, Edit::Element | Edit::Trigger | Edit::Shape | Edit::PowerUp | Edit::PowerDown)
            && self.draft.technique.burst.is_none()
        {
            return;
        }
        self.prepare_error = None;
        self.draft.name = "Custom maneuver".into();
        let maneuver = &mut self.draft;
        let technique = &mut maneuver.technique;
        match edit {
            Edit::Weapon => maneuver.weapon = maneuver.weapon.next(),
            Edit::Delivery => {
                technique.delivery = match technique.delivery {
                    Delivery::Weapon => Delivery::Touch,
                    Delivery::Touch => Delivery::Ground,
                    Delivery::Ground => Delivery::Weapon,
                };
            }
            Edit::Aim => {
                technique.aim = match technique.aim {
                    Aim::Direct => Aim::Opening,
                    Aim::Opening => Aim::Direct,
                };
            }
            Edit::Payload => {
                technique.burst = if technique.burst.is_some() {
                    None
                } else {
                    Some(Burst { power: 3, trigger: Trigger::Contact, element: Element::Force, shape: Shape::Point })
                };
            }
            Edit::Element => {
                if let Some(burst) = &mut technique.burst {
                    burst.element = match burst.element {
                        Element::Force => Element::Fire,
                        Element::Fire => Element::Force,
                    };
                }
            }
            Edit::Trigger => {
                if let Some(burst) = &mut technique.burst {
                    burst.trigger = match burst.trigger {
                        Trigger::Contact => Trigger::Penetration,
                        Trigger::Penetration => Trigger::Contact,
                    };
                }
            }
            Edit::Shape => {
                if let Some(burst) = &mut technique.burst {
                    burst.shape = match burst.shape {
                        Shape::Point => Shape::Area(1),
                        Shape::Area(1) => Shape::Area(2),
                        Shape::Area(2) => Shape::Area(MAX_AREA_RADIUS),
                        Shape::Area(_) => Shape::Point,
                    };
                }
            }
            Edit::PowerUp => {
                if let Some(burst) = &mut technique.burst {
                    burst.power = burst.power.saturating_add(1).min(MAX_BURST_POWER);
                }
            }
            Edit::PowerDown => {
                if let Some(burst) = &mut technique.burst {
                    burst.power = burst.power.saturating_sub(1).max(1);
                }
            }
            Edit::DaggerExample | Edit::StaffExample => unreachable!(),
        }
    }

    /// Uses the core as the sole authority for validation and preparation.
    pub(crate) fn prepare(&mut self, game: &mut Game) -> Result<(), &'static str> {
        match game.prepare(self.draft.clone()) {
            Ok(()) => {
                self.prepare_error = None;
                Ok(())
            }
            Err(reason) => {
                self.prepare_error = Some(reason);
                Err(reason)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_edits_and_cancellation_leave_the_live_game_untouched() {
        let game = Game::new(42);
        let prepared = game.maneuver().clone();
        let turn = game.turn();
        let mut workshop = Workshop::new(prepared.clone());

        workshop.apply(Edit::Payload);
        workshop.apply(Edit::Delivery);
        assert_ne!(workshop.draft(), &prepared);
        drop(workshop);

        assert_eq!(game.maneuver(), &prepared);
        assert_eq!(game.turn(), turn);
    }

    #[test]
    fn invalid_drafts_show_the_core_reason_and_cannot_be_prepared() {
        let mut game = Game::new(42);
        let prepared = game.maneuver().clone();
        let turn = game.turn();
        let mut workshop = Workshop::new(prepared.clone());
        workshop.apply(Edit::Delivery); // Touch without a payload is intentionally an invalid draft.

        let reason = workshop.draft().validate().unwrap_err();
        assert_eq!(workshop.reason(), Some(reason));
        assert_eq!(workshop.prepare(&mut game), Err(reason));
        assert_eq!(workshop.reason(), Some(reason));
        assert_eq!(game.maneuver(), &prepared);
        assert_eq!(game.turn(), turn);
    }

    #[test]
    fn valid_drafts_prepare_without_advancing_time() {
        let mut game = Game::new(42);
        assert!(game.can_prepare(), "fresh-game seed should begin at a safe preparation point");
        let turn = game.turn();
        let mut workshop = Workshop::new(game.maneuver().clone());
        workshop.apply(Edit::DaggerExample);
        let prepared = workshop.draft().clone();

        assert_eq!(workshop.prepare(&mut game), Ok(()));
        assert_eq!(game.maneuver(), &prepared);
        assert_eq!(game.turn(), turn);
    }

    #[test]
    fn shape_and_power_edits_stay_within_the_supported_bounds() {
        let mut workshop = Workshop::new(Maneuver::default());
        workshop.apply(Edit::Payload);
        for expected in [Shape::Area(1), Shape::Area(2), Shape::Area(MAX_AREA_RADIUS), Shape::Point] {
            workshop.apply(Edit::Shape);
            assert_eq!(workshop.draft().technique.burst.unwrap().shape, expected);
        }
        for _ in 0..MAX_BURST_POWER + 2 {
            workshop.apply(Edit::PowerUp);
        }
        assert_eq!(workshop.draft().technique.burst.unwrap().power, MAX_BURST_POWER);
        for _ in 0..MAX_BURST_POWER + 2 {
            workshop.apply(Edit::PowerDown);
        }
        assert_eq!(workshop.draft().technique.burst.unwrap().power, 1);
    }

    #[test]
    fn examples_are_only_drafts_and_cannot_bypass_unsafe_preparation() {
        let mut game = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
        let before = game.clone();
        let mut workshop = Workshop::new(game.maneuver().clone());
        workshop.apply(Edit::StaffExample);
        assert_eq!(workshop.draft(), &Maneuver::staff_inferno());
        assert!(workshop.prepare(&mut game).is_err());
        assert_eq!(game, before);
        workshop.apply(Edit::Weapon);
        assert_eq!(game, before);
    }
}

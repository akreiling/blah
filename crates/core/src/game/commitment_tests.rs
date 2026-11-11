use super::*;
use crate::{Adaptation::*, LearnedMove, Maneuver, Shape, Weapon};

fn earned() -> Game {
    let mut game = Game::new(42);
    game.monsters.clear();
    for weapon in [Weapon::Dagger, Weapon::Spear, Weapon::Club, Weapon::Staff] {
        game.wanderer.weapons.learn(weapon, 40, 100);
    }
    game.wanderer.focus_skill.learn(4_000, Rank::Expert);
    game.wanderer.stealth.learn(4_000, Rank::Expert);
    game
}

#[test]
fn practice_opens_different_offers_and_deferring_changes_nothing() {
    let mut fighter = Game::new(42);
    fighter.wanderer.weapons.learn(Weapon::Dagger, 40, 100);
    let mut sneak = Game::new(42);
    sneak.wanderer.stealth.learn(4_000, Rank::Expert);
    assert_eq!(fighter.adaptation_offers().collect::<Vec<_>>(), vec![Discipline(Weapon::Dagger)]);
    assert_eq!(sneak.adaptation_offers().collect::<Vec<_>>(), vec![VeiledMovement]);
    let before = fighter.clone();
    for _ in 0..3 {
        assert_eq!(fighter.adaptation_offers().count(), 1);
    }
    assert_eq!(fighter, before);
    assert_eq!(Game::new(42).adaptation_offers().count(), 0);
}

#[test]
fn commitment_checks_history_room_exclusions_and_safe_place_atomically() {
    let mut game = earned();
    let before = game.clone();
    assert!(game.commit_adaptation(ConcentratedChanneling).is_err());
    assert_eq!(game, before, "Focus rank alone has not earned a released-burst commitment");
    game.commit_adaptation(Discipline(Weapon::Dagger)).unwrap();
    let before = game.clone();
    for blocked in [Discipline(Weapon::Dagger), Discipline(Weapon::Spear)] {
        assert!(game.commit_adaptation(blocked).is_err());
        assert_eq!(game, before);
    }
    game.commit_adaptation(DeepReserves).unwrap();
    let before = game.clone();
    assert!(game.commit_adaptation(VeiledMovement).is_err());
    assert_eq!(game, before);
    assert_eq!(game.wanderer.adaptations.free(), 0);
    assert_eq!(game.turn, 0);

    let mut danger = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
    danger.wanderer.weapons = game.wanderer.weapons;
    let before = danger.clone();
    assert!(danger.commit_adaptation(Discipline(Weapon::Dagger)).is_err());
    assert_eq!(danger, before);

    for (first, second) in [(DeepReserves, ConcentratedChanneling), (ConcentratedChanneling, DeepReserves)] {
        let mut game = earned();
        game.wanderer.adaptations.channeled = true;
        game.commit_adaptation(first).unwrap();
        let before = game.clone();
        assert!(game.commit_adaptation(second).is_err());
        assert_eq!(game, before);
    }
}

#[test]
fn refitting_death_and_snapshot_keep_commitments_without_passing_commit_time() {
    let mut game = earned();
    let before = game.wanderer;
    game.commit_adaptation(Discipline(Weapon::Dagger)).unwrap();
    game.commit_adaptation(VeiledMovement).unwrap();
    assert_eq!(
        (game.wanderer.hp, game.wanderer.focus, game.wanderer.weapons),
        (before.hp, before.focus, before.weapons)
    );
    assert_eq!(game.turn, 0);
    let held = game.wanderer.adaptations;
    game.prepare(Maneuver::staff_inferno()).unwrap();
    assert_eq!(game.wanderer.adaptations, held);
    let loaded = Game::from_snapshot(game.snapshot()).unwrap();
    assert_eq!(loaded, game);
    game.die();
    assert_eq!(game.wanderer.adaptations, held);
}

#[test]
fn deep_reserves_raise_capacity_not_current_focus_and_slow_rest() {
    let mut game = earned();
    let focus = game.wanderer.focus;
    let capacity = game.wanderer.max_focus();
    game.commit_adaptation(DeepReserves).unwrap();
    assert_eq!(game.wanderer.focus, focus);
    assert_eq!(game.wanderer.max_focus(), capacity + 6);
    game.recover_focus();
    assert_eq!(game.wanderer.focus, focus + REST_FOCUS - 1);
    for _ in 0..10 {
        game.recover_focus();
    }
    assert_eq!(game.wanderer.focus, capacity + 6);
}

#[test]
fn every_legal_commitment_pair_round_trips_with_its_effects() {
    for first in crate::Adaptation::ALL {
        for second in crate::Adaptation::ALL {
            let mut game = earned();
            game.wanderer.adaptations.channeled = true;
            game.commit_adaptation(first).unwrap();
            if game.wanderer.adaptations.fits(second) {
                game.commit_adaptation(second).unwrap();
            }
            let state = serde_json::to_vec(&game.snapshot()).unwrap();
            let loaded = Game::from_snapshot(serde_json::from_slice(&state).unwrap()).unwrap();
            assert_eq!(loaded, game);
            for weapon in [Weapon::Dagger, Weapon::Spear, Weapon::Club, Weapon::Staff] {
                assert_eq!(loaded.wanderer.training(weapon), game.wanderer.training(weapon));
            }
            assert_eq!(loaded.wanderer.max_focus(), game.wanderer.max_focus());
        }
    }
}

#[test]
fn veiled_conceals_and_frees_alternate_moves_but_only_weakens_free_strikes() {
    let mut game = earned();
    game.sneaking = true;
    let concealment = game.concealment();
    game.commit_adaptation(VeiledMovement).unwrap();
    assert_eq!(game.concealment(), concealment + 2);
    for moves in 0..10 {
        game.sneak_moves = moves;
        assert_eq!(game.sneak_cost(), if moves % 2 == 0 { 1 } else { 0 });
    }
    game.sneaking = false;
    assert_eq!(game.concealment(), 0);

    let mut trial = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
    trial.wanderer.stealth = game.wanderer.stealth;
    let plain = trial.preview_strike(Dir::East).unwrap();
    trial.wanderer.adaptations = game.wanderer.adaptations;
    let veiled = trial.preview_strike(Dir::East).unwrap();
    assert_eq!((veiled.min_hit, veiled.max_hit), ((plain.min_hit - 1).max(0), (plain.max_hit - 1).max(0)));
    trial.wanderer.weapons.learn(Weapon::Dagger, 10, 100);
    let preview = trial.preview_move(LearnedMove::Withdraw, Dir::East).unwrap();
    let mut plain_trial = trial.clone();
    plain_trial.wanderer.adaptations = crate::Adaptations::default();
    assert_eq!(
        trial.preview_move(LearnedMove::Withdraw, Dir::East),
        plain_trial.preview_move(LearnedMove::Withdraw, Dir::East)
    );
    trial.use_move(LearnedMove::Withdraw, Dir::East);
    let hit = trial.last_attack.unwrap();
    assert!(!hit.hit || (preview.min_hit..=preview.max_hit).contains(&hit.total()));
}

#[test]
fn discipline_caps_new_other_weapon_learning_without_removing_old_practice() {
    let mut game = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
    game.wanderer.adaptations.add(Discipline(Weapon::Spear));
    game.monsters[0].phenotype.stats.hp = 10_000;
    game.monsters[0].hp = 10_000;
    game.monsters[0].practice_left = 10_000;
    let result = AttackResult { hit: true, physical: 30, ..AttackResult::default() };
    for _ in 0..3 {
        game.record_attack(0, result, AttackSource::Maneuver(&Maneuver::default()), 0);
    }
    assert_eq!(game.wanderer.weapons.get(Weapon::Dagger).rank(), Rank::Practiced);
    game.wanderer.weapons.learn(Weapon::Dagger, 40, 100);
    let before = game.wanderer.weapons;
    game.record_attack(0, result, AttackSource::Maneuver(&Maneuver::default()), 0);
    assert_eq!(game.wanderer.weapons, before, "earlier skilled practice is not erased");
}

#[test]
fn concentration_limits_preparation_and_existing_radius_three_without_rewriting_it() {
    let mut game = earned();
    game.wanderer.adaptations.channeled = true;
    let mut wide = Maneuver::staff_inferno();
    wide.technique.burst.as_mut().unwrap().shape = Shape::Area(3);
    game.prepare(wide.clone()).unwrap();
    game.commit_adaptation(ConcentratedChanneling).unwrap();
    assert_eq!(game.maneuver, wide);
    let before = game.clone();
    assert!(game.prepare(wide).is_err());
    assert_eq!(game, before);
    assert!(game.preview_area().is_err());
    assert_eq!(game.unleash(), Step::Blocked);
    assert_eq!(game.turn, 0);
    assert_eq!(game.wanderer.focus, before.wanderer.focus);
    assert!(Game::from_snapshot(game.snapshot()).is_ok());
    game.prepare(Maneuver::staff_inferno()).unwrap();
    assert!(game.preview_area().is_ok());
}

#[test]
fn concentration_adds_two_to_every_area_target_and_self_and_tracks_real_releases() {
    let mut game = Game::inferno_trial(42).unwrap();
    let base = game.preview_area().unwrap();
    game.wanderer.adaptations.add(ConcentratedChanneling);
    let preview = game.preview_area().unwrap();
    assert_eq!(preview.damage, base.damage + 2 * base.targets as i32);
    assert_eq!(preview.self_damage, base.self_damage + 2);
    assert_eq!(game.maneuver_backlash(game.maneuver()), Ok(preview.self_damage));
    let hp = game.wanderer.hp;
    game.unleash();
    assert_eq!(game.wanderer.hp, hp - preview.self_damage);
    assert!(game.wanderer.adaptations.channeled);

    let mut empty = earned();
    empty.prepare(Maneuver::staff_inferno()).unwrap();
    empty.unleash();
    assert!(!empty.wanderer.adaptations.channeled, "self-hit is not a burst into a creature");
}

#[test]
fn successful_burst_not_miss_or_failed_trigger_earns_concentration_history() {
    let mut released = 0;
    let mut missed = 0;
    for seed in 0..100 {
        let mut game = Game::composition_trial(seed, Maneuver::backstab_burst()).unwrap();
        game.wanderer.focus_skill.learn(4_000, Rank::Expert);
        game.use_technique(Dir::East);
        let result = game.last_attack.unwrap();
        assert_eq!(game.wanderer.adaptations.channeled, result.burst_released);
        assert_eq!(game.adaptation_offers().any(|a| a == ConcentratedChanneling), result.burst_released);
        released += usize::from(result.burst_released);
        missed += usize::from(!result.hit);
        let mut blocked = Game::composition_trial(seed, Maneuver::backstab_burst()).unwrap();
        blocked.monsters[0].phenotype.stats.defense = 100;
        blocked.use_technique(Dir::East);
        assert!(!blocked.wanderer.adaptations.channeled);
    }
    assert!(released > 0 && missed > 0);
}

#[test]
fn concentrated_pulse_uses_the_same_bonus_in_preview_and_execution() {
    for seed in 0..40 {
        let mut game = Game::composition_trial(seed, Maneuver::default()).unwrap();
        game.wanderer.focus_skill.learn(4_000, Rank::Expert);
        game.wanderer.adaptations.add(ConcentratedChanneling);
        let p = game.preview_move(LearnedMove::Pulse, Dir::East).unwrap();
        assert_eq!((p.min_hit, p.max_hit), (3, 3));
        game.use_move(LearnedMove::Pulse, Dir::East);
        let result = game.last_attack.unwrap();
        assert_eq!(result.burst, if result.hit { 3 } else { 0 });
    }
}

//! Earn the first weapon ranks through public actions, without injected practice or resources.

use blah_core::{Dir, Game, LearnedMove, LogKind, MAX_FOCUS, Maneuver, Rank, Skill, Step, Weapon};

#[test]
fn every_weapon_earns_its_own_rank_through_ordinary_strikes() {
    let weapons = [Weapon::Dagger, Weapon::Spear, Weapon::Club, Weapon::Staff];
    for (weapon, threat) in [(Weapon::Dagger, 133), (Weapon::Spear, 44), (Weapon::Club, 33), (Weapon::Staff, 66)] {
        let mut completed = 0;
        for seed in 0..8 {
            let maneuver = Maneuver { weapon, ..Maneuver::default() };
            let mut game = Game::composition_trial(seed, maneuver).unwrap();
            assert!(!LearnedMove::for_weapon(weapon).unlocked(game.wanderer()));
            let sentinel = game.monsters()[0].id;
            let initial = game.preview_strike(Dir::East).unwrap();
            let mut learned = Skill::default();
            for _ in 0..80 {
                if !game.monsters().iter().any(|m| m.id == sentinel) {
                    break;
                }
                learned = game.wanderer().weapons.get(weapon);
                assert_eq!(game.step(Dir::East), Step::Attacked);
                if game.deaths() > 0 {
                    break;
                }
                assert_eq!(game.wanderer().focus, MAX_FOCUS, "basic practice is free");
                if game.wanderer().weapons.get(weapon).rank() == Rank::Practiced
                    && game.monsters().iter().any(|m| m.id == sentinel)
                {
                    assert_eq!(game.preview_strike(Dir::East).unwrap().max_hit, initial.max_hit + 1);
                }
            }
            for other in weapons.into_iter().filter(|&w| w != weapon) {
                assert_eq!(game.wanderer().weapons.get(other), Skill::default());
            }
            if game.deaths() > 0 {
                // Wild creatures can join the fight. The fatal turn's own strike may still count;
                // dying never takes practice away.
                assert!(game.wanderer().weapons.get(weapon).rank() >= learned.rank());
                continue;
            }
            assert!(!game.monsters().iter().any(|m| m.id == sentinel), "the encounter completes");
            let mut expected = Skill::default();
            // Independent full-encounter accounting: exactly 40 real physical damage, no overkill,
            // scaled by the sentinel's base threat with this weapon. No kill bonus.
            expected.learn(40 * threat, Rank::Expert);
            assert_eq!(game.wanderer().weapons.get(weapon), expected);
            assert!(expected.rank() >= Rank::Practiced);
            assert!(LearnedMove::for_weapon(weapon).unlocked(game.wanderer()));
            let learned = format!("Learned {}.", LearnedMove::for_weapon(weapon).name());
            assert_eq!(game.log().lines().filter(|line| line.starts_with(&learned)).count(), 1);
            assert!(game.log().entries().any(|entry| entry.kind == LogKind::Learning));
            completed += 1;
        }
        assert!(completed >= 6, "{weapon:?}: only {completed} of 8 trial fights completed");
    }
}

#[test]
fn earned_weapon_moves_are_usable_without_replacing_the_prepared_technique() {
    for weapon in [Weapon::Dagger, Weapon::Club, Weapon::Staff] {
        let mut game = Game::composition_trial(42, Maneuver { weapon, ..Maneuver::default() }).unwrap();
        let action = LearnedMove::for_weapon(weapon);
        for _ in 0..64 {
            if action.unlocked(game.wanderer()) {
                break;
            }
            assert_eq!(game.step(Dir::East), Step::Attacked);
        }
        assert!(action.unlocked(game.wanderer()));
        let prepared = game.maneuver().clone();
        let p = game.preview_move(action, Dir::East).expect("the original target and open trial ground remain");
        let focus = game.wanderer().focus;
        let turn = game.turn();
        assert_eq!(game.use_move(action, Dir::East), Step::Attacked);
        assert_eq!(game.turn(), turn + 1);
        assert_eq!(game.wanderer().focus, focus - p.focus);
        assert_eq!(game.maneuver(), &prepared);
    }
}

#[test]
fn unseen_movement_earns_slip_and_real_paid_hits_earn_the_depleted_casters_pulse() {
    let mut sneak = Game::composition_trial(42, Maneuver::default()).unwrap();
    for _ in 0..16 {
        if LearnedMove::SlipAway.unlocked(sneak.wanderer()) {
            break;
        }
        let position = sneak.position();
        let direction = Dir::ALL
            .into_iter()
            .filter(|&dir| {
                let (dx, dy) = dir.delta();
                sneak.can_step(dir) && sneak.monster_at(position.0 + dx, position.1 + dy).is_none()
            })
            .min_by_key(|&dir| {
                let (dx, dy) = dir.delta();
                let destination = (position.0 + dx, position.1 + dy);
                sneak
                    .monsters()
                    .iter()
                    .map(|m| (m.position.0 - destination.0).abs().max((m.position.1 - destination.1).abs()))
                    .min()
                    .unwrap_or(i64::MAX)
            })
            .expect("at least one open trial tile");
        assert_eq!(sneak.step(direction), Step::Moved);
    }
    assert!(LearnedMove::SlipAway.unlocked(sneak.wanderer()));
    let dir = Dir::ALL.into_iter().find(|&dir| sneak.preview_move(LearnedMove::SlipAway, dir).is_ok()).unwrap();
    assert_eq!(sneak.use_move(LearnedMove::SlipAway, dir), Step::Moved);
    assert_eq!(sneak.log().lines().filter(|line| line.starts_with("Learned Slip away.")).count(), 1);

    let mut caster = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
    for _ in 0..3 {
        assert_eq!(caster.use_technique(Dir::East), Step::Attacked);
    }
    assert!(LearnedMove::Pulse.unlocked(caster.wanderer()));
    assert_eq!(caster.wanderer().focus, 3);
    assert_eq!(caster.toggle_sneak(), Step::Waited);
    for _ in 0..2 {
        assert_eq!(caster.step(Dir::North), Step::Moved);
    }
    assert_eq!(caster.wanderer().focus, 0);
    let direction = (0..4)
        .find_map(|_| {
            let dir = Dir::ALL.into_iter().find(|&d| caster.preview_move(LearnedMove::Pulse, d).is_ok());
            if dir.is_none() {
                caster.wait();
            }
            dir
        })
        .expect("the sentinel follows the retreat");
    let focus_skill = caster.wanderer().focus_skill;
    let weapon_skill = caster.wanderer().weapons;
    let gain = caster.preview_move(LearnedMove::Pulse, direction).unwrap().focus_gain;
    assert_eq!(caster.use_move(LearnedMove::Pulse, direction), Step::Attacked);
    let hit = caster.last_attack().unwrap().hit;
    assert_eq!(caster.wanderer().focus, if hit { gain } else { 0 }, "a depleted caster draws Focus back");
    assert_eq!(caster.wanderer().focus_skill, focus_skill);
    assert_eq!(caster.wanderer().weapons, weapon_skill);
    assert_eq!(caster.log().lines().filter(|line| line.starts_with("Learned Pulse.")).count(), 1);
}

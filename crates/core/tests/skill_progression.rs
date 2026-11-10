//! Earn the first weapon ranks through public actions, without injected practice or resources.

use blah_core::{Dir, Game, LogKind, MAX_FOCUS, Maneuver, Rank, Skill, Step, Weapon};

#[test]
fn every_weapon_earns_its_own_rank_through_ordinary_strikes() {
    let weapons = [Weapon::Dagger, Weapon::Spear, Weapon::Club, Weapon::Staff];
    for (weapon, threat) in [(Weapon::Dagger, 133), (Weapon::Spear, 44), (Weapon::Club, 33), (Weapon::Staff, 66)] {
        let mut completed = 0;
        for seed in 0..8 {
            let maneuver = Maneuver { weapon, ..Maneuver::default() };
            let mut game = Game::composition_trial(seed, maneuver).unwrap();
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
            assert!(game.log().entries().any(|entry| entry.kind == LogKind::Learning));
            completed += 1;
        }
        assert!(completed >= 6, "{weapon:?}: only {completed} of 8 trial fights completed");
    }
}

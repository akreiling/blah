//! Mechanical fixtures are explicit; public earned-progression checks live in skill_progression.

use super::*;
use crate::{Access, Rng, Weapon};
use proptest::prelude::*;

const LEARNED_MOVES: [LearnedMove; 6] = [
    LearnedMove::Withdraw,
    LearnedMove::Reach,
    LearnedMove::Shove,
    LearnedMove::Sweep,
    LearnedMove::SlipAway,
    LearnedMove::Pulse,
];

fn ready(seed: u64, weapon: Weapon) -> Game {
    try_ready(seed, weapon).expect("test seed has an open trial clearing")
}

fn try_ready(seed: u64, weapon: Weapon) -> Option<Game> {
    let game = Game::composition_trial(seed, Maneuver { weapon, ..Maneuver::default() }).ok()?;
    Some(ready_game(game, weapon))
}

fn ready_game(mut game: Game, weapon: Weapon) -> Game {
    game.sneaking = false;
    game.monsters[0].energy = -100_000;
    game.wanderer.weapons.learn(weapon, 10, 100);
    game.wanderer.focus_skill.learn(1_000, Rank::Expert);
    game.wanderer.stealth.learn(1_000, Rank::Expert);
    game
}

fn add(game: &mut Game, offset: (i64, i64)) -> MonsterId {
    let mut m = game.monsters[0].clone();
    m.id = MonsterId(game.next_monster);
    game.next_monster += 1;
    m.position = (game.position.0 + offset.0, game.position.1 + offset.1);
    m.home = m.position;
    let id = m.id;
    game.monsters.push(m);
    id
}

fn blocked(game: &mut Game, action: LearnedMove, dir: Dir) {
    let before = game.clone();
    assert!(game.preview_move(action, dir).is_err());
    assert_eq!(game, &before);
    assert_eq!(game.use_move(action, dir), Step::Blocked);
    let mut expected = before;
    expected.log = game.log.clone();
    assert_eq!(game, &expected, "rejection may only add an explanation");
}

#[test]
fn locks_equipment_focus_and_empty_targets_are_atomic() {
    let mut novice = Game::composition_trial(42, Maneuver::default()).unwrap();
    for action in LEARNED_MOVES {
        blocked(&mut novice, action, Dir::East);
    }
    let mut game = ready(42, Weapon::Dagger);
    game.wanderer.weapons.learn(Weapon::Spear, 10, 100);
    blocked(&mut game, LearnedMove::Reach, Dir::East);
    game.wanderer.focus = 2;
    blocked(&mut game, LearnedMove::Withdraw, Dir::East);
    game.wanderer.focus = 24;
    blocked(&mut game, LearnedMove::Withdraw, Dir::North);
    blocked(&mut game, LearnedMove::Pulse, Dir::North);
    let mut staff = ready(42, Weapon::Staff);
    staff.monsters.clear();
    blocked(&mut staff, LearnedMove::Sweep, Dir::East);
}

#[test]
fn blocked_paths_do_not_charge() {
    let mut game = ready(42, Weapon::Dagger);
    add(&mut game, (-1, 0));
    blocked(&mut game, LearnedMove::Withdraw, Dir::East);
    blocked(&mut game, LearnedMove::SlipAway, Dir::East);
    game.monsters[0].position = (game.position.0 + 2, game.position.1);
    blocked(&mut game, LearnedMove::SlipAway, Dir::East);

    let mut spear = ready(42, Weapon::Spear);
    blocked(&mut spear, LearnedMove::Reach, Dir::East); // Occupied intervening tile.
    spear.monsters[0].position = (spear.position.0 + 2, spear.position.1);
    add(&mut spear, (1, 0));
    blocked(&mut spear, LearnedMove::Reach, Dir::East);

    let mut club = ready(42, Weapon::Club);
    add(&mut club, (2, 0));
    let before = club.clone();
    assert!(club.preview_move(LearnedMove::Shove, Dir::East).unwrap().push.is_none());
    assert_eq!(club.use_move(LearnedMove::Shove, Dir::East), Step::Attacked);
    assert_eq!(club.turn, before.turn + 1);
    assert_eq!(club.wanderer.focus, before.wanderer.focus - 3);
    assert_eq!(
        club.monsters[0].position, before.monsters[0].position,
        "an occupied push tile does not cancel the strike"
    );
    club.monsters.pop();
    club.monsters[0].phenotype.access = Access { land: false, shallow: false, deep: true };
    assert!(club.preview_move(LearnedMove::Shove, Dir::East).unwrap().push.is_none());
}

#[test]
fn withdraw_and_shove_pay_once_on_hits_and_misses_and_leave_preparation_unchanged() {
    let mut hits = 0;
    let mut misses = 0;
    for seed in 0..32 {
        for (weapon, action) in [(Weapon::Dagger, LearnedMove::Withdraw), (Weapon::Club, LearnedMove::Shove)] {
            let mut game = ready(seed, weapon);
            let before = game.clone();
            let preview = game.preview_move(action, Dir::East).unwrap();
            assert_eq!(game, before, "preview is pure");
            assert_eq!(game.use_move(action, Dir::East), Step::Attacked);
            assert_eq!(game.turn, before.turn + 1);
            assert_eq!(game.wanderer.focus, before.wanderer.focus - 3);
            assert_eq!(game.maneuver, before.maneuver);
            let result = game.last_attack.unwrap();
            if result.hit {
                hits += 1;
                assert!((preview.min_hit..=preview.max_hit).contains(&result.total()));
            } else {
                misses += 1;
                assert_eq!(game.wanderer.weapons, before.wanderer.weapons);
                assert_eq!(game.wanderer.focus_skill, before.wanderer.focus_skill);
            }
            if action == LearnedMove::Withdraw {
                assert_eq!(game.position, (before.position.0 - 1, before.position.1), "withdraw even on a miss");
                assert!(game.monsters[0].alerted, "a one-tile withdrawal does not erase tracking");
            } else {
                assert_eq!(game.position, before.position);
                assert_eq!(game.monsters[0].position.0, before.position.0 + if result.hit { 2 } else { 1 });
            }
        }
    }
    assert!(hits > 0 && misses > 0);
}

#[test]
fn reach_strikes_two_tiles_away_using_the_existing_weapon_preview() {
    let mut game = ready(42, Weapon::Spear);
    game.monsters[0].position = (game.position.0 + 2, game.position.1);
    let before = game.clone();
    let p = game.preview_move(LearnedMove::Reach, Dir::East).unwrap();
    let ordinary = game
        .maneuver
        .basic_strike()
        .preview_trained(game.attack_context(&game.monsters[0]), game.wanderer.training(Weapon::Spear))
        .unwrap();
    assert_eq!((p.min_hit, p.max_hit), (ordinary.min_hit, ordinary.max_hit));
    assert_eq!(game.use_move(LearnedMove::Reach, Dir::East), Step::Attacked);
    assert_eq!(game.position, before.position);
    assert_eq!(game.wanderer.focus, 22);
    assert_eq!(game.turn, before.turn + 1);
    assert!(game.last_attack.unwrap().hit);
}

#[test]
fn sweep_uses_snapshot_training_handles_removed_targets_and_credits_focus_once() {
    let mut game = ready(42, Weapon::Staff);
    for at in [(0, -1), (0, 1), (-1, 0), (1, 1)] {
        add(&mut game, at);
    }
    let diagonal = game.monsters.last().unwrap().id;
    game.monsters[1].hp = 1;
    game.monsters[2].hp = 1;
    // Approach adept from below: the first physical hit changes the damage bonus.
    game.wanderer.weapons = crate::WeaponSkills::default();
    game.wanderer.weapons.learn(Weapon::Staff, 59, 200); // 11,800 practice.
    game.wanderer.weapons.learn(Weapon::Staff, 1, 199); // 11,999.
    game.maneuver = Maneuver::staff_inferno();
    let before = game.clone();
    let p = game.preview_move(LearnedMove::Sweep, Dir::East).unwrap();
    assert_eq!(p.targets, 4);
    let maneuver = game.maneuver.basic_strike();
    let mut rng = Rng::keyed(game.world.seed(), b"wanderer", &[game.turn + 1]);
    let expected: Vec<_> = Dir::ALL
        .into_iter()
        .map(|dir| {
            let at = game.ahead(dir);
            let m = game.monster_at(at.0, at.1).unwrap();
            let result = maneuver
                .resolve_trained(game.attack_context(m), game.wanderer.training(Weapon::Staff), &mut rng)
                .unwrap();
            (m.id, m.hp, result)
        })
        .collect();
    assert!(expected.iter().any(|(_, hp, result)| *hp == 1 && result.hit));
    assert_eq!(game.use_move(LearnedMove::Sweep, Dir::North), Step::Attacked, "direction does not narrow a sweep");
    assert_eq!(game.turn, before.turn + 1);
    assert_eq!(game.wanderer.focus, 20);
    assert_eq!(game.maneuver, before.maneuver);
    assert_eq!(game.wanderer.hp, before.wanderer.hp, "no area fire or self-damage");
    for (id, hp, result) in &expected {
        let remaining = game.monsters.iter().find(|m| m.id == *id).map_or(0, |m| m.hp);
        assert_eq!(remaining, (hp - result.total()).max(0));
        if *hp == 1 && result.hit {
            assert!(game.monsters.iter().all(|m| m.id != *id), "removed targets do not shift the sweep");
        }
    }
    assert_eq!(game.monsters.iter().find(|m| m.id == diagonal).unwrap().hp, 40);
    let physical: i32 = expected.iter().map(|(_, _, r)| r.physical).sum();
    assert_eq!(game.last_attack.unwrap().physical, physical);
    assert_eq!(game.last_attack.unwrap().burst, 0);
    let mut focus = before.wanderer.focus_skill;
    focus.learn(4 * 66, Rank::Expert); // Four paid Focus, one maximum threat, not four awards.
    assert_eq!(game.wanderer.focus_skill, focus);
    assert_eq!(game.wanderer.weapons.get(Weapon::Staff).rank(), Rank::Adept);
}

#[test]
fn slip_is_two_steps_one_turn_and_tracking_depends_on_actual_range() {
    for (sense, found) in [(2, false), (4, true)] {
        let mut game = ready(42, Weapon::Dagger);
        game.monsters[0].phenotype.sense = sense;
        game.monsters[0].alerted = true;
        game.monsters[0].tracking = true;
        let before = game.clone();
        let p = game.preview_move(LearnedMove::SlipAway, Dir::West).unwrap();
        assert_eq!(p.targets, 0);
        assert_eq!(p.destination, Some((game.position.0 - 2, game.position.1)));
        assert_eq!(game.use_move(LearnedMove::SlipAway, Dir::West), Step::Moved);
        assert_eq!(game.turn, before.turn + 1);
        assert_eq!(game.wanderer.focus, 20);
        assert_eq!(game.position, p.destination.unwrap());
        assert!(game.sneaking);
        assert_eq!(game.monsters[0].alerted, found);
        assert_eq!(game.monsters[0].tracking, found);
        assert_eq!(game.wanderer.weapons, before.wanderer.weapons);
        assert_eq!(game.wanderer.focus_skill, before.wanderer.focus_skill);
    }
}

#[test]
fn pulse_is_weak_free_and_cannot_train_itself_or_the_equipped_weapon() {
    let mut hits = 0;
    let mut misses = 0;
    for seed in 0..32 {
        let mut game = ready(seed, Weapon::Dagger);
        game.wanderer.focus = 0;
        game.wanderer.stats.attack = 100;
        game.monsters[0].phenotype.stats.defense = 100;
        game.monsters[0].practice_left = 1;
        let before = game.clone();
        let p = game.preview_move(LearnedMove::Pulse, Dir::East).unwrap();
        assert_eq!((p.focus, p.focus_gain, p.min_hit, p.max_hit), (0, 2, 1, 1));
        assert_eq!(game.use_move(LearnedMove::Pulse, Dir::East), Step::Attacked);
        let result = game.last_attack.unwrap();
        assert_eq!(result.physical, 0);
        assert_eq!(game.wanderer.weapons, before.wanderer.weapons);
        assert_eq!(game.wanderer.focus_skill, before.wanderer.focus_skill);
        if result.hit {
            hits += 1;
            assert_eq!(result.burst, 1);
            assert_eq!(game.monsters[0].practice_left, 0, "uses the shared damage allowance");
            assert_eq!(game.wanderer.focus, 2, "a landed pulse draws Focus");
            // The allowance is spent, so the same creature cannot be farmed for more Focus.
            let p = game.preview_move(LearnedMove::Pulse, Dir::East).unwrap();
            assert_eq!(p.focus_gain, 0);
            for _ in 0..8 {
                game.use_move(LearnedMove::Pulse, Dir::East);
            }
            assert_eq!(game.wanderer.focus, 2);
        } else {
            misses += 1;
            assert_eq!(game.monsters[0].practice_left, 1);
            assert_eq!(game.wanderer.focus, 0);
        }
    }
    assert!(hits > 0 && misses > 0);
}

#[test]
fn pulse_focus_never_exceeds_capacity() {
    for seed in 0..16 {
        let mut game = ready(seed, Weapon::Dagger);
        game.wanderer.stats.attack = 100;
        game.monsters[0].phenotype.stats.defense = 100;
        game.wanderer.focus = game.wanderer.max_focus() - 1;
        assert_eq!(game.preview_move(LearnedMove::Pulse, Dir::East).unwrap().focus_gain, 1);
        game.use_move(LearnedMove::Pulse, Dir::East);
        assert!(game.wanderer.focus <= game.wanderer.max_focus());
        game.wanderer.focus = game.wanderer.max_focus();
        assert_eq!(game.preview_move(LearnedMove::Pulse, Dir::East).unwrap().focus_gain, 0);
    }
}

#[test]
fn learned_actions_survive_death_but_do_not_transfer_between_weapon_families() {
    let mut game = ready(42, Weapon::Dagger);
    game.die();
    assert!(LearnedMove::Withdraw.unlocked(&game.wanderer));
    assert!(LearnedMove::SlipAway.unlocked(&game.wanderer));
    assert!(LearnedMove::Pulse.unlocked(&game.wanderer));
    assert!(!LearnedMove::Reach.unlocked(&game.wanderer));
    game.monsters.clear();
    game.prepare(Maneuver { weapon: Weapon::Spear, ..Maneuver::default() }).unwrap();
    assert!(LearnedMove::Withdraw.unlocked(&game.wanderer));
    assert!(!LearnedMove::Reach.unlocked(&game.wanderer));
    blocked(&mut game, LearnedMove::Withdraw, Dir::East);
}

#[test]
fn generated_move_properties_skip_seeds_without_a_supported_trial() {
    assert!(try_ready(7_605_929_170_872_761_880, Weapon::Dagger).is_none());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn previews_replay_atomicity_resources_and_turns_match_real_actions(
        seed in any::<u64>(), weapon in 0usize..4,
        actions in prop::collection::vec((0usize..6, 0usize..4), 1..24),
    ) {
        let weapons = [Weapon::Dagger, Weapon::Spear, Weapon::Club, Weapon::Staff];
        let Some(mut game) = try_ready(seed, weapons[weapon]) else {
            // Some seeds have no open trial clearing; the invariant is about any supported trial.
            return Ok(());
        };
        let mut twin = game.clone();
        for (action, direction) in actions {
            let action = LEARNED_MOVES[action];
            let direction = Dir::ALL[direction];
            let before = game.clone();
            let preview = game.preview_move(action, direction);
            prop_assert_eq!(&game, &before);
            let result = game.use_move(action, direction);
            prop_assert_eq!(result, twin.use_move(action, direction));
            prop_assert_eq!(&game, &twin);
            prop_assert_eq!(preview.is_ok(), result != Step::Blocked);
            if let Ok(p) = preview {
                prop_assert_eq!(game.turn, before.turn + 1 + u64::from(game.deaths - before.deaths) * DEATH_TURNS);
                if game.deaths == before.deaths {
                    let hit = result == Step::Attacked && game.last_attack.unwrap().hit;
                    let gain = if hit { p.focus_gain } else { 0 };
                    prop_assert_eq!(game.wanderer.focus, before.wanderer.focus - p.focus + gain);
                    if let Some(destination) = p.destination { prop_assert_eq!(game.position, destination); }
                    if result == Step::Attacked {
                        prop_assert!(game.last_attack.unwrap().total() <= p.max_hit);
                    }
                }
            } else {
                let mut expected = before;
                expected.log = game.log.clone();
                prop_assert_eq!(&game, &expected);
            }
            prop_assert!((0..=game.wanderer.max_focus()).contains(&game.wanderer.focus));
        }
    }
}

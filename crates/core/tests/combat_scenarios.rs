//! Player-action reproductions of the current demo dead ends, not assertions that those rules are fun.
//! Only public Game entry points are used: no injected Focus, enemy stats, positions, or RNG results.

use blah_core::{Aim, Dir, Element, Game, LogKind, MAX_FOCUS, Maneuver, Step, Technique, Trigger, Weapon};

/// Print actions, resources, enemies, and only the messages produced by this action.
fn act(game: &mut Game, label: &str, action: impl FnOnce(&mut Game) -> Step) -> Step {
    let mark = game.log().total();
    let result = action(game);
    println!(
        "{label}: {result:?}, turn={}, HP={}, Focus={}, position={:?}",
        game.turn(),
        game.wanderer().hp,
        game.wanderer().focus,
        game.position(),
    );
    for monster in game.monsters_in_view() {
        println!("  {}: HP={}, tracking={}, at={:?}", monster.name, monster.hp, monster.alerted, monster.position);
    }
    for entry in game.log().last((game.log().total() - mark) as usize) {
        println!("  {:?}: {}", entry.kind, entry.text);
    }
    result
}

/// A rejected action may explain itself in the log, but cannot advance combat or spend resources.
fn blocked(game: &mut Game, label: &str, action: impl FnOnce(&mut Game) -> Step) {
    let before = game.clone();
    assert_eq!(act(game, label, action), Step::Blocked);
    assert_eq!(game.turn(), before.turn());
    assert_eq!(game.position(), before.position());
    assert_eq!(game.wanderer(), before.wanderer());
    assert_eq!(game.monsters(), before.monsters());
    assert_eq!(game.maneuver(), before.maneuver());
    assert_eq!(game.sneaking(), before.sneaking());
    assert_eq!(game.last_attack(), before.last_attack());
    assert_eq!(game.deaths(), before.deaths());
    assert_eq!(game.log().entries().next_back().unwrap().kind, LogKind::Blocked);
}

#[test]
fn dagger_demo_runs_out_before_the_sentinel_dies_and_cannot_change_approach() {
    for seed in [0, 7, 42] {
        println!("\nDAGGER seed {seed}");
        let mut game = Game::composition_trial(seed, Maneuver::backstab_burst()).unwrap();
        let sentinel = game.monsters()[0].id;
        assert_eq!(game.wanderer().focus, MAX_FOCUS);
        assert!(game.preview_attack(Dir::East).unwrap().opening);

        for attempt in 1..=3 {
            assert_eq!(act(&mut game, "bump east", |g| g.step(Dir::East)), Step::Attacked);
            assert_eq!(game.wanderer().focus, MAX_FOCUS - 7 * attempt);
            assert!(game.monsters().iter().any(|m| m.id == sentinel && m.hp > 0));
            assert!(!game.preview_attack(Dir::East).unwrap().opening);
        }

        assert!(!game.can_step(Dir::East));
        blocked(&mut game, "fourth bump", |g| g.step(Dir::East));
        blocked(&mut game, "repeat blocked bump", |g| g.step(Dir::East));
        assert_eq!(game.position(), game.home(), "the demo already starts at the recovery location");
        blocked(&mut game, "recover at home with sentinel alive", Game::recover_focus);

        let before = game.clone();
        let reason = game.prepare(Maneuver::default()).unwrap_err();
        println!("prepare a free basic strike: {reason}");
        assert_eq!(game, before, "nearby sentinel prevents switching to a free attack");

        let focus = game.wanderer().focus;
        assert_eq!(act(&mut game, "wait instead", Game::wait), Step::Waited);
        assert_eq!(game.wanderer().focus, focus, "waiting does not repair the exhausted combat loop");
        assert_eq!(game.deaths(), 0);
    }
}

#[test]
fn sneaking_again_after_the_opening_spends_focus_without_restoring_surprise() {
    let mut game = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
    assert_eq!(act(&mut game, "opening strike", |g| g.step(Dir::East)), Step::Attacked);
    assert!(game.last_attack().unwrap().amplified);
    let focus = game.wanderer().focus;
    assert_eq!(act(&mut game, "try sneaking again", Game::toggle_sneak), Step::Waited);
    assert!(game.sneaking());
    assert_eq!(game.wanderer().focus, focus - 1);
    assert!(!game.preview_attack(Dir::East).unwrap().opening);

    assert_eq!(act(&mut game, "attack while tracked", |g| g.step(Dir::East)), Step::Attacked);
    assert!(!game.last_attack().unwrap().amplified);
    assert!(!game.sneaking());
    assert_eq!(game.wanderer().focus, focus - 1 - 7);
    assert_eq!(game.deaths(), 0);
}

#[test]
fn staff_demo_leaves_a_survivor_but_neither_cast_nor_bump_can_finish_it() {
    let mut game = Game::inferno_trial(42).unwrap();
    let survivor = game.monsters().iter().find(|m| m.name == "heat-hardy trial sentinel").unwrap().id;
    assert_eq!(act(&mut game, "strike staff", Game::unleash), Step::Attacked);
    assert_eq!((game.wanderer().hp, game.wanderer().focus), (18, 6));
    assert_eq!(game.monsters().iter().find(|m| m.id == survivor).unwrap().hp, 3);
    assert_eq!(game.log().entries().filter(|e| e.kind == LogKind::Kill).count(), 2);
    blocked(&mut game, "repeat staff strike", Game::unleash);
    blocked(&mut game, "try recovering", Game::recover_focus);
    let before = game.clone();
    assert!(game.prepare(Maneuver::default()).is_err());
    assert_eq!(game, before);

    // Let the actual survivor approach, rather than moving it beside the player in test setup.
    let adjacent = |game: &Game| {
        let (x, y) = game.position();
        let monster = game.monsters().iter().find(|m| m.id == survivor).unwrap();
        ((monster.position.0 - x).abs() + (monster.position.1 - y).abs()) == 1
    };
    for _ in 0..3 {
        if adjacent(&game) {
            break;
        }
        assert_eq!(act(&mut game, "wait for survivor", Game::wait), Step::Waited);
    }
    assert!(adjacent(&game), "the real survivor should close to melee range");
    let monster = game.monsters().iter().find(|m| m.id == survivor).unwrap();
    let (x, y) = game.position();
    let direction = if monster.position.0 > x {
        Dir::East
    } else if monster.position.0 < x {
        Dir::West
    } else if monster.position.1 > y {
        Dir::South
    } else {
        Dir::North
    };
    assert!(!game.can_step(direction));
    blocked(&mut game, "bump survivor with staff", |g| g.step(direction));
    assert_eq!(game.monsters().iter().find(|m| m.id == survivor).unwrap().hp, 3);
    assert_eq!(game.wanderer().focus, 6);
    assert_eq!(game.deaths(), 0);
}

const SURVEY_SEEDS: [u64; 18] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 42, u64::MAX];

/// Compare loadouts against precisely the same encounter, including misses and retaliation.
/// This measures these fixtures, not general build balance or optimal play.
#[test]
fn survey_committed_loadouts_against_the_same_sentinel() {
    let mut loadouts = vec![Maneuver::backstab_burst()];
    for weapon in [Weapon::Dagger, Weapon::Spear, Weapon::Club, Weapon::Staff] {
        loadouts.push(Maneuver { name: format!("Basic {}", weapon.name()), weapon, technique: Technique::default() });
    }
    for (name, aim, trigger, element, power) in [
        ("Direct penetration", Aim::Direct, Trigger::Penetration, Element::Force, 3),
        ("Direct force contact", Aim::Direct, Trigger::Contact, Element::Force, 3),
        ("Direct fire contact", Aim::Direct, Trigger::Contact, Element::Fire, 3),
        ("Cheap fire contact", Aim::Direct, Trigger::Contact, Element::Fire, 1),
        ("Large opening burst", Aim::Opening, Trigger::Penetration, Element::Force, 6),
    ] {
        let mut maneuver = Maneuver::backstab_burst();
        maneuver.name = name.into();
        maneuver.technique.aim = aim;
        let burst = maneuver.technique.burst.as_mut().unwrap();
        burst.trigger = trigger;
        burst.element = element;
        burst.power = power;
        loadouts.push(maneuver);
    }

    println!("\nLOADOUT SURVEY: 18 seeds, at most 64 attacks each; no retreat or re-preparation");
    println!("loadout | cost | kills | exhausted | deaths | budget reached | attacks min..max");
    for maneuver in loadouts {
        let (mut kills, mut exhausted, mut deaths, mut budget) = (0, 0, 0, 0);
        let (mut minimum, mut maximum) = (usize::MAX, 0);
        for seed in SURVEY_SEEDS {
            let mut game = Game::composition_trial(seed, maneuver.clone()).unwrap();
            let sentinel = game.monsters()[0].id;
            let mut attempts = 0;
            let outcome = loop {
                if game.deaths() > 0 {
                    break 2;
                }
                if !game.monsters().iter().any(|m| m.id == sentinel) {
                    // Death and despawn must not masquerade as a victory.
                    assert!(game.log().entries().any(|e| e.kind == LogKind::Kill && e.text.contains("trial sentinel")));
                    break 0;
                }
                if attempts == 64 {
                    break 3;
                }
                match game.step(Dir::East) {
                    Step::Attacked => attempts += 1,
                    Step::Blocked => break 1,
                    other => panic!("sentinel did not stay adjacent: {other:?}, seed {seed}"),
                }
            };
            match outcome {
                0 => kills += 1,
                1 => exhausted += 1,
                2 => deaths += 1,
                _ => budget += 1,
            }
            minimum = minimum.min(attempts);
            maximum = maximum.max(attempts);
        }
        assert_eq!(kills + exhausted + deaths + budget, SURVEY_SEEDS.len());
        println!(
            "{} | {} | {kills} | {exhausted} | {deaths} | {budget} | {minimum}..{maximum}",
            maneuver.name,
            maneuver.technique.focus_cost()
        );
    }
}

/// Probe three simple escape routes. A failed bounded policy is not proof no escape route exists.
#[test]
fn survey_retreat_after_exhaustion_in_both_demos() {
    println!("\nRETREAT SURVEY: 18 seeds per row, at most 32 straight moves after the initial attacks");
    println!(
        "demo/direction | lost original tracking | safe to prepare | deaths | terrain/occupancy stop | moved min..max"
    );
    for inferno in [false, true] {
        for direction in [Dir::West, Dir::North, Dir::South] {
            let (mut lost, mut safe, mut deaths, mut stopped) = (0, 0, 0, 0);
            let (mut minimum, mut maximum) = (usize::MAX, 0);
            let (mut returned, mut recovered, mut return_deaths, mut return_budget) = (0, 0, 0, 0);
            for seed in SURVEY_SEEDS {
                let mut game = if inferno {
                    Game::inferno_trial(seed).unwrap()
                } else {
                    Game::composition_trial(seed, Maneuver::backstab_burst()).unwrap()
                };
                if inferno {
                    game.unleash();
                } else {
                    for _ in 0..3 {
                        game.step(Dir::East);
                    }
                }
                let sentinel = game.monsters().iter().find(|m| m.name.contains("trial sentinel")).unwrap().id;
                let (mut lost_tracking, mut could_prepare, mut moves) = (false, false, 0);
                let mut first_safe = None;
                for _ in 0..32 {
                    let (x, y) = game.position();
                    let (dx, dy) = direction.delta();
                    // Do not turn a retreat into an accidental staff discharge or attack.
                    if game.monster_at(x + dx, y + dy).is_some() || !game.can_step(direction) {
                        stopped += 1;
                        break;
                    }
                    assert_eq!(game.step(direction), Step::Moved);
                    moves += 1;
                    if game.deaths() > 0 {
                        deaths += 1;
                        break;
                    }
                    lost_tracking |= game.monsters().iter().find(|m| m.id == sentinel).is_none_or(|m| !m.alerted);
                    could_prepare |= game.can_prepare();
                    if first_safe.is_none() && game.can_prepare() {
                        first_safe = Some(game.clone());
                    }
                }
                lost += usize::from(lost_tracking);
                safe += usize::from(could_prepare);
                minimum = minimum.min(moves);
                maximum = maximum.max(moves);
                // Even a successful retreat cannot recover Focus away from home.
                if game.deaths() == 0 && game.position() != game.home() {
                    let focus = game.wanderer().focus;
                    assert_eq!(game.recover_focus(), Step::Blocked);
                    assert_eq!(game.wanderer().focus, focus);
                }
                if let Some(mut escape) = first_safe {
                    let distance = moves_from_home(&escape);
                    let focus = escape.wanderer().focus;
                    escape
                        .prepare(Maneuver {
                            name: "Retreat and refit".into(),
                            weapon: Weapon::Club,
                            technique: Technique::default(),
                        })
                        .unwrap();
                    assert_eq!(escape.wanderer().focus, focus, "preparation is not recovery");
                    assert_eq!(escape.recover_focus(), Step::Blocked, "safe ground is not necessarily home");
                    let reverse = match direction {
                        Dir::North => Dir::South,
                        Dir::South => Dir::North,
                        Dir::East => Dir::West,
                        Dir::West => Dir::East,
                    };
                    // Retrace the walked route with the free club, fighting anything blocking it.
                    // Keep failures separate from success, including the explicit action budget.
                    for _ in 0..128 {
                        if escape.position() == escape.home() || escape.deaths() > 0 {
                            break;
                        }
                        assert_ne!(escape.step(reverse), Step::Blocked);
                    }
                    if escape.deaths() > 0 {
                        return_deaths += 1;
                    } else if escape.position() == escape.home() {
                        returned += 1;
                        let success = escape.recover_focus() == Step::Waited;
                        recovered += usize::from(success && escape.deaths() == 0);
                        if success {
                            assert_eq!(escape.wanderer().focus, MAX_FOCUS);
                        }
                        println!(
                            "  return seed={seed}, first safe={distance} tiles from home, home turn={}, recovery={success}",
                            escape.turn()
                        );
                    } else {
                        return_budget += 1;
                    }
                }
            }
            println!(
                "{}/{direction:?} | {lost} | {safe} | {deaths} | {stopped} | {minimum}..{maximum}",
                if inferno { "staff" } else { "dagger" }
            );
            assert_eq!(returned + return_deaths + return_budget, safe);
            println!(
                "  from first safe point: returned={returned}, recovered={recovered}, deaths={return_deaths}, budget reached={return_budget}"
            );
        }
    }
}

fn moves_from_home(game: &Game) -> i64 {
    let (x, y) = game.position();
    let (hx, hy) = game.home();
    (x - hx).abs() + (y - hy).abs()
}

#[test]
fn exhausted_dagger_can_walk_but_a_same_speed_pursuer_stays_close() {
    let mut game = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
    for _ in 0..3 {
        game.step(Dir::East);
    }
    let sentinel = game.monsters()[0].id;
    let hp = game.wanderer().hp;
    for _ in 0..2 {
        assert_eq!(act(&mut game, "retreat west", |g| g.step(Dir::West)), Step::Moved);
        assert_eq!(game.wanderer().focus, 3, "ordinary movement is still free");
        let pursuer = game.monsters().iter().find(|m| m.id == sentinel).unwrap();
        assert!(pursuer.alerted);
        assert_eq!(pursuer.position, (game.position().0 + 1, game.position().1));
        assert!(!game.can_prepare());
    }
    assert_eq!(game.wanderer().hp, hp, "moving away avoids this pursuer's attack, but not its tracking");
    blocked(&mut game, "recover after retreat", Game::recover_focus);
}

#[test]
fn waiting_for_death_restores_the_resources_that_waiting_alive_does_not() {
    let mut game = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
    for _ in 0..3 {
        game.step(Dir::East);
    }
    let exhausted_turn = game.turn();
    for _ in 0..64 {
        if game.deaths() > 0 {
            break;
        }
        let focus = game.wanderer().focus;
        act(&mut game, "wait under attack", Game::wait);
        if game.deaths() == 0 {
            assert_eq!(game.wanderer().focus, focus);
        }
    }
    assert_eq!(game.deaths(), 1);
    assert_eq!(game.position(), game.home());
    assert_eq!(game.wanderer().focus, MAX_FOCUS);
    assert_eq!(game.wanderer().hp, game.wanderer().max_hp());
    assert!(game.monsters().is_empty());
    assert!(game.can_prepare());
    assert_eq!(game.maneuver(), &Maneuver::backstab_burst());
    println!(
        "Death recovery: {} turns elapsed since exhaustion, including the death penalty",
        game.turn() - exhausted_turn
    );
}

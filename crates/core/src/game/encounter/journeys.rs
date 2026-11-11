//! Engine acceptance routes, not pacing or human-play evidence. Only mature development is injected,
//! at home before departure; travel, combat, mechanisms, recovery, and claims use public actions.
//! Routing knows terrain and current actors, so these are not exploration-policy measurements.

use super::*;
use crate::{Aim, Delivery, Rank, Technique, Weapon};

fn reload(game: &mut Game) {
    let json = serde_json::to_vec(&game.snapshot()).unwrap();
    let restored = Game::from_snapshot(serde_json::from_slice(&json).unwrap()).unwrap();
    assert_eq!(*game, restored);
    *game = restored;
}

fn walk(game: &mut Game, goal: Point) {
    for _ in 0..160 {
        if game.position() == goal {
            return;
        }
        // Avoid known creatures and committed lanes. Replan after every actual world turn.
        let start = game.position();
        let sentries = game.encounter_actor_ids();
        let pursuer = Dir::ALL.into_iter().find(|d| {
            let (dx, dy) = d.delta();
            game.monster_at(start.0 + dx, start.1 + dy).is_some_and(|m| m.alerted && !sentries.contains(&m.id))
        });
        if game.wanderer().hp <= 12 && game.salves() > 0 {
            assert_eq!(game.use_salve(), Step::Waited);
            continue;
        }
        if let Some(dir) = pursuer.filter(|_| game.encounter_danger_at(start) == 0) {
            // Fight back against a following ambient creature rather than dragging it along.
            assert_eq!(game.step(dir), Step::Attacked);
            assert_eq!(game.deaths(), 0);
            continue;
        }
        let mut seen = HashSet::from([start]);
        let mut queue = VecDeque::from([(start, None)]);
        let mut found = None;
        while let Some((at, first)) = queue.pop_front() {
            if at == goal {
                found = first;
                break;
            }
            for dir in Dir::ALL {
                let (dx, dy) = dir.delta();
                let next = (at.0 + dx, at.1 + dy);
                if chebyshev(next, start) > 80
                    || !game.world().tile(next.0, next.1).biome.biome().passable
                    || game.monster_at(next.0, next.1).is_some()
                    || game.encounter_danger_at(next) > 0
                    || !seen.insert(next)
                {
                    continue;
                }
                queue.push_back((next, first.or(Some(dir))));
            }
        }
        let Some(dir) = found else {
            // A creature can stand on the destination: strike it when adjacent, else wait. A committed
            // lane can cover it: hold a safe tile until the shot clears.
            let lane = game.encounter_danger_at(goal) > 0 && game.encounter_danger_at(start) == 0;
            assert!(lane || game.monster_at(goal.0, goal.1).is_some(), "no route from {start:?} to {goal:?}");
            if lane {
                game.wait();
            } else if game.wanderer().hp <= 12 && game.salves() > 0 {
                assert_eq!(game.use_salve(), Step::Waited);
            } else if let Some(dir) = Dir::ALL.into_iter().find(|d| {
                let (dx, dy) = d.delta();
                (start.0 + dx, start.1 + dy) == goal
            }) {
                assert_eq!(game.step(dir), Step::Attacked);
            } else {
                game.wait();
            }
            assert_eq!(game.deaths(), 0);
            continue;
        };
        assert_eq!(game.step(dir), Step::Moved);
        assert_eq!(
            game.deaths(),
            0,
            "route died toward {goal:?}: {:#?}",
            game.log().lines().rev().take(14).collect::<Vec<_>>()
        );
        reload(game);
    }
    panic!("route exhausted its movement bound");
}

fn approach(weapon: Weapon, magical: bool) -> Game {
    let mut game = Game::new(42);
    // Explicit advanced fixture, not a claim about fresh-character progression.
    if weapon == Weapon::Club {
        game.wanderer.weapons.learn(weapon, 20_000, 200);
    }
    if magical {
        game.wanderer.focus_skill.learn(20_000, Rank::Expert);
        game.wanderer.focus = game.wanderer.max_focus();
    }
    let maneuver = if magical {
        Maneuver {
            name: "Contact force".into(),
            weapon,
            technique: Technique {
                delivery: Delivery::Touch,
                aim: Aim::Direct,
                burst: Some(Burst {
                    power: 6,
                    trigger: Trigger::Contact,
                    element: Element::Force,
                    shape: Shape::Point,
                }),
            },
        }
    } else {
        Maneuver { weapon, ..Maneuver::default() }
    };
    game.prepare(maneuver).unwrap();
    assert!(game.chart_encounter());
    let entrance = game.encounter().unwrap().definition.entrance();
    // The approach is a route fixture, not an uninformed human policy. Sneak near the site, not for
    // the entire trip, and carry the normal starting salves.
    walk(&mut game, entrance);
    assert_eq!(game.encounter().unwrap().phase, EncounterPhase::Revealed);
    game.toggle_sneak();
    assert_eq!(game.use_site(), Step::Waited);
    reload(&mut game);
    game
}

/// Returns dodges, attacks, and whether the warden died; a magical route stops when out of Focus.
fn fight(game: &mut Game, magical: bool) -> (usize, usize, bool) {
    if game.sneaking() {
        game.toggle_sneak();
    }
    let mut dodges = 0;
    let mut attacks = 0;
    for _ in 0..100 {
        let info = game.encounter().unwrap();
        let warden = info.sentries.iter().find(|s| s.kind == SentryKind::Warden).unwrap();
        if warden.hp == 0 {
            return (dodges, attacks, true);
        }
        let pos = game.monsters().iter().find(|m| m.id == warden.id).unwrap().position;
        let at = game.position();
        if game.encounter_danger_at(at) > 0 {
            let dir = Dir::ALL
                .into_iter()
                .find(|d| {
                    let (dx, dy) = d.delta();
                    let next = (at.0 + dx, at.1 + dy);
                    game.world().tile(next.0, next.1).biome.biome().passable
                        && game.monster_at(next.0, next.1).is_none()
                        && game.encounter_danger_at(next) == 0
                })
                .expect("a marked lane leaves a safe side step");
            assert_eq!(game.step(dir), Step::Moved);
            dodges += 1;
        } else if manhattan(at, pos) == 1 {
            if magical && game.wanderer().focus < game.maneuver().technique.focus_cost() {
                return (dodges, attacks, false);
            } else if matches!(warden.cycle, SentryCycle::Exposed(_)) {
                let dir = Dir::ALL
                    .into_iter()
                    .find(|d| {
                        let (dx, dy) = d.delta();
                        (at.0 + dx, at.1 + dy) == pos
                    })
                    .unwrap();
                let step = if magical { game.use_technique(dir) } else { game.step(dir) };
                assert_eq!(step, Step::Attacked);
                attacks += 1;
            } else {
                game.wait();
            }
        } else {
            // The north approach leaves the tender's western firing lane behind.
            walk(game, (pos.0, pos.1 - 1));
        }
        assert_eq!(game.deaths(), 0);
        reload(game);
    }
    panic!("fight exhausted its action bound");
}

#[test]
fn three_identities_use_distinct_opportunities_in_the_same_immutable_site() {
    let mut stealth = approach(Weapon::Dagger, false);
    let definition = stealth.encounter().unwrap().definition;
    walk(&mut stealth, definition.salvage());
    assert_eq!(stealth.use_site(), Step::Waited);
    assert!(stealth.has_relic(Relic::Hushstone));
    assert!(stealth.encounter().unwrap().sentries.iter().all(|s| s.hp > 0));
    reload(&mut stealth);
    // Leave the site before dropping concealment. Both exceptional actors must survive despawn.
    walk(&mut stealth, definition.entrance());
    stealth.toggle_sneak();
    let home = stealth.home();
    walk(&mut stealth, home);
    assert_eq!(stealth.recover_focus(), Step::Waited);
    assert_eq!(stealth.encounter().unwrap().definition, definition);
    assert!(stealth.encounter().unwrap().sentries.iter().all(|s| s.hp > 0));

    let mut force = approach(Weapon::Club, false);
    assert_eq!(force.encounter().unwrap().definition, definition);
    // Close unseen, then strike openly so the kill reflects force rather than an opening bonus.
    walk(&mut force, (definition.origin.0 - 4, definition.origin.1));
    force.toggle_sneak();
    assert!(!force.sneaking());
    let mut strikes = 0;
    while force.encounter().unwrap().sentries[0].hp > 0 {
        assert_eq!(force.step(Dir::East), Step::Attacked);
        strikes += 1;
        assert!(strikes <= 2);
    }
    let (dodges, attacks, won) = fight(&mut force, false);
    assert!(won && dodges > 0 && attacks > 0);
    assert_eq!(force.encounter().unwrap().braziers, [true; 2]);
    walk(&mut force, definition.origin);
    assert_eq!(force.use_site(), Step::Waited);
    assert!(force.has_relic(Relic::HearthEmber));
    reload(&mut force);

    let mut magic = approach(Weapon::Staff, true);
    assert_eq!(magic.encounter().unwrap().definition, definition);
    for brazier in definition.braziers() {
        walk(&mut magic, brazier);
        assert_eq!(magic.use_site(), Step::Waited);
        reload(&mut magic);
    }
    assert_eq!(magic.encounter().unwrap().braziers, [false; 2]);
    // The arc does not reset on retreat: return after home rest to the same wounds and shut braziers.
    let mut casts = 0;
    let mut trips = 0;
    loop {
        let (_, attacks, won) = fight(&mut magic, true);
        casts += attacks;
        if won {
            break;
        }
        trips += 1;
        assert!(trips <= 3, "magic route made no progress");
        let wounded = magic.encounter().unwrap().sentries[1].hp;
        assert!(wounded < SentryKind::Warden.health());
        let home = magic.home();
        walk(&mut magic, home);
        while magic.recover_focus() == Step::Waited {}
        let info = magic.encounter().unwrap();
        assert_eq!(
            (info.phase, info.braziers, info.sentries[1].hp),
            (EncounterPhase::Investigated, [false; 2], wounded)
        );
        assert!(info.retreats > 0);
        walk(&mut magic, definition.entrance());
    }
    assert!(casts > 0 && trips > 0);
    assert_eq!(magic.wanderer().weapons.get(Weapon::Staff).rank(), Rank::Novice);
    walk(&mut magic, definition.origin);
    assert_eq!(magic.use_site(), Step::Waited);
    reload(&mut magic);
    walk(&mut magic, definition.entrance());
    // A real aftermath interaction, not a terminal-state injection.
    assert_eq!(magic.use_site(), Step::Waited);
    assert_eq!((magic.salves(), magic.draughts()), (SALVE_CAPACITY, DRAUGHT_CAPACITY));
    reload(&mut magic);
    assert_eq!(magic.use_site(), Step::Blocked);
    assert_eq!(magic.encounter().unwrap().definition, definition);
}

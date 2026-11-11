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
    for _ in 0..400 {
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
        if let Some(dir) = pursuer.filter(|_| game.encounter_danger_at(start) == 0 && game.wanderer().hp > 12) {
            // Fight back against a following ambient creature while healthy; otherwise keep moving.
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
        if game.sneaking() && game.wanderer().focus == 0 {
            // Out of Focus: sneaking would refuse the step, so walk openly.
            game.toggle_sneak();
        }
        let Some(dir) = found else {
            // Lanes or creatures block every route: leave a lane, strike a creature on an adjacent
            // destination, or hold a safe tile until the shot clears or the creature moves.
            let safe_side = Dir::ALL.into_iter().find(|d| {
                let (dx, dy) = d.delta();
                let next = (start.0 + dx, start.1 + dy);
                game.world().tile(next.0, next.1).biome.biome().passable
                    && game.monster_at(next.0, next.1).is_none()
                    && game.encounter_danger_at(next) == 0
            });
            let onto_goal = Dir::ALL.into_iter().find(|d| {
                let (dx, dy) = d.delta();
                (start.0 + dx, start.1 + dy) == goal && game.monster_at(goal.0, goal.1).is_some()
            });
            if let Some(dir) = safe_side.filter(|_| game.encounter_danger_at(start) > 0) {
                assert_eq!(game.step(dir), Step::Moved);
            } else if let Some(dir) = onto_goal {
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
    panic!(
        "route exhausted its movement bound toward {goal:?}, at {:?}, home {:?}: {:#?}",
        game.position(),
        game.home(),
        game.log().lines().rev().take(8).collect::<Vec<_>>()
    );
}

fn approach(seed: u64, weapon: Weapon, magical: bool) -> Game {
    let mut game = Game::new(seed);
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
    if !game.sneaking() {
        game.toggle_sneak();
    }
    // An ambient creature on a sentry station blocks investigation without passing time; wait it out.
    for _ in 0..40 {
        if game.use_site() == Step::Waited {
            break;
        }
        game.wait();
        assert_eq!(game.deaths(), 0);
    }
    assert_eq!(game.encounter().unwrap().phase, EncounterPhase::Investigated);
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

fn identities(seed: u64) {
    let mut stealth = approach(seed, Weapon::Dagger, false);
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

    let mut force = approach(seed, Weapon::Club, false);
    assert_eq!(force.encounter().unwrap().definition, definition);
    // Close unseen, then strike openly so the kill reflects force rather than an opening bonus.
    walk(&mut force, (definition.origin.0 - 4, definition.origin.1));
    if force.sneaking() {
        force.toggle_sneak();
    }
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

    let mut magic = approach(seed, Weapon::Staff, true);
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

#[test]
fn three_identities_use_distinct_opportunities_in_the_same_immutable_site() {
    identities(42);
}

/// Shortest passable route from home, independent of creatures. None means the site is unreachable.
fn route_length(game: &Game, goal: Point) -> Option<usize> {
    let home = game.home();
    let mut seen = HashSet::from([home]);
    let mut queue = VecDeque::from([(home, 0)]);
    while let Some((at, n)) = queue.pop_front() {
        if at == goal {
            return Some(n);
        }
        for dir in Dir::ALL {
            let (dx, dy) = dir.delta();
            let next = (at.0 + dx, at.1 + dy);
            if chebyshev(next, home) <= ENCOUNTER_SEARCH_RADIUS + 8
                && game.world().tile(next.0, next.1).biome.biome().passable
                && seen.insert(next)
            {
                queue.push_back((next, n + 1));
            }
        }
    }
    None
}

/// Run: `cargo test -p blah-core --release cinder_watch_placement_probe -- --ignored --nocapture`.
#[test]
#[ignore = "diagnostic probe over many seeds"]
fn cinder_watch_placement_probe() {
    let seeds = 200;
    let (mut missing, mut unreachable, mut lengths) = (Vec::new(), Vec::new(), Vec::new());
    for seed in 0..seeds {
        let mut game = Game::new(seed);
        if !game.chart_encounter() {
            // Classify: an island too small to reach the minimum distance, or land without open ground.
            let home = game.home();
            let mut seen = HashSet::from([home]);
            let mut queue = VecDeque::from([home]);
            let mut far = 0;
            while let Some(at) = queue.pop_front() {
                far = far.max(chebyshev(at, home));
                for dir in Dir::ALL {
                    let (dx, dy) = dir.delta();
                    let next = (at.0 + dx, at.1 + dy);
                    if chebyshev(next, home) <= ENCOUNTER_SEARCH_RADIUS
                        && is_land(game.world().tile(next.0, next.1).biome.biome())
                        && seen.insert(next)
                    {
                        queue.push_back(next);
                    }
                }
            }
            missing.push((seed, seen.len(), far));
            continue;
        }
        let def = game.encounter().unwrap().definition;
        // Every authored object, not just the entrance, must be standable.
        let objects = [def.entrance(), def.origin, def.salvage(), def.braziers()[0], def.braziers()[1]];
        match objects.iter().map(|&p| route_length(&game, p)).collect::<Option<Vec<_>>>() {
            Some(n) => lengths.push(n[0]),
            None => unreachable.push(seed),
        }
    }
    lengths.sort_unstable();
    let pick = |q: usize| lengths.get(lengths.len() * q / 100).copied().unwrap_or(0);
    println!("seeds {seeds}: no site {} {missing:?}", missing.len());
    println!("object unreachable on foot {} {unreachable:?}", unreachable.len());
    println!("route to stone: min {} p50 {} p90 {} max {}", pick(0), pick(50), pick(90), lengths.last().unwrap_or(&0));
}

/// Run: `CINDER_SEEDS=0..12 cargo test -p blah-core --release cinder_watch_identity_probe -- --ignored --nocapture`.
#[test]
#[ignore = "diagnostic probe over many seeds"]
fn cinder_watch_identity_probe() {
    let range = std::env::var("CINDER_SEEDS").unwrap_or_else(|_| "0..12".into());
    let (a, b) = range.split_once("..").expect("CINDER_SEEDS is start..end");
    let (a, b): (u64, u64) = (a.parse().unwrap(), b.parse().unwrap());
    let mut failed = Vec::new();
    for seed in a..b {
        if !Game::new(seed).chart_encounter() {
            println!("seed {seed}: no site");
            continue;
        }
        match std::panic::catch_unwind(|| identities(seed)) {
            Ok(()) => println!("seed {seed}: ok"),
            Err(e) => {
                let why = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| (*s).into()));
                println!("seed {seed}: FAILED {}", why.unwrap_or_default().lines().next().unwrap_or(""));
                failed.push(seed);
            }
        }
    }
    println!("failed {failed:?}");
}

#[test]
fn a_fresh_character_is_warned_sees_every_shot_coming_and_can_walk_away() {
    let mut game = Game::new(42);
    assert!(game.chart_encounter());
    let def = game.encounter().unwrap().definition;
    let full = game.wanderer().max_hp();
    // The strongest shot cannot end a full-health fresh character in one turn.
    let shot = Burst { power: 6, trigger: Trigger::Contact, element: Element::Fire, shape: Shape::Point };
    assert!(shot.damage(game.wanderer().stats.defense, 0, false, false) < full);
    walk(&mut game, def.entrance());
    assert_eq!(game.encounter().unwrap().phase, EncounterPhase::Revealed);
    for _ in 0..40 {
        if game.use_site() == Step::Waited {
            break;
        }
        game.wait();
    }
    let warned = game.log().lines().rev().take(12).collect::<Vec<_>>().join(" ");
    assert!(warned.contains("54-Health warden") && warned.contains("12 fire damage"), "{warned}");
    // The tender may start drawing at the stone's edge, but nothing is aimed yet and the warden sleeps.
    let info = game.encounter().unwrap();
    assert!(info.sentries.iter().all(|s| !matches!(s.cycle, SentryCycle::Aiming(_))));
    assert_eq!(info.sentries[1].cycle, SentryCycle::Dormant);

    // Naive commitment: stand in the open and press east toward the warden, ignoring lanes.
    let mut hits = 0;
    for _ in 0..30 {
        if game.wanderer().hp <= 16 {
            break;
        }
        let aimed = game.encounter().unwrap().sentries.iter().any(|s| matches!(s.cycle, SentryCycle::Aiming(_)));
        let seen = game.log().total();
        if game.step(Dir::East) == Step::Blocked {
            game.wait();
        }
        let fresh = (game.log().total() - seen) as usize;
        let burned = game.log().lines().rev().take(fresh).any(|line| line.contains("marked fire lane hits you"));
        if burned {
            hits += 1;
            assert!(aimed, "fire hit without a lane aimed and drawn before the action");
        }
        assert_eq!(game.deaths(), 0);
    }
    assert!(hits > 0, "the naive route should feel the cycle");

    // Walk away: leaving the site cancels pending fire and keeps the site's state.
    let away = (def.entrance().0 - 4, def.entrance().1);
    walk(&mut game, away);
    assert_eq!(game.deaths(), 0);
    let info = game.encounter().unwrap();
    assert_eq!(info.phase, EncounterPhase::Investigated);
    assert!(info.retreats > 0);
    assert!(info.sentries.iter().all(|s| s.cycle == SentryCycle::Dormant));
    assert_eq!(info.definition, def);
    reload(&mut game);
}

#[test]
fn island_starts_wade_to_a_site_on_dry_ground_that_survives_reload() {
    let mut game = Game::new(1);
    assert!(Game::cinder_watch_search(game.world(), game.home(), is_land).is_none(), "seed 1 is an island start");
    assert!(game.chart_encounter());
    let def = game.encounter().unwrap().definition;
    for p in [def.entrance(), def.origin, def.salvage(), def.braziers()[0], def.braziers()[1]] {
        assert!(is_land(game.world().tile(p.0, p.1).biome.biome()));
        assert!(route_length(&game, p).is_some());
    }
    assert_eq!(Game::from_snapshot(game.snapshot()).unwrap(), game);
}

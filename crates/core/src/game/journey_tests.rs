use super::*;
use crate::Weapon;
use std::collections::{HashMap, HashSet, VecDeque};

fn next_step(game: &Game, goal: (i64, i64)) -> Option<Dir> {
    let start = game.position;
    if start == goal {
        return None;
    }
    let mut queue = VecDeque::from([start]);
    let mut previous = HashMap::from([(start, (start, Dir::North))]);
    while let Some(here) = queue.pop_front() {
        for dir in Dir::ALL {
            let (dx, dy) = dir.delta();
            let next = (here.0 + dx, here.1 + dy);
            if (next.0 - start.0).abs() > 96
                || (next.1 - start.1).abs() > 96
                || previous.contains_key(&next)
                || !game.world.tile(next.0, next.1).biome.biome().passable
                || (next != goal && game.monster_at(next.0, next.1).is_some())
            {
                continue;
            }
            previous.insert(next, (here, dir));
            if next == goal {
                let mut cursor = next;
                while previous[&cursor].0 != start {
                    cursor = previous[&cursor].0;
                }
                return Some(previous[&cursor].1);
            }
            queue.push_back(next);
        }
    }
    None
}

fn current_region_cache(game: &mut Game) -> RegionId {
    let region = game.region.clone();
    let id = region.id;
    if !game.caches.contains_key(&id) {
        let position =
            Game::cache_site(&game.world, &mut game.regions, &region, game.home).expect("starting region is land");
        game.reveal_cache(&region, position);
    }
    id
}

#[test]
fn fresh_sneaking_route_claims_a_cache_with_every_guard_alive() {
    assert!(fresh_sneaking_claim(0), "fresh seed 0 has no unnoticed route to its guarded cache");
}

fn fresh_sneaking_claim(seed: u64) -> bool {
    let mut game = Game::new(seed);
    let region = game.region.clone();
    let Some(site) = Game::cache_site(&game.world, &mut game.regions, &region, game.home) else {
        return false;
    };
    if game.toggle_sneak() != Step::Waited {
        return false;
    }
    for _ in 0..32 {
        if game.position == site {
            break;
        }
        let Some(dir) = next_step(&game, site) else {
            return false;
        };
        if game.step(dir) != Step::Moved {
            return false;
        }
    }
    if game.position != site || !game.sneaking() {
        return false;
    }
    let Some(info) = game.cache_at(site.0, site.1) else {
        return false;
    };
    if !(2..=3).contains(&info.guards_alive) || info.guards != info.guards_alive {
        return false;
    }
    if game.log.entries().any(|entry| entry.kind == LogKind::Hurt) {
        return false;
    }
    if game.claim_cache() != Step::Waited {
        return false;
    }
    let claimed = game.cache_at(site.0, site.1).unwrap();
    claimed.claimed && claimed.guards_alive == claimed.guards && !game.sneaking() && game.wanderer.hp > 0
}

/// Every sampled land region gets a guarded cache, except a tiny pocket lying inside the home view, where
/// no guard could stand out of sight of home.
#[test]
fn sampled_land_regions_get_a_cache_unless_they_sit_inside_the_home_view() {
    for seed in 0..64 {
        let mut game = Game::new(seed);
        let mut ids = game.regions.region_neighbors(game.region.id);
        ids.push(game.region.id);
        for id in ids {
            let region = game.regions.region(id).unwrap();
            if !is_land(region.biome.biome()) {
                continue;
            }
            let site = Game::cache_site(&game.world, &mut game.regions, &region, game.home)
                .unwrap_or_else(|| panic!("seed {seed}, region {id:?} has no cache site"));
            assert_eq!(game.regions.region_at(site.0, site.1), id);
            assert!(game.world.tile(site.0, site.1).biome.biome().passable);
            game.monsters.clear();
            game.position = site;
            game.reveal_cache(&region, site);
            let Some(cache) = game.caches.get(&id) else {
                assert!(
                    region.tiles < crate::region::SLIVER_TILES && chebyshev(site, game.home) <= VIEW_RADIUS,
                    "seed {seed}, region {id:?} ({} tiles) has no guarded cache",
                    region.tiles
                );
                continue;
            };
            assert!((2..=3).contains(&cache.guards.len()), "seed {seed}, region {id:?}");
            assert!(cache.guards.iter().all(|guard| chebyshev(guard.position, game.home) > VIEW_RADIUS));
        }
    }
}

#[test]
fn cache_sites_and_guards_are_deterministic_and_territorial() {
    for seed in [0, 4, 42] {
        let mut first = Game::new(seed);
        let mut second = Game::new(seed);
        current_region_cache(&mut first);
        current_region_cache(&mut second);
        assert_eq!(first.caches, second.caches);
        for cache in first.caches.values() {
            let region = first.regions.region(cache.region).unwrap();
            assert_eq!(Game::cache_site(&first.world, &mut first.regions, &region, first.home), Some(cache.position));
            assert!(is_land(first.world.tile(cache.position.0, cache.position.1).biome.biome()));
            assert!((2..=3).contains(&cache.guards.len()));
            for guard in &cache.guards {
                assert_eq!(guard.species.region, cache.region);
                assert_eq!(guard.phenotype.temperament, Temperament::Territorial);
                assert!(chebyshev(guard.position, cache.position) <= TERRITORY);
                if guard.hp > 0 {
                    let monster = first.monsters.iter().find(|monster| monster.id == guard.id).unwrap();
                    assert_eq!(monster.home, cache.position);
                    assert_eq!(monster.hp, guard.hp);
                }
            }
        }
    }
}

#[test]
fn cache_claim_is_one_time_and_survives_save_load_and_death() {
    let mut game = Game::new(42);
    let region = current_region_cache(&mut game);
    let cache = game.caches[&region].clone();
    game.position = cache.position;
    game.update_region(false);
    game.salves = 0;
    let treasure = cache.treasure;

    assert_eq!(game.claim_cache(), Step::Waited);
    assert_eq!(game.salves, 1);
    assert_eq!(game.inventory.iter().filter(|&&item| item == treasure).count(), 1);
    let turn = game.turn;
    assert_eq!(game.claim_cache(), Step::Blocked);
    assert_eq!(game.turn, turn);
    assert_eq!(game.inventory.iter().filter(|&&item| item == treasure).count(), 1);
    let snapshot = game.snapshot();
    let encoded = serde_json::to_value(&snapshot).unwrap();
    let encoded_cache = &encoded["caches"][0];
    assert!(encoded_cache.get("position").is_none());
    assert!(encoded_cache.get("treasure").is_none());
    let restored = Game::from_snapshot(snapshot).unwrap();
    assert_eq!(restored, game);

    game.die();
    assert!(game.inventory.contains(&treasure));
    let after_death = Game::from_snapshot(game.snapshot()).unwrap();
    assert_eq!(after_death, game);
    assert_eq!(after_death.inventory.iter().filter(|&&item| item == treasure).count(), 1);
}

#[test]
fn cache_guard_health_survives_despawn_and_is_restored_near_its_site() {
    let mut game = Game::new(42);
    let region = current_region_cache(&mut game);
    let cache = game.caches[&region].clone();
    let guard = cache.guards.iter().find(|guard| guard.hp > 0).unwrap().clone();
    let monster = game.monsters.iter_mut().find(|monster| monster.id == guard.id).unwrap();
    monster.hp -= 2;
    let remaining = monster.hp;
    game.sync_cache_guards();

    game.caches.retain(|id, _| *id == region);
    let ids: HashSet<_> = cache.guards.iter().map(|guard| guard.id).collect();
    game.monsters.retain(|monster| ids.contains(&monster.id));
    game.position = (cache.position.0 + DESPAWN_RADIUS + TERRITORY + 2, cache.position.1);
    game.update_region(false);
    game.sync_cache_guards();
    game.monsters.retain(|monster| chebyshev(monster.position, game.position) <= DESPAWN_RADIUS);
    assert!(!game.monsters.iter().any(|monster| monster.id == guard.id));
    assert_eq!(game.caches[&region].guards.iter().find(|saved| saved.id == guard.id).unwrap().hp, remaining);

    let restored = Game::from_snapshot(game.snapshot()).unwrap();
    assert_eq!(restored, game);
    game.position = cache.position;
    game.update_region(false);
    game.update_visible_caches();
    let recovered = game.monsters.iter().find(|monster| monster.id == guard.id).unwrap();
    assert_eq!(recovered.hp, remaining);
    assert_eq!(recovered.home, cache.position);
}

#[test]
fn killed_cache_guard_stays_dead_through_save_load_and_reentry() {
    let mut game = Game::new(0);
    let region = current_region_cache(&mut game);
    let cache = game.caches[&region].clone();
    let guard = cache.guards.iter().find(|guard| guard.hp > 0).unwrap().clone();
    let monster = game.monsters.iter_mut().find(|monster| monster.id == guard.id).unwrap();
    monster.hp = 1;
    game.sync_cache_guards();

    let target = game.monsters.iter().find(|monster| monster.id == guard.id).unwrap();
    let approach = Dir::ALL
        .into_iter()
        .find_map(|dir| {
            let (dx, dy) = dir.delta();
            let at = (target.position.0 - dx, target.position.1 - dy);
            (game.world.tile(at.0, at.1).biome.biome().passable && game.monster_at(at.0, at.1).is_none()).then_some(at)
        })
        .expect("a guard has an adjacent passable tile");
    game.position = approach;
    game.update_region(false);
    assert_eq!(chebyshev(game.position, guard.position), 1);

    let maneuver = game.maneuver.clone();
    for seed in 0..64 {
        let Some(index) = game.monsters.iter().position(|monster| monster.id == guard.id) else {
            break;
        };
        game.wanderer_attacks(index, &maneuver, &mut Rng::new(seed));
    }
    assert!(!game.monsters.iter().any(|monster| monster.id == guard.id), "a successful hit kills the weakened guard");
    let dead = game.caches[&region].guards.iter().find(|saved| saved.id == guard.id).unwrap();
    assert_eq!(dead.hp, 0);

    let restored = Game::from_snapshot(game.snapshot()).unwrap();
    assert_eq!(restored, game);
    game.sync_cache_guards();
    game.position = (cache.position.0 + DESPAWN_RADIUS + TERRITORY + 2, cache.position.1);
    game.update_region(false);
    game.monsters.retain(|monster| chebyshev(monster.position, game.position) <= DESPAWN_RADIUS);
    game.position = cache.position;
    game.update_region(false);
    game.update_visible_caches();
    assert!(!game.monsters.iter().any(|monster| monster.id == guard.id));
    let info = game.cache_at(cache.position.0, cache.position.1).unwrap();
    assert_eq!(info.guards_alive, info.guards - 1);
}

#[test]
fn starting_cache_guards_do_not_make_home_unsafe_after_death() {
    let mut game = Game::new(0);
    let region = current_region_cache(&mut game);
    let cache = game.caches[&region].clone();
    assert!(chebyshev(cache.position, game.home) > VIEW_RADIUS + TERRITORY);
    assert!(cache.guards.iter().all(|guard| chebyshev(guard.position, game.home) > VIEW_RADIUS));
    assert!(game.can_prepare());

    let guard_id = cache.guards[0].id;
    let guard = game.monsters.iter_mut().find(|monster| monster.id == guard_id).unwrap();
    guard.alerted = true;
    guard.tracking = true;
    game.sync_cache_guards();
    game.die();

    assert_eq!(game.position, game.home);
    assert!(game.can_prepare(), "cache guards remain outside the home view after waking");
    let saved_guard = game.caches[&region].guards.iter().find(|saved| saved.id == guard_id).unwrap();
    assert!(!saved_guard.alerted && !saved_guard.tracking);
    assert_eq!(Game::from_snapshot(game.snapshot()).unwrap(), game);
}

#[test]
fn fresh_home_is_not_blocked_by_a_visible_cache_guard() {
    for seed in [0, 1, 2, 3, 4, 5, 7, 42, 100, 999] {
        let game = Game::new(seed);
        assert!(game.can_prepare(), "seed {seed} starts with a creature in the safe home view");
    }
}

#[test]
fn regional_field_signs_do_not_reveal_guards_or_change_the_world() {
    let game = Game::new(42);
    assert!(game.caches.is_empty(), "the destination starts outside normal view");
    let before = game.clone();
    let trail = game.cache_trail().expect("starting cache has field signs");
    assert!(chebyshev(trail, game.position()) > VIEW_RADIUS);
    assert!(game.cache_at(trail.0, trail.1).is_none());
    for _ in 0..3 {
        assert_eq!(game.cache_trail(), Some(trail));
        for herb in Herb::ALL {
            if let Some(at) = game.herb_trail(herb) {
                assert_eq!(Herb::of(game.world.tile(at.0, at.1).biome.biome()), Some(herb));
            }
        }
    }
    assert_eq!(game, before);
    let restored = Game::from_snapshot(game.snapshot()).unwrap();
    assert_eq!(restored.cache_trail(), Some(trail));
    assert!(restored.caches.is_empty(), "restoring clues must not create guards");
    for herb in Herb::ALL {
        assert_eq!(restored.herb_trail(herb), game.herb_trail(herb));
    }
    let trial = Game::composition_trial(42, Maneuver::default()).unwrap();
    assert!(trial.cache_trail().is_none());
    assert!(Herb::ALL.into_iter().all(|herb| trial.herb_trail(herb).is_none()));
}

#[test]
fn cache_guard_placement_never_uses_the_wanderers_tile() {
    let mut game = Game::new(0);
    let region = game.region.clone();
    let site = Game::cache_site(&game.world, &mut game.regions, &region, game.home).unwrap();
    let (x, y) = region.id.anchor();
    let mut rng = Rng::keyed(game.world.seed(), b"cachegrd", &[x as u64, y as u64]);
    let _count = 2 + rng.below(2);
    let species = game.life(region.id).species.clone();
    let mut candidates: Vec<_> =
        (1..=TERRITORY).flat_map(|radius| ring(radius).map(move |(dx, dy)| (site.0 + dx, site.1 + dy))).collect();
    let first = rng.below(candidates.len());
    candidates.rotate_left(first);
    let weights: Vec<_> = species.iter().map(|source| source.abundance).collect();
    let source = &species[rng.weighted(&weights)];
    let genome = individual(&source.pool[rng.below(source.pool.len())], &mut rng);
    let mut phenotype = Phenotype::of(&genome);
    phenotype.temperament = Temperament::Territorial;
    let player_tile = candidates
        .into_iter()
        .find(|&(gx, gy)| {
            is_land(game.world.tile(gx, gy).biome.biome())
                && phenotype.access.allows(game.world.tile(gx, gy).biome.biome())
        })
        .expect("cache guard has another land tile in its territory");

    game.monsters.clear();
    game.caches.clear();
    game.position = player_tile;
    game.reveal_cache(&region, site);
    assert!(game.monsters.iter().all(|monster| monster.position != player_tile));
}

#[test]
fn camp_requires_no_creature_in_view_and_restores_without_shelter() {
    let mut threatened = Game::composition_trial(42, Maneuver::default()).unwrap();
    threatened.wanderer.hp -= 4;
    threatened.wanderer.focus -= 5;
    let before = (threatened.turn, threatened.wanderer.hp, threatened.wanderer.focus);
    assert_eq!(threatened.camp(), Step::Blocked);
    assert_eq!((threatened.turn, threatened.wanderer.hp, threatened.wanderer.focus), before);

    let mut camp = Game::new(0);
    camp.journey = false;
    camp.caches.clear();
    camp.monsters.clear();
    camp.wanderer.hp -= 4;
    camp.wanderer.focus -= 5;
    let before = (camp.turn, camp.wanderer.hp, camp.wanderer.focus);
    assert_eq!(camp.camp(), Step::Waited);
    assert_eq!(camp.turn, before.0 + 1);
    assert_eq!(camp.wanderer.hp, before.1 + CAMP_HEALTH);
    assert_eq!(camp.wanderer.focus, before.2 + CAMP_FOCUS);
}

#[test]
fn salves_work_in_combat_and_home_rest_restock_them() {
    let mut fight = Game::composition_trial(42, Maneuver::default()).unwrap();
    fight.wanderer.hp = 10;
    fight.salves = 1;
    fight.monsters[0].energy = 0;
    fight.monsters[0].phenotype.stats.speed = 0;
    let before_turn = fight.turn;
    assert_eq!(fight.use_salve(), Step::Waited);
    assert_eq!(fight.turn, before_turn + 1);
    assert_eq!(fight.wanderer.hp, 20);
    assert_eq!(fight.salves, 0);

    let mut home = Game::composition_trial(42, Maneuver::default()).unwrap();
    home.journey = false;
    home.caches.clear();
    home.monsters.clear();
    home.salves = 0;
    assert!(home.can_rest());
    assert_eq!(home.recover_focus(), Step::Waited);
    assert_eq!(home.salves, SALVE_CAPACITY);
}

#[test]
fn claiming_a_weapon_makes_its_tradeoff_change_the_attack_preview() {
    let mut game = Game::new(42);
    let region = current_region_cache(&mut game);
    let cache = game.caches[&region].clone();
    game.position = cache.position;
    game.update_region(false);
    let item = cache.treasure;
    assert_eq!(game.claim_cache(), Step::Waited);
    assert!(game.inventory.contains(&item));

    let mut target =
        game.monsters.iter().find(|monster| cache.guards.iter().any(|guard| guard.id == monster.id)).unwrap().clone();
    target.id = MonsterId(game.next_monster);
    game.next_monster += 1;
    target.position = (game.position.0 + 1, game.position.1);
    target.home = target.position;
    target.phenotype.sense = 0;
    target.phenotype.stats.defense = 0;
    target.phenotype.stats.hp = 80;
    target.hp = 80;
    target.practice_left = 80;

    let maneuver = Maneuver { weapon: item.weapon(), ..Maneuver::default() };
    let plain = WeaponItem::STARTING.into_iter().find(|owned| owned.weapon() == item.weapon()).unwrap();
    game.monsters.clear();
    game.prepare_with_item(maneuver.clone(), plain).unwrap();
    game.monsters.push(target.clone());
    let base = game.preview_strike(Dir::East).unwrap();

    game.monsters.clear();
    game.prepare_with_item(maneuver, item).unwrap();
    game.monsters.push(target);
    let equipped = game.preview_strike(Dir::East).unwrap();
    assert_ne!(base.max_hit, equipped.max_hit);
}

#[test]
fn weapon_items_require_ownership_and_a_safe_point_to_equip() {
    let mut game = Game::new(0);
    game.journey = false;
    game.caches.clear();
    game.monsters.clear();
    let club = Maneuver { weapon: Weapon::Club, ..Maneuver::default() };
    let original = game.maneuver.clone();
    assert_eq!(game.prepare_with_item(club.clone(), WeaponItem::HeavyClub), Err("You do not own that weapon."));
    assert_eq!(game.maneuver, original);

    game.inventory.push(WeaponItem::HeavyClub);
    assert_eq!(
        game.prepare_with_item(Maneuver::default(), WeaponItem::HeavyClub),
        Err("The equipped item does not match this maneuver's weapon family.")
    );
    game.prepare_with_item(club, WeaponItem::HeavyClub).unwrap();
    assert_eq!(game.equipped_item(), WeaponItem::HeavyClub);

    game.inventory.push(WeaponItem::QuietStaff);
    let staff = Maneuver { weapon: Weapon::Staff, ..Maneuver::default() };
    game.prepare_with_item(staff.clone(), WeaponItem::Staff).unwrap();
    game.sneaking = true;
    let plain_concealment = game.concealment();
    game.prepare_with_item(staff.clone(), WeaponItem::QuietStaff).unwrap();
    assert_eq!(game.concealment(), plain_concealment + 1);

    let mut threat = Game::composition_trial(42, Maneuver::default()).unwrap().monsters[0].clone();
    threat.position = (game.position.0 + 1, game.position.1);
    game.monsters.push(threat);
    game.position.1 += 1; // Away from home's shelter, nearby creatures still prevent equipment changes.
    let before = (game.maneuver.clone(), game.equipped_item(), game.turn);
    let mut changed = staff;
    changed.name = "Safe-point check".into();
    assert_eq!(game.prepare_with_item(changed, WeaponItem::QuietStaff), Err("Prepare away from nearby creatures."));
    assert_eq!((game.maneuver.clone(), game.equipped_item(), game.turn), before);
}

/// A fresh character who walks openly while nothing is in view, sneaks while something is, and camps to refill
/// Focus when the view is clear. Returns `None` when the starting region has no cache.
fn watchful_claim(seed: u64) -> Option<bool> {
    let mut game = Game::new(seed);
    let region = game.region.clone();
    let site = Game::cache_site(&game.world, &mut game.regions, &region, game.home)?;
    for _ in 0..200 {
        if game.position == site {
            break;
        }
        let watched = game.monsters_in_view().next().is_some();
        if !watched && game.wanderer.focus < game.wanderer.max_focus() / 2 {
            while game.monsters_in_view().next().is_none() && game.wanderer.focus < game.wanderer.max_focus() {
                assert_eq!(game.camp(), Step::Waited);
            }
            continue;
        }
        if watched != game.sneaking() {
            game.toggle_sneak();
            continue;
        }
        let dir = next_step(&game, site)?;
        if game.step(dir) != Step::Moved {
            return Some(false);
        }
    }
    let unharmed = !game.log.entries().any(|entry| entry.kind == LogKind::Hurt);
    let info = game.cache_at(site.0, site.1)?;
    Some(
        game.position == site
            && unharmed
            && info.guards_alive == info.guards
            && game.claim_cache() == Step::Waited
            && game.inventory.contains(&Game::cache_reward(seed, region.id)),
    )
}

#[test]
fn a_watchful_sneaker_claims_most_starting_caches_without_a_fight() {
    let results: Vec<_> = (0..48).filter_map(watchful_claim).collect();
    let claimed = results.iter().filter(|&&ok| ok).count();
    assert!(results.len() >= 44, "only {} of 48 seeds have a starting cache", results.len());
    assert!(claimed * 10 >= results.len() * 7, "{claimed} of {} unharmed stealth claims", results.len());
}

use super::*;

#[test]
fn charting_is_no_time_and_does_not_commit_actors_or_change_the_build() {
    let mut game = Game::new(42);
    let before = game.clone();
    assert!(game.chart_encounter());
    assert_eq!(game.turn, before.turn);
    assert_eq!(game.wanderer, before.wanderer);
    assert_eq!(game.monsters, before.monsters);
    let info = game.encounter().unwrap();
    assert_eq!(info.phase, EncounterPhase::Revealed);
    assert!(info.sentries.is_empty());
    assert!(chebyshev(info.definition.entrance(), game.home) > VIEW_RADIUS);
    assert_eq!(Game::from_snapshot(game.snapshot()).unwrap(), game);
}

// Isolated phase fixtures are not whole-journey evidence. Public-action journeys live in the example.
fn investigated() -> Game {
    let mut game = Game::new(42);
    assert!(game.chart_encounter());
    let def = game.encounter().unwrap().definition;
    game.position = def.entrance();
    game.update_region(false);
    game.sneaking = true;
    assert_eq!(game.use_site(), Step::Waited);
    game.sneaking = false;
    let ids = game.encounter_actor_ids();
    game.monsters.retain(|m| ids.contains(&m.id));
    game.caches.clear();
    game
}

fn at(game: &mut Game, offset: Point) {
    let origin = game.encounter().unwrap().definition.origin;
    game.position = (origin.0 + offset.0, origin.1 + offset.1);
    game.update_region(false);
}

fn warden(game: &Game) -> SentryInfo {
    game.encounter().unwrap().sentries.into_iter().find(|s| s.kind == SentryKind::Warden).unwrap()
}

fn aim_warden(game: &mut Game) {
    at(game, (0, -1));
    game.wait();
    assert_eq!(warden(game).cycle, SentryCycle::Drawing);
    game.wait();
    assert_eq!(warden(game).cycle, SentryCycle::Aiming(Dir::North));
}

fn round_trip(game: &Game) -> Game {
    let json = serde_json::to_vec(&game.snapshot()).unwrap();
    let loaded = Game::from_snapshot(serde_json::from_slice(&json).unwrap()).unwrap();
    assert_eq!(*game, loaded);
    loaded
}

#[test]
fn the_exact_marked_lane_fires_after_one_action_then_has_two_exposed_actions() {
    let mut game = investigated();
    aim_warden(&mut game);
    let origin = game.encounter().unwrap().definition.origin;
    let hp = game.wanderer.hp;
    assert_eq!(game.encounter_danger_at(game.position), 12);
    for n in 1..=4 {
        assert_eq!(game.encounter_danger_at((origin.0, origin.1 - n)), 12);
    }
    assert_eq!(game.encounter_danger_at((origin.0, origin.1 - 5)), 0);
    assert_eq!(game.encounter_danger_at((origin.0 + 1, origin.1 - 1)), 0);
    let mut hit = round_trip(&game);
    hit.wait();
    assert_eq!(hit.wanderer.hp, hp - 12);
    assert_eq!(warden(&hit).cycle, SentryCycle::Exposed(2));
    assert_eq!(game.step(Dir::East), Step::Moved);
    assert_eq!(game.wanderer.hp, hp);
    assert_eq!(warden(&game).defense, 0);
    assert_eq!(warden(&game).cycle, SentryCycle::Exposed(2));
    game.wait();
    assert_eq!(warden(&game).cycle, SentryCycle::Exposed(1));
    game.wait();
    assert_eq!(warden(&game).cycle, SentryCycle::Drawing);
    assert_eq!(warden(&game).defense, 8);
}

#[test]
fn pending_fire_and_every_cycle_round_trip_without_retaking_turns() {
    let mut game = investigated();
    round_trip(&game);
    at(&mut game, (0, -1));
    for _ in 0..5 {
        game = round_trip(&game);
        let mut restored = round_trip(&game);
        game.wait();
        restored.wait();
        assert_eq!(game, restored);
        if game.deaths > 0 {
            break;
        }
    }
}

#[test]
fn braziers_reduce_real_preview_armor_and_fire_without_requiring_a_specific_weapon() {
    let mut game = investigated();
    let definition = game.encounter().unwrap().definition;
    at(&mut game, (-1, -2));
    game.wanderer.focus = 3;
    let before = game.clone();
    assert_eq!(game.use_site(), Step::Blocked);
    assert_eq!(game.turn, before.turn);
    assert_eq!(game.wanderer, before.wanderer);
    assert_eq!(game.encounter, before.encounter);
    game.wanderer.focus = 24;
    game.sneaking = true;
    assert_eq!(game.use_site(), Step::Waited);
    assert_eq!(game.wanderer.focus, 22);
    assert_eq!(warden(&game).defense, 4);
    assert_eq!(game.encounter().unwrap().braziers, [false, true]);
    assert!(game.sneaking);
    at(&mut game, (1, 2));
    assert_eq!(game.use_site(), Step::Waited);
    assert_eq!(warden(&game).defense, 0);
    game.sneaking = false;
    aim_warden(&mut game);
    assert_eq!(game.encounter_danger_at(game.position), 4);
    assert_eq!(game.encounter().unwrap().definition, definition);
    round_trip(&game);
}

#[test]
fn a_real_shove_interrupts_fire_instead_of_moving_a_committed_lane() {
    let mut game = investigated();
    game.wanderer.weapons.learn(crate::Weapon::Club, 20_000, 200);
    game.position = game.home;
    game.update_region(false);
    game.prepare(Maneuver { weapon: crate::Weapon::Club, ..Maneuver::default() }).unwrap();
    aim_warden(&mut game);
    let before = game.wanderer.hp;
    // A miss must not interrupt; a landed shove must. This uses the real per-turn attack stream.
    for _ in 0..12 {
        if matches!(warden(&game).cycle, SentryCycle::Aiming(_)) {
            let preview = game.preview_move(crate::LearnedMove::Shove, Dir::South).unwrap();
            assert!(preview.push.is_some());
            game.use_move(crate::LearnedMove::Shove, Dir::South);
            if game.last_attack().unwrap().hit {
                assert!(matches!(warden(&game).cycle, SentryCycle::Exposed(_)));
                assert_eq!(game.wanderer.hp, before);
                assert_eq!(game.encounter_danger_at(game.position), 0);
                round_trip(&game);
                return;
            }
        } else {
            game.wait();
        }
    }
    panic!("fixture never landed its shove");
}

#[test]
fn retreat_and_death_preserve_wounds_shutdowns_and_finite_opportunities() {
    let mut game = investigated();
    let tender = game.encounter().unwrap().sentries[0].id;
    game.mark_guard_dead(tender);
    game.monsters.retain(|m| m.id != tender);
    let id = warden(&game).id;
    game.monsters.iter_mut().find(|m| m.id == id).unwrap().hp -= 7;
    aim_warden(&mut game);
    game.wanderer.hp = 1;
    let skills = game.wanderer.weapons;
    game.wait();
    assert_eq!(game.deaths, 1);
    assert_eq!(game.position, game.home);
    assert_eq!(game.wanderer.hp, 1);
    assert_eq!(game.wanderer.focus, 0);
    assert_eq!(game.wanderer.weapons, skills);
    assert_eq!(warden(&game).hp, 47);
    assert_eq!(warden(&game).cycle, SentryCycle::Dormant);
    assert_eq!(game.encounter().unwrap().retreats, 1);
    assert!(game.monsters.iter().any(|m| m.id == id));
    let mut restored = round_trip(&game);
    restored.recover_focus();
    assert_eq!(warden(&restored).hp, 47);
    assert_eq!(warden(&restored).id, id);
    at(&mut restored, (0, -1));
    restored.wait();
    at(&mut restored, (8, 0));
    restored.wait();
    assert_eq!(warden(&restored).cycle, SentryCycle::Dormant);
    assert_eq!(warden(&restored).hp, 47);
    assert_eq!(restored.encounter().unwrap().retreats, 2);
    round_trip(&restored);
}

#[test]
fn ancillary_reward_does_not_require_kills_and_cannot_be_claimed_twice() {
    let mut game = investigated();
    at(&mut game, (2, -3));
    game.sneaking = true;
    assert_eq!(game.use_site(), Step::Waited);
    assert!(game.relics.contains(&Relic::Hushstone));
    assert!(game.has_relic(Relic::Hushstone));
    assert_eq!(warden(&game).hp, 54);
    assert!(game.encounter().unwrap().sentries.iter().all(|s| s.hp > 0));
    let mut restored = round_trip(&game);
    let before = restored.clone();
    assert_eq!(restored.use_site(), Step::Blocked);
    assert_eq!(restored.turn, before.turn);
    assert_eq!(restored.relics, before.relics);
}

#[test]
fn advanced_force_clears_the_tender_but_blind_stationary_attacks_do_not_trivialize_the_warden() {
    let mut game = investigated();
    let weapon = crate::Weapon::Club;
    game.wanderer.weapons.learn(weapon, 20_000, 200);
    game.position = game.home;
    game.update_region(false);
    game.prepare(Maneuver { weapon, ..Maneuver::default() }).unwrap();
    at(&mut game, (-4, 0));
    let mut strikes = 0;
    while game.encounter().unwrap().sentries[0].hp > 0 && strikes < 3 {
        assert_eq!(game.step(Dir::East), Step::Attacked);
        strikes += 1;
    }
    assert_eq!(game.encounter().unwrap().sentries[0].hp, 0);
    assert!(strikes <= 2, "advanced fixture should convincingly clear the ordinary tender");
    let definition = game.encounter().unwrap().definition;
    // The isolated tender fixture already advanced the distant warden's cycle. Leave and return
    // through the real retreat rule so this race starts against a full shell, not a free opening.
    at(&mut game, (8, 0));
    game.wait();
    assert_eq!(warden(&game).cycle, SentryCycle::Dormant);
    round_trip(&game);
    at(&mut game, (0, -1));
    for _ in 0..40 {
        if game.deaths > 0 || warden(&game).hp == 0 {
            break;
        }
        game.step(Dir::South);
    }
    assert!(game.deaths > 0, "the same fixture cannot ignore the cycle and race the full shell");
    assert!(warden(&game).hp > 0);
    assert_eq!(game.encounter().unwrap().definition, definition);
}

#[test]
fn primary_claim_and_restored_spring_survive_reload_with_no_reward_duplication() {
    let mut game = investigated();
    let id = warden(&game).id;
    // Terminal-state fixture; public combat routes are checked separately.
    game.mark_guard_dead(id);
    game.monsters.retain(|m| m.id != id);
    at(&mut game, (0, 0));
    assert_eq!(game.use_site(), Step::Waited);
    assert!(game.has_relic(Relic::HearthEmber));
    assert_eq!(game.encounter().unwrap().phase, EncounterPhase::Resolved);
    let mut game = round_trip(&game);
    assert_eq!(game.use_site(), Step::Blocked);
    at(&mut game, (-6, 0));
    game.salves = 0;
    assert_eq!(game.use_site(), Step::Waited);
    assert_eq!((game.salves, game.draughts), (2, 2));
    let used = game.turn;
    game.salves = 0;
    assert_eq!(game.use_site(), Step::Blocked);
    round_trip(&game);
    game.turn = used + SPRING_COOLDOWN_TURNS - 1;
    assert_eq!(game.use_site(), Step::Blocked);
    game.turn += 1;
    assert_eq!(game.use_site(), Step::Waited);
}

#[test]
fn malformed_definitions_and_inconsistent_runtime_are_rejected_and_v5_migrates() {
    let game = investigated();
    let encoded = serde_json::to_value(game.snapshot()).unwrap();
    for field in ["version", "origin"] {
        let mut changed = encoded.clone();
        changed["encounter"]["definition"][field] =
            if field == "version" { serde_json::json!(99) } else { serde_json::json!([1, 1]) };
        assert!(Game::from_snapshot(serde_json::from_value(changed).unwrap()).is_err());
    }
    let mut changed = encoded.clone();
    changed["encounter"]["sentries"][1]["alive"] = false.into();
    assert!(Game::from_snapshot(serde_json::from_value(changed).unwrap()).is_err());
    let mut changed = encoded;
    changed["monsters"].as_array_mut().unwrap().retain(|m| m["id"] != serde_json::json!(warden(&game).id));
    assert!(Game::from_snapshot(serde_json::from_value(changed).unwrap()).is_err());
    let fresh = Game::new(42);
    let mut old = serde_json::to_value(fresh.snapshot()).unwrap();
    old["snapshot_version"] = 5.into();
    old.as_object_mut().unwrap().remove("encounter");
    let mut migrated = Game::from_snapshot(serde_json::from_value(old).unwrap()).unwrap();
    assert_eq!(fresh, migrated);
    assert!(migrated.chart_encounter());
}

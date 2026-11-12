use super::*;

fn made() -> Game {
    let mut game = Game::new(7);
    game.answer("Wren");
    game.clear_creatures();
    game
}

fn reload(game: &Game) -> Game {
    serde_json::from_str(&serde_json::to_string(game).unwrap()).unwrap()
}

fn natural_twenty() -> Rng {
    Rng::new((0..10_000).find(|&seed| Rng::new(seed).d20() == 20).unwrap())
}

fn kill_keeper(game: &mut Game, town: Town, depth: u32, den: bool) -> (usize, Pos) {
    let deep = Deep::new(town, depth);
    game.player.at = deep.level();
    game.open_deep(deep);
    game.creatures.clear();
    for spawn in &mut game.spawns {
        spawn.ready = u64::MAX;
        spawn.occupant = None;
    }
    let spawn = game.spawns.iter().position(|spawn| spawn.at == deep.level() && spawn.den == den).unwrap();
    let pos = deep.spawns()[game.spawns[spawn].index].0;
    game.spawn(spawn, pos);
    game.creatures.last_mut().unwrap().hp = 1;
    game.creatures.last_mut().unwrap().stunned = 100;
    game.player.pos = [(0, 1), (0, -1), (1, 0), (-1, 0)]
        .into_iter()
        .map(|(x, y)| (pos.0 + x, pos.1 + y))
        .find(|&at| deep.tile(at).walkable())
        .unwrap();
    game.rng = natural_twenty();
    game.round(Some(Command::Attack(None)));
    assert!(game.creatures.is_empty());
    (spawn, pos)
}

fn count(game: &Game, relic: Relic) -> usize {
    game.piles.iter().flat_map(|pile| &pile.items).filter(|&&item| item == Item::Relic(relic)).count()
}

#[test]
fn every_first_keeper_kill_awards_its_own_prize_once_even_after_reload_and_death() {
    let mut game = made();
    let town = game.towns.home;
    game.player.sack = vec![Item::Potion; SACK];
    for depth in 1..=10 {
        let reward = Relic::conquest_reward(depth).unwrap();
        assert!(!game.conquered(LevelId::Deep(town, depth)));
        kill_keeper(&mut game, town, depth, false);
        assert!(!game.conquered(game.player.at), "ordinary kills are not conquests");
        assert_eq!(count(&game, reward), 0);
        let (_, pos) = kill_keeper(&mut game, town, depth, true);
        assert!(game.conquered(game.player.at));
        assert_eq!(count(&game, reward), 1);
        game.player.pos = pos;
        game.round(Some(Command::Get(reward.name().into())));
        assert_eq!(count(&game, reward), 1, "a full sack leaves the prize in its pile");
        game.player.hp = 0;
        game.die();
        game = reload(&game);
        assert!(game.conquered(LevelId::Deep(town, depth)));
        kill_keeper(&mut game, town, depth, true);
        assert_eq!(count(&game, reward), 1, "respawns never renew the first-clear prize");
    }
    assert_eq!(game.conquests.len(), 10);
    game.instant(Instant::Inventory);
    for depth in 1..=10 {
        assert!(game.log.iter().any(|line| line.text == format!("Conquered: {} Deep, depth {depth}.", town.name())));
    }
}

#[test]
fn conquests_are_local_to_town_and_depth_and_old_saves_start_empty() {
    let mut game = made();
    let home = game.towns.home;
    let mut other = home;
    other.origin.0 += 1000;
    kill_keeper(&mut game, home, 1, true);
    assert!(!game.conquered(LevelId::Deep(other, 1)));
    assert!(!game.conquered(LevelId::Deep(home, 2)));
    kill_keeper(&mut game, other, 1, true);
    assert_eq!(game.conquests.len(), 2);
    assert_eq!(count(&game, Relic::IronrootBead), 2);
    let loaded = reload(&game);
    assert_eq!(loaded, game);
    let mut old = serde_json::to_value(&game).unwrap();
    old.as_object_mut().unwrap().remove("conquests");
    let old: Game = serde_json::from_value(old).unwrap();
    assert!(old.conquests.is_empty());
}

#[test]
fn every_stair_stays_open_before_any_conquest_including_beyond_depth_ten() {
    let mut game = made();
    let home = game.towns.home;
    game.player.pos = home.stairs();
    game.round(Some(Command::Down));
    for depth in 1..=10 {
        assert_eq!(game.player.at, LevelId::Deep(home, depth));
        let before = game.log_total;
        game.instant(Instant::Look);
        assert!(game.log.iter().rev().take((game.log_total - before) as usize).any(|line| {
            line.text.contains(&format!("Depth {depth}: unconquered."))
                && line.text.contains(Relic::conquest_reward(depth).unwrap().name())
                && line.text.contains("stairs remain open")
        }));
        game.player.pos = Deep::new(home, depth).stairs_down();
        game.round(Some(Command::Down));
    }
    assert_eq!(game.player.at, LevelId::Deep(home, 11));
    assert!(game.conquests.is_empty());
    assert_eq!(Relic::conquest_reward(0), None);
    assert_eq!(Relic::conquest_reward(11), None);
}

#[test]
fn conquered_keeper_respawns_on_schedule_and_still_has_a_normal_hoard() {
    let mut game = made();
    let home = game.towns.home;
    let (spawn, _) = kill_keeper(&mut game, home, 1, true);
    let ready = game.spawns[spawn].ready;
    game.player.pos = Deep::new(home, 1).stairs();
    while game.round < ready {
        assert!(game.spawns[spawn].occupant.is_none());
        game.round(None);
    }
    let id = game.spawns[spawn].occupant.expect("the defeated den repopulates");
    let index = game.creatures.iter().position(|creature| creature.id == id).unwrap();
    let at = game.creatures[index].pos;
    let before = game.pile_at(at).unwrap().items.len();
    game.wound(index, 9999, Practice::Blades, 0);
    assert!(game.pile_at(at).unwrap().items.len() > before);
    assert_eq!(count(&game, Relic::IronrootBead), 1);
    assert_eq!(game.conquests.len(), 1);
    assert_eq!(game.spawns[spawn].ready, game.round + DEN_RESPAWN);
}

fn equipped(relic: Relic) -> (Game, Game) {
    let mut base = made();
    base.player.hp = 1;
    base.player.focus = 10;
    let mut worn = base.clone();
    worn.player.sack.push(Item::Relic(relic));
    base.round(None);
    worn.round(Some(Command::Wear(relic.name().into())));
    assert_eq!(worn.player.relic, Some(relic));
    assert_eq!(worn.player.hp, 1, "equipping capacity never heals");
    assert_eq!(worn.player.focus, 10, "equipping capacity never refills Focus");
    (base, worn)
}

#[test]
fn each_prize_has_a_distinct_tradeoff_through_the_real_equipment_and_combat_paths() {
    // Independent expected values: HP, Focus, AC, archery, hiding, ward, melee, arrow, Spark, Heal.
    let expected = [
        (0, 0, 0, -1, 0, 0, 1, 0, 0, 0),
        (0, 0, -1, 0, 15, 0, 0, 0, 0, 0),
        (0, 0, 0, 0, 0, 0, 0, 2, -2, 0),
        (0, 4, 0, 0, 0, 0, -2, 0, 0, 0),
        (0, 0, -2, 0, 0, 0, 3, 0, 0, 0),
        (0, 0, 0, 0, 0, 0, 0, 0, -2, 4),
        (0, -4, 0, 0, 0, 0, 0, 0, 4, 0),
        (0, 0, 0, 0, 0, 3, 0, 0, 0, -3),
        (0, 0, 2, 0, -15, 0, 0, 0, 0, 0),
        (8, -6, 0, 0, 0, 0, 0, 0, 0, 0),
    ];
    for (relic, (hp, focus, ac, archery, hiding, ward, melee, arrow, spark, heal)) in
        Relic::CONQUEST.into_iter().zip(expected)
    {
        let (mut base, mut worn) = equipped(relic);
        assert_eq!(worn.player.max_hp - base.player.max_hp, hp, "{relic:?}");
        assert_eq!(worn.player.max_focus - base.player.max_focus, focus, "{relic:?}");
        assert_eq!(worn.player.ac() - base.player.ac(), ac, "{relic:?}");
        assert_eq!(worn.player.missile_bonus() - base.player.missile_bonus(), archery, "{relic:?}");
        base.player.dex = 10;
        worn.player.dex = 10;
        assert_eq!(worn.player.hiding_chance(0) - base.player.hiding_chance(0), hiding, "{relic:?}");
        assert_eq!(worn.player.ward_protection(false), 0);
        base.player.shield = 10;
        worn.player.shield = 10;
        for missile in [false, true] {
            assert_eq!(worn.player.ward_protection(missile) - base.player.ward_protection(missile), ward);
        }
        for (action, delta) in [(0, melee), (1, arrow), (2, spark), (3, heal)] {
            let mut base = base.clone();
            let mut worn = worn.clone();
            let mut result = Vec::new();
            for game in [&mut base, &mut worn] {
                game.player.str = 30; // Avoid the minimum-damage floor in this comparison.
                game.player.focus = game.player.max_focus;
                let pos = game.player.pos;
                game.place(Kind::Troll, (pos.0 + 1, pos.1));
                game.creatures[0].hp = 500;
                game.creatures[0].stunned = 100;
                game.rng = natural_twenty();
                let command = match action {
                    0 => Command::Attack(None),
                    1 => {
                        game.player.nocked = true;
                        Command::Shoot(Some(Target::Label('A')))
                    }
                    _ => {
                        game.player.chanted = Some(if action == 2 { Spell::Spark } else { Spell::Heal });
                        Command::Cast((action == 2).then_some(Target::Label('A')))
                    }
                };
                game.round(Some(command));
                result.push(if action == 3 { game.player.hp - 1 } else { 500 - game.creatures[0].hp });
            }
            assert_eq!(result[1] - result[0], delta, "{relic:?}, action {action}");
        }
        assert_eq!(reload(&worn), worn);
        worn.player.hp = 1;
        worn.player.focus = 2;
        let practice = worn.player.practice;
        worn.round(Some(Command::Remove("relic".into())));
        assert_eq!(worn.player.relic, None);
        assert_eq!(worn.player.max_hp, base.player.max_hp);
        assert_eq!(worn.player.max_focus, base.player.max_focus);
        assert_eq!(worn.player.hp, 1);
        assert_eq!(worn.player.focus, 2);
        assert_eq!(worn.player.practice, practice);
    }
}

#[test]
fn health_capacity_swaps_cannot_heal_and_full_sacks_cannot_lose_prizes() {
    let (base, mut game) = equipped(Relic::Deepheart);
    game.player.hp = game.player.max_hp;
    game.round(Some(Command::Remove("relic".into())));
    assert_eq!(game.player.hp, base.player.max_hp);
    game.round(Some(Command::Wear("deepheart".into())));
    assert_eq!(game.player.hp, base.player.max_hp, "putting capacity back is not healing");
    game.player.sack = vec![Item::Potion; SACK];
    game.round(Some(Command::Remove("relic".into())));
    assert_eq!(game.player.relic, Some(Relic::Deepheart));
    game.player.sack[0] = Item::Relic(Relic::HollowBell);
    game.round(Some(Command::Wear("hollow bell".into())));
    assert_eq!(game.player.sack[0], Item::Relic(Relic::Deepheart));
    assert_eq!(game.player.sack.len(), SACK);
    assert_eq!(game.player.max_hp, base.player.max_hp);
    assert_eq!(game.player.max_focus, base.player.max_focus + 4);
    game.die();
    let saved = reload(&game);
    assert_eq!(saved.player.relic, Some(Relic::HollowBell));
    assert_eq!(saved.player.focus, saved.player.max_focus);
}

#[test]
fn hiding_caps_and_inventory_descriptions_cover_all_prizes() {
    for relic in Relic::CONQUEST {
        let (_, mut game) = equipped(relic);
        game.player.dex = 100;
        assert!(game.player.hiding_chance(0) <= 100);
        game.player.dex = 0;
        assert!(game.player.hiding_chance(100) >= 0);
        game.instant(Instant::Inventory);
        assert!(game.log.iter().any(|line| line.text.contains(relic.description())));
        game.round(Some(Command::Remove("relic".into())));
        game.instant(Instant::Inventory);
        assert!(game.log.iter().rev().take(8).any(|line| line.text.contains(relic.description())));
    }
}

#[test]
fn conquest_penalties_preserve_minimum_damage_and_healing() {
    for relic in [Relic::FlintEye, Relic::PaleKnot, Relic::GraveSeal, Relic::HollowBell] {
        let mut minimum = i32::MAX;
        for seed in 0..64 {
            let (_, mut game) = equipped(relic);
            game.player.str = 0;
            game.player.int = 0;
            game.player.weapon = Weapon::Dagger;
            game.player.armor = None;
            let pos = game.player.pos;
            game.place(Kind::Troll, (pos.0 + 1, pos.1));
            game.creatures[0].hp = 500;
            game.creatures[0].stunned = 100;
            // The first draw hits; the following draw exercises the damage/recovery range.
            game.rng = Rng::new((seed * 100..seed * 100 + 10_000).find(|&seed| Rng::new(seed).d20() == 20).unwrap());
            let command = match relic {
                Relic::HollowBell => Command::Attack(None),
                Relic::GraveSeal => {
                    game.player.chanted = Some(Spell::Heal);
                    game.rng = Rng::new(seed);
                    Command::Cast(None)
                }
                _ => {
                    game.player.chanted = Some(Spell::Spark);
                    Command::Cast(Some(Target::Label('A')))
                }
            };
            game.round(Some(command));
            let effect = if relic == Relic::GraveSeal { game.player.hp - 1 } else { 500 - game.creatures[0].hp };
            assert!(effect >= 1, "{relic:?} retains the minimum on seed {seed}");
            minimum = minimum.min(effect);
        }
        assert_eq!(minimum, 1, "exercise the actual minimum for {relic:?}");
    }
}

#[test]
fn first_depth_ten_conquest_guarantees_deepheart_but_not_the_mythic_blade() {
    let seed = (0..1000).find(|&seed| !Covenant::default().roll(seed)).unwrap();
    let mut game = Game::new(seed);
    game.answer("Wren");
    let town = game.towns.home;
    kill_keeper(&mut game, town, 10, true);
    assert!(game.conquered(LevelId::Deep(town, 10)));
    assert_eq!(count(&game, Relic::Deepheart), 1);
    assert!(!game.covenant.first_dropped);
    assert_eq!(game.covenant.misses, 1);
    assert!(!game.piles.iter().flat_map(|pile| &pile.items).any(|&item| item == Item::Weapon(Weapon::Oathfang)));
}

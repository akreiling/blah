use super::*;
use crate::{CovenantTrial, LandmarkKind, parse};

fn made(seed: u64) -> Game {
    let mut game = Game::new(seed);
    game.answer("Wren");
    game.clear_creatures();
    game
}

fn run(game: &mut Game, line: &str) {
    match parse(line).unwrap() {
        Parsed::Round(command) => game.round(Some(command)),
        Parsed::Instant(command) => game.instant(command),
    }
}

fn reload(game: &Game) -> Game {
    serde_json::from_str(&serde_json::to_string(game).unwrap()).unwrap()
}

fn natural(face: i32) -> Rng {
    Rng::new((0..10_000).find(|&seed| Rng::new(seed).d20() == face).unwrap())
}

fn awaken(game: &mut Game) {
    game.covenant.first_dropped = true;
    game.drop_at(game.player.at, game.player.pos, Item::Weapon(Weapon::Oathfang));
    run(game, "get oathfang");
    assert!(game.covenant.awakened);
}

fn expert(game: &mut Game) {
    (game.player.str, game.player.dex, game.player.int, game.player.con) = (14, 14, 14, 14);
    for practice in Practice::ALL {
        game.learn_practice(practice, 30_000, 100);
    }
    game.player.weapon = Weapon::LongSword;
    game.player.armor = Some(Armor::Plate);
    game.player.ring = true;
    game.player.refresh_health_capacity();
    game.player.hp = game.player.max_hp;
    game.player.focus = game.player.max_focus;
}

#[test]
fn quadratic_ramp_uses_basis_points_and_keeps_a_real_failure_tail() {
    for (misses, chance) in
        [(0, 200), (1, 224), (5, 806), (10, 2625), (15, 5656), (19, 8954), (20, 9900), (999, 9900), (u64::MAX, 9900)]
    {
        let covenant = Covenant { misses, ..Covenant::default() };
        assert_eq!(covenant.chance_basis_points(), chance);
    }
    let seed = (0..100_000)
        .find(|&seed| {
            let mut covenant = Covenant { misses: 20, ..Covenant::default() };
            !covenant.roll(seed)
        })
        .unwrap();
    let mut covenant = Covenant { misses: 20, ..Covenant::default() };
    assert!(!covenant.roll(seed), "even a capped roll can fail");
    assert_eq!(covenant.misses, 21);
    let mut resumed: Covenant = serde_json::from_str(&serde_json::to_string(&covenant).unwrap()).unwrap();
    assert_eq!(covenant.roll(seed), resumed.roll(seed));
    assert_eq!(covenant, resumed);
    let mut found = Covenant { first_dropped: true, ..Covenant::default() };
    assert!(!(0..100).any(|seed| found.roll(seed)));
    assert_eq!(found.misses, 0);
}

fn keeper(game: &mut Game, depth: u32, den: bool) -> u32 {
    let deep = Deep::new(game.towns.home, depth);
    game.player.at = deep.level();
    game.open_deep(deep);
    let spawn = game.spawns.iter().position(|spawn| spawn.at == deep.level() && spawn.den == den).unwrap();
    let pos = deep.spawns()[game.spawns[spawn].index].0;
    for state in &mut game.spawns {
        state.ready = u64::MAX;
    }
    game.creatures.clear();
    game.spawns[spawn].occupant = None;
    game.spawn(spawn, pos);
    let creature = game.creatures.last_mut().unwrap();
    creature.hp = 1;
    creature.stunned = 100;
    let id = creature.id;
    game.player.pos = [(0, 1), (0, -1), (1, 0), (-1, 0)]
        .into_iter()
        .map(|(dx, dy)| (pos.0 + dx, pos.1 + dy))
        .find(|&at| deep.tile(at).walkable())
        .unwrap();
    game.rng = natural(20);
    run(game, "attack");
    assert!(!game.creatures.iter().any(|creature| creature.id == id));
    id
}

#[test]
fn only_real_depth_ten_keepers_roll_and_reload_preserves_the_next_roll() {
    let seed = (0..1000).find(|&seed| !Covenant::default().roll(seed)).unwrap();
    let mut game = made(seed);
    keeper(&mut game, 9, true);
    keeper(&mut game, 10, false);
    assert_eq!(game.covenant, Covenant::default());
    keeper(&mut game, 10, true);
    assert_eq!(game.covenant.misses, 1);
    let mut resumed = reload(&game);
    for _ in 0..64 {
        game.round += DEN_RESPAWN;
        resumed.round += DEN_RESPAWN;
        keeper(&mut game, 10, true);
        keeper(&mut resumed, 10, true);
        assert_eq!(game, resumed);
        if game.covenant.first_dropped {
            break;
        }
    }
    assert!(game.covenant.first_dropped);
    assert_eq!(
        game.piles.iter().flat_map(|pile| &pile.items).filter(|&&item| item == Item::Weapon(Weapon::Oathfang)).count(),
        1
    );
    let misses = game.covenant.misses;
    keeper(&mut game, 10, true);
    assert_eq!(game.covenant.misses, misses);
    assert_eq!(
        game.piles.iter().flat_map(|pile| &pile.items).filter(|&&item| item == Item::Weapon(Weapon::Oathfang)).count(),
        1
    );
}

#[test]
fn awakened_sites_are_seeded_distinct_reachable_and_do_not_spend_combat_dice() {
    for seed in 0..64 {
        let mut game = made(seed);
        game.covenant.awakened = true;
        let dice = game.rng.clone();
        game.awaken_covenant();
        assert_eq!(game.rng, dice);
        assert_eq!(game.covenant.trials.len(), 2, "seed {seed}");
        let saved = game.covenant.clone();
        game.awaken_covenant();
        assert_eq!(game.covenant, saved);
        let home = game.towns.home.start();
        // Independently flood the final playable map, including actual town overlays.
        let mut reached = BTreeSet::from([home]);
        let mut queue = VecDeque::from([(home, 0)]);
        while let Some((pos, steps)) = queue.pop_front() {
            if steps == 192 {
                continue;
            }
            for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
                let next = (pos.0 + dx, pos.1 + dy);
                let level = Place::new(LevelId::Surface, game.terrain, game.towns.nearby(game.terrain, next), next);
                if !reached.contains(&next) && level.tile(next).walkable() && !level.ground(next).water {
                    reached.insert(next);
                    queue.push_back((next, steps + 1));
                }
            }
            if game.covenant.trials.iter().all(|trial| reached.contains(&trial.pos)) {
                break;
            }
        }
        for trial in &game.covenant.trials {
            assert!(reached.contains(&trial.pos), "seed {seed}: {:?}", trial.pos);
            let level =
                Place::new(LevelId::Surface, game.terrain, game.towns.nearby(game.terrain, trial.pos), trial.pos);
            assert!(level.town_at(trial.pos).is_none());
            for (_, pos) in trial.guards_to_place() {
                assert!(level.tile(pos).walkable());
                assert!(!level.ground(pos).water);
            }
        }
        assert!(
            distance(game.covenant.trials[0].pos, game.covenant.trials[1].pos) >= 6,
            "guard approaches must not overlap, even on small islands"
        );
    }
}

#[test]
fn first_pickup_calls_from_the_deep_and_full_sacks_leave_the_blade_persistent() {
    let mut game = made(7);
    game.player.at = LevelId::Deep(game.towns.home, 10);
    game.player.pos = Deep::new(game.towns.home, 10).stairs();
    game.player.sack = vec![Item::Potion; SACK];
    game.covenant.first_dropped = true;
    game.drop_at(game.player.at, game.player.pos, Item::Weapon(Weapon::Oathfang));
    run(&mut game, "get oathfang");
    assert!(!game.covenant.awakened);
    assert!(reload(&game).pile_at(game.player.pos).unwrap().items.contains(&Item::Weapon(Weapon::Oathfang)));
    game.player.sack.pop();
    run(&mut game, "get oathfang");
    assert!(game.covenant.awakened);
    assert!(game.log.iter().any(|line| line.text.contains("calls toward the surface")));
    assert_eq!(game.covenant.trials.len(), 2);
    game.player.at = LevelId::Surface;
    game.player.pos = game.towns.home.start();
    run(&mut game, "look");
    assert!(game.log.iter().any(|line| line.text.contains("Oathfang's call: Ashen Forge lies")));
    let trail = game.covenant.clone();
    run(&mut game, "drop oathfang");
    run(&mut game, "get oathfang");
    assert_eq!(game.covenant, trail);
}

#[test]
fn trials_keep_original_guards_through_retreat_death_reload_and_partial_victory() {
    let mut game = made(7);
    awaken(&mut game);
    game.player.pos = game.covenant.trials[1].pos;
    run(&mut game, "look");
    run(&mut game, "get all");
    assert!(!game.player.sack.contains(&Item::Relic(Relic::ExileHeart)));
    run(&mut game, "challenge");
    assert_eq!(game.covenant.trials[1].phase, WatchPhase::Active);
    let ids = game.covenant.trials[1].guards;
    run(&mut game, "challenge");
    assert_eq!(game.covenant.trials[1].guards, ids);
    let first = game.creatures.iter().position(|creature| Some(creature.id) == ids[0]).unwrap();
    game.wound(first, 10_000, Practice::Blades, 0);
    let second = game.creatures.iter().position(|creature| Some(creature.id) == ids[1]).unwrap();
    game.creatures[second].hp -= 7;
    let remaining = game.creatures[second].clone();
    game.die();
    let mut game = reload(&game);
    assert_eq!(game.creatures.iter().find(|creature| creature.id == remaining.id).unwrap().hp, remaining.hp);
    game.player.pos = game.covenant.trials[1].pos;
    game.challenge_covenant();
    assert_eq!(game.covenant.trials[1].guards, [None, ids[1]]);
    let second = game.creatures.iter().position(|creature| creature.id == remaining.id).unwrap();
    game.wound(second, 10_000, Practice::Blades, 0);
    assert_eq!(game.covenant.trials[1].phase, WatchPhase::Cleared);
    game.player.sack = vec![Item::Potion; SACK];
    run(&mut game, "get all");
    assert!(game.landmark_at(game.player.pos).unwrap().opened);
    assert_eq!(game.pile_at(game.player.pos).unwrap().items, vec![Item::Relic(Relic::ExileHeart)]);
    let mut game = reload(&game);
    game.player.sack.pop();
    run(&mut game, "get all");
    run(&mut game, "get all");
    run(&mut game, "challenge");
    assert_eq!(game.player.sack.iter().filter(|&&item| item == Item::Relic(Relic::ExileHeart)).count(), 1);
    assert!(game.creatures.is_empty());
}

#[test]
fn all_three_must_be_equipped_and_swapping_never_refills_health() {
    let mut game = made(7);
    expert(&mut game);
    let mortal_max = game.player.max_hp;
    game.player.sack =
        vec![Item::Weapon(Weapon::Oathfang), Item::Armor(Armor::AshenCarapace), Item::Relic(Relic::ExileHeart)];
    assert!(!game.player.is_demigod());
    run(&mut game, "wield oathfang");
    run(&mut game, "wear carapace");
    assert!(!game.player.is_demigod());
    run(&mut game, "wear exile heart");
    assert!(game.player.is_demigod());
    assert_eq!((game.player.hp, game.player.max_hp), (mortal_max, mortal_max + 20));
    game.player.hp = game.player.max_hp;
    let mut game = reload(&game);
    assert!(game.player.is_demigod());
    run(&mut game, "remove exile heart");
    assert_eq!((game.player.hp, game.player.max_hp), (mortal_max, mortal_max));
    run(&mut game, "wear exile heart");
    assert_eq!((game.player.hp, game.player.max_hp), (mortal_max, mortal_max + 20));
    run(&mut game, "wield long sword");
    assert!(!game.player.is_demigod());
    assert_eq!(game.player.max_hp, mortal_max);
    run(&mut game, "wield oathfang");
    run(&mut game, "wear plate");
    assert!(!game.player.is_demigod());
    assert_eq!(game.player.max_hp, mortal_max);
}

#[test]
fn vitality_gains_keep_the_active_set_bonus_without_healing() {
    let mut game = made(7);
    game.player.weapon = Weapon::Oathfang;
    game.player.armor = Some(Armor::AshenCarapace);
    game.player.relic = Some(Relic::ExileHeart);
    game.player.refresh_health_capacity();
    let (health, maximum) = (game.player.hp, game.player.max_hp);
    game.learn_practice(Practice::Vitality, 30_000, 100);
    assert_eq!(game.player.hp, health);
    assert_eq!(game.player.max_hp, maximum + 16);
}

#[test]
fn occupied_trial_posts_block_challenge_atomically_and_towns_shelter_from_guardians() {
    let mut game = made(7);
    awaken(&mut game);
    game.player.pos = game.covenant.trials[1].pos;
    let occupied = game.covenant.trials[1].guards_to_place()[1].1;
    let blocker = game.place(Kind::Rat, occupied);
    game.challenge_covenant();
    assert_eq!(game.covenant.trials[1].phase, WatchPhase::Dormant);
    assert_eq!(game.covenant.trials[1].guards, [None; 2]);
    assert_eq!(game.creatures.len(), 1);
    assert_eq!(game.creatures[0].id, blocker);
    game.creatures.clear();
    game.challenge_covenant();
    game.player.pos = game.towns.home.start();
    // Even a close guardian cannot sense or step onto a safe town cell.
    game.creatures[0].home = game.player.pos;
    game.creatures[0].pos = (game.player.pos.0, game.player.pos.1 - 1);
    assert!(!game.creature_can_see(0, game.player.pos));
    assert!(!game.open_for_creature(0, (game.player.pos.0 + 1, game.player.pos.1)));
    let health = game.player.hp;
    game.creatures_act();
    assert_eq!(game.player.hp, health);
}

#[test]
fn mythic_pieces_cannot_be_sold_but_dropped_pieces_and_progress_survive() {
    let mut game = made(7);
    awaken(&mut game);
    game.player.pos = game.towns.home.shop();
    game.player.sack.extend([Item::Armor(Armor::AshenCarapace), Item::Relic(Relic::ExileHeart)]);
    let before = (game.player.sack.clone(), game.player.gold);
    for name in ["oathfang", "carapace", "exile heart"] {
        run(&mut game, &format!("sell {name}"));
    }
    assert_eq!((game.player.sack.clone(), game.player.gold), before);
    run(&mut game, "drop oathfang");
    let mut resumed = reload(&game);
    assert_eq!(resumed.covenant, game.covenant);
    run(&mut resumed, "get oathfang");
    assert!(resumed.player.sack.contains(&Item::Weapon(Weapon::Oathfang)));
}

#[test]
fn oathfang_drinks_only_finite_dangerous_damage_and_costs_extra_technique_focus() {
    let mut game = made(7);
    expert(&mut game);
    game.player.weapon = Weapon::Oathfang;
    game.player.hp = 10;
    let id = game.place(Kind::Troll, (game.player.pos.0 + 1, game.player.pos.1));
    game.wound(0, 9, Practice::Blades, 0);
    assert_eq!(game.player.hp, 13);
    game.creatures[0].learning_damage_left = 0;
    game.wound(0, 9, Practice::Blades, 0);
    assert_eq!(game.player.hp, 13);
    game.creatures[0].learning_damage_left = 40;
    game.player.armor = Some(Armor::AshenCarapace);
    game.wound(0, 12, Practice::Blades, 0);
    assert_eq!(game.player.hp, 15, "the shell halves stolen Health too");
    game.player.focus = 2;
    assert!(!game.use_technique(Technique::Parry, 0));
    game.player.focus = 3;
    assert!(game.use_technique(Technique::Parry, 0));
    assert_eq!(game.player.focus, 0);
    assert_eq!(game.creatures[0].id, id);
    game.creatures.clear();
    game.place(Kind::Rat, (game.player.pos.0 + 1, game.player.pos.1));
    game.wound(0, 2, Practice::Blades, 0);
    assert_eq!(game.player.hp, 15, "harmless prey cannot feed the blade");
}

#[test]
fn heart_charges_only_released_spells_blocks_lethal_payment_and_never_teaches_self_harm() {
    let mut game = made(7);
    game.player.relic = Some(Relic::ExileHeart);
    game.player.hp = 10;
    run(&mut game, "chant spark");
    run(&mut game, "cast");
    assert_eq!(game.player.hp, 10, "no target, no payment");
    run(&mut game, "chant shield");
    run(&mut game, "cast");
    assert_eq!(game.player.hp, 8);
    assert!(game.player.injuries.is_empty());
    game.player.hp = 2;
    run(&mut game, "chant heal");
    let focus = game.player.focus;
    run(&mut game, "cast");
    assert_eq!(game.player.hp, 2);
    assert_eq!(game.player.chanted, Some(Spell::Heal));
    assert!(game.player.focus >= focus, "refusal spends no Focus");
    game.player.hp = 3;
    game.player.focus = 5;
    run(&mut game, "cast");
    assert!(game.player.hp > 1);
    assert!(game.player.injuries.is_empty());
    game.round = FOCUS_TICK;
    game.player.focus = 1;
    game.recover();
    assert_eq!(game.player.focus, 3);
}

#[test]
fn forge_fire_and_well_focus_theft_use_actual_combat_turns() {
    let mut game = made(7);
    game.player.pos = game.towns.home.start();
    let id = game.place(Kind::AshColossus, (game.player.pos.0 + 1, game.player.pos.1));
    game.creatures[0].hp = 50;
    game.player.chanted = Some(Spell::Spark);
    game.rng = natural(20);
    game.cast(None);
    let burned = game.creatures[0].hp;
    assert!(burned < 50);
    assert_eq!(game.creatures[0].seared, 3);
    // Move off town safety so the real guardian's turns run.
    let home = game.cinder_watch.as_ref().unwrap().pos;
    game.player.pos = home;
    game.creatures[0].pos = (home.0, home.1 - 2);
    game.creatures[0].home = game.creatures[0].pos;
    for _ in 0..3 {
        game.creature_turn(0);
    }
    assert_eq!(game.creatures[0].hp, burned);
    let mut resumed = reload(&game);
    resumed.creature_turn(0);
    assert_eq!(resumed.creatures[0].hp, burned + 4);
    assert_eq!(resumed.creatures[0].id, id);
    resumed.creatures.clear();
    resumed.place(Kind::HollowCantor, (home.0, home.1 - 2));
    resumed.player.focus = 10;
    resumed.rng = natural(20);
    resumed.creature_strike(0, 100, (1, 1), Some("arrow"));
    assert_eq!(resumed.player.focus, 8);
}

#[test]
fn older_saves_default_to_an_unstarted_covenant_and_unseared_creatures() {
    let mut game = made(7);
    game.place(Kind::Rat, (game.player.pos.0 + 1, game.player.pos.1));
    let mut value = serde_json::to_value(&game).unwrap();
    value.as_object_mut().unwrap().remove("covenant");
    value["creatures"][0].as_object_mut().unwrap().remove("seared");
    let loaded: Game = serde_json::from_value(value).unwrap();
    assert_eq!(loaded.covenant, Covenant::default());
    assert_eq!(loaded.creatures[0].seared, 0);
}

#[test]
fn trial_data_explains_both_distinct_counters_before_commitment() {
    for kind in [LandmarkKind::AshenForge, LandmarkKind::WellOfNames] {
        let trial = CovenantTrial { pos: (0, 0), kind, phase: WatchPhase::Dormant, guards: [None; 2] };
        assert!(trial.description().contains("challenge"));
        assert!(trial.description().contains(if kind == LandmarkKind::AshenForge { "Spark" } else { "Focus" }));
    }
}

#[test]
fn carapace_halves_every_field_health_source_but_not_the_temple() {
    let base = made(7);
    for action in ["quaff", "use woundleaf", "rest", "cast"] {
        let mut plain = base.clone();
        plain.player.hp = 1;
        plain.player.sack = vec![Item::Potion, Item::Herb(crate::Herb::Woundleaf)];
        plain.player.chanted = Some(Spell::Heal);
        let mut armored = plain.clone();
        armored.player.armor = Some(Armor::AshenCarapace);
        run(&mut plain, action);
        run(&mut armored, action);
        let ordinary = plain.player.hp - 1;
        assert_eq!(armored.player.hp - 1, (ordinary / 2).max(1), "{action}");
    }
    let mut armored = base;
    armored.player.armor = Some(Armor::AshenCarapace);
    armored.player.hp = 1;
    armored.player.pos = armored.towns.home.respawn();
    run(&mut armored, "heal");
    assert_eq!(armored.player.hp, armored.player.max_hp);
}

#[test]
fn heart_adds_six_spark_damage_and_set_pierces_armor_and_arrow_plating() {
    let mut base = made(7);
    expert(&mut base);
    base.place(Kind::AshColossus, (base.player.pos.0 + 1, base.player.pos.1));
    base.player.chanted = Some(Spell::Spark);
    base.rng = natural(20);
    let mut heart = base.clone();
    heart.player.relic = Some(Relic::ExileHeart);
    base.cast(None);
    heart.cast(None);
    assert_eq!(base.creatures[0].hp - heart.creatures[0].hp, 6);
    let mut set = heart.clone();
    set.player.weapon = Weapon::Oathfang;
    set.player.armor = Some(Armor::AshenCarapace);
    set.player.nocked = true;
    set.player.dex = 14;
    let mut broken = set.clone();
    broken.player.relic = None;
    // A roll that hits only after the complete set removes four points of armor.
    let face = broken.creatures[0].stats().ac - broken.player.missile_bonus() + 4 - 4;
    assert!((1..20).contains(&face));
    set.rng = natural(face);
    broken.rng = natural(face);
    let before = broken.creatures[0].hp;
    set.shoot(None);
    broken.shoot(None);
    assert_eq!(broken.creatures[0].hp, before);
    assert!(set.creatures[0].hp < before);
    // At a guaranteed hit, full covenant arrows deal unhalved damage.
    broken.player.nocked = true;
    set.player.nocked = true;
    broken.rng = natural(20);
    set.rng = natural(20);
    let (before_broken, before_set) = (broken.creatures[0].hp, set.creatures[0].hp);
    broken.shoot(None);
    set.shoot(None);
    assert_eq!(before_broken - broken.creatures[0].hp, (before_set - set.creatures[0].hp) / 2);
}

fn pursue(game: &Game) -> Command {
    let start = game.player.pos;
    if game.creatures.iter().any(|creature| creature.at == game.player.at && distance(start, creature.pos) <= 1) {
        return Command::Attack(None);
    }
    let mut queue = VecDeque::from([start]);
    let mut first_step = BTreeMap::from([(start, None)]);
    while let Some(pos) = queue.pop_front() {
        for dir in [Dir::N, Dir::S, Dir::E, Dir::W, Dir::Ne, Dir::Nw, Dir::Se, Dir::Sw] {
            let (dx, dy) = dir.delta();
            let next = (pos.0 + dx, pos.1 + dy);
            if distance(next, start) > 24
                || first_step.contains_key(&next)
                || !game.level().tile(next).walkable()
                || game.creature_at(next).is_some()
            {
                continue;
            }
            let first = first_step[&pos].unwrap_or(dir);
            if game.creatures.iter().any(|creature| creature.at == game.player.at && distance(next, creature.pos) <= 1)
            {
                return Command::Move(vec![first]);
            }
            first_step.insert(next, Some(first));
            queue.push_back(next);
        }
    }
    Command::Wait
}

/// Matched, actual round resolution, not damage arithmetic or an authored win-rate.
#[test]
fn measure_covenant_progression_and_both_wilderness_trials() {
    const TRIALS: usize = 128;
    let mut wins = [0; 4];
    let mut health = [0_i64; 4];
    for world in 1..=32 {
        let mut base = made(world);
        expert(&mut base);
        base.player.practice = Practices::default();
        base.learn_practice(Practice::Blades, 30_000, 100);
        base.learn_practice(Practice::Vitality, 30_000, 100);
        base.player.refresh_focus_capacity();
        let deep = Deep::new(base.towns.home, 10);
        base.player.at = deep.level();
        base.open_deep(deep);
        let spawn = base.spawns.iter().position(|spawn| spawn.den).unwrap();
        let den = deep.spawns()[base.spawns[spawn].index].0;
        base.player.pos = [(0, 1), (0, -1), (1, 0), (-1, 0)]
            .into_iter()
            .map(|(dx, dy)| (den.0 + dx, den.1 + dy))
            .find(|&at| deep.tile(at).walkable() && !deep.landing(at))
            .unwrap();
        for draw in [0, 17, 43, 97] {
            base.creatures.clear();
            base.round = draw;
            base.spawn(spawn, den);
            for state in &mut base.spawns {
                state.ready = u64::MAX;
            }
            for stage in 0..4 {
                let mut game = base.clone();
                game.round = 0;
                game.rng = Rng::new(world.wrapping_mul(0x9e37_79b9) ^ draw);
                if stage >= 1 {
                    game.player.weapon = Weapon::Oathfang;
                }
                if stage >= 2 {
                    game.player.armor = Some(Armor::AshenCarapace);
                }
                if stage >= 3 {
                    game.player.relic = Some(Relic::ExileHeart);
                }
                game.player.refresh_health_capacity();
                game.player.hp = game.player.max_hp;
                for _ in 0..160 {
                    let action = pursue(&game);
                    game.round(Some(action));
                    if game.player.deaths > 0 || game.creatures.is_empty() {
                        break;
                    }
                }
                if game.player.deaths == 0 && game.creatures.is_empty() {
                    wins[stage] += 1;
                    health[stage] += i64::from(game.player.hp);
                }
            }
        }
    }
    for (stage, label) in
        ["ordinary expert", "Oathfang", "blade + carapace", "complete covenant"].into_iter().enumerate()
    {
        println!(
            "D10 {label}: {}/{} wins, mean Health kept on wins {:.1}",
            wins[stage],
            TRIALS,
            health[stage] as f64 / wins[stage].max(1) as f64
        );
    }
    assert!(wins[1] > wins[0], "the first piece must make a real difference");
    assert!(wins[3] > wins[0], "the covenant must surpass mortal equipment");

    let mut base = made(7);
    expert(&mut base);
    awaken(&mut base);
    base.player.weapon = Weapon::Oathfang;
    let mut trial_wins = [[0; 2]; 2];
    let mut trial_health = [[0_i64; 2]; 2];
    for trial in 0..2 {
        for prepared in 0..2 {
            for seed in 0..TRIALS {
                let mut game = base.clone();
                if trial == 1 {
                    game.player.armor = Some(Armor::AshenCarapace);
                }
                game.player.pos = game.covenant.trials[trial].pos;
                game.player.sack = vec![Item::Potion; 6];
                game.rng = Rng::new(seed as u64);
                run(&mut game, "challenge");
                for turn in 0..160 {
                    if game.player.deaths > 0 || game.covenant.trials[trial].phase == WatchPhase::Cleared {
                        break;
                    }
                    let action = if prepared == 1 && game.player.hp < 18 && game.player.sack.contains(&Item::Potion) {
                        Command::Quaff
                    } else if prepared == 1 && turn == 0 {
                        Command::Chant(Spell::Shield)
                    } else if prepared == 1 && game.player.chanted.is_some() {
                        Command::Cast(None)
                    } else if prepared == 1 && trial == 0 && turn % 5 == 2 && game.player.focus >= 5 {
                        Command::Chant(Spell::Spark)
                    } else {
                        pursue(&game)
                    };
                    game.round(Some(action));
                }
                if game.player.deaths == 0 && game.covenant.trials[trial].phase == WatchPhase::Cleared {
                    trial_wins[trial][prepared] += 1;
                    trial_health[trial][prepared] += i64::from(game.player.hp);
                }
            }
            println!(
                "{} {}: {}/{} wins, mean Health kept on wins {:.1}",
                base.covenant.trials[trial].kind.name(),
                if prepared == 0 { "melee only" } else { "ward, fire, six potions" },
                trial_wins[trial][prepared],
                TRIALS,
                trial_health[trial][prepared] as f64 / trial_wins[trial][prepared].max(1) as f64
            );
        }
        assert!(trial_wins[trial][1] > 0, "a prepared expert must be able to finish either trial");
    }
}

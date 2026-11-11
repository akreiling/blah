//! Whole-round behavior: commands through creatures, deaths, and saves.

use crate::{
    Armor, Bow, Command, Game, Item, Kind, LevelId, Parsed, Phase, Practice, Spell, Target, Tone, Zone, parse,
};

fn made() -> Game {
    let mut game = Game::new(7);
    game.answer("Tamsin");
    assert_eq!(game.phase(), &Phase::Play);
    game
}

fn round(game: &mut Game, line: &str) {
    match parse(line).unwrap() {
        Parsed::Round(c) => game.round(Some(c)),
        Parsed::Instant(i) => game.instant(i),
    }
}

fn said(game: &Game, needle: &str) -> bool {
    game.log().iter().any(|l| l.text.contains(needle))
}

#[test]
fn creation_asks_only_for_a_name_and_grants_neutral_starter_gear() {
    let mut game = Game::new(1);
    game.answer("");
    assert_eq!(game.phase(), &Phase::Name);
    game.answer("tamsin");
    assert_eq!(game.phase(), &Phase::Play);
    assert_eq!(game.player().name, "Tamsin");
    assert_eq!(game.player().weapon, crate::Weapon::ShortSword);
    assert_eq!(game.player().armor, Some(Armor::Leather));
    assert_eq!(game.player().bow, Some(Bow::Shortbow));
    assert_eq!(game.player().arrows, 12);
    assert_eq!(game.player().sack, vec![Item::Weapon(crate::Weapon::Staff), Item::Potion]);
    assert_eq!(game.player().gold, 50);
    assert_eq!((game.player().focus, game.player().max_focus), (24, 24));
    assert!(Practice::ALL.iter().all(|&s| game.player().practice.get(s).rank() == blah_core::Rank::Novice));
}

#[test]
fn skills_is_a_no_time_practice_and_benefits_listing() {
    let mut game = made();
    let turn = game.round_number();
    let lines = game.log_total();
    round(&mut game, "sk");
    assert_eq!(game.round_number(), turn);
    assert_eq!(game.log_total(), lines + Practice::ALL.len() as u64);
    assert!(said(&game, "blades: novice"));
    assert!(said(&game, "maximum Focus: 24"));
}

#[test]
fn nock_then_shoot_kills_a_rat_and_drops_arrows_nearby() {
    let mut game = made();
    game.clear_creatures();
    let me = game.player().pos;
    game.place(Kind::Rat, (me.0 + 3, me.1));
    game.player_mut().dex = 18;
    round(&mut game, "shoot rat");
    assert!(said(&game, "You must nock an arrow first."));
    let arrows = game.player().arrows;
    for _ in 0..20 {
        if game.creatures().is_empty() {
            break;
        }
        round(&mut game, "nock bow");
        round(&mut game, "shoot a");
    }
    assert!(game.creatures().is_empty(), "the rat dies to arrows");
    assert!(game.player().arrows < arrows);
    assert!(said(&game, "The rat dies!"));
}

#[test]
fn creatures_hunt_and_strike_you() {
    let mut game = made();
    game.clear_creatures();
    let me = game.player().pos;
    game.place(Kind::Orc, (me.0 + 4, me.1));
    game.player_mut().max_hp = 999;
    game.player_mut().hp = 999;
    for _ in 0..12 {
        game.round(Some(Command::Rest));
    }
    assert!(game.creatures()[0].hunting);
    assert!(crate::distance(game.creatures()[0].pos, game.player().pos) <= 1);
    assert!(game.log().iter().any(|l| matches!(l.tone, Tone::Hurt | Tone::Miss)));
}

#[test]
fn standing_ready_strikes_back_at_an_adjacent_hunter() {
    let mut game = made();
    game.clear_creatures();
    let me = game.player().pos;
    game.place(Kind::Rat, (me.0 + 1, me.1));
    game.round(None);
    assert!(!said(&game, "You hit") && !said(&game, "You miss"), "a rat that has not yet hunted is left alone");
    game.player_mut().max_hp = 999;
    game.player_mut().hp = 999;
    for _ in 0..20 {
        game.round(None);
    }
    assert!(game.creatures().is_empty(), "the rat falls to blows struck back");
}

#[test]
fn explicit_wait_matches_standing_ready_on_the_clock() {
    let mut clocked = made();
    clocked.clear_creatures();
    let me = clocked.player().pos;
    clocked.place(Kind::Rat, (me.0 + 1, me.1));
    let mut turns = clocked.clone();
    for _ in 0..20 {
        clocked.round(None);
        turns.round(Some(Command::Wait));
        assert_eq!(turns, clocked);
    }
    assert!(turns.creatures().is_empty(), "explicit waiting strikes back too");
}

#[test]
fn surface_life_does_not_spawn_inside_town() {
    let mut game = made();
    game.player_mut().max_hp = 999;
    game.player_mut().hp = 999;
    for _ in 0..300 {
        game.round(None);
    }
    assert!(!game.log().iter().any(|l| l.tone == crate::Tone::Hurt), "nothing reaches the town square");
    assert!(game.creatures().iter().all(|c| c.at != LevelId::Surface));
}

#[test]
fn shopping_buys_a_bow_and_arrows_and_sells_back() {
    let mut game = made();
    game.clear_creatures();
    game.player_mut().pos = game.level().town_at(game.player().pos).unwrap().shop();
    assert_eq!(game.zone(), Zone::Shop);
    round(&mut game, "buy shortbow");
    round(&mut game, "buy arrows");
    let p = game.player();
    assert_eq!((p.bow, p.arrows, p.gold), (Some(Bow::Shortbow), 22, 0));
    assert!(p.sack.contains(&Item::Bow(Bow::Shortbow)));
    round(&mut game, "sell potion");
    assert_eq!(game.player().gold, 12);
    round(&mut game, "buy long sword");
    assert!(said(&game, "That's 80 gold"));
}

#[test]
fn stairs_lead_down_and_back_up() {
    let mut game = made();
    game.clear_creatures();
    game.player_mut().pos = game.level().stairs_down.unwrap();
    round(&mut game, "d");
    assert!(matches!(game.player().at, LevelId::Deep(_, 1)));
    round(&mut game, "u");
    assert_eq!(game.player().at, LevelId::Surface);
    round(&mut game, "u");
    assert!(said(&game, "There are no stairs up here."));
}

#[test]
fn death_wakes_you_in_the_temple_and_leaves_your_gold() {
    let mut game = made();
    game.clear_creatures();
    let me = game.player().pos;
    game.player_mut().hp = 1;
    game.place(Kind::Troll, (me.0 + 1, me.1));
    for _ in 0..30 {
        game.round(None);
        if game.player().deaths > 0 {
            break;
        }
    }
    assert_eq!(game.player().deaths, 1);
    assert_eq!(game.zone(), Zone::Temple);
    assert_eq!(game.player().gold, 0);
    assert_eq!(game.player().hp, game.player().max_hp);
}

#[test]
fn everyone_can_cast_every_spell_and_metal_only_penalizes_non_wards() {
    let mut game = made();
    game.clear_creatures();
    let me = game.player().pos;
    game.place(Kind::Orc, (me.0 + 1, me.1));
    game.player_mut().armor = Some(Armor::Chainmail);

    round(&mut game, "chant heal");
    assert_eq!(game.player().chanted, Some(Spell::Heal));
    round(&mut game, "cast");
    assert_eq!(game.player().focus, 18, "metal adds two Focus to Heal");

    round(&mut game, "chant shield");
    round(&mut game, "cast");
    assert!(game.player().shield > 0);
    assert_eq!(game.player().focus, 14, "Warding has no metal penalty");

    round(&mut game, "chant spark");
    round(&mut game, "cast orc");
    assert_eq!(game.player().focus, 9, "Spark costs three plus two in metal");

    round(&mut game, "chant stun");
    round(&mut game, "cast orc");
    assert_eq!(game.player().focus, 2, "Stun costs five plus two in metal");

    game.round(Some(Command::Hide));
    assert!(!said(&game, "no knack for hiding"));
}

#[test]
fn targets_by_letter_and_by_ordinal_name() {
    let mut game = made();
    game.clear_creatures();
    let me = game.player().pos;
    let near = game.place(Kind::Goblin, (me.0 + 2, me.1));
    let far = game.place(Kind::Goblin, (me.0 + 4, me.1));
    let sighted = game.sighted();
    assert_eq!((sighted[0].label, sighted[0].creature.id), ('A', near));
    assert_eq!((sighted[1].label, sighted[1].creature.id), ('B', far));
    round(&mut game, "nock");
    let Parsed::Round(Command::Shoot(Some(Target::Named { nth: 2, .. }))) = parse("shoot 2 gob").unwrap() else {
        panic!("ordinal target");
    };
    round(&mut game, "shoot 2 gob");
    assert!(said(&game, "goblin"), "the shot names a goblin");
    round(&mut game, "shoot z");
    assert!(said(&game, "No creature is marked Z."));
}

#[test]
fn a_saved_game_resumes_exactly() {
    let mut game = made();
    for line in ["s s s", "nock", "e e", "look", "i"] {
        round(&mut game, line);
    }
    let json = serde_json::to_string(&game).unwrap();
    let mut resumed: Game = serde_json::from_str(&json).unwrap();
    assert_eq!(resumed, game);
    for _ in 0..30 {
        game.round(None);
        resumed.round(None);
    }
    assert_eq!(resumed, game, "same dice after loading");
}

#[test]
fn get_all_takes_gold_arrows_and_items() {
    let mut game = made();
    game.clear_creatures();
    let me = game.player().pos;
    game.place(Kind::Rat, (me.0 + 1, me.1));
    for _ in 0..30 {
        if game.creatures().is_empty() {
            break;
        }
        round(&mut game, "a");
        game.player_mut().hp = game.player().max_hp;
    }
    assert!(game.creatures().is_empty());
    let gold = game.player().gold;
    round(&mut game, "e");
    if game.pile_at(game.player().pos).is_some() {
        round(&mut game, "get all");
        assert!(game.player().gold >= gold);
        assert!(game.pile_at(game.player().pos).is_none());
    }
    assert!(!game.player().sack.contains(&Item::Gold(0)));
}

#[test]
fn you_can_walk_from_the_town_square_into_the_wilderness_and_back() {
    use crate::Dir;
    use std::collections::{BTreeMap, VecDeque};

    let mut game = made();
    game.clear_creatures();
    let start = game.player().pos;
    let mut queue = VecDeque::from([start]);
    let mut previous = BTreeMap::from([(start, None)]);
    let exit = loop {
        let pos = queue.pop_front().expect("the town has an exit onto generated ground");
        if !game.level().authored(pos) {
            break pos;
        }
        for dir in [Dir::N, Dir::S, Dir::E, Dir::W] {
            let (dx, dy) = dir.delta();
            let next = (pos.0 + dx, pos.1 + dy);
            if game.level().tile(next).walkable() && !previous.contains_key(&next) {
                previous.insert(next, Some((pos, dir)));
                queue.push_back(next);
            }
        }
    };
    let mut path = Vec::new();
    let mut pos = exit;
    while let Some((from, dir)) = previous[&pos] {
        path.push(dir);
        pos = from;
    }
    path.reverse();
    for dir in &path {
        game.round(Some(Command::Move(vec![*dir])));
    }
    assert_eq!(game.player().pos, exit);
    assert!(game.remembers(exit));
    round(&mut game, "look");
    assert!(said(&game, &game.level().name_at(exit)));
    for dir in path.into_iter().rev() {
        let (dx, dy) = dir.delta();
        let reverse = Dir::from_delta(-dx, -dy).unwrap();
        game.round(Some(Command::Move(vec![reverse])));
    }
    assert_eq!(game.player().pos, start);
    assert!(!game.sees(exit));
    assert!(game.remembers(exit));
}

#[test]
fn wilderness_memory_spans_signed_chunks_and_survives_saving_and_level_changes() {
    let mut game = made();
    game.clear_creatures();
    let positions = [(-17, -17), (-16, -16), (-1, -1), (0, -1), (15, -1), (16, -1), (1_000_000, -1_000_000)];
    for pos in positions {
        game.player_mut().pos = pos;
        game.round(Some(Command::Wait));
        assert!(game.remembers(pos));
    }
    game.player_mut().pos = (22, 9);
    game.round(Some(Command::Wait));
    let json = serde_json::to_string(&game).unwrap();
    assert!(json.len() < 50_000, "memory grows with visited cells, not the distance traveled");
    let mut loaded: Game = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded, game);
    for pos in positions {
        assert!(!loaded.sees(pos));
        assert!(loaded.remembers(pos));
    }
    loaded.player_mut().at = LevelId::Deep(loaded.level().towns().next().unwrap(), 1);
    assert!(!loaded.remembers((-1, -1)));
    loaded.player_mut().at = LevelId::Surface;
    assert!(loaded.remembers((-1, -1)));
    loaded.round(Some(Command::Wait));
    game.round(Some(Command::Wait));
    assert_eq!(loaded, game);
}

fn another_town(game: &mut Game) -> crate::Town {
    for y in [-384, 0, 384] {
        for x in [-384, 0, 384] {
            game.player_mut().pos = (x, y);
            game.round(Some(Command::Rest));
            if let Some(town) = game.level().towns().find(|town| town.origin != (0, 0)) {
                return town;
            }
        }
    }
    panic!("seed 7 has other settlements");
}

#[test]
fn other_towns_trade_heal_and_return_you_to_the_correct_stairs_after_reload() {
    let mut game = made();
    game.clear_creatures();
    let first = game.level().town_at(game.player().pos).unwrap();
    let second = another_town(&mut game);
    for town in [first, second, first] {
        game.player_mut().pos = town.start();
        game.round(Some(Command::Rest));
        game.player_mut().pos = town.shop();
        let gold = game.player().gold;
        round(&mut game, "sell potion");
        if game.player().sack.contains(&Item::Potion) {
            panic!("the outfitter buys the potion");
        }
        if gold == 50 {
            assert_eq!(game.player().gold, 62);
        }
        round(&mut game, "buy potion");
        assert!(game.player().sack.contains(&Item::Potion));
        game.player_mut().pos = town.respawn();
        game.player_mut().hp = 1;
        round(&mut game, "heal");
        assert_eq!(game.player().hp, game.player().max_hp);
        game.player_mut().pos = town.stairs();
        round(&mut game, "down");
        assert_eq!(game.player().at, LevelId::Deep(town, 1));
        game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        round(&mut game, "up");
        assert_eq!(game.player().pos, town.stairs());
        assert_eq!(game.level().town_at(game.player().pos), Some(town));
    }
}

#[test]
fn visiting_another_temple_changes_revival_and_death_keeps_the_new_town() {
    let mut game = made();
    game.clear_creatures();
    let town = another_town(&mut game);
    let temple = town.respawn();
    // Enter the temple through normal movement, including an intermediate step in a multi-step command.
    game.player_mut().pos = (temple.0, temple.1 + 1);
    game.round(Some(Command::Move(vec![crate::Dir::N, crate::Dir::S])));
    game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    game.player_mut().pos = town.start();
    game.player_mut().hp = 1;
    let me = game.player().pos;
    game.place(Kind::Troll, (me.0 + 1, me.1));
    for _ in 0..20 {
        game.round(Some(Command::Rest));
        if game.player().deaths > 0 {
            break;
        }
    }
    assert_eq!(game.player().deaths, 1);
    assert_eq!(game.player().pos, temple);
    assert_eq!(game.zone(), Zone::Temple);
    assert_eq!(game.level().town_at(temple), Some(town));
}

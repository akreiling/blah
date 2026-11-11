//! Whole-round behavior: commands through creatures, deaths, and saves.

use crate::{Bow, Class, Command, Game, Item, Kind, LevelId, Parsed, Phase, Spell, Target, Tone, Zone, parse};

fn made(class: &str) -> Game {
    let mut game = Game::new(7);
    game.answer("Tamsin");
    game.answer(class);
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
fn creation_asks_name_then_a_unique_calling() {
    let mut game = Game::new(1);
    game.answer("");
    assert_eq!(game.phase(), &Phase::Name);
    game.answer("tamsin");
    assert_eq!(game.phase(), &Phase::Calling("Tamsin".into()));
    game.answer("th");
    assert!(matches!(game.phase(), Phase::Calling(_)), "th is both thief and thaumaturge");
    game.answer("tha");
    assert_eq!(game.player().class, Class::Thaumaturge);
    assert!(game.player().max_mp > 0 && !game.creatures().is_empty());
}

#[test]
fn nock_then_shoot_kills_a_rat_and_drops_arrows_nearby() {
    let mut game = made("thief");
    game.clear_creatures();
    let me = game.player().pos;
    game.place(Kind::Rat, (me.0 + 3, me.1));
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
    let mut game = made("fighter");
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
    let mut game = made("fighter");
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
    let mut clocked = made("fighter");
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
fn woods_beasts_stay_out_of_town() {
    let mut game = made("thief");
    game.player_mut().max_hp = 999;
    game.player_mut().hp = 999;
    for _ in 0..300 {
        game.round(None);
    }
    assert!(!game.log().iter().any(|l| l.tone == crate::Tone::Hurt), "nothing reaches the town square");
    assert!(game.creatures().iter().filter(|c| c.at == LevelId::Surface).all(|c| crate::distance(c.pos, c.home) <= 7));
}

#[test]
fn shopping_buys_a_bow_and_arrows_and_sells_back() {
    let mut game = made("fighter");
    game.clear_creatures();
    game.player_mut().pos = (6, 5);
    assert_eq!(game.zone(), Zone::Shop);
    round(&mut game, "buy shortbow");
    round(&mut game, "buy arrows");
    let p = game.player();
    assert_eq!((p.bow, p.arrows, p.gold), (Some(Bow::Shortbow), 10, 0));
    round(&mut game, "sell potion");
    assert_eq!(game.player().gold, 12);
    round(&mut game, "buy long sword");
    assert!(said(&game, "That's 80 gold"));
}

#[test]
fn stairs_lead_down_and_back_up() {
    let mut game = made("fighter");
    game.clear_creatures();
    game.player_mut().pos = crate::Level::get(LevelId::Surface).stairs_down.unwrap();
    round(&mut game, "d");
    assert_eq!(game.player().at, LevelId::Undercroft);
    round(&mut game, "u");
    assert_eq!(game.player().at, LevelId::Surface);
    round(&mut game, "u");
    assert!(said(&game, "There are no stairs up here."));
}

#[test]
fn death_wakes_you_in_the_temple_and_leaves_your_gold() {
    let mut game = made("wizard");
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
fn spells_belong_to_their_callings() {
    let mut wizard = made("wizard");
    wizard.clear_creatures();
    wizard.round(Some(Command::Chant(Spell::Heal)));
    assert!(said(&wizard, "Only a thaumaturge"));
    wizard.round(Some(Command::Chant(Spell::Shield)));
    wizard.round(Some(Command::Cast(None)));
    assert!(wizard.player().shield > 0);
    wizard.player_mut().armor = Some(crate::Armor::Chainmail);
    wizard.round(Some(Command::Chant(Spell::Spark)));
    assert!(said(&wizard, "metal armor smothers"));

    let mut thaum = made("thaumaturge");
    thaum.clear_creatures();
    let me = thaum.player().pos;
    thaum.place(Kind::Goblin, (me.0 + 2, me.1));
    thaum.player_mut().max_hp = 999;
    thaum.player_mut().hp = 999;
    for _ in 0..10 {
        thaum.player_mut().mp = 99;
        round(&mut thaum, "chant stun");
        round(&mut thaum, "cast goblin");
        if thaum.creatures()[0].stunned > 0 {
            break;
        }
    }
    assert!(thaum.creatures()[0].stunned > 0, "stun lands within a few tries");
    thaum.player_mut().mp = thaum.player().max_mp;
    thaum.player_mut().hp = 3;
    round(&mut thaum, "chant heal");
    round(&mut thaum, "cast");
    assert!(said(&thaum, "Warmth knits your wounds"));
}

#[test]
fn targets_by_letter_and_by_ordinal_name() {
    let mut game = made("thief");
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
    let mut game = made("thief");
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
    let mut game = made("fighter");
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

    let mut game = made("fighter");
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
    assert!(said(&game, game.level().name_at(exit)));
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
    let mut game = made("fighter");
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
    loaded.player_mut().at = LevelId::Undercroft;
    assert!(!loaded.remembers((-1, -1)));
    loaded.player_mut().at = LevelId::Surface;
    assert!(loaded.remembers((-1, -1)));
    loaded.round(Some(Command::Wait));
    game.round(Some(Command::Wait));
    assert_eq!(loaded, game);
}

#[test]
fn pre_wilderness_saves_keep_their_character_and_gain_seeded_terrain() {
    let game = made("fighter");
    let mut old = serde_json::to_value(&game).unwrap();
    old.as_object_mut().unwrap().remove("terrain");
    old.as_object_mut().unwrap().remove("explored");
    let mut loaded: Game = serde_json::from_value(old).unwrap();
    assert_eq!(loaded.player(), game.player());
    assert_eq!(loaded.level().tile((-30, -30)), game.level().tile((-30, -30)));
    loaded.round(Some(Command::Wait));
    let mut expected = game;
    expected.round(Some(Command::Wait));
    assert_eq!(loaded, expected);
}

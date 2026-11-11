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
        game.round(Some(Command::Wait));
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

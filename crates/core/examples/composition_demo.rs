//! Run both composition trials through the same public Game entry points as the frontend.
//!
//! These are explicit developer fixtures, not evidence of earned progression or a balanced encounter.

use blah_core::{Dir, Game, LogKind, Maneuver, Rank, Step, Weapon};

fn main() {
    let mut game = Game::composition_trial(42, Maneuver::backstab_burst()).expect("trial ground");
    println!("DAGGER: {:?}", game.maneuver());
    println!("Before opening strike: {:?}", game.preview_attack(Dir::East).unwrap());
    assert_eq!(game.use_technique(Dir::East), Step::Attacked);
    println!("Opening result: {:?}", game.last_attack().unwrap());
    let w = game.wanderer();
    println!("Ranks after the opening: stealth {}, Focus {}", w.stealth.rank().name(), w.focus_skill.rank().name());
    println!("After detection: {:?}", game.preview_attack(Dir::East).unwrap());
    assert_eq!(game.use_technique(Dir::East), Step::Attacked);
    println!("Detected result: {:?}", game.last_attack().unwrap());
    println!("Focus left: {} of {}", game.wanderer().focus, game.wanderer().max_focus());

    let mut game = Game::inferno_trial(42).expect("trial ground");
    println!("\nSTAFF: {:?}", game.maneuver());
    println!("Area preview: {:?}", game.preview_area().unwrap());
    assert_eq!(game.unleash(), Step::Attacked);
    for entry in game.log().last(9) {
        println!("{}", entry.text);
    }
    println!("Health: {}, Focus: {}", game.wanderer().hp, game.wanderer().focus);
    assert_eq!(game.unleash(), Step::Blocked);
    println!("Immediate repeat: {}", game.log().lines().last().unwrap());
    let mut adjacent = None;
    for _ in 0..4 {
        adjacent = Dir::ALL.into_iter().find(|&dir| game.preview_strike(dir).is_ok());
        if adjacent.is_some() {
            break;
        }
        game.wait();
    }
    let direction = adjacent.expect("the survivor approaches within four turns");
    println!("Free staff strike preview: {:?}", game.preview_strike(direction).unwrap());
    assert_eq!(game.step(direction), Step::Attacked);
    println!("Free staff result: {:?}", game.last_attack().unwrap());
    assert_eq!(game.wanderer().focus, 6);

    let mut game = Game::composition_trial(42, Maneuver::default()).expect("trial ground");
    println!("\nWEAPON PRACTICE: {}", game.wanderer().weapons.get(Weapon::Dagger).rank().name());
    for _ in 0..32 {
        assert_eq!(game.step(Dir::East), Step::Attacked);
        if game.wanderer().weapons.get(Weapon::Dagger).rank() != Rank::Novice {
            let learned = game.log().entries().rfind(|e| e.kind == LogKind::Learning).unwrap();
            println!("{}", learned.text);
            println!("Earned strike preview: {:?}", game.preview_strike(Dir::East).unwrap());
            return;
        }
    }
    panic!("the trial must demonstrate an earned weapon rank");
}

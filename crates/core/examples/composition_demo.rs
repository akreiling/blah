//! Run both composition trials through the same public Game entry points as the frontend.
//!
//! These are explicit developer fixtures, not evidence of earned progression or a balanced encounter.

use blah_core::{Dir, Game, Maneuver, Step};

fn main() {
    let mut game = Game::composition_trial(42, Maneuver::backstab_burst()).expect("trial ground");
    println!("DAGGER: {:?}", game.maneuver());
    println!("Before opening strike: {:?}", game.preview_attack(Dir::East).unwrap());
    assert_eq!(game.step(Dir::East), Step::Attacked);
    println!("Opening result: {:?}", game.last_attack().unwrap());
    println!("After detection: {:?}", game.preview_attack(Dir::East).unwrap());
    assert_eq!(game.step(Dir::East), Step::Attacked);
    println!("Detected result: {:?}", game.last_attack().unwrap());
    println!("Focus left: {}", game.wanderer().focus);

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
}

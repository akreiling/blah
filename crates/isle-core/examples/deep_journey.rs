//! Fresh-character campaign diagnostic, not a difficulty or elapsed-human-time guarantee.
//! Run: cargo run --release -p isle-core --example deep_journey -- SEEDS ROUNDS FIRST_SEED

#[path = "deep_journey/pilot.rs"]
mod pilot;

use std::collections::BTreeMap;
use std::time::Instant;

use isle_core::{Command, Game, LevelId, Practice};
use pilot::Pilot;
use serde_json::{Value, json};

fn measure(seed: u64, limit: u64, prepared: bool, reload_each: bool) -> Value {
    let mut game = Game::new(seed);
    game.answer("Wren");
    let start = game.player().clone();
    let mut pilot = Pilot::new(&game, prepared);
    let mut first = [const { None }; 10];
    let mut entered = [None; 10];
    let mut categories: BTreeMap<&str, u64> = BTreeMap::new();
    let mut deaths_by_floor = [0; 11];
    let mut potions = 0;
    let mut buys = 0;
    let mut kills = 0;
    let mut blocked_moves = 0;
    let mut expert_blades = None;
    let mut shelter_recoveries = 0;
    let mut peak_depth = 0;
    let mut trail = std::collections::VecDeque::new();
    let mut bouncing_rounds = 0;
    for _ in 0..limit {
        let command = pilot.choose(&game);
        let category = match command {
            Command::Move(_) | Command::Down | Command::Up => "travel",
            Command::Rest => "rest",
            Command::Chant(_) | Command::Cast(_) => "magic",
            Command::Attack(_) | Command::Wait | Command::Nock | Command::Shoot(_) => "combat",
            Command::Quaff | Command::Use(_) | Command::Heal => "healing",
            _ => "gear_loot_shop",
        };
        *categories.entry(category).or_default() += 1;
        let safe = game.level().safe(game.player().pos);
        if matches!(command, Command::Rest) && safe {
            shelter_recoveries += 1;
        }
        if matches!(command, Command::Quaff) {
            potions += 1;
        }
        if matches!(command, Command::Buy(_)) {
            buys += 1;
        }
        let before = game.player().clone();
        let before_xp = before.xp;
        let moving = matches!(command, Command::Move(_));
        game.round(Some(command));
        if reload_each {
            game = serde_json::from_slice(&serde_json::to_vec(&game).unwrap()).unwrap();
        }
        if moving && game.player().at == before.at && game.player().pos == before.pos {
            blocked_moves += 1;
        }
        trail.push_back((game.player().at, game.player().pos));
        if trail.len() > 40 {
            trail.pop_front();
        }
        if pilot::bouncing(&trail) {
            bouncing_rounds += 1;
        }
        if game.player().practice.get(Practice::Blades).rank() == blah_core::Rank::Expert {
            expert_blades.get_or_insert(game.round_number());
        }
        if game.player().deaths > before.deaths {
            let depth = match before.at {
                LevelId::Surface => 0,
                LevelId::Deep(_, depth) => depth.min(10) as usize,
            };
            deaths_by_floor[depth] += 1;
        }
        if game.player().xp > before_xp {
            kills += 1;
        }
        if let LevelId::Deep(_, depth) = game.player().at {
            peak_depth = peak_depth.max(depth);
            if depth <= 10 {
                entered[(depth - 1) as usize].get_or_insert(game.round_number());
            }
        }
        for at in game.conquests() {
            if let LevelId::Deep(_, depth @ 1..=10) = at {
                first[(*depth - 1) as usize].get_or_insert_with(|| {
                    json!({
                        "round": game.round_number(), "deaths": game.player().deaths,
                        "hp": game.player().hp, "max_hp": game.player().max_hp,
                        "gold": game.player().gold, "weapon": game.player().weapon,
                        "armor": game.player().armor, "relic": game.player().relic,
                        "blades": game.player().practice.get(Practice::Blades).rank().name(),
                        "vitality": game.player().practice.get(Practice::Vitality).rank().name(),
                        "warding": game.player().practice.get(Practice::Warding).rank().name(),
                        "categories": categories,
                    })
                });
            }
        }
        if first[9].is_some() {
            break;
        }
    }
    assert_eq!(categories.values().sum::<u64>(), game.round_number());
    json!({
        "seed": seed, "policy": if prepared { "warded" } else { "melee" },
        "start": {"str": start.str, "dex": start.dex, "int": start.int, "con": start.con, "hp": start.hp},
        "rounds": game.round_number(), "first_conquests": first, "first_entries": entered,
        "deaths": game.player().deaths, "deaths_by_floor": deaths_by_floor,
        "categories": categories, "shelter_rest_rounds": shelter_recoveries,
        "retreats": pilot.retreats, "potions_used": potions, "purchases": buys,
        "kills": kills, "blocked_moves": blocked_moves, "bouncing_rounds": bouncing_rounds, "first_expert_blades": expert_blades,
        "peak_depth": peak_depth, "final_xp": game.player().xp,
        "final_hp": game.player().max_hp, "final_focus": game.player().max_focus,
        "final_blades": game.player().practice.get(Practice::Blades).rank().name(),
        "final_vitality": game.player().practice.get(Practice::Vitality).rank().name(),
        "final_warding": game.player().practice.get(Practice::Warding).rank().name(),
        "final_weapon": game.player().weapon, "final_armor": game.player().armor,
        "final_relic": game.player().relic,
    })
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(args.len() <= 3, "SEEDS ROUNDS FIRST_SEED");
    let parse =
        |index: usize, default: u64| args.get(index).map_or(default, |s| s.parse::<u64>().expect("positive integer"));
    let (seeds, rounds, first) = (parse(0, 16), parse(1, 20_000), parse(2, 1));
    assert!(seeds > 0 && rounds > 0 && first.checked_add(seeds).is_some());
    let began = Instant::now();
    for seed in first..first + seeds {
        for prepared in [false, true] {
            println!("{}", measure(seed, rounds, prepared, false));
        }
        eprintln!("seed {seed} finished, {:.2}s", began.elapsed().as_secs_f64());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_runs_are_reproducible_and_reload_preserves_the_policy() {
        for prepared in [false, true] {
            let first = measure(7, 600, prepared, false);
            assert_eq!(first, measure(7, 600, prepared, false));
            assert_eq!(first, measure(7, 600, prepared, true));
            assert_eq!(first["start"]["hp"], 22);
            assert_eq!(first["rounds"], 600);
        }
    }

    #[test]
    #[ignore = "diagnostic for capped journeys"]
    fn inspect_capped_journeys() {
        for (seed, prepared) in [(1, false), (22, false), (29, true)] {
            let mut game = Game::new(seed);
            game.answer("Wren");
            let mut pilot = Pilot::new(&game, prepared);
            let mut tail = std::collections::VecDeque::new();
            for round in 0..20_000 {
                let command = pilot.choose(&game);
                if round >= 19_988 {
                    tail.push_back(format!(
                        "{} {:?} pos {:?}, hp {}, focus {}, safe {}, visible {:?}",
                        round,
                        command,
                        game.player().pos,
                        game.player().hp,
                        game.player().focus,
                        game.level().safe(game.player().pos),
                        game.sighted()
                            .iter()
                            .map(|s| (
                                s.creature.id,
                                s.creature.pos,
                                s.creature.hp,
                                s.creature.phenotype().map(|p| p.temperament)
                            ))
                            .collect::<Vec<_>>()
                    ));
                }
                game.round(Some(command));
                if game.conquests().iter().any(|at| matches!(at, LevelId::Deep(_, 10))) {
                    eprintln!("seed {seed} completed at {}", game.round_number());
                    break;
                }
            }
            eprintln!("seed {seed} prepared {prepared}: {tail:#?}");
        }
    }
}

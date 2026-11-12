//! Finds that change a build through a benefit and a cost, never through permanent practice.

use serde::{Deserialize, Serialize};

/// One relic can be worn at a time, independently of armor and the ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Relic {
    /// Hotter sparks at the cost of weaker healing magic.
    EmberCharm,
    /// A stronger active ward at the cost of Focus capacity.
    Stillstone,
    /// Truer arrows at the cost of finding cover.
    HuntersToken,
}

impl Relic {
    /// Every relic in the first wilderness set.
    pub const ALL: [Self; 3] = [Self::EmberCharm, Self::Stillstone, Self::HuntersToken];

    /// Its name in the log and commands.
    pub const fn name(self) -> &'static str {
        match self {
            Self::EmberCharm => "ember charm",
            Self::Stillstone => "stillstone",
            Self::HuntersToken => "hunter's token",
        }
    }

    /// Both sides of the trade-off, while worn.
    pub const fn description(self) -> &'static str {
        match self {
            Self::EmberCharm => "+2 Spark damage; -3 Heal recovery (minimum 1)",
            Self::Stillstone => "+2 active ward protection; -4 maximum Focus",
            Self::HuntersToken => "+2 archery accuracy; -15 percentage points to hiding attempts",
        }
    }

    pub(crate) const fn words(self) -> &'static str {
        match self {
            Self::EmberCharm => "ember charm relic",
            Self::Stillstone => "stillstone stone relic",
            Self::HuntersToken => "hunter's hunter token relic",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Game, Instant, Item, Kind, Parsed, SACK, Spell, parse};

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

    fn wear(game: &mut Game, relic: Relic) {
        game.player_mut().sack.push(Item::Relic(relic));
        run(game, &format!("wear {}", relic.name()));
        assert_eq!(game.player().relic, Some(relic));
    }

    fn target(game: &mut Game, paused: bool) {
        let pos = game.player().pos;
        game.place(Kind::Troll, (pos.0 + 1, pos.1));
        if paused {
            let mut save = serde_json::to_value(&game).unwrap();
            save["creatures"][0]["stunned"] = serde_json::json!(100);
            *game = serde_json::from_value(save).unwrap();
        }
    }

    #[test]
    fn relic_slot_swaps_safely_and_never_refills_focus_or_changes_practice() {
        let mut game = made(7);
        let practice = game.player().practice;
        let rng = serde_json::to_value(&game).unwrap()["rng"].clone();
        wear(&mut game, Relic::Stillstone);
        assert_eq!((game.player().focus, game.player().max_focus), (20, 20));
        for _ in 0..4 {
            run(&mut game, "remove relic");
            assert_eq!((game.player().focus, game.player().max_focus), (20, 24));
            run(&mut game, "wear stillstone");
            assert_eq!((game.player().focus, game.player().max_focus), (20, 20));
        }
        game.player_mut().focus = 7;
        game.player_mut().sack = vec![Item::Potion; SACK];
        let before = game.player().clone();
        run(&mut game, "remove stillstone");
        assert_eq!(game.player(), &before, "a full sack cannot lose the equipped item");
        game.player_mut().sack[0] = Item::Relic(Relic::EmberCharm);
        run(&mut game, "wear ember");
        assert_eq!(game.player().relic, Some(Relic::EmberCharm));
        assert_eq!(game.player().sack.len(), SACK);
        assert_eq!(game.player().sack[0], Item::Relic(Relic::Stillstone));
        assert_eq!((game.player().focus, game.player().max_focus), (7, 24));
        run(&mut game, "remove hunter");
        assert_eq!(game.player().relic, Some(Relic::EmberCharm));
        assert_eq!(game.player().practice, practice);
        assert_eq!(serde_json::to_value(&game).unwrap()["rng"], rng);
    }

    #[test]
    fn relic_inventory_explains_both_sides_before_equipping_and_survives_reload() {
        let mut game = made(7);
        assert!(game.player().relic.is_none());
        game.player_mut().sack.extend(Relic::ALL.map(Item::Relic));
        let round = game.round_number();
        game.instant(Instant::Inventory);
        assert_eq!(game.round_number(), round);
        for relic in Relic::ALL {
            assert!(game.log().iter().any(|line| line.text.contains(relic.description())));
        }
        run(&mut game, "wear stillstone");
        game.player_mut().chanted = Some(Spell::Spark);
        game.player_mut().shield = 5;
        game.player_mut().focus = 3;
        let mut resumed: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        assert_eq!(resumed, game);
        run(&mut resumed, "remove relic");
        assert_eq!((resumed.player().focus, resumed.player().max_focus), (3, 24));
        assert_eq!(resumed.player().chanted, Some(Spell::Spark));
        assert_eq!(resumed.player().ward_protection(false), 3);
        let mut old = serde_json::to_value(made(7)).unwrap();
        old["player"].as_object_mut().unwrap().remove("relic");
        let old: Game = serde_json::from_value(old).unwrap();
        assert!(old.player().relic.is_none());
    }

    #[test]
    fn ember_charm_adds_two_spark_damage_and_removes_three_heal_recovery() {
        let mut hits = 0;
        let mut smallest_heal = i32::MAX;
        for seed in 0..64 {
            let mut base = made(seed);
            let mut ember = base.clone();
            base.round(Some(Command::Rest));
            wear(&mut ember, Relic::EmberCharm);
            for game in [&mut base, &mut ember] {
                game.player_mut().hp = game.player().max_hp - 20;
                run(game, "chant heal");
                run(game, "cast");
            }
            assert_eq!(base.player().hp - ember.player().hp, 3);
            smallest_heal = smallest_heal.min(ember.player().hp - (ember.player().max_hp - 20));
            for game in [&mut base, &mut ember] {
                game.player_mut().hp = game.player().max_hp;
                target(game, true);
                run(game, "chant spark");
                run(game, "cast a");
            }
            if base.log().iter().any(|line| line.text.starts_with("A spark leaps")) {
                hits += 1;
                assert_eq!(base.creatures()[0].hp - ember.creatures()[0].hp, 2);
            } else {
                assert_eq!(base.creatures()[0].hp, ember.creatures()[0].hp);
            }
        }
        assert!(hits > 16, "exercise landed casts rather than only misses");
        assert_eq!(smallest_heal, 1, "exercise the weakest possible recovery");
    }

    #[test]
    fn ember_charm_does_not_weaken_potions_or_herbs() {
        let mut base = made(7);
        let mut ember = base.clone();
        base.round(Some(Command::Rest));
        wear(&mut ember, Relic::EmberCharm);
        for game in [&mut base, &mut ember] {
            game.player_mut().hp = 1;
            run(game, "quaff");
        }
        assert_eq!(base.player().hp, ember.player().hp);
        for game in [&mut base, &mut ember] {
            game.player_mut().hp = 1;
            game.player_mut().sack.push(Item::Herb(crate::Herb::Woundleaf));
            run(game, "use woundleaf");
        }
        assert_eq!((base.player().hp, ember.player().hp), (7, 7));
    }

    #[test]
    fn hunters_token_improves_real_shots_without_changing_landed_damage() {
        let mut improved = 0;
        for seed in 0..128 {
            let mut base = made(seed);
            base.player_mut().dex = 10;
            let mut token = base.clone();
            base.round(Some(Command::Rest));
            wear(&mut token, Relic::HuntersToken);
            for game in [&mut base, &mut token] {
                target(game, true);
                run(game, "nock");
                run(game, "shoot a");
            }
            let hit = |game: &Game| game.log().iter().any(|line| line.text.starts_with("Your arrow strikes"));
            if hit(&base) {
                assert!(hit(&token));
                assert_eq!(base.creatures()[0].hp, token.creatures()[0].hp);
            } else if hit(&token) {
                improved += 1;
            }
        }
        assert!(improved > 0, "the extra accuracy must turn actual misses into hits");
    }

    #[test]
    fn hunters_token_reduces_real_hiding_attempts_by_fifteen_points_even_at_the_cap() {
        let mut changed = 0;
        for seed in 0..128 {
            let mut base = made(seed);
            base.player_mut().dex = 10;
            let mut token = base.clone();
            base.round(Some(Command::Rest));
            wear(&mut token, Relic::HuntersToken);
            assert_eq!(base.player().hiding_chance(0), 60);
            assert_eq!(token.player().hiding_chance(0), 45);
            let mut dice: crate::rng::Rng =
                serde_json::from_value(serde_json::to_value(&token).unwrap()["rng"].clone()).unwrap();
            let expected = dice.chance(45);
            run(&mut base, "hide");
            run(&mut token, "hide");
            assert_eq!(token.player().hidden, expected);
            if base.player().hidden && !token.player().hidden {
                changed += 1;
            }
            token.player_mut().dex = 100;
            assert_eq!(token.player().hiding_chance(0), 85);
            token.player_mut().dex = 0;
            assert_eq!(token.player().hiding_chance(20), 0);
        }
        assert!(changed > 0);
    }

    #[test]
    fn stillstone_only_strengthens_an_active_ward_and_prevents_real_blows() {
        let mut prevented = 0;
        for seed in 0..128 {
            let mut base = made(seed);
            base.player_mut().dex = 10;
            base.player_mut().armor = None;
            let mut stone = base.clone();
            base.round(Some(Command::Rest));
            wear(&mut stone, Relic::Stillstone);
            assert_eq!(stone.player().ward_protection(false), 0);
            for game in [&mut base, &mut stone] {
                game.player_mut().shield = 5;
                target(game, false);
            }
            assert_eq!((base.player().ward_protection(false), base.player().ward_protection(true)), (3, 6));
            assert_eq!((stone.player().ward_protection(false), stone.player().ward_protection(true)), (5, 8));
            for game in [&mut base, &mut stone] {
                run(game, "get nothing");
            }
            assert!(stone.player().hp >= base.player().hp);
            if stone.player().hp > base.player().hp {
                prevented += 1;
                assert!(stone.log().iter().any(|line| line.text.starts_with("Your ward turns aside")));
            }
        }
        assert!(prevented > 0, "extra protection must prevent an otherwise landed blow");
    }

    #[test]
    fn relics_come_from_seeded_caches_not_starter_gear_or_shop_stock() {
        let game = made(7);
        assert!(!game.player().sack.iter().any(|item| matches!(item, Item::Relic(_))));
        let mut found = std::collections::HashSet::new();
        for y in -8..=8 {
            for x in -8..=8 {
                if let Some(site) = crate::Landmark::generate(7, (x, y), &game.level()) {
                    let first = site.contents(7);
                    assert_eq!(first, site.contents(7));
                    for item in first {
                        if let Item::Relic(relic) = item {
                            found.insert(relic);
                        }
                    }
                    if site.kind == crate::LandmarkKind::Shrine {
                        assert!(site.contents(7).iter().any(|item| matches!(item, Item::Relic(_))));
                    }
                }
            }
        }
        assert_eq!(found.len(), 3);
        for relic in Relic::ALL {
            assert!(!crate::SHOP.iter().any(|(item, _)| *item == Item::Relic(relic)));
            assert_eq!(crate::sale_price(Item::Relic(relic)), 75);
        }
    }
}

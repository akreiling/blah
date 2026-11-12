//! Biome-specific field supplies, with finite, seeded patches.

use blah_core::Biome;
use serde::{Deserialize, Serialize};

use crate::landmark::{Landmark, SECTOR as LANDMARK_SECTOR};
use crate::map::{LevelId, Place, Pos, Tile};

/// Cells per side of a herb sector.
pub(crate) const SECTOR: i32 = 16;

/// A gathered herb, carried as one item in the sack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Herb {
    /// A field and woodland herb for dressing wounds.
    Woundleaf,
    /// Wetland mint that clears a weary mind.
    Bogmint,
    /// A dry-country root that restores a little of both resources.
    Sunroot,
}

impl Herb {
    /// Its name in commands and the log.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Woundleaf => "woundleaf",
            Self::Bogmint => "bogmint",
            Self::Sunroot => "sunroot",
        }
    }

    /// Maximum Health and Focus restored by one use; neither resource can exceed its capacity.
    pub const fn recovery(self) -> (i32, i32) {
        match self {
            Self::Woundleaf => (6, 0),
            Self::Bogmint => (0, 8),
            Self::Sunroot => (3, 4),
        }
    }

    /// What using it restores.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Woundleaf => "restores 6 Health",
            Self::Bogmint => "restores 8 Focus",
            Self::Sunroot => "restores 3 Health and 4 Focus",
        }
    }

    pub(crate) fn in_biome(biome: &Biome) -> Option<Self> {
        match biome.name {
            "Grassland" | "Forest" | "Hills" => Some(Self::Woundleaf),
            "Swamp" | "Jungle" => Some(Self::Bogmint),
            "Desert" | "Savanna" | "Badlands" => Some(Self::Sunroot),
            _ => None,
        }
    }
}

/// One discovered patch. Harvesting never replenishes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HerbPatch {
    /// Its surface cell.
    pub pos: Pos,
    /// What grows here.
    pub herb: Herb,
    /// Whether its single useful herb has been gathered.
    pub picked: bool,
}

impl HerbPatch {
    pub(crate) fn generate(seed: u64, sector: Pos, level: &Place) -> Option<Self> {
        if level.id != LevelId::Surface {
            return None;
        }
        let mut rng = blah_core::Rng::keyed(seed, b"isl-herb", &[sector.0 as u64, sector.1 as u64]);
        for _ in 0..4 {
            let pos = (sector.0 * SECTOR + 2 + rng.below(12) as i32, sector.1 * SECTOR + 2 + rng.below(12) as i32);
            if level.town_at(pos).is_some() || !matches!(level.tile(pos), Tile::Wild(_)) {
                continue;
            }
            let Some(herb) = Herb::in_biome(level.ground(pos)) else { continue };
            let site_sector = (pos.0.div_euclid(LANDMARK_SECTOR), pos.1.div_euclid(LANDMARK_SECTOR));
            if Landmark::generate(seed, site_sector, level).is_some_and(|site| site.pos == pos) {
                continue;
            }
            if [(0, 1), (1, 0), (0, -1), (-1, 0)]
                .iter()
                .any(|&(dx, dy)| level.tile((pos.0 + dx, pos.1 + dy)).walkable())
            {
                return Some(Self { pos, herb, picked: false });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Dir, Game, Instant, Item, Kind, Parsed, SACK, parse};

    fn made() -> Game {
        let mut game = Game::new(7);
        game.answer("Wren");
        game.clear_creatures();
        game
    }

    fn patch(game: &Game) -> HerbPatch {
        (-3..=3)
            .flat_map(|y| (-3..=3).map(move |x| (x, y)))
            .find_map(|sector| HerbPatch::generate(7, sector, &game.level()))
            .expect("a natural patch near town")
    }

    fn visit(game: &mut Game, patch: HerbPatch) {
        game.player_mut().pos = patch.pos;
        game.round(Some(Command::Rest));
        assert_eq!(game.herb_at(patch.pos), Some(&patch));
    }

    fn run(game: &mut Game, line: &str) {
        match parse(line).unwrap() {
            Parsed::Round(command) => game.round(Some(command)),
            Parsed::Instant(command) => game.instant(command),
        }
    }

    #[test]
    fn herbs_belong_to_their_biomes_and_never_water_or_cold_bare_ground() {
        for biome in blah_core::BIOMES {
            let expected = match biome.name {
                "Grassland" | "Forest" | "Hills" => Some(Herb::Woundleaf),
                "Swamp" | "Jungle" => Some(Herb::Bogmint),
                "Desert" | "Savanna" | "Badlands" => Some(Herb::Sunroot),
                _ => None,
            };
            assert_eq!(Herb::in_biome(biome), expected, "{}", biome.name);
        }
    }

    #[test]
    fn herb_patches_are_seeded_accessible_and_separate_from_towns_and_landmarks() {
        let mut game = made();
        let mut kinds = std::collections::HashSet::new();
        let mut count = 0;
        for pos in [(-36, 273), (94, 39), (-96, 359), (6, 0)] {
            game.player_mut().pos = pos;
            game.round(Some(Command::Rest));
            let level = game.level();
            let (sx, sy) = (pos.0.div_euclid(SECTOR), pos.1.div_euclid(SECTOR));
            let sectors: Vec<_> = (sy - 3..=sy + 3).flat_map(|y| (sx - 3..=sx + 3).map(move |x| (x, y))).collect();
            let forward: Vec<_> = sectors.iter().map(|&s| HerbPatch::generate(7, s, &level)).collect();
            let mut reverse: Vec<_> = sectors.iter().rev().map(|&s| HerbPatch::generate(7, s, &level)).collect();
            reverse.reverse();
            assert_eq!(forward, reverse);
            for patch in forward.into_iter().flatten() {
                assert!(level.town_at(patch.pos).is_none());
                assert!(matches!(level.tile(patch.pos), Tile::Wild(id) if !id.biome().water && id.biome().passable));
                assert_eq!(Herb::in_biome(level.ground(patch.pos)), Some(patch.herb));
                let s = (patch.pos.0.div_euclid(LANDMARK_SECTOR), patch.pos.1.div_euclid(LANDMARK_SECTOR));
                assert!(Landmark::generate(7, s, &level).is_none_or(|site| site.pos != patch.pos));
                kinds.insert(patch.herb);
                count += 1;
            }
        }
        assert_eq!(kinds.len(), 3);
        assert!(count > 30);
    }

    #[test]
    fn herbs_are_discovered_only_in_sight_and_gathered_once_across_reload() {
        let mut game = made();
        let patch = patch(&game);
        assert!(game.herb_at(patch.pos).is_none());
        game.player_mut().pos = (patch.pos.0 + 20, patch.pos.1);
        game.round(Some(Command::Rest));
        assert!(game.herb_at(patch.pos).is_none());
        visit(&mut game, patch);
        assert!(game.remembers(patch.pos));
        let practice = game.player().practice;
        let rng = serde_json::to_value(&game).unwrap()["rng"].clone();
        let before = game.round_number();
        run(&mut game, "get herb");
        assert_eq!(game.round_number(), before + 1);
        assert!(game.herb_at(patch.pos).unwrap().picked);
        assert!(game.player().sack.contains(&Item::Herb(patch.herb)));
        assert_eq!(game.player().practice, practice);
        assert_eq!(serde_json::to_value(&game).unwrap()["rng"], rng);
        let mut resumed: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        assert_eq!(resumed, game);
        let before = resumed.player().clone();
        run(&mut resumed, "get all");
        assert_eq!(resumed.player(), &before);
        let town = resumed.level().towns().next().unwrap();
        resumed.player_mut().at = LevelId::Deep(town, 1);
        assert!(resumed.herb_at(patch.pos).is_none());
        assert!(HerbPatch::generate(7, (0, 0), &resumed.level()).is_none());
    }

    #[test]
    fn herbs_with_full_sack_stay_growing_and_named_gathering_coexists_with_a_pile() {
        let mut game = made();
        let patch = patch(&game);
        visit(&mut game, patch);
        game.player_mut().sack = vec![Item::Potion; SACK];
        run(&mut game, "get all");
        assert!(!game.herb_at(patch.pos).unwrap().picked);
        let mut game: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        run(&mut game, "drop potion");
        let mark = game.log_total();
        run(&mut game, &format!("get {}", patch.herb.name()));
        assert!(game.herb_at(patch.pos).unwrap().picked);
        assert_eq!(game.pile_at(patch.pos).unwrap().items, vec![Item::Potion]);
        assert_eq!(game.player().sack.len(), SACK);
        assert!(
            !game.log().iter().rev().take((game.log_total() - mark) as usize).any(|l| l.text.contains("You see no"))
        );
        run(&mut game, &format!("drop {}", patch.herb.name()));
        run(&mut game, "get herb");
        assert_eq!(game.player().sack.iter().filter(|&&i| i == Item::Herb(patch.herb)).count(), 1);
    }

    #[test]
    fn herb_recovery_is_exact_capped_and_grants_no_practice_or_combat_dice() {
        for (herb, expected) in [(Herb::Woundleaf, (6, 0)), (Herb::Bogmint, (0, 8)), (Herb::Sunroot, (3, 4))] {
            let mut game = made();
            game.player_mut().sack = vec![Item::Herb(herb); 2];
            game.player_mut().hp -= 10;
            game.player_mut().focus -= 12;
            let before = game.player().clone();
            let rng = serde_json::to_value(&game).unwrap()["rng"].clone();
            run(&mut game, &format!("use {}", herb.name()));
            assert_eq!(game.player().hp, before.hp + expected.0);
            assert_eq!(game.player().focus, before.focus + expected.1);
            assert_eq!(game.player().sack, vec![Item::Herb(herb)]);
            assert_eq!(game.player().practice, before.practice);
            assert_eq!(serde_json::to_value(&game).unwrap()["rng"], rng);
            game.player_mut().hp = game.player().max_hp - 1;
            game.player_mut().focus = game.player().max_focus - 1;
            run(&mut game, "use herb");
            assert_eq!(game.player().hp, game.player().max_hp - i32::from(expected.0 == 0));
            assert_eq!(game.player().focus, game.player().max_focus - i32::from(expected.1 == 0));
            assert!(game.player().sack.is_empty());
        }
    }

    #[test]
    fn herbs_that_cannot_help_and_invalid_uses_do_not_consume_items() {
        let mut game = made();
        game.player_mut().sack = vec![Item::Herb(Herb::Woundleaf), Item::Potion];
        let before = game.player().clone();
        run(&mut game, "use woundleaf");
        run(&mut game, "use potion");
        run(&mut game, "use bogmint");
        assert_eq!(game.player(), &before);
        game.player_mut().focus -= 4;
        run(&mut game, "use woundleaf");
        assert_eq!(game.player().sack, before.sack, "Health-only herbs do not treat missing Focus");
    }

    #[test]
    fn gathered_herbs_sell_for_two_gold_but_are_not_shop_stock() {
        let mut game = made();
        let shop = game.level().towns().next().unwrap().shop();
        game.player_mut().pos = shop;
        let gold = game.player().gold;
        for herb in [Herb::Woundleaf, Herb::Bogmint, Herb::Sunroot] {
            assert!(!crate::SHOP.iter().any(|(item, _)| *item == Item::Herb(herb)));
            game.player_mut().sack.push(Item::Herb(herb));
            let before = game.player().gold;
            run(&mut game, &format!("sell {}", herb.name()));
            assert_eq!(game.player().gold, before + 2);
            assert!(!game.player().sack.contains(&Item::Herb(herb)));
        }
        assert_eq!(game.player().gold, gold + 6);
    }

    #[test]
    fn gathering_and_herb_use_allow_adjacent_hunters_to_act() {
        let mut game = made();
        let patch = patch(&game);
        visit(&mut game, patch);
        let near = [(0, 1), (1, 0), (0, -1), (-1, 0)]
            .into_iter()
            .map(|(dx, dy)| (patch.pos.0 + dx, patch.pos.1 + dy))
            .find(|&p| game.level().tile(p).walkable())
            .unwrap();
        game.place(Kind::Orc, near);
        let mark = game.log_total();
        run(&mut game, "get all");
        assert!(game.log().iter().rev().take((game.log_total() - mark) as usize).any(|l| l.text.contains("orc")));
        game.player_mut().hp = game.player().max_hp - 1;
        game.player_mut().focus -= 1;
        let mark = game.log_total();
        run(&mut game, &format!("use {}", patch.herb.name()));
        assert!(game.log().iter().rev().take((game.log_total() - mark) as usize).any(|l| l.text.contains("orc")));
        assert!(!game.player().sack.contains(&Item::Herb(patch.herb)));
    }

    #[test]
    fn herbs_can_be_seen_during_multi_step_movement_and_old_saves_can_discover_them() {
        let mut game = made();
        let patch = patch(&game);
        let near = [(Dir::N, (0, 1)), (Dir::S, (0, -1)), (Dir::E, (-1, 0)), (Dir::W, (1, 0))]
            .into_iter()
            .find(|(_, (dx, dy))| game.level().tile((patch.pos.0 + dx, patch.pos.1 + dy)).walkable())
            .unwrap();
        game.player_mut().pos = (patch.pos.0 + near.1.0, patch.pos.1 + near.1.1);
        game.round(Some(Command::Move(vec![near.0, near.0, near.0])));
        assert!(game.herb_at(patch.pos).is_some() && game.remembers(patch.pos));
        assert!(game.log().iter().any(|l| l.text.contains("grows here")));
        let mut old = serde_json::to_value(&game).unwrap();
        old.as_object_mut().unwrap().remove("herbs");
        let mut old: Game = serde_json::from_value(old).unwrap();
        old.player_mut().pos = patch.pos;
        old.instant(Instant::Look);
        assert_eq!(old.herb_at(patch.pos), Some(&patch));
    }

    #[test]
    fn herb_healing_consumes_recorded_injury_without_teaching_restoration() {
        let mut game = made();
        let patch = patch(&game);
        visit(&mut game, patch);
        let near = [(0, 1), (1, 0), (0, -1), (-1, 0)]
            .into_iter()
            .map(|(dx, dy)| (patch.pos.0 + dx, patch.pos.1 + dy))
            .find(|&p| game.level().tile(p).walkable())
            .unwrap();
        game.place(Kind::Orc, near);
        for _ in 0..100 {
            game.round(Some(Command::Rest));
            if game.player().max_hp - game.player().hp >= 6 {
                break;
            }
        }
        assert_eq!(game.player().deaths, 0);
        game.clear_creatures();
        let injuries = |game: &Game| -> i64 {
            serde_json::to_value(game).unwrap()["player"]["injuries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|injury| injury["remaining"].as_i64().unwrap())
                .sum()
        };
        let remaining = injuries(&game);
        assert!(remaining >= 6, "exercise real tracked wounds");
        game.player_mut().sack.push(Item::Herb(Herb::Woundleaf));
        let before = game.player().clone();
        run(&mut game, "use woundleaf");
        assert_eq!(game.player().hp, before.hp + 6);
        assert_eq!(injuries(&game), remaining - 6);
        assert_eq!(game.player().practice, before.practice);
    }

    #[test]
    fn herb_harvest_does_not_reset_after_death_and_return() {
        let mut game = made();
        let patch = patch(&game);
        visit(&mut game, patch);
        run(&mut game, "get all");
        let count = game.player().sack.iter().filter(|&&item| item == Item::Herb(patch.herb)).count();
        game.player_mut().hp = 1;
        let near = [(0, 1), (1, 0), (0, -1), (-1, 0)]
            .into_iter()
            .map(|(dx, dy)| (patch.pos.0 + dx, patch.pos.1 + dy))
            .find(|&p| game.level().tile(p).walkable())
            .unwrap();
        game.place(Kind::Troll, near);
        for _ in 0..100 {
            game.round(Some(Command::Rest));
            if game.player().deaths > 0 {
                break;
            }
        }
        assert_eq!(game.player().deaths, 1);
        assert!(game.herbs().iter().find(|p| p.pos == patch.pos).unwrap().picked);
        game.clear_creatures();
        game.player_mut().pos = patch.pos;
        run(&mut game, "get all");
        assert_eq!(game.player().sack.iter().filter(|&&item| item == Item::Herb(patch.herb)).count(), count);
    }
}

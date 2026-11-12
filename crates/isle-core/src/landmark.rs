//! Seeded wilderness sites. Only discovered sites and their opened state enter the save.

use serde::{Deserialize, Serialize};

use crate::data::{Item, treasure};
use crate::map::{Place, Pos, Tile};
use crate::rng::Rng;

/// Cells per side of a landmark sector.
pub(crate) const SECTOR: i32 = 32;

/// The remains of an earlier traveler or settlement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LandmarkKind {
    /// A weathered heap of memorial stones.
    Cairn,
    /// A cold fire and a traveler's abandoned shelter.
    Camp,
    /// The broken pillars of a small wayside shrine.
    Shrine,
}

impl LandmarkKind {
    /// What the site is called in directions and discovery messages.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cairn => "weathered cairn",
            Self::Camp => "abandoned camp",
            Self::Shrine => "ruined shrine",
        }
    }

    /// What the character finds underfoot.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Cairn => "Loose stones shelter a traveler's cache.",
            Self::Camp => "A waxed bundle lies beside the cold fire.",
            Self::Shrine => "A small coffer rests beneath the broken altar.",
        }
    }
}

/// A discovered wilderness site, with a durable, one-time cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Landmark {
    /// Its surface cell.
    pub pos: Pos,
    /// What remains here.
    pub kind: LandmarkKind,
    /// Whether the cache has been opened, even if some contents were left on the ground.
    pub opened: bool,
}

impl Landmark {
    pub(crate) fn generate(seed: u64, sector: Pos, level: &Place) -> Option<Self> {
        let mut rng = blah_core::Rng::keyed(seed, b"isl-site", &[sector.0 as u64, sector.1 as u64]);
        let kind = [LandmarkKind::Cairn, LandmarkKind::Camp, LandmarkKind::Shrine][rng.below(3)];
        for _ in 0..8 {
            let pos = (sector.0 * SECTOR + 3 + rng.below(26) as i32, sector.1 * SECTOR + 3 + rng.below(26) as i32);
            // Town footprints never host sites, and sites neither clear trees nor alter the land.
            if level.town_at(pos).is_none()
                && matches!(level.tile(pos), Tile::Wild(id) if id.biome().passable && !id.biome().water)
                && [(0, 1), (1, 0), (0, -1), (-1, 0)]
                    .iter()
                    .all(|&(dx, dy)| level.tile((pos.0 + dx, pos.1 + dy)).walkable())
            {
                return Some(Self { pos, kind, opened: false });
            }
        }
        None
    }

    pub(crate) fn contents(self, seed: u64) -> Vec<Item> {
        let key = blah_core::Rng::keyed(seed, b"isl-loot", &[self.pos.0 as u64, self.pos.1 as u64]).next_u64();
        let mut rng = Rng::new(key);
        let mut items = treasure(90 + rng.below(91), 100, &mut rng);
        items.push(match self.kind {
            LandmarkKind::Camp => Item::Arrows(12),
            _ => Item::Potion,
        });
        // A separate stream leaves ordinary treasure and combat dice unchanged.
        let mut relics = blah_core::Rng::keyed(seed, b"isl-reli", &[self.pos.0 as u64, self.pos.1 as u64]);
        if self.kind == LandmarkKind::Shrine || relics.below(4) == 0 {
            items.push(Item::Relic(crate::Relic::ALL[relics.below(crate::Relic::ALL.len())]));
        }
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Dir, Game, Instant, LevelId, SACK};

    fn made(seed: u64) -> Game {
        let mut game = Game::new(seed);
        game.answer("Wren");
        game.clear_creatures();
        game
    }

    fn site(game: &Game) -> Landmark {
        (-3..=3)
            .flat_map(|y| (-3..=3).map(move |x| (x, y)))
            .find_map(|sector| Landmark::generate(game.seed(), sector, &game.level()))
            .expect("a dry site near the starting town")
    }

    fn visit(game: &mut Game, site: Landmark) {
        game.player_mut().pos = site.pos;
        game.round(Some(Command::Wait));
        assert_eq!(game.landmark_at(site.pos), Some(&site));
    }

    #[test]
    fn landmark_generation_is_seeded_dry_clear_and_outside_towns() {
        let mut kinds = std::collections::BTreeSet::new();
        let mut count = 0;
        for seed in 0..16 {
            let game = made(seed);
            let level = game.level();
            let sectors: Vec<Pos> = (-3..=3).flat_map(|y| (-3..=3).map(move |x| (x, y))).collect();
            let forward: Vec<_> = sectors.iter().map(|&s| Landmark::generate(seed, s, &level)).collect();
            let mut reverse: Vec<_> = sectors.iter().rev().map(|&s| Landmark::generate(seed, s, &level)).collect();
            reverse.reverse();
            assert_eq!(forward, reverse, "visit order must not pick sites");
            for (sector, site) in sectors.iter().zip(forward).filter_map(|(s, site)| site.map(|site| (s, site))) {
                assert_eq!((site.pos.0.div_euclid(SECTOR), site.pos.1.div_euclid(SECTOR)), *sector);
                assert!(level.town_at(site.pos).is_none());
                assert!(matches!(level.tile(site.pos), Tile::Wild(id) if !id.biome().water && id.biome().passable));
                assert!(!site.opened);
                assert_eq!(site.contents(seed), site.contents(seed));
                assert!(site.contents(seed).iter().any(|i| matches!(i, Item::Gold(n) if *n > 0)));
                kinds.insert(site.kind.name());
                count += 1;
            }
        }
        assert!(count > 100);
        assert_eq!(kinds.len(), 3);
    }

    #[test]
    fn landmark_discovery_requires_sight_and_logs_only_once() {
        let mut game = made(7);
        let site = site(&game);
        assert!(game.landmark_at(site.pos).is_none(), "generation alone reveals nothing");
        game.player_mut().pos = (site.pos.0 + 20, site.pos.1);
        game.round(Some(Command::Wait));
        assert!(game.landmark_at(site.pos).is_none());
        game.player_mut().pos = (site.pos.0 - 1, site.pos.1);
        game.round(Some(Command::Wait));
        assert!(game.sees(site.pos) && game.remembers(site.pos));
        assert_eq!(game.landmark_at(site.pos), Some(&site));
        let discoveries = game.log().iter().filter(|l| l.text.starts_with("You discover")).count();
        game.instant(Instant::Look);
        assert!(game.log().iter().any(|l| l.text.contains("unopened cache")));
        game.round(Some(Command::Move(vec![Dir::E])));
        assert!(game.log().iter().any(|l| l.text.contains("Use get all")));
        game.round(Some(Command::Wait));
        assert_eq!(game.log().iter().filter(|l| l.text.starts_with("You discover")).count(), discoveries);
        let at = game.level().towns().next().unwrap();
        game.player_mut().at = LevelId::Deep(at, 1);
        assert!(game.landmark_at(site.pos).is_none(), "surface sites never leak into the Deep");
    }

    #[test]
    fn landmark_claim_is_one_time_and_does_not_spend_combat_dice() {
        let mut game = made(7);
        let site = site(&game);
        visit(&mut game, site);
        game.player_mut().sack.clear();
        let before = game.player().clone();
        let rng = serde_json::to_value(&game).unwrap()["rng"].clone();
        game.round(Some(Command::Get("all".into())));
        assert!(game.landmark_at(site.pos).unwrap().opened);
        assert!(game.pile_at(site.pos).is_none());
        let gold: u32 =
            site.contents(7).iter().filter_map(|i| if let Item::Gold(n) = i { Some(n) } else { None }).sum();
        assert_eq!(game.player().gold, before.gold + gold);
        assert_eq!(serde_json::to_value(&game).unwrap()["rng"], rng);
        let claimed = game.player().clone();
        game.round(Some(Command::Get("all".into())));
        assert_eq!(game.player(), &claimed);
        let saved = serde_json::to_string(&game).unwrap();
        let mut resumed: Game = serde_json::from_str(&saved).unwrap();
        assert_eq!(resumed, game);
        resumed.round(Some(Command::Get("all".into())));
        assert_eq!(resumed.player(), &claimed);
        assert!(resumed.pile_at(site.pos).is_none());
    }

    #[test]
    fn landmark_is_discovered_and_described_during_a_multi_step_round() {
        let mut game = made(7);
        let site = site(&game);
        game.player_mut().pos = (site.pos.0 - 1, site.pos.1);
        assert!(game.landmark_at(site.pos).is_none());
        game.round(Some(Command::Move(vec![Dir::E, Dir::E, Dir::E])));
        assert!(game.landmark_at(site.pos).is_some());
        assert!(game.remembers(site.pos));
        assert!(game.log().iter().any(|l| l.text.contains("Use get all")));
    }

    #[test]
    fn landmark_full_sack_leaves_real_contents_and_reload_never_replenishes_them() {
        let mut game = made(7);
        let site = site(&game);
        visit(&mut game, site);
        game.player_mut().sack = vec![Item::Potion; SACK];
        game.round(Some(Command::Get("all".into())));
        assert!(game.landmark_at(site.pos).unwrap().opened);
        let left = game.pile_at(site.pos).unwrap().items.clone();
        assert!(!left.is_empty() && left.iter().all(|i| !matches!(i, Item::Gold(_) | Item::Arrows(_))));
        let mut resumed: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        resumed.clear_creatures();
        resumed.player_mut().pos = (site.pos.0 - 1, site.pos.1);
        resumed.round(Some(Command::Move(vec![Dir::E])));
        resumed.round(Some(Command::Get("all".into())));
        assert_eq!(resumed.pile_at(site.pos).unwrap().items, left);
        resumed.player_mut().sack.clear();
        resumed.round(Some(Command::Get("all".into())));
        assert_eq!(resumed.player().sack, left);
        assert!(resumed.pile_at(site.pos).is_none());
        resumed.round(Some(Command::Drop("potion".into())));
        resumed.round(Some(Command::Get("all".into())));
        assert_eq!(resumed.player().sack.len(), left.len(), "dropped items do not reopen the cache");
    }

    #[test]
    fn landmark_unopened_reload_and_older_saves_preserve_discovery_rules() {
        let mut game = made(7);
        let site = site(&game);
        visit(&mut game, site);
        let value = serde_json::to_value(&game).unwrap();
        let mut resumed: Game = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(resumed.landmark_at(site.pos), Some(&site));
        game.round(Some(Command::Get("all".into())));
        resumed.round(Some(Command::Get("all".into())));
        assert_eq!(game, resumed);
        let mut older = value;
        older.as_object_mut().unwrap().remove("landmarks");
        let mut older: Game = serde_json::from_value(older).unwrap();
        assert!(older.landmarks().is_empty());
        older.round(Some(Command::Wait));
        assert_eq!(older.landmark_at(site.pos), Some(&site));
    }

    #[test]
    fn landmark_stays_open_after_death_and_return() {
        let mut game = made(7);
        let site = site(&game);
        visit(&mut game, site);
        game.round(Some(Command::Get("all".into())));
        game.player_mut().hp = 1;
        game.place(crate::Kind::Troll, (site.pos.0 + 1, site.pos.1));
        for _ in 0..100 {
            game.round(Some(Command::Rest));
            if game.player().deaths > 0 {
                break;
            }
        }
        assert_eq!(game.player().deaths, 1);
        assert!(game.level().safe(game.player().pos));
        game.clear_creatures();
        game.player_mut().pos = site.pos;
        game.round(Some(Command::Get("all".into())));
        assert!(game.landmark_at(site.pos).unwrap().opened);
        assert_eq!(game.log().iter().filter(|l| l.text.starts_with("You open the cache")).count(), 1);
        assert!(game.pile_at(site.pos).is_none(), "only the gold dropped on death could be recovered");
    }
}

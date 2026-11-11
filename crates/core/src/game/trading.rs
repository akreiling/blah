//! Cache weapons can be exchanged at home without spending the permanent starting kit.

use super::*;

impl Game {
    /// Exchange an owned cache weapon at home for one salve and one Focus draught.
    /// Full packs, nearby creatures, and equipped weapons block the trade without spending anything.
    pub fn trade_weapon(&mut self, item: WeaponItem) -> Step {
        if self.position != self.home || !self.can_prepare() {
            self.log.push(LogKind::Blocked, "Trade at home, away from nearby creatures.");
            return Step::Blocked;
        }
        if !WeaponItem::CACHE_REWARDS.contains(&item) || !self.inventory.contains(&item) {
            self.log.push(LogKind::Blocked, "Only an owned cache weapon can be traded.");
            return Step::Blocked;
        }
        if self.equipped == item {
            self.log.push(LogKind::Blocked, "Equip another weapon before trading this one.");
            return Step::Blocked;
        }
        if self.salves >= SALVE_CAPACITY && self.draughts >= DRAUGHT_CAPACITY {
            self.log.push(LogKind::Blocked, "Your supplies are full; the weapon is worth keeping.");
            return Step::Blocked;
        }
        self.inventory.retain(|&held| held != item);
        self.salves = (self.salves + 1).min(SALVE_CAPACITY);
        self.draughts = (self.draughts + 1).min(DRAUGHT_CAPACITY);
        self.turn += 1;
        self.sneaking = false;
        self.log.push(
            LogKind::Attack,
            format!("You trade {} for a salve and a Focus draught, as room allows.", item.name()),
        );
        self.finish_turn(false);
        Step::Waited
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trading_requires_home_room_an_unequipped_reward_and_a_clear_view() {
        let mut threatened = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
        threatened.inventory.push(WeaponItem::HeavyClub);
        assert_eq!(threatened.trade_weapon(WeaponItem::HeavyClub), Step::Blocked);
        assert!(threatened.inventory.contains(&WeaponItem::HeavyClub));
        assert_eq!(threatened.turn, 0);
        let mut game = Game::new(42);
        game.monsters.clear();
        assert_eq!(game.trade_weapon(WeaponItem::Dagger), Step::Blocked);
        assert_eq!(game.trade_weapon(WeaponItem::HeavyClub), Step::Blocked);
        game.inventory.push(WeaponItem::HeavyClub);
        game.draughts = DRAUGHT_CAPACITY;
        assert_eq!(game.trade_weapon(WeaponItem::HeavyClub), Step::Blocked);
        game.salves = 0;
        game.equipped = WeaponItem::HeavyClub;
        assert_eq!(game.trade_weapon(WeaponItem::HeavyClub), Step::Blocked);
        game.equipped = WeaponItem::Dagger;
        game.position.0 += 1;
        assert_eq!(game.trade_weapon(WeaponItem::HeavyClub), Step::Blocked);
        assert_eq!(game.turn, 0);
        game.position = game.home;
        assert_eq!(game.trade_weapon(WeaponItem::HeavyClub), Step::Waited);
        assert_eq!(game.turn, 1);
        assert_eq!(game.salves, 1);
        assert_eq!(game.draughts, DRAUGHT_CAPACITY);
        assert!(!game.inventory.contains(&WeaponItem::HeavyClub));
        assert_eq!(Game::from_snapshot(game.snapshot()).unwrap(), game);
        assert_eq!(game.trade_weapon(WeaponItem::HeavyClub), Step::Blocked);
    }

    #[test]
    fn trading_a_claimed_reward_survives_reload_without_reopening_the_cache() {
        let mut game = Game::new(42);
        let region = game.region.clone();
        game.position = Game::cache_site(&game.world, &mut game.regions, &region, game.home).unwrap();
        game.update_region(false);
        game.claim_cache();
        let item = game.cache_at(game.position.0, game.position.1).unwrap().treasure;
        game.position = game.home;
        game.update_region(false);
        game.monsters.clear();
        assert_eq!(game.trade_weapon(item), Step::Waited);
        let mut loaded = Game::from_snapshot(game.snapshot()).unwrap();
        assert_eq!(loaded, game);
        let site = loaded.caches[&region.id].position;
        loaded.position = site;
        assert_eq!(loaded.claim_cache(), Step::Blocked);
        assert!(!loaded.inventory.contains(&item));
    }
}

//! Cache weapons can be exchanged at home without spending the permanent starting kit.

use super::*;

impl Game {
    /// Exchange an owned cache weapon at home for one salve and one Focus draught.
    /// Full packs and equipped weapons block the trade without spending anything. Home shelters the turn
    /// even when creatures followed the wanderer back; it does not remove or harm those creatures.
    pub fn trade_weapon(&mut self, item: WeaponItem) -> Step {
        if self.position != self.home {
            self.log.push(LogKind::Blocked, "Trade in the shelter at home.");
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
        self.finish_turn(true);
        Step::Waited
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_management_is_sheltered_but_keeps_followers_and_exposed_actions_dangerous() {
        let mut game = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
        game.inventory.push(WeaponItem::HeavyClub);
        game.wanderer.hp = 1;
        game.wanderer.focus = 0;
        let follower = game.monsters[0].id;
        game.monsters[0].phenotype.stats.attack = 100;
        game.monsters[0].phenotype.stats.speed = 2 * ACTION_COST;
        game.monsters[0].energy = ACTION_COST;
        assert!(game.can_prepare());
        let before = game.turn;
        game.prepare(Maneuver::default()).unwrap();
        assert_eq!(game.turn, before);
        assert_eq!(game.trade_weapon(WeaponItem::HeavyClub), Step::Waited);
        assert_eq!(game.wanderer.hp, 1, "the trade turn is sheltered");
        assert_eq!(game.deaths, 0);
        assert!(game.monsters.iter().any(|m| m.id == follower), "home does not delete the encounter");
        assert_eq!(game.draughts, 1);
        assert!(game.can_prepare(), "the same nearby creature must not block fitting the next loadout");
        game.wait();
        assert_eq!(game.deaths, 1, "ordinary waiting is still exposed");
    }

    #[test]
    fn trading_requires_home_room_and_an_unequipped_reward() {
        let mut threatened = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
        threatened.inventory.push(WeaponItem::HeavyClub);
        threatened.position.1 += 1;
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

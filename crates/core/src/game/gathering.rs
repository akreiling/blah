//! Herbs gathered in the field and brewed into Focus draughts.
//!
//! Each land region has a few herb patches whose sites follow from the seed and the region alone, like its
//! cache. Minority-biome slivers get extra sites when they grow an otherwise missing herb.
//! Which herb grows depends on the patch's biome, so a draught, which needs two different herbs, asks
//! for a journey across biomes. A picked patch regrows after [`HERB_REGROW_TURNS`]; only the turns patches
//! were picked are state, and a record is dropped once its patch has regrown.

use super::*;

/// One kind of gathered herb. The order is the pouch layout and the brewing tie-break.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Herb {
    /// Wet and wooded ground: swamp, jungle, forest, taiga.
    Mossbloom,
    /// Dry and open ground: grassland, savanna, desert, beach.
    Sunthistle,
    /// High and cold ground: hills, mountains, badlands, tundra.
    Stonecap,
}

impl Herb {
    /// Every herb, in pouch order.
    pub const ALL: [Self; 3] = [Self::Mossbloom, Self::Sunthistle, Self::Stonecap];

    /// The herb's name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Mossbloom => "Mossbloom",
            Self::Sunthistle => "Sunthistle",
            Self::Stonecap => "Stonecap",
        }
    }

    /// The herb that grows on `biome`, if any does. Water and snow peaks grow none.
    #[must_use]
    pub fn of(biome: &Biome) -> Option<Self> {
        match biome.name {
            "Swamp" | "Jungle" | "Forest" | "Taiga" => Some(Self::Mossbloom),
            "Grassland" | "Savanna" | "Desert" | "Beach" => Some(Self::Sunthistle),
            "Hills" | "Mountains" | "Badlands" | "Tundra" => Some(Self::Stonecap),
            _ => None,
        }
    }

    const fn index(self) -> usize {
        self as usize
    }
}

/// Most of one herb the pouch holds.
pub const HERB_CAPACITY: u8 = 3;
/// Focus draughts the character can carry.
pub const DRAUGHT_CAPACITY: u8 = 2;
/// Focus one draught restores.
pub const DRAUGHT_FOCUS: i32 = 8;
/// Turns a picked patch takes to regrow.
pub const HERB_REGROW_TURNS: u64 = 300;
/// Most original patches one region grows, before adding minority-biome sites.
const MAX_PATCHES: usize = 4;

/// An herb patch on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HerbPatch {
    /// Where it grows.
    pub position: (i64, i64),
    /// What grows there.
    pub herb: Herb,
}

/// An herb patch as the player sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HerbInfo {
    /// What grows there.
    pub herb: Herb,
    /// Whether it can be gathered now, rather than regrowing.
    pub ready: bool,
}

impl Game {
    /// The herb patch at `(x, y)` in a region in view.
    #[must_use]
    pub fn herb_at(&self, x: i64, y: i64) -> Option<HerbInfo> {
        let patch = self.herb_sites.values().flatten().find(|patch| patch.position == (x, y))?;
        Some(HerbInfo { herb: patch.herb, ready: self.patch_ready(patch.position) })
    }

    /// How many of `herb` are in the pouch.
    #[must_use]
    pub const fn herbs(&self, herb: Herb) -> u8 {
        self.herbs[herb.index()]
    }

    /// Focus draughts carried.
    #[must_use]
    pub const fn draughts(&self) -> u8 {
        self.draughts
    }

    /// Gather the ready patch underfoot during one turn. Skilled herbalists keep their sneaking stance.
    pub fn gather(&mut self) -> Step {
        self.update_visible_herbs();
        let Some(info) = self.herb_at(self.position.0, self.position.1) else {
            self.log.push(LogKind::Blocked, "There is no herb patch underfoot.");
            return Step::Blocked;
        };
        if !info.ready {
            self.log.push(LogKind::Blocked, format!("This {} patch is still regrowing.", info.herb.name()));
            return Step::Blocked;
        }
        if self.herbs(info.herb) >= HERB_CAPACITY {
            self.log.push(LogKind::Blocked, format!("Your pouch has no room for more {}.", info.herb.name()));
            return Step::Blocked;
        }
        self.harvest_herb(info.herb);
        self.picked.insert(self.position, self.turn);
        self.turn += 1;
        self.sneaking &= self.wanderer.herbalism.quiet_gathering();
        self.practice_herbalism(100);
        self.log.push(
            LogKind::Attack,
            format!("You gather {}. You carry {}/{HERB_CAPACITY}.", info.herb.name(), self.herbs(info.herb)),
        );
        self.finish_turn(false);
        Step::Waited
    }

    /// Drink a Focus draught during one exposed turn, including while creatures are nearby.
    pub fn drink_draught(&mut self) -> Step {
        if self.draughts == 0 {
            self.log.push(LogKind::Blocked, "No draughts remain. Brew one at camp from two different herbs.");
            return Step::Blocked;
        }
        if self.wanderer.focus >= self.wanderer.max_focus() {
            self.log.push(LogKind::Blocked, "Your Focus is already full.");
            return Step::Blocked;
        }
        self.draughts -= 1;
        self.turn += 1;
        self.sneaking = false;
        self.wanderer.focus =
            (self.wanderer.focus + self.wanderer.herbalism.draught_focus()).min(self.wanderer.max_focus());
        self.log.push(
            LogKind::Attack,
            format!("You drink a Focus draught. Focus {}/{}.", self.wanderer.focus, self.wanderer.max_focus()),
        );
        self.finish_turn(false);
        Step::Waited
    }

    /// The two different herbs a draught would use: the two most plentiful, ties broken by [`Herb::ALL`]
    /// order. `None` without room for a draught or without two different herbs.
    pub(super) fn brew_pair(&self) -> Option<(Herb, Herb)> {
        if self.draughts >= DRAUGHT_CAPACITY {
            return None;
        }
        let mut held: Vec<_> = Herb::ALL.into_iter().filter(|&herb| self.herbs(herb) > 0).collect();
        held.sort_by_key(|&herb| std::cmp::Reverse(self.herbs(herb)));
        match held[..] {
            [first, second, ..] => Some((first, second)),
            _ => None,
        }
    }

    /// Brew one draught if the pouch allows, returning the log text.
    pub(super) fn brew(&mut self) -> Option<String> {
        let (first, second) = self.brew_pair()?;
        self.herbs[first.index()] -= 1;
        self.herbs[second.index()] -= 1;
        self.draughts += 1;
        self.practice_herbalism(200);
        Some(format!(
            " You brew a Focus draught from {} and {} ({}/{DRAUGHT_CAPACITY}).",
            first.name(),
            second.name(),
            self.draughts
        ))
    }

    pub(super) fn patch_ready(&self, position: (i64, i64)) -> bool {
        self.picked
            .get(&position)
            .is_none_or(|&picked| self.turn.saturating_sub(picked) >= self.wanderer.herbalism.herb_regrow_turns())
    }

    /// Picks whose patches have not regrown: the only gathering state that matters.
    pub(super) fn active_picks(&self) -> impl Iterator<Item = ((i64, i64), u64)> + '_ {
        self.picked
            .iter()
            .filter(|&(_, &picked)| self.turn.saturating_sub(picked) < self.wanderer.herbalism.herb_regrow_turns())
            .map(|(&position, &picked)| (position, picked))
    }

    /// Know the patches of every region in view, and forget picks that have regrown.
    pub(super) fn update_visible_herbs(&mut self) {
        let turn = self.turn;
        let regrowth = self.wanderer.herbalism.herb_regrow_turns();
        self.picked.retain(|_, picked| turn.saturating_sub(*picked) < regrowth);
        if !self.journey {
            self.herb_sites.clear();
            return;
        }
        let visible = self.visible_land_regions();
        self.herb_sites.retain(|id, _| visible.contains(id));
        for id in visible {
            if self.herb_sites.contains_key(&id) {
                continue;
            }
            let Some(region) = self.regions.region(id) else {
                continue;
            };
            let patches = Self::herb_patches(&self.world, &mut self.regions, &region, self.home);
            self.herb_sites.insert(id, patches);
        }
    }

    /// A region's patches, chosen only from the world seed, region identity, and home: two to four original
    /// sites, plus up to two minority-herb sites, never home or the region's cache site.
    pub(super) fn herb_patches(
        world: &World,
        regions: &mut RegionCache,
        region: &Region,
        home: (i64, i64),
    ) -> Vec<HerbPatch> {
        if !is_land(region.biome.biome()) {
            return Vec::new();
        }
        let (ax, ay) = region.id.anchor();
        let mut rng = Rng::keyed(world.seed(), b"herbsite", &[ax as u64, ay as u64]);
        let wanted = 2 + rng.below(MAX_PATCHES - 1);
        let cache = Self::cache_site(world, regions, region, home);
        let bounds = region.bounds;
        let width = (bounds.max_x - bounds.min_x + 1) as usize;
        let height = (bounds.max_y - bounds.min_y + 1) as usize;
        let mut patches: Vec<HerbPatch> = Vec::new();
        for _ in 0..64 {
            if patches.len() == wanted {
                break;
            }
            let at = (bounds.min_x + rng.below(width) as i64, bounds.min_y + rng.below(height) as i64);
            if at == home || Some(at) == cache || patches.iter().any(|patch| patch.position == at) {
                continue;
            }
            if regions.region_at(at.0, at.1) != region.id {
                continue;
            }
            if let Some(herb) = Herb::of(world.tile(at.0, at.1).biome.biome()) {
                patches.push(HerbPatch { position: at, herb });
            }
        }
        // Thin regions can evade the random probes. Walk the bounds from a seeded offset so every
        // eligible region still gets its quota, or every free tile if it is smaller than the quota.
        if patches.len() < wanted {
            let area = width * height;
            let start = rng.below(area);
            for offset in 0..area {
                let index = (start + offset) % area;
                let at = (bounds.min_x + (index % width) as i64, bounds.min_y + (index / width) as i64);
                if at == home || Some(at) == cache || patches.iter().any(|patch| patch.position == at) {
                    continue;
                }
                if regions.region_at(at.0, at.1) != region.id {
                    continue;
                }
                if let Some(herb) = Herb::of(world.tile(at.0, at.1).biome.biome()) {
                    patches.push(HerbPatch { position: at, herb });
                    if patches.len() == wanted {
                        break;
                    }
                }
            }
        }
        // Preserve the original sites for old saves, then add one site for each otherwise missing herb
        // that actually grows in a sliver of this region.
        let area = width * height;
        let start = rng.below(area);
        for offset in 0..area {
            if Herb::ALL.iter().all(|herb| patches.iter().any(|patch| patch.herb == *herb)) {
                break;
            }
            let index = (start + offset) % area;
            let at = (bounds.min_x + (index % width) as i64, bounds.min_y + (index / width) as i64);
            if at == home || Some(at) == cache {
                continue;
            }
            let Some(herb) = Herb::of(world.tile(at.0, at.1).biome.biome()) else {
                continue;
            };
            if patches.iter().any(|patch| patch.herb == herb) || regions.region_at(at.0, at.1) != region.id {
                continue;
            }
            patches.push(HerbPatch { position: at, herb });
        }
        patches
    }

    pub(super) fn harvest_herb(&mut self, herb: Herb) -> u8 {
        let mut rng = Rng::keyed(
            self.world.seed(),
            b"herbpick",
            &[self.turn, self.position.0 as u64, self.position.1 as u64, herb as u64],
        );
        let amount = if rng.below(100) < self.wanderer.herbalism.second_herb_chance() { 2 } else { 1 };
        let before = self.herbs[herb.index()];
        self.herbs[herb.index()] = (before + amount).min(HERB_CAPACITY);
        self.herbs[herb.index()] - before
    }

    pub(super) fn practice_herbalism(&mut self, evidence: u32) {
        if self.wanderer.herbalism.learn(evidence, Rank::Expert) {
            self.log
                .push(LogKind::Learning, format!("Your herbalism is now {}.", self.wanderer.herbalism.rank().name()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starting_patch(game: &mut Game) -> HerbPatch {
        let region = game.region.clone();
        Game::herb_patches(&game.world, &mut game.regions, &region, game.home)[0]
    }

    #[test]
    fn every_land_biome_has_one_herb_and_water_and_peaks_have_none() {
        for biome in crate::biome::BIOMES {
            assert_eq!(Herb::of(biome).is_some(), is_land(biome), "{}", biome.name);
        }
        assert_eq!(Herb::of(crate::biome::BIOMES.iter().find(|b| b.name == "Forest").unwrap()), Some(Herb::Mossbloom));
        assert_eq!(Herb::of(crate::biome::BIOMES.iter().find(|b| b.name == "Desert").unwrap()), Some(Herb::Sunthistle));
        assert_eq!(
            Herb::of(crate::biome::BIOMES.iter().find(|b| b.name == "Mountains").unwrap()),
            Some(Herb::Stonecap)
        );
    }

    #[test]
    fn patches_are_deterministic_biome_correct_and_separate_from_home_and_caches() {
        let mut kinds = BTreeSet::new();
        for seed in 0..64 {
            let mut game = Game::new(seed);
            let mut ids = game.regions.region_neighbors(game.region.id);
            ids.push(game.region.id);
            for id in ids {
                let region = game.regions.region(id).unwrap();
                let patches = Game::herb_patches(&game.world, &mut game.regions, &region, game.home);
                let mut fresh = RegionCache::new(game.world);
                assert_eq!(patches, Game::herb_patches(&game.world, &mut fresh, &region, game.home));
                if !is_land(region.biome.biome()) {
                    assert!(patches.is_empty());
                    continue;
                }
                let cache = Game::cache_site(&game.world, &mut game.regions, &region, game.home);
                // Tiny pockets may have fewer than two unoccupied tiles.
                if region.tiles >= 4 {
                    assert!((2..=6).contains(&patches.len()), "seed {seed} {id:?}: {patches:?}");
                }
                assert_eq!(patches.iter().map(|p| p.position).collect::<BTreeSet<_>>().len(), patches.len());
                for patch in patches {
                    assert_ne!(patch.position, game.home);
                    assert_ne!(Some(patch.position), cache);
                    assert_eq!(game.regions.region_at(patch.position.0, patch.position.1), id);
                    assert_eq!(
                        Herb::of(game.world.tile(patch.position.0, patch.position.1).biome.biome()),
                        Some(patch.herb)
                    );
                    kinds.insert(patch.herb);
                }
            }
        }
        assert_eq!(kinds.len(), 3);
    }

    #[test]
    fn every_available_minority_herb_gets_a_site() {
        let mut diverse = 0;
        for seed in 0..12 {
            let mut game = Game::new(seed);
            let region = game.region.clone();
            let patches = Game::herb_patches(&game.world, &mut game.regions, &region, game.home);
            let represented: BTreeSet<_> = patches.iter().map(|p| p.herb).collect();
            let cache = Game::cache_site(&game.world, &mut game.regions, &region, game.home);
            let mut available = BTreeSet::new();
            for y in region.bounds.min_y..=region.bounds.max_y {
                for x in region.bounds.min_x..=region.bounds.max_x {
                    if (x, y) == game.home || Some((x, y)) == cache {
                        continue;
                    }
                    if let Some(herb) = Herb::of(game.world.tile(x, y).biome.biome())
                        && game.regions.region_at(x, y) == region.id
                    {
                        available.insert(herb);
                    }
                }
            }
            assert_eq!(represented, available, "seed {seed}");
            diverse += usize::from(represented.len() > 1);
        }
        assert!(diverse > 0, "the sample must exercise minority-biome herbs");
    }

    #[test]
    fn gathering_depletes_through_reload_and_reentry_and_regrows_at_300_turns() {
        let mut game = Game::new(42);
        let patch = starting_patch(&mut game);
        game.position = patch.position;
        game.update_region(false);
        let picked_at = game.turn;
        assert_eq!(game.gather(), Step::Waited);
        assert_eq!(game.turn, picked_at + 1);
        assert_eq!(game.herbs(patch.herb), 1);
        assert!(!game.herb_at(patch.position.0, patch.position.1).unwrap().ready);
        game = Game::from_snapshot(game.snapshot()).unwrap();
        let turn = game.turn;
        assert_eq!(game.gather(), Step::Blocked);
        assert_eq!(game.turn, turn);
        game.position = (patch.position.0 + 200, patch.position.1);
        game.update_visible_herbs();
        assert!(game.herb_at(patch.position.0, patch.position.1).is_none());
        game.position = patch.position;
        game.update_visible_herbs();
        assert!(!game.herb_at(patch.position.0, patch.position.1).unwrap().ready);
        // Exercise actual wait turns, ending each at sheltered home to avoid unrelated deaths.
        game.position = game.home;
        game.update_region(false);
        while game.turn < picked_at + HERB_REGROW_TURNS - 1 {
            game.monsters.clear();
            game.wait();
        }
        game.position = patch.position;
        game.update_region(false);
        game.update_visible_herbs();
        assert!(!game.herb_at(patch.position.0, patch.position.1).unwrap().ready);
        game.monsters.clear();
        game.wait();
        assert!(game.herb_at(patch.position.0, patch.position.1).unwrap().ready);
        assert!(game.active_picks().next().is_none());
        assert_eq!(game.gather(), Step::Waited);
        assert_eq!(game.herbs(patch.herb), 2);
    }

    #[test]
    fn death_keeps_supplies_but_its_elapsed_time_regrows_patches() {
        let mut game = Game::new(42);
        let patch = starting_patch(&mut game);
        game.position = patch.position;
        game.update_region(false);
        game.gather();
        game.draughts = 1;
        game.die();
        assert_eq!(game.herbs(patch.herb), 1);
        assert_eq!(game.draughts(), 1);
        assert!(game.picked.is_empty());
        game = Game::from_snapshot(game.snapshot()).unwrap();
        game.position = patch.position;
        game.update_region(false);
        game.update_visible_herbs();
        assert!(game.herb_at(patch.position.0, patch.position.1).unwrap().ready);
    }

    #[test]
    fn gather_blocks_without_a_patch_or_pouch_room_without_spending_time() {
        let mut game = Game::new(42);
        assert_eq!(game.gather(), Step::Blocked);
        assert_eq!(game.turn, 0);
        let patch = starting_patch(&mut game);
        game.position = patch.position;
        game.herbs[patch.herb.index()] = HERB_CAPACITY;
        assert_eq!(game.gather(), Step::Blocked);
        assert_eq!(game.turn, 0);
        assert!(game.patch_ready(patch.position));
    }

    #[test]
    fn camp_brews_one_draught_from_two_different_herbs_and_respects_capacity() {
        let mut game = Game::new(42);
        game.monsters.clear();
        game.herbs = [3, 0, 0];
        assert_eq!(game.camp(), Step::Blocked);
        assert_eq!(game.turn, 0);
        game.herbs = [3, 2, 1];
        assert_eq!(game.camp(), Step::Waited); // Brewing is allowed even at full Health and Focus.
        assert_eq!(game.herbs, [2, 1, 1]);
        assert_eq!(game.draughts(), 1);
        game.monsters.clear();
        assert_eq!(game.camp(), Step::Waited);
        assert_eq!(game.herbs, [1, 0, 1]);
        assert_eq!(game.draughts(), DRAUGHT_CAPACITY);
        game.monsters.clear();
        let turn = game.turn;
        assert_eq!(game.camp(), Step::Blocked);
        assert_eq!(game.turn, turn);
        assert_eq!(game.herbs, [1, 0, 1]);
        assert_eq!(Game::from_snapshot(game.snapshot()).unwrap(), game);
    }

    #[test]
    fn drinking_is_exposed_in_combat_and_clamps_focus_without_free_refills() {
        let mut game = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
        assert!(game.herb_sites.is_empty(), "combat demos have no gathering sites");
        game.herbs = [1, 1, 0];
        assert_eq!(game.camp(), Step::Blocked);
        assert_eq!(game.herbs, [1, 1, 0]);
        assert_eq!(game.drink_draught(), Step::Blocked);
        game.draughts = 2;
        assert_eq!(game.drink_draught(), Step::Blocked); // Full Focus wastes neither time nor draught.
        assert_eq!(game.draughts(), 2);
        assert_eq!(game.turn, 0);
        game.wanderer.focus = 0;
        game.monsters[0].alerted = true;
        game.monsters[0].energy = ACTION_COST;
        let before = game.log.total();
        assert_eq!(game.drink_draught(), Step::Waited);
        assert_eq!(game.wanderer.focus, DRAUGHT_FOCUS);
        assert_eq!(game.draughts(), 1);
        assert_eq!(game.turn, 1);
        assert!(!game.sneaking);
        assert!(
            game.log
                .entries()
                .skip((before.min(LOG_LIMIT as u64)) as usize)
                .any(|e| matches!(e.kind, LogKind::Hurt | LogKind::Missed))
        );
        game.monsters.clear();
        game.wanderer.focus = game.wanderer.max_focus() - 1;
        assert_eq!(game.drink_draught(), Step::Waited);
        assert_eq!(game.wanderer.focus, game.wanderer.max_focus());
        assert_eq!(game.draughts(), 0);
    }

    #[test]
    fn snapshots_reject_invalid_supply_counts_and_pick_records() {
        let game = Game::new(42);
        let value = serde_json::to_value(game.snapshot()).unwrap();
        for (field, invalid) in [
            ("herbs", serde_json::json!([HERB_CAPACITY + 1, 0, 0])),
            ("draughts", serde_json::json!(DRAUGHT_CAPACITY + 1)),
            ("picked", serde_json::json!([{"position": game.home, "turn": 0}])),
            ("picked", serde_json::json!([{"position": game.home, "turn": 1}])),
            ("picked", serde_json::json!([{"position": game.home, "turn": 0}, {"position": game.home, "turn": 0}])),
        ] {
            let mut state = value.clone();
            state[field] = invalid;
            assert!(Game::from_snapshot(serde_json::from_value(state).unwrap()).is_err(), "{field}");
        }
        let mut state = value;
        state["turn"] = HERB_REGROW_TURNS.into();
        state["picked"] = serde_json::json!([{"position": game.home, "turn": 0}]);
        assert!(Game::from_snapshot(serde_json::from_value(state).unwrap()).is_err());
    }

    #[test]
    fn herbalism_is_earned_only_by_success_and_survives_reload() {
        let mut game = Game::new(42);
        let before = game.wanderer.herbalism;
        assert_eq!(game.gather(), Step::Blocked);
        assert_eq!(game.wanderer.herbalism, before);
        let patch = starting_patch(&mut game);
        game.position = patch.position;
        game.update_region(false);
        game.monsters.clear();
        game.gather();
        assert_ne!(game.wanderer.herbalism, before);
        let learned = game.wanderer.herbalism;
        assert_eq!(game.gather(), Step::Blocked);
        assert_eq!(game.wanderer.herbalism, learned);
        game.position = game.home;
        game.update_region(false);
        game.monsters.clear();
        game.herbs = [1, 1, 0];
        assert_eq!(game.camp(), Step::Waited);
        assert_ne!(game.wanderer.herbalism, learned);
        assert_eq!(Game::from_snapshot(game.snapshot()).unwrap(), game);
    }

    #[test]
    fn skilled_gathering_keeps_sneaking_and_expert_yields_and_regrowth_are_bounded() {
        let mut game = Game::new(42);
        let patch = starting_patch(&mut game);
        game.position = patch.position;
        game.update_region(false);
        game.monsters.clear();
        game.sneaking = true;
        assert_eq!(game.gather(), Step::Waited);
        assert!(!game.sneaking);
        game.wanderer.herbalism.learn(4_000, Rank::Expert);
        game.picked.clear();
        game.sneaking = true;
        game.monsters.clear();
        assert_eq!(game.gather(), Step::Waited);
        assert!(game.sneaking);
        game.wanderer.herbalism.learn(30_000, Rank::Expert);
        let mut yields = BTreeSet::new();
        for _ in 0..40 {
            game.monsters.clear();
            game.picked.clear();
            game.herbs = [0; 3];
            assert_eq!(game.gather(), Step::Waited);
            yields.insert(game.herbs(patch.herb));
        }
        assert_eq!(yields, BTreeSet::from([1, 2]));
        game.picked.clear();
        game.herbs[patch.herb.index()] = HERB_CAPACITY - 1;
        game.monsters.clear();
        game.gather();
        assert_eq!(game.herbs(patch.herb), HERB_CAPACITY);
        let picked = game.picked[&patch.position];
        game.turn = picked + 149;
        assert!(!game.patch_ready(patch.position));
        game.turn += 1;
        assert!(game.patch_ready(patch.position));
        game.wanderer.focus = 0;
        game.draughts = 1;
        game.monsters.clear();
        game.drink_draught();
        assert_eq!(game.wanderer.focus, 12);
    }
}

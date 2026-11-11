//! Durable sites discovered on the journey and the actions they offer.

use super::*;
use crate::relics::{HEARTH_EMBER_HEAL, RETURNING_LANTERN_HEALTH};
use crate::{RELIC_SLOTS, Relic};

/// A fixed site generated in a land region.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LandmarkKind {
    /// A one-time source of a relic.
    Shrine,
    /// Restocks salves and Focus draughts after a cooldown.
    Spring,
    /// Reveals caches and landmarks in a wider area.
    Lookout,
    /// A renewable source of all three herbs.
    WildGarden,
}

impl LandmarkKind {
    /// Every landmark type.
    pub const ALL: [Self; 4] = [Self::Shrine, Self::Spring, Self::Lookout, Self::WildGarden];

    /// The landmark's display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shrine => "Shrine",
            Self::Spring => "Spring",
            Self::Lookout => "Lookout",
            Self::WildGarden => "Wild garden",
        }
    }

    /// A compact map marker, dimmed while unavailable or regrowing.
    #[must_use]
    pub const fn glyph(self, ready: bool) -> char {
        match (self, ready) {
            (Self::Shrine, true) => 'S',
            (Self::Shrine, false) => 's',
            (Self::Spring, true) => 'W',
            (Self::Spring, false) => 'w',
            (Self::Lookout, true) => 'O',
            (Self::Lookout, false) => 'o',
            (Self::WildGarden, true) => 'G',
            (Self::WildGarden, false) => 'g',
        }
    }
}

/// Turns between a spring's refills.
pub const SPRING_COOLDOWN_TURNS: u64 = 300;
/// Turns between wild-garden harvests.
pub const GARDEN_REGROW_TURNS: u64 = 300;
/// A lookout discovers sites up to this Chebyshev distance.
pub const LOOKOUT_RADIUS: i64 = 32;

/// A discovered landmark and its current visible status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LandmarkInfo {
    /// The landmark's tile.
    pub position: (i64, i64),
    /// Which action the site offers.
    pub kind: LandmarkKind,
    /// Whether it has ever been used.
    pub used: bool,
    /// Whether it can be used now.
    pub ready: bool,
    /// The shrine's reward, whether claimed or not.
    pub relic_reward: Option<Relic>,
}

/// Durable per-site state. The tile is derived from the world and is not stored in snapshots.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct LandmarkRecord {
    pub(super) region: RegionId,
    pub(super) kind: LandmarkKind,
    #[serde(skip)]
    pub(super) position: (i64, i64),
    pub(super) used: bool,
    pub(super) last_used: Option<u64>,
    pub(super) relic_reward: Option<Relic>,
}

impl LandmarkRecord {
    fn new(region: RegionId, kind: LandmarkKind, position: (i64, i64)) -> Self {
        Self { region, kind, position, used: false, last_used: None, relic_reward: None }
    }
}

impl Game {
    /// A landmark at `(x, y)`, if it has been seen or marked by a lookout.
    #[must_use]
    pub fn landmark_at(&self, x: i64, y: i64) -> Option<LandmarkInfo> {
        self.landmarks.values().find(|record| record.position == (x, y)).map(|record| self.landmark_info(record))
    }

    /// Every known landmark, including sites currently beyond the map.
    pub fn known_landmarks(&self) -> impl Iterator<Item = LandmarkInfo> + '_ {
        self.landmarks.values().map(|record| self.landmark_info(record))
    }

    fn landmark_info(&self, record: &LandmarkRecord) -> LandmarkInfo {
        let ready = match record.kind {
            LandmarkKind::Shrine => !record.used && self.relic_reward_for(record.region).is_some(),
            LandmarkKind::Lookout => !record.used,
            LandmarkKind::Spring => {
                record.last_used.is_none_or(|used| self.turn.saturating_sub(used) >= self.spring_cooldown())
            }
            LandmarkKind::WildGarden => record
                .last_used
                .is_none_or(|used| self.turn.saturating_sub(used) >= self.wanderer.herbalism.herb_regrow_turns()),
        };
        LandmarkInfo {
            position: record.position,
            kind: record.kind,
            used: record.used,
            ready,
            relic_reward: if record.kind == LandmarkKind::Shrine && !record.used {
                self.relic_reward_for(record.region)
            } else {
                record.relic_reward
            },
        }
    }

    /// The relics this character owns, in acquisition order.
    #[must_use]
    pub fn relics(&self) -> &[Relic] {
        &self.relics
    }

    /// The two currently attuned relic slots.
    #[must_use]
    pub const fn attuned_relics(&self) -> [Option<Relic>; RELIC_SLOTS] {
        self.attuned_relics
    }

    /// Whether `relic` is active in either slot.
    #[must_use]
    pub fn has_relic(&self, relic: Relic) -> bool {
        self.attuned_relics.contains(&Some(relic))
    }

    /// Choose both attunement slots without passing time. Preparation is allowed only at home with no
    /// creature in view; each selected relic must be owned and may appear in only one slot.
    pub fn attune_relics(&mut self, slots: [Option<Relic>; RELIC_SLOTS]) -> Result<(), &'static str> {
        if self.position != self.home || !self.can_prepare() {
            return Err("Relics can only be attuned safely at home.");
        }
        let selected: Vec<_> = slots.iter().flatten().copied().collect();
        if selected.iter().any(|relic| !self.relics.contains(relic))
            || selected.iter().collect::<HashSet<_>>().len() != selected.len()
        {
            return Err("Attune distinct relics that you own.");
        }
        self.attuned_relics = slots;
        Ok(())
    }

    /// Cycle the active pair through owned relics at home without passing time.
    pub fn cycle_relics(&mut self) -> Result<(), &'static str> {
        if self.position != self.home || !self.can_prepare() {
            return Err("Relics can only be attuned safely at home.");
        }
        let available: Vec<_> = Relic::ALL.into_iter().filter(|relic| self.relics.contains(relic)).collect();
        let mut pairs = Vec::new();
        for first in 0..available.len() {
            for second in first + 1..available.len() {
                pairs.push([available[first], available[second]]);
            }
        }
        if pairs.len() <= 1 {
            return Err("You have no alternate relic pairing.");
        }
        let current = self.attuned_relics;
        let index = pairs
            .iter()
            .position(|pair| current == [Some(pair[0]), Some(pair[1])] || current == [Some(pair[1]), Some(pair[0])]);
        let next = index.map_or(0, |index| (index + 1) % pairs.len());
        self.attuned_relics = [Some(pairs[next][0]), Some(pairs[next][1])];
        Ok(())
    }

    /// Use the landmark underfoot during one exposed turn.
    pub fn use_landmark(&mut self) -> Step {
        self.update_visible_landmarks();
        let Some(record) = self.landmarks.values().find(|record| record.position == self.position) else {
            self.log.push(LogKind::Blocked, "There is no landmark underfoot.");
            return Step::Blocked;
        };
        let region = record.region;
        let kind = record.kind;
        let info = self.landmark_info(record);
        if !info.ready {
            let message = match kind {
                LandmarkKind::Shrine if self.relics.len() == Relic::ALL.len() => {
                    "You already carry every relic this shrine can offer."
                }
                LandmarkKind::Shrine => "This shrine has already given its relic.",
                LandmarkKind::Spring => "This spring is still recovering.",
                LandmarkKind::Lookout => "You have already surveyed from this lookout.",
                LandmarkKind::WildGarden => "The wild garden is regrowing.",
            };
            self.log.push(LogKind::Blocked, message);
            return Step::Blocked;
        }

        let quiet = kind == LandmarkKind::WildGarden && self.wanderer.herbalism.quiet_gathering();
        let line = match kind {
            LandmarkKind::Shrine => {
                let reward = self.relic_reward_for(region);
                let new_relic = reward.is_some_and(|relic| self.grant_relic(relic));
                let record = self.landmarks.get_mut(&region).expect("landmark is in the map");
                record.used = true;
                record.relic_reward = reward;
                match (reward, new_relic) {
                    (Some(relic), true) => format!("The shrine gives you the {}.", relic.name()),
                    (Some(relic), false) => format!("The shrine offers the {} you already own.", relic.name()),
                    (None, _) => "The shrine is quiet; you already carry every relic.".to_owned(),
                }
            }
            LandmarkKind::Spring => {
                if self.salves >= SALVE_CAPACITY && self.draughts >= DRAUGHT_CAPACITY {
                    self.log.push(LogKind::Blocked, "Your salves and draughts are already full.");
                    return Step::Blocked;
                }
                let salves = SALVE_CAPACITY - self.salves;
                let draughts = DRAUGHT_CAPACITY - self.draughts;
                self.salves = SALVE_CAPACITY;
                self.draughts = DRAUGHT_CAPACITY;
                let record = self.landmarks.get_mut(&region).expect("landmark is in the map");
                record.used = true;
                record.last_used = Some(self.turn);
                format!("The spring restores {salves} salves and {draughts} Focus draughts.")
            }
            LandmarkKind::Lookout => {
                let (caches, landmarks) = self.survey_nearby();
                self.landmarks.get_mut(&region).expect("landmark is in the map").used = true;
                format!("From the lookout you mark {caches} caches and {landmarks} landmarks nearby.")
            }
            LandmarkKind::WildGarden => {
                if self.herbs.iter().all(|&count| count >= HERB_CAPACITY) {
                    self.log.push(LogKind::Blocked, "Your herb pouch is already full.");
                    return Step::Blocked;
                }
                let mut found = [0; 3];
                for herb in Herb::ALL {
                    let index = herb as usize;
                    found[index] = self.harvest_herb(herb);
                }
                self.practice_herbalism(100 * found.iter().filter(|&&count| count > 0).count() as u32);
                let record = self.landmarks.get_mut(&region).expect("landmark is in the map");
                record.used = true;
                record.last_used = Some(self.turn);
                format!(
                    "You gather from the wild garden: Mossbloom {}, Sunthistle {}, Stonecap {}.",
                    found[0], found[1], found[2]
                )
            }
        };
        self.turn += 1;
        self.sneaking &= quiet;
        self.log.push(LogKind::Attack, line);
        self.finish_turn(false);
        Step::Waited
    }

    /// Add landmarks in ordinary view. Site generation uses its own keyed stream and never touches a
    /// simulation stream.
    pub(super) fn update_visible_landmarks(&mut self) {
        if !self.journey {
            return;
        }
        let visible = self.visible_land_regions();
        for id in visible {
            if self.landmarks.contains_key(&id) {
                continue;
            }
            let Some(region) = self.regions.region(id) else {
                continue;
            };
            let Some((kind, position)) = Self::landmark_site(&self.world, &mut self.regions, &region, self.home) else {
                continue;
            };
            if chebyshev(position, self.position) <= VIEW_RADIUS {
                self.landmarks.insert(id, LandmarkRecord::new(id, kind, position));
            }
        }
    }

    /// The deterministic landmark, if a region has one.
    pub(super) fn landmark_site(
        world: &World,
        regions: &mut RegionCache,
        region: &Region,
        home: (i64, i64),
    ) -> Option<(LandmarkKind, (i64, i64))> {
        if !is_land(region.biome.biome()) {
            return None;
        }
        let (ax, ay) = region.id.anchor();
        let mut rng = Rng::keyed(world.seed(), b"landmark", &[ax as u64, ay as u64]);
        if rng.below(4) != 0 {
            return None;
        }
        let kind = LandmarkKind::ALL[rng.below(LandmarkKind::ALL.len())];
        let cache = Game::cache_site(world, regions, region, home);
        let bounds = region.bounds;
        let width = usize::try_from(bounds.max_x - bounds.min_x + 1).ok()?;
        let height = usize::try_from(bounds.max_y - bounds.min_y + 1).ok()?;
        if width == 0 || height == 0 {
            return None;
        }
        let mut suitable = |(x, y): (i64, i64)| {
            (x, y) != home
                && Some((x, y)) != cache
                && regions.region_at(x, y) == region.id
                && is_land(world.tile(x, y).biome.biome())
        };
        for _ in 0..64 {
            let at = (bounds.min_x + rng.below(width) as i64, bounds.min_y + rng.below(height) as i64);
            if suitable(at) {
                return Some((kind, at));
            }
        }
        let area = width.checked_mul(height)?;
        let start = rng.below(area);
        (0..area).find_map(|offset| {
            let index = (start + offset) % area;
            let at = (bounds.min_x + (index % width) as i64, bounds.min_y + (index / width) as i64);
            suitable(at).then_some((kind, at))
        })
    }

    /// Discover nearby caches and landmarks from a lookout.
    fn survey_nearby(&mut self) -> (usize, usize) {
        let mut regions = BTreeSet::new();
        for dy in -LOOKOUT_RADIUS..=LOOKOUT_RADIUS {
            for dx in -LOOKOUT_RADIUS..=LOOKOUT_RADIUS {
                let at = (self.position.0 + dx, self.position.1 + dy);
                if is_land(self.world.tile(at.0, at.1).biome.biome()) {
                    regions.insert(self.regions.region_at(at.0, at.1));
                }
            }
        }
        let mut caches_found = 0;
        let mut landmarks_found = 0;
        for id in regions {
            let Some(region) = self.regions.region(id) else {
                continue;
            };
            if !self.caches.contains_key(&id)
                && let Some(position) = Self::cache_site(&self.world, &mut self.regions, &region, self.home)
                && chebyshev(position, self.position) <= LOOKOUT_RADIUS
            {
                self.reveal_cache(&region, position);
                caches_found += usize::from(self.caches.contains_key(&id));
            }
            if self.landmarks.contains_key(&id) {
                continue;
            }
            if let Some((kind, position)) = Self::landmark_site(&self.world, &mut self.regions, &region, self.home)
                && chebyshev(position, self.position) <= LOOKOUT_RADIUS
            {
                self.landmarks.insert(id, LandmarkRecord::new(id, kind, position));
                landmarks_found += 1;
            }
        }
        self.materialize_cache_guards();
        (caches_found, landmarks_found)
    }

    /// Whether this deterministic cache is an upgraded guarded ruin.
    pub(super) fn cache_is_ruin(seed: u64, id: RegionId) -> bool {
        let (x, y) = id.anchor();
        let mut rng = Rng::keyed(seed, b"ruins___", &[x as u64, y as u64]);
        rng.below(4) == 0
    }

    /// Select an unowned relic deterministically for a site. The independently keyed rotation avoids
    /// always awarding the same relic first while still ensuring the four unique relics are obtainable.
    pub(super) fn relic_reward_for(&self, id: RegionId) -> Option<Relic> {
        let (x, y) = id.anchor();
        let mut rng = Rng::keyed(self.world.seed(), b"reliclot", &[x as u64, y as u64]);
        let start = rng.below(Relic::ALL.len());
        (0..Relic::ALL.len())
            .map(|offset| Relic::ALL[(start + offset) % Relic::ALL.len()])
            .find(|relic| !self.relics.contains(relic))
    }

    /// Record and automatically attune a new relic if a slot is free.
    pub(super) fn grant_relic(&mut self, relic: Relic) -> bool {
        if self.relics.contains(&relic) {
            return false;
        }
        self.relics.push(relic);
        if let Some(slot) = self.attuned_relics.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(relic);
        }
        true
    }

    pub(super) fn spring_cooldown(&self) -> u64 {
        if self.has_relic(Relic::SpringVial) { SPRING_COOLDOWN_TURNS / 2 } else { SPRING_COOLDOWN_TURNS }
    }

    pub(super) fn camp_healing_bonus(&self) -> i32 {
        if self.has_relic(Relic::HearthEmber) { HEARTH_EMBER_HEAL } else { 0 }
    }

    pub(super) fn sneaking_focus_cost(&self) -> i32 {
        if self.has_relic(Relic::Hushstone) { 0 } else { 1 }
    }

    pub(super) fn death_health(&self) -> i32 {
        if self.has_relic(Relic::ReturningLantern) { RETURNING_LANTERN_HEALTH } else { 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_at_generated(kind: LandmarkKind) -> Game {
        (0..96)
            .find_map(|seed| {
                let mut game = Game::new(seed);
                let mut ids = vec![game.region.id];
                ids.extend(game.regions.region_neighbors(game.region.id));
                let site = ids.into_iter().find_map(|id| {
                    let region = game.regions.region(id)?;
                    let (found, position) = Game::landmark_site(&game.world, &mut game.regions, &region, game.home)?;
                    (found == kind).then_some((id, position))
                })?;
                game.position = site.1;
                game.update_region(false);
                game.landmarks.insert(site.0, LandmarkRecord::new(site.0, kind, site.1));
                for monster in &mut game.monsters {
                    monster.energy = -100_000;
                }
                Some(game)
            })
            .expect("the sample contains the requested generated landmark")
    }

    fn game_with_site(kind: LandmarkKind) -> Game {
        let mut game = Game::new(42);
        game.monsters.clear();
        let position = game.position;
        game.landmarks.insert(game.region.id, LandmarkRecord::new(game.region.id, kind, position));
        game
    }

    #[test]
    fn shrine_awards_once_and_fills_a_free_attunement_slot() {
        let mut game = game_with_site(LandmarkKind::Shrine);
        let turn = game.turn;
        assert_eq!(game.use_landmark(), Step::Waited);
        assert_eq!(game.turn, turn + 1);
        assert_eq!(game.relics().len(), 1);
        assert_eq!(game.attuned_relics().iter().flatten().count(), 1);
        let turn = game.turn;
        assert_eq!(game.use_landmark(), Step::Blocked);
        assert_eq!(game.turn, turn);
    }

    #[test]
    fn spring_refills_both_supplies_and_honors_the_vial_cooldown() {
        let mut game = game_with_site(LandmarkKind::Spring);
        game.salves = 0;
        game.draughts = 0;
        game.grant_relic(Relic::SpringVial);

        assert_eq!(game.use_landmark(), Step::Waited);
        assert_eq!(game.salves, SALVE_CAPACITY);
        assert_eq!(game.draughts, DRAUGHT_CAPACITY);
        let record = game.landmarks.get(&game.region.id).unwrap();
        let last_used = record.last_used.unwrap();
        assert!(!game.landmark_info(record).ready);

        game.turn = last_used + SPRING_COOLDOWN_TURNS / 2 - 1;
        assert!(!game.landmark_info(game.landmarks.get(&game.region.id).unwrap()).ready);
        game.turn += 1;
        assert!(game.landmark_info(game.landmarks.get(&game.region.id).unwrap()).ready);
    }

    #[test]
    fn wild_garden_yields_each_herb_and_regrows() {
        let mut game = game_with_site(LandmarkKind::WildGarden);
        assert_eq!(game.use_landmark(), Step::Waited);
        assert_eq!(game.herbs, [1, 1, 1]);
        assert_eq!(game.use_landmark(), Step::Blocked);
        let last_used = game.landmarks[&game.region.id].last_used.unwrap();
        game.turn = last_used + GARDEN_REGROW_TURNS;
        assert!(game.landmark_info(&game.landmarks[&game.region.id]).ready);
        let before = game.wanderer.herbalism;
        game.wanderer.herbalism.learn(30_000, Rank::Expert);
        game.sneaking = true;
        game.monsters.clear();
        assert_eq!(game.use_landmark(), Step::Waited);
        assert!(game.sneaking);
        assert_ne!(game.wanderer.herbalism, before);
        let last_used = game.landmarks[&game.region.id].last_used.unwrap();
        game.turn = last_used + 149;
        assert!(!game.landmark_info(&game.landmarks[&game.region.id]).ready);
        game.turn += 1;
        assert!(game.landmark_info(&game.landmarks[&game.region.id]).ready);
    }

    #[test]
    fn lookout_marks_the_starting_cache_beyond_normal_view() {
        let mut game = Game::new(42);
        game.monsters.clear();
        let region = game.region.clone();
        let cache = Game::cache_site(&game.world, &mut game.regions, &region, game.home).unwrap();
        assert!(chebyshev(cache, game.position) > VIEW_RADIUS);
        assert!(!game.caches.contains_key(&region.id));
        game.landmarks.insert(region.id, LandmarkRecord::new(region.id, LandmarkKind::Lookout, game.position));

        assert_eq!(game.use_landmark(), Step::Waited);
        assert!(game.caches.contains_key(&region.id));
        assert!(!game.landmark_info(&game.landmarks[&region.id]).ready);
    }

    #[test]
    fn used_landmark_cooldowns_and_relics_survive_snapshot_round_trip() {
        let mut game = game_at_generated(LandmarkKind::Spring);
        game.salves = 0;
        game.draughts = 0;
        game.grant_relic(Relic::SpringVial);
        assert_eq!(game.use_landmark(), Step::Waited);

        let restored = Game::from_snapshot(game.snapshot()).unwrap();
        assert_eq!(restored, game);
        let spring = restored.landmark_at(game.position.0, game.position.1).unwrap();
        assert!(!spring.ready);
        assert_eq!(restored.relics(), &[Relic::SpringVial]);
        assert_eq!(restored.attuned_relics()[0], Some(Relic::SpringVial));
    }

    #[test]
    fn each_attuned_relic_changes_only_its_bounded_action() {
        let mut ember = Game::new(42);
        ember.monsters.clear();
        ember.grant_relic(Relic::HearthEmber);
        ember.wanderer.hp = ember.wanderer.max_hp() - 3;
        assert_eq!(ember.camp(), Step::Waited);
        assert_eq!(ember.wanderer.hp, ember.wanderer.max_hp());

        let mut hushstone = Game::new(42);
        hushstone.monsters.clear();
        hushstone.grant_relic(Relic::Hushstone);
        hushstone.wanderer.focus = 0;
        assert_eq!(hushstone.toggle_sneak(), Step::Waited);
        assert_eq!(hushstone.wanderer.focus, 0);

        let mut lantern = Game::new(42);
        lantern.monsters.clear();
        lantern.grant_relic(Relic::ReturningLantern);
        lantern.die();
        assert_eq!(lantern.wanderer.hp, RETURNING_LANTERN_HEALTH);
    }

    #[test]
    fn attunement_is_unique_owned_and_safe_at_home() {
        let mut game = Game::new(42);
        game.monsters.clear();
        for relic in Relic::ALL {
            game.grant_relic(relic);
        }
        assert!(game.attune_relics([Some(Relic::HearthEmber), Some(Relic::Hushstone)]).is_ok());
        assert!(game.attune_relics([Some(Relic::HearthEmber), Some(Relic::HearthEmber)]).is_err());
        assert!(game.attune_relics([Some(Relic::HearthEmber), None]).is_ok());
        assert!(game.cycle_relics().is_ok());
        assert_ne!(game.attuned_relics(), [Some(Relic::HearthEmber), None]);
        game.position = (game.home.0 + 1, game.home.1);
        assert!(game.cycle_relics().is_err());
    }

    #[test]
    fn cycling_relics_reaches_every_pair_of_four() {
        let mut game = Game::new(42);
        game.monsters.clear();
        for relic in Relic::ALL {
            game.grant_relic(relic);
        }
        let mut pairs = HashSet::new();
        for _ in 0..6 {
            pairs.insert(game.attuned_relics());
            game.cycle_relics().unwrap();
        }
        assert_eq!(pairs.len(), 6);
    }

    #[test]
    fn deterministic_ruins_have_three_tougher_guards_and_a_relic_reward() {
        let (mut game, region, position) = (0..64)
            .find_map(|seed| {
                let mut game = Game::new(seed);
                let mut ids = vec![game.region.id];
                ids.extend(game.regions.region_neighbors(game.region.id));
                let site = ids.into_iter().find_map(|id| {
                    let region = game.regions.region(id)?;
                    let position = Game::cache_site(&game.world, &mut game.regions, &region, game.home)?;
                    (chebyshev(position, game.home) > 4 * VIEW_RADIUS
                        && Game::cache_is_ruin(game.world.seed(), region.id))
                    .then_some((region, position))
                });
                site.map(|(region, position)| {
                    game.monsters.clear();
                    (game, region, position)
                })
            })
            .expect("the sample includes a nearby guarded ruin");
        game.reveal_cache(&region, position);
        let cache = game.caches.get(&region.id).expect("the three-guard ruin fits");
        assert!(cache.ruin);
        assert_eq!(cache.guards.len(), 3);
        for guard in &cache.guards {
            let base = Phenotype::of(&guard.genome).stats;
            assert!(guard.phenotype.stats.hp > base.hp);
            assert!(guard.phenotype.stats.attack > base.attack);
            assert!(guard.phenotype.stats.defense > base.defense);
        }
        let info = game.cache_at(position.0, position.1).unwrap();
        assert!(info.relic_reward.is_some());

        game.position = position;
        game.update_region(false);
        assert_eq!(game.claim_cache(), Step::Waited);
        assert_eq!(game.relics.len(), 1);
        assert_eq!(Game::from_snapshot(game.snapshot()).unwrap(), game);
    }
}

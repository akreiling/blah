//! One immutable, authored arc. Exceptional actors stay authoritative beyond ambient despawn.

use super::*;
use crate::{Burst, Element, Shape, Stats, Trigger};

#[cfg(test)]
mod journeys;
#[cfg(test)]
mod tests;

type Point = (i64, i64);

/// Compatibility of the hand-authored mechanics, independent of a character's development.
pub const CINDER_WATCH_VERSION: u32 = 1;
/// Maximum search distance from home. A failed bounded search offers no site, rather than bad terrain.
pub const ENCOUNTER_SEARCH_RADIUS: i64 = 64;

/// The supported authored encounter template.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterTemplate {
    /// Two braziers feed a fire warden; a lesser tender teaches its firing cycle.
    CinderWatch,
}

/// A committed placement and mechanics revision. No player rank or loadout enters this definition.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EncounterDefinition {
    /// Executable template, not free-form prose.
    pub template: EncounterTemplate,
    /// Mechanics revision.
    pub version: u32,
    /// The warden's original station.
    pub origin: Point,
}

impl EncounterDefinition {
    /// Warning stone and, after resolution, restored spring.
    #[must_use]
    pub const fn entrance(self) -> Point {
        (self.origin.0 - 6, self.origin.1)
    }

    /// The two independently disruptable fuel sources.
    #[must_use]
    pub const fn braziers(self) -> [Point; 2] {
        [(self.origin.0 - 1, self.origin.1 - 2), (self.origin.0 + 1, self.origin.1 + 2)]
    }

    /// An ancillary objective that does not require killing the warden.
    #[must_use]
    pub const fn salvage(self) -> Point {
        (self.origin.0 + 2, self.origin.1 - 3)
    }

    const fn station(self, kind: SentryKind) -> Point {
        match kind {
            SentryKind::Tender => (self.origin.0 - 3, self.origin.1),
            SentryKind::Warden => self.origin,
        }
    }
}

/// Durable arc progress. Retreat returns an engaged site to investigated, not to unrevealed.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterPhase {
    /// The warning destination is charted; actors do not yet exist.
    Revealed,
    /// Mechanics are explained and actors committed, including after retreat.
    Investigated,
    /// The warden has entered its capability cycle.
    Engaged,
    /// The warden is dead and the spring is restored.
    Resolved,
}

/// A capability cycle shared by the lesser tender and the apex.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SentryCycle {
    /// Waiting for detection or a hostile action.
    Dormant,
    /// Gathering heat before committing a lane.
    Drawing,
    /// This exact lane fires after the player's next accepted turn.
    Aiming(Dir),
    /// Remaining player actions with the shell open.
    Exposed(u8),
}

impl SentryCycle {
    /// Compact state suitable for a viewport-sized status panel.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dormant => "dormant",
            Self::Drawing => "drawing heat",
            Self::Aiming(_) => "FIRING NEXT TURN",
            Self::Exposed(_) => "shell exposed",
        }
    }
}

/// A template role, not a player identity or a biological species.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SentryKind {
    /// A lesser inhabitant demonstrating the same cycle.
    Tender,
    /// The exceptional apex, powered by the two braziers.
    Warden,
}

impl SentryKind {
    /// Name of this exceptional individual.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tender => "Cinder tender",
            Self::Warden => "Cinder warden",
        }
    }

    const fn health(self) -> i32 {
        match self {
            Self::Tender => 8,
            Self::Warden => 54,
        }
    }

    const fn range(self) -> i64 {
        match self {
            Self::Tender => 3,
            Self::Warden => 4,
        }
    }
}

/// Public actor state; no hidden future targeting or random rolls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SentryInfo {
    /// Stable individual identity.
    pub id: MonsterId,
    /// Authored role.
    pub kind: SentryKind,
    /// Remaining Health; zero means dead.
    pub hp: i32,
    /// Current shell protection, also used by ordinary combat previews.
    pub defense: i32,
    /// Current action commitment.
    pub cycle: SentryCycle,
}

/// The discovered site's objectives and immutable placement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncounterInfo {
    /// Immutable mechanics and placement.
    pub definition: EncounterDefinition,
    /// Current arc progress.
    pub phase: EncounterPhase,
    /// Which fuel sources remain lit.
    pub braziers: [bool; 2],
    /// Whether the ancillary objective was taken.
    pub salvage_claimed: bool,
    /// Whether the apex's Hearth ember was taken.
    pub reward_claimed: bool,
    /// Whether the restored spring has recovered its supplies.
    pub spring_ready: bool,
    /// Recorded abandonments, including deaths during an active cycle.
    pub retreats: u32,
    /// Visible committed actors.
    pub sentries: Vec<SentryInfo>,
}

/// Readable map objects operated with the ordinary site-interaction key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterTile {
    /// Investigate here before engaging.
    Warning,
    /// A fuel source; true means it remains lit.
    Brazier(bool),
    /// The ancillary objective; true means it is unclaimed.
    Salvage(bool),
    /// The apex reward; true means it is unclaimed.
    Heart(bool),
    /// Restored supplies; true means the spring is ready.
    Spring(bool),
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct SentryState {
    pub(super) id: MonsterId,
    pub(super) kind: SentryKind,
    pub(super) alive: bool,
    pub(super) cycle: SentryCycle,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct EncounterRecord {
    pub(super) definition: EncounterDefinition,
    pub(super) phase: EncounterPhase,
    pub(super) sentries: Vec<SentryState>,
    pub(super) braziers: [bool; 2],
    pub(super) salvage_claimed: bool,
    pub(super) reward_claimed: bool,
    pub(super) last_refill: Option<u64>,
    pub(super) retreats: u32,
    pub(super) last_retreat: Option<u64>,
}

impl EncounterRecord {
    fn new(definition: EncounterDefinition) -> Self {
        Self {
            definition,
            phase: EncounterPhase::Revealed,
            sentries: Vec::new(),
            braziers: [true; 2],
            salvage_claimed: false,
            reward_claimed: false,
            last_refill: None,
            retreats: 0,
            last_retreat: None,
        }
    }

    fn armor(&self, kind: SentryKind, cycle: SentryCycle) -> i32 {
        if self.phase == EncounterPhase::Resolved || matches!(cycle, SentryCycle::Exposed(_)) {
            0
        } else if kind == SentryKind::Tender {
            1
        } else {
            4 * self.braziers.iter().filter(|&&lit| lit).count() as i32
        }
    }

    fn power(&self, kind: SentryKind) -> u8 {
        if kind == SentryKind::Tender { 2 } else { 2 + 2 * self.braziers.iter().filter(|&&lit| lit).count() as u8 }
    }
}

#[derive(Clone, Copy)]
struct FireLane {
    origin: Point,
    direction: Dir,
    range: i64,
    power: u8,
}

impl FireLane {
    fn contains(self, at: Point) -> bool {
        let (dx, dy) = self.direction.delta();
        (1..=self.range).any(|n| at == (self.origin.0 + n * dx, self.origin.1 + n * dy))
    }

    const fn burst(self) -> Burst {
        Burst { power: self.power, trigger: Trigger::Contact, element: Element::Fire, shape: Shape::Point }
    }
}

impl Game {
    /// Trace the cinder smoke without passing time. Existing saves can opt into this new arc too.
    /// Search and definition depend on the world and home only, never the player's build.
    pub fn chart_encounter(&mut self) -> bool {
        if !self.journey || self.encounter.is_some() {
            self.log.push(LogKind::Blocked, "No new cinder trail to chart.");
            return false;
        }
        let Some(definition) = Self::cinder_watch_site(&self.world, self.home) else {
            self.log.push(LogKind::Blocked, "The smoke leads beyond reachable open ground. No site charted.");
            return false;
        };
        self.encounter = Some(EncounterRecord::new(definition));
        self.log.push(LogKind::Region, "Cinder Watch: follow the smoke to the warning stone (X).");
        self.log
            .push(LogKind::Region, "Scorched lanes warn of aimed fire. Investigate before entering; retreat is open.");
        true
    }

    /// The current authored arc, once its attraction has been charted.
    #[must_use]
    pub fn encounter(&self) -> Option<EncounterInfo> {
        let record = self.encounter.as_ref()?;
        Some(EncounterInfo {
            definition: record.definition,
            phase: record.phase,
            braziers: record.braziers,
            salvage_claimed: record.salvage_claimed,
            reward_claimed: record.reward_claimed,
            spring_ready: record.phase == EncounterPhase::Resolved
                && record.last_refill.is_none_or(|t| self.turn - t >= SPRING_COOLDOWN_TURNS),
            retreats: record.retreats,
            sentries: record
                .sentries
                .iter()
                .map(|state| {
                    let monster = self.monsters.iter().find(|m| m.id == state.id);
                    SentryInfo {
                        id: state.id,
                        kind: state.kind,
                        hp: monster.map_or(0, |m| m.hp),
                        defense: record.armor(state.kind, state.cycle),
                        cycle: state.cycle,
                    }
                })
                .collect(),
        })
    }

    /// Known site objects. Charting reveals a destination, not the approach terrain.
    #[must_use]
    pub fn encounter_tile(&self, at: Point) -> Option<EncounterTile> {
        let record = self.encounter.as_ref()?;
        let def = record.definition;
        if at == def.entrance() {
            return Some(if record.phase == EncounterPhase::Resolved {
                EncounterTile::Spring(record.last_refill.is_none_or(|t| self.turn - t >= SPRING_COOLDOWN_TURNS))
            } else {
                EncounterTile::Warning
            });
        }
        if record.phase == EncounterPhase::Revealed {
            return None;
        }
        if let Some(i) = def.braziers().iter().position(|&p| p == at) {
            Some(EncounterTile::Brazier(record.braziers[i]))
        } else if at == def.salvage() {
            Some(EncounterTile::Salvage(!record.salvage_claimed))
        } else if at == def.origin && record.phase == EncounterPhase::Resolved {
            Some(EncounterTile::Heart(!record.reward_claimed))
        } else {
            None
        }
    }

    /// Damage in all currently committed lanes. These exact tiles resolve after the next accepted turn.
    #[must_use]
    pub fn encounter_danger_at(&self, at: Point) -> i32 {
        self.encounter
            .as_ref()
            .into_iter()
            .flat_map(|r| &r.sentries)
            .filter_map(|state| self.fire_lane(state))
            .filter(|lane| lane.contains(at))
            .map(|lane| lane.burst().damage(self.wanderer.stats.defense, 0, false, false))
            .sum()
    }

    /// Interact with a known encounter object, otherwise use the ordinary landmark underfoot.
    pub fn use_site(&mut self) -> Step {
        let Some(tile) = self.encounter_tile(self.position) else {
            return self.use_landmark();
        };
        let record = self.encounter.as_ref().expect("known object");
        let def = record.definition;
        match tile {
            EncounterTile::Warning => {
                if record.phase != EncounterPhase::Revealed {
                    self.log.push(LogKind::Blocked, "Already investigated: dodge marked lanes or shut the braziers.");
                    return Step::Blocked;
                }
                if [SentryKind::Tender, SentryKind::Warden]
                    .iter()
                    .any(|&kind| self.monster_at(def.station(kind).0, def.station(kind).1).is_some())
                {
                    self.log
                        .push(LogKind::Blocked, "A creature occupies a sentry station. Clear it before investigating.");
                    return Step::Blocked;
                }
                self.investigate_cinder_watch();
            }
            EncounterTile::Brazier(true) => {
                let quiet = self.sneaking
                    && self.encounter.as_ref().expect("site").sentries.iter().all(|state| {
                        self.monsters
                            .iter()
                            .find(|m| m.id == state.id)
                            .is_none_or(|m| !m.detects(self.position, self.concealment()))
                    });
                let cost = if quiet { 2 } else { 4 };
                if self.wanderer.focus < cost {
                    self.log
                        .push(LogKind::Blocked, "Shutting a brazier needs 4 Focus, or 2 while unseen and sneaking.");
                    return Step::Blocked;
                }
                self.wanderer.focus -= cost;
                let i = def.braziers().iter().position(|&p| p == self.position).expect("brazier");
                self.encounter.as_mut().expect("site").braziers[i] = false;
                self.log.push(
                    LogKind::Attack,
                    format!("You shut a brazier for {cost} Focus. The warden's shell and fire weaken."),
                );
                if !quiet {
                    self.sneaking = false;
                } else {
                    self.encounter_stealth_lesson();
                }
                self.refresh_sentry_armor();
            }
            EncounterTile::Salvage(true) => {
                self.encounter.as_mut().expect("site").salvage_claimed = true;
                self.grant_relic(Relic::Hushstone);
                self.log.push(
                    LogKind::Attack,
                    "You recover the alcove's Hushstone. The warden need not die for this objective.",
                );
                if self.sneaking {
                    self.encounter_stealth_lesson();
                }
            }
            EncounterTile::Heart(true) => {
                self.encounter.as_mut().expect("site").reward_claimed = true;
                self.grant_relic(Relic::HearthEmber);
                self.log.push(
                    LogKind::Attack,
                    "You take the Hearth ember. Cinder Watch's restored spring remains a resupply stop.",
                );
            }
            EncounterTile::Spring(true) if self.salves < SALVE_CAPACITY || self.draughts < DRAUGHT_CAPACITY => {
                self.salves = SALVE_CAPACITY;
                self.draughts = DRAUGHT_CAPACITY;
                self.encounter.as_mut().expect("site").last_refill = Some(self.turn + 1);
                self.log.push(
                    LogKind::Attack,
                    "The restored spring refills salves and draughts. It recovers in 300 turns.",
                );
            }
            _ => {
                self.log.push(LogKind::Blocked, "This object is spent, recovering, or your supplies are already full.");
                return Step::Blocked;
            }
        }
        self.turn += 1;
        self.end_turn();
        Step::Waited
    }

    fn encounter_stealth_lesson(&mut self) {
        let ids = self.encounter_actor_ids();
        for id in ids {
            if let Some(i) = self.monsters.iter().position(|m| m.id == id)
                && !self.monsters[i].detects(self.position, self.concealment())
                && self.monsters[i].detects(self.position, 0)
                && let Some(before) = self.stealth_lesson(i, OPENING_LESSON)
            {
                self.announce("stealth", before, self.wanderer.stealth.rank(), crate::LearnedMove::SlipAway);
            }
        }
    }

    fn cinder_watch_site(world: &World, home: Point) -> Option<EncounterDefinition> {
        let mut queue = VecDeque::from([home]);
        let mut seen = HashSet::from([home]);
        while let Some(at) = queue.pop_front() {
            if seen.len() > 8_192 {
                break;
            }
            if chebyshev(at, home) >= 24
                && (-4..=4).all(|dy| (-6..=4).all(|dx| is_land(world.tile(at.0 + dx, at.1 + dy).biome.biome())))
            {
                return Some(EncounterDefinition {
                    template: EncounterTemplate::CinderWatch,
                    version: CINDER_WATCH_VERSION,
                    origin: at,
                });
            }
            for dir in Dir::ALL {
                let (dx, dy) = dir.delta();
                let next = (at.0 + dx, at.1 + dy);
                if chebyshev(next, home) <= ENCOUNTER_SEARCH_RADIUS
                    && !seen.contains(&next)
                    && is_land(world.tile(next.0, next.1).biome.biome())
                {
                    seen.insert(next);
                    queue.push_back(next);
                }
            }
        }
        None
    }

    fn investigate_cinder_watch(&mut self) {
        let def = self.encounter.as_ref().expect("charted site").definition;
        for kind in [SentryKind::Tender, SentryKind::Warden] {
            let position = def.station(kind);
            let region = self.regions.region_at(position.0, position.1);
            let source = self.life(region).species[0].clone();
            let mut genome = source.pool[0];
            genome.set(crate::Gene::Water, 0.2);
            let mut phenotype = Phenotype::of(&genome);
            phenotype.temperament = Temperament::Territorial;
            phenotype.sense = kind.range();
            phenotype.stats = Stats { hp: kind.health(), attack: 4, defense: 0, speed: ACTION_COST };
            let id = MonsterId(self.next_monster);
            self.next_monster += 1;
            let sprite = Sprite::draw(&phenotype, &self.clade(source.id.clade).plan, source.biome);
            self.monsters.push(Monster {
                id,
                species: source.id,
                name: kind.name().into(),
                genome,
                phenotype,
                sprite,
                position,
                hp: kind.health(),
                home: position,
                alerted: false,
                energy: 0,
                practice_left: kind.health(),
                stealth_left: STEALTH_LESSONS,
                tracking: false,
            });
            self.encounter.as_mut().expect("site").sentries.push(SentryState {
                id,
                kind,
                alive: true,
                cycle: SentryCycle::Dormant,
            });
        }
        self.encounter.as_mut().expect("site").phase = EncounterPhase::Investigated;
        self.refresh_sentry_armor();
        self.log.push(
            LogKind::Region,
            "The tender draws, aims a straight lane, fires, then exposes its shell for two actions.",
        );
        self.log.push(
            LogKind::Region,
            "The 54-Health warden does the same: 12 fire damage, range 4. Red lanes fire next turn.",
        );
        self.log.push(
            LogKind::Region,
            "X shuts braziers (4 Focus; 2 unseen), weakening shell and fire. The alcove holds a Hushstone.",
        );
    }

    pub(super) fn encounter_actor_ids(&self) -> Vec<MonsterId> {
        self.encounter.as_ref().map_or_else(Vec::new, |r| r.sentries.iter().map(|s| s.id).collect())
    }

    fn fire_lane(&self, state: &SentryState) -> Option<FireLane> {
        let SentryCycle::Aiming(direction) = state.cycle else { return None };
        let record = self.encounter.as_ref()?;
        let monster = self.monsters.iter().find(|m| m.id == state.id)?;
        (state.alive && record.phase != EncounterPhase::Resolved).then_some(FireLane {
            origin: monster.position,
            direction,
            range: state.kind.range(),
            power: record.power(state.kind),
        })
    }

    fn refresh_sentry_armor(&mut self) {
        if let Some(record) = &self.encounter {
            for state in &record.sentries {
                if let Some(monster) = self.monsters.iter_mut().find(|m| m.id == state.id) {
                    monster.phenotype.stats.defense = record.armor(state.kind, state.cycle);
                }
            }
        }
    }

    pub(super) fn encounter_actor_died(&mut self, id: MonsterId) {
        let Some(record) = &mut self.encounter else { return };
        let Some(state) = record.sentries.iter_mut().find(|s| s.id == id) else { return };
        state.alive = false;
        state.cycle = SentryCycle::Dormant;
        if state.kind == SentryKind::Warden {
            record.phase = EncounterPhase::Resolved;
            for sentry in &mut record.sentries {
                sentry.cycle = SentryCycle::Dormant;
            }
            self.log.push(
                LogKind::Region,
                "The warden falls. The furnace goes quiet and the spring runs clear. X claims the heart.",
            );
            self.refresh_sentry_armor();
        }
    }

    /// Abandonment cancels pending fire but preserves wounds, shutdowns, lessons, deaths, and claims.
    pub(super) fn retreat_encounter(&mut self) {
        let Some(record) = &mut self.encounter else { return };
        if record.phase == EncounterPhase::Resolved || record.sentries.iter().all(|s| s.cycle == SentryCycle::Dormant) {
            return;
        }
        record.retreats = record.retreats.saturating_add(1);
        record.last_retreat = Some(self.turn);
        record.phase = EncounterPhase::Investigated;
        for state in &mut record.sentries {
            state.cycle = SentryCycle::Dormant;
            if let Some(monster) = self.monsters.iter_mut().find(|m| m.id == state.id) {
                monster.alerted = false;
                monster.tracking = false;
            }
        }
        self.refresh_sentry_armor();
        self.log.push(LogKind::Region, "You abandon Cinder Watch. The fire settles; wounds and shut braziers remain.");
    }

    pub(super) fn advance_encounter(&mut self, sheltered: bool) {
        let Some(record) = &self.encounter else { return };
        if record.phase == EncounterPhase::Revealed || record.phase == EncounterPhase::Resolved {
            return;
        }
        if chebyshev(self.position, record.definition.origin) > 7 {
            self.retreat_encounter();
            return;
        }
        let states = record.sentries.clone();
        for state in states {
            let Some(monster) = self.monsters.iter().find(|m| m.id == state.id) else { continue };
            if self.encounter.as_ref().expect("site").phase == EncounterPhase::Resolved {
                break;
            }
            let next = match state.cycle {
                SentryCycle::Dormant if !monster.alerted => continue,
                SentryCycle::Dormant => {
                    self.log.push(
                        LogKind::Attack,
                        format!("The {} draws heat. Watch for its aimed lane.", state.kind.name()),
                    );
                    SentryCycle::Drawing
                }
                SentryCycle::Drawing => {
                    let (dx, dy) = (self.position.0 - monster.position.0, self.position.1 - monster.position.1);
                    let dir = if dx.abs() >= dy.abs() {
                        if dx < 0 { Dir::West } else { Dir::East }
                    } else if dy < 0 {
                        Dir::North
                    } else {
                        Dir::South
                    };
                    self.log.push(
                        LogKind::Attack,
                        format!("The {} aims {dir:?}. Red lane fires after your next action.", state.kind.name()),
                    );
                    SentryCycle::Aiming(dir)
                }
                SentryCycle::Aiming(_) => {
                    if let Some(lane) = self.fire_lane(&state) {
                        let deaths = self.deaths;
                        self.release_fire(lane, state.kind, sheltered);
                        if self.deaths != deaths {
                            return;
                        }
                    }
                    SentryCycle::Exposed(2)
                }
                SentryCycle::Exposed(2) => SentryCycle::Exposed(1),
                SentryCycle::Exposed(_) => SentryCycle::Drawing,
            };
            let record = self.encounter.as_mut().expect("site");
            if record.phase == EncounterPhase::Resolved {
                break;
            }
            if state.kind == SentryKind::Warden {
                record.phase = EncounterPhase::Engaged;
            }
            if let Some(sentry) = record.sentries.iter_mut().find(|s| s.id == state.id && s.alive) {
                sentry.cycle = next;
            }
        }
        self.refresh_sentry_armor();
    }

    fn release_fire(&mut self, lane: FireLane, kind: SentryKind, sheltered: bool) {
        self.log.push(LogKind::Attack, format!("The {} fires. Its shell opens for two actions.", kind.name()));
        let mut dead = Vec::new();
        // Sentries share the braziers' fire and never burn each other; ambient creatures can be caught.
        let sentries = self.encounter_actor_ids();
        for monster in &mut self.monsters {
            if lane.contains(monster.position) && !sentries.contains(&monster.id) {
                let hit = lane.burst().damage(monster.phenotype.stats.defense, monster.fire_resistance(), false, false);
                monster.hp = monster.hp.saturating_sub(hit);
                self.log.push(LogKind::Hurt, format!("The fire hits the {} for {hit}.", monster.name));
                if monster.hp <= 0 {
                    self.log.push(LogKind::Kill, format!("The {} dies in the fire.", monster.name));
                    dead.push(monster.id);
                }
            }
        }
        for id in dead {
            self.mark_guard_dead(id);
        }
        self.monsters.retain(|m| m.hp > 0);
        if !sheltered && lane.contains(self.position) {
            let hit = lane.burst().damage(self.wanderer.stats.defense, 0, false, false);
            self.wanderer.hp -= hit;
            self.log.push(LogKind::Hurt, format!("The marked fire lane hits you for {hit}."));
            if self.wanderer.hp <= 0 {
                self.die();
            }
        }
    }

    pub(super) fn encounter_displaced(&mut self, id: MonsterId) {
        let Some(record) = &mut self.encounter else { return };
        let Some(state) = record.sentries.iter_mut().find(|s| s.id == id && s.alive) else { return };
        if matches!(state.cycle, SentryCycle::Drawing | SentryCycle::Aiming(_)) {
            state.cycle = SentryCycle::Exposed(2);
            self.log.push(LogKind::Attack, "The shove interrupts the committed fire and opens the shell.");
            self.refresh_sentry_armor();
        }
    }

    pub(super) fn validate_encounter(&self) -> Result<(), &'static str> {
        let Some(record) = &self.encounter else { return Ok(()) };
        let invalid = || Err("Invalid Cinder Watch definition or runtime in save.");
        if !self.journey
            || Some(record.definition) != Self::cinder_watch_site(&self.world, self.home)
            || record.last_retreat.is_some_and(|t| t > self.turn)
            || (record.retreats == 0) != record.last_retreat.is_none()
            || record.last_refill.is_some_and(|t| t > self.turn)
            || (record.phase != EncounterPhase::Resolved && (record.reward_claimed || record.last_refill.is_some()))
            || (record.salvage_claimed && !self.relics.contains(&Relic::Hushstone))
            || (record.reward_claimed && !self.relics.contains(&Relic::HearthEmber))
        {
            return invalid();
        }
        if record.phase == EncounterPhase::Revealed {
            return if record.sentries.is_empty()
                && record.braziers == [true; 2]
                && !record.salvage_claimed
                && record.retreats == 0
            {
                Ok(())
            } else {
                invalid()
            };
        }
        if record.sentries.len() != 2
            || record.sentries[0].kind != SentryKind::Tender
            || record.sentries[1].kind != SentryKind::Warden
            || record.sentries[0].id == record.sentries[1].id
            || (record.phase == EncounterPhase::Resolved) == record.sentries[1].alive
        {
            return invalid();
        }
        for state in &record.sentries {
            if state.id.0 >= self.next_monster
                || self.caches.values().any(|cache| cache.guards.iter().any(|guard| guard.id == state.id))
                || matches!(state.cycle, SentryCycle::Exposed(n) if !(1..=2).contains(&n))
                || ((!state.alive || record.phase == EncounterPhase::Resolved) && state.cycle != SentryCycle::Dormant)
            {
                return invalid();
            }
            let monster = self.monsters.iter().find(|m| m.id == state.id);
            if !state.alive {
                if monster.is_some() {
                    return invalid();
                }
                continue;
            }
            let Some(monster) = monster else { return invalid() };
            if monster.name != state.kind.name()
                || monster.max_hp() != state.kind.health()
                || monster.home != record.definition.station(state.kind)
                || chebyshev(monster.position, monster.home) > TERRITORY
                || !monster
                    .phenotype
                    .access
                    .allows(self.world.tile(monster.position.0, monster.position.1).biome.biome())
                || monster.phenotype.stats.defense != record.armor(state.kind, state.cycle)
                || monster.phenotype.stats.attack != 4
                || monster.phenotype.stats.speed != ACTION_COST
                || monster.phenotype.sense != state.kind.range()
                || monster.phenotype.temperament != Temperament::Territorial
                || monster.energy != 0
            {
                return invalid();
            }
        }
        Ok(())
    }
}

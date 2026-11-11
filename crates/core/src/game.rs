//! The game state the frontend drives: one wanderer walking a world, the region it is in, the monsters
//! around it, a turn counter, and a message log.
//!
//! Everything here is a pure function of the seed and the actions taken so far, so the same seed and the
//! same actions always give the same game. Randomness in a turn comes from one stream per purpose: the
//! wanderer's attack keyed by `(seed, turn)`, each monster's actions by `(seed, turn, monster)`, and
//! spawning by `(seed, turn)` under its own label. A draw added to one of them never shifts another.
//!
//! A turn is the wanderer's action (a step, a bump attack, or a wait), then every monster's actions, then
//! healing, despawning, and spawning. Monsters act by temperament and bank their speed as energy, so a fast
//! one sometimes acts twice in a turn and a slow one sometimes not at all.
//!
//! Coordinates follow the terrain's: `x` grows eastward and `y` grows southward, down the screen.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::Adaptation;
use crate::biome::Biome;
use crate::clade::{Clade, CladeId, clades};
use crate::combat::{AreaPreview, AttackContext, AttackPreview, AttackResult, Delivery, Maneuver};
use crate::genome::Genome;
use crate::hash::Rng;
use crate::life::{RegionLife, Species, SpeciesId, TraitDelta, compare};
use crate::monster::{ACTION_COST, MAX_ACTIONS, Monster, MonsterId, Wanderer, damage};
use crate::phenotype::{Phenotype, Temperament};
use crate::region::{Region, RegionCache, RegionId};
use crate::skills::Rank;
use crate::sprite::Sprite;
use crate::world::{Tile, World};

#[cfg(test)]
mod commitment_tests;
mod learned;
mod snapshot;

pub use snapshot::{GameSnapshot, RULES_VERSION, SNAPSHOT_VERSION, TERRAIN_VERSION};

/// Provenance for shared damage and practice accounting.
enum AttackSource<'a> {
    Maneuver(&'a Maneuver),
    Pulse(crate::Weapon),
}

impl AttackSource<'_> {
    fn weapon(&self) -> crate::Weapon {
        match self {
            Self::Maneuver(m) => m.weapon,
            Self::Pulse(weapon) => *weapon,
        }
    }

    fn is_weapon(&self) -> bool {
        matches!(self, Self::Maneuver(m) if m.technique.delivery == Delivery::Weapon)
    }

    fn has_payload(&self) -> bool {
        match self {
            Self::Maneuver(m) => m.technique.burst.is_some(),
            Self::Pulse(_) => true,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Maneuver(m) => &m.name,
            Self::Pulse(_) => crate::LearnedMove::Pulse.name(),
        }
    }
}

/// Lines the message log keeps. Older lines fall off the front.
pub const LOG_LIMIT: usize = 100;

/// How far [`find_land`] looks, in tiles along each axis, before giving up. Oceans span a few hundred tiles
/// at most, so this is never reached in practice; it only keeps a pathological seed from searching forever.
pub const LAND_SEARCH_RADIUS: i64 = 4_096;

/// One of the four directions the wanderer can step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    /// Up the screen, toward smaller `y`.
    North,
    /// Down the screen, toward larger `y`.
    South,
    /// Right, toward larger `x`.
    East,
    /// Left, toward smaller `x`.
    West,
}

impl Dir {
    /// All four directions.
    pub const ALL: [Dir; 4] = [Dir::North, Dir::South, Dir::East, Dir::West];

    /// The `(dx, dy)` of one step this way.
    #[must_use]
    pub const fn delta(self) -> (i64, i64) {
        match self {
            Dir::North => (0, -1),
            Dir::South => (0, 1),
            Dir::East => (1, 0),
            Dir::West => (-1, 0),
        }
    }
}

/// What a [`Game::step`] or [`Game::wait`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The wanderer moved and a turn passed.
    Moved,
    /// A monster stood in the way, so the wanderer attacked it, and a turn passed.
    Attacked,
    /// The wanderer stayed put and a turn passed.
    Waited,
    /// The target tile can't be entered. Nothing moved and no turn passed.
    Blocked,
}

/// Whether the wanderer can start on this biome: dry and walkable.
fn is_land(biome: &Biome) -> bool {
    biome.passable && !biome.water
}

/// The land tile nearest to `(x, y)` by straight-line distance: passable and not water. `(x, y)` itself
/// if it qualifies.
///
/// Searches outward in square rings. Ties at equal distance go to the smallest `y`, then the smallest `x`,
/// so the answer never depends on search order. Returns `None` if there is no land within
/// [`LAND_SEARCH_RADIUS`] tiles along either axis.
#[must_use]
pub fn find_land(world: &World, x: i64, y: i64) -> Option<(i64, i64)> {
    // Best so far as (squared distance, dy, dx), which orders ties the way the doc says.
    let mut best: Option<(i64, i64, i64)> = None;
    for r in 0..=LAND_SEARCH_RADIUS {
        // Every tile on ring r is at least r away, so once r² passes the best distance nothing closer remains.
        if best.is_some_and(|(d2, _, _)| r * r > d2) {
            break;
        }
        for (dx, dy) in ring(r) {
            let d2 = dx * dx + dy * dy;
            if best.is_some_and(|b| (d2, dy, dx) >= b) {
                continue;
            }
            if is_land(world.tile(x + dx, y + dy).biome.biome()) {
                best = Some((d2, dy, dx));
            }
        }
    }
    best.map(|(_, dy, dx)| (x + dx, y + dy))
}

/// The offsets at Chebyshev distance exactly `r` from the center: the border of a `(2r+1)`-wide square, row
/// by row from the top, each row from the left. Ring 0 is the center alone.
pub fn ring(r: i64) -> impl Iterator<Item = (i64, i64)> {
    (-r..=r).flat_map(move |dy| {
        let edge = dy.abs() == r;
        // Top and bottom rows are whole; the rows between only have their two ends.
        let step = if edge || r == 0 { 1 } else { 2 * r as usize };
        (-r..=r).step_by(step).map(move |dx| (dx, dy))
    })
}

/// What a log line reports. Set where the line is written, so a frontend can style lines by what happened
/// rather than by their wording.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LogKind {
    /// The wanderer is in a new region: waking at the start of a game, or walking into one.
    Region,
    /// A step ran into terrain that can't be entered.
    Blocked,
    /// The wanderer attacked a monster, hitting or missing.
    Attack,
    /// A monster attacked the wanderer and missed.
    Missed,
    /// A monster hurt the wanderer.
    Hurt,
    /// A monster died.
    Kill,
    /// The wanderer died, or woke after dying.
    Death,
    /// Earned practice raised a weapon proficiency band.
    Learning,
}

/// One line of the log and what it reports.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    /// What the line reports.
    pub kind: LogKind,
    /// The line itself, a sentence for the player.
    pub text: String,
}

/// A bounded log of messages, oldest first, holding the last [`LOG_LIMIT`] lines.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Log {
    entries: VecDeque<LogEntry>,
    total: u64,
}

impl Log {
    /// Appends a line of `kind`, dropping the oldest if the log is full.
    pub fn push(&mut self, kind: LogKind, line: impl Into<String>) {
        if self.entries.len() == LOG_LIMIT {
            self.entries.pop_front();
        }
        self.entries.push_back(LogEntry { kind, text: line.into() });
        self.total += 1;
    }

    /// Entries held, oldest first.
    pub fn entries(&self) -> impl DoubleEndedIterator<Item = &LogEntry> + ExactSizeIterator {
        self.entries.iter()
    }

    /// The text of the entries held, oldest first.
    pub fn lines(&self) -> impl DoubleEndedIterator<Item = &str> + ExactSizeIterator {
        self.entries.iter().map(|e| e.text.as_str())
    }

    /// The last `n` entries (fewer if the log is shorter), oldest first.
    pub fn last(&self, n: usize) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter().skip(self.entries.len().saturating_sub(n))
    }

    /// Number of lines held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log holds no lines.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Lines ever pushed, including those that have fallen off. The difference across an action is how many
    /// lines it added.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.total
    }
}

/// Half the side of the square the frontend shows around the wanderer: the view is 17×17 tiles. Monsters
/// spawn just outside it.
pub const VIEW_RADIUS: i64 = 8;
/// Nearest a monster spawns, in tiles (Chebyshev distance): just out of view.
pub const SPAWN_MIN: i64 = VIEW_RADIUS + 1;
/// Farthest a monster spawns.
pub const SPAWN_MAX: i64 = VIEW_RADIUS + 3;
/// Monsters farther than this are dropped.
pub const DESPAWN_RADIUS: i64 = 2 * VIEW_RADIUS;
/// The wanderer regains one hit point every this many turns.
pub const REGEN_TURNS: u64 = 6;
/// Turns lost to dying.
pub const DEATH_TURNS: u64 = 1_000;
/// Health one turn of rest at home restores.
pub const REST_HEALTH: i32 = 2;
/// Focus one turn of rest at home restores.
pub const REST_FOCUS: i32 = 3;
/// Stealth lessons each creature can teach, however it is evaded.
const STEALTH_LESSONS: i32 = 12;
/// Lessons a landed opening strike takes from its target.
const OPENING_LESSON: i32 = 3;
/// Lessons breaking a creature's tracking takes from it.
const ESCAPE_LESSON: i32 = 3;
/// Stealth practice per lesson per point of threat.
const STEALTH_PRACTICE: u32 = 2;
/// How far from home a territorial monster will go after the wanderer, in tiles.
pub const TERRITORY: i64 = 4;
/// Chance an idle monster takes a step.
pub const WANDER_CHANCE: f32 = 0.5;
/// Chance each gene of a pool genome varies in an individual drawn from it; see [`individual`].
const INDIVIDUAL_RATE: f32 = 0.5;
/// Size of that variation.
const INDIVIDUAL_SIGMA: f32 = 0.03;
/// Spawn tiles tried per spawn before giving up for the turn.
const SPAWN_TRIES: usize = 6;
/// Regions remembered for comparison, most recent last.
const VISITED_LIMIT: usize = 8;
/// Region lives held before the ones nothing refers to are forgotten. Comfortably above what is always kept
/// (the visited regions, the current one, and one per monster), so forgetting always makes room, and high
/// enough that the regions around the wanderer, where spawns look, stay grown between turns.
const LIVES_LIMIT: usize = 32;

/// Most monsters around the wanderer in a region of this productivity: from 2 in barren land to 12 in jungle.
#[must_use]
pub fn capacity(productivity: f32) -> usize {
    2 + (10.0 * productivity).round() as usize
}

/// Chance per turn that a monster spawns, while under [`capacity`].
#[must_use]
pub fn spawn_chance(productivity: f32) -> f32 {
    0.06 + 0.3 * productivity
}

/// One individual of a species: a genome from its pool with the small variation every spawn gets, so no
/// two monsters of a species are quite alike.
#[must_use]
pub fn individual(pool_member: &Genome, rng: &mut Rng) -> Genome {
    pool_member.mutate(rng, INDIVIDUAL_RATE, INDIVIDUAL_SIGMA)
}

/// How a species differs from its clade's population in a region visited earlier.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    /// The species compared against: the same clade, in the most recently visited other region that has it.
    pub against: SpeciesId,
    /// Gene by gene, this species minus that one.
    pub deltas: Vec<TraitDelta>,
}

/// The immutable consequences of an area action before any actors are removed.
struct AreaPlan {
    preview: AreaPreview,
    hits: Vec<(usize, i32)>,
}

/// One game in progress: a world, the wanderer in it, the creatures around it, the turn count, and what
/// has been said.
#[derive(Clone, Debug)]
pub struct Game {
    world: World,
    /// Memoizes region lookups, which run on every step. Not part of the game's state.
    regions: RegionCache,
    /// The world's clades, a function of the seed. Not part of the game's state.
    clades: Vec<Clade>,
    /// The life of regions looked at lately, a function of the seed and the region for now, so any of it
    /// can be grown again. Holds at most [`LIVES_LIMIT`] regions beyond those still referred to. Not part of
    /// the game's state yet; once regions grow from their neighbors, it will be.
    lives: HashMap<RegionId, RegionLife>,
    position: (i64, i64),
    /// Where the wanderer wakes after dying.
    home: (i64, i64),
    /// The region under the wanderer.
    region: Region,
    /// Regions entered, most recent (the current one) last, without repeats.
    visited: Vec<RegionId>,
    turn: u64,
    log: Log,
    wanderer: Wanderer,
    monsters: Vec<Monster>,
    next_monster: u64,
    deaths: u32,
    maneuver: Maneuver,
    sneaking: bool,
    /// Last resolved attempt, for the prototype's explanation panel. Not advancement credit.
    last_attack: Option<AttackResult>,
    /// Sneaking moves made so far, which sets the rhythm of free moves for practiced stealth.
    sneak_moves: u32,
    /// Whether this turn's action was a sneaking move. Cleared when the turn ends.
    crept: bool,
}

/// Games are equal when their state is; what the caches happen to hold doesn't count.
impl PartialEq for Game {
    fn eq(&self, other: &Game) -> bool {
        (self.world, self.position, self.home, self.turn, self.wanderer, self.next_monster, self.deaths)
            == (other.world, other.position, other.home, other.turn, other.wanderer, other.next_monster, other.deaths)
            && (&self.region, &self.visited, &self.log, &self.monsters)
                == (&other.region, &other.visited, &other.log, &other.monsters)
            && (&self.maneuver, self.sneaking, self.last_attack, self.sneak_moves)
                == (&other.maneuver, other.sneaking, other.last_attack, other.sneak_moves)
    }
}

impl Game {
    /// A new game in the world for `seed`, with the wanderer on the land tile nearest the origin and a few
    /// monsters already about, out of view.
    ///
    /// # Panics
    ///
    /// If [`find_land`] finds no land near the origin, which no seed tried so far comes close to.
    #[must_use]
    pub fn new(seed: u64) -> Game {
        let world = World::new(seed);
        let position = find_land(&world, 0, 0).expect("no land near the origin");
        let mut regions = RegionCache::new(world);
        let region = region_of(&mut regions, position);
        let mut log = Log::default();
        log.push(LogKind::Region, format!("You wake in {}.", region.name));
        let mut game = Game {
            world,
            regions,
            clades: clades(seed),
            lives: HashMap::new(),
            position,
            home: position,
            visited: vec![region.id],
            region,
            turn: 0,
            log,
            wanderer: Wanderer::new(),
            monsters: Vec::new(),
            next_monster: 0,
            deaths: 0,
            maneuver: Maneuver::default(),
            sneaking: false,
            last_attack: None,
            sneak_moves: 0,
            crept: false,
        };
        game.populate(&mut Rng::keyed(seed, b"populate", &[]));
        game
    }

    /// A clearly labeled developer trial: the normal world and action loop, with a known loadout and a
    /// durable adjacent sentinel. It starts sneaking so the first eastward technique can test the opening.
    ///
    /// This does not simulate earned mastery or a naturally generated encounter.
    pub fn composition_trial(seed: u64, maneuver: Maneuver) -> Result<Game, &'static str> {
        maneuver.validate()?;
        let mut game = Self::new(seed);
        let start = game.position;
        let position = (0..80)
            .flat_map(|r| ring(r).map(move |(dx, dy)| (start.0 + dx, start.1 + dy)))
            .find(|&(x, y)| {
                (-2..=2).all(|dy| (-2..=2).all(|dx| is_land(game.world.tile(x + dx, y + dy).biome.biome())))
            })
            .ok_or("No open ground near this seed's starting point.")?;
        game.position = position;
        game.home = position;
        game.update_region(false);
        game.monsters.clear();
        game.prepare(maneuver)?;
        let species = game.local_life().species[0].clone();
        let mut genome = species.pool[0];
        genome.set(crate::Gene::Water, 0.2);
        genome.set(crate::Gene::Aggression, 1.0);
        let mut phenotype = Phenotype::of(&genome);
        phenotype.stats = crate::Stats { hp: 40, attack: 2, defense: 3, speed: ACTION_COST };
        phenotype.sense = 4;
        let at = (position.0 + 1, position.1);
        let sprite = Sprite::draw(&phenotype, &game.clade(species.id.clade).plan, species.biome);
        game.monsters.push(Monster {
            id: MonsterId(game.next_monster),
            species: species.id,
            name: "trial sentinel".into(),
            genome,
            phenotype,
            sprite,
            position: at,
            hp: 40,
            practice_left: 40,
            stealth_left: STEALTH_LESSONS,
            tracking: false,
            home: at,
            alerted: false,
            energy: 0,
        });
        game.next_monster += 1;
        game.sneaking = true;
        game.log.push(LogKind::Region, "COMPOSITION TRIAL: Shift+east uses the technique; bump east strikes for free.");
        Ok(game)
    }

    /// A developer trial of costly area fire: several nearby creatures with different heat tolerances.
    pub fn inferno_trial(seed: u64) -> Result<Game, &'static str> {
        let mut game = Self::composition_trial(seed, Maneuver::staff_inferno())?;
        game.sneaking = false;
        let (x, y) = game.position;
        game.monsters[0].hp = 6;
        game.monsters[0].phenotype.stats.hp = 6;
        game.monsters[0].practice_left = 6;
        game.monsters[0].phenotype.heat = 0.0;
        game.monsters[0].name = "tender trial sentinel".into();
        for (at, heat, name) in
            [((x - 2, y), 0.4, "warm-adapted trial sentinel"), ((x, y + 2), 0.9, "heat-hardy trial sentinel")]
        {
            let mut creature = game.monsters[0].clone();
            creature.id = MonsterId(game.next_monster);
            game.next_monster += 1;
            creature.position = at;
            creature.home = at;
            creature.phenotype.heat = heat;
            creature.name = name.into();
            game.monsters.push(creature);
        }
        // The fixture edits phenotypes after construction; keep the derived pictures in sync.
        for monster in &mut game.monsters {
            monster.sprite = Sprite::draw(
                &monster.phenotype,
                &game.clades[usize::from(monster.species.clade.0)].plan,
                game.lives[&monster.species.region].biome,
            );
        }
        game.log
            .push(LogKind::Region, "INFERNO TRIAL: F strikes the staff. The fire costs 18 Focus and also burns you.");
        Ok(game)
    }

    /// The world being walked.
    #[must_use]
    pub const fn world(&self) -> &World {
        &self.world
    }

    /// The wanderer's tile coordinates.
    #[must_use]
    pub const fn position(&self) -> (i64, i64) {
        self.position
    }

    /// Where the wanderer wakes after dying.
    #[must_use]
    pub const fn home(&self) -> (i64, i64) {
        self.home
    }

    /// The tile under the wanderer.
    #[must_use]
    pub fn tile(&self) -> Tile {
        self.world.tile(self.position.0, self.position.1)
    }

    /// The biome under the wanderer.
    #[must_use]
    pub fn biome(&self) -> &'static Biome {
        self.tile().biome.biome()
    }

    /// The region the wanderer is in.
    #[must_use]
    pub const fn region(&self) -> &Region {
        &self.region
    }

    /// The region entered before the current one, if any.
    #[must_use]
    pub fn previous_region(&self) -> Option<RegionId> {
        self.visited.iter().rev().nth(1).copied()
    }

    /// Turns passed, including those lost to dying.
    #[must_use]
    pub const fn turn(&self) -> u64 {
        self.turn
    }

    /// The message log.
    #[must_use]
    pub const fn log(&self) -> &Log {
        &self.log
    }

    /// The wanderer's hit points and combat numbers.
    #[must_use]
    pub const fn wanderer(&self) -> &Wanderer {
        &self.wanderer
    }

    /// The prepared executable structure and its cosmetic name.
    #[must_use]
    pub const fn maneuver(&self) -> &Maneuver {
        &self.maneuver
    }

    /// Whether careful movement is active, not a guarantee that every observer is unaware.
    #[must_use]
    pub const fn sneaking(&self) -> bool {
        self.sneaking
    }

    /// The actual effects of the most recent attack attempt.
    #[must_use]
    pub const fn last_attack(&self) -> Option<AttackResult> {
        self.last_attack
    }

    /// Preparation is safe only when no creature is in the visible neighborhood.
    #[must_use]
    pub fn can_prepare(&self) -> bool {
        self.monsters_in_view().next().is_none()
    }

    /// Earned developments that still fit. Looking at or deferring offers changes nothing.
    pub fn adaptation_offers(&self) -> impl Iterator<Item = Adaptation> + '_ {
        Adaptation::ALL.into_iter().filter(|&a| a.earned(&self.wanderer) && self.wanderer.adaptations.fits(a))
    }

    /// Make one irreversible commitment without passing time. Rejection changes nothing.
    pub fn commit_adaptation(&mut self, adaptation: Adaptation) -> Result<(), &'static str> {
        if !self.can_prepare() {
            return Err("Commit away from nearby creatures.");
        }
        if !self.adaptation_offers().any(|a| a == adaptation) {
            return Err("This adaptation has not been earned or does not fit.");
        }
        self.wanderer.adaptations.add(adaptation);
        self.log.push(LogKind::Attack, format!("Committed to {}. This cannot be undone.", adaptation.name()));
        Ok(())
    }

    /// Validate a composition against both its grammar and this character's commitments.
    pub fn validate_maneuver(&self, maneuver: &Maneuver) -> Result<(), &'static str> {
        maneuver.validate()?;
        if self.wanderer.adaptations.has(Adaptation::ConcentratedChanneling)
            && matches!(maneuver.technique.burst, Some(crate::Burst { shape: crate::Shape::Area(3..), .. }))
        {
            return Err("Concentrated channeling limits ground areas to radius 2.");
        }
        Ok(())
    }

    /// Draft self-damage with this character's commitments, for the workshop safety preview.
    pub fn maneuver_backlash(&self, maneuver: &Maneuver) -> Result<i32, &'static str> {
        self.validate_maneuver(maneuver)?;
        Ok(maneuver.backlash()? + self.burst_bonus())
    }

    fn burst_bonus(&self) -> i32 {
        if self.wanderer.adaptations.has(Adaptation::ConcentratedChanneling) { 2 } else { 0 }
    }

    /// Prepare a validated composition at a safe point. Invalid edits change nothing.
    ///
    /// Preparation and previews pass no time and grant no learning. No AI verdict is required.
    pub fn prepare(&mut self, maneuver: Maneuver) -> Result<(), &'static str> {
        self.validate_maneuver(&maneuver)?;
        if !self.can_prepare() {
            return Err("Prepare away from nearby creatures.");
        }
        self.log.push(
            LogKind::Attack,
            format!("Prepared {} ({} Focus per attempt).", maneuver.name, maneuver.technique.focus_cost()),
        );
        self.maneuver = maneuver;
        Ok(())
    }

    /// Preview the prepared technique against an adjacent target, without advancing time.
    pub fn preview_attack(&self, dir: Dir) -> Result<AttackPreview, &'static str> {
        let (x, y) = self.ahead(dir);
        let target = self.monster_at(x, y).ok_or("No adjacent target in that direction.")?;
        self.maneuver.preview_trained(self.attack_context(target), self.wanderer.training(self.maneuver.weapon))
    }

    /// Preview the free weapon strike used by a bump, regardless of the prepared delivery.
    pub fn preview_strike(&self, dir: Dir) -> Result<AttackPreview, &'static str> {
        let (x, y) = self.ahead(dir);
        let target = self.monster_at(x, y).ok_or("No adjacent target in that direction.")?;
        self.maneuver
            .basic_strike()
            .preview_trained(self.attack_context(target), self.wanderer.training(self.maneuver.weapon))
    }

    /// Deliberately use the prepared technique on an adjacent creature. A ground technique discharges
    /// its whole area, just like [`Game::unleash`]. Rejection spends neither time nor resources.
    pub fn use_technique(&mut self, dir: Dir) -> Step {
        let (x, y) = self.ahead(dir);
        let Some(target) = self.monsters.iter().position(|m| m.position == (x, y)) else {
            self.log.push(LogKind::Blocked, "No adjacent target in that direction.");
            return Step::Blocked;
        };
        if self.maneuver.technique.delivery == Delivery::Ground {
            return self.unleash();
        }
        if self.wanderer.focus < self.maneuver.technique.focus_cost() {
            self.log.push(LogKind::Blocked, "Not enough Focus for the technique. Bump to strike for free.");
            return Step::Blocked;
        }
        self.attack(target, self.maneuver.clone())
    }

    fn attack_context(&self, target: &Monster) -> AttackContext {
        AttackContext {
            attack: self.wanderer.stats.attack,
            defense: target.phenotype.stats.defense,
            unguarded: self.sneaking && !target.detects(self.position, self.concealment()),
            fire_resistance: target.fire_resistance(),
        }
    }

    fn area_plan(&self) -> Result<AreaPlan, &'static str> {
        self.validate_maneuver(&self.maneuver)?;
        let (radius, burst) = self.maneuver.area_payload()?;
        let hits: Vec<_> = self
            .monsters
            .iter()
            .enumerate()
            .filter(|(_, m)| chebyshev(m.position, self.position) <= i64::from(radius))
            .map(|(i, m)| {
                (i, burst.damage(m.phenotype.stats.defense, m.fire_resistance(), false, false) + self.burst_bonus())
            })
            .collect();
        let preview = AreaPreview {
            radius,
            focus: self.maneuver.technique.focus_cost(),
            targets: hits.len(),
            damage: hits.iter().map(|(_, damage)| damage).sum(),
            self_damage: burst.damage(self.wanderer.stats.defense, 0, false, false) + self.burst_bonus(),
        };
        Ok(AreaPlan { preview, hits })
    }

    /// Preview the current area discharge. It hits all nearby creatures and the caster, without a miss
    /// roll or faction filtering. Terrain does not obstruct the wave in this small prototype.
    pub fn preview_area(&self) -> Result<AreaPreview, &'static str> {
        self.area_plan().map(|plan| plan.preview)
    }

    /// Strike the ground and resolve a bounded area discharge. Focus is paid before any effect.
    ///
    /// Self-damage can kill the caster. Empty casts still cost resources; no learning is awarded.
    pub fn unleash(&mut self) -> Step {
        let AreaPlan { preview, hits } = match self.area_plan() {
            Ok(plan) => plan,
            Err(reason) => {
                self.log.push(LogKind::Blocked, reason);
                return Step::Blocked;
            }
        };
        if self.wanderer.focus < preview.focus {
            self.log.push(LogKind::Blocked, "Not enough Focus to discharge this area.");
            return Step::Blocked;
        }
        self.turn += 1;
        self.wanderer.focus -= preview.focus;
        self.sneaking = false;
        self.last_attack = None;
        self.log.push(
            LogKind::Attack,
            format!(
                "{} hits {} creatures for {} total damage; {} Focus spent.",
                self.maneuver.name, preview.targets, preview.damage, preview.focus
            ),
        );
        let mut threat = 0;
        self.wanderer.adaptations.channeled |= !hits.is_empty();
        for (i, damage) in hits {
            if self.monsters[i].spend_practice(damage) > 0 {
                threat = threat.max(self.monsters[i].weapon_threat(&self.wanderer, self.maneuver.weapon));
            }
            let monster = &mut self.monsters[i];
            monster.hp = monster.hp.saturating_sub(damage);
            monster.alerted = true;
            self.log.push(LogKind::Attack, format!("The {} takes {damage}.", monster.name));
            if monster.hp <= 0 {
                self.log.push(LogKind::Kill, format!("The {} dies.", monster.name));
            }
        }
        self.monsters.retain(|monster| monster.hp > 0);
        self.learn_focus(preview.focus, threat);
        self.wanderer.hp -= preview.self_damage;
        if preview.self_damage > 0 {
            self.log
                .push(LogKind::Hurt, format!("The same area hits you for {}. No safe center.", preview.self_damage));
        }
        if self.wanderer.hp <= 0 {
            self.die();
        } else {
            self.end_turn();
        }
        Step::Attacked
    }

    /// Enter or leave careful movement. Entering spends one Focus and a turn; it does not erase tracking.
    pub fn toggle_sneak(&mut self) -> Step {
        if !self.sneaking && self.wanderer.focus < 1 {
            self.log.push(LogKind::Blocked, "Not enough Focus to sneak.");
            return Step::Blocked;
        }
        self.sneaking = !self.sneaking;
        if self.sneaking {
            self.wanderer.focus -= 1;
        }
        self.turn += 1;
        self.log.push(
            LogKind::Attack,
            if self.sneaking {
                "You move carefully. Enemies already tracking you still know where you are."
            } else {
                "You stop sneaking."
            },
        );
        self.end_turn();
        Step::Waited
    }

    /// Whether sheltered rest at home can restore Health or Focus.
    #[must_use]
    pub fn can_rest(&self) -> bool {
        self.position == self.home
            && (self.wanderer.hp < self.wanderer.max_hp() || self.wanderer.focus < self.wanderer.max_focus())
    }

    /// Rest sheltered at home for [`REST_HEALTH`] Health and [`REST_FOCUS`] Focus.
    /// Creatures still move, but cannot hurt the wanderer during this rest action.
    /// Other actions at home are exposed. A full character cannot take a protected rest turn.
    /// Ordinary waiting never restores Focus.
    pub fn recover_focus(&mut self) -> Step {
        if self.position != self.home {
            self.log.push(LogKind::Blocked, "Rest in the shelter at home.");
            return Step::Blocked;
        }
        let w = &mut self.wanderer;
        let most = w.max_focus();
        if w.focus >= most && w.hp >= w.max_hp() {
            self.log.push(LogKind::Blocked, "You are already fully rested.");
            return Step::Blocked;
        }
        self.turn += 1;
        self.sneaking = false;
        w.focus = (w.focus + REST_FOCUS - i32::from(w.adaptations.has(Adaptation::DeepReserves))).min(most);
        w.hp = (w.hp + REST_HEALTH).min(w.max_hp());
        let line = if w.focus == most && w.hp == w.max_hp() {
            "You rest sheltered at home. You are fully recovered.".to_owned()
        } else {
            format!("You rest sheltered at home. Health {}/{}, Focus {}/{most}.", w.hp, w.max_hp(), w.focus)
        };
        self.log.push(LogKind::Attack, line);
        self.finish_turn(true);
        Step::Waited
    }
    /// Times the wanderer has died.
    #[must_use]
    pub const fn deaths(&self) -> u32 {
        self.deaths
    }

    /// The monsters around the wanderer, in order of appearance. Some are out of view; see
    /// [`VIEW_RADIUS`].
    #[must_use]
    pub fn monsters(&self) -> &[Monster] {
        &self.monsters
    }

    /// The monster on tile `(x, y)`, if any.
    #[must_use]
    pub fn monster_at(&self, x: i64, y: i64) -> Option<&Monster> {
        self.monsters.iter().find(|m| m.position == (x, y))
    }

    /// The world's clades.
    #[must_use]
    pub fn clades(&self) -> &[Clade] {
        &self.clades
    }

    /// The clade `id` names.
    ///
    /// # Panics
    ///
    /// If `id` isn't one of this world's clades.
    #[must_use]
    pub fn clade(&self, id: CladeId) -> &Clade {
        &self.clades[usize::from(id.0)]
    }

    /// The life of the region the wanderer is in.
    #[must_use]
    pub fn local_life(&self) -> &RegionLife {
        self.lives.get(&self.region.id).expect("the current region's life is grown on entry")
    }

    /// A species the game has met: any species of the current or a recently visited region, or of a monster
    /// still about.
    #[must_use]
    pub fn species(&self, id: SpeciesId) -> Option<&Species> {
        self.lives.get(&id.region)?.species_of(id.clade)
    }

    /// The monsters within [`VIEW_RADIUS`] of the wanderer, the ones a frontend shows, in order of appearance.
    pub fn monsters_in_view(&self) -> impl Iterator<Item = &Monster> {
        self.monsters.iter().filter(|m| chebyshev(m.position, self.position) <= VIEW_RADIUS)
    }

    /// The species of the monsters in view, nearest first, without repeats.
    #[must_use]
    pub fn nearby_species(&self) -> Vec<SpeciesId> {
        let mut seen: Vec<(i64, SpeciesId)> =
            self.monsters_in_view().map(|m| (manhattan(m.position, self.position), m.species)).collect();
        seen.sort();
        let mut out: Vec<SpeciesId> = Vec::new();
        for (_, s) in seen {
            if !out.contains(&s) {
                out.push(s);
            }
        }
        out
    }

    /// How species `id` differs from its clade in the most recently visited other region where that clade
    /// lives: the gradient the NEARBY panel shows. `None` if no visited region has the clade.
    #[must_use]
    pub fn compare(&self, id: SpeciesId) -> Option<Comparison> {
        let species = self.species(id)?;
        self.visited.iter().rev().filter(|&&r| r != id.region).find_map(|&r| {
            let other = self.lives.get(&r)?.species_of(id.clade)?;
            Some(Comparison { against: other.id, deltas: compare(species, other) })
        })
    }

    /// Tries to step one tile in `dir`. A monster there takes a free basic weapon strike instead.
    /// The prepared technique is unchanged and never discharged by a bump. Otherwise the wanderer moves if
    /// the target is passable, naming the region if that changes, or stays put and logs what is in the way,
    /// unless that is already the last line, so pushing against a wall doesn't flood the log. Attacking or
    /// moving passes a turn, and the monsters act.
    pub fn step(&mut self, dir: Dir) -> Step {
        let (x, y) = self.ahead(dir);
        if !self.can_step(dir) {
            // Biome names are sentence case ("Deep water"); mid-sentence they are common nouns.
            let line = if self.sneaking && self.wanderer.focus < self.sneak_cost() {
                "Not enough Focus. Change your approach or recover at home.".to_owned()
            } else {
                format!("The way is blocked by {}.", self.world.tile(x, y).biome.biome().name.to_lowercase())
            };
            if self.log.lines().next_back() != Some(line.as_str()) {
                self.log.push(LogKind::Blocked, line);
            }
            return Step::Blocked;
        }
        if let Some(target) = self.monsters.iter().position(|m| m.position == (x, y)) {
            return self.attack(target, self.maneuver.basic_strike());
        }
        self.turn += 1;
        self.position = (x, y);
        if self.sneaking {
            self.wanderer.focus -= self.sneak_cost();
            self.sneak_moves = self.sneak_moves.wrapping_add(1);
            self.crept = true;
        }
        self.update_region(true);
        self.end_turn();
        Step::Moved
    }

    /// Whether [`Game::step`] in `dir` would pass a turn: a monster stands there to attack, or the tile can be
    /// entered. False exactly when the step would be [`Step::Blocked`].
    #[must_use]
    pub fn can_step(&self, dir: Dir) -> bool {
        let (x, y) = self.ahead(dir);
        if self.monster_at(x, y).is_some() {
            true
        } else {
            self.world.tile(x, y).biome.biome().passable && (!self.sneaking || self.wanderer.focus >= self.sneak_cost())
        }
    }

    /// Focus the next sneaking move costs: one, except for practiced stealth's periodic free move.
    fn sneak_cost(&self) -> i32 {
        let every = if self.wanderer.adaptations.has(Adaptation::VeiledMovement) {
            Some(2)
        } else {
            self.wanderer.stealth.free_sneak_every()
        };
        let free = every.is_some_and(|n| self.sneak_moves.wrapping_add(1).is_multiple_of(n));
        i32::from(!free)
    }

    /// How many tiles sneaking currently takes off an unalerted observer's initial sensing range.
    fn concealment(&self) -> i64 {
        if self.sneaking {
            self.wanderer.stealth.sneak_reduction()
                + if self.wanderer.adaptations.has(Adaptation::VeiledMovement) { 2 } else { 0 }
        } else {
            0
        }
    }

    /// Log a newly reached rank.
    fn announce(&mut self, skill: &str, before: Rank, rank: Rank, action: crate::LearnedMove) {
        self.log.push(LogKind::Learning, format!("Your {skill} is now {}.", rank.name()));
        if before == Rank::Novice {
            self.log
                .push(LogKind::Learning, format!("Learned {}. Tab selects; Alt + direction uses it.", action.name()));
        }
    }

    /// Credit Focus spent on a technique that did real damage to a creature of this threat.
    fn learn_focus(&mut self, spent: i32, threat: u32) {
        let before = self.wanderer.focus_skill.rank();
        let evidence = u32::try_from(spent.max(0)).unwrap_or(0).saturating_mul(threat);
        if self.wanderer.focus_skill.learn(evidence, Rank::ceiling(threat)) {
            self.announce("Focus", before, self.wanderer.focus_skill.rank(), crate::LearnedMove::Pulse);
        }
    }

    /// Spend up to `units` of monster `i`'s stealth lessons, scaled by its threat. Returns whether the
    /// previous rank if it rose; the caller logs it after the action's own messages.
    fn stealth_lesson(&mut self, i: usize, units: i32) -> Option<Rank> {
        let threat = self.monsters[i].weapon_threat(&self.wanderer, self.maneuver.weapon);
        let ceiling = Rank::ceiling(threat);
        if ceiling == Rank::Novice {
            return None;
        }
        let spent = self.monsters[i].spend_stealth(units);
        let before = self.wanderer.stealth.rank();
        self.wanderer
            .stealth
            .learn(spent.saturating_mul(threat).saturating_mul(STEALTH_PRACTICE), ceiling)
            .then_some(before)
    }

    /// The tile one step from the wanderer in `dir`.
    const fn ahead(&self, dir: Dir) -> (i64, i64) {
        let (dx, dy) = dir.delta();
        (self.position.0 + dx, self.position.1 + dy)
    }

    /// Stays put for a turn, letting the monsters act.
    pub fn wait(&mut self) -> Step {
        self.turn += 1;
        self.end_turn();
        Step::Waited
    }

    /// Everything after the wanderer's action: monsters act, the wanderer heals, monsters come and go.
    fn end_turn(&mut self) {
        self.finish_turn(false);
    }

    fn finish_turn(&mut self, sheltered: bool) {
        let deaths = self.deaths;
        let crept = std::mem::take(&mut self.crept);
        self.monsters_act(crept, sheltered);
        if self.deaths != deaths {
            return;
        }
        if self.turn.is_multiple_of(REGEN_TURNS) && self.wanderer.hp < self.wanderer.max_hp() {
            self.wanderer.hp += 1;
        }
        let here = self.position;
        self.monsters.retain(|m| chebyshev(m.position, here) <= DESPAWN_RADIUS);
        let productivity = self.local_life().productivity();
        let mut rng = Rng::keyed(self.world.seed(), b"spawning", &[self.turn]);
        if self.monsters.len() < capacity(productivity) && rng.chance(spawn_chance(productivity)) {
            self.spawn(&mut rng);
        }
    }

    /// Notes the region under the wanderer, growing its life if it is new. With `announce`, a change of
    /// region is logged.
    fn update_region(&mut self, announce: bool) {
        let (x, y) = self.position;
        if self.regions.region_at(x, y) == self.region.id {
            return;
        }
        self.region = region_of(&mut self.regions, self.position);
        self.life(self.region.id);
        self.visited.retain(|&r| r != self.region.id);
        self.visited.push(self.region.id);
        if self.visited.len() > VISITED_LIMIT {
            self.visited.remove(0);
        }
        if announce {
            self.log.push(LogKind::Region, format!("You enter {}.", self.region.name));
        }
    }

    /// The life of region `id`, grown now if it isn't held. Growing one into a full cache first forgets
    /// what nothing refers to.
    fn life(&mut self, id: RegionId) -> &RegionLife {
        if !self.lives.contains_key(&id) {
            if self.lives.len() >= LIVES_LIMIT {
                self.forget_lives();
            }
            let region = self.regions.region(id).expect("ids come from region_at");
            let life = RegionLife::grow(self.world.seed(), &self.clades, &region);
            self.lives.insert(id, life);
        }
        &self.lives[&id]
    }

    /// Drops every region life except those the game still looks up: the current region's, the visited
    /// regions' (for comparisons), and each monster's species' region.
    fn forget_lives(&mut self) {
        let mut keep: HashSet<RegionId> = self.visited.iter().copied().collect();
        keep.insert(self.region.id);
        keep.extend(self.monsters.iter().map(|m| m.species.region));
        self.lives.retain(|id, _| keep.contains(id));
    }

    /// Fills the area around a new game to half its capacity, out of view.
    fn populate(&mut self, rng: &mut Rng) {
        self.life(self.region.id);
        let target = capacity(self.local_life().productivity()) / 2;
        for _ in 0..target * 2 {
            if self.monsters.len() >= target {
                break;
            }
            self.spawn(rng);
        }
    }

    /// Tries to place one monster on a random tile between [`SPAWN_MIN`] and [`SPAWN_MAX`] from the
    /// wanderer, drawn from the life of that tile's region. Gives up quietly after a few unsuitable tiles.
    fn spawn(&mut self, rng: &mut Rng) {
        for _ in 0..SPAWN_TRIES {
            let r = SPAWN_MIN + rng.below((SPAWN_MAX - SPAWN_MIN + 1) as usize) as i64;
            let (dx, dy) = ring_point(r, rng.below(8 * r as usize) as i64);
            let (x, y) = (self.position.0 + dx, self.position.1 + dy);
            if self.monster_at(x, y).is_some() {
                continue;
            }
            let id = self.regions.region_at(x, y);
            self.life(id);
            let life = &self.lives[&id];
            if life.species.is_empty() {
                continue;
            }
            let weights: Vec<f32> = life.species.iter().map(|s| s.abundance).collect();
            let species = &life.species[rng.weighted(&weights)];
            let genome = individual(&species.pool[rng.below(species.pool.len())], rng);
            let phenotype = Phenotype::of(&genome);
            let tile = self.world.tile(x, y).biome;
            if !phenotype.access.allows(tile.biome()) {
                continue;
            }
            let plan = &self.clades[usize::from(species.id.clade.0)].plan;
            let monster = Monster {
                id: MonsterId(self.next_monster),
                species: species.id,
                name: species.name.clone(),
                genome,
                phenotype,
                sprite: Sprite::draw(&phenotype, plan, species.biome),
                position: (x, y),
                hp: phenotype.stats.hp,
                practice_left: phenotype.stats.hp,
                stealth_left: STEALTH_LESSONS,
                tracking: false,
                home: (x, y),
                alerted: false,
                energy: rng.below(ACTION_COST as usize) as i32,
            };
            self.next_monster += 1;
            self.monsters.push(monster);
            return;
        }
    }

    fn attack(&mut self, index: usize, maneuver: Maneuver) -> Step {
        self.turn += 1;
        let mut rng = Rng::keyed(self.world.seed(), b"wanderer", &[self.turn]);
        self.wanderer_attacks(index, &maneuver, &mut rng);
        self.end_turn();
        Step::Attacked
    }

    /// The wanderer attacks monster `index` using the selected action, not necessarily the preparation.
    fn wanderer_attacks(&mut self, index: usize, maneuver: &Maneuver, rng: &mut Rng) {
        let context = self.attack_context(&self.monsters[index]);
        let training = self.wanderer.training(maneuver.weapon);
        let result = maneuver.resolve_trained(context, training, rng).expect("attack maneuvers are validated");
        self.wanderer.focus -= maneuver.technique.focus_cost();
        self.record_attack(index, result, AttackSource::Maneuver(maneuver), maneuver.technique.focus_cost());
    }

    /// Apply a pre-resolved strike with the same finite damage allowance for every delivery.
    /// Returns credited threat, so a multi-target action can teach Focus once, not once per target.
    fn record_attack(&mut self, index: usize, result: AttackResult, source: AttackSource<'_>, focus_paid: i32) -> u32 {
        let weapon = source.weapon();
        let before = self.wanderer.weapons.get(weapon).rank();
        let threat = self.monsters[index].weapon_threat(&self.wanderer, weapon);
        let opportunity = self.monsters[index].spend_practice(result.total());
        let physical = opportunity.min(result.physical.max(0) as u32);
        let ceiling = self.wanderer.adaptations.weapon_ceiling(weapon);
        let improved = source.is_weapon()
            && if ceiling == Rank::Expert {
                self.wanderer.weapons.learn(weapon, physical, threat)
            } else {
                self.wanderer.weapons.learn_capped(weapon, physical, threat, ceiling)
            };
        self.wanderer.adaptations.channeled |= result.burst_released;
        let stealthier = if result.opening { self.stealth_lesson(index, OPENING_LESSON) } else { None };
        self.last_attack = Some(result);
        // Even a miss reveals the attempt. No repeated surprise by toggling sneak.
        self.sneaking = false;
        let monster = &mut self.monsters[index];
        monster.alerted = true;
        if !result.hit {
            self.log.push(LogKind::Attack, format!("You miss the {}.", monster.name));
        } else {
            monster.hp = monster.hp.saturating_sub(result.total());
            self.log.push(LogKind::Attack, format!("You hit the {} for {}.", monster.name, result.total()));
            if source.has_payload() || result.opening {
                let detail = if result.amplified {
                    "unguarded placement amplifies the burst"
                } else if source.has_payload() && !result.burst_released {
                    "no penetration; the burst does not release"
                } else {
                    "no amplified placement"
                };
                self.log.push(
                    LogKind::Attack,
                    format!("{}: {} physical + {} burst; {detail}.", source.name(), result.physical, result.burst),
                );
            }
            if monster.hp <= 0 {
                self.log.push(LogKind::Kill, format!("The {} dies.", monster.name));
                self.monsters.remove(index);
            }
        }
        if improved {
            let skill = format!("{} skill", weapon.name().to_lowercase());
            self.announce(
                &skill,
                before,
                self.wanderer.weapons.get(weapon).rank(),
                crate::LearnedMove::for_weapon(weapon),
            );
        }
        self.learn_focus(if opportunity > 0 { focus_paid } else { 0 }, threat);
        if let Some(before) = stealthier {
            self.announce("stealth", before, self.wanderer.stealth.rank(), crate::LearnedMove::SlipAway);
        }
        if opportunity > 0 { threat } else { 0 }
    }

    /// Every monster acts, in order of appearance, as often as its speed allows, each drawing from its own
    /// stream for the turn. Stops if the wanderer dies.
    fn monsters_act(&mut self, crept: bool, sheltered: bool) {
        let deaths = self.deaths;
        let concealment = self.concealment();
        for i in 0..self.monsters.len() {
            // Observation belongs to the turn, not to a speed-dependent opportunity to act.
            let now = self.monsters[i].detects(self.position, concealment);
            // Escaping counts only from tracking seen at the last turn's observation, so being
            // revealed by an attack and immediately out of range is not an escape.
            let escaped = self.monsters[i].tracking && !now;
            self.monsters[i].alerted = now;
            self.monsters[i].tracking = now;
            let lesson = if escaped {
                ESCAPE_LESSON
            } else if crept && !now && self.monsters[i].detects(self.position, 0) {
                // Unseen inside the range that would have found an ordinary walker.
                1
            } else {
                0
            };
            if lesson > 0
                && let Some(before) = self.stealth_lesson(i, lesson)
            {
                self.announce("stealth", before, self.wanderer.stealth.rank(), crate::LearnedMove::SlipAway);
            }
            let mut rng = Rng::keyed(self.world.seed(), b"monsters", &[self.turn, self.monsters[i].id.0]);
            self.monsters[i].energy += self.monsters[i].phenotype.stats.speed;
            let mut actions = 0;
            while self.monsters[i].energy >= ACTION_COST && actions < MAX_ACTIONS {
                self.monsters[i].energy -= ACTION_COST;
                actions += 1;
                self.monster_acts(i, &mut rng, sheltered);
                if self.deaths != deaths {
                    return;
                }
            }
        }
    }

    /// One action of monster `i`, by temperament: aggressive ones close in and attack, territorial ones
    /// attack near home, skittish ones flee, and all of them wander when nothing has their attention.
    fn monster_acts(&mut self, i: usize, rng: &mut Rng, sheltered: bool) {
        self.monsters[i].alerted = self.monsters[i].detects(self.position, self.concealment());
        let m = &self.monsters[i];
        let (me, you, home) = (m.position, self.position, m.home);
        let adjacent = manhattan(me, you) == 1;
        let senses = m.alerted;
        match m.phenotype.temperament {
            Temperament::Aggressive => {
                if adjacent && senses {
                    if !sheltered {
                        self.monster_attacks(i, rng);
                    }
                } else if senses {
                    self.approach(i, you);
                } else {
                    self.wander(i, rng);
                }
            }
            Temperament::Territorial => {
                let trespassing = senses && chebyshev(you, home) <= TERRITORY;
                if adjacent && trespassing {
                    if !sheltered {
                        self.monster_attacks(i, rng);
                    }
                } else if trespassing {
                    self.approach(i, you);
                } else if chebyshev(me, home) > TERRITORY {
                    self.approach(i, home);
                } else {
                    self.wander(i, rng);
                }
            }
            Temperament::Skittish => {
                if senses {
                    if !self.flee(i) && adjacent && !sheltered {
                        self.monster_attacks(i, rng);
                    }
                } else {
                    self.wander(i, rng);
                }
            }
        }
    }

    fn monster_attacks(&mut self, i: usize, rng: &mut Rng) {
        let m = &self.monsters[i];
        let verb = self.clades[usize::from(m.species.clade.0)].plan.family.verb();
        match damage(m.phenotype.stats.attack, self.wanderer.stats.defense, rng) {
            None => self.log.push(LogKind::Missed, format!("The {} misses you.", m.name)),
            Some(d) => {
                self.wanderer.hp -= d;
                self.log.push(LogKind::Hurt, format!("The {} {verb} you for {d}.", m.name));
                if self.wanderer.hp <= 0 {
                    self.die();
                }
            }
        }
    }

    /// The wanderer dies: time jumps, and they wake at home with 1 Health and no Focus.
    ///
    /// Dying refills nothing, so it never beats retreating. Creatures survive and recover while the
    /// wanderer is away. Those within view of home are driven off, so waking weak is not a second fight.
    fn die(&mut self) {
        self.log.push(LogKind::Death, "You fall.");
        self.deaths += 1;
        self.turn += DEATH_TURNS;
        self.position = self.home;
        self.wanderer.hp = 1;
        self.wanderer.focus = 0;
        self.sneaking = false;
        self.last_attack = None;
        let home = self.home;
        self.monsters.retain(|m| (VIEW_RADIUS + 1..=DESPAWN_RADIUS).contains(&chebyshev(m.position, home)));
        for monster in &mut self.monsters {
            monster.hp = monster.max_hp();
            monster.alerted = false;
            monster.tracking = false;
        }
        self.update_region(false);
        self.log.push(
            LogKind::Death,
            format!("You wake, much later, in {}. Weak and drained, rest (R).", self.region.name),
        );
    }
    /// Whether monster `i` may stand on `(x, y)`: not the wanderer's tile, not another monster's, and terrain
    /// its body allows.
    fn open_to(&self, i: usize, (x, y): (i64, i64)) -> bool {
        (x, y) != self.position
            && !self.monsters.iter().enumerate().any(|(j, m)| j != i && m.position == (x, y))
            && self.monsters[i].phenotype.access.allows(self.world.tile(x, y).biome.biome())
    }

    /// Moves monster `i` one step to the open neighbor with the lowest `score`, if that is lower than the
    /// current tile's. Ties go to the smallest position. Returns whether it moved.
    fn step_by(&mut self, i: usize, score: impl Fn((i64, i64)) -> i64) -> bool {
        let here = self.monsters[i].position;
        let best = Dir::ALL
            .iter()
            .map(|d| (here.0 + d.delta().0, here.1 + d.delta().1))
            .filter(|&p| self.open_to(i, p))
            .map(|p| (score(p), p))
            .min();
        match best {
            Some((s, p)) if s < score(here) => {
                self.monsters[i].position = p;
                true
            }
            _ => false,
        }
    }

    /// Monster `i` steps toward `goal`.
    fn approach(&mut self, i: usize, goal: (i64, i64)) -> bool {
        self.step_by(i, |p| manhattan(p, goal))
    }

    /// Monster `i` steps away from the wanderer. Returns whether it could.
    fn flee(&mut self, i: usize) -> bool {
        let you = self.position;
        self.step_by(i, |p| -manhattan(p, you))
    }

    /// Monster `i` maybe takes a random step.
    fn wander(&mut self, i: usize, rng: &mut Rng) {
        if rng.chance(WANDER_CHANCE) {
            let (dx, dy) = Dir::ALL[rng.below(4)].delta();
            let here = self.monsters[i].position;
            let to = (here.0 + dx, here.1 + dy);
            if self.open_to(i, to) {
                self.monsters[i].position = to;
            }
        }
    }
}

/// The region holding tile `(x, y)`, described.
fn region_of(regions: &mut RegionCache, (x, y): (i64, i64)) -> Region {
    let id = regions.region_at(x, y);
    regions.region(id).expect("region_at only returns real regions")
}

/// The `k`th tile, for `k` in `0..8r`, of the ring at Chebyshev distance `r` from the origin.
fn ring_point(r: i64, k: i64) -> (i64, i64) {
    let side = 2 * r;
    let (edge, t) = (k / side, k % side);
    match edge {
        0 => (-r + t, -r),
        1 => (r, -r + t),
        2 => (r - t, r),
        _ => (-r, r - t),
    }
}

fn chebyshev(a: (i64, i64), b: (i64, i64)) -> i64 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

fn manhattan(a: (i64, i64), b: (i64, i64)) -> i64 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_FOCUS;

    /// Seeds to check: 1, 2, 3, and 7 start in deep water and 5 in shallows; the rest start on land.
    const SEEDS: [u64; 10] = [0, 1, 2, 3, 4, 5, 7, 42, 1_234, u64::MAX];

    fn biome_at(world: &World, (x, y): (i64, i64)) -> &'static Biome {
        world.tile(x, y).biome.biome()
    }

    #[test]
    fn ring_covers_the_square_border_once() {
        for r in 0..6 {
            let tiles: Vec<_> = ring(r).collect();
            let unique: std::collections::HashSet<_> = tiles.iter().copied().collect();
            assert_eq!(unique.len(), tiles.len(), "ring {r} repeats a tile");
            assert_eq!(tiles.len() as i64, if r == 0 { 1 } else { 8 * r });
            assert!(tiles.iter().all(|&(dx, dy)| dx.abs().max(dy.abs()) == r));
        }
    }

    #[test]
    fn some_seeds_start_at_sea() {
        // Keeps the spawn tests honest: they must cover origins that aren't land.
        for seed in [1, 2, 3, 5, 7] {
            let world = World::new(seed);
            assert!(!is_land(biome_at(&world, (0, 0))), "seed {seed} origin is land now; pick another sea seed");
        }
    }

    #[test]
    fn spawn_is_on_land() {
        for seed in SEEDS {
            let game = Game::new(seed);
            let biome = game.biome();
            assert!(biome.passable && !biome.water, "seed {seed} spawns on {}", biome.name);
        }
    }

    #[test]
    fn find_land_is_nearest() {
        for seed in SEEDS {
            let world = World::new(seed);
            let (x, y) = find_land(&world, 0, 0).unwrap();
            let d2 = x * x + y * y;
            let r = d2.isqrt() + 1;
            for ty in -r..=r {
                for tx in -r..=r {
                    if tx * tx + ty * ty < d2 {
                        assert!(
                            !is_land(biome_at(&world, (tx, ty))),
                            "seed {seed}: ({tx}, {ty}) is closer than ({x}, {y})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn find_land_keeps_a_land_start() {
        let world = World::new(0);
        let (x, y) = find_land(&world, 0, 0).unwrap();
        assert_eq!(find_land(&world, x, y), Some((x, y)));
    }

    /// A game for `seed` with the wanderer next to impassable terrain, and the direction it lies in.
    fn facing_a_wall(seed: u64) -> (Game, Dir) {
        let mut game = Game::new(seed);
        // Walk straight until something blocks; a straight line leaves any continent for deep water.
        for dir in Dir::ALL {
            for _ in 0..10_000 {
                if game.step(dir) == Step::Blocked {
                    return (game, dir);
                }
            }
        }
        panic!("seed {seed}: walked every way without meeting a wall");
    }

    #[test]
    fn blocked_moves_change_nothing_but_the_log() {
        for seed in [0, 1, 5] {
            let (mut game, dir) = facing_a_wall(seed);
            // Make sure the last line isn't already this wall's, so the first bump is logged.
            game.log.push(LogKind::Region, "marker");
            let (position, turn, before) = (game.position(), game.turn(), game.log().total());
            assert_eq!(game.step(dir), Step::Blocked);
            assert_eq!((game.position(), game.turn()), (position, turn));
            let added = new_lines(&game, before);
            assert_eq!(added.len(), 1, "seed {seed}: {added:?}");
            assert_eq!(added[0].kind, LogKind::Blocked, "seed {seed}: {added:?}");
            assert!(added[0].text.starts_with("The way is blocked by "), "seed {seed}: {added:?}");
            // Pushing on against the same wall says nothing new.
            for _ in 0..LOG_LIMIT + 5 {
                assert_eq!(game.step(dir), Step::Blocked);
            }
            assert_eq!(game.log().total(), before + 1, "seed {seed}");
            assert_eq!(game.log().last(2).next().map(|e| e.text.as_str()), Some("marker"), "seed {seed}");
        }
    }

    #[test]
    fn actions_pass_turns() {
        let mut game = Game::new(4);
        let mut passed = 0;
        for dir in [Dir::East, Dir::South, Dir::West, Dir::North] {
            if game.step(dir) != Step::Blocked {
                passed += 1;
            }
        }
        assert_eq!(game.wait(), Step::Waited);
        assert_eq!(game.turn(), passed + 1 + DEATH_TURNS * u64::from(game.deaths()));
    }

    #[test]
    fn wake_names_the_region() {
        for seed in [0, 5] {
            let game = Game::new(seed);
            let (x, y) = game.position();
            assert_eq!(game.region().id, game.world().region_at(x, y));
            assert_eq!(game.log().lines().collect::<Vec<_>>(), [format!("You wake in {}.", game.region().name)]);
        }
    }

    /// The entries the last action added to the log.
    fn new_lines(game: &Game, before: u64) -> Vec<LogEntry> {
        let added = (game.log().total() - before) as usize;
        game.log().last(added).cloned().collect()
    }

    #[test]
    fn region_messages_fire_only_on_change() {
        for seed in [0, 1, 5] {
            let mut game = Game::new(seed);
            let mut changes = 0;
            for i in 0..1_500 {
                let (from, before, deaths) = (game.region().id, game.log().total(), game.deaths());
                let dir = Dir::ALL[(i / 40) % 4];
                if game.step(dir) == Step::Moved && game.deaths() == deaths {
                    let after = game.region().id;
                    let entered: Vec<_> = new_lines(&game, before)
                        .into_iter()
                        .filter(|e| e.kind == LogKind::Region)
                        .map(|e| e.text)
                        .collect();
                    assert_eq!(!entered.is_empty(), after != from, "seed {seed} step {i}");
                    if after != from {
                        changes += 1;
                        assert_eq!(entered, [format!("You enter {}.", game.region().name)]);
                        assert_eq!(game.previous_region(), Some(from));
                    }
                }
            }
            assert!(changes > 0, "seed {seed}: never changed region, so the test proved nothing");
            // The cached region the game tracks is the one the world computes from scratch.
            let (x, y) = game.position();
            assert_eq!(game.region().id, game.world().region_at(x, y), "seed {seed}");
        }
    }

    #[test]
    fn log_keeps_the_last_lines() {
        let mut log = Log::default();
        for i in 0..LOG_LIMIT + 7 {
            log.push(LogKind::Region, i.to_string());
        }
        assert_eq!(log.len(), LOG_LIMIT);
        assert_eq!(log.lines().next(), Some("7"));
        let tail: Vec<_> = log.last(3).map(|e| e.text.as_str()).collect();
        assert_eq!(tail, [(LOG_LIMIT + 4).to_string(), (LOG_LIMIT + 5).to_string(), (LOG_LIMIT + 6).to_string()]);
        assert_eq!(log.last(1_000).count(), LOG_LIMIT);
        assert_eq!(log.total(), (LOG_LIMIT + 7) as u64);
    }

    /// A fixed mix of steps and waits.
    fn play(game: &mut Game, actions: usize) {
        for i in 0..actions {
            if i % 9 == 4 {
                game.wait();
            } else {
                game.step(Dir::ALL[(i * 7 + i / 13) % 4]);
            }
        }
    }

    #[test]
    fn same_seed_same_game() {
        let route = |seed| {
            let mut game = Game::new(seed);
            play(&mut game, 400);
            game
        };
        for seed in [0, 4, 42] {
            let game = route(seed);
            assert!(game.next_monster > 0, "seed {seed}: nothing spawned, so the replay proved little");
            assert_eq!(game, route(seed));
        }
    }

    /// A monster of the local region's first species, placed at `at`, with its aggression set.
    fn put_monster(game: &mut Game, at: (i64, i64), aggression: f32) -> MonsterId {
        put_species(game, 0, at, aggression)
    }

    /// [`put_monster`], of the local region's `which`th species.
    fn put_species(game: &mut Game, which: usize, at: (i64, i64), aggression: f32) -> MonsterId {
        let species = game.local_life().species[which].clone();
        let mut genome = species.pool[0];
        genome.set(crate::Gene::Aggression, aggression);
        // Land-bound and middling, so it can stand next to a land spawn and survives a hit or two.
        genome.set(crate::Gene::Water, 0.2);
        genome.set(crate::Gene::Size, 0.6);
        let phenotype = Phenotype::of(&genome);
        let id = MonsterId(game.next_monster);
        game.next_monster += 1;
        let plan = &game.clade(species.id.clade).plan;
        game.monsters.push(Monster {
            id,
            species: species.id,
            name: species.name.clone(),
            genome,
            phenotype,
            sprite: Sprite::draw(&phenotype, plan, species.biome),
            position: at,
            hp: phenotype.stats.hp,
            practice_left: phenotype.stats.hp,
            stealth_left: STEALTH_LESSONS,
            tracking: false,
            home: at,
            alerted: false,
            energy: 0,
        });
        id
    }

    /// A game for `seed` with every monster removed and an open land tile east of the wanderer.
    fn cleared(seed: u64) -> (Game, (i64, i64)) {
        let mut game = Game::new(seed);
        game.monsters.clear();
        for _ in 0..200 {
            let (x, y) = game.position();
            if is_land(biome_at(game.world(), (x + 1, y))) {
                return (game, (x + 1, y));
            }
            game.step(Dir::ALL[(game.turn() as usize / 7) % 4]);
            game.monsters.clear();
        }
        panic!("seed {seed}: no land beside the wanderer");
    }

    #[test]
    fn bumping_attacks_instead_of_moving() {
        let (mut game, east) = cleared(0);
        let id = put_monster(&mut game, east, 0.5);
        let (position, turn, before) = (game.position(), game.turn(), game.log().total());
        assert_eq!(game.step(Dir::East), Step::Attacked);
        assert_eq!((game.position(), game.turn()), (position, turn + 1));
        let name = game.monsters().iter().find(|m| m.id == id).map_or_else(String::new, |m| m.name.clone());
        let first = new_lines(&game, before).remove(0);
        assert_eq!(first.kind, LogKind::Attack, "{first:?}");
        assert!(first.text.starts_with("You hit the ") || first.text.starts_with("You miss the "), "{first:?}");
        assert!(name.is_empty() || first.text.contains(&name));
    }

    #[test]
    fn dead_monsters_are_removed() {
        let (mut game, east) = cleared(0);
        let id = put_monster(&mut game, east, 0.0);
        let mut died = false;
        for _ in 0..40 {
            let before = game.log().total();
            if let Some(m) = game.monsters.iter_mut().find(|m| m.id == id) {
                m.position = east;
                m.hp = m.hp.min(2);
            }
            game.wanderer.hp = game.wanderer.max_hp();
            game.step(Dir::East);
            if !game.monsters().iter().any(|m| m.id == id) {
                let lines = new_lines(&game, before);
                let deaths: Vec<_> = lines.iter().filter(|e| e.kind == LogKind::Kill).collect();
                assert_eq!(deaths.len(), 1, "{lines:?}");
                assert!(deaths[0].text.starts_with("The ") && deaths[0].text.ends_with(" dies."), "{lines:?}");
                died = true;
                break;
            }
        }
        assert!(died, "the monster never died");
    }

    #[test]
    fn death_costs_time_and_sends_you_home() {
        let (mut game, east) = cleared(0);
        let home_region = game.world().region_at(game.home().0, game.home().1);
        put_monster(&mut game, east, 1.0);
        game.wanderer.hp = 1;
        let (turn, deaths) = (game.turn(), game.deaths());
        for _ in 0..50 {
            if game.deaths() > deaths {
                break;
            }
            game.wanderer.hp = 1;
            game.wait();
        }
        assert_eq!(game.deaths(), deaths + 1, "the wanderer never died");
        assert!(game.turn() >= turn + DEATH_TURNS);
        assert_eq!(game.position(), game.home());
        assert_eq!((game.wanderer().hp, game.wanderer().focus), (1, 0));
        assert!(game.monsters_in_view().next().is_none(), "the killer near home is driven off");
        assert_eq!(game.region().id, home_region);
        let last = game.log().lines().last().unwrap();
        assert_eq!(last, format!("You wake, much later, in {}. Weak and drained, rest (R).", game.region().name));
    }

    #[test]
    fn log_kinds_mark_what_the_frontend_styles() {
        // The frontend colors lines by kind and tints the wanderer when the latest turn added Hurt or Death.
        let game = Game::new(0);
        assert_eq!(game.log().entries().map(|e| e.kind).collect::<Vec<_>>(), [LogKind::Region]);

        // A monster's attack is Hurt exactly when it lands.
        let mut game = on_open_land(0, 2);
        let (x, y) = game.position();
        let id = put_monster(&mut game, (x + 1, y), 1.0);
        let (i, _) = where_is(&game, id);
        let mut rng = Rng::new(4);
        let mut kinds = HashSet::new();
        for _ in 0..40 {
            game.wanderer.hp = game.wanderer.max_hp();
            let before = game.log().total();
            game.monster_attacks(i, &mut rng);
            let added = new_lines(&game, before);
            let hurt = game.wanderer.hp < game.wanderer.max_hp();
            assert_eq!(added.len(), 1, "{added:?}");
            assert_eq!(added[0].kind, if hurt { LogKind::Hurt } else { LogKind::Missed }, "{added:?}");
            kinds.insert(added[0].kind);
        }
        assert_eq!(kinds.len(), 2, "every attack landed or every one missed, so the test proved little");

        // The fatal blow is Hurt, and falling and waking are both Death.
        for _ in 0..40 {
            game.wanderer.hp = 1;
            let before = game.log().total();
            game.monster_attacks(i, &mut rng);
            if game.deaths() > 0 {
                let kinds: Vec<_> = new_lines(&game, before).into_iter().map(|e| e.kind).collect();
                assert_eq!(kinds, [LogKind::Hurt, LogKind::Death, LogKind::Death]);
                return;
            }
        }
        panic!("the wanderer never died");
    }

    #[test]
    fn can_step_is_whether_a_step_passes_a_turn() {
        for seed in [0, 1, 5] {
            let (mut game, wall) = facing_a_wall(seed);
            assert!(!game.can_step(wall), "seed {seed}");
            // A monster standing in the way is attacked, whatever the ground under it.
            let (dx, dy) = wall.delta();
            let (x, y) = game.position();
            put_monster(&mut game, (x + dx, y + dy), 0.5);
            assert!(game.can_step(wall), "seed {seed}");
            assert_eq!(game.step(wall), Step::Attacked, "seed {seed}");
        }
        // Walking straight until something is in the way, then turning, every step agrees with can_step.
        for seed in [0, 4] {
            let mut game = Game::new(seed);
            let (mut way, mut blocked) = (0, 0);
            for i in 0..1_500 {
                game.wanderer.hp = game.wanderer.max_hp();
                let dir = Dir::ALL[way];
                let can = game.can_step(dir);
                let step = game.step(dir);
                assert_eq!(can, step != Step::Blocked, "seed {seed} step {i}: {step:?}");
                if step == Step::Blocked {
                    blocked += 1;
                    way = (way + 1) % 4;
                }
            }
            assert!(blocked > 1, "seed {seed}: nothing was ever in the way, so the test proved little");
        }
    }

    #[test]
    fn monsters_keep_to_their_terrain_and_their_bounds() {
        for seed in [0, 4] {
            let mut game = Game::new(seed);
            let mut known = std::collections::HashSet::new();
            for m in game.monsters() {
                let d = chebyshev(m.position, game.position());
                assert!((SPAWN_MIN..=SPAWN_MAX).contains(&d), "seed {seed}: started at distance {d}");
                known.insert(m.id);
            }
            for i in 0..400 {
                // Keep the wanderer alive, so the run covers walking rather than waking.
                game.wanderer.hp = game.wanderer.max_hp();
                if i % 5 == 0 {
                    game.wait();
                } else {
                    game.step(Dir::ALL[(i / 30) % 4]);
                }
                let here = game.position();
                let mut tiles = std::collections::HashSet::new();
                for m in game.monsters() {
                    let d = chebyshev(m.position, here);
                    assert!(d <= DESPAWN_RADIUS, "seed {seed} turn {i}: monster at distance {d}");
                    if known.insert(m.id) {
                        assert!((SPAWN_MIN..=SPAWN_MAX).contains(&d), "seed {seed} turn {i}: spawned at {d}");
                    }
                    assert_ne!(m.position, here, "seed {seed} turn {i}: monster on the wanderer");
                    assert!(tiles.insert(m.position), "seed {seed} turn {i}: monsters share a tile");
                    let biome = biome_at(game.world(), m.position);
                    assert!(m.phenotype.access.allows(biome), "seed {seed} turn {i}: {} on {}", m.name, biome.name);
                }
            }
            assert!(known.len() > 3, "seed {seed}: only {} monsters ever appeared", known.len());
        }
    }

    #[test]
    fn the_wanderer_heals_over_time() {
        let (mut game, _) = cleared(0);
        game.wanderer.hp = 10;
        for _ in 0..REGEN_TURNS * 3 {
            game.monsters.clear();
            game.wait();
        }
        assert_eq!(game.wanderer().hp, 13);
    }

    #[test]
    fn comparisons_reach_back_to_the_last_region_with_the_clade() {
        let mut game = Game::new(0);
        let mut compared = 0;
        for i in 0..1_500 {
            game.wanderer.hp = game.wanderer.max_hp();
            let from = game.region().id;
            game.step(Dir::ALL[(i / 60) % 4]);
            if game.region().id == from {
                continue;
            }
            for species in &game.local_life().species {
                let comparison = game.compare(species.id);
                let earlier =
                    game.visited.iter().rev().skip(1).find(|r| game.lives[r].species_of(species.id.clade).is_some());
                assert_eq!(comparison.as_ref().map(|c| c.against.region), earlier.copied());
                if let Some(c) = comparison {
                    assert_eq!(c.against.clade, species.id.clade);
                    assert_eq!(c.deltas.len(), crate::COMPARED.len());
                    compared += 1;
                }
            }
        }
        assert!(compared > 0, "no comparison was ever possible, so the test proved little");
    }

    #[test]
    fn nearby_species_are_in_view_nearest_first_and_once() {
        let seed = (0..).find(|&s| Game::new(s).local_life().species.len() >= 3).unwrap();
        let mut game = Game::new(seed);
        game.monsters.clear();
        let (x, y) = game.position();
        let ids: Vec<SpeciesId> = game.local_life().species.iter().map(|s| s.id).collect();
        // Species 1 is nearest and seen twice, species 0 farther and again in the view's corner, and species 2
        // just out of view.
        put_species(&mut game, 1, (x + 1, y), 0.5);
        put_species(&mut game, 0, (x - 2, y + 1), 0.5);
        put_species(&mut game, 1, (x, y + 2), 0.5);
        let corner = put_species(&mut game, 0, (x - VIEW_RADIUS, y - VIEW_RADIUS), 0.5);
        let outside = put_species(&mut game, 2, (x + VIEW_RADIUS + 1, y), 0.5);
        let in_view: Vec<MonsterId> = game.monsters_in_view().map(|m| m.id).collect();
        assert_eq!(in_view.len(), 4, "seed {seed}");
        assert!(in_view.contains(&corner) && !in_view.contains(&outside), "seed {seed}");
        let nearby = game.nearby_species();
        assert_eq!(nearby, [ids[1], ids[0]], "seed {seed}");
        for id in nearby {
            let species = game.species(id).expect("a monster's species is known");
            assert!(!species.name.is_empty() && !species.description.is_empty());
        }
    }

    /// A game for `seed` with the wanderer in the middle of a square of land at least `2r + 1` wide, and no
    /// monsters, so a monster placed near it can move any way.
    fn on_open_land(seed: u64, r: i64) -> Game {
        let mut game = Game::new(seed);
        let world = *game.world();
        let open =
            |(cx, cy): (i64, i64)| (-r..=r).all(|dy| (-r..=r).all(|dx| is_land(biome_at(&world, (cx + dx, cy + dy)))));
        let (x0, y0) = game.position();
        let at = (0..100)
            .flat_map(|k| ring(k).map(move |(dx, dy)| (x0 + dx, y0 + dy)))
            .find(|&p| open(p))
            .expect("open land near the start");
        game.position = at;
        game.update_region(false);
        game.monsters.clear();
        game
    }

    /// Monster `id`'s index and Manhattan distance from the wanderer.
    fn where_is(game: &Game, id: MonsterId) -> (usize, i64) {
        let i = game.monsters.iter().position(|m| m.id == id).unwrap();
        (i, manhattan(game.monsters[i].position, game.position()))
    }

    #[test]
    fn aggressive_monsters_close_in_and_attack() {
        let mut game = on_open_land(0, 5);
        let (x, y) = game.position();
        let id = put_monster(&mut game, (x + 3, y), 1.0);
        let mut rng = Rng::new(1);
        let (i, d) = where_is(&game, id);
        game.monster_acts(i, &mut rng, false);
        assert_eq!(where_is(&game, id).1, d - 1, "it steps toward the wanderer it senses");
        game.monsters[i].position = (x + 1, y);
        let before = game.log().total();
        game.monster_acts(i, &mut rng, false);
        assert_eq!(game.monsters[i].position, (x + 1, y), "adjacent, it attacks rather than moves");
        let lines = new_lines(&game, before);
        assert!(lines.len() == 1 && lines[0].text.contains(&game.monsters[i].name), "{lines:?}");
    }

    #[test]
    fn skittish_monsters_flee_and_fight_only_when_cornered() {
        let mut game = on_open_land(0, 5);
        let (x, y) = game.position();
        let id = put_monster(&mut game, (x + 2, y), 0.0);
        let mut rng = Rng::new(2);
        let (i, d) = where_is(&game, id);
        let before = game.log().total();
        game.monster_acts(i, &mut rng, false);
        assert_eq!(where_is(&game, id).1, d + 1, "it steps away from the wanderer it senses");
        assert_eq!(game.log().total(), before, "fleeing says nothing");
        // Hemmed in by three others and the wanderer, it fights.
        game.monsters[i].position = (x + 1, y);
        for at in [(x + 2, y), (x + 1, y - 1), (x + 1, y + 1)] {
            put_monster(&mut game, at, 0.5);
        }
        game.monster_acts(i, &mut rng, false);
        assert_eq!(game.monsters[i].position, (x + 1, y));
        assert_eq!(game.log().total(), before + 1, "cornered, it attacks");
    }

    #[test]
    fn territorial_monsters_guard_home_but_never_chase_far() {
        let mut game = on_open_land(0, 5);
        let (x, y) = game.position();
        let id = put_monster(&mut game, (x + 3, y), 0.5);
        let mut rng = Rng::new(3);
        let (i, d) = where_is(&game, id);
        assert!(chebyshev(game.monsters[i].home, (x, y)) <= TERRITORY);
        game.monster_acts(i, &mut rng, false);
        assert_eq!(where_is(&game, id).1, d - 1, "a trespasser near home is approached");
        // Far from home, with the wanderer outside its territory, it heads home instead.
        game.monsters[i].position = (x + 2, y);
        game.monsters[i].home = (x + 2 + TERRITORY + 3, y);
        game.monster_acts(i, &mut rng, false);
        assert_eq!(game.monsters[i].position, (x + 3, y), "it goes back toward home");
        // Adjacent and on its ground, it attacks.
        game.monsters[i].position = (x + 1, y);
        game.monsters[i].home = (x + 1, y);
        let before = game.log().total();
        game.monster_acts(i, &mut rng, false);
        assert_eq!(game.monsters[i].position, (x + 1, y));
        assert_eq!(game.log().total(), before + 1, "it attacks a trespasser beside it");
    }

    #[test]
    fn speed_banks_as_energy_and_actions_are_capped() {
        // An aggressive monster beside the wanderer attacks, logging one line, on every action it gets.
        let actions = |speed: i32, turns: u64| {
            let mut game = on_open_land(0, 2);
            let (x, y) = game.position();
            let id = put_monster(&mut game, (x + 1, y), 1.0);
            let (i, _) = where_is(&game, id);
            game.monsters[i].phenotype.stats.speed = speed;
            let before = game.log().total();
            for _ in 0..turns {
                game.wanderer.hp = game.wanderer.max_hp();
                game.monsters_act(false, false);
                game.turn += 1;
            }
            (game.log().total() - before, game.monsters[i].energy)
        };
        // Speed is tenths of an action: 14 acts 14 times in ten turns, 6 acts 6 times, with nothing left over.
        assert_eq!(actions(14, 10), (14, 0));
        assert_eq!(actions(6, 10), (6, 0));
        assert_eq!(actions(ACTION_COST, 10), (10, 0));
        // Too slow to act at all in one turn; the speed is banked for the next.
        assert_eq!(actions(4, 1), (0, 4));
        // However fast, no more than MAX_ACTIONS a turn; the rest stays banked.
        let (count, banked) = actions(3 * ACTION_COST, 10);
        assert_eq!(count, 10 * u64::from(MAX_ACTIONS));
        assert_eq!(banked, 10 * (3 * ACTION_COST - MAX_ACTIONS as i32 * ACTION_COST));
    }

    #[test]
    fn one_monsters_draws_never_shift_anothers() {
        // Two games alike but for an extra monster that acts first. If the streams were shared, its draws
        // would shift everyone else's; with a stream per monster, the watched one moves the same either way.
        let mut differed = 0;
        for seed in [0, 4, 42] {
            let (mut alone, _) = cleared(seed);
            let (x, y) = alone.position();
            let watched = put_monster(&mut alone, (x + 7, y), 0.5);
            let mut crowded = alone.clone();
            let extra = put_monster(&mut crowded, (x - 7, y), 0.5);
            crowded.monsters.rotate_right(1);
            let mut paths = (Vec::new(), Vec::new());
            for _ in 0..12 {
                for (game, path, keep) in [(&mut alone, &mut paths.0, watched), (&mut crowded, &mut paths.1, extra)] {
                    game.wanderer.hp = game.wanderer.max_hp();
                    game.wait();
                    // Spawns would differ between the two (capacity counts the extra monster), so drop them.
                    game.monsters.retain(|m| m.id == watched || m.id == keep);
                    path.push(game.monsters.iter().find(|m| m.id == watched).map(|m| m.position));
                }
            }
            assert_eq!(paths.0, paths.1, "seed {seed}");
            differed += usize::from(paths.0.windows(2).any(|w| w[0] != w[1]));
        }
        assert!(differed > 0, "the watched monster never moved, so the test proved little");
    }

    #[test]
    fn region_lives_stay_bounded_and_cover_what_is_looked_up() {
        let mut game = Game::new(0);
        let mut seen = HashSet::new();
        // Short walks with long jumps between them, so the trip keeps reaching regions it hasn't grown.
        for leg in 0..30 {
            let (x, y) = game.position();
            if let Some(to) = find_land(game.world(), x + 300, y + 40 * (leg % 3 - 1)) {
                game.position = to;
                game.update_region(true);
            }
            for i in 0..30 {
                game.wanderer.hp = game.wanderer.max_hp();
                game.step(Dir::ALL[(i / 10) % 4]);
                seen.extend(game.lives.keys().copied());
                assert!(game.lives.len() <= LIVES_LIMIT, "leg {leg}: {} lives held", game.lives.len());
                assert!(game.lives.contains_key(&game.region().id), "leg {leg}: current region forgotten");
                for r in &game.visited {
                    assert!(game.lives.contains_key(r), "leg {leg}: visited region forgotten");
                }
                for m in game.monsters() {
                    assert!(game.species(m.species).is_some(), "leg {leg}: {}'s species forgotten", m.name);
                }
            }
        }
        assert!(seen.len() > 2 * LIVES_LIMIT, "only {} regions grown, so little was forgotten", seen.len());
    }

    #[test]
    fn ring_points_walk_the_ring() {
        for r in 1..5 {
            let points: std::collections::HashSet<_> = (0..8 * r).map(|k| ring_point(r, k)).collect();
            assert_eq!(points, ring(r).collect());
        }
    }

    #[test]
    fn composed_attacks_use_real_openings_and_charge_for_every_attempt() {
        let mut hits = 0;
        let mut misses = 0;
        for seed in 0..40 {
            let mut game = Game::composition_trial(seed, Maneuver::backstab_burst()).unwrap();
            let before = game.clone();
            let preview = game.preview_attack(Dir::East).unwrap();
            assert_eq!(game, before, "preview must not mutate the world or RNG");
            assert_eq!((preview.min_hit, preview.max_hit, preview.focus, preview.opening), (15, 17, 7, true));
            let mut guarded = game.clone();
            guarded.sneaking = false;
            assert_eq!(game.use_technique(Dir::East), Step::Attacked);
            assert_eq!(guarded.use_technique(Dir::East), Step::Attacked);
            let result = game.last_attack().unwrap();
            assert_eq!(game.wanderer.focus, MAX_FOCUS - 7);
            assert!(!game.sneaking);
            assert!(game.monsters[0].alerted);
            assert_eq!(game.monsters[0].hp, 40 - result.total());
            if result.hit {
                hits += 1;
                assert!(result.amplified);
                assert!(!guarded.last_attack().unwrap().amplified);
                assert!(result.total() > guarded.last_attack().unwrap().total());
            } else {
                misses += 1;
                assert_eq!(result.total(), 0);
            }
            game.toggle_sneak();
            assert!(!game.preview_attack(Dir::East).unwrap().opening, "toggling cannot refresh surprise");
        }
        assert!(hits > 0 && misses > 0);
    }

    #[test]
    fn basic_strikes_use_the_equipped_weapon_without_focus_aim_or_payload() {
        let mut hits = 0;
        let mut misses = 0;
        for (weapon, minimum, maximum) in [
            (crate::Weapon::Dagger, 1, 2),
            (crate::Weapon::Spear, 2, 4),
            (crate::Weapon::Club, 3, 5),
            (crate::Weapon::Staff, 1, 3),
        ] {
            for delivery in [Delivery::Weapon, Delivery::Touch, Delivery::Ground] {
                let mut maneuver = if delivery == Delivery::Ground {
                    Maneuver::staff_inferno()
                } else {
                    let mut m = Maneuver::backstab_burst();
                    m.technique.delivery = delivery;
                    m.technique.burst.as_mut().unwrap().trigger = crate::Trigger::Contact;
                    m
                };
                maneuver.weapon = weapon;
                if maneuver.validate().is_err() {
                    continue;
                }
                for seed in 0..16 {
                    let fixture = Game::composition_trial(seed, maneuver.clone()).unwrap();
                    for focus in [0, maneuver.technique.focus_cost() - 1, maneuver.technique.focus_cost(), MAX_FOCUS] {
                        let mut game = fixture.clone();
                        game.wanderer.focus = focus;
                        game.monsters[0].phenotype.stats.speed = 0;
                        let before = game.clone();
                        let preview = game.preview_strike(Dir::East).unwrap();
                        assert_eq!(game, before, "preview changes nothing");
                        assert_eq!((preview.min_hit, preview.max_hit, preview.focus), (minimum, maximum, 0));
                        assert!(!preview.opening);
                        assert!(game.can_step(Dir::East));
                        assert_eq!(game.step(Dir::East), Step::Attacked);
                        assert_eq!(game.turn, before.turn + 1);
                        assert_eq!(game.position, before.position);
                        assert_eq!(game.wanderer.focus, focus);
                        assert_eq!(game.wanderer.hp, before.wanderer.hp, "no area backlash");
                        assert_eq!(game.maneuver, maneuver, "no re-preparation or weapon swap");
                        assert!(!game.sneaking);
                        assert!(game.monsters[0].alerted);
                        let result = game.last_attack.unwrap();
                        assert_eq!(result.burst, 0);
                        assert!(!result.burst_released && !result.opening && !result.amplified);
                        if result.hit {
                            hits += 1;
                            assert!((minimum..=maximum).contains(&result.total()));
                        } else {
                            misses += 1;
                            assert_eq!(result.total(), 0);
                        }
                        assert_eq!(game.monsters[0].hp, before.monsters[0].hp - result.total());
                    }
                }
            }
        }
        assert!(hits > 0 && misses > 0, "both hits and misses spend a turn without Focus");
    }

    #[test]
    fn failed_preparation_and_unaffordable_attacks_are_atomic() {
        let mut game = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
        let before = game.clone();
        assert!(game.prepare(Maneuver::default()).is_err());
        assert_eq!(game, before);
        game.wanderer.focus = 6;
        let before = game.clone();
        assert!(game.can_step(Dir::East), "a basic strike is still available");
        assert_eq!(game.use_technique(Dir::East), Step::Blocked);
        assert_eq!(game.turn, before.turn);
        assert_eq!(game.wanderer, before.wanderer);
        assert_eq!(game.monsters, before.monsters);
        assert_eq!(game.last_attack, None);
        assert!(game.sneaking);
        assert_eq!(game.use_technique(Dir::West), Step::Blocked);
        assert_eq!(game.turn, before.turn);
        assert_eq!(game.wanderer, before.wanderer);
        assert_eq!(game.monsters, before.monsters);

        game.monsters.clear();
        let before = game.clone();
        let mut invalid = Maneuver::backstab_burst();
        invalid.weapon = crate::Weapon::Club;
        assert!(game.prepare(invalid).is_err());
        assert_eq!(game, before);

        let mut game = Game::inferno_trial(42).unwrap();
        let before = game.clone();
        assert_eq!(game.use_technique(Dir::North), Step::Blocked, "no target does not cast an area");
        assert_eq!(game.turn, before.turn);
        assert_eq!(game.wanderer, before.wanderer);
        assert_eq!(game.monsters, before.monsters);
    }

    #[test]
    fn blocked_delivery_spends_focus_without_creating_a_burst() {
        let mut game = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
        game.monsters[0].phenotype.stats.defense = 100;
        assert_eq!(game.preview_attack(Dir::East).unwrap().max_hit, 1);
        game.use_technique(Dir::East);
        let result = game.last_attack().unwrap();
        assert_eq!(result.burst, 0);
        assert!(!result.burst_released && !result.penetrated);
        assert_eq!(game.wanderer.focus, MAX_FOCUS - 7);
    }

    #[test]
    fn observation_does_not_depend_on_monster_speed_and_tracking_must_be_broken() {
        let mut game = on_open_land(0, 3);
        let (x, y) = game.position;
        game.toggle_sneak();
        put_monster(&mut game, (x + 1, y), 1.0);
        game.monsters[0].phenotype.stats.speed = 0;
        game.monsters[0].phenotype.sense = 4;
        game.wait();
        assert!(!game.monsters[0].alerted);
        game.monsters[0].phenotype.sense = 8;
        game.wait();
        assert!(game.monsters[0].alerted, "an acute observer can detect a sneak before it gets an action");
        game.monsters[0].phenotype.sense = 4;
        game.wait();
        assert!(game.monsters[0].alerted, "normal range maintains tracking");
        game.monsters[0].position = (x + 6, y);
        game.wait();
        assert!(!game.monsters[0].alerted, "leaving normal range loses tracking");
    }

    #[test]
    fn focus_recovers_only_by_resting_at_a_safe_home() {
        let mut game = on_open_land(0, 3);
        game.home = game.position;
        game.toggle_sneak();
        assert_eq!(game.wanderer.focus, MAX_FOCUS - 1);
        game.step(Dir::East);
        assert_eq!(game.wanderer.focus, MAX_FOCUS - 2);
        let before = game.turn;
        assert_eq!(game.recover_focus(), Step::Blocked);
        assert_eq!(game.turn, before);
        game.wait();
        assert_eq!(game.wanderer.focus, MAX_FOCUS - 2);
        game.step(Dir::West);
        game.monsters.clear();
        assert_eq!(game.recover_focus(), Step::Waited);
        assert_eq!(game.wanderer.focus, (MAX_FOCUS - 2 + REST_FOCUS).min(MAX_FOCUS));
        assert!(!game.sneaking);
        game.wanderer.focus = 0;
        assert_eq!(game.toggle_sneak(), Step::Blocked);
        game.die();
        assert_eq!(game.wanderer.focus, 0, "death does not restore Focus");
        assert!(!game.sneaking);
    }

    #[test]
    fn home_rest_recovers_through_nearby_threats_without_protecting_other_actions() {
        for after_death in [false, true] {
            let mut game = on_open_land(0, 4);
            game.home = game.position;
            if after_death {
                game.die();
            } else {
                game.wanderer.hp = 1;
                game.wanderer.focus = 0;
            }
            let deaths = game.deaths;
            let (x, y) = game.home;
            let attacker = put_monster(&mut game, (x + 1, y), 1.0);
            game.monsters.last_mut().unwrap().phenotype.stats =
                crate::Stats { hp: 40, attack: 100, defense: 0, speed: 2 * ACTION_COST };
            game.monsters.last_mut().unwrap().hp = 40;
            let approach = put_monster(&mut game, (x + 3, y), 1.0);
            game.monsters.last_mut().unwrap().phenotype.stats.speed = ACTION_COST;
            let learned = game.wanderer;
            let mut rests = 0;
            while game.wanderer.hp < game.wanderer.max_hp() || game.wanderer.focus < game.wanderer.max_focus() {
                assert_eq!(game.recover_focus(), Step::Waited, "home remains usable with creatures in view");
                assert_eq!(game.deaths, deaths, "rest must not chain into another death");
                rests += 1;
                assert!(rests <= 15, "recovery remains gradual but must complete");
            }
            assert!(rests >= 8);
            assert!(
                game.monsters.iter().find(|m| m.id == approach).unwrap().position.0 < x + 3,
                "creatures still move during sheltered rest"
            );
            assert_eq!(game.wanderer.weapons, learned.weapons);
            assert_eq!(game.wanderer.focus_skill, learned.focus_skill);
            assert_eq!(game.wanderer.stealth, learned.stealth);
            assert!(game.monsters.iter().any(|m| m.id == attacker), "rest does not clear encounters");
            assert_eq!(game.recover_focus(), Step::Blocked, "a full character cannot buy protected wait turns");
            let mut attack = game.clone();
            assert_eq!(attack.step(Dir::East), Step::Attacked);
            assert_eq!(attack.deaths, deaths + 1, "attacking from home is exposed");
            game.wait();
            assert_eq!(game.deaths, deaths + 1, "ordinary waiting is exposed too");
        }
    }

    #[test]
    fn home_shelter_also_blocks_territorial_and_cornered_skittish_attacks() {
        for temperament in [Temperament::Territorial, Temperament::Skittish] {
            let mut game = on_open_land(0, 4);
            game.home = game.position;
            game.wanderer.hp = 1;
            game.wanderer.focus = 0;
            let (x, y) = game.home;
            put_monster(&mut game, (x + 1, y), 1.0);
            let m = game.monsters.last_mut().unwrap();
            m.phenotype.temperament = temperament;
            m.phenotype.sense = 8;
            m.phenotype.stats.attack = 100;
            m.phenotype.stats.speed = 2 * ACTION_COST;
            for at in [(x + 2, y), (x + 1, y - 1), (x + 1, y + 1)] {
                put_monster(&mut game, at, 0.0);
                game.monsters.last_mut().unwrap().phenotype.stats.speed = 0;
            }
            for _ in 0..15 {
                if !game.can_rest() {
                    break;
                }
                assert_eq!(game.recover_focus(), Step::Waited);
                assert_eq!(game.deaths, 0);
            }
            assert!(!game.can_rest());
            assert_eq!(game.wanderer.hp, game.wanderer.max_hp());
            assert_eq!(game.wanderer.focus, game.wanderer.max_focus());
            game.wait();
            assert_eq!(game.deaths, 1, "the same cornered creature attacks an exposed action");
        }
    }

    #[test]
    fn death_wakes_weak_keeps_distant_creatures_and_rest_recovers_gradually() {
        let mut game = on_open_land(0, 3);
        game.home = game.position;
        let (x, y) = game.home;
        let near = put_monster(&mut game, (x + 3, y), 1.0);
        let far = put_monster(&mut game, (x + VIEW_RADIUS + 2, y), 1.0);
        let gone = put_monster(&mut game, (x + DESPAWN_RADIUS + 1, y), 1.0);
        for m in &mut game.monsters {
            m.hp = 1;
            m.alerted = true;
        }
        game.position = (x - 5, y);
        game.die();
        assert_eq!(game.position, game.home);
        assert_eq!((game.wanderer.hp, game.wanderer.focus), (1, 0), "death refills nothing");
        let ids: Vec<_> = game.monsters.iter().map(|m| m.id).collect();
        assert_eq!(ids, [far], "creatures in view of home are driven off; distant ones stay");
        assert!(!ids.contains(&near) && !ids.contains(&gone));
        let kept = &game.monsters[0];
        assert_eq!(kept.hp, kept.max_hp(), "creatures recover while you are away");
        assert!(!kept.alerted);

        game.monsters.clear();
        let mut rests = 0;
        while game.wanderer.focus < MAX_FOCUS || game.wanderer.hp < game.wanderer.max_hp() {
            let (hp, focus, turn) = (game.wanderer.hp, game.wanderer.focus, game.turn);
            assert_eq!(game.recover_focus(), Step::Waited);
            assert_eq!(game.turn, turn + 1);
            assert!(game.wanderer.focus <= (focus + REST_FOCUS).min(MAX_FOCUS));
            assert!(game.wanderer.hp >= hp && game.wanderer.hp <= game.wanderer.max_hp());
            assert!(game.wanderer.focus > focus || focus == MAX_FOCUS);
            rests += 1;
            game.monsters.clear();
        }
        assert!(rests >= (MAX_FOCUS / REST_FOCUS) as usize, "recovery takes turns, not one keypress");
        let before = game.clone();
        assert_eq!(game.recover_focus(), Step::Blocked, "nothing left to recover");
        assert_eq!(game.turn, before.turn);
    }

    #[test]
    fn an_unlisted_composition_runs_through_the_same_game_entry_point() {
        let mut maneuver = Maneuver::backstab_burst();
        maneuver.weapon = crate::Weapon::Spear;
        maneuver.name = "Quiet comet".into();
        let mut game = Game::composition_trial(42, maneuver.clone()).unwrap();
        let mut twin = game.clone();
        twin.maneuver.name = "A name suggested by an AI".into();
        for action in [Dir::East, Dir::East, Dir::West, Dir::West] {
            assert_eq!(game.preview_attack(action), twin.preview_attack(action));
            assert_eq!(game.use_technique(action), twin.use_technique(action));
            assert_eq!(game.wanderer, twin.wanderer);
            assert_eq!(game.monsters, twin.monsters);
            assert_eq!(game.last_attack, twin.last_attack);
        }
        assert_eq!(game.maneuver, maneuver);
    }

    #[test]
    fn a_fire_payload_can_use_the_same_precise_weapon_relationship() {
        let mut maneuver = Maneuver::backstab_burst();
        maneuver.name = "Ember needle".into();
        maneuver.technique.burst.as_mut().unwrap().element = crate::Element::Fire;
        let mut game = Game::composition_trial(42, maneuver).unwrap();
        game.monsters[0].phenotype.heat = 2.0 / 3.0;
        assert_eq!(game.monsters[0].fire_resistance(), 50);
        let preview = game.preview_attack(Dir::East).unwrap();
        assert_eq!((preview.min_hit, preview.max_hit, preview.focus), (9, 11, 7));
        game.use_technique(Dir::East);
        let result = game.last_attack().unwrap();
        assert_eq!((result.physical, result.burst, result.amplified), (4, 6, true));
    }

    #[test]
    fn area_fire_hits_the_whole_boundary_and_caster_but_not_outside_it() {
        let mut game = on_open_land(0, 4);
        let mut maneuver = Maneuver::staff_inferno();
        maneuver.name = "Another legal shape".into();
        maneuver.weapon = crate::Weapon::Club;
        maneuver.technique.burst.as_mut().unwrap().shape = crate::Shape::Area(3);
        game.prepare(maneuver).unwrap();
        let (x, y) = game.position;
        let mut inside = Vec::new();
        for dy in -3..=3 {
            for dx in -3..=3 {
                if (dx, dy) != (0, 0) {
                    inside.push(put_monster(&mut game, (x + dx, y + dy), if dx < 0 { 0.0 } else { 1.0 }));
                }
            }
        }
        let outside = put_monster(&mut game, (x + 4, y), 1.0);
        for monster in &mut game.monsters {
            monster.phenotype.heat = 0.0;
            monster.phenotype.stats.speed = 0;
            monster.hp = 40;
        }
        game.wanderer.hp = 30;
        game.turn = 0;
        let before = game.clone();
        let preview = game.preview_area().unwrap();
        assert_eq!(game, before);
        assert_eq!(preview, AreaPreview { radius: 3, focus: 23, targets: 48, damage: 576, self_damage: 12 });
        assert_eq!(game.unleash(), Step::Attacked);
        assert_eq!(game.turn, 1);
        assert_eq!((game.wanderer.focus, game.wanderer.hp), (1, 18));
        for monster in &game.monsters {
            if inside.contains(&monster.id) {
                assert_eq!(monster.hp, 28, "every inside tile, regardless of temperament, is hit");
            } else if monster.id == outside {
                assert_eq!(monster.hp, 40);
            }
        }
        let before = game.clone();
        assert_eq!(game.unleash(), Step::Blocked);
        assert_eq!(game.turn, before.turn);
        assert_eq!(game.wanderer, before.wanderer);
        assert_eq!(game.monsters, before.monsters);
    }

    #[test]
    fn staff_trial_demonstrates_tolerance_cost_and_identical_technique_resolution() {
        let mut game = Game::inferno_trial(42).unwrap();
        let preview = game.preview_area().unwrap();
        // 0%, 30%, and rounded 68% resistance: 12 + 8 + 3.
        assert_eq!(preview, AreaPreview { radius: 2, focus: 18, targets: 3, damage: 23, self_damage: 12 });
        let mut bump = game.clone();
        assert_eq!(game.unleash(), bump.use_technique(Dir::East));
        assert_eq!(game, bump);
        assert_eq!(game.wanderer.focus, 6);
        assert_eq!(game.log.last(20).filter(|e| e.kind == LogKind::Kill).count(), 2);
        assert!(game.monsters.iter().any(|m| m.name == "heat-hardy trial sentinel" && m.hp == 3));
    }

    #[test]
    fn area_self_damage_can_kill_and_cannot_be_avoided_by_an_empty_cast() {
        let mut game = Game::inferno_trial(42).unwrap();
        game.wanderer.hp = 12;
        assert_eq!(game.unleash(), Step::Attacked);
        assert_eq!(game.deaths, 1);
        assert_eq!(game.turn, 1 + DEATH_TURNS);
        assert_eq!((game.wanderer.hp, game.wanderer.focus), (1, 0));
        assert!(!game.sneaking && game.monsters.is_empty(), "the survivor beside home is driven off");

        let mut game = Game::inferno_trial(42).unwrap();
        game.monsters.clear();
        assert_eq!(game.preview_area().unwrap().targets, 0);
        assert_eq!(game.unleash(), Step::Attacked);
        assert_eq!((game.wanderer.focus, game.wanderer.hp), (6, 18));
        let before = game.clone();
        game.maneuver = Maneuver::default();
        assert_eq!(game.unleash(), Step::Blocked);
        assert_eq!(game.turn, before.turn);
        assert_eq!(game.wanderer, before.wanderer);
    }

    #[test]
    fn weapon_learning_uses_landed_physical_damage_not_magic_or_attempts() {
        let mut hits = 0;
        let mut misses = 0;
        for seed in 0..32 {
            for technique in [false, true] {
                let mut game = Game::composition_trial(seed, Maneuver::backstab_burst()).unwrap();
                if technique {
                    game.use_technique(Dir::East);
                } else {
                    game.step(Dir::East);
                }
                let result = game.last_attack.unwrap();
                let mut expected = crate::Skill::default();
                // Sentinel: 40 HP, one incoming damage per action, one unskilled dagger damage
                // against defense 3. Relative to 30 full HP: floor(100 * 40 / 30) = 133.
                expected.learn(result.physical as u32 * 133, crate::Rank::Expert);
                assert_eq!(game.wanderer.weapons.get(crate::Weapon::Dagger), expected);
                for other in [crate::Weapon::Spear, crate::Weapon::Club, crate::Weapon::Staff] {
                    assert_eq!(game.wanderer.weapons.get(other), crate::Skill::default());
                }
                if result.hit {
                    hits += 1;
                } else {
                    misses += 1;
                }
            }
            let mut touch = Maneuver::backstab_burst();
            touch.technique.delivery = Delivery::Touch;
            touch.technique.burst.as_mut().unwrap().trigger = crate::Trigger::Contact;
            let mut game = Game::composition_trial(seed, touch).unwrap();
            let before = game.wanderer.weapons;
            game.use_technique(Dir::East);
            assert_eq!(game.wanderer.weapons, before);

            let mut game = Game::inferno_trial(seed).unwrap();
            let before = game.wanderer.weapons;
            game.unleash();
            assert_eq!(game.wanderer.weapons, before);
        }
        assert!(hits > 0 && misses > 0);
    }

    #[test]
    fn weak_or_inert_creatures_and_noncombat_actions_give_no_practice() {
        for inert in [false, true] {
            let mut game = Game::composition_trial(42, Maneuver::default()).unwrap();
            if inert {
                game.monsters[0].phenotype.stats.speed = 0;
            } else {
                game.monsters[0].hp = 3;
                game.monsters[0].phenotype.stats = crate::Stats { hp: 3, attack: 1, defense: 0, speed: ACTION_COST };
            }
            let before = game.wanderer.weapons;
            game.step(Dir::East);
            assert!(game.last_attack.unwrap().hit);
            assert_eq!(game.wanderer.weapons, before);
        }
        let mut game = Game::composition_trial(42, Maneuver::default()).unwrap();
        game.step(Dir::East);
        let learned = game.wanderer.weapons;
        assert_ne!(learned, crate::WeaponSkills::default(), "fixture has real earned practice");
        game.monsters.clear();
        game.wait();
        game.toggle_sneak();
        game.step(Dir::West);
        game.step(Dir::East);
        game.monsters.clear();
        game.wanderer.hp = 20;
        game.recover_focus();
        game.die();
        assert_eq!(game.wanderer.weapons, learned, "time, injury, death, rest, and movement neither train nor erase");
        game.monsters.clear();
        let spear = Maneuver { weapon: crate::Weapon::Spear, ..Maneuver::default() };
        game.prepare(spear).unwrap();
        game.prepare(Maneuver::default()).unwrap();
        assert_eq!(game.wanderer.weapons, learned, "refitting does not transfer or reset practice");
    }

    #[test]
    fn overkill_and_death_healing_cannot_refill_an_individuals_practice() {
        let mut game = Game::composition_trial(42, Maneuver::backstab_burst()).unwrap();
        game.monsters[0].hp = 1;
        game.use_technique(Dir::East);
        assert!(game.last_attack.unwrap().hit);
        let mut expected = crate::Skill::default();
        expected.learn(133, crate::Rank::Expert);
        assert_eq!(game.wanderer.weapons.get(crate::Weapon::Dagger), expected, "only one remaining HP counts");

        let mut game = Game::composition_trial(42, Maneuver::default()).unwrap();
        game.monsters[0].practice_left = 1;
        game.step(Dir::East);
        assert_eq!(game.monsters[0].practice_left, 0);
        let learned = game.wanderer.weapons;
        let (x, y) = game.home;
        game.monsters[0].position = (x + VIEW_RADIUS + 1, y);
        game.die();
        assert_eq!(game.monsters[0].hp, 40);
        assert_eq!(game.monsters[0].practice_left, 0);
        game.monsters[0].position = (x + 1, y);
        // The previously trained creature survives death but cannot pay again, even with a new weapon.
        game.wanderer.hp = game.wanderer.max_hp();
        game.maneuver.weapon = crate::Weapon::Club;
        game.step(Dir::East);
        assert_eq!(game.wanderer.weapons, learned);
    }

    #[test]
    fn practice_improves_real_strike_previews_and_logs_only_rank_changes() {
        let mut game = Game::composition_trial(42, Maneuver::default()).unwrap();
        let initial = game.preview_strike(Dir::East).unwrap();
        let id = game.monsters[0].id;
        for _ in 0..32 {
            if !game.monsters.iter().any(|m| m.id == id) {
                break;
            }
            game.step(Dir::East);
            if game.wanderer.weapons.get(crate::Weapon::Dagger).rank() >= crate::Rank::Practiced {
                let preview = game.preview_strike(Dir::East).unwrap();
                assert_eq!(preview.max_hit, initial.max_hit + 1);
                assert_eq!(game.log.entries().filter(|e| e.kind == LogKind::Learning).count(), 2);
                assert!(game.log.lines().any(|line| line == "Your dagger skill is now practiced."));
                return;
            }
        }
        panic!("ordinary play must reach the first rank without injected practice");
    }

    #[test]
    fn threat_tracks_full_strength_and_speed_not_injury_or_earned_skill() {
        let mut game = Game::composition_trial(42, Maneuver::default()).unwrap();
        for (weapon, expected) in [
            (crate::Weapon::Dagger, 133),
            (crate::Weapon::Spear, 44),
            (crate::Weapon::Club, 33),
            (crate::Weapon::Staff, 66),
        ] {
            assert_eq!(game.monsters[0].weapon_threat(&game.wanderer, weapon), expected);
            game.wanderer.hp = 1;
            game.wanderer.focus = 0;
            game.monsters[0].hp = 1;
            game.wanderer.weapons.learn(weapon, u32::MAX, 200);
            assert_eq!(game.monsters[0].weapon_threat(&game.wanderer, weapon), expected);
        }
        for (speed, expected) in [(-1, 0), (0, 0), (1, 13), (10, 133), (20, 200), (i32::MAX, 200)] {
            game.monsters[0].phenotype.stats.speed = speed;
            assert_eq!(game.monsters[0].weapon_threat(&game.wanderer, crate::Weapon::Dagger), expected);
        }
        game.monsters[0].phenotype.temperament = Temperament::Skittish;
        assert_eq!(game.monsters[0].weapon_threat(&game.wanderer, crate::Weapon::Dagger), 0);
    }

    /// A dangerous creature with controlled numbers, at dagger threat 66. It never gets to act, so it
    /// stays exactly where a test puts it.
    fn still_threat(game: &mut Game, at: (i64, i64), sense: i64) {
        put_monster(game, at, 1.0);
        let m = game.monsters.last_mut().unwrap();
        m.phenotype.stats = crate::Stats { hp: 30, attack: 3, defense: 1, speed: ACTION_COST };
        m.phenotype.temperament = Temperament::Aggressive;
        m.hp = 30;
        m.phenotype.sense = sense;
        m.energy = -100_000;
        assert_eq!(game.monsters.last().unwrap().weapon_threat(&game.wanderer, crate::Weapon::Dagger), 66);
    }

    fn practiced(evidence: u32) -> crate::Skill {
        let mut skill = crate::Skill::default();
        skill.learn(evidence, Rank::Expert);
        skill
    }

    #[test]
    fn unseen_sneaking_moves_near_real_threats_teach_stealth() {
        let mut game = on_open_land(0, 3);
        let (x, y) = game.position;
        still_threat(&mut game, (x + 5, y), 8);
        game.toggle_sneak();
        game.wait();
        assert_eq!(game.wanderer.stealth, crate::Skill::default(), "entering sneak and standing still teach nothing");
        game.step(Dir::East);
        assert!(!game.monsters[0].alerted, "inside normal range 8, outside sneaking range 2");
        assert_eq!(game.wanderer.stealth, practiced(66 * STEALTH_PRACTICE));
        assert_eq!(game.monsters[0].stealth_left, STEALTH_LESSONS - 1);

        game.monsters[0].stealth_left = 0;
        let learned = game.wanderer.stealth;
        game.step(Dir::West);
        assert_eq!(game.wanderer.stealth, learned, "an exhausted creature teaches no more");

        game.monsters[0].stealth_left = STEALTH_LESSONS;
        game.monsters[0].phenotype.temperament = Temperament::Skittish;
        game.step(Dir::East);
        assert_eq!(game.wanderer.stealth, learned, "harmless creatures teach nothing");
        assert_eq!(game.monsters[0].stealth_left, STEALTH_LESSONS, "and keep their lessons");

        game.monsters[0].phenotype.temperament = Temperament::Aggressive;
        game.toggle_sneak();
        game.step(Dir::West);
        assert!(game.monsters[0].alerted, "an ordinary walker in range is simply found");
        assert_eq!(game.wanderer.stealth, learned);
    }

    #[test]
    fn breaking_observed_tracking_teaches_stealth_but_reveals_and_death_do_not() {
        let mut game = on_open_land(0, 3);
        let (x, y) = game.position;
        game.home = (x, y);
        still_threat(&mut game, (x + 3, y), 4);
        game.wait();
        assert!(game.monsters[0].tracking);
        game.monsters[0].position = (x + 6, y);
        game.wait();
        assert!(!game.monsters[0].alerted);
        let learned = practiced(ESCAPE_LESSON as u32 * 66 * STEALTH_PRACTICE);
        assert_eq!(game.wanderer.stealth, learned);

        // Alerted by an attack and never observed in range: losing it is not an escape.
        game.monsters[0].alerted = true;
        game.wait();
        assert_eq!(game.wanderer.stealth, learned);

        // Death forgets tracking without paying for it.
        game.monsters[0].position = (x + 3, y);
        game.wait();
        assert!(game.monsters[0].tracking);
        game.monsters[0].position = (x + VIEW_RADIUS + 2, y);
        game.die();
        game.wait();
        assert_eq!(game.wanderer.stealth, learned);
    }

    #[test]
    fn landed_openings_teach_stealth_and_paid_techniques_teach_focus() {
        let mut openings = 0;
        for seed in 0..16 {
            let mut game = Game::composition_trial(seed, Maneuver::backstab_burst()).unwrap();
            game.use_technique(Dir::East);
            let result = game.last_attack.unwrap();
            if result.hit {
                openings += 1;
                assert!(result.opening);
                // Sentinel threat 133: three stealth lessons at 2 each, and 7 Focus paid.
                assert_eq!(game.wanderer.stealth, practiced(3 * 133 * 2));
                assert_eq!(game.wanderer.focus_skill, practiced(7 * 133));
                assert_eq!(game.wanderer.stealth.rank(), Rank::Novice, "one opening is not an entire rank");
                assert!(!game.log.lines().any(|line| line == "Your stealth is now practiced."));
            } else {
                assert_eq!(game.wanderer.stealth, crate::Skill::default());
                assert_eq!(game.wanderer.focus_skill, crate::Skill::default(), "a miss teaches no Focus");
            }
            let mut free = Game::composition_trial(seed, Maneuver::backstab_burst()).unwrap();
            free.sneaking = false;
            free.step(Dir::East);
            assert_eq!(free.wanderer.focus_skill, crate::Skill::default(), "free strikes spend no Focus");
            assert_eq!(free.wanderer.stealth, crate::Skill::default(), "no opening without concealment");
        }
        assert!(openings > 0);

        let mut game = Game::inferno_trial(42).unwrap();
        assert_eq!(game.monsters[0].weapon_threat(&game.wanderer, crate::Weapon::Staff), 10);
        game.unleash();
        assert_eq!(game.wanderer.focus_skill, practiced(18 * 10));
        let mut empty = Game::inferno_trial(42).unwrap();
        empty.monsters.clear();
        empty.unleash();
        assert_eq!(empty.wanderer.focus_skill, crate::Skill::default(), "an empty cast teaches nothing");
    }

    #[test]
    fn focus_practice_enlarges_the_pool_that_rest_refills() {
        let mut game = on_open_land(0, 3);
        game.home = game.position;
        game.wanderer.focus_skill.learn(u32::MAX, Rank::Practiced);
        assert_eq!(game.wanderer.max_focus(), MAX_FOCUS + 2);
        assert_eq!(game.wanderer.focus, MAX_FOCUS, "growth raises capacity, not current Focus");
        game.monsters.clear();
        assert_eq!(game.recover_focus(), Step::Waited);
        assert_eq!(game.wanderer.focus, MAX_FOCUS + 2);
        game.monsters.clear();
        assert_eq!(game.recover_focus(), Step::Blocked);
        let mut expert = crate::Skill::default();
        expert.learn(u32::MAX, Rank::Expert);
        assert_eq!(expert.focus_capacity(), MAX_FOCUS + 8);
    }

    #[test]
    fn stealth_practice_conceals_further_and_frees_some_sneaking_moves() {
        let mut game = on_open_land(0, 3);
        let (x, y) = game.position;
        still_threat(&mut game, (x + 2, y), 9);
        game.sneaking = true;
        assert_eq!(game.concealment(), 6);
        assert!(game.monsters[0].detects(game.position, game.concealment()));
        game.wanderer.stealth.learn(u32::MAX, Rank::Expert);
        assert_eq!(game.concealment(), 8);
        assert!(!game.monsters[0].detects(game.position, game.concealment()));
        game.sneaking = false;
        assert_eq!(game.concealment(), 0);

        let mut game = on_open_land(0, 3);
        game.wanderer.stealth.learn(1_000, Rank::Expert);
        assert_eq!(game.wanderer.stealth.rank(), Rank::Practiced);
        game.toggle_sneak();
        let start = game.wanderer.focus;
        for i in 0..8 {
            game.monsters.clear();
            assert_eq!(game.step(if i % 2 == 0 { Dir::East } else { Dir::West }), Step::Moved);
        }
        assert_eq!(game.wanderer.focus, start - 6, "every fourth sneaking move is free");
        game.monsters.clear();
        game.wanderer.focus = 0;
        game.sneak_moves = 3;
        assert!(game.can_step(Dir::East), "a free move needs no Focus");
        assert_eq!(game.step(Dir::East), Step::Moved);
        assert_eq!(game.wanderer.focus, 0);
        game.monsters.clear();
        assert!(!game.can_step(Dir::West), "the next one does");
    }

    /// A scripted game's exact course, pinned: the same seed and moves must give the same monsters in the
    /// same places with the same hit points on every machine, since spawns draw from evolved pools weighted by
    /// fitness.
    ///
    /// If this fails and nobody meant to change the rules, spawning, or evolution, the sim's arithmetic came
    /// out differently on this platform or toolchain (see `evolution_is_pinned_to_exact_bits` in `life`, which
    /// narrows it down if it fails too). Find the cause; don't update the constant. Update it only when the
    /// game was changed on purpose, and say so in the commit.
    #[test]
    fn scripted_play_is_pinned_to_exact_bits() {
        use crate::hash::combine;
        let mut h = 0;
        for seed in [0, 42] {
            let mut game = Game::new(seed);
            for i in 0..240 {
                game.step(Dir::ALL[(i / 9) % 4]);
                let (x, y) = game.position();
                h = [x as u64, y as u64, game.turn(), u64::from(game.deaths()), game.wanderer().hp as u64]
                    .into_iter()
                    .fold(h, combine);
                for m in game.monsters() {
                    h = [m.id.0, m.position.0 as u64, m.position.1 as u64, m.hp as u64].into_iter().fold(h, combine);
                    h = m.genome.genes().iter().fold(h, |h, v| combine(h, u64::from(v.to_bits())));
                }
            }
        }
        assert_eq!(
            h, 0xcd3b_2621_7ed4_cc68,
            "the sim's arithmetic changed; see this test's doc before touching the constant"
        );
    }
}

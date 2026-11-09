//! The game state the frontend drives: one wanderer walking a world, the region it is in, the monsters
//! around it, a turn counter, and a message log.
//!
//! Everything here is a pure function of the seed and the actions taken so far, so the same seed and the
//! same actions always give the same game. Randomness in a turn comes from a stream keyed by `(seed,
//! turn)`.
//!
//! A turn is the wanderer's action (a step, a bump attack, or a wait), then every monster's actions, then
//! healing, despawning, and spawning. Monsters act by temperament and bank their speed as energy, so a fast
//! one sometimes acts twice in a turn and a slow one sometimes not at all.
//!
//! Coordinates follow the terrain's: `x` grows eastward and `y` grows southward, down the screen.

use std::collections::{HashMap, VecDeque};

use crate::biome::Biome;
use crate::clade::{Clade, CladeId, clades};
use crate::hash::Rng;
use crate::life::{RegionLife, Species, SpeciesId, TraitDelta, compare};
use crate::monster::{ACTION_COST, MAX_ACTIONS, Monster, MonsterId, Wanderer, damage};
use crate::phenotype::{Phenotype, Temperament};
use crate::region::{Region, RegionCache, RegionId};
use crate::sprite::Sprite;
use crate::world::{Tile, World};

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

/// The offsets at Chebyshev distance exactly `r` from the center: the border of a `(2r+1)`-wide square.
fn ring(r: i64) -> impl Iterator<Item = (i64, i64)> {
    (-r..=r).flat_map(move |dy| {
        let edge = dy.abs() == r;
        // Top and bottom rows are whole; the rows between only have their two ends.
        let step = if edge || r == 0 { 1 } else { 2 * r as usize };
        (-r..=r).step_by(step).map(move |dx| (dx, dy))
    })
}

/// A bounded log of messages, oldest first, holding the last [`LOG_LIMIT`] lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Log {
    lines: VecDeque<String>,
    total: u64,
}

impl Log {
    /// Appends a line, dropping the oldest if the log is full.
    pub fn push(&mut self, line: impl Into<String>) {
        if self.lines.len() == LOG_LIMIT {
            self.lines.pop_front();
        }
        self.lines.push_back(line.into());
        self.total += 1;
    }

    /// Lines held, oldest first.
    pub fn lines(&self) -> impl DoubleEndedIterator<Item = &str> + ExactSizeIterator {
        self.lines.iter().map(String::as_str)
    }

    /// The last `n` lines (fewer if the log is shorter), oldest first.
    pub fn last(&self, n: usize) -> impl Iterator<Item = &str> {
        self.lines.iter().skip(self.lines.len().saturating_sub(n)).map(String::as_str)
    }

    /// Number of lines held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the log holds no lines.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
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
/// How far from home a territorial monster will go after the wanderer, in tiles.
pub const TERRITORY: i64 = 4;
/// Chance an idle monster takes a step.
pub const WANDER_CHANCE: f32 = 0.5;
/// Mutation rate and step that make an individual from a pool genome.
const INDIVIDUAL: (f32, f32) = (0.5, 0.03);
/// Spawn tiles tried per spawn before giving up for the turn.
const SPAWN_TRIES: usize = 6;
/// Regions remembered for comparison, most recent last.
const VISITED_LIMIT: usize = 8;

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

/// How a species differs from its clade's population in a region visited earlier.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    /// The species compared against: the same clade, in the most recently visited other region that has it.
    pub against: SpeciesId,
    /// Gene by gene, this species minus that one.
    pub deltas: Vec<TraitDelta>,
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
    /// The life of every region looked at so far, a function of the seed and the region for now. Not part
    /// of the game's state yet; once regions grow from their neighbors, it will be.
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
}

/// Games are equal when their state is; what the caches happen to hold doesn't count.
impl PartialEq for Game {
    fn eq(&self, other: &Game) -> bool {
        (self.world, self.position, self.home, self.turn, self.wanderer, self.next_monster, self.deaths)
            == (other.world, other.position, other.home, other.turn, other.wanderer, other.next_monster, other.deaths)
            && (&self.region, &self.visited, &self.log, &self.monsters)
                == (&other.region, &other.visited, &other.log, &other.monsters)
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
        log.push(format!("You wake in {}.", region.name));
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
        };
        game.populate(&mut Rng::keyed(seed, b"populate", &[]));
        game
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

    /// A species the game has met: any species of a region entered, or of a monster spawned.
    #[must_use]
    pub fn species(&self, id: SpeciesId) -> Option<&Species> {
        self.lives.get(&id.region)?.species_of(id.clade)
    }

    /// The species of the monsters in view, nearest first, without repeats.
    #[must_use]
    pub fn nearby_species(&self) -> Vec<SpeciesId> {
        let mut seen: Vec<(i64, SpeciesId)> = self
            .monsters
            .iter()
            .filter(|m| chebyshev(m.position, self.position) <= VIEW_RADIUS)
            .map(|m| (manhattan(m.position, self.position), m.species))
            .collect();
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

    /// Tries to step one tile in `dir`. A monster there is attacked instead. Otherwise the wanderer moves if
    /// the target is passable, naming the region if that changes, or stays put and logs what is in the way.
    /// Attacking or moving passes a turn, and the monsters act.
    pub fn step(&mut self, dir: Dir) -> Step {
        let (dx, dy) = dir.delta();
        let (x, y) = (self.position.0 + dx, self.position.1 + dy);
        if let Some(target) = self.monsters.iter().position(|m| m.position == (x, y)) {
            let mut rng = self.begin_turn();
            self.wanderer_attacks(target, &mut rng);
            self.end_turn(&mut rng);
            return Step::Attacked;
        }
        let target = self.world.tile(x, y).biome.biome();
        if !target.passable {
            // Biome names are sentence case ("Deep water"); mid-sentence they are common nouns.
            self.log.push(format!("The way is blocked by {}.", target.name.to_lowercase()));
            return Step::Blocked;
        }
        let mut rng = self.begin_turn();
        self.position = (x, y);
        self.update_region(true);
        self.end_turn(&mut rng);
        Step::Moved
    }

    /// Stays put for a turn, letting the monsters act.
    pub fn wait(&mut self) -> Step {
        let mut rng = self.begin_turn();
        self.end_turn(&mut rng);
        Step::Waited
    }

    /// Starts a turn: counts it and returns its random stream.
    fn begin_turn(&mut self) -> Rng {
        self.turn += 1;
        Rng::keyed(self.world.seed(), b"turnturn", &[self.turn])
    }

    /// Everything after the wanderer's action: monsters act, the wanderer heals, monsters come and go.
    fn end_turn(&mut self, rng: &mut Rng) {
        let deaths = self.deaths;
        self.monsters_act(rng);
        if self.deaths != deaths {
            return;
        }
        if self.turn.is_multiple_of(REGEN_TURNS) && self.wanderer.hp < self.wanderer.max_hp() {
            self.wanderer.hp += 1;
        }
        let here = self.position;
        self.monsters.retain(|m| chebyshev(m.position, here) <= DESPAWN_RADIUS);
        let productivity = self.local_life().productivity();
        if self.monsters.len() < capacity(productivity) && rng.chance(spawn_chance(productivity)) {
            self.spawn(rng);
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
            self.log.push(format!("You enter {}.", self.region.name));
        }
    }

    /// The life of region `id`, grown now if it hasn't been.
    fn life(&mut self, id: RegionId) -> &RegionLife {
        if !self.lives.contains_key(&id) {
            let region = self.regions.region(id).expect("ids come from region_at");
            let life = RegionLife::grow(self.world.seed(), &self.clades, &region);
            self.lives.insert(id, life);
        }
        &self.lives[&id]
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
            let genome = species.pool[rng.below(species.pool.len())].mutate(rng, INDIVIDUAL.0, INDIVIDUAL.1);
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
                home: (x, y),
                energy: rng.below(ACTION_COST as usize) as i32,
            };
            self.next_monster += 1;
            self.monsters.push(monster);
            return;
        }
    }

    /// The wanderer attacks monster `index`.
    fn wanderer_attacks(&mut self, index: usize, rng: &mut Rng) {
        let monster = &mut self.monsters[index];
        match damage(self.wanderer.stats.attack, monster.phenotype.stats.defense, rng) {
            None => self.log.push(format!("You miss the {}.", monster.name)),
            Some(d) => {
                monster.hp -= d;
                self.log.push(format!("You hit the {} for {d}.", monster.name));
                if monster.hp <= 0 {
                    self.log.push(format!("The {} dies.", monster.name));
                    self.monsters.remove(index);
                }
            }
        }
    }

    /// Every monster acts, in order of appearance, as often as its speed allows. Stops if the wanderer dies.
    fn monsters_act(&mut self, rng: &mut Rng) {
        let deaths = self.deaths;
        for i in 0..self.monsters.len() {
            self.monsters[i].energy += self.monsters[i].phenotype.stats.speed;
            let mut actions = 0;
            while self.monsters[i].energy >= ACTION_COST && actions < MAX_ACTIONS {
                self.monsters[i].energy -= ACTION_COST;
                actions += 1;
                self.monster_acts(i, rng);
                if self.deaths != deaths {
                    return;
                }
            }
        }
    }

    /// One action of monster `i`, by temperament: aggressive ones close in and attack, territorial ones
    /// attack near home, skittish ones flee, and all of them wander when nothing has their attention.
    fn monster_acts(&mut self, i: usize, rng: &mut Rng) {
        let m = &self.monsters[i];
        let (me, you, home) = (m.position, self.position, m.home);
        let adjacent = manhattan(me, you) == 1;
        let senses = chebyshev(me, you) <= m.phenotype.sense;
        match m.phenotype.temperament {
            Temperament::Aggressive => {
                if adjacent {
                    self.monster_attacks(i, rng);
                } else if senses {
                    self.approach(i, you);
                } else {
                    self.wander(i, rng);
                }
            }
            Temperament::Territorial => {
                let trespassing = senses && chebyshev(you, home) <= TERRITORY;
                if adjacent && trespassing {
                    self.monster_attacks(i, rng);
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
                    if !self.flee(i) && adjacent {
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
            None => self.log.push(format!("The {} misses you.", m.name)),
            Some(d) => {
                self.wanderer.hp -= d;
                self.log.push(format!("The {} {verb} you for {d}.", m.name));
                if self.wanderer.hp <= 0 {
                    self.die();
                }
            }
        }
    }

    /// The wanderer dies: time jumps, and they wake at home, whole, with the monsters gone.
    fn die(&mut self) {
        self.log.push("You fall.");
        self.deaths += 1;
        self.turn += DEATH_TURNS;
        self.position = self.home;
        self.wanderer.hp = self.wanderer.max_hp();
        self.monsters.clear();
        self.update_region(false);
        self.log.push(format!("You wake, much later, in {}.", self.region.name));
    }

    /// Whether monster `i` may stand on `(x, y)`: not the wanderer's tile, not another monster's, and terrain
    /// its body allows.
    fn open_to(&self, i: usize, (x, y): (i64, i64)) -> bool {
        (x, y) != self.position
            && !self.monsters.iter().enumerate().any(|(j, m)| j != i && m.position == (x, y))
            && self.monsters[i].phenotype.access.allows(self.world.tile(x, y).biome.biome())
    }

    /// Moves monster `i` one step to the open neighbor that best satisfies `better`, which compares a
    /// candidate's score against the current tile's. Returns whether it moved.
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
            let (position, turn, lines) = (game.position(), game.turn(), game.log().len());
            assert_eq!(game.step(dir), Step::Blocked);
            assert_eq!((game.position(), game.turn()), (position, turn));
            assert_eq!(game.log().len(), (lines + 1).min(LOG_LIMIT));
            assert!(game.log().lines().last().unwrap().starts_with("The way is blocked by "));
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

    /// The lines the last action added to the log.
    fn new_lines(game: &Game, before: u64) -> Vec<String> {
        let added = (game.log().total() - before) as usize;
        game.log().last(added).map(String::from).collect()
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
                    let entered: Vec<_> =
                        new_lines(&game, before).into_iter().filter(|l| l.starts_with("You enter ")).collect();
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
            log.push(i.to_string());
        }
        assert_eq!(log.len(), LOG_LIMIT);
        assert_eq!(log.lines().next(), Some("7"));
        let tail: Vec<_> = log.last(3).collect();
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
        let species = game.local_life().species[0].clone();
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
            home: at,
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
        assert!(first.starts_with("You hit the ") || first.starts_with("You miss the "), "{first}");
        assert!(name.is_empty() || first.contains(&name));
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
                assert!(lines.iter().any(|l| l.starts_with("The ") && l.ends_with(" dies.")), "{lines:?}");
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
        assert_eq!(game.wanderer().hp, game.wanderer().max_hp());
        assert!(game.monsters().is_empty());
        assert_eq!(game.region().id, home_region);
        let last = game.log().lines().last().unwrap();
        assert_eq!(last, format!("You wake, much later, in {}.", game.region().name));
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
    fn nearby_species_are_in_view_and_known() {
        let mut game = Game::new(42);
        play(&mut game, 200);
        for id in game.nearby_species() {
            let species = game.species(id).expect("a spawned monster's species is known");
            assert!(!species.name.is_empty() && !species.description.is_empty());
        }
    }

    #[test]
    fn ring_points_walk_the_ring() {
        for r in 1..5 {
            let points: std::collections::HashSet<_> = (0..8 * r).map(|k| ring_point(r, k)).collect();
            assert_eq!(points, ring(r).collect());
        }
    }
}

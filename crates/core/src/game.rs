//! The game state the frontend drives: one wanderer walking a world, a turn counter, and a message log.
//!
//! Everything here is a pure function of the seed and the moves made so far, so the same seed and the same
//! steps always give the same position, turn, and log.
//!
//! Coordinates follow the terrain's: `x` grows eastward and `y` grows southward, down the screen.

use std::collections::VecDeque;

use crate::biome::Biome;
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

/// What a [`Game::step`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The wanderer moved and a turn passed.
    Moved,
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
}

impl Log {
    /// Appends a line, dropping the oldest if the log is full.
    pub fn push(&mut self, line: impl Into<String>) {
        if self.lines.len() == LOG_LIMIT {
            self.lines.pop_front();
        }
        self.lines.push_back(line.into());
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
}

/// One game in progress: a world, the wanderer in it, the turn count, and what has been said.
#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    world: World,
    position: (i64, i64),
    turn: u64,
    log: Log,
}

impl Game {
    /// A new game in the world for `seed`, with the wanderer on the land tile nearest the origin.
    ///
    /// # Panics
    ///
    /// If [`find_land`] finds no land near the origin, which no seed tried so far comes close to.
    #[must_use]
    pub fn new(seed: u64) -> Game {
        let world = World::new(seed);
        let position = find_land(&world, 0, 0).expect("no land near the origin");
        let mut game = Game { world, position, turn: 0, log: Log::default() };
        let name = game.biome().name.to_lowercase();
        game.log.push(format!("You wake. The land here is {name}."));
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

    /// Successful moves so far.
    #[must_use]
    pub const fn turn(&self) -> u64 {
        self.turn
    }

    /// The message log.
    #[must_use]
    pub const fn log(&self) -> &Log {
        &self.log
    }

    /// Tries to step one tile in `dir`. Moves and passes a turn if the target is passable, saying so if the
    /// biome changes; otherwise stays put and logs what is in the way.
    pub fn step(&mut self, dir: Dir) -> Step {
        let (dx, dy) = dir.delta();
        let (x, y) = (self.position.0 + dx, self.position.1 + dy);
        let from = self.tile().biome;
        let to = self.world.tile(x, y).biome;
        let target = to.biome();
        if !target.passable {
            self.log.push(format!("The way is blocked by {}.", target.name.to_lowercase()));
            return Step::Blocked;
        }
        self.position = (x, y);
        self.turn += 1;
        if to != from {
            self.log.push(format!("You enter the {}.", target.name));
        }
        Step::Moved
    }
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
    fn moves_pass_turns() {
        let mut game = Game::new(4);
        let start = game.position();
        let mut moved = 0;
        for dir in [Dir::East, Dir::South, Dir::West, Dir::North] {
            if game.step(dir) == Step::Moved {
                moved += 1;
            }
        }
        assert_eq!(game.turn(), moved);
        // Each move is one tile, so going round a square from a fully open start comes home.
        if moved == 4 {
            assert_eq!(game.position(), start);
        }
    }

    #[test]
    fn biome_messages_fire_only_on_change() {
        for seed in [0, 1, 5] {
            let mut game = Game::new(seed);
            let mut changes = 0;
            // Stop before the log fills, so a new line always shows as a longer log.
            for i in 0..2_000 {
                if game.log().len() == LOG_LIMIT {
                    break;
                }
                let (from, lines) = (game.tile().biome, game.log().len());
                let dir = Dir::ALL[(i / 40) % 4];
                if game.step(dir) == Step::Moved {
                    let after = game.tile().biome;
                    assert_eq!(game.log().len() > lines, after != from, "seed {seed} step {i}");
                    if after != from {
                        changes += 1;
                        let line = game.log().lines().last().unwrap();
                        assert_eq!(line, format!("You enter the {}.", after.biome().name));
                    }
                }
            }
            assert!(changes > 0, "seed {seed}: never changed biome, so the test proved nothing");
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
    }

    #[test]
    fn same_seed_same_game() {
        let route = |seed| {
            let mut game = Game::new(seed);
            for i in 0..500 {
                game.step(Dir::ALL[(i * 7 + i / 13) % 4]);
            }
            game
        };
        for seed in [0, 4, 42] {
            assert_eq!(route(seed), route(seed));
        }
    }
}

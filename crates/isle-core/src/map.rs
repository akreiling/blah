//! Procedural surface terrain and towns, with an authored dungeon beneath their stairs.

use std::sync::OnceLock;

use crate::town::Town;
use blah_core::{Biome, BiomeId, Rng, World};
use serde::{Deserialize, Serialize};

/// A cell position, `(x, y)`, with `y` growing down the screen.
pub type Pos = (i32, i32);

/// What a cell is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    /// Stone that blocks movement and sight.
    Wall,
    /// Bare ground, street, or flagstones.
    Floor,
    /// Open grass.
    Grass,
    /// A tree: passable, but it blocks sight.
    Tree,
    /// Water too deep to wade.
    Water,
    /// An open doorway.
    Door,
    /// Stairs leading down.
    StairsDown,
    /// Stairs leading up.
    StairsUp,
    /// A shop counter.
    Counter,
    /// The temple altar.
    Altar,
    /// Open ground of one of the world's biomes.
    Wild(BiomeId),
    /// Bare rock that blocks movement and sight.
    Rock,
}

impl Tile {
    /// Whether a walker can stand here.
    pub fn walkable(self) -> bool {
        match self {
            Tile::Wall | Tile::Water | Tile::Counter | Tile::Altar | Tile::Rock => false,
            Tile::Wild(biome) => biome.biome().passable,
            _ => true,
        }
    }

    /// Whether this cell hides what lies beyond it.
    pub const fn opaque(self) -> bool {
        matches!(self, Tile::Wall | Tile::Tree | Tile::Rock)
    }
}

/// Which map a thing is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LevelId {
    /// The town and the fields around it.
    Surface,
    /// The halls beneath the watchtower.
    Undercroft,
}

impl LevelId {
    /// Both levels, in save order.
    pub const ALL: [LevelId; 2] = [LevelId::Surface, LevelId::Undercroft];

    pub(crate) const fn index(self) -> usize {
        match self {
            LevelId::Surface => 0,
            LevelId::Undercroft => 1,
        }
    }
}

/// A part of a map where a service is offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// Nothing special.
    Open,
    /// Inside the outfitter's shop.
    Shop,
    /// Inside the temple.
    Temple,
}

/// What kind of creature a spawn point raises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnKind {
    /// Woods beasts on the surface.
    Wild,
    /// The undercroft's rank and file.
    Deep,
    /// The thing at the bottom.
    Lair,
}

/// One authored map.
#[derive(Debug)]
pub struct Level {
    /// Which level this is.
    pub id: LevelId,
    /// Its name, as the log says it.
    pub name: &'static str,
    /// Cells across.
    pub width: i32,
    /// Cells down.
    pub height: i32,
    tiles: Vec<Tile>,
    zones: Vec<Zone>,
    /// Where creatures rise, in map order.
    pub spawns: Vec<(Pos, SpawnKind)>,
    /// Where a new character stands, if on this level.
    pub start: Option<Pos>,
    /// Where the fallen wake, if on this level.
    pub respawn: Option<Pos>,
    /// The stairs down, if any.
    pub stairs_down: Option<Pos>,
    /// The stairs up, if any.
    pub stairs_up: Option<Pos>,
}

const UNDERCROFT: [&str; 19] = [
    "##################################################",
    "#<....#..........#...............#...............#",
    "#.....#..m.......#.......m.......#...............#",
    "#.....+..........+...............#.......m.......#",
    "#.....#..........#...............+...............#",
    "###+###..........#...............#...............#",
    "#.....############.......#########.......m.......#",
    "#.................#......#.......#...............#",
    "#....m............#......#.......#######+#########",
    "#.................+......#...............#.......#",
    "#######+###########......+.......m.......#...B...#",
    "#.................#......#...............+.......#",
    "#.........m.......#......#################.......#",
    "#.................#......#...............#.......#",
    "#.................#...m..#...............#########",
    "#######.###########......#.......m.......#.......#",
    "#.................+......+...............+...m...#",
    "#..m..............#......#...............#.......#",
    "##################################################",
];

impl Level {
    fn parse(id: LevelId, name: &'static str, rows: &[&str]) -> Level {
        let height = rows.len() as i32;
        let width = rows[0].len() as i32;
        let mut level = Level {
            id,
            name,
            width,
            height,
            tiles: Vec::with_capacity((width * height) as usize),
            zones: Vec::with_capacity((width * height) as usize),
            spawns: Vec::new(),
            start: None,
            respawn: None,
            stairs_down: None,
            stairs_up: None,
        };
        for (y, row) in rows.iter().enumerate() {
            assert_eq!(row.len() as i32, width, "{name} row {y} is ragged");
            for (x, c) in row.chars().enumerate() {
                let at = (x as i32, y as i32);
                let (tile, zone) = match c {
                    '#' => (Tile::Wall, Zone::Open),
                    '.' => (Tile::Floor, Zone::Open),
                    ',' => (Tile::Grass, Zone::Open),
                    'T' => (Tile::Tree, Zone::Open),
                    '~' => (Tile::Water, Zone::Open),
                    '+' => (Tile::Door, Zone::Open),
                    '=' => (Tile::Counter, Zone::Shop),
                    '_' => (Tile::Altar, Zone::Temple),
                    'S' => (Tile::Floor, Zone::Shop),
                    'H' => (Tile::Floor, Zone::Temple),
                    'R' => {
                        level.respawn = Some(at);
                        (Tile::Floor, Zone::Temple)
                    }
                    '@' => {
                        level.start = Some(at);
                        (Tile::Floor, Zone::Open)
                    }
                    '>' => {
                        level.stairs_down = Some(at);
                        (Tile::StairsDown, Zone::Open)
                    }
                    '<' => {
                        level.stairs_up = Some(at);
                        (Tile::StairsUp, Zone::Open)
                    }
                    'm' | 'B' => {
                        let kind = match (c, id) {
                            ('B', _) => SpawnKind::Lair,
                            (_, LevelId::Surface) => SpawnKind::Wild,
                            _ => SpawnKind::Deep,
                        };
                        level.spawns.push((at, kind));
                        (if id == LevelId::Surface { Tile::Grass } else { Tile::Floor }, Zone::Open)
                    }
                    other => panic!("{name} has an unknown map character {other:?}"),
                };
                level.tiles.push(tile);
                level.zones.push(zone);
            }
        }
        level
    }

    /// The authored dungeon, built once. The surface is procedural; use [`crate::Game::level`].
    ///
    /// # Panics
    ///
    /// If asked for the procedural surface.
    pub fn get(id: LevelId) -> &'static Level {
        assert_eq!(id, LevelId::Undercroft, "the surface is procedural");
        static UNDER: OnceLock<Level> = OnceLock::new();
        UNDER.get_or_init(|| Level::parse(LevelId::Undercroft, "the Undercroft", &UNDERCROFT))
    }

    fn index(&self, (x, y): Pos) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.width && y < self.height).then(|| (y * self.width + x) as usize)
    }

    /// The cell at `at`; outside the map is wall.
    pub fn tile(&self, at: Pos) -> Tile {
        self.index(at).map_or(Tile::Wall, |i| self.tiles[i])
    }

    /// The service zone at `at`.
    pub fn zone(&self, at: Pos) -> Zone {
        self.index(at).map_or(Zone::Open, |i| self.zones[i])
    }

    /// Cells on the map.
    pub fn cells(&self) -> usize {
        self.tiles.len()
    }

    pub(crate) fn cell_index(&self, at: Pos) -> Option<usize> {
        self.index(at)
    }

    /// Whether `from` can see `to` within `radius` cells: no opaque cell strictly between them.
    pub fn sees(&self, from: Pos, to: Pos, radius: i32) -> bool {
        if distance(from, to) > radius {
            return false;
        }
        line(from, to).iter().skip(1).take_while(|&&p| p != to).all(|&p| !self.tile(p).opaque())
    }
}

/// How much of a biome's ground is trees (passable, blocking sight) and rock (blocking both).
fn cover(biome: &Biome) -> (f32, f32) {
    match biome.name {
        "Jungle" => (0.55, 0.0),
        "Forest" => (0.45, 0.0),
        "Taiga" => (0.40, 0.0),
        "Swamp" => (0.12, 0.0),
        "Savanna" => (0.04, 0.0),
        "Grassland" => (0.03, 0.0),
        "Hills" => (0.05, 0.08),
        "Mountains" => (0.0, 0.25),
        "Badlands" => (0.0, 0.12),
        "Tundra" => (0.0, 0.03),
        _ => (0.0, 0.0),
    }
}

/// Blah's world, shifted so a dry site for the starting town surrounds the arrival square.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terrain {
    seed: u64,
    offset: (i64, i64),
}

impl Terrain {
    /// The land for `seed`.
    ///
    /// # Panics
    ///
    /// If no suitable dry town site can be found near the origin.
    pub fn new(seed: u64) -> Terrain {
        let (x, y) = Town::home_site(seed);
        Terrain { seed, offset: (x - i64::from(Town::START.0), y - i64::from(Town::START.1)) }
    }

    /// The world coordinates of `at`.
    pub fn world_pos(&self, (x, y): Pos) -> (i64, i64) {
        (i64::from(x) + self.offset.0, i64::from(y) + self.offset.1)
    }

    /// The biome under `at`.
    pub fn biome(&self, at: Pos) -> BiomeId {
        let (x, y) = self.world_pos(at);
        World::new(self.seed).tile(x, y).biome
    }

    /// The cell at `at`.
    pub fn tile(&self, at: Pos) -> Tile {
        let id = self.biome(at);
        let biome = id.biome();
        if !biome.passable {
            return if biome.water { Tile::Water } else { Tile::Rock };
        }
        let (trees, rocks) = cover(biome);
        if trees + rocks == 0.0 {
            return Tile::Wild(id);
        }
        let (x, y) = self.world_pos(at);
        let roll = Rng::keyed(self.seed, b"isle-cov", &[x as u64, y as u64]).unit();
        if roll < trees {
            Tile::Tree
        } else if roll < trees + rocks {
            Tile::Rock
        } else {
            Tile::Wild(id)
        }
    }
}

/// The active map: generated towns over terrain, or the authored dungeon.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    /// Current map identity.
    pub id: LevelId,
    /// Width of control inspection windows, or of the dungeon.
    pub width: i32,
    /// Height of control inspection windows, or of the dungeon.
    pub height: i32,
    /// The active town's arrival square, if standing in a town.
    pub start: Option<Pos>,
    /// The active town's revival spot.
    pub respawn: Option<Pos>,
    /// The active town's dungeon entrance.
    pub stairs_down: Option<Pos>,
    /// The dungeon exit.
    pub stairs_up: Option<Pos>,
    terrain: Terrain,
    towns: [Option<Town>; 10],
}

impl Place {
    pub(crate) fn new(id: LevelId, terrain: Terrain, towns: [Option<Town>; 10], at: Pos) -> Place {
        let town = towns.iter().flatten().find(|t| t.contains(at)).copied();
        let under = Level::get(LevelId::Undercroft);
        let surface = id == LevelId::Surface;
        Place {
            id,
            terrain,
            towns,
            width: if surface { 50 } else { under.width },
            height: if surface { 28 } else { under.height },
            start: if surface { town.map(Town::start) } else { None },
            respawn: if surface { town.map(Town::respawn) } else { None },
            stairs_down: if surface { town.map(Town::stairs) } else { None },
            stairs_up: if surface { None } else { under.stairs_up },
        }
    }

    /// The town covering a surface cell.
    pub fn town_at(&self, at: Pos) -> Option<Town> {
        (self.id == LevelId::Surface).then(|| self.towns.iter().flatten().find(|t| t.contains(at)).copied()).flatten()
    }

    /// Generated towns in the currently inspected neighborhood, including the starting town.
    pub fn towns(&self) -> impl Iterator<Item = Town> + '_ {
        self.towns.iter().flatten().copied().filter(|_| self.id == LevelId::Surface)
    }

    /// Whether a coordinate is on this map.
    pub fn contains(&self, at: Pos) -> bool {
        self.id == LevelId::Surface || Level::get(LevelId::Undercroft).index(at).is_some()
    }

    /// Whether a cell belongs to a safe town or to the authored dungeon.
    pub fn authored(&self, at: Pos) -> bool {
        if self.id == LevelId::Surface {
            self.town_at(at).is_some()
        } else {
            Level::get(LevelId::Undercroft).index(at).is_some()
        }
    }

    /// The cell at a coordinate, including wilderness and any generated town.
    pub fn tile(&self, at: Pos) -> Tile {
        if self.id == LevelId::Undercroft {
            Level::get(LevelId::Undercroft).tile(at)
        } else {
            self.town_at(at).map_or_else(|| self.terrain.tile(at), |town| town.cell(at).0)
        }
    }

    /// A service zone at a coordinate.
    pub fn zone(&self, at: Pos) -> Zone {
        if self.id == LevelId::Undercroft {
            Zone::Open
        } else {
            self.town_at(at).map_or(Zone::Open, |town| town.cell(at).1)
        }
    }

    pub(crate) fn cell_index(&self, at: Pos) -> Option<usize> {
        if self.id == LevelId::Surface { None } else { Level::get(LevelId::Undercroft).cell_index(at) }
    }

    /// The town, biome, or dungeon name at a coordinate.
    pub fn name_at(&self, at: Pos) -> String {
        if self.id == LevelId::Undercroft {
            Level::get(LevelId::Undercroft).name.to_owned()
        } else {
            self.town_at(at).map_or_else(|| self.terrain.biome(at).biome().name.to_owned(), Town::name)
        }
    }

    /// Whether `from` can see `to` within `radius` cells.
    pub fn sees(&self, from: Pos, to: Pos, radius: i32) -> bool {
        distance(from, to) <= radius
            && line(from, to).iter().skip(1).take_while(|&&p| p != to).all(|&p| !self.tile(p).opaque())
    }
}
/// Steps between two cells when diagonal steps count as one.
pub fn distance(a: Pos, b: Pos) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// The cells from `a` to `b` inclusive, by Bresenham's line.
pub fn line(a: Pos, b: Pos) -> Vec<Pos> {
    let (mut x, mut y) = a;
    let (dx, dy) = ((b.0 - x).abs(), -(b.1 - y).abs());
    let (sx, sy) = ((b.0 - x).signum(), (b.1 - y).signum());
    let mut err = dx + dy;
    let mut cells = vec![(x, y)];
    while (x, y) != b {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        cells.push((x, y));
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    #[test]
    fn every_walkable_dungeon_cell_is_reachable() {
        let level = Level::get(LevelId::Undercroft);
        let start = level.stairs_up.unwrap();
        let mut reached = HashSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some((x, y)) = queue.pop_front() {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let next = (x + dx, y + dy);
                    if level.tile(next).walkable() && reached.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
        }
        for y in 0..level.height {
            for x in 0..level.width {
                assert!(!level.tile((x, y)).walkable() || reached.contains(&(x, y)));
            }
        }
        assert!(level.spawns.iter().any(|&(_, k)| k == SpawnKind::Lair));
    }

    #[test]
    fn walls_block_sight_but_not_the_cell_itself() {
        let level = Level::get(LevelId::Undercroft);
        assert!(level.sees((1, 1), (3, 1), 6));
        assert!(!level.sees((1, 1), (8, 1), 6));
        assert!(level.sees((1, 1), (6, 1), 6));
    }

    #[test]
    fn lines_run_end_to_end() {
        assert_eq!(line((0, 0), (3, 1)), vec![(0, 0), (1, 0), (2, 1), (3, 1)]);
        assert_eq!(line((2, 2), (2, 2)), vec![(2, 2)]);
        assert_eq!(distance((0, 0), (3, -5)), 5);
    }

    #[test]
    fn wilderness_still_uses_the_seeded_world() {
        for seed in [0, 1, 7, 42, u64::MAX] {
            let game = crate::Game::new(seed);
            let world = World::new(seed);
            for y in [-1000, -16, -1, 100, 1000] {
                for x in [-1000, -16, -1, 100, 1000] {
                    let at = (x, y);
                    let (wx, wy) = game.level().terrain.world_pos(at);
                    let biome = world.tile(wx, wy).biome;
                    assert_eq!(game.level().terrain.biome(at), biome);
                    if !game.level().authored(at) {
                        assert_eq!(game.level().name_at(at), biome.biome().name);
                        if !biome.biome().passable {
                            assert!(!game.level().tile(at).walkable());
                        }
                    }
                }
            }
        }
    }
}

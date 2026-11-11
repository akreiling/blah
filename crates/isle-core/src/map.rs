//! Procedural wilderness around the authored town of Harrowmere and the undercroft beneath it.

use std::sync::OnceLock;

use blah_core::{Biome, BiomeId, Rng, World, find_land};
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

const SURFACE: [&str; 28] = [
    "TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT",
    "T,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,~~~~~~,,,,,T",
    "T,,###########,,,,,,,,,,,,,,,,#########,~~~~~~~,,T",
    "T,,#SSSSSSSSS#,,,,,,,,,,,,,,,,#HHHHHHH#,,~~~~~~,,T",
    "T,,#S=======S#,,,,,,,,,,,,,,,,#HHH_HHH#,,,~~~~,,,T",
    "T,,#SSSSSSSSS#,,,,,,,,,,,,,,,,#HHHRHHH#,,,,~~,,,,T",
    "T,,#SSSSSSSSS#,,,,,,,,,,,,,,,,#HHHHHHH#,,,,,,,,,,T",
    "T,,#####+#####,,,,,,,,,,,,,,,,####+####,,,,,,,,,,T",
    "T,,,,,,,.,,,,,,,,,,,,,,,,,,,,,,,,,.,,,,,,,,,,,,,,T",
    "T,,,,,,,..............@............,,,,,,,,,,,,,,T",
    "T,,,,,,,,,,,,,,,,,,,,,.,,,,,,,,,,,,,,,,,,,,,,,,,,T",
    "T,,,,,,,,,,,,,,,,,,,,,.,,,,,,,,,,,,,,,,,T,,,,,,,,T",
    "T,,,,,T,,,,,,,,,,,,,,,.,,,,,,,,,,,,,,,,,,,,,,,,,,T",
    "T,,,,,,,,,,,,,,,,,,,,,.,,,,,,,,,,,,T,,,,,,,,,,,,,T",
    "T,,,,,,,,,,,,,,,,,,,,,.,,,,,,,,,,,,,,,,,,,,,,,,,,T",
    "T,,,,,,,TT,,,,,,,,,,,,.,,,,,,,,,,,,,,,,,,,,TT,,,,T",
    "T,,,,,,TTTT,,,,,,,,,,,.,,,,,,,,,,,,,,,,,,,TTTT,,,T",
    "T,,,,,TTmTTT,,,,,,,,,,.,,,,,,,,,,,,,,,,,,TTTmTT,,T",
    "T,,,,,,TTTT,,,,,,,,,,,.,,,,,,,,,,,,,,,,,,,TTTT,,,T",
    "T,,,,,,,TT,,,,,,,,,,,,.,,,,,,,,,,,,,,,,,,,,TT,,,,T",
    "T,,,,,,,,,,,,,,,,,,,,,.,,,,,,,,,,,,,,,,,,,,,,,,,,T",
    "T,,,,,,,,,,,,,,,,,,,,,..,,,,,,,,,,,,,,,,,,,,,,,,,T",
    "T,,,,,,,,,,,,,,,,,,,,###+###,,,,,,,,,,,,,,,,,,,,,T",
    "T,,,,,,,,,,,,,,,,,,,,#.....#,,,,,,,,,,,,,,,,,,,,,T",
    "T,,,,,,,,,,,,,,,,,,,,#..>..#,,,,,,,,,,,,,,,,,,,,,T",
    "T,,,,,,,,,,,,,,,,,,,,#.....#,,,,,,,,,,,,,,,,,,,,,T",
    "T,,,,,,,,,,,,,,,,,,,,#######,,,,,,,,,,,,,,,,,,,,,T",
    "TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT",
];

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

    /// The level `id`, built once.
    pub fn get(id: LevelId) -> &'static Level {
        static LEVELS: OnceLock<[Level; 2]> = OnceLock::new();
        let levels = LEVELS.get_or_init(|| {
            [
                Level::parse(LevelId::Surface, "Harrowmere", &SURFACE),
                Level::parse(LevelId::Undercroft, "the Undercroft", &UNDERCROFT),
            ]
        });
        &levels[id.index()]
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

/// The endless land around Harrowmere: blah's world, shifted so the town's start sits on the land nearest
/// the world's origin. A pure function of the seed, so only the seed and the shift are saved.
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
    /// If the world has no land near its origin, which no seed tried so far comes close to.
    pub fn new(seed: u64) -> Terrain {
        let world = World::new(seed);
        let start = Level::get(LevelId::Surface).start.expect("the surface has a start");
        let (x, y) = find_land(&world, 0, 0).expect("land near the origin");
        Terrain { seed, offset: (x - i64::from(start.0), y - i64::from(start.1)) }
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

/// A level as play sees it: its authored cells, and on the surface the terrain beyond them.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    level: &'static Level,
    terrain: Option<Terrain>,
}

impl std::ops::Deref for Place {
    type Target = Level;

    fn deref(&self) -> &Level {
        self.level
    }
}

impl Place {
    pub(crate) fn new(level: &'static Level, terrain: Option<Terrain>) -> Place {
        Place { level, terrain }
    }

    /// Whether `at` is part of this place, authored or generated.
    pub fn contains(&self, at: Pos) -> bool {
        self.terrain.is_some() || self.level.index(at).is_some()
    }

    /// Whether `at` is one of the level's authored cells.
    pub fn authored(&self, at: Pos) -> bool {
        self.level.index(at).is_some()
    }

    /// The cell at `at`; outside an authored level with no terrain is wall.
    pub fn tile(&self, at: Pos) -> Tile {
        match (self.level.index(at), self.terrain) {
            (Some(i), _) => self.level.tiles[i],
            (None, Some(terrain)) => terrain.tile(at),
            (None, None) => Tile::Wall,
        }
    }

    /// The name of where `at` lies: the level's, or the biome's out in the wild.
    pub fn name_at(&self, at: Pos) -> &'static str {
        match (self.level.index(at), self.terrain) {
            (None, Some(terrain)) => terrain.biome(at).biome().name,
            _ => self.level.name,
        }
    }

    /// Whether `from` can see `to` within `radius` cells: no opaque cell strictly between them.
    pub fn sees(&self, from: Pos, to: Pos, radius: i32) -> bool {
        if distance(from, to) > radius {
            return false;
        }
        line(from, to).iter().skip(1).take_while(|&&p| p != to).all(|&p| !self.tile(p).opaque())
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

    fn reachable(level: &Level, from: Pos) -> HashSet<Pos> {
        let mut seen = HashSet::from([from]);
        let mut queue = VecDeque::from([from]);
        while let Some((x, y)) = queue.pop_front() {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let next = (x + dx, y + dy);
                    if level.tile(next).walkable() && seen.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
        }
        seen
    }

    #[test]
    fn every_walkable_cell_and_service_is_reachable() {
        let surface = Level::get(LevelId::Surface);
        let under = Level::get(LevelId::Undercroft);
        for (level, from) in [(surface, surface.start.unwrap()), (under, under.stairs_up.unwrap())] {
            let reached = reachable(level, from);
            for y in 0..level.height {
                for x in 0..level.width {
                    if level.tile((x, y)).walkable() {
                        assert!(reached.contains(&(x, y)), "{} cell {x},{y} is cut off", level.name);
                    }
                }
            }
        }
        assert!(surface.stairs_down.is_some() && surface.respawn.is_some());
        assert_eq!(surface.zone(surface.respawn.unwrap()), Zone::Temple);
        assert!(under.spawns.iter().any(|&(_, k)| k == SpawnKind::Lair));
        assert!(surface.spawns.iter().all(|&(_, k)| k == SpawnKind::Wild));
    }

    #[test]
    fn walls_and_trees_block_sight_but_not_the_cell_itself() {
        let surface = Level::get(LevelId::Surface);
        let start = surface.start.unwrap();
        assert!(surface.sees(start, (start.0 + 3, start.1), 6));
        assert!(!surface.sees(start, (start.0 + 7, start.1), 6), "beyond the radius");
        // The shop's south wall stands between the street and the counter.
        assert!(!surface.sees((5, 9), (5, 4), 6));
        assert!(surface.sees((5, 8), (5, 7), 6), "the wall itself is seen");
    }

    #[test]
    fn lines_run_end_to_end() {
        assert_eq!(line((0, 0), (3, 1)), vec![(0, 0), (1, 0), (2, 1), (3, 1)]);
        assert_eq!(line((2, 2), (2, 2)), vec![(2, 2)]);
        assert_eq!(distance((0, 0), (3, -5)), 5);
    }

    #[test]
    fn wilderness_uses_the_seeded_world_and_keeps_the_town_intact() {
        let town = Level::get(LevelId::Surface);
        for seed in [0, 1, 7, 42, u64::MAX] {
            let terrain = Terrain::new(seed);
            let place = Place::new(town, Some(terrain));
            let world = World::new(seed);
            let start = terrain.world_pos(town.start.unwrap());
            assert_eq!(Some(start), find_land(&world, 0, 0));
            for y in 0..town.height {
                for x in 0..town.width {
                    assert_eq!(place.tile((x, y)), town.tile((x, y)));
                }
            }
            for y in [-1000, -16, -1, 100, 1000] {
                for x in [-1000, -16, -1, 100, 1000] {
                    let pos = (x, y);
                    let (wx, wy) = terrain.world_pos(pos);
                    let biome = world.tile(wx, wy).biome;
                    assert_eq!(terrain.biome(pos), biome);
                    assert_eq!(place.name_at(pos), biome.biome().name);
                    assert_eq!(place.tile(pos), Terrain::new(seed).tile(pos));
                    if !biome.biome().passable {
                        assert!(!place.tile(pos).walkable());
                    } else if let Tile::Wild(id) = place.tile(pos) {
                        assert_eq!(id, biome);
                        assert!(place.tile(pos).walkable());
                    }
                    assert_eq!(place.zone(pos), Zone::Open);
                }
            }
        }
        let under = Place::new(Level::get(LevelId::Undercroft), None);
        assert!(!under.contains((-1, -1)));
        assert_eq!(under.tile((-1, -1)), Tile::Wall);
    }
}

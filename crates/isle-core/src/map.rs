//! The island's two places, drawn by hand: the town of Harrowmere and the undercroft beneath its old
//! watchtower. Each map character is one cell; the window draws each cell as two characters.

use std::sync::OnceLock;

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
}

impl Tile {
    /// Whether a walker can stand here.
    pub const fn walkable(self) -> bool {
        !matches!(self, Tile::Wall | Tile::Water | Tile::Counter | Tile::Altar)
    }

    /// Whether this cell hides what lies beyond it.
    pub const fn opaque(self) -> bool {
        matches!(self, Tile::Wall | Tile::Tree)
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
}

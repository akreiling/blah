//! Procedural surface terrain and towns, with a generated Deep beneath each town's stairs.

use crate::deep::Deep;
use crate::town::Town;
use crate::wildlife::DEEP;
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
    /// A tree: passable, and it thins sight by [`TREE_SIGHT`] cells rather than ending it.
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
        matches!(self, Tile::Wall | Tile::Rock)
    }
}

/// Cells of sight each tree between a viewer and its target costs, so woods thin sight rather than end it.
pub const TREE_SIGHT: i32 = 3;

/// Whether `to` lies within a round `radius` of `from`: inside the circle of radius `radius + ½`, so sight
/// reaches as far along a diagonal as along an axis instead of filling a square.
fn within(from: Pos, to: Pos, radius: i32) -> bool {
    let (dx, dy) = (i64::from(from.0 - to.0), i64::from(from.1 - to.1));
    let r = i64::from(radius);
    r >= 0 && dx * dx + dy * dy <= r * r + r
}

/// Sub-cell units per cell along a sight ray.
const RAY: i64 = 8;
/// Where sight rays aim inside a cell, in sub-cell units: its center, then four points toward its corners, so
/// a target half behind a trunk can still be glimpsed past it.
const AIMS: [(i64, i64); 5] = [(4, 4), (2, 2), (6, 2), (2, 6), (6, 6)];

/// Whether `from` sees `to` within a round `radius` (see [`within`]) over the cells `tile` gives. Rays run from the center of
/// each end to the aim points of the other; the target is seen if any ray crosses no wall or rock, and if the
/// target stays within `radius` less [`TREE_SIGHT`] for each tree that ray crosses. The cells at either end
/// never count. Both ends cast the same rays, so sight is mutual.
fn sight(from: Pos, to: Pos, radius: i32, tile: impl Fn(Pos) -> Tile) -> bool {
    if !within(from, to, radius) {
        return false;
    }
    if distance(from, to) <= 1 {
        return true;
    }
    let trees = (1..).take_while(|&k| within(from, to, radius - k * TREE_SIGHT)).count();
    // Rays stay inside the box spanned by both cells, so each tile there is looked up at most once.
    let (x0, y0) = (from.0.min(to.0), from.1.min(to.1));
    let width = (from.0 - to.0).unsigned_abs() as usize + 1;
    let mut seen: Vec<Option<Tile>> = vec![None; width * ((from.1 - to.1).unsigned_abs() as usize + 1)];
    let mut at = |p: Pos| *seen[(p.1 - y0) as usize * width + (p.0 - x0) as usize].get_or_insert_with(|| tile(p));
    let mut clear = |a: Pos, b: Pos, aim: (i64, i64)| {
        let start = (i64::from(a.0) * RAY + RAY / 2, i64::from(a.1) * RAY + RAY / 2);
        let end = (i64::from(b.0) * RAY + aim.0, i64::from(b.1) * RAY + aim.1);
        let (dx, dy) = (end.0 - start.0, end.1 - start.1);
        let steps = dx.abs().max(dy.abs());
        let (mut last, mut crossed) = (a, 0);
        for i in 1..steps {
            let cell =
                ((start.0 + dx * i / steps).div_euclid(RAY) as i32, (start.1 + dy * i / steps).div_euclid(RAY) as i32);
            if cell == last || cell == a || cell == b {
                continue;
            }
            last = cell;
            match at(cell) {
                t if t.opaque() => return false,
                Tile::Tree => {
                    crossed += 1;
                    if crossed > trees {
                        return false;
                    }
                }
                _ => {}
            }
        }
        true
    };
    AIMS.iter().any(|&aim| clear(from, to, aim) || clear(to, from, aim))
}

/// Which map a thing is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LevelId {
    /// The surface: towns and the wilds between them.
    Surface,
    /// A floor of the Deep beneath a town's stairs, from depth 1.
    Deep(Town, u32),
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

/// The active map: generated towns over terrain, or the Deep under one of them.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    /// Current map identity.
    pub id: LevelId,
    /// Width of control inspection windows, or of the Deep.
    pub width: i32,
    /// Height of control inspection windows, or of the Deep.
    pub height: i32,
    /// The active town's arrival square, if standing in a town.
    pub start: Option<Pos>,
    /// The active town's revival spot.
    pub respawn: Option<Pos>,
    /// The active town's or floor's stairs down.
    pub stairs_down: Option<Pos>,
    /// The Deep's stairs up.
    pub stairs_up: Option<Pos>,
    terrain: Terrain,
    towns: [Option<Town>; 10],
    deep: Option<Deep>,
}

impl Place {
    pub(crate) fn new(id: LevelId, terrain: Terrain, towns: [Option<Town>; 10], at: Pos) -> Place {
        let deep = match id {
            LevelId::Surface => None,
            LevelId::Deep(town, depth) => Some(Deep::new(town, depth)),
        };
        let town = towns.iter().flatten().find(|t| t.contains(at)).copied().filter(|_| deep.is_none());
        Place {
            id,
            terrain,
            towns,
            width: if deep.is_some() { Deep::WIDTH } else { 50 },
            height: if deep.is_some() { Deep::HEIGHT } else { 28 },
            start: town.map(Town::start),
            respawn: town.map(Town::respawn),
            stairs_down: town.map(Town::stairs).or(deep.map(|d| d.stairs_down())),
            stairs_up: deep.map(|d| d.stairs()),
            deep,
        }
    }

    /// The Deep, when below ground.
    pub fn deep(&self) -> Option<&Deep> {
        self.deep.as_ref()
    }

    /// The town covering a surface cell.
    pub fn town_at(&self, at: Pos) -> Option<Town> {
        if self.deep.is_some() {
            return None;
        }
        self.towns.iter().flatten().find(|t| t.contains(at)).copied()
    }

    /// Generated towns in the currently inspected neighborhood, including the starting town.
    pub fn towns(&self) -> impl Iterator<Item = Town> + '_ {
        self.towns.iter().flatten().copied().filter(|_| self.deep.is_none())
    }

    /// Whether a coordinate is on this map.
    pub fn contains(&self, at: Pos) -> bool {
        self.deep.is_none_or(|d| d.contains(at))
    }

    /// Whether a cell belongs to a generated town or to the Deep's fixed layout, rather than open wilderness.
    pub fn authored(&self, at: Pos) -> bool {
        match &self.deep {
            Some(deep) => deep.contains(at),
            None => self.town_at(at).is_some(),
        }
    }

    /// Whether living things leave you alone here: a town, or the Deep's stair landing.
    pub fn safe(&self, at: Pos) -> bool {
        match &self.deep {
            Some(deep) => deep.landing(at),
            None => self.town_at(at).is_some(),
        }
    }

    /// The conditions life meets on a cell.
    pub fn ground(&self, at: Pos) -> &'static Biome {
        if self.deep.is_some() { &DEEP } else { self.terrain.biome(at).biome() }
    }

    /// The cell at a coordinate, including wilderness and any generated town.
    pub fn tile(&self, at: Pos) -> Tile {
        match &self.deep {
            Some(deep) => deep.tile(at),
            None => self.town_at(at).map_or_else(|| self.terrain.tile(at), |town| town.cell(at).0),
        }
    }

    /// A service zone at a coordinate.
    pub fn zone(&self, at: Pos) -> Zone {
        self.town_at(at).map_or(Zone::Open, |town| town.cell(at).1)
    }

    /// The town, biome, or Deep name at a coordinate.
    pub fn name_at(&self, at: Pos) -> String {
        match &self.deep {
            Some(deep) => deep.name(),
            None => self.town_at(at).map_or_else(|| self.terrain.biome(at).biome().name.to_owned(), Town::name),
        }
    }

    /// Whether `from` can see `to` within `radius` cells. See [`sight`]: walls and rocks block a ray, and
    /// each tree it crosses costs [`TREE_SIGHT`] cells of the radius.
    pub fn sees(&self, from: Pos, to: Pos, radius: i32) -> bool {
        sight(from, to, radius, |p| self.tile(p))
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

    #[test]
    fn lines_run_end_to_end() {
        assert_eq!(line((0, 0), (3, 1)), vec![(0, 0), (1, 0), (2, 1), (3, 1)]);
        assert_eq!(line((2, 2), (2, 2)), vec![(2, 2)]);
        assert_eq!(distance((0, 0), (3, -5)), 5);
    }

    fn field(blocks: &[(Pos, Tile)]) -> impl Fn(Pos) -> Tile + '_ {
        |p| blocks.iter().find(|(at, _)| *at == p).map_or(Tile::Grass, |&(_, t)| t)
    }

    #[test]
    fn trees_thin_sight_while_walls_and_rocks_end_it() {
        let open = field(&[]);
        assert!(sight((0, 0), (8, 2), 8, &open) && !sight((0, 0), (9, 0), 8, &open));
        // Round, not square: the corners of the old square are out of reach.
        assert!(
            sight((0, 0), (6, 5), 8, &open) && !sight((0, 0), (7, 6), 8, &open) && !sight((0, 0), (8, 8), 8, &open)
        );
        let tree = field(&[((1, 0), Tile::Tree)]);
        assert!(sight((0, 0), (5, 0), 8, &tree), "past one tree, five cells away");
        assert!(!sight((0, 0), (6, 0), 8, &tree));
        let two = field(&[((1, 0), Tile::Tree), ((3, 0), Tile::Tree)]);
        assert!(sight((0, 0), (2, 0), 8, &two) && !sight((0, 0), (4, 0), 8, &two));
        for wall in [Tile::Wall, Tile::Rock] {
            assert!(!sight((0, 0), (3, 0), 8, field(&[((1, 0), wall)])));
        }
        // You see the tree beside you, and a creature standing in a tree.
        assert!(sight((0, 0), (1, 0), 8, &tree) && sight((0, 0), (1, 0), 1, &tree));
        assert!(sight((-2, 0), (1, 0), 8, &tree));
    }

    #[test]
    fn rays_glimpse_past_the_edge_of_a_trunk_but_not_a_wall_face() {
        // Bresenham's one line from (0,0) to (6,1) runs through (2,0); other rays pass beside it.
        assert!(line((0, 0), (6, 1)).contains(&(2, 0)));
        assert!(sight((0, 0), (6, 1), 6, field(&[((2, 0), Tile::Tree)])), "a free ray costs nothing");
        let face: Vec<_> = (-3..=3).map(|y| ((2, y), Tile::Wall)).collect();
        assert!(!sight((0, 0), (6, 1), 8, field(&face)));
    }

    #[test]
    fn sight_is_mutual_and_never_reaches_past_the_radius() {
        let mut rng = Rng::keyed(7, b"sighttst", &[]);
        for _ in 0..300 {
            let blocks: Vec<(Pos, Tile)> = (0..14)
                .map(|_| {
                    let at = ((rng.unit() * 11.0) as i32 - 5, (rng.unit() * 11.0) as i32 - 5);
                    (at, if rng.unit() < 0.7 { Tile::Tree } else { Tile::Wall })
                })
                .collect();
            let tiles = field(&blocks);
            let a = ((rng.unit() * 11.0) as i32 - 5, (rng.unit() * 11.0) as i32 - 5);
            let b = ((rng.unit() * 11.0) as i32 - 5, (rng.unit() * 11.0) as i32 - 5);
            assert_eq!(sight(a, b, 8, &tiles), sight(b, a, 8, &tiles), "{a:?} {b:?} {blocks:?}");
            assert!(!sight(a, b, distance(a, b) - 1, &tiles));
        }
    }

    #[test]
    fn trees_on_the_real_surface_cost_sight() {
        let game = crate::Game::new(7);
        let level = game.level();
        let me = game.player().pos;
        let checked = (me.1 - 40..me.1 + 40)
            .flat_map(|y| (me.0 - 40..me.0 + 40).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                level.tile((x, y)) == Tile::Tree
                    && (-1..=6).all(|k| k == 0 || matches!(level.tile((x + k, y)), Tile::Wild(_) | Tile::Grass))
                    && (-1..=6).all(|k| {
                        [-1, 1].iter().all(|&dy| !matches!(level.tile((x + k, y + dy)), Tile::Wall | Tile::Rock))
                    })
            })
            .inspect(|&(x, y)| {
                assert!(level.sees((x - 1, y), (x + 4, y), 8), "{x},{y}");
                assert!(!level.sees((x - 1, y), (x + 5, y), 8), "{x},{y}");
            })
            .count();
        assert!(checked > 0, "seed 7 has open trees near the start");
    }

    /// Time one frame's sight in thick forest: every cell of the view and the ring around it.
    /// `cargo test -p isle-core sight_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn sight_cost() {
        let game = crate::Game::new(7);
        let level = game.level();
        let reach = crate::SIGHT + 2;
        for me in [(-96, 359), (94, 39), game.player().pos] {
            let start = std::time::Instant::now();
            let seen = (-reach..=reach)
                .flat_map(|dy| (-reach..=reach).map(move |dx| (me.0 + dx, me.1 + dy)))
                .filter(|&p| level.sees(me, p, crate::SIGHT))
                .count();
            eprintln!("{me:?}: {seen} seen, {:?}", start.elapsed());
        }
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

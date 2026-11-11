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
    /// The surface: towns and the wilds between them.
    Surface,
    /// The Deep beneath a town's stairs.
    Deep(Town),
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
    /// The active town's stairs down.
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
            LevelId::Deep(town) => Some(Deep::new(town)),
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
            stairs_down: town.map(Town::stairs),
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

//! Seeded settlements on dry ground. Geometry is compact; tiles are computed rather than saved.

use std::collections::BTreeMap;

use blah_core::{Rng, World, find_land};
use serde::{Deserialize, Serialize};

use crate::map::{Pos, Terrain, Tile, Zone};

/// A terrain-placed town with three guaranteed services and a safe central square.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Town {
    /// Northwest corner, in surface coordinates.
    pub origin: Pos,
    variant: u64,
}

impl Town {
    /// Cells across a town's safe footprint.
    pub const WIDTH: i32 = 29;
    /// Cells down a town's safe footprint.
    pub const HEIGHT: i32 = 25;
    /// The starting town's arrival square, in surface coordinates.
    pub const START: Pos = (14, 12);

    fn new(seed: u64, origin: Pos, world_center: (i64, i64)) -> Town {
        let variant = Rng::keyed(seed, b"isletown", &[world_center.0 as u64, world_center.1 as u64]).next_u64();
        Town { origin, variant }
    }

    /// The town's seeded name.
    pub fn name(self) -> String {
        const FIRST: [&str; 16] = [
            "Ash", "Alder", "Briar", "Cinder", "Dun", "Elder", "Fern", "Grey", "Hazel", "Iron", "Moss", "Oak", "Reed",
            "Stone", "Thorn", "Willow",
        ];
        const LAST: [&str; 16] = [
            "bridge", "brook", "cross", "dale", "fall", "field", "ford", "gate", "haven", "hill", "mere", "rest",
            "stead", "vale", "watch", "wick",
        ];
        format!("{}{}", FIRST[(self.variant >> 16 & 15) as usize], LAST[(self.variant >> 20 & 15) as usize])
    }

    /// Whether a surface coordinate lies in the safe footprint.
    pub fn contains(self, (x, y): Pos) -> bool {
        x >= self.origin.0 && y >= self.origin.1 && x < self.origin.0 + Self::WIDTH && y < self.origin.1 + Self::HEIGHT
    }

    fn local(self, (x, y): Pos) -> Pos {
        let (x, y) = (x - self.origin.0, y - self.origin.1);
        (
            if self.variant & 1 != 0 { Self::WIDTH - 1 - x } else { x },
            if self.variant & 2 != 0 { Self::HEIGHT - 1 - y } else { y },
        )
    }

    fn global(self, (x, y): Pos) -> Pos {
        let (x, y) = (
            if self.variant & 1 != 0 { Self::WIDTH - 1 - x } else { x },
            if self.variant & 2 != 0 { Self::HEIGHT - 1 - y } else { y },
        );
        (x + self.origin.0, y + self.origin.1)
    }

    fn rooms(self) -> [(Pos, i32, i32, Zone); 3] {
        let swap = self.variant & 4 != 0;
        [
            (
                (7, 5),
                5 + 2 * (self.variant >> 3 & 3).min(2) as i32,
                5 + 2 * (self.variant >> 5 & 1) as i32,
                if swap { Zone::Temple } else { Zone::Shop },
            ),
            (
                (21, 5),
                5 + 2 * (self.variant >> 6 & 3).min(2) as i32,
                5 + 2 * (self.variant >> 8 & 1) as i32,
                if swap { Zone::Shop } else { Zone::Temple },
            ),
            ((14, 20), 5 + 2 * (self.variant >> 9 & 3).min(2) as i32, 5, Zone::Open),
        ]
    }

    /// The safe arrival square.
    pub fn start(self) -> Pos {
        self.global(Self::START)
    }

    /// A walkable point inside the outfitter.
    pub fn shop(self) -> Pos {
        let (center, _, _, _) = self.rooms().into_iter().find(|r| r.3 == Zone::Shop).unwrap();
        self.global((center.0, center.1 + 1))
    }

    /// The temple's revival point.
    pub fn respawn(self) -> Pos {
        let (center, _, _, _) = self.rooms().into_iter().find(|r| r.3 == Zone::Temple).unwrap();
        self.global(center)
    }

    /// The dungeon entrance.
    pub fn stairs(self) -> Pos {
        self.global((14, 20))
    }

    /// Town tile and service zone at a coordinate inside its footprint.
    pub(crate) fn cell(self, at: Pos) -> (Tile, Zone) {
        let (x, y) = self.local(at);
        for ((cx, cy), width, height, zone) in self.rooms() {
            let (rx, ry) = ((x - cx).abs(), (y - cy).abs());
            if rx > width / 2 || ry > height / 2 {
                continue;
            }
            if rx == width / 2 || ry == height / 2 {
                let door = x == cx && (y == cy + height / 2 || zone == Zone::Open);
                return (if door { Tile::Door } else { Tile::Wall }, Zone::Open);
            }
            let tile = if zone == Zone::Shop && y == cy - height / 2 + 1 && rx <= 1 {
                Tile::Counter
            } else if zone == Zone::Temple && x == cx && y == cy - height / 2 + 1 {
                Tile::Altar
            } else if zone == Zone::Open && (x, y) == (cx, cy) {
                Tile::StairsDown
            } else {
                Tile::Floor
            };
            return (tile, zone);
        }
        let street = x == 14 || y == 12 || ((x == 7 || x == 21) && (5..=12).contains(&y));
        (if street { Tile::Floor } else { Tile::Grass }, Zone::Open)
    }

    fn fits(world: &World, center: (i64, i64)) -> bool {
        let dry = |dx, dy| {
            let biome = world.tile(center.0 + dx, center.1 + dy).biome;
            biome.biome().passable && !biome.biome().water
        };
        if ![(-14, -12), (14, -12), (-14, 12), (14, 12), (0, 0)].into_iter().all(|(x, y)| dry(x, y)) {
            return false;
        }
        (-12..=12).all(|y| (-14..=14).all(|x| dry(x, y)))
    }

    pub(crate) fn home_site(seed: u64) -> (i64, i64) {
        let world = World::new(seed);
        let land = find_land(&world, 0, 0).expect("land near the origin");
        for radius in 0_i64..=64 {
            for y in -radius..=radius {
                for x in -radius..=radius {
                    if x.abs().max(y.abs()) != radius {
                        continue;
                    }
                    let center = (land.0 + x * 16, land.1 + y * 16);
                    if Self::fits(&world, center) {
                        return center;
                    }
                }
            }
        }
        panic!("no dry town site within 1024 cells of the first land");
    }
}

const SPACING: i64 = 192;

/// Towns revealed around explored sectors. Empty sectors are remembered too, avoiding repeated searches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Towns {
    pub(crate) home: Town,
    sectors: BTreeMap<i64, BTreeMap<i64, Option<Town>>>,
}

impl Towns {
    pub(crate) fn new(seed: u64, terrain: Terrain) -> Towns {
        Towns { home: Town::new(seed, (0, 0), terrain.world_pos(Town::START)), sectors: BTreeMap::new() }
    }

    fn sector(terrain: Terrain, at: Pos) -> (i64, i64) {
        let (x, y) = terrain.world_pos(at);
        (x.div_euclid(SPACING), y.div_euclid(SPACING))
    }

    pub(crate) fn ensure(&mut self, seed: u64, terrain: Terrain, at: Pos) {
        let sector = Self::sector(terrain, at);
        let world = World::new(seed);
        let offset = terrain.world_pos((0, 0));
        let home = terrain.world_pos(self.home.start());
        for y in sector.1 - 1..=sector.1 + 1 {
            for x in sector.0 - 1..=sector.0 + 1 {
                self.sectors.entry(x).or_default().entry(y).or_insert_with(|| {
                    let mut rng = Rng::keyed(seed, b"townsite", &[x as u64, y as u64]);
                    for _ in 0..24 {
                        let center =
                            (x * SPACING + 32 + rng.below(128) as i64, y * SPACING + 32 + rng.below(128) as i64);
                        if (center.0 - home.0).abs().max((center.1 - home.1).abs()) < 64 || !Town::fits(&world, center)
                        {
                            continue;
                        }
                        let Ok(ox) = i32::try_from(center.0 - offset.0 - 14) else { continue };
                        let Ok(oy) = i32::try_from(center.1 - offset.1 - 12) else { continue };
                        return Some(Town::new(seed, (ox, oy), center));
                    }
                    None
                });
            }
        }
    }

    pub(crate) fn nearby(&self, terrain: Terrain, at: Pos) -> [Option<Town>; 10] {
        let (x, y) = Self::sector(terrain, at);
        let mut towns = [None; 10];
        towns[0] = Some(self.home);
        for dy in -1..=1 {
            for dx in -1..=1 {
                towns[(1 + (dy + 1) * 3 + dx + 1) as usize] =
                    self.sectors.get(&(x + dx)).and_then(|column| column.get(&(y + dy))).copied().flatten();
            }
        }
        towns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    fn check(town: Town) {
        let start = town.start();
        let mut seen = HashSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some((x, y)) = queue.pop_front() {
            for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
                let next = (x + dx, y + dy);
                if town.contains(next) && town.cell(next).0.walkable() && seen.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        let mut cells = Vec::new();
        for y in town.origin.1..town.origin.1 + Town::HEIGHT {
            for x in town.origin.0..town.origin.0 + Town::WIDTH {
                let tile = town.cell((x, y)).0;
                assert!(!tile.walkable() || seen.contains(&(x, y)), "{}: {x},{y}", town.name());
                cells.push(tile);
            }
        }
        for service in [town.shop(), town.respawn(), town.stairs()] {
            assert!(seen.contains(&service));
        }
        assert_eq!(town.cell(town.shop()).1, Zone::Shop);
        assert_eq!(town.cell(town.respawn()).1, Zone::Temple);
        assert_eq!(town.cell(town.stairs()).0, Tile::StairsDown);
        assert!(cells.contains(&Tile::Counter) && cells.contains(&Tile::Altar));
        assert_eq!(cells.iter().filter(|&&tile| tile == Tile::StairsDown).count(), 1);
        assert!(seen.iter().any(|&(x, _)| x == town.origin.0));
        assert!(seen.iter().any(|&(x, _)| x == town.origin.0 + Town::WIDTH - 1));
        assert!(seen.iter().any(|&(_, y)| y == town.origin.1));
        assert!(seen.iter().any(|&(_, y)| y == town.origin.1 + Town::HEIGHT - 1));
    }

    #[test]
    fn every_layout_has_services_and_cardinal_paths_to_them_and_the_edges() {
        // Exhaust every geometry bit: both mirrors, swapped services, and every room size.
        for variant in 0..4096 {
            check(Town { origin: (-80, 37), variant });
        }
    }

    #[test]
    fn settlements_fit_dry_terrain_across_seeds() {
        let mut names = HashSet::new();
        let mut others = 0;
        for seed in 0..32 {
            let terrain = Terrain::new(seed);
            let mut towns = Towns::new(seed, terrain);
            towns.ensure(seed, terrain, Town::START);
            let world = World::new(seed);
            names.insert(towns.home.name());
            check(towns.home);
            for town in towns.nearby(terrain, Town::START).into_iter().flatten() {
                if town != towns.home {
                    others += 1;
                }
                for y in town.origin.1..town.origin.1 + Town::HEIGHT {
                    for x in town.origin.0..town.origin.0 + Town::WIDTH {
                        let (wx, wy) = terrain.world_pos((x, y));
                        let biome = world.tile(wx, wy).biome.biome();
                        assert!(biome.passable && !biome.water, "seed {seed}: {} at {x},{y}", town.name());
                    }
                }
            }
        }
        assert!(names.len() > 16, "town names vary with seed");
        assert!(others > 16, "settlements continue beyond the starting town");
    }

    #[test]
    fn discovery_order_and_reload_do_not_change_sites_or_layouts() {
        let terrain = Terrain::new(7);
        let positions = [(-400, -400), (400, 400), (-400, 400), (400, -400), Town::START];
        let mut a = Towns::new(7, terrain);
        let mut b = a.clone();
        for pos in positions {
            a.ensure(7, terrain, pos);
        }
        for pos in positions.into_iter().rev() {
            b.ensure(7, terrain, pos);
        }
        assert_eq!(a, b);
        let mut loaded: Towns = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        loaded.ensure(7, terrain, (-800, -800));
        a.ensure(7, terrain, (-800, -800));
        assert_eq!(a, loaded);
        let towns: Vec<_> =
            a.sectors.values().flat_map(|column| column.values()).flatten().copied().chain([a.home]).collect();
        for (i, town) in towns.iter().enumerate() {
            for other in towns.iter().skip(i + 1) {
                let separated = town.origin.0 + Town::WIDTH <= other.origin.0
                    || other.origin.0 + Town::WIDTH <= town.origin.0
                    || town.origin.1 + Town::HEIGHT <= other.origin.1
                    || other.origin.1 + Town::HEIGHT <= town.origin.1;
                assert!(separated);
            }
        }
    }
}

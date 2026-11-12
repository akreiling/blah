//! Seeded settlements on dry ground. Geometry is compact; tiles are computed rather than saved.

use std::collections::BTreeMap;

use blah_core::{Rng, World, find_land};
use serde::{Deserialize, Serialize};

use crate::map::{Pos, Terrain, Tile, Zone};

/// A terrain-placed settlement. Legacy layouts retain their original geometry and services.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Town {
    /// Northwest corner, in surface coordinates.
    pub origin: Pos,
    variant: u64,
    #[serde(default, skip_serializing_if = "is_legacy")]
    layout: u8,
}

fn is_legacy(layout: &u8) -> bool {
    *layout == 0
}

#[derive(Clone, Copy)]
struct Building {
    center: Pos,
    width: i32,
    height: i32,
    zone: Zone,
    stairs: bool,
    south: bool,
}

impl Town {
    /// Cells across a legacy town. Use [`Town::width`] for generated settlements.
    pub const WIDTH: i32 = 29;
    /// Cells down a legacy town. Use [`Town::height`] for generated settlements.
    pub const HEIGHT: i32 = 25;
    /// The starting town's arrival square, in surface coordinates.
    pub const START: Pos = (14, 12);

    fn new(seed: u64, origin: Pos, world_center: (i64, i64)) -> Town {
        let variant = Rng::keyed(seed, b"isletown", &[world_center.0 as u64, world_center.1 as u64]).next_u64();
        Town { origin, variant, layout: 0 }
    }

    /// Settlement category; existing towns keep their original plan.
    pub fn kind(self) -> &'static str {
        if self.layout == 0 {
            "town"
        } else {
            match self.variant % 3 {
                0 => "hamlet",
                1 => "village",
                _ => "town",
            }
        }
    }

    /// Width of this settlement's safe footprint.
    pub fn width(self) -> i32 {
        if self.layout == 0 {
            Self::WIDTH
        } else {
            match self.variant % 3 {
                0 => 17,
                1 => 25,
                _ => 41,
            }
        }
    }

    /// Height of this settlement's safe footprint.
    pub fn height(self) -> i32 {
        if self.layout == 0 {
            Self::HEIGHT
        } else {
            match self.variant % 3 {
                0 => 17,
                1 => 25,
                _ => 33,
            }
        }
    }

    fn has_shop(self) -> bool {
        self.layout == 0 || self.variant % 3 == 2 || self.variant >> 12 & 1 != 0
    }

    fn has_temple(self) -> bool {
        self.layout == 0 || self.variant % 3 == 2 || self.variant >> 13 & 1 != 0
    }

    fn has_deep(self) -> bool {
        self.layout == 0 || self.variant >> 14 & 1 != 0
    }

    /// The outfitter, only where the settlement offers one.
    pub fn shop_site(self) -> Option<Pos> {
        self.has_shop().then(|| self.shop())
    }

    /// The temple, only where the settlement offers one.
    pub fn temple_site(self) -> Option<Pos> {
        self.has_temple().then(|| self.respawn())
    }

    /// The Deep entrance, only where the settlement offers one.
    pub fn deep_site(self) -> Option<Pos> {
        self.has_deep().then(|| self.stairs())
    }

    pub(crate) fn variant(self) -> u64 {
        self.variant
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
        x >= self.origin.0
            && y >= self.origin.1
            && x < self.origin.0 + self.width()
            && y < self.origin.1 + self.height()
    }

    fn local(self, (x, y): Pos) -> Pos {
        let (x, y) = (x - self.origin.0, y - self.origin.1);
        (
            if self.variant & 1 != 0 { self.width() - 1 - x } else { x },
            if self.variant & 2 != 0 { self.height() - 1 - y } else { y },
        )
    }

    fn global(self, (x, y): Pos) -> Pos {
        let (x, y) = (
            if self.variant & 1 != 0 { self.width() - 1 - x } else { x },
            if self.variant & 2 != 0 { self.height() - 1 - y } else { y },
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
        self.global(if self.layout == 0 { Self::START } else { (self.width() / 16 * 8, self.height() / 16 * 8) })
    }

    /// The outfitter's reserved position. Use [`Town::shop_site`] to check that it exists.
    pub fn shop(self) -> Pos {
        if self.layout != 0 {
            return self.global((4, 5));
        }
        let (center, _, _, _) = self.rooms().into_iter().find(|r| r.3 == Zone::Shop).unwrap();
        self.global((center.0, center.1 + 1))
    }

    /// The temple's reserved revival point. Use [`Town::temple_site`] to check that it exists.
    pub fn respawn(self) -> Pos {
        if self.layout != 0 {
            return self.global((self.width() - 5, 4));
        }
        let (center, _, _, _) = self.rooms().into_iter().find(|r| r.3 == Zone::Temple).unwrap();
        self.global(center)
    }

    /// The reserved dungeon entrance. Use [`Town::deep_site`] to check that it exists.
    pub fn stairs(self) -> Pos {
        if self.layout != 0 {
            return self.global((4, self.height() - 5));
        }
        self.global((14, 20))
    }

    /// Town tile and service zone at a coordinate inside its footprint.
    pub(crate) fn cell(self, at: Pos) -> (Tile, Zone) {
        if self.layout != 0 {
            return self.generated_cell(self.local(at));
        }
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

    fn building(self, col: i32, row: i32) -> Option<Building> {
        let cols = (self.width() - 1) / 8;
        let rows = (self.height() - 1) / 8;
        let shop = col == 0 && row == 0 && self.has_shop();
        let temple = col == cols - 1 && row == 0 && self.has_temple();
        let stairs = col == 0 && row == rows - 1 && self.has_deep();
        let mut rng = Rng::keyed(self.variant, b"building", &[col as u64, row as u64]);
        if !shop && !temple && !stairs && rng.below(4) == 0 {
            return None;
        }
        Some(Building {
            center: (col * 8 + 4, row * 8 + 4),
            width: 5 + 2 * rng.below(2) as i32,
            height: 5 + 2 * rng.below(2) as i32,
            zone: if shop {
                Zone::Shop
            } else if temple {
                Zone::Temple
            } else {
                Zone::Open
            },
            stairs,
            south: row != rows - 1,
        })
    }

    fn generated_cell(self, (x, y): Pos) -> (Tile, Zone) {
        let (w, h) = (self.width(), self.height());
        let (col, row) = ((x / 8).min((w - 2) / 8), (y / 8).min((h - 2) / 8));
        if let Some(b) = self.building(col, row) {
            let (cx, cy) = b.center;
            let (rx, ry) = ((x - cx).abs(), (y - cy).abs());
            if rx <= b.width / 2 && ry <= b.height / 2 {
                if rx == b.width / 2 || ry == b.height / 2 {
                    let door_y = cy + if b.south { b.height / 2 } else { -b.height / 2 };
                    return (if x == cx && y == door_y { Tile::Door } else { Tile::Wall }, Zone::Open);
                }
                let tile = if b.zone == Zone::Shop && y == cy - b.height / 2 + 1 && rx <= 1 {
                    Tile::Counter
                } else if b.zone == Zone::Temple && x == cx && y == cy - b.height / 2 + 1 {
                    Tile::Altar
                } else if b.stairs && (x, y) == b.center {
                    Tile::StairsDown
                } else {
                    Tile::Floor
                };
                return (tile, b.zone);
            }
            if x == cx && (if b.south { y > cy && y < (row + 1) * 8 } else { y < cy && y > row * 8 }) {
                return (Tile::Floor, Zone::Open);
            }
        }
        // Seeded lanes join the central avenues. Larger places gain walls and gated exits.
        let avenue = x == w / 16 * 8 || y == h / 16 * 8;
        let lane = y % 8 == 0 || (x % 8 == 0 && (self.variant >> (x / 8 + 24) & 1 != 0));
        let edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
        let walled = self.variant % 3 == 2 || self.variant % 3 == 1 && self.variant & 8 != 0;
        if edge && walled {
            return (if avenue { Tile::Door } else { Tile::Wall }, Zone::Open);
        }
        (if avenue || lane { Tile::Floor } else { Tile::Grass }, Zone::Open)
    }

    fn fits(world: &World, center: (i64, i64), width: i32, height: i32) -> bool {
        let dry = |dx, dy| {
            let biome = world.tile(center.0 + dx, center.1 + dy).biome;
            biome.biome().passable && !biome.biome().water
        };
        let (hw, hh) = (i64::from(width / 2), i64::from(height / 2));
        if ![(-hw, -hh), (hw, -hh), (-hw, hh), (hw, hh), (0, 0)].into_iter().all(|(x, y)| dry(x, y)) {
            return false;
        }
        (-hh..=hh).all(|y| (-hw..=hw).all(|x| dry(x, y)))
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
                    if Self::fits(&world, center, Self::WIDTH, Self::HEIGHT) {
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
                        let mut town = Town::new(seed, (0, 0), center);
                        town.layout = 1;
                        if (center.0 - home.0).abs().max((center.1 - home.1).abs()) < 64
                            || !Town::fits(&world, center, town.width(), town.height())
                        {
                            continue;
                        }
                        let Ok(ox) = i32::try_from(center.0 - offset.0 - i64::from(town.width() / 2)) else { continue };
                        let Ok(oy) = i32::try_from(center.1 - offset.1 - i64::from(town.height() / 2)) else {
                            continue;
                        };
                        town.origin = (ox, oy);
                        return Some(town);
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
        for y in town.origin.1..town.origin.1 + town.height() {
            for x in town.origin.0..town.origin.0 + town.width() {
                let tile = town.cell((x, y)).0;
                assert!(
                    !tile.walkable() || seen.contains(&(x, y)),
                    "{} variant {} {}: {x},{y} local {:?}",
                    town.name(),
                    town.variant,
                    town.kind(),
                    town.local((x, y))
                );
                cells.push(tile);
            }
        }
        for service in [town.shop_site(), town.temple_site(), town.deep_site()].into_iter().flatten() {
            assert!(seen.contains(&service));
        }
        if let Some(shop) = town.shop_site() {
            assert_eq!(town.cell(shop).1, Zone::Shop);
        }
        if let Some(temple) = town.temple_site() {
            assert_eq!(town.cell(temple).1, Zone::Temple);
        }
        if let Some(stairs) = town.deep_site() {
            assert_eq!(town.cell(stairs).0, Tile::StairsDown);
        }
        assert_eq!(cells.contains(&Tile::Counter), town.has_shop());
        assert_eq!(cells.contains(&Tile::Altar), town.has_temple());
        assert_eq!(cells.iter().filter(|&&tile| tile == Tile::StairsDown).count(), usize::from(town.has_deep()));
        assert!(seen.iter().any(|&(x, _)| x == town.origin.0));
        assert!(seen.iter().any(|&(x, _)| x == town.origin.0 + town.width() - 1));
        assert!(seen.iter().any(|&(_, y)| y == town.origin.1));
        assert!(seen.iter().any(|&(_, y)| y == town.origin.1 + town.height() - 1));
    }

    #[test]
    fn every_layout_has_services_and_cardinal_paths_to_them_and_the_edges() {
        // Exhaust every geometry bit: both mirrors, swapped services, and every room size.
        for variant in 0..4096 {
            check(Town { origin: (-80, 37), variant, layout: 0 });
        }
    }

    #[test]
    fn generated_settlements_vary_in_size_buildings_services_and_walls() {
        let mut sizes = HashSet::new();
        let mut services = HashSet::new();
        let mut plans = HashSet::new();
        for variant in 0..2048_u64 {
            let variant = variant.wrapping_mul(0x9e37_79b9_7f4a_7c15);
            let town = Town { origin: (-80, 37), variant, layout: 1 };
            check(town);
            sizes.insert((town.width(), town.height()));
            services.insert((town.has_shop(), town.has_temple(), town.has_deep()));
            plans.insert(
                (0..town.height())
                    .flat_map(|y| {
                        (0..town.width())
                            .map(move |x| format!("{:?}", town.cell((town.origin.0 + x, town.origin.1 + y))))
                    })
                    .collect::<Vec<_>>(),
            );
        }
        assert_eq!(sizes.len(), 3);
        assert_eq!(services.len(), 8, "every service combination can occur");
        assert!(plans.len() > 1000, "not just mirrored copies of one fixed plan");
    }

    #[test]
    fn old_serialized_towns_keep_their_identity_geometry_and_deep_seed() {
        let json = r#"{"origin":[-80,37],"variant":12345678}"#;
        let town: Town = serde_json::from_str(json).unwrap();
        assert_eq!(town, Town { origin: (-80, 37), variant: 12345678, layout: 0 });
        assert_eq!(serde_json::to_string(&town).unwrap(), json);
        check(town);
        assert_eq!(town.variant(), 12345678);
        let generated = Town { layout: 1, ..town };
        assert_eq!(serde_json::from_str::<Town>(&serde_json::to_string(&generated).unwrap()).unwrap(), generated);
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
                for y in town.origin.1..town.origin.1 + town.height() {
                    for x in town.origin.0..town.origin.0 + town.width() {
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
                let separated = town.origin.0 + town.width() <= other.origin.0
                    || other.origin.0 + other.width() <= town.origin.0
                    || town.origin.1 + town.height() <= other.origin.1
                    || other.origin.1 + other.height() <= town.origin.1;
                assert!(separated);
            }
        }
    }
}

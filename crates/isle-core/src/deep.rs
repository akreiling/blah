//! A generated Deep beneath each town: rooms joined into one tree of corridors, around a safe stair landing.
//! The layout is a pure function of the town, so saves keep only what was seen and what lives there.

use blah_core::Rng;

use crate::map::{Pos, Tile, distance};
use crate::town::Town;

/// Most rooms in one Deep, the landing included.
const ROOMS: usize = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
struct Room {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Room {
    fn center(self) -> Pos {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    fn contains(self, (x, y): Pos) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    /// Rooms closer than this keep no wall between them, so they are not placed.
    fn crowds(self, other: Room) -> bool {
        self.x - 2 < other.x + other.w
            && other.x - 2 < self.x + self.w
            && self.y - 2 < other.y + other.h
            && other.y - 2 < self.y + self.h
    }
}

/// The dungeon beneath one town.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deep {
    /// The town whose stairs lead here.
    pub town: Town,
    rooms: [Room; ROOMS],
    /// Each room after the landing joins an earlier one; the flag says whether the corridor runs across first.
    links: [(usize, bool); ROOMS],
    count: usize,
}

impl Deep {
    /// Cells across.
    pub const WIDTH: i32 = 64;
    /// Cells down.
    pub const HEIGHT: i32 = 32;

    /// The Deep under `town`.
    pub fn new(town: Town) -> Deep {
        let mut rng = Rng::keyed(town.variant(), b"isledeep", &[town.origin.0 as u64, town.origin.1 as u64]);
        let mut rooms = [Room::default(); ROOMS];
        let mut count = 0;
        for _ in 0..400 {
            if count == ROOMS {
                break;
            }
            let (w, h) = if count == 0 { (5, 5) } else { (5 + rng.below(8) as i32, 4 + rng.below(4) as i32) };
            let room = Room {
                x: 1 + rng.below((Self::WIDTH - w - 1) as usize) as i32,
                y: 1 + rng.below((Self::HEIGHT - h - 1) as usize) as i32,
                w,
                h,
            };
            if rooms[..count].iter().all(|r| !r.crowds(room)) {
                rooms[count] = room;
                count += 1;
            }
        }
        let mut links = [(0, false); ROOMS];
        for i in 1..count {
            let c = rooms[i].center();
            let nearest = (0..i)
                .min_by_key(|&j| {
                    let d = rooms[j].center();
                    ((c.0 - d.0).abs() + (c.1 - d.1).abs(), j)
                })
                .expect("the landing comes first");
            links[i] = (nearest, rng.chance(0.5));
        }
        Deep { town, rooms, links, count }
    }

    /// The stairs up, in the middle of the landing.
    pub fn stairs(&self) -> Pos {
        self.rooms[0].center()
    }

    /// Whether a cell is on the stair landing, where nothing that lives here goes.
    pub fn landing(&self, at: Pos) -> bool {
        self.rooms[0].contains(at)
    }

    /// Whether a coordinate is on this map.
    pub fn contains(&self, (x, y): Pos) -> bool {
        x >= 0 && y >= 0 && x < Self::WIDTH && y < Self::HEIGHT
    }

    /// Where creatures rise: one in every room past the landing, two in big ones. The flag marks the den in
    /// the room farthest from the stairs.
    pub fn spawns(&self) -> Vec<(Pos, bool)> {
        let stairs = self.stairs();
        let den = (1..self.count).max_by_key(|&i| (distance(self.rooms[i].center(), stairs), i));
        let mut spawns = Vec::new();
        for (i, room) in self.rooms.iter().enumerate().take(self.count).skip(1) {
            spawns.push((room.center(), Some(i) == den));
            if room.w * room.h >= 40 {
                spawns.push(((room.x + 1, room.y + 1), false));
            }
        }
        spawns
    }

    fn floor(&self, at: Pos) -> bool {
        let on = |a: Pos, b: Pos| {
            (a.0 == b.0 && at.0 == a.0 && at.1 >= a.1.min(b.1) && at.1 <= a.1.max(b.1))
                || (a.1 == b.1 && at.1 == a.1 && at.0 >= a.0.min(b.0) && at.0 <= a.0.max(b.0))
        };
        self.rooms[..self.count].iter().any(|r| r.contains(at))
            || (1..self.count).any(|i| {
                let (j, across) = self.links[i];
                let (a, b) = (self.rooms[i].center(), self.rooms[j].center());
                let corner = if across { (b.0, a.1) } else { (a.0, b.1) };
                on(a, corner) || on(corner, b)
            })
    }

    /// The cell at `at`; outside the map is wall.
    pub fn tile(&self, at: Pos) -> Tile {
        if !self.contains(at) {
            Tile::Wall
        } else if at == self.stairs() {
            Tile::StairsUp
        } else if self.floor(at) {
            Tile::Floor
        } else {
            Tile::Wall
        }
    }

    pub(crate) fn cell_index(&self, at: Pos) -> Option<usize> {
        self.contains(at).then(|| (at.1 * Self::WIDTH + at.0) as usize)
    }

    /// Its name, as the log says it.
    pub fn name(&self) -> String {
        format!("the Deep beneath {}", self.town.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    fn deeps() -> Vec<Deep> {
        let mut found = Vec::new();
        for seed in 0..24 {
            let mut game = crate::Game::new(seed);
            game.answer("Ash");
            game.answer("fighter");
            found.extend(game.level().towns().map(Deep::new));
        }
        found
    }

    #[test]
    fn every_deep_is_connected_walled_and_has_rooms_a_landing_and_a_den() {
        let all = deeps();
        assert!(all.len() > 24);
        let mut layouts = HashSet::new();
        for deep in all {
            assert!(deep.count >= 6, "{} has {} rooms", deep.name(), deep.count);
            layouts.insert(deep.rooms);
            let start = deep.stairs();
            assert_eq!(deep.tile(start), Tile::StairsUp);
            let mut reached = HashSet::from([start]);
            let mut queue = VecDeque::from([start]);
            while let Some((x, y)) = queue.pop_front() {
                for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
                    let next = (x + dx, y + dy);
                    if deep.tile(next).walkable() && reached.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
            for y in -1..=Deep::HEIGHT {
                for x in -1..=Deep::WIDTH {
                    let edge = x <= 0 || y <= 0 || x >= Deep::WIDTH - 1 || y >= Deep::HEIGHT - 1;
                    let walkable = deep.tile((x, y)).walkable();
                    assert!(!(edge && walkable), "the border is wall");
                    assert!(!walkable || reached.contains(&(x, y)), "every floor cell is reachable from the stairs");
                }
            }
            let spawns = deep.spawns();
            assert_eq!(spawns.iter().filter(|s| s.1).count(), 1);
            for (at, _) in spawns {
                assert_eq!(deep.tile(at), Tile::Floor);
                assert!(!deep.landing(at));
            }
        }
        assert!(layouts.len() > 20, "towns have their own layouts");
    }

    #[test]
    fn a_town_always_has_the_same_deep() {
        for deep in deeps().into_iter().take(8) {
            assert_eq!(Deep::new(deep.town), deep);
        }
    }
}

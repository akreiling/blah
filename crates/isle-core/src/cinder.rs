//! One small, persistent encounter near the starting town, not a repeatable wilderness spawn.

use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{HerbPatch, Landmark, LevelId, Place, Pos, Tile, distance};

/// What remains to do at Cinder Watch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchPhase {
    /// The warning can be read; only an explicit challenge wakes the guards.
    Dormant,
    /// The original guards remain, with their current health and finite lessons.
    Active,
    /// Both original guards are dead; the cache is available.
    Cleared,
}

/// The island's one ruined watchtower. Discovery and claiming use the ordinary landmark record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CinderWatch {
    /// The ruined tower and its cache.
    pub pos: Pos,
    /// Whether its guards have been challenged and defeated.
    pub phase: WatchPhase,
    /// Surviving original guard identities; kills, not absence from sight, clear the tower.
    pub(crate) guards: [Option<u32>; 2],
}

impl CinderWatch {
    /// Try a dry, cardinally reachable clearing 24 or more cells from home, within 64 walking steps.
    /// Search order is seeded; neither the search nor a failed search consumes combat dice.
    pub(crate) fn locate(seed: u64, home: Pos, level: &Place) -> Option<Self> {
        debug_assert_eq!(level.id, LevelId::Surface);
        let mut rng = blah_core::Rng::keyed(seed, b"isl-wtch", &[]);
        let mut dirs = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        dirs.rotate_left(rng.below(4));
        let mut seen = HashSet::from([home]);
        let mut queue = VecDeque::from([(home, 0)]);
        while let Some((pos, steps)) = queue.pop_front() {
            if steps >= 32 && distance(home, pos) >= 24 && Self::clearing(pos, level) {
                let sector = (pos.0.div_euclid(32), pos.1.div_euclid(32));
                let herbs = (pos.0.div_euclid(16), pos.1.div_euclid(16));
                if Landmark::generate(seed, sector, level).is_none_or(|site| site.pos != pos)
                    && HerbPatch::generate(seed, herbs, level).is_none_or(|patch| patch.pos != pos)
                {
                    return Some(Self { pos, phase: WatchPhase::Dormant, guards: [None; 2] });
                }
            }
            if steps == 64 {
                continue;
            }
            for (dx, dy) in dirs {
                let next = (pos.0 + dx, pos.1 + dy);
                if seen.insert(next) && level.tile(next).walkable() && !level.ground(next).water {
                    queue.push_back((next, steps + 1));
                }
            }
        }
        None
    }

    fn clearing(pos: Pos, level: &Place) -> bool {
        // Leave a clear line from the tower to each guard, without changing a single terrain cell.
        [(0, -2), (0, -1), (0, 0), (0, 1), (0, 2), (-1, 0), (1, 0)].into_iter().all(|(dx, dy)| {
            let at = (pos.0 + dx, pos.1 + dy);
            level.town_at(at).is_none()
                && matches!(level.tile(at), Tile::Wild(id) if id.biome().passable && !id.biome().water)
        })
    }

    /// Where the warden and the sentry stand when challenged.
    pub fn guard_positions(&self) -> [Pos; 2] {
        [(self.pos.0, self.pos.1 - 2), (self.pos.0, self.pos.1 + 2)]
    }

    /// The warning and next action, visible before making any commitment.
    pub const fn description(&self) -> &'static str {
        match self.phase {
            WatchPhase::Dormant => {
                "Two guards wait in the ruins: an ash sentry with a bow and a cinder warden whose plating halves arrow damage. Wards turn their attacks; Spark ignores the plating. Bring recovery supplies. Use challenge here when ready."
            }
            WatchPhase::Active => {
                "The surviving guards still hold the tower. Retreat does not replace them or heal their wounds. Defeat both to unlock the cache."
            }
            WatchPhase::Cleared => {
                "The last brazier is cold. The guards are gone for good. Use get all here to claim the unlocked cache."
            }
        }
    }

    pub(crate) fn defeated(&mut self, id: u32) -> bool {
        if self.phase != WatchPhase::Active || !self.guards.contains(&Some(id)) {
            return false;
        }
        for guard in &mut self.guards {
            if *guard == Some(id) {
                *guard = None;
            }
        }
        if self.guards == [None; 2] {
            self.phase = WatchPhase::Cleared;
            return true;
        }
        false
    }
}

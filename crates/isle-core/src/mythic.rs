//! The Exile's covenant: one recurring Deep drop and two finite, reachable wilderness trials.

use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{HerbPatch, Kind, Landmark, LandmarkKind, Place, Pos, Tile, WatchPhase, distance};

/// Persistent progress, independent of where the physical pieces have been left.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Covenant {
    /// Failed eligible keeper kills. The probability caps, not this counter.
    pub misses: u64,
    /// A physical Oathfang has been generated; no further keeper can duplicate it.
    pub first_dropped: bool,
    /// Picking up Oathfang awakens the trail, which remains in the character's memory.
    pub awakened: bool,
    /// The two seeded destinations and their original, finite guards.
    pub trials: Vec<CovenantTrial>,
}

impl Covenant {
    /// Chance in basis points, floored to 0.01 percentage points.
    /// 2% initially, quadratic to 99% after 20 misses, never guaranteed.
    pub fn chance_basis_points(&self) -> u32 {
        let misses = self.misses.min(20) as u32;
        200 + 9_700 * misses * misses / 400
    }

    pub(crate) fn roll(&mut self, seed: u64) -> bool {
        if self.first_dropped {
            return false;
        }
        let mut rng = blah_core::Rng::keyed(seed, b"isl-oath", &[self.misses]);
        if rng.below(10_000) < self.chance_basis_points() as usize {
            self.first_dropped = true;
            true
        } else {
            self.misses = self.misses.saturating_add(1);
            false
        }
    }

    /// A bounded, cardinal search from home, rather than a coordinate guessed across water.
    /// Destination placement never consumes the combat stream or changes terrain.
    pub(crate) fn locate(
        seed: u64,
        home: Pos,
        reserved: Option<Pos>,
        mut level_at: impl FnMut(Pos) -> Place,
    ) -> Vec<CovenantTrial> {
        let mut rng = blah_core::Rng::keyed(seed, b"isl-covt", &[]);
        let mut dirs = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        dirs.rotate_left(rng.below(4));
        let mut seen = HashSet::from([home]);
        let mut queue = VecDeque::from([(home, 0)]);
        let mut trials: Vec<CovenantTrial> = Vec::new();
        let mut clearings = Vec::new();
        while let Some((pos, steps)) = queue.pop_front() {
            let level = level_at(pos);
            if reserved.is_none_or(|other| distance(other, pos) > 6)
                && Self::clearing(pos, &level)
                && Landmark::generate(seed, (pos.0.div_euclid(32), pos.1.div_euclid(32)), &level)
                    .is_none_or(|site| distance(site.pos, pos) > 3)
                && HerbPatch::generate(seed, (pos.0.div_euclid(16), pos.1.div_euclid(16)), &level)
                    .is_none_or(|patch| distance(patch.pos, pos) > 3)
            {
                clearings.push(pos);
                if steps >= 48 && distance(home, pos) >= 32 && trials.iter().all(|trial| distance(trial.pos, pos) >= 32)
                {
                    trials.push(CovenantTrial::new(pos, trials.is_empty()));
                    if trials.len() == 2 {
                        return trials;
                    }
                }
            }
            if steps == 192 {
                continue;
            }
            for (dx, dy) in dirs {
                let next = (pos.0 + dx, pos.1 + dy);
                let next_level = level_at(next);
                if seen.insert(next) && next_level.tile(next).walkable() && !next_level.ground(next).water {
                    queue.push_back((next, steps + 1));
                }
            }
        }
        // A small island may not offer two distant destinations. Prefer closer *reachable*
        // clearings to an impressive-sounding bearing across an impassable ocean.
        while trials.len() < 2 {
            let next = clearings
                .iter()
                .copied()
                .filter(|&pos| trials.iter().all(|trial| distance(trial.pos, pos) >= 6))
                .max_by_key(|&pos| {
                    trials.iter().map(|trial| distance(trial.pos, pos)).min().unwrap_or(distance(home, pos))
                });
            let Some(pos) = next else { break };
            trials.push(CovenantTrial::new(pos, trials.is_empty()));
        }
        trials
    }

    fn clearing(pos: Pos, level: &Place) -> bool {
        [(0, -2), (0, -1), (0, 0), (0, 1), (0, 2), (-1, 0), (1, 0)].into_iter().all(|(dx, dy)| {
            let at = (pos.0 + dx, pos.1 + dy);
            level.town_at(at).is_none()
                && matches!(level.tile(at), Tile::Wild(id) if id.biome().passable && !id.biome().water)
        })
    }
}

/// A one-time encounter, not a respawning boss or a new Deep floor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CovenantTrial {
    /// Its reachable wilderness clearing.
    pub pos: Pos,
    /// Which covenant piece it protects.
    pub kind: LandmarkKind,
    /// Whether it has been challenged and cleared.
    pub phase: WatchPhase,
    /// Original guard identities. Retreat, death, and reload do not replace them.
    pub(crate) guards: [Option<u32>; 2],
}

impl CovenantTrial {
    fn new(pos: Pos, forge: bool) -> Self {
        Self {
            pos,
            kind: if forge { LandmarkKind::AshenForge } else { LandmarkKind::WellOfNames },
            phase: WatchPhase::Dormant,
            guards: [None; 2],
        }
    }

    /// Both the danger and counterplay, before the explicit challenge.
    pub const fn description(&self) -> &'static str {
        match self.phase {
            WatchPhase::Dormant => match self.kind {
                LandmarkKind::AshenForge => {
                    "The ash colossus regrows 4 Health each turn and halves arrow damage. A landed Spark seals its wounds for three turns. Oathfang drinks only from dangerous, unspent foes. Bring healing and use challenge when ready."
                }
                _ => {
                    "Two hollow cantors attack from opposite sides. Each landed blow or shot steals 2 Focus. Break their lines, bind one, or close the distance. Bring recovery and use challenge when ready."
                }
            },
            WatchPhase::Active => {
                "The original guardians remain. Their wounds and spent lessons persist; retreat cannot replace them. Defeat every guardian to unlock the piece."
            }
            WatchPhase::Cleared => "The trial is broken forever. Use get all here to claim the covenant piece.",
        }
    }

    pub(crate) fn guards_to_place(&self) -> Vec<(Kind, Pos)> {
        let (x, y) = self.pos;
        match self.kind {
            LandmarkKind::AshenForge => vec![(Kind::AshColossus, (x, y - 2))],
            _ => vec![(Kind::HollowCantor, (x, y - 2)), (Kind::HollowCantor, (x, y + 2))],
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

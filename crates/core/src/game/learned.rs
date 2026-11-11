//! Learned actions use the normal turn loop and shared damage/practice accounting.

use super::*;
use crate::{LearnedMove, MovePreview};

#[cfg(test)]
mod tests;

struct Plan {
    preview: MovePreview,
    targets: Vec<MonsterId>,
}

impl Game {
    /// Preview an earned action, including equipment, Focus, target and movement requirements.
    /// Rejection never changes state or spends time.
    pub fn preview_move(&self, action: LearnedMove, dir: Dir) -> Result<MovePreview, &'static str> {
        self.move_plan(action, dir).map(|plan| plan.preview)
    }

    /// Use an earned action without replacing the prepared technique. Every accepted action takes
    /// exactly one normal turn. Focus is paid once even if all strikes miss.
    pub fn use_move(&mut self, action: LearnedMove, dir: Dir) -> Step {
        let plan = match self.move_plan(action, dir) {
            Ok(plan) => plan,
            Err(reason) => {
                self.log.push(LogKind::Blocked, reason);
                return Step::Blocked;
            }
        };
        self.turn += 1;
        self.wanderer.focus -= plan.preview.focus;
        self.log.push(LogKind::Attack, format!("{}: {} Focus.", action.name(), plan.preview.focus));
        if action == LearnedMove::SlipAway {
            self.sneaking = true;
            self.position = plan.preview.destination.expect("slip has a destination");
            self.update_region(true);
            self.sneak_moves = self.sneak_moves.wrapping_add(2);
            self.crept = true;
            self.last_attack = None;
            self.end_turn();
            return Step::Moved;
        }

        let maneuver = self.maneuver.basic_strike();
        let mut training = self.wanderer.training(maneuver.weapon);
        // These strikes are paid learned moves, not the free bump strike Veiled movement weakens.
        training.veiled = false;
        let mut rng = Rng::keyed(self.world.seed(), b"wanderer", &[self.turn]);
        // Resolve all hits against the same pre-action training/context as the preview. A rank
        // earned by the first sweep hit cannot silently strengthen later hits above that preview.
        let results: Vec<_> = plan
            .targets
            .iter()
            .map(|&id| {
                let target = self.monsters.iter().find(|m| m.id == id).expect("planned target");
                let result = if action == LearnedMove::Pulse {
                    if rng.chance(crate::MISS_CHANCE) {
                        AttackResult::default()
                    } else {
                        AttackResult {
                            hit: true,
                            burst: 1 + self.burst_bonus(),
                            burst_released: true,
                            ..AttackResult::default()
                        }
                    }
                } else {
                    maneuver
                        .resolve_with_item(self.attack_context(target), training, self.equipped, &mut rng)
                        .expect("basic strike and matching equipment")
                };
                (id, result)
            })
            .collect();
        let mut threat = 0;
        let mut total = AttackResult::default();
        for (id, result) in results {
            let i = self.monsters.iter().position(|m| m.id == id).expect("distinct planned target");
            let drawn = if result.hit && self.monsters[i].practice_left > 0 { plan.preview.focus_gain } else { 0 };
            let source = if action == LearnedMove::Pulse {
                AttackSource::Pulse(maneuver.weapon)
            } else {
                AttackSource::Maneuver(&maneuver)
            };
            threat = threat.max(self.record_attack(i, result, source, 0));
            total.hit |= result.hit;
            total.physical = total.physical.saturating_add(result.physical);
            total.burst = total.burst.saturating_add(result.burst);
            total.penetrated |= result.penetrated;
            total.burst_released |= result.burst_released;
            if drawn > 0 {
                self.wanderer.focus += drawn;
                self.log.push(LogKind::Attack, format!("{} draws {drawn} Focus.", action.name()));
            }
            if action == LearnedMove::Shove
                && result.hit
                && let Some(push) = plan.preview.push
                && let Some(m) = self.monsters.iter_mut().find(|m| m.id == id)
            {
                m.position = push;
            }
        }
        self.last_attack = Some(total);
        self.learn_focus(plan.preview.focus, threat);
        if let Some(destination) = plan.preview.destination {
            self.position = destination;
            self.update_region(true);
        }
        self.end_turn();
        Step::Attacked
    }

    fn move_plan(&self, action: LearnedMove, dir: Dir) -> Result<Plan, &'static str> {
        if !action.unlocked(&self.wanderer) {
            return Err(action.requirement());
        }
        if action.weapon().is_some_and(|weapon| weapon != self.maneuver.weapon) {
            return Err("Equip the weapon that earned this move.");
        }
        let focus_cost = action.focus_cost_with_item(self.equipped);
        if self.wanderer.focus < focus_cost {
            return Err("Not enough Focus for the learned move. Bump strikes remain free.");
        }
        let (dx, dy) = dir.delta();
        let at = self.position;
        let near = (at.0 + dx, at.1 + dy);
        let far = (at.0 + 2 * dx, at.1 + 2 * dy);
        let empty =
            |p: (i64, i64)| self.world.tile(p.0, p.1).biome.biome().passable && self.monster_at(p.0, p.1).is_none();
        let mut preview = MovePreview {
            focus: focus_cost,
            focus_gain: 0,
            targets: 0,
            min_hit: 0,
            max_hit: 0,
            destination: None,
            push: None,
        };
        let targets = match action {
            LearnedMove::SlipAway => {
                if !empty(near) || !empty(far) {
                    return Err("Slip away needs two empty, passable tiles.");
                }
                preview.destination = Some(far);
                Vec::new()
            }
            LearnedMove::Sweep => {
                let targets: Vec<_> = Dir::ALL
                    .into_iter()
                    .filter_map(|d| {
                        let (dx, dy) = d.delta();
                        self.monster_at(at.0 + dx, at.1 + dy).map(|m| m.id)
                    })
                    .collect();
                if targets.is_empty() {
                    return Err("No adjacent targets to sweep.");
                }
                targets
            }
            _ => {
                let target_at = if action == LearnedMove::Reach {
                    if !empty(near) {
                        return Err("Reach needs an empty, passable intervening tile.");
                    }
                    far
                } else {
                    near
                };
                let m = self.monster_at(target_at.0, target_at.1).ok_or("No target at this move's range.")?;
                if action == LearnedMove::Withdraw {
                    let back = (at.0 - dx, at.1 - dy);
                    if !empty(back) {
                        return Err("No empty, passable tile to withdraw into.");
                    }
                    preview.destination = Some(back);
                }
                if action == LearnedMove::Shove {
                    let i = self.monsters.iter().position(|target| target.id == m.id).expect("target index");
                    if self.open_to(i, far) {
                        preview.push = Some(far);
                    }
                }
                if m.practice_left > 0 {
                    let room = self.wanderer.max_focus() - (self.wanderer.focus - preview.focus);
                    preview.focus_gain = action.focus_on_hit().min(room).max(0);
                }
                vec![m.id]
            }
        };
        for &id in &targets {
            let target = self.monsters.iter().find(|m| m.id == id).expect("preview target");
            let (min, max) = if action == LearnedMove::Pulse {
                (1 + self.burst_bonus(), 1 + self.burst_bonus())
            } else {
                let mut training = self.wanderer.training(self.maneuver.weapon);
                training.veiled = false;
                let p = self.maneuver.basic_strike().preview_with_item(
                    self.attack_context(target),
                    training,
                    self.equipped,
                )?;
                (p.min_hit, p.max_hit)
            };
            preview.min_hit = preview.min_hit.saturating_add(min);
            preview.max_hit = preview.max_hit.saturating_add(max);
        }
        preview.targets = targets.len();
        Ok(Plan { preview, targets })
    }
}

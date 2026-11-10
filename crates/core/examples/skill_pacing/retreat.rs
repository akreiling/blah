//! Conservative, visible-information decisions and explicit retreat/recovery accounting.

use super::*;
use blah_core::{ACTION_COST, AttackContext, MAX_ACTIONS, Monster, TERRITORY};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Policy {
    Baseline,
    Cautious,
}

impl Policy {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Cautious => "cautious",
        }
    }
}

fn hostile(m: &Monster, at: (i64, i64)) -> bool {
    m.phenotype.stats.speed > 0
        && match m.phenotype.temperament {
            Temperament::Aggressive => true,
            Temperament::Territorial => distance(at, m.home) <= TERRITORY,
            Temperament::Skittish => false,
        }
}

fn incoming(game: &Game, m: &Monster) -> i32 {
    (m.phenotype.stats.attack - game.wanderer().stats.defense + 1).max(1)
}

/// Pessimistic guarded-fight estimate: minimum previewed strike, two extra miss turns,
/// maximum incoming rolls and rounded-up action count. Not an authoritative combat preview.
fn fight_cost(game: &Game, m: &Monster) -> i32 {
    let w = game.wanderer();
    let hit = game
        .maneuver()
        .basic_strike()
        .preview_trained(
            AttackContext {
                attack: w.stats.attack,
                defense: m.phenotype.stats.defense,
                unguarded: false,
                fire_resistance: m.fire_resistance(),
            },
            w.training(game.maneuver().weapon),
        )
        .expect("a basic strike is legal")
        .min_hit
        .max(1);
    let turns = (m.hp + hit - 1) / hit + 2;
    let speed = m.phenotype.stats.speed.clamp(0, ACTION_COST * MAX_ACTIONS as i32);
    incoming(game, m) * ((turns * speed + ACTION_COST - 1) / ACTION_COST)
}

fn adjacent(game: &Game, dir: Dir) -> Option<&Monster> {
    let (dx, dy) = dir.delta();
    let at = game.position();
    game.monster_at(at.0 + dx, at.1 + dy)
}

/// Avoid damage and exposure while retreating or near unaffordable fights; otherwise follow the route.
/// Occupied tiles are never treated as escape moves. Hidden creatures and banked energy are not read.
fn safe_step(game: &Game, goal: (i64, i64), retreating: bool) -> Option<Dir> {
    let at = game.position();
    let route = toward(game, goal, true);
    Dir::ALL
        .into_iter()
        .filter(|&dir| game.can_step(dir) && adjacent(game, dir).is_none())
        .map(|dir| {
            let (dx, dy) = dir.delta();
            let next = (at.0 + dx, at.1 + dy);
            let mut damage = 0;
            let mut exposure = 0;
            for m in game.monsters_in_view().filter(|m| hostile(m, next)) {
                let concealment = if game.sneaking() { game.wanderer().stealth.sneak_reduction() } else { 0 };
                if !m.detects(next, concealment) {
                    continue;
                }
                let moves = (m.phenotype.stats.speed.max(0) + ACTION_COST - 1) / ACTION_COST;
                let gap = (next.0 - m.position.0).abs() + (next.1 - m.position.1).abs();
                if retreating || fight_cost(game, m) > game.wanderer().hp - 10 {
                    damage += incoming(game, m) * (i64::from(moves.min(MAX_ACTIONS as i32)) - (gap - 1)).max(0) as i32;
                    exposure += (m.phenotype.sense + 1 - distance(next, m.position)).max(0);
                }
            }
            ((damage, exposure, usize::from(route != Some(dir)), distance(next, goal)), dir)
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, dir)| dir)
}

impl Pilot {
    pub(super) fn choose_cautious(&mut self, game: &Game, style: Style) -> Action {
        let w = game.wanderer();
        let specialist = !matches!(style, Style::Fighter(_));
        self.returning |= w.hp <= 2 * w.max_hp() / 3 || (specialist && w.focus < 14);
        if self.returning && game.position() == game.home() {
            if w.hp == w.max_hp() && w.focus == w.max_focus() {
                self.returning = false;
                self.waypoint += 1;
            } else if game.can_rest() {
                return Action::Rest;
            }
        }
        let danger: i32 = game
            .monsters_in_view()
            .filter(|m| hostile(m, game.position()) && m.detects(game.position(), 0))
            .map(|m| fight_cost(game, m))
            .sum();
        self.returning |= danger > (w.hp - 10).max(0);
        if self.returning {
            // Movement comes before adjacency attacks. Toggling sneak costs a turn and does not
            // erase pursuit, so keep moving unless the next paid step is unaffordable.
            if game.sneaking() && w.focus == 0 {
                return Action::ToggleSneak;
            }
            if let Some(dir) = safe_step(game, game.home(), true) {
                return Action::Move(dir);
            }
            // Cornered, prefer the cheapest real escape opening over standing still.
            if let Some(dir) = Dir::ALL
                .into_iter()
                .filter(|&d| adjacent(game, d).is_some())
                .min_by_key(|&d| adjacent(game, d).unwrap().hp)
            {
                return Action::Strike(dir);
            }
            return Action::Wait;
        }
        let target = game
            .monsters_in_view()
            .filter(|m| m.phenotype.temperament != Temperament::Skittish && fight_cost(game, m) <= w.hp - 10)
            .min_by_key(|m| (distance(game.position(), m.position), m.id));
        if let Some(m) = target {
            if let Some(dir) = Dir::ALL.into_iter().find(|&d| adjacent(game, d).is_some_and(|a| a.id == m.id)) {
                let paid = match style {
                    Style::Fighter(_) => false,
                    Style::Sneak => game.preview_attack(dir).is_ok_and(|p| p.opening),
                    Style::Caster => true,
                };
                if paid && w.focus >= game.maneuver().technique.focus_cost() {
                    return Action::Technique(dir);
                }
                return Action::Strike(dir);
            }
            if matches!(style, Style::Sneak) && !game.sneaking() && w.focus >= 14 {
                return Action::ToggleSneak;
            }
        }
        let (hx, hy) = game.home();
        let offsets = [(6, 0), (0, 6), (-6, 0), (0, -6)];
        let offset = offsets[self.waypoint % offsets.len()];
        let goal = target.map_or((hx + offset.0, hy + offset.1), |m| m.position);
        // Rotate inaccessible waypoints; movement still prioritizes local safety.
        let route = toward(game, goal, true);
        let safe = safe_step(game, goal, false);
        if game.position() == goal || route.is_none() {
            self.waypoint += 1;
        }
        safe.map_or(Action::Wait, Action::Move)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Attempt {
    start: u64,
    wake: bool,
    pursuers: HashSet<MonsterId>,
    escaped: bool,
    arrived: bool,
    rests: u64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Recovery {
    starts: u64,
    escapes: u64,
    recovered: u64,
    deaths: u64,
    wakes: u64,
    wake_recovered: u64,
    wake_deaths: u64,
    blocked: u64,
    arrivals: Vec<u64>,
    durations: Vec<u64>,
    rests: Vec<u64>,
    wake_durations: Vec<u64>,
    wake_rests: Vec<u64>,
    active: Option<Attempt>,
}

impl Recovery {
    pub(super) fn before(&mut self, game: &Game, action: u64, returning: bool, chosen: Action) {
        if returning && self.active.is_none() {
            self.starts += 1;
            self.active = Some(Attempt {
                start: action,
                wake: false,
                pursuers: HashSet::new(),
                escaped: false,
                arrived: false,
                rests: 0,
            });
        }
        if let Some(attempt) = &mut self.active {
            attempt.rests += u64::from(matches!(chosen, Action::Rest));
            attempt.pursuers.extend(
                game.monsters_in_view()
                    .filter(|m| m.alerted && hostile(m, game.position()) && m.detects(game.position(), 0))
                    .map(|m| m.id),
            );
            let w = game.wanderer();
            self.blocked += u64::from(
                game.position() == game.home() && (w.hp < w.max_hp() || w.focus < w.max_focus()) && !game.can_rest(),
            );
        }
    }

    pub(super) fn after(&mut self, game: &Game, action: u64, deaths_before: u32) {
        if game.deaths() > deaths_before {
            if let Some(attempt) = self.active.take() {
                if attempt.wake {
                    self.wake_deaths += 1;
                } else {
                    self.deaths += 1;
                }
            }
            self.wakes += 1;
            self.active = Some(Attempt {
                start: action + 1,
                wake: true,
                pursuers: HashSet::new(),
                escaped: false,
                arrived: true,
                rests: 0,
            });
            return;
        }
        if let Some(attempt) = &mut self.active {
            if !attempt.wake && !attempt.escaped && escaped(&attempt.pursuers, game.monsters(), game.position()) {
                attempt.escaped = true;
                self.escapes += 1;
            }
            if !attempt.arrived && game.position() == game.home() {
                attempt.arrived = true;
                self.arrivals.push(action - attempt.start + 1);
            }
            let w = game.wanderer();
            if game.position() == game.home() && w.hp == w.max_hp() && w.focus == w.max_focus() {
                if attempt.wake {
                    self.wake_recovered += 1;
                    self.wake_durations.push(action - attempt.start + 1);
                    self.wake_rests.push(attempt.rests);
                } else {
                    self.recovered += 1;
                    self.durations.push(action - attempt.start + 1);
                    self.rests.push(attempt.rests);
                }
                self.active = None;
            }
        }
    }

    pub(super) fn csv(&self) -> String {
        fn median(values: &[u64]) -> String {
            let mut values = values.to_vec();
            values.sort_unstable();
            values.get(values.len() / 2).map_or_else(|| "-".into(), u64::to_string)
        }
        let unfinished = u64::from(self.active.as_ref().is_some_and(|a| !a.wake));
        let wake_unfinished = u64::from(self.active.as_ref().is_some_and(|a| a.wake));
        assert_eq!(self.starts, self.recovered + self.deaths + unfinished);
        assert_eq!(self.wakes, self.wake_recovered + self.wake_deaths + wake_unfinished);
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.starts,
            self.escapes,
            self.recovered,
            self.deaths,
            unfinished,
            self.wakes,
            self.wake_recovered,
            self.wake_deaths,
            wake_unfinished,
            self.blocked,
            median(&self.arrivals),
            median(&self.durations),
            median(&self.rests),
            median(&self.wake_durations),
            median(&self.wake_rests)
        )
    }
}

/// Full-state scoring is separate from the pilot's visible-only decisions. Losing sight of a
/// long-sensed pursuer is not escape; killing or clearing every pursuer is not escape either.
fn escaped(pursuers: &HashSet<MonsterId>, monsters: &[Monster], at: (i64, i64)) -> bool {
    monsters.iter().any(|m| pursuers.contains(&m.id))
        && !monsters.iter().any(|m| m.alerted && hostile(m, at) && m.detects(at, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn losing_view_is_not_escape_and_killing_is_not_escape() {
        let game = Game::composition_trial(42, Maneuver::default()).unwrap();
        let mut m = game.monsters()[0].clone();
        let at = game.position();
        let ids = HashSet::from([m.id]);
        m.alerted = true;
        m.position = (at.0 + blah_core::VIEW_RADIUS + 1, at.1);
        m.phenotype.sense = blah_core::VIEW_RADIUS + 2;
        assert!(!escaped(&ids, &[m.clone()], at), "an out-of-view observer still tracks");
        assert!(!escaped(&ids, &[], at), "all pursuers removed is not escape");
        m.alerted = false;
        assert!(escaped(&ids, &[m.clone()], at));
        m.alerted = true;
        m.phenotype.temperament = Temperament::Territorial;
        m.home = (at.0 + TERRITORY + 1, at.1);
        assert!(escaped(&ids, &[m], at), "leaving defended ground ends territorial pursuit");
    }

    #[test]
    fn cautious_pilot_moves_instead_of_attacking_an_adjacent_bad_fight() {
        let game = Game::composition_trial(42, Maneuver::default()).unwrap();
        assert!(matches!(Pilot::default().choose_baseline(&game, Style::Fighter(Weapon::Dagger)), Action::Strike(_)));
        assert!(matches!(Pilot::default().choose_cautious(&game, Style::Fighter(Weapon::Dagger)), Action::Move(_)));
    }

    #[test]
    fn death_ends_retreat_and_starts_a_separate_recovery_episode() {
        let mut game = Game::composition_trial(42, Maneuver::default()).unwrap();
        game.toggle_sneak(); // The trial starts concealed; expose it so waiting really is dangerous.
        let mut r = Recovery::default();
        let mut action = 0;
        while game.deaths() == 0 {
            action += 1;
            r.before(&game, action, true, Action::Wait);
            let deaths = game.deaths();
            game.wait();
            r.after(&game, action, deaths);
            assert!(action < 100);
        }
        assert_eq!(r.starts, 1);
        assert_eq!(r.deaths, 1);
        assert_eq!(r.escapes, 0);
        assert_eq!(r.wakes, 1);
        while game.can_rest() {
            action += 1;
            r.before(&game, action, true, Action::Rest);
            let deaths = game.deaths();
            assert_eq!(Action::Rest.apply(&mut game), Step::Waited);
            r.after(&game, action, deaths);
        }
        assert_eq!(r.wake_recovered, 1);
        assert_eq!(r.wake_deaths, 0);
        assert_eq!(r.wake_rests.len(), 1);
        assert!((8..=15).contains(&r.wake_rests[0]));
        assert!(r.active.is_none());
        r.csv(); // Includes conservation assertions for both episode types.
    }
}

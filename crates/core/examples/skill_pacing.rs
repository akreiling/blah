//! Fresh-character pacing probes, not a model of human play or a balanced progression target.
//! Run with `just skill-pacing [seeds] [actions]`. No injected practice, monsters, or resources.

use std::collections::{HashSet, VecDeque};

use blah_core::{
    Aim, Burst, Delivery, Dir, Element, Game, LogKind, Maneuver, MonsterId, Rank, Shape, Step, Technique, Temperament,
    Trigger, Weapon,
};

#[derive(Clone, Copy, Debug)]
enum Style {
    Fighter(Weapon),
    Sneak,
    Caster,
}

impl Style {
    const ALL: [Self; 6] = [
        Self::Fighter(Weapon::Dagger),
        Self::Fighter(Weapon::Spear),
        Self::Fighter(Weapon::Club),
        Self::Fighter(Weapon::Staff),
        Self::Sneak,
        Self::Caster,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Fighter(Weapon::Dagger) => "dagger",
            Self::Fighter(Weapon::Spear) => "spear",
            Self::Fighter(Weapon::Club) => "club",
            Self::Fighter(Weapon::Staff) => "staff",
            Self::Sneak => "sneak",
            Self::Caster => "caster",
        }
    }

    fn maneuver(self) -> Maneuver {
        match self {
            Self::Fighter(weapon) => Maneuver { weapon, ..Maneuver::default() },
            Self::Sneak => Maneuver::backstab_burst(),
            Self::Caster => Maneuver {
                name: "Contact flame".into(),
                weapon: Weapon::Staff,
                technique: Technique {
                    delivery: Delivery::Touch,
                    aim: Aim::Direct,
                    burst: Some(Burst {
                        power: 3,
                        trigger: Trigger::Contact,
                        element: Element::Fire,
                        shape: Shape::Point,
                    }),
                },
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Milestone {
    actions: u64,
    encounters: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct Report {
    actions: u64,
    turns: u64,
    encounters: usize,
    kills: u64,
    deaths: u32,
    rests: u64,
    techniques: u64,
    sneak_moves: u64,
    waits: u64,
    ranks: [Rank; 3],
    milestones: [[Option<Milestone>; 4]; 3],
}

fn distance(a: (i64, i64), b: (i64, i64)) -> i64 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// Shortest path through the current visible square. Only visible monsters affect decisions.
/// If the goal is outside that square or unreachable, take the reachable tile nearest it.
fn toward(game: &Game, goal: (i64, i64), avoid: bool) -> Option<Dir> {
    let origin = game.position();
    let occupied: HashSet<_> = game.monsters_in_view().map(|m| m.position).collect();
    let mut seen = HashSet::from([origin]);
    let mut queue = VecDeque::from([(origin, None)]);
    let mut best = (distance(origin, goal), None);
    while let Some((at, first)) = queue.pop_front() {
        let gap = distance(at, goal);
        if gap < best.0 {
            best = (gap, first);
        }
        if at == goal {
            return first;
        }
        for dir in Dir::ALL {
            let (dx, dy) = dir.delta();
            let next = (at.0 + dx, at.1 + dy);
            if distance(origin, next) > blah_core::VIEW_RADIUS
                || !game.world().tile(next.0, next.1).biome.biome().passable
                || (occupied.contains(&next) && (avoid || next != goal))
                || !seen.insert(next)
            {
                continue;
            }
            queue.push_back((next, first.or(Some(dir))));
        }
    }
    best.1
}

#[derive(Default)]
struct Pilot {
    returning: bool,
    waypoint: usize,
    clearing_home: bool,
}

#[derive(Clone, Copy)]
enum Action {
    Rest,
    ToggleSneak,
    Strike(Dir),
    Technique(Dir),
    Move(Dir),
    Wait,
}

impl Action {
    fn apply(self, game: &mut Game) -> Step {
        match self {
            Self::Rest => game.recover_focus(),
            Self::ToggleSneak => game.toggle_sneak(),
            Self::Strike(dir) | Self::Move(dir) => game.step(dir),
            Self::Technique(dir) => game.use_technique(dir),
            Self::Wait => game.wait(),
        }
    }
}

impl Pilot {
    fn choose(&mut self, game: &Game, style: Style) -> Action {
        let w = game.wanderer();
        let specialist = !matches!(style, Style::Fighter(_));
        self.returning |= w.hp <= 10 || (specialist && w.focus < 8);
        if game.can_prepare() {
            self.clearing_home = false;
        }
        if game.position() == game.home() && self.returning {
            if w.hp == w.max_hp() && w.focus == w.max_focus() {
                self.returning = false;
                self.waypoint += 1;
            } else if game.can_prepare() {
                return Action::Rest;
            } else {
                self.clearing_home = true;
            }
        }
        if game.sneaking() && (self.returning || game.wanderer().focus < 8) {
            return Action::ToggleSneak;
        }
        let adjacent = Dir::ALL.into_iter().find(|&dir| game.preview_strike(dir).is_ok());
        if let Some(dir) = adjacent {
            let paid = match style {
                Style::Fighter(_) => false,
                Style::Sneak => game.preview_attack(dir).is_ok_and(|p| p.opening),
                Style::Caster => true,
            };
            if paid && game.wanderer().focus >= game.maneuver().technique.focus_cost() {
                return Action::Technique(dir);
            }
            return Action::Strike(dir);
        }
        let target = game
            .monsters_in_view()
            .filter(|m| m.phenotype.temperament != Temperament::Skittish)
            .min_by_key(|m| (distance(game.position(), m.position), m.id))
            .map(|m| m.position);
        if matches!(style, Style::Sneak) && !self.returning && !game.sneaking() && target.is_some() {
            return Action::ToggleSneak;
        }
        let (hx, hy) = game.home();
        let offsets = [(6, 0), (0, 6), (-6, 0), (0, -6)];
        let offset = offsets[self.waypoint % offsets.len()];
        let waypoint = (hx + offset.0, hy + offset.1);
        let goal = if self.returning && !self.clearing_home { game.home() } else { target.unwrap_or(waypoint) };
        if let Some(dir) = toward(game, goal, self.returning && !self.clearing_home)
            && game.can_step(dir)
        {
            return Action::Move(dir);
        }
        // An inaccessible waypoint must not trap the pilot on an island or behind a coast.
        self.waypoint += 1;
        Action::Wait
    }
}

fn ranks(game: &Game) -> [Rank; 3] {
    let w = game.wanderer();
    [w.weapons.get(game.maneuver().weapon).rank(), w.focus_skill.rank(), w.stealth.rank()]
}

fn run(seed: u64, actions: u64, style: Style) -> Report {
    let mut game = Game::new(seed);
    game.prepare(style.maneuver()).expect("initial creatures are outside view");
    let mut pilot = Pilot::default();
    let mut encounters: HashSet<MonsterId> = HashSet::new();
    let mut report = Report {
        actions: 0,
        turns: 0,
        encounters: 0,
        kills: 0,
        deaths: 0,
        rests: 0,
        techniques: 0,
        sneak_moves: 0,
        waits: 0,
        ranks: ranks(&game),
        milestones: [[None; 4]; 3],
    };
    for action in 1..=actions {
        let before_log = game.log().total();
        let before_turn = game.turn();
        // An encounter means a visible, non-skittish observer in its normal sensing range,
        // or a non-skittish attack target, not a kill or an invented fight boundary.
        encounters.extend(
            game.monsters_in_view()
                .filter(|m| {
                    m.phenotype.temperament != Temperament::Skittish
                        && distance(game.position(), m.position) <= m.phenotype.sense
                })
                .map(|m| m.id),
        );
        let chosen = pilot.choose(&game, style);
        if let Action::Strike(dir) | Action::Technique(dir) = chosen {
            let (dx, dy) = dir.delta();
            let at = game.position();
            if let Some(m) = game.monster_at(at.0 + dx, at.1 + dy)
                && m.phenotype.temperament != Temperament::Skittish
            {
                encounters.insert(m.id);
            }
        }
        let result = chosen.apply(&mut game);
        assert_ne!(result, Step::Blocked, "the pilot must spend an action");
        assert!(game.turn() > before_turn);
        report.kills +=
            game.log().last((game.log().total() - before_log) as usize).filter(|e| e.kind == LogKind::Kill).count()
                as u64;
        report.rests += u64::from(matches!(chosen, Action::Rest));
        report.techniques += u64::from(matches!(chosen, Action::Technique(_)));
        report.sneak_moves += u64::from(result == Step::Moved && game.sneaking());
        report.waits += u64::from(matches!(chosen, Action::Wait));
        // Include observers reached by this action, even if the next action retreats.
        encounters.extend(
            game.monsters_in_view()
                .filter(|m| {
                    m.phenotype.temperament != Temperament::Skittish
                        && distance(game.position(), m.position) <= m.phenotype.sense
                })
                .map(|m| m.id),
        );
        let after = ranks(&game);
        for (skill, rank) in after.into_iter().enumerate() {
            for band in 0..rank as usize {
                report.milestones[skill][band]
                    .get_or_insert(Milestone { actions: action, encounters: encounters.len() });
            }
        }
        report.ranks = after;
    }
    report.actions = actions;
    report.turns = game.turn();
    report.deaths = game.deaths();
    report.encounters = encounters.len();
    report
}

fn positive_arg(value: Option<String>, default: u64) -> Result<u64, String> {
    value.map_or(Ok(default), |s| {
        s.parse::<u64>().ok().filter(|&n| n > 0).ok_or_else(|| format!("Expected a positive integer, got {s:?}"))
    })
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let seeds = positive_arg(args.next(), 8)?;
    let actions = positive_arg(args.next(), 600)?;
    if args.next().is_some() {
        return Err("Usage: skill-pacing [seeds] [actions]".into());
    }
    println!(
        "style,seed,actions,turns,encounters,kills,deaths,rests,techniques,sneak_moves,waits,weapon,focus,stealth"
    );
    let mut reports = Vec::new();
    for style in Style::ALL {
        for seed in 0..seeds {
            let r = run(seed, actions, style);
            println!(
                "{},{seed},{},{},{},{},{},{},{},{},{},{},{},{}",
                style.name(),
                r.actions,
                r.turns,
                r.encounters,
                r.kills,
                r.deaths,
                r.rests,
                r.techniques,
                r.sneak_moves,
                r.waits,
                r.ranks[0].name(),
                r.ranks[1].name(),
                r.ranks[2].name()
            );
            reports.push((style, r));
        }
    }
    println!("\nstyle,skill,rank,reached,runs,actions_min,actions_median,actions_max,encounters_median");
    for style in Style::ALL {
        for (skill, name) in ["weapon", "focus", "stealth"].into_iter().enumerate() {
            for (band, rank) in ["practiced", "skilled", "adept", "expert"].into_iter().enumerate() {
                let milestones: Vec<_> = reports
                    .iter()
                    .filter(|(s, _)| s.name() == style.name())
                    .filter_map(|(_, r)| r.milestones[skill][band])
                    .collect();
                let mut times: Vec<_> = milestones.iter().map(|m| m.actions).collect();
                let mut encounters: Vec<_> = milestones.iter().map(|m| m.encounters).collect();
                times.sort_unstable();
                encounters.sort_unstable();
                let stats = if times.is_empty() {
                    "-,-,-,-".into()
                } else {
                    format!(
                        "{},{},{},{}",
                        times[0],
                        times[times.len() / 2],
                        times[times.len() - 1],
                        encounters[encounters.len() / 2]
                    )
                };
                println!("{},{name},{rank},{},{seeds},{stats}", style.name(), times.len());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_runs_are_deterministic_and_count_death_time_separately() {
        for style in Style::ALL {
            let a = run(42, 120, style);
            assert_eq!(a, run(42, 120, style));
            assert_eq!(a.turns, a.actions + u64::from(a.deaths) * blah_core::DEATH_TURNS);
            for skill in a.milestones {
                let reached: Vec<_> = skill.into_iter().flatten().collect();
                assert!(reached.windows(2).all(|pair| pair[0].actions <= pair[1].actions));
            }
        }
    }

    #[test]
    fn free_fighters_never_train_focus_and_caster_really_casts() {
        let fighter = run(0, 240, Style::Fighter(Weapon::Club));
        assert_eq!(fighter.ranks[1], Rank::Novice);
        assert_eq!(fighter.techniques, 0);
        let caster = run(0, 240, Style::Caster);
        assert!(caster.techniques > 0);
        let sneak = run(0, 240, Style::Sneak);
        assert!(sneak.sneak_moves > 0);
    }

    #[test]
    fn cli_rejects_zero_and_invalid_counts() {
        for value in ["0", "-1", "bad"] {
            assert!(positive_arg(Some(value.into()), 8).is_err());
        }
        assert_eq!(positive_arg(None, 8), Ok(8));
    }
}

//! Fresh-character pacing probes, not a model of human play or a balanced progression target.
//! Run with `just skill-pacing [seeds] [actions] [baseline|cautious|compare|move-compare]`.
//! No injected practice, monsters, or resources.

use std::collections::{HashSet, VecDeque};

use blah_core::{
    Aim, Burst, Delivery, Dir, Element, Game, LearnedMove, LogKind, Maneuver, MonsterId, Rank, Shape, Step, Technique,
    Temperament, Trigger, Weapon,
};

#[path = "skill_pacing/retreat.rs"]
mod retreat;
use retreat::{Policy, Recovery};

const LEARNED_MOVES: [LearnedMove; 6] = [
    LearnedMove::Withdraw,
    LearnedMove::Reach,
    LearnedMove::Shove,
    LearnedMove::Sweep,
    LearnedMove::SlipAway,
    LearnedMove::Pulse,
];
const POLICIES: [Policy; 4] = [Policy::Baseline, Policy::BaselineMoves, Policy::Cautious, Policy::CautiousMoves];

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
    health: i32,
    focus: i32,
    ranks: [Rank; 3],
    milestones: [[Option<Milestone>; 4]; 3],
    moves: [MoveStats; 6],
    recovery: Recovery,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MoveStats {
    unlocked_at: Option<u64>,
    opportunities: u64,
    uses: u64,
    hit_actions: u64,
    targets: u64,
    damage: u64,
    focus_spent: u64,
    steps: u64,
    pushes: u64,
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
    Learned(LearnedMove, Dir),
    Wait,
}

impl Action {
    fn apply(self, game: &mut Game) -> Step {
        match self {
            Self::Rest => game.recover_focus(),
            Self::ToggleSneak => game.toggle_sneak(),
            Self::Strike(dir) | Self::Move(dir) => game.step(dir),
            Self::Technique(dir) => game.use_technique(dir),
            Self::Learned(action, dir) => game.use_move(action, dir),
            Self::Wait => game.wait(),
        }
    }
}

fn move_aware_action(game: &Game, style: Style, retreating: bool, fallback: Action) -> Action {
    let weapon_move = LearnedMove::for_weapon(game.maneuver().weapon);
    match fallback {
        Action::Strike(dir) => {
            if matches!(style, Style::Caster)
                && game.wanderer().focus < game.maneuver().technique.focus_cost()
                && game.preview_move(LearnedMove::Pulse, dir).is_ok()
            {
                return Action::Learned(LearnedMove::Pulse, dir);
            }
            match weapon_move {
                LearnedMove::Withdraw if game.preview_move(weapon_move, dir).is_ok() => {
                    Action::Learned(weapon_move, dir)
                }
                LearnedMove::Shove if game.preview_move(weapon_move, dir).is_ok_and(|p| p.push.is_some()) => {
                    Action::Learned(weapon_move, dir)
                }
                LearnedMove::Sweep if game.preview_move(weapon_move, dir).is_ok_and(|p| p.targets >= 2) => {
                    Action::Learned(weapon_move, dir)
                }
                _ => fallback,
            }
        }
        Action::Move(dir) => {
            if !retreating && weapon_move == LearnedMove::Reach && game.preview_move(weapon_move, dir).is_ok() {
                return Action::Learned(weapon_move, dir);
            }
            if retreating
                && let Ok(preview) = game.preview_move(LearnedMove::SlipAway, dir)
                && let Some(destination) = preview.destination
            {
                let at = game.position();
                let pursuers: Vec<_> = game
                    .monsters_in_view()
                    .filter(|m| m.alerted && m.phenotype.temperament != Temperament::Skittish)
                    .collect();
                if pursuers.is_empty()
                    || pursuers.iter().all(|m| distance(destination, m.position) > distance(at, m.position))
                {
                    return Action::Learned(LearnedMove::SlipAway, dir);
                }
            }
            fallback
        }
        _ => fallback,
    }
}

fn action_targets(game: &Game, action: Action) -> Vec<MonsterId> {
    let at = game.position();
    let mut targets = Vec::new();
    let mut add = |position: (i64, i64)| {
        if let Some(monster) = game.monster_at(position.0, position.1) {
            targets.push(monster.id);
        }
    };
    match action {
        Action::Strike(dir) | Action::Technique(dir) => {
            let (dx, dy) = dir.delta();
            add((at.0 + dx, at.1 + dy));
        }
        Action::Learned(LearnedMove::Withdraw | LearnedMove::Shove | LearnedMove::Pulse, dir) => {
            let (dx, dy) = dir.delta();
            add((at.0 + dx, at.1 + dy));
        }
        Action::Learned(LearnedMove::Reach, dir) => {
            let (dx, dy) = dir.delta();
            add((at.0 + 2 * dx, at.1 + 2 * dy));
        }
        Action::Learned(LearnedMove::Sweep, _) => {
            for dir in Dir::ALL {
                let (dx, dy) = dir.delta();
                add((at.0 + dx, at.1 + dy));
            }
        }
        Action::Rest
        | Action::ToggleSneak
        | Action::Move(_)
        | Action::Learned(LearnedMove::SlipAway, _)
        | Action::Wait => {}
    }
    targets
}

impl Pilot {
    fn choose_baseline(&mut self, game: &Game, style: Style) -> Action {
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
            } else if game.can_rest() {
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

fn move_index(action: LearnedMove) -> usize {
    LEARNED_MOVES.iter().position(|&candidate| candidate == action).expect("all learned actions are reported")
}

fn manhattan(a: (i64, i64), b: (i64, i64)) -> u64 {
    ((a.0 - b.0).abs() + (a.1 - b.1).abs()) as u64
}

fn record_opportunities(game: &Game, stats: &mut [MoveStats; 6]) {
    for (index, learned) in LEARNED_MOVES.into_iter().enumerate() {
        if learned.unlocked(game.wanderer()) {
            let metric = &mut stats[index];
            if Dir::ALL.into_iter().any(|dir| game.preview_move(learned, dir).is_ok()) {
                metric.opportunities += 1;
            }
        }
    }
}

fn record_unlocks(game: &Game, action: u64, stats: &mut [MoveStats; 6]) {
    for (index, learned) in LEARNED_MOVES.into_iter().enumerate() {
        if learned.unlocked(game.wanderer()) {
            stats[index].unlocked_at.get_or_insert(action);
        }
    }
}

fn record_move_use(
    game: &Game,
    action: LearnedMove,
    preview: blah_core::MovePreview,
    start: (i64, i64),
    targets_before: &[(MonsterId, i32)],
    deaths_before: u32,
    stats: &mut [MoveStats; 6],
) {
    let metric = &mut stats[move_index(action)];
    metric.uses += 1;
    metric.hit_actions += u64::from(game.last_attack().is_some_and(|attack| attack.hit));
    metric.targets += preview.targets as u64;
    metric.damage += if game.deaths() == deaths_before {
        targets_before
            .iter()
            .map(|(id, before)| {
                let after = game.monsters().iter().find(|m| m.id == *id).map_or(0, |m| m.hp);
                before.saturating_sub(after).max(0) as u64
            })
            .sum::<u64>()
    } else {
        // The death reset removes nearby targets, so their HP no longer shows what the move dealt.
        let remaining: i32 = targets_before.iter().map(|(_, hp)| *hp).sum();
        game.last_attack().filter(|attack| attack.hit).map_or(0, |attack| attack.total().min(remaining).max(0) as u64)
    };
    metric.focus_spent += preview.focus.max(0) as u64;
    if game.deaths() == deaths_before {
        metric.steps += manhattan(start, game.position());
    }
    if action == LearnedMove::Shove
        && preview.push.is_some()
        && game.last_attack().is_some_and(|attack| attack.hit)
        && targets_before.first().is_some_and(|(id, _)| game.monsters().iter().any(|m| m.id == *id))
    {
        metric.pushes += 1;
    }
}

#[cfg(test)]
fn run(seed: u64, actions: u64, style: Style) -> Report {
    run_policy(seed, actions, style, Policy::Baseline)
}

fn run_policy(seed: u64, actions: u64, style: Style, policy: Policy) -> Report {
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
        health: 0,
        focus: 0,
        ranks: ranks(&game),
        milestones: [[None; 4]; 3],
        moves: [MoveStats::default(); 6],
        recovery: Recovery::default(),
    };
    for action in 1..=actions {
        let before_log = game.log().total();
        let before_turn = game.turn();
        record_opportunities(&game, &mut report.moves);
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
        let fallback = match policy.base() {
            Policy::Baseline => pilot.choose_baseline(&game, style),
            Policy::Cautious => pilot.choose_cautious(&game, style),
            Policy::BaselineMoves | Policy::CautiousMoves => unreachable!("base policy is normalized"),
        };
        let chosen =
            if policy.uses_moves() { move_aware_action(&game, style, pilot.returning, fallback) } else { fallback };
        let deaths_before = game.deaths();
        report.recovery.before(&game, action, pilot.returning, chosen);
        let target_ids = action_targets(&game, chosen);
        for id in &target_ids {
            if game.monsters().iter().any(|m| m.id == *id && m.phenotype.temperament != Temperament::Skittish) {
                encounters.insert(*id);
            }
        }
        let learned_before = if let Action::Learned(learned, dir) = chosen {
            let preview = game.preview_move(learned, dir).expect("the move-aware policy previews before use");
            let targets_before: Vec<_> = target_ids
                .iter()
                .filter_map(|id| game.monsters().iter().find(|m| m.id == *id).map(|m| (*id, m.hp)))
                .collect();
            Some((learned, preview, game.position(), targets_before))
        } else {
            None
        };
        let result = chosen.apply(&mut game);
        report.recovery.after(&game, action, deaths_before);
        assert_ne!(result, Step::Blocked, "the pilot must spend an action");
        assert!(game.turn() > before_turn);
        record_unlocks(&game, action, &mut report.moves);
        report.kills +=
            game.log().last((game.log().total() - before_log) as usize).filter(|e| e.kind == LogKind::Kill).count()
                as u64;
        report.rests += u64::from(matches!(chosen, Action::Rest));
        report.techniques += u64::from(matches!(chosen, Action::Technique(_)));
        report.sneak_moves += u64::from(result == Step::Moved && game.sneaking());
        report.waits += u64::from(matches!(chosen, Action::Wait));
        if let Some((learned, preview, start, targets_before)) = learned_before {
            record_move_use(&game, learned, preview, start, &targets_before, deaths_before, &mut report.moves);
        }
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
    report.health = game.wanderer().hp;
    report.focus = game.wanderer().focus;
    report
}

fn positive_arg(value: Option<String>, default: u64) -> Result<u64, String> {
    value.map_or(Ok(default), |s| {
        s.parse::<u64>().ok().filter(|&n| n > 0).ok_or_else(|| format!("Expected a positive integer, got {s:?}"))
    })
}

fn median(values: &mut [u64]) -> Option<u64> {
    values.sort_unstable();
    values.get(values.len() / 2).copied()
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let seeds = positive_arg(args.next(), 8)?;
    let actions = positive_arg(args.next(), 600)?;
    let policies: &[Policy] = match args.next().as_deref() {
        None | Some("baseline") => &[Policy::Baseline],
        Some("cautious") => &[Policy::Cautious],
        Some("compare") => &[Policy::Baseline, Policy::Cautious],
        Some("move-compare") => &POLICIES,
        Some(_) => return Err("Policy must be baseline, cautious, compare, or move-compare".into()),
    };
    if args.next().is_some() {
        return Err("Usage: skill-pacing [seeds] [actions] [baseline|cautious|compare|move-compare]".into());
    }
    println!(
        "policy,style,seed,actions,turns,encounters,kills,deaths,rests,techniques,sneak_moves,waits,health,focus_left,weapon,focus_rank,stealth"
    );
    let mut reports = Vec::new();
    for &policy in policies {
        for style in Style::ALL {
            for seed in 0..seeds {
                let r = run_policy(seed, actions, style, policy);
                println!(
                    "{},{},{seed},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                    policy.name(),
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
                    r.health,
                    r.focus,
                    r.ranks[0].name(),
                    r.ranks[1].name(),
                    r.ranks[2].name()
                );
                reports.push((policy, style, seed, r));
            }
        }
    }
    println!("\npolicy,style,skill,rank,reached,runs,actions_min,actions_median,actions_max,encounters_median");
    for &policy in policies {
        for style in Style::ALL {
            for (skill, name) in ["weapon", "focus", "stealth"].into_iter().enumerate() {
                for (band, rank) in ["practiced", "skilled", "adept", "expert"].into_iter().enumerate() {
                    let milestones: Vec<_> = reports
                        .iter()
                        .filter(|(p, s, _, _)| *p == policy && s.name() == style.name())
                        .filter_map(|(_, _, _, r)| r.milestones[skill][band])
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
                    println!("{},{},{name},{rank},{},{seeds},{stats}", policy.name(), style.name(), times.len());
                }
            }
        }
    }
    println!(
        "\npolicy,style,move,unlocked_runs,unlock_action_median,available_opportunities,uses,hit_actions,targets,damage,focus_spent,steps,pushes"
    );
    for &policy in policies {
        for style in Style::ALL {
            let runs: Vec<_> = reports
                .iter()
                .filter(|(p, s, _, _)| *p == policy && s.name() == style.name())
                .map(|(_, _, _, report)| report)
                .collect();
            for (index, learned) in LEARNED_MOVES.into_iter().enumerate() {
                let mut unlocks: Vec<_> = runs.iter().filter_map(|r| r.moves[index].unlocked_at).collect();
                let unlock_median = median(&mut unlocks).map_or_else(|| "-".into(), |n| n.to_string());
                let sum = |f: fn(&MoveStats) -> u64| runs.iter().map(|r| f(&r.moves[index])).sum::<u64>();
                println!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{}",
                    policy.name(),
                    style.name(),
                    learned.name(),
                    unlocks.len(),
                    unlock_median,
                    sum(|m| m.opportunities),
                    sum(|m| m.uses),
                    sum(|m| m.hit_actions),
                    sum(|m| m.targets),
                    sum(|m| m.damage),
                    sum(|m| m.focus_spent),
                    sum(|m| m.steps),
                    sum(|m| m.pushes)
                );
            }
        }
    }
    println!(
        "\npolicy,style,seed,retreats,escapes,recovered,retreat_deaths,unfinished,wake_recoveries,wake_recovered,wake_deaths,wake_unfinished,home_blocked,arrival_median,recovery_median,rest_actions_median,wake_recovery_median,wake_rest_actions_median"
    );
    for (policy, style, seed, r) in reports {
        println!("{},{},{seed},{}", policy.name(), style.name(), r.recovery.csv());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(16))]
        #[test]
        fn policy_runs_spend_real_actions_and_conserve_recovery_episodes(
            seed in any::<u64>(), style in 0usize..6, actions in 1u64..64, policy in 0usize..4
        ) {
            let r = run_policy(seed, actions, Style::ALL[style], POLICIES[policy]);
            prop_assert_eq!(r.turns, actions + u64::from(r.deaths) * blah_core::DEATH_TURNS);
            r.recovery.csv();
        }
    }

    #[test]
    fn fresh_runs_are_deterministic_and_count_death_time_separately() {
        for policy in POLICIES {
            for style in Style::ALL {
                let a = run_policy(42, 120, style, policy);
                assert_eq!(a, run_policy(42, 120, style, policy));
                a.recovery.csv();
                assert_eq!(a.turns, a.actions + u64::from(a.deaths) * blah_core::DEATH_TURNS);
                for skill in a.milestones {
                    let reached: Vec<_> = skill.into_iter().flatten().collect();
                    assert!(reached.windows(2).all(|pair| pair[0].actions <= pair[1].actions));
                }
            }
        }
    }

    #[test]
    fn move_aware_policy_uses_real_unlocks_and_records_opportunities() {
        let baseline = run_policy(0, 600, Style::Fighter(Weapon::Dagger), Policy::Baseline);
        let move_aware = run_policy(0, 600, Style::Fighter(Weapon::Dagger), Policy::BaselineMoves);
        let index = move_index(LearnedMove::Withdraw);
        assert!(baseline.moves[index].unlocked_at.is_some());
        assert!(move_aware.moves[index].unlocked_at.is_some());
        assert_eq!(baseline.moves[index].unlocked_at, baseline.milestones[0][0].map(|m| m.actions));
        assert_eq!(move_aware.moves[index].unlocked_at, move_aware.milestones[0][0].map(|m| m.actions));
        assert_eq!(baseline.moves[index].uses, 0);
        assert!(move_aware.moves[index].opportunities >= move_aware.moves[index].uses);
        assert!(move_aware.moves[index].uses > 0);
        assert!(move_aware.moves[index].focus_spent > 0);
        assert!(move_aware.moves[index].hit_actions <= move_aware.moves[index].uses);

        let caster = run_policy(0, 1_200, Style::Caster, Policy::BaselineMoves);
        let pulse = caster.moves[move_index(LearnedMove::Pulse)];
        assert!(pulse.unlocked_at.is_some());
        assert!(pulse.uses > 0);
        assert!(pulse.hit_actions > 0);
        assert!(pulse.damage > 0);
        assert_eq!(pulse.focus_spent, 0);
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

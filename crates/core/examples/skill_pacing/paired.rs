//! Paired move effects: at each point where the move-aware pilot picks a learned move, split the game.
//! One copy uses the move and the other takes the action the pilot would otherwise have taken; both then
//! play on with the same move-aware pilot. Turn-keyed randomness gives both copies the same dice, so the
//! difference isolates that one decision, not the divergent later worlds a policy comparison mixes in.

use blah_core::{Game, LogKind, Step};

use super::{Action, LEARNED_MOVES, Pilot, Policy, Style, decide, move_index};

/// End state of one copy after the horizon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Outcome {
    pub(super) kills: i64,
    pub(super) deaths: i64,
    pub(super) health: i64,
    pub(super) focus: i64,
}

/// Running sums of move-minus-alternative differences within one run.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Effect {
    pub(super) samples: u64,
    sums: [i64; 4],
}

impl Effect {
    pub(super) fn add(&mut self, with: Outcome, without: Outcome) {
        let diffs = [
            with.kills - without.kills,
            with.deaths - without.deaths,
            with.health - without.health,
            with.focus - without.focus,
        ];
        self.samples += 1;
        for (i, d) in diffs.into_iter().enumerate() {
            self.sums[i] += d;
        }
    }
}

/// Mean difference per decision and its standard error, clustered by run: decisions within one run share
/// a trajectory, so treating them as independent would overstate precision.
pub(super) fn summary(runs: &[Effect]) -> [(f64, f64); 4] {
    let n: u64 = runs.iter().map(|r| r.samples).sum();
    let groups = runs.iter().filter(|r| r.samples > 0).count();
    let n = n.max(1) as f64;
    std::array::from_fn(|i| {
        let mean = runs.iter().map(|r| r.sums[i]).sum::<i64>() as f64 / n;
        if groups < 2 {
            return (mean, 0.0);
        }
        let spread: f64 = runs
            .iter()
            .map(|r| {
                let gap = r.sums[i] as f64 - r.samples as f64 * mean;
                gap * gap
            })
            .sum();
        let g = groups as f64;
        (mean, (spread * g / (g - 1.0)).sqrt() / n)
    })
}

fn kills_in(game: &Game, before: u64) -> i64 {
    game.log().last((game.log().total() - before) as usize).filter(|e| e.kind == LogKind::Kill).count() as i64
}

/// Apply `first`, then let the pilot choose for the rest of the horizon.
pub(super) fn play_out(mut game: Game, mut pilot: Pilot, style: Style, first: Action, horizon: u64) -> Outcome {
    let deaths = game.deaths();
    let mut kills = 0;
    for step in 0..horizon {
        let action = if step == 0 { first } else { decide(&mut pilot, &game, style, Policy::BaselineMoves).1 };
        let before = game.log().total();
        assert_ne!(action.apply(&mut game), Step::Blocked, "the pilot must spend an action");
        kills += kills_in(&game, before);
    }
    Outcome {
        kills,
        deaths: i64::from(game.deaths() - deaths),
        health: i64::from(game.wanderer().hp),
        focus: i64::from(game.wanderer().focus),
    }
}

/// Paired effects for one move-aware run, indexed like [`LEARNED_MOVES`].
pub(super) fn effects(seed: u64, actions: u64, style: Style, horizon: u64) -> [Effect; 6] {
    let mut game = Game::new(seed);
    game.prepare(style.maneuver()).expect("initial creatures are outside view");
    let mut pilot = Pilot::default();
    let mut effects = [Effect::default(); 6];
    for _ in 0..actions {
        let (fallback, chosen) = decide(&mut pilot, &game, style, Policy::BaselineMoves);
        if let Action::Learned(learned, _) = chosen {
            let with = play_out(game.clone(), pilot.clone(), style, chosen, horizon);
            let without = play_out(game.clone(), pilot.clone(), style, fallback, horizon);
            effects[move_index(learned)].add(with, without);
        }
        assert_ne!(chosen.apply(&mut game), Step::Blocked, "the pilot must spend an action");
    }
    effects
}

pub(super) fn run(seeds: u64, actions: u64, horizon: u64) {
    println!(
        "style,move,horizon,samples,runs,kills_diff,kills_se,deaths_diff,deaths_se,health_diff,health_se,focus_diff,focus_se"
    );
    for style in Style::ALL {
        let runs: Vec<_> = (0..seeds).map(|seed| effects(seed, actions, style, horizon)).collect();
        for (index, learned) in LEARNED_MOVES.into_iter().enumerate() {
            let per_run: Vec<_> = runs.iter().map(|r| r[index]).collect();
            let samples: u64 = per_run.iter().map(|r| r.samples).sum();
            if samples == 0 {
                continue;
            }
            let runs_used = per_run.iter().filter(|r| r.samples > 0).count();
            let cells: Vec<_> = summary(&per_run).iter().map(|(mean, se)| format!("{mean:.3},{se:.3}")).collect();
            println!("{},{},{horizon},{samples},{runs_used},{}", style.name(), learned.name(), cells.join(","));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blah_core::{LearnedMove, Weapon};

    #[test]
    fn paired_copies_share_dice_and_identical_choices_cancel() {
        let mut game = Game::new(3);
        game.prepare(Style::Fighter(Weapon::Dagger).maneuver()).unwrap();
        let pilot = Pilot::default();
        let (_, chosen) = decide(&mut pilot.clone(), &game, Style::Fighter(Weapon::Dagger), Policy::BaselineMoves);
        let a = play_out(game.clone(), pilot.clone(), Style::Fighter(Weapon::Dagger), chosen, 30);
        let b = play_out(game.clone(), pilot.clone(), Style::Fighter(Weapon::Dagger), chosen, 30);
        assert_eq!(a, b, "the same first action must give the same outcome");
        let mut effect = Effect::default();
        effect.add(a, b);
        assert!(summary(&[effect, effect]).iter().all(|&(mean, se)| mean == 0.0 && se == 0.0));
    }

    #[test]
    fn clustered_error_reflects_spread_between_runs() {
        let at = |kills| Outcome { kills, ..Outcome::default() };
        let run = |diffs: &[i64]| {
            let mut e = Effect::default();
            for &d in diffs {
                e.add(at(d), at(0));
            }
            e
        };
        let [(mean, se), ..] = summary(&[run(&[1, 1]), run(&[1, 1])]);
        assert_eq!((mean, se), (1.0, 0.0));
        let [(mean, se), ..] = summary(&[run(&[2, 2]), run(&[0, 0])]);
        assert_eq!(mean, 1.0);
        assert!(se > 0.9, "two runs that disagree cannot give a precise mean: {se}");
    }

    #[test]
    fn effects_sample_real_move_choices() {
        let effects = effects(0, 600, Style::Fighter(Weapon::Dagger), 10);
        assert!(effects[move_index(LearnedMove::Withdraw)].samples > 0);
        assert_eq!(effects[move_index(LearnedMove::Reach)].samples, 0, "a dagger cannot reach");
    }
}

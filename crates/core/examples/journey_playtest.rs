//! Complete fresh-character journeys through the public game API, not a model of human play.
//! Run: `just journey-playtest [seeds] [actions] [first-seed] [trails|blind]`.
//! The pilot uses seen terrain and public field signs, never injects state, and reloads at milestones.

#[path = "journey_playtest/pilot.rs"]
mod pilot;

fn main() {
    let mut args = std::env::args().skip(1);
    let seeds = args.next().map_or(24, |s| s.parse().expect("seeds must be an integer"));
    let actions = args.next().map_or(500, |s| s.parse().expect("actions must be an integer"));
    let start = args.next().map_or(0, |s| s.parse().expect("first seed must be an integer"));
    let trails = match args.next().as_deref() {
        None | Some("trails") => true,
        Some("blind") => false,
        Some(_) => panic!("guidance must be trails or blind"),
    };
    assert!(args.next().is_none(), "unexpected extra arguments");
    println!("seed outcome actions moves camps home_rests picks brews claims relics trades drinks deaths");
    for seed in start..start + seeds {
        let report = if trails { pilot::run(seed, actions) } else { pilot::run_with_trails(seed, actions, false) };
        println!(
            "{} {} {} {} {} {} {} {} {} {} {} {} {}",
            seed,
            report.outcome,
            report.actions,
            report.moves,
            report.camps,
            report.rests,
            report.picks,
            report.brews,
            report.claims,
            report.relics,
            report.trades,
            report.drinks,
            report.deaths,
        );
        for event in report.milestones {
            println!("  {event}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::pilot;

    #[test]
    fn whole_journeys_gather_brew_recover_and_use_rewards_on_the_next_outing() {
        for seed in [10, 16] {
            let report = pilot::run(seed, 300);
            assert_eq!(report.outcome, "complete", "seed {seed}: {report:?}");
            assert!(report.picks >= 2 && report.brews >= 1, "{report:?}");
            assert!(report.rests > 0 && report.trades == 1 && report.drinks > 0, "{report:?}");
            assert_eq!(report.claims, 2, "{report:?}");
            assert_eq!(report.deaths, 0);
        }
    }

    #[test]
    fn actual_followers_do_not_block_home_preparation_after_recovery() {
        // Before sheltered management, this exact visible-only route reached home at action 96 and
        // stopped with nearby creatures still preventing preparation.
        let report = pilot::run_with_trails(2, 300, false);
        assert_eq!(report.outcome, "complete", "{report:?}");
        assert!(report.rests > 0 && report.trades == 1);
        assert_eq!(report.deaths, 0);
    }

    #[test]
    fn a_discovered_shrine_relic_changes_later_actions_on_the_same_journey() {
        let report = pilot::run(18, 300);
        assert_eq!(report.outcome, "complete", "{report:?}");
        assert_eq!(report.relics, 1);
        assert!(report.relic_uses > 0, "owning a relic alone is not evidence of using its effect");
        assert!(report.milestones.iter().any(|event| event.contains("use Shrine")));
        assert_eq!(report.deaths, 0);
    }

    #[test]
    fn reloading_every_action_preserves_the_complete_route_and_rewards() {
        for seed in [10, 18] {
            assert_eq!(pilot::run_reloading(seed, 300), pilot::run(seed, 300), "seed {seed}");
        }
    }
}

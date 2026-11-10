//! Property tests over random legal compositions, seeds, and player action sequences.
//!
//! These are rule invariants. Running out of options in a fight is allowed: retreat and death are
//! legitimate ways out, so no property requires an affordable attack.

use blah_core::{
    Aim, Burst, Delivery, Dir, Element, Game, MAX_FOCUS, Maneuver, Monster, Shape, Step, Technique, Trigger, Wanderer,
    Weapon,
};
use proptest::prelude::*;

fn legal_maneuvers() -> Vec<Maneuver> {
    let mut out = Vec::new();
    for weapon in [Weapon::Dagger, Weapon::Spear, Weapon::Club, Weapon::Staff] {
        for delivery in [Delivery::Weapon, Delivery::Touch, Delivery::Ground] {
            for aim in [Aim::Direct, Aim::Opening] {
                let mut bursts = vec![None];
                for element in [Element::Force, Element::Fire] {
                    for trigger in [Trigger::Contact, Trigger::Penetration] {
                        for shape in [Shape::Point, Shape::Area(1), Shape::Area(2), Shape::Area(3)] {
                            for power in 1..=6 {
                                bursts.push(Some(Burst { power, trigger, element, shape }));
                            }
                        }
                    }
                }
                for burst in bursts {
                    let m =
                        Maneuver { name: "Generated".into(), weapon, technique: Technique { delivery, aim, burst } };
                    if m.validate().is_ok() {
                        out.push(m);
                    }
                }
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug)]
enum Action {
    Step(Dir),
    Wait,
    Sneak,
    Recover,
    Unleash,
    PrepareBasic,
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        4 => prop::sample::select(Dir::ALL.to_vec()).prop_map(Action::Step),
        1 => Just(Action::Wait),
        1 => Just(Action::Sneak),
        1 => Just(Action::Recover),
        1 => Just(Action::Unleash),
        1 => Just(Action::PrepareBasic),
    ]
}

fn maneuver() -> impl Strategy<Value = Maneuver> {
    prop::sample::select(legal_maneuvers())
}

type State = (u64, (i64, i64), Wanderer, Vec<Monster>, Maneuver, bool, u32);

/// Everything an action can change except the log.
fn state(g: &Game) -> State {
    (g.turn(), g.position(), *g.wanderer(), g.monsters().to_vec(), g.maneuver().clone(), g.sneaking(), g.deaths())
}

fn apply(g: &mut Game, a: Action) -> Step {
    match a {
        Action::Step(d) => g.step(d),
        Action::Wait => g.wait(),
        Action::Sneak => g.toggle_sneak(),
        Action::Recover => g.recover_focus(),
        Action::Unleash => g.unleash(),
        Action::PrepareBasic => match g.prepare(Maneuver::default()) {
            Ok(()) | Err(_) => Step::Blocked,
        },
    }
}

fn trial(seed: u64, m: Maneuver) -> Option<Game> {
    Game::composition_trial(seed, m).ok()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// Resources stay in range, blocked actions are atomic, previews bound results, and time moves forward.
    #[test]
    fn random_play_keeps_the_rules_honest(
        seed in any::<u64>(),
        m in maneuver(),
        actions in prop::collection::vec(action(), 1..48),
    ) {
        let Some(mut g) = trial(seed, m) else { return Ok(()) };
        for a in actions {
            let before = state(&g);
            let (deaths, focus) = (g.deaths(), g.wanderer().focus);
            let attack_preview = match a {
                Action::Step(d) if g.maneuver().technique.delivery != Delivery::Ground => g.preview_attack(d).ok(),
                _ => None,
            };
            let ground_bump = matches!(a, Action::Step(d) if g.maneuver().technique.delivery == Delivery::Ground
                && g.preview_attack(d).is_err() && {
                    let (x, y) = g.position();
                    let (dx, dy) = d.delta();
                    g.monster_at(x + dx, y + dy).is_some()
                });
            let area_preview = if matches!(a, Action::Unleash) || ground_bump { g.preview_area().ok() } else { None };
            if let Action::Step(d) = a {
                let can = g.can_step(d);
                let result = apply(&mut g, a);
                if !ground_bump {
                    prop_assert_eq!(can, result != Step::Blocked, "can_step disagrees with step");
                }
                check(&g, &before, result, deaths, focus, attack_preview, area_preview)?;
            } else {
                let result = apply(&mut g, a);
                check(&g, &before, result, deaths, focus, attack_preview, area_preview)?;
            }
        }
    }

    /// The same seed, loadout, and inputs always produce the same game.
    #[test]
    fn replay_is_deterministic(
        seed in any::<u64>(),
        m in maneuver(),
        actions in prop::collection::vec(action(), 1..32),
    ) {
        let Some(mut a) = trial(seed, m) else { return Ok(()) };
        let mut b = a.clone();
        for x in &actions {
            apply(&mut a, *x);
            apply(&mut b, *x);
        }
        prop_assert_eq!(a, b);
    }

    /// A maneuver's name never changes what happens.
    #[test]
    fn names_are_cosmetic_in_real_play(
        seed in any::<u64>(),
        m in maneuver(),
        name in "[A-Za-z][A-Za-z ]{0,40}",
        actions in prop::collection::vec(action(), 1..32),
    ) {
        let mut renamed = m.clone();
        renamed.name = name;
        let (Some(mut a), Some(mut b)) = (trial(seed, m), trial(seed, renamed)) else { return Ok(()) };
        for x in &actions {
            prop_assert_eq!(apply(&mut a, *x), apply(&mut b, *x));
        }
        let strip = |s: State| (s.0, s.1, s.2, s.3, s.4.technique, s.5, s.6);
        prop_assert_eq!(strip(state(&a)), strip(state(&b)));
    }
}

fn check(
    g: &Game,
    before: &State,
    result: Step,
    deaths: u32,
    focus: i32,
    attack: Option<blah_core::AttackPreview>,
    area: Option<blah_core::AreaPreview>,
) -> Result<(), TestCaseError> {
    let f = g.wanderer().focus;
    prop_assert!((0..=MAX_FOCUS).contains(&f), "Focus {f} out of range");
    if result == Step::Blocked {
        prop_assert_eq!(&state(g).0, &before.0, "blocked action passed time");
        let (t, p, w, ms, _, s, d) = before;
        let now = state(g);
        prop_assert_eq!((&now.0, &now.1, &now.2, &now.3, &now.5, &now.6), (t, p, w, ms, s, d));
        return Ok(());
    }
    prop_assert!(g.turn() > before.0, "a non-blocked action must pass time");
    let died = g.deaths() != deaths;
    if let (Some(p), Step::Attacked, false) = (attack, result, died)
        && let Some(r) = g.last_attack()
        && r.hit
    {
        prop_assert!((p.min_hit..=p.max_hit).contains(&r.total()), "{:?} outside {:?}", r, p);
        prop_assert_eq!(focus - f, p.focus);
    }
    if let (Some(p), Step::Attacked, false) = (area, result, died) {
        prop_assert_eq!(focus - f, p.focus, "area discharge cost differs from preview");
    }
    Ok(())
}

//! Property tests over random legal compositions, seeds, and player action sequences.
//!
//! These are rule invariants. Techniques can run out of Focus, but ordinary weapon strikes remain free.

use blah_core::{
    Aim, Burst, Delivery, Dir, Element, Game, LearnedMove, Maneuver, Monster, MovePreview, Shape, Step, Technique,
    Trigger, Wanderer, Weapon,
};
use proptest::prelude::*;

const LEARNED_MOVES: [LearnedMove; 6] = [
    LearnedMove::Withdraw,
    LearnedMove::Reach,
    LearnedMove::Shove,
    LearnedMove::Sweep,
    LearnedMove::SlipAway,
    LearnedMove::Pulse,
];

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
    Technique(Dir),
    Wait,
    Sneak,
    Recover,
    Unleash,
    PrepareBasic,
    Learned(LearnedMove, Dir),
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        4 => prop::sample::select(Dir::ALL.to_vec()).prop_map(Action::Step),
        3 => prop::sample::select(Dir::ALL.to_vec()).prop_map(Action::Technique),
        1 => Just(Action::Wait),
        1 => Just(Action::Sneak),
        1 => Just(Action::Recover),
        1 => Just(Action::Unleash),
        1 => Just(Action::PrepareBasic),
        2 => (prop::sample::select(LEARNED_MOVES.to_vec()), prop::sample::select(Dir::ALL.to_vec()))
            .prop_map(|(action, direction)| Action::Learned(action, direction)),
    ]
}

fn maneuver() -> impl Strategy<Value = Maneuver> {
    prop::sample::select(legal_maneuvers())
}

type State = (u64, (i64, i64), Wanderer, Vec<Monster>, Maneuver, bool, u32);

#[derive(Default)]
struct ActionPreviews {
    attack: Option<blah_core::AttackPreview>,
    area: Option<blah_core::AreaPreview>,
    learned: Option<MovePreview>,
}

/// Everything an action can change except the log.
fn state(g: &Game) -> State {
    (g.turn(), g.position(), *g.wanderer(), g.monsters().to_vec(), g.maneuver().clone(), g.sneaking(), g.deaths())
}

fn apply(g: &mut Game, a: Action) -> Step {
    match a {
        Action::Step(d) => g.step(d),
        Action::Technique(d) => g.use_technique(d),
        Action::Wait => g.wait(),
        Action::Sneak => g.toggle_sneak(),
        Action::Recover => g.recover_focus(),
        Action::Unleash => g.unleash(),
        Action::PrepareBasic => match g.prepare(Maneuver::default()) {
            Ok(()) | Err(_) => Step::Blocked,
        },
        Action::Learned(m, d) => g.use_move(m, d),
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
                Action::Step(d) => g.preview_strike(d).ok(),
                Action::Technique(d) if g.maneuver().technique.delivery != Delivery::Ground => g.preview_attack(d).ok(),
                _ => None,
            };
            let learned_preview = if let Action::Learned(m, d) = a { g.preview_move(m, d).ok() } else { None };
            let ground_technique = matches!(a, Action::Technique(d) if g.maneuver().technique.delivery == Delivery::Ground
                && {
                    let (x, y) = g.position();
                    let (dx, dy) = d.delta();
                    g.monster_at(x + dx, y + dy).is_some()
                });
            let area_preview = if matches!(a, Action::Unleash) || ground_technique { g.preview_area().ok() } else { None };
            let previews = ActionPreviews { attack: attack_preview, area: area_preview, learned: learned_preview };
            if let Action::Step(d) = a {
                let can = g.can_step(d);
                let result = apply(&mut g, a);
                prop_assert_eq!(can, result != Step::Blocked, "can_step disagrees with step");
                check(&g, &before, result, deaths, focus, previews)?;
            } else {
                let result = apply(&mut g, a);
                check(&g, &before, result, deaths, focus, previews)?;
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
    previews: ActionPreviews,
) -> Result<(), TestCaseError> {
    let f = g.wanderer().focus;
    prop_assert!((0..=g.wanderer().max_focus()).contains(&f), "Focus {f} out of range");
    if result == Step::Blocked {
        prop_assert_eq!(&state(g).0, &before.0, "blocked action passed time");
        let (t, p, w, ms, _, s, d) = before;
        let now = state(g);
        prop_assert_eq!((&now.0, &now.1, &now.2, &now.3, &now.5, &now.6), (t, p, w, ms, s, d));
        return Ok(());
    }
    prop_assert!(g.turn() > before.0, "a non-blocked action must pass time");
    let died = g.deaths() != deaths;
    if let (Some(p), Step::Attacked, false) = (previews.attack, result, died)
        && let Some(r) = g.last_attack()
    {
        if r.hit {
            prop_assert!((p.min_hit..=p.max_hit).contains(&r.total()), "{:?} outside {:?}", r, p);
        } else {
            prop_assert_eq!(r.total(), 0);
        }
        prop_assert_eq!(focus - f, p.focus);
    }
    if let (Some(p), Step::Attacked, false) = (previews.area, result, died) {
        prop_assert_eq!(focus - f, p.focus, "area discharge cost differs from preview");
    }
    if let (Some(p), Step::Attacked, false) = (previews.learned, result, died) {
        prop_assert_eq!(focus - f, p.focus, "learned move cost differs from preview");
        if let Some(hit) = g.last_attack() {
            if hit.hit {
                prop_assert!(hit.total() <= p.max_hit, "{:?} exceeds {:?}", hit, p);
                if p.targets == 1 {
                    prop_assert!(hit.total() >= p.min_hit, "{:?} below {:?}", hit, p);
                }
            } else {
                prop_assert_eq!(hit.total(), 0);
            }
        }
    }
    Ok(())
}

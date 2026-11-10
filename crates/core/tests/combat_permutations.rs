//! Exhaustive supported structures, sampled real encounters, and explicit (non-optimal) player styles.
//! No game rules or private state are changed to admit a loadout or force an outcome.

use std::collections::BTreeMap;

use blah_core::{
    Aim, AttackContext, Burst, Delivery, Dir, Element, Game, LogKind, Maneuver, MonsterId, Rng, Shape, Step, Technique,
    Trigger, Weapon,
};

fn permutations() -> Vec<Maneuver> {
    let mut maneuvers = Vec::new();
    for weapon in [Weapon::Dagger, Weapon::Spear, Weapon::Club, Weapon::Staff] {
        for delivery in [Delivery::Weapon, Delivery::Touch, Delivery::Ground] {
            for aim in [Aim::Direct, Aim::Opening] {
                let mut payloads = vec![None];
                for element in [Element::Force, Element::Fire] {
                    for trigger in [Trigger::Contact, Trigger::Penetration] {
                        for shape in [Shape::Point, Shape::Area(1), Shape::Area(2), Shape::Area(3)] {
                            for power in 1..=6 {
                                payloads.push(Some(Burst { power, trigger, element, shape }));
                            }
                        }
                    }
                }
                for burst in payloads {
                    let maneuver = Maneuver {
                        name: "Survey composition".into(),
                        weapon,
                        technique: Technique { delivery, aim, burst },
                    };
                    if maneuver.validate().is_ok() {
                        maneuvers.push(maneuver);
                    }
                }
            }
        }
    }
    maneuvers
}

#[derive(Clone, Copy, Debug)]
enum Style {
    /// Start with the fixture's untracked, adjacent sneak opportunity.
    Ambush,
    /// Deliberately reveal before attacking. Costs a turn and allows retaliation.
    Exposed,
    /// Walk away and return while sneaking before engaging, paying actual movement costs.
    Stalk,
    /// After each attack, try one ordinary step away, then close and attack again.
    Skirmish,
}

const STYLES: [Style; 4] = [Style::Ambush, Style::Exposed, Style::Stalk, Style::Skirmish];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Kill,
    Exhausted,
    Death,
    LostTarget,
    Budget,
}

#[derive(Default)]
struct Summary {
    counts: [usize; 5],
    max_actions: usize,
}

impl Summary {
    fn record(&mut self, outcome: Outcome, actions: usize) {
        let index = match outcome {
            Outcome::Kill => 0,
            Outcome::Exhausted => 1,
            Outcome::Death => 2,
            Outcome::LostTarget => 3,
            Outcome::Budget => 4,
        };
        self.counts[index] += 1;
        self.max_actions = self.max_actions.max(actions);
    }
}

fn verdict(game: &Game, target: MonsterId, target_name: &str) -> Option<Outcome> {
    if game.deaths() > 0 {
        Some(Outcome::Death)
    } else if !game.monsters().iter().any(|m| m.id == target) {
        // Disappearance alone is not a victory: despawning and death both clear creatures.
        Some(if game.log().entries().any(|e| e.kind == LogKind::Kill && e.text == format!("The {target_name} dies.")) {
            Outcome::Kill
        } else {
            Outcome::LostTarget
        })
    } else if game.wanderer().focus < game.maneuver().technique.focus_cost() {
        Some(Outcome::Exhausted)
    } else {
        None
    }
}

fn play(mut game: Game, style: Style) -> (Outcome, usize) {
    let target = game.monsters()[0].id;
    let target_name = game.monsters()[0].name.clone();
    let mut actions = 0;
    let setup: &[Dir] = if matches!(style, Style::Stalk) { &[Dir::West, Dir::East] } else { &[] };
    if matches!(style, Style::Exposed) {
        game.toggle_sneak();
        actions += 1;
    }
    for direction in setup {
        game.step(*direction);
        actions += 1;
    }
    let mut retreat = false;
    while actions < 64 {
        if let Some(outcome) = verdict(&game, target, &target_name) {
            return (outcome, actions);
        }
        // The policy only pursues a visible target. Hidden state is used solely to classify outcomes.
        let Some(monster) = game.monsters_in_view().find(|m| m.id == target) else {
            return (Outcome::LostTarget, actions);
        };
        let position = monster.position;
        let (x, y) = game.position();
        let distance = |(mx, my): (i64, i64)| (mx - position.0).abs() + (my - position.1).abs();
        if retreat {
            retreat = false;
            let away = Dir::ALL
                .into_iter()
                .filter(|d| {
                    let (dx, dy) = d.delta();
                    game.can_step(*d) && game.monster_at(x + dx, y + dy).is_none()
                })
                .max_by_key(|d| {
                    let (dx, dy) = d.delta();
                    distance((x + dx, y + dy))
                });
            if let Some(direction) = away {
                game.step(direction);
                actions += 1;
                continue;
            }
        }
        let result = if let Some(Burst { shape: Shape::Area(radius), .. }) = game.maneuver().technique.burst
            && (position.0 - x).abs().max((position.1 - y).abs()) <= i64::from(radius)
        {
            game.unleash()
        } else {
            let approach = Dir::ALL.into_iter().filter(|d| game.can_step(*d)).min_by_key(|d| {
                let (dx, dy) = d.delta();
                distance((x + dx, y + dy))
            });
            match approach {
                Some(direction) => {
                    let (dx, dy) = direction.delta();
                    if game.monster_at(x + dx, y + dy).is_some() {
                        game.use_technique(direction)
                    } else {
                        game.step(direction)
                    }
                }
                None => game.wait(),
            }
        };
        assert_ne!(result, Step::Blocked, "policy selected an action it cannot execute");
        actions += 1;
        retreat = matches!(style, Style::Skirmish) && result == Step::Attacked;
    }
    (verdict(&game, target, &target_name).unwrap_or(Outcome::Budget), actions)
}

#[test]
fn all_supported_permutations_through_real_combat_under_four_styles() {
    let maneuvers = permutations();
    // Independent grammar count: 8 bare weapon forms; piercing payloads 2*2*2*2*6=96,
    // blunt payloads 2*2*2*1*6=48; touch 4*2*2*6=96; ground 3*2*3*6=108.
    assert_eq!(maneuvers.len(), 356);
    for (delivery, expected) in [(Delivery::Weapon, 152), (Delivery::Touch, 96), (Delivery::Ground, 108)] {
        assert_eq!(maneuvers.iter().filter(|m| m.technique.delivery == delivery).count(), expected);
    }
    let mut summaries: BTreeMap<String, Summary> = BTreeMap::new();
    let mut robust = Vec::new();
    let mut total = 0;
    // A reproducible seed sample, not an exhaustive world or encounter survey.
    for maneuver in &maneuvers {
        let mut wins = 0;
        for seed in [0, 7, 42] {
            let game = Game::composition_trial(seed, maneuver.clone()).unwrap();
            for style in STYLES {
                let (outcome, actions) = play(game.clone(), style);
                wins += usize::from(outcome == Outcome::Kill);
                total += 1;
                let technique = maneuver.technique;
                let element = technique.burst.map(|b| format!("{:?}", b.element)).unwrap_or("None".into());
                let trigger = technique.burst.map(|b| format!("{:?}", b.trigger)).unwrap_or("None".into());
                let power = technique.burst.map_or(0, |b| b.power);
                let shape = technique.burst.map(|b| format!("{:?}", b.shape)).unwrap_or("None".into());
                for group in [
                    format!("weapon/{:?}/{style:?}", maneuver.weapon),
                    format!("delivery/{:?}/{style:?}", technique.delivery),
                    format!("element/{element}/{style:?}"),
                    format!("trigger/{trigger}/{style:?}"),
                    format!("aim/{:?}/{style:?}", technique.aim),
                    format!("power/{power}/{style:?}"),
                    format!("shape/{shape}/{style:?}"),
                ] {
                    summaries.entry(group).or_default().record(outcome, actions);
                }
            }
        }
        if wins == 12 {
            robust.push(maneuver);
        }
    }
    assert_eq!(total, 4_272);
    println!("\n356 legal structures x 3 seeds x 4 styles = {total} real-Game trajectories");
    println!("Single defense-3, HP-40 sentinel; sampled natural heat tolerance; 64-action cap.");
    println!("Groups overlap. Raw totals are NOT controlled comparisons or general balance rankings.");
    println!("group | kill | exhausted | death | lost target | budget | max actions");
    for (group, summary) in summaries {
        let [kills, exhausted, deaths, lost, budget] = summary.counts;
        println!("{group} | {kills} | {exhausted} | {deaths} | {lost} | {budget} | {}", summary.max_actions);
    }
    println!("\nStructures winning all 12 sampled trajectories: {}", robust.len());
    for maneuver in robust {
        println!("{:?} / {:?}; cost={}", maneuver.weapon, maneuver.technique, maneuver.technique.focus_cost());
    }
}

#[test]
fn styled_policy_is_deterministic_and_does_not_change_the_supplied_game() {
    for maneuver in [Maneuver::default(), Maneuver::backstab_burst(), Maneuver::staff_inferno()] {
        let game = Game::composition_trial(42, maneuver).unwrap();
        let before = game.clone();
        for style in STYLES {
            assert_eq!(play(game.clone(), style), play(game.clone(), style));
        }
        assert_eq!(game, before);
    }
}

/// Controlled comparisons expose lost identity that mixed-group victory totals cannot establish.
#[test]
fn current_weapon_coefficients_leave_dagger_and_staff_without_an_exclusive_niche() {
    let mut comparisons = 0;
    for original in permutations().into_iter().filter(|m| matches!(m.weapon, Weapon::Dagger | Weapon::Staff)) {
        let mut replacement = original.clone();
        replacement.weapon = if original.weapon == Weapon::Dagger { Weapon::Spear } else { Weapon::Club };
        replacement.validate().unwrap();
        assert_eq!(original.technique.focus_cost(), replacement.technique.focus_cost());
        if original.technique.delivery == Delivery::Ground {
            assert_eq!(original.area_payload(), replacement.area_payload());
            assert_eq!(original.backlash(), replacement.backlash());
            continue;
        }
        // Same hit/miss stream, physical attack, armor, resistance, and actual opening in every pair.
        for defense in 0..=12 {
            for fire_resistance in [0, 40, 75] {
                for unguarded in [false, true] {
                    let context = AttackContext { attack: 4, defense, fire_resistance, unguarded };
                    let a = original.preview(context).unwrap();
                    let b = replacement.preview(context).unwrap();
                    assert!(b.min_hit >= a.min_hit && b.max_hit >= a.max_hit);
                    for seed in 0..16 {
                        let a = original.resolve(context, &mut Rng::new(seed)).unwrap();
                        let b = replacement.resolve(context, &mut Rng::new(seed)).unwrap();
                        assert_eq!(a.hit, b.hit);
                        assert!(b.total() >= a.total(), "{original:?} versus {replacement:?}, {context:?}");
                        comparisons += 1;
                    }
                }
            }
        }
    }
    println!(
        "{comparisons} paired point attacks: spear never deals less than dagger; club never less than staff, at the same Focus cost. Ground payloads are identical."
    );
}

#[test]
fn opening_aim_on_touch_or_nonprecise_weapons_adds_cost_but_not_damage() {
    let mut compared = 0;
    for original in permutations().into_iter().filter(|m| {
        m.technique.aim == Aim::Opening
            && (m.technique.delivery == Delivery::Touch || matches!(m.weapon, Weapon::Club | Weapon::Staff))
    }) {
        let mut direct = original.clone();
        direct.technique.aim = Aim::Direct;
        assert_eq!(original.technique.focus_cost(), direct.technique.focus_cost() + 1);
        for defense in [0, 3, 8] {
            for fire_resistance in [0, 40, 75] {
                for unguarded in [false, true] {
                    let context = AttackContext { attack: 4, defense, fire_resistance, unguarded };
                    for seed in 0..32 {
                        assert_eq!(
                            original.resolve(context, &mut Rng::new(seed)).unwrap(),
                            direct.resolve(context, &mut Rng::new(seed)).unwrap(),
                        );
                        compared += 1;
                    }
                }
            }
        }
    }
    println!("{compared} paired attacks: opening aim is a strictly extra cost for touch, club, and staff.");
}

#[test]
fn armor_heat_and_trigger_requirements_can_reverse_the_better_magick() {
    let force = Maneuver {
        name: "Controlled contact".into(),
        weapon: Weapon::Staff,
        technique: Technique {
            delivery: Delivery::Weapon,
            aim: Aim::Direct,
            burst: Some(Burst { power: 3, trigger: Trigger::Contact, element: Element::Force, shape: Shape::Point }),
        },
    };
    let mut fire = force.clone();
    fire.technique.burst.as_mut().unwrap().element = Element::Fire;
    assert_eq!(force.technique.focus_cost(), fire.technique.focus_cost());
    for seed in 0..64 {
        for (defense, fire_resistance, fire_is_better) in [(8, 0, true), (0, 75, false)] {
            let context = AttackContext { attack: 4, defense, fire_resistance, unguarded: false };
            let a = force.resolve(context, &mut Rng::new(seed)).unwrap();
            let b = fire.resolve(context, &mut Rng::new(seed)).unwrap();
            assert_eq!(a.hit, b.hit);
            if a.hit {
                // Independent payload arithmetic: Force 6-8/2=2 vs Fire 6 at high armor;
                // Force 6 vs Fire floor(6*25/100)=1 at high heat tolerance.
                assert_eq!((a.burst, b.burst), if fire_is_better { (2, 6) } else { (6, 1) });
                assert_eq!(b.total() > a.total(), fire_is_better);
            }
        }
    }

    let mut penetration = Maneuver::backstab_burst();
    penetration.technique.burst.as_mut().unwrap().power = 6;
    let mut contact = penetration.clone();
    contact.technique.burst.as_mut().unwrap().trigger = Trigger::Contact;
    let armor = AttackContext { attack: 4, defense: 12, fire_resistance: 0, unguarded: true };
    for seed in 0..64 {
        let a = penetration.resolve(armor, &mut Rng::new(seed)).unwrap();
        let b = contact.resolve(armor, &mut Rng::new(seed)).unwrap();
        if a.hit {
            assert_eq!((a.physical, a.burst, a.penetrated), (1, 0, false));
            assert_eq!((b.physical, b.burst), (1, 6));
        }
    }
    assert_eq!(penetration.technique.focus_cost(), contact.technique.focus_cost() + 1);

    let mut cheap_touch = fire;
    cheap_touch.technique.delivery = Delivery::Touch;
    cheap_touch.technique.burst.as_mut().unwrap().power = 1;
    let resistant = AttackContext { attack: 4, defense: 0, fire_resistance: 75, unguarded: true };
    assert_eq!(cheap_touch.technique.focus_cost(), 3);
    let preview = cheap_touch.preview(resistant).unwrap();
    assert_eq!((preview.min_hit, preview.max_hit), (0, 0), "rounding can erase an entire cheap touch spell");
}

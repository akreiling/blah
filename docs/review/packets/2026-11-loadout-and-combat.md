# Jev review packet: loadout properties and combat proof

[Review process](../README.md) · [Technique review contract](../technique-review.md)

**Revision:** `0a44374` on `main`. **Status:** sent as typed questions on 2026-11-10. Results:
[`../jev/loadout-review-2026-11-10.md`](../jev/loadout-review-2026-11-10.md).

Jev 1.14 is a decision model. It returns probabilities for yes/no, choice, and score questions, not
written critique. `scripts/jev_loadout_review.py` turns the questions below into narrow typed questions,
sends the rules as `state`, and combines the answers in code. A repeat run of all 246 answers changed by
0.02 on average (largest 0.14); 7 answers near the 0.6 cutoff flipped. The prose below remains the
human-readable version of the questions, and is suitable for a text-generating reviewer.
## Instructions for the reviewer

You are reviewing a proposed design and a small executable prototype for a turn-based, ecology-driven
roguelike RPG. Critique only what is supplied. Do not invent lore, new classes, or replacement designs.
Do not treat the example builds as a whitelist.

For each finding, return:

1. **Target:** the exact rule, claim, or assumption you are challenging (quote it).
2. **Kind:** contradiction, exploit, dominant strategy, unclear rule, or missing evidence.
3. **Counterexample:** a concrete loadout, situation, or action sequence.
4. **Discriminating check:** a specific test or playtest that would show whether you are right.
5. **Confidence:** low, medium, or high, with one sentence saying why.

Prefer a few strong findings over many weak ones. Say "no finding" for a question if you have none.
Do not give balance scores or fun ratings.

## Design goals the critique should respect

- **Committed direction:** classless growth by doing; players should discover builds the designers never
  wrote down; physical and magical play are equally deep and combine; costly specialized power is fine,
  but cost-free universal dominance is not.
- **Accepted by the designer:** losing a fight is fine. Retreat and dying are legitimate ways out.
  Death returns you home with full Health and Focus after a 1,000-turn time penalty.
- **Prototype choices (open to challenge):** every number below, the property list, and the item list.

## Rules under review

### Proposed loadout model (not implemented)

From [`docs/systems/loadout-properties.md`](../../systems/loadout-properties.md):

> A loadout is not a stat bundle. Each item has a few physical properties, and every action and effect
> reads those properties. Each property makes some things harder and others better.

| Property | Makes harder | Makes better |
| --- | --- | --- |
| Mass | Sneaking, dodging, swimming (heavy sinks), travel in heat | Charges, bashes, resisting knockback, heavy-weapon cleave |
| Noise | Sneaking, ambushes, avoiding notice | Drawing enemies onto yourself, scaring off weak creatures |
| Material | Metal: fire heats it and burns you, lightning arcs through it | Metal holds wards; wood carries channeled magic |
| Rigidity (later) | Martial arts, acrobatics | Blades glance off |
| Size (later) | Small weapons lack reach and force | Concealable; suits backstabs |

Actions read properties:

- **Basic strike:** always available, never costs Focus.
- **Sneak:** only with a quiet, not-heavy loadout.
- **Backstab:** small weapon, while sneaking, from behind an unaware creature. Creatures face the way they
  last moved, or toward you once they notice you.
- **Martial arts:** staff or empty hands, flexible armor.
- **Cleave:** a heavy weapon hits every adjacent enemy.
- **Charge:** heavier hits harder.
- **Casting:** wood and cloth carry magic freely; metal resists channeling unless warded.
- **Fire:** burns cloth; heats metal and hurts its wearer over a few turns.
- A loadout's property value is mostly its loudest, heaviest, or stiffest piece.

The world reads the same properties: desert heat punishes heavy metal, deep water drowns plate,
fire-breathers punish metal, clawed creatures reward rigid armor, keen-eared creatures hear normal noise.

### Executable prototype (implemented)

From [`crates/core/src/combat.rs`](../../../crates/core/src/combat.rs) and
[`composition-poc.md`](../../systems/composition-poc.md):

- Player: 30 HP, attack 4, defense 1. Focus 24, restored only at home with no creatures in view, or by death.
- Weapons (force, precision): dagger (0, 3), spear (2, 1), club (3, 0), staff (1, 0). Dagger and spear
  pierce. All but the dagger can strike the ground for area spells.
- Physical hit: attack + weapon force − defense + roll (−1..1), at least 1. 10% miss.
- Opening aim (1 Focus) against an unaware target subtracts the weapon's precision from defense.
- Payloads: Force or Fire, power 1–6, released on contact or on penetration. Point cost 2 + power
  (+1 for penetration). Area cost 2 + 2 × power + radius², hits everyone in the square, caster included.
- Every bump uses the prepared maneuver. There is currently **no** free fallback strike, and preparing a
  different maneuver is blocked while creatures are in view.

## Evidence

Reproduce with:

```sh
mise exec -- cargo test -p blah-core --test combat_scenarios -- --nocapture --test-threads=1
mise exec -- cargo test -p blah-core --test combat_permutations -- --nocapture --test-threads=1
mise exec -- cargo test -p blah-core --test combat_properties
```

Results against one 40-HP, defense-3 trial sentinel (full detail in
[`combat-exploration.md`](../../systems/combat-exploration.md)):

- 356 legal structures × 3 seeds × 4 play styles = 4,272 real-game runs. Only bare spear and bare club
  won every run. No touch or ground spell killed the sentinel in any of 1,632 runs.
- In 154,752 paired attacks with identical randomness, the spear never dealt less than the dagger and
  the club never less than the staff, at the same Focus cost.
- Opening aim on touch, club, or staff adds 1 Focus and nothing else (42,624 paired attacks).
- The featured dagger backstab maneuver (7 Focus) exhausts Focus after 3 attacks in all 18 seeds tested.
- Waiting while exhausted beside the sentinel leads to death and a full reset 1,025 turns later.
- Proptest invariants hold: Focus stays in range, blocked actions change nothing, previews bound
  results, replay is deterministic, names are cosmetic.

**Missing evidence:** none of the loadout model is implemented. There is no facing, armor, noise,
material, cleave, martial arts, or environment interaction in code. There has been no human playtest of
the current prototype beyond one session the designer described as "zero fun". The prototype has a
single enemy type in its trials.

## Questions

### A. Every property cuts both ways

1. Find a property or item whose downside never matters in practice, or whose upside always applies.
2. Find a loadout that is never worse than some alternative in any listed situation (strict dominance).
3. "Mostly its loudest, heaviest, or stiffest piece": what mixed loadouts does this rule make strange
   or exploitable (one plate glove on a robe, swapping gear mid-fight, carrying plate but not wearing it)?

### B. Backstab and facing

4. With facing defined as "last move direction, or toward you once noticed", how can a player
   reliably get behind creatures (luring, waiting for a turn, corners, other creatures, decoys)?
   Is that a satisfying skill or a trivial routine?
5. Conversely, can backstabs become so rare that the dagger has no role? What would show that?
6. What must the screen show for a failed sneak to feel fair rather than random?

### C. Material, magic, and dominance

7. Is plate + wards + force (or a heavy charge) a combination with no real counter among the listed
   rules and environments?
8. Does "metal resists channeling unless warded" create a loop where wards remove the only cost of metal?
9. Does the staff's dual use (magic focus and martial weapon) make it strictly better than single-purpose
   options, or does each use still cost something?

### D. Death as recovery

10. Since death fully restores Health and Focus, how can a player benefit from dying on purpose (spend
    everything, die, return)? Is the 1,000-turn penalty a real cost if nothing in the world advances
    with time yet?
11. What world rule would make the time penalty meaningful without punishing ordinary losses?

### E. Current prototype dominance

12. The free fallback strike, facing, and weapon verbs are proposed fixes for: spear dominating dagger,
    club dominating staff, magic never finishing a fight. Do they fix the causes, or only rename them?
13. With a free basic strike always available, does Focus spending become optional to the point that
    magic is a side feature? What would keep payloads attractive?

### F. Anything else

14. Name the single most important missing rule or test in this packet, and why.

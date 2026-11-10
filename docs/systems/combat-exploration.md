# Combat exploration: choices, not just damage checks

[Composition proof](composition-poc.md) · [Validation goals](validation.md)

This investigation records the technique-only combat baseline. It does not approve its balance.
Passing tests can reproduce a bad design just as accurately as a good one.

**Implemented since that baseline:** bump attacks are free weapon strikes, while Shift + direction
explicitly uses the prepared technique. The dagger exhaustion and staff survivor scenarios now check
this fix. The permutation and loadout surveys deliberately retain technique-only policies to isolate
their costs; their exhaustion outcomes do not mean the player lacks an affordable attack.
See [current controls and regression coverage](composition-poc.md).

## Reproduce

```sh
mise exec -- cargo test -p blah-core --test combat_scenarios -- --nocapture --test-threads=1
mise exec -- cargo test -p blah-core --test combat_permutations -- --nocapture --test-threads=1
mise exec -- cargo test -p blah-core --test combat_properties
```

The first suite follows longer journeys and compares selected loadouts across 18 seeds:
0–15, 42, and the maximum `u64`. The second enumerates every legal structure in the current grammar,
then plays each through the public `Game` actions. Neither suite injects resources, enemy positions, or
random attack results into the game.

## Permutations and player styles

There are **356 legal structures**, not 356 distinct play styles:

| Delivery | Structures | Independent count |
| --- | ---: | --- |
| Weapon, no payload | 8 | 4 weapons × 2 aims |
| Piercing weapon payload | 96 | 2 weapons × 2 aims × 2 elements × 2 triggers × 6 powers |
| Blunt weapon payload | 48 | 2 weapons × 2 aims × 2 elements × contact × 6 powers |
| Touch payload | 96 | 4 instruments × 2 aims × 2 elements × contact × 6 powers |
| Ground payload | 108 | 3 instruments × direct aim × 2 elements × contact × 3 radii × 6 powers |

Each structure runs at seeds 0, 7, and 42 under four explicit policies, for **4,272 trajectories**:

- **Ambush:** use the adjacent, untracked sneaking start.
- **Exposed:** leave sneak before engaging, allowing a turn of retaliation.
- **Stalk:** step west and east while sneaking, paying movement costs, then engage.
- **Skirmish:** try a step away after each attack, then close and attack again.

All policies use real turns, movement, misses, enemy actions, spawning, costs, and self-damage. They have
a 64-action cap. They pursue only a visible original target. A target lost to view or despawning is not
counted as a kill; dying and clearing the monsters is not a victory. Exhaustion means the policy cannot
afford another use of its committed technique, **not** that all possible actions have been exhausted.

The printed breakdown includes weapon, delivery, element, trigger, aim, power, shape, and style. Groups
overlap and contain different numbers of structures. Raw aggregate kill totals are not fair weapon
rankings. Controlled paired comparisons belong alongside these totals.

These runs share a single HP-40, defense-3 sentinel fixture with seed-dependent heat tolerance. They do
not establish results against the full ecology. Radius changes are enumerated but a single enemy
cannot establish the value of area coverage. The separate staff trial and existing full-area tests
exercise multiple targets.

### Results

- **Only 4 of 356 structures won all 12 trajectories:** bare spear and bare club, with direct or
  opening aim. No structure carrying magic did.
- **No touch or ground structure killed the sentinel in any of 1,632 trajectories.** All exhausted Focus.
- Payload-carrying structures exhausted Focus in nearly every trajectory. Fire killed once in 2,088
  payload trajectories; Force killed 75 times.
- Bare weapons rarely exhausted (only opening aim costs Focus) and account for most kills and all deaths.
  A bare dagger usually loses the attrition race.

Against one durable target, magic alone is a finite Focus pool. The surveys do not switch to free
strikes after exhausting it.

## Property tests

[`combat_properties.rs`](../../crates/core/tests/combat_properties.rs) uses proptest to generate a random
seed, any of the 356 legal structures, and random player action sequences (steps, techniques, waits, sneak, recover,
discharge, attempted re-preparation). These must always hold, and do:

- Focus stays within 0–24. Blocked actions change nothing but the log. Every other action passes time.
- `can_step` agrees with `step`. Hits land inside their preview's range, and attack and area costs match
  their previews.
- Same seed, loadout, and inputs give the same game. Renaming a maneuver changes no outcome.

Running out of Focus blocks costly techniques, not weapon strikes. For example, the cheapest area spell
(5 Focus) leaves 4 Focus after four discharges beside a living sentinel; the player can still strike,
retreat, or die. Death now returns the player home after 1,000 turns with 1 Health and no Focus
([death rule](actions.md#death-and-retreat)).

## What longer journeys already show

In the 18-seed stationary loadout survey:

| Prepared loadout | Kills | Exhausted | Deaths |
| --- | ---: | ---: | ---: |
| Backstab Burst Spine, power 3 | 0 | 18 | 0 |
| Basic dagger | 3 | 0 | 15 |
| Basic spear | 18 | 0 | 0 |
| Basic club | 18 | 0 | 0 |
| Basic staff | 17 | 0 | 1 |
| Direct force penetration, power 3 | 0 | 18 | 0 |
| Direct force contact, power 3 | 0 | 18 | 0 |
| Direct fire contact, power 3 | 0 | 18 | 0 |
| Direct fire contact, power 1 | 0 | 18 | 0 |
| Opening force penetration, power 6 | 11 | 7 | 0 |

These are comparisons against this sentinel, not evidence that magic is universally worse. They do
show that the featured combination is a poor complete encounter plan, while the free basic spear and
club provide reliable plans in the same fixture. Increasing burst investment can cross a kill threshold,
rather than merely consuming more of the same scarce resource.

Retreat is not universally impossible. Across three straight retreat directions, with at most 32 moves
per direction, the exhausted dagger reached somewhere safe to prepare in **12 of 54 routes**. The staff
did so in **6 of 54 routes**. These are routes, not unique seeds or an optimal escape search.

From each first safe point, the policy prepared a free club and retraced its route, fighting obstacles
along the way. All 18 such routes returned home alive, but only **5 immediately permitted recovery**:
3 dagger routes and 2 staff routes. Nearby creatures still blocked recovery on the other returns.
The first safe points were 11–25 tiles from home. Being safe to prepare did not itself restore Focus.

At seed 42, ordinary westward retreat after dagger exhaustion preserves Focus and avoids immediate
attacks, but the equal-speed sentinel follows one tile behind. It still blocks preparation.
Waiting in place eventually dies instead: from the exhausted turn 3, 25 waits cause death at turn 1028,
including the 1,000-turn death penalty. This survey predates the death rule, when death restored full
Health and Focus. Now death refills nothing, and recovering at home takes about 14 turns of rest.

## Questions exposed by the mechanics

The controlled tests compare identical contexts and identical random streams, separate from the mixed
trajectory groups:

- Does the dagger have a reason to exist beside the spear? Its extra precision is offset by the spear's
  extra force under the current arithmetic; both admit penetration and the same payloads.
- Does the staff have a casting advantage beside the club? Both ground the same areas. There is no
  staff-specific channeling, reach, protection, or cost reduction in this prototype.
- Why admit opening aim on touch or a nonprecise weapon if it only adds Focus cost?
- Does a successful cheap Fire touch communicate that integer rounding can reduce its entire effect
  to zero against a heat-hardy target?
- Can the player finish a fight without replacing their entire prepared composition? Free bump strikes
  now permit this, including after tracking removes the technique's opening benefit. The staff survivor
  regression exercises it; broader mixed strike/technique tactics remain unmeasured.
- Is recovery a tactical decision, or a long trip back to an exact tile that can still refuse recovery?

Force and Fire really do have different counters: armor reduces surface Force, heat tolerance reduces
Fire, and penetration requires actual physical success. A contact payload can outperform a more expensive
penetration payload when armor prevents the trigger. That distinction is worth preserving.

## Missing rather than merely unbalanced

Only Force and Fire payloads exist. The prototype has no executable frost, poison, wards, healing,
bindings, persistent effects, ranged delivery, interruptible casting, or defensive reactions.
Physical variation is force, precision, piercing, and ground capability, not reach, tempo, knockback,
evasion, stamina, or positioning techniques. There is no earned specialization or practice loop.

Consequently many legal permutations are numerically different expressions of the same action, or
mechanically redundant. Enumerating them is useful for finding traps, but does not prove expressive
character identities. More vocabulary alone would not fix mandatory costly attacks, blocked recovery,
or weapon choices that lack a distinct advantage.

This investigation does not diagnose keyboard focus, validate live rendering, or prove that any policy
is optimal. No proposed feature is marked implemented by this report.

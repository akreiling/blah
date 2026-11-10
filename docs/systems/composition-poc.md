# Playable composition proof

[Design index](../SYSTEMS_ROADMAP.md) · [Technique rules](techniques.md) · [Jev review](../review/technique-review.md)

This is a narrow executable proof, not the full advancement system. It deliberately gives the player
developer equipment and known components. Foundations, earned adaptations, saved techniques, remote
naming, and Jev calls are **not implemented**. Weapon practice is implemented; see
[weapon practice](learning.md#implemented-prototype-weapon-practice).

## Try it

From the repository:

```sh
mise exec -- just run --combat-demo
mise exec -- just run --inferno-demo
```

- **Combat demo:** start beside a durable sentinel while sneaking. Press **Shift + east** to attempt the opening strike.
  Compare the next attack after it has located you.
- **Inferno demo:** press **F** to strike the staff. Three nearby creatures have different heat tolerances.
  The fire hits all of them, and you. Bumping a creature uses a free staff strike, not the fire.
- **Normal world:** `mise exec -- just run`. Open the workshop with **M** while safely away from creatures.
  Both proofs use the same `Game` action methods as normal play, not a separate combat simulator.
- **Without a window:** `mise exec -- just composition-demo` prints both trials' structures, previews,
  actual results, costs, and the rejected immediate repeat. It then strikes the sentinel with free
  dagger strikes until the dagger reaches *practiced*, and prints the improved strike preview.

The trial enemies have deliberately controlled stats. They are developer fixtures, not naturally generated
apex encounters or proof that the progression curve is balanced.

Workshop keys: **1** instrument, **2** delivery, **3** aim, **4** payload on/off, **5** element,
**6** release, **7** shape, **+ / −** power. **0** loads the dagger example and **9** the staff example
into the draft only. **Enter** prepares; **Esc** discards. Movement and combat remain paused while editing.

Combat keys: arrows, WASD, or hjkl move and **bump for a free weapon strike**. **Shift + direction**
uses the prepared technique against an adjacent target. **F** discharges a ground-area technique even
without an adjacent target. A basic strike is available at any Focus total, with any prepared delivery.
It uses the equipped weapon, never the prepared aim or payload. It still takes a turn, can miss,
reveals the player, and permits retaliation. It neither changes the preparation nor swaps equipment.

## Composition, not recipe lookup

The mechanical structure in [`combat.rs`](../../crates/core/src/combat.rs) contains:

| Part | Current vocabulary |
| --- | --- |
| Instrument | Dagger, spear, club, staff, with piercing, precision, force, and ground-impact capabilities |
| Delivery | Weapon contact, touch, ground impact |
| Aim | Direct or seeking an unguarded opening |
| Payload | None, force, or fire |
| Release | Contact or penetration |
| Shape | One target or a square area of radius 1–3 |
| Investment | Burst power 1–6 |

A weapon contact can deliver a payload without appearing in a named recipe. A club cannot establish
penetration, but can release a contact burst or ground an area. Touch needs a payload and cannot
penetrate. Area payloads require a ground-capable instrument, ground delivery, contact release, and direct
aim. Invalid intermediate drafts are allowed in the editor, but cannot be prepared.

The name is separate metadata. Renaming a valid maneuver changes neither the mechanical result nor the
random stream. A name cannot grant anatomy, armor penetration, free energy, or an extra trigger.

## Dagger + thaumaturgy + stealth

The example **Backstab Burst Spine** is:

```text
dagger / weapon contact / opening aim
force burst / power 3 / penetration release / point shape
```

Stealth changes whether an observer locates the player. An unguarded opening lets a precise weapon overcome
more defense. If the weapon actually penetrates, the burst releases at that location. Precise placement
at the opening doubles its local effect.

Against the trial's defense 3:

- Opening strike: **15–17 total damage on a hit**, including a 12-damage burst.
- Located strike: **1–8 total damage on a hit**. A glancing hit does not release the penetration payload.
- Each attempt costs **7 Focus**, including misses and failed penetration. A miss does zero damage.
- Every attempted attack reveals the player. Toggling sneak again does not erase the observer's tracking.

These are prototype coefficients, not settled balance. There is no simulated spinal anatomy here; the
example name does not make every creature have a spine.

## Staff + ground impact + fire

The example **Cinder Reckoning** is:

```text
staff / ground impact / direct aim
fire burst / power 6 / contact release / area radius 2
```

It costs **18 of 24 Focus**. The 5×5 square includes the caster's tile:

- Every creature inside is hit, regardless of temperament. No friend/foe filtering.
- The player takes **12 Health damage**. There is no safe center, and casting can kill the player.
- Creature heat tolerance reduces fire damage by up to 75%. The trial creatures take **12, 8, and 3** damage.
- The initial three-target trial does **23 total creature damage**, before limiting it to remaining Health.
- The remaining **6 Focus** cannot pay for an immediate repeat.
- An empty cast still spends Focus and damages the caster.

The same structure works with another ground-capable instrument. Force areas obey armor instead of heat
tolerance. Area investment costs `2 + 2 × power + radius²`; the largest legal area costs 23 Focus at power 6.

Areas resolve immediately, with no miss roll, before normal enemy retaliation. Their square boundary is
explicit. **This proof does not yet implement line-of-sight obstruction, lingering burning, spread, allies,
or delayed telegraphs.** Terrain does not block the wave. Do not treat this as a finished area-combat design.

## Turn and resource rules

- All attacks and discharges pass one turn. Failed affordability checks pass no time or resources.
- Entering sneak costs one Focus and one turn. Each successful sneaking move costs one more Focus.
- Initial detection range while sneaking is `max(normal sense − 6, 0)`. Acute observers can still detect it.
- Already located actors remain tracked within normal sensing range. Leaving that range loses tracking.
- Observation updates even when a slow creature has no action that turn.
- Waiting does not restore Focus. **R** rests one turn at home, only when no creatures are nearby,
  restoring 2 Health and 3 Focus.
- Existing Health regeneration remains. Death returns you home with 1 Health and no Focus, after 1,000
  turns. Creatures survive and recover; those within view of home are driven off.
- Preparation is free but requires no creatures in the visible neighborhood. Editing and canceling spend
  no time. The workshop is paused and cannot be used to move or cast behind the editor.

## What the proof checks

The automated checks exercise:

- Components alone, composed, detected, blocked, and missed.
- Unlisted equipment/composition combinations through the real `Game::use_technique` entry point.
- Free weapon strikes through `Game::step`, across weapons, prepared deliveries, and Focus totals
  including zero. Their previews come from `Game::preview_strike`; `Game::preview_attack` previews
  the prepared technique.
- Cosmetic names producing identical damage and resource outcomes.
- Preview versus resolution, atomic rejection, and turn accounting.
- Full radius-three occupancy: **48 creature tiles plus the caster**, excluding an outside tile.
- Heat tolerance, repeat affordability, empty casts, simultaneous creature deaths, and lethal self-damage.
- Existing world, ecology, movement, and exact-bit replay fixtures.

### Player-action scenario reproductions

See [combat exploration](combat-exploration.md) for the full weapon/magick permutation survey,
four player styles, controlled comparisons, retreat, refitting, and return-to-home journeys.

[`combat_scenarios.rs`](../../crates/core/tests/combat_scenarios.rs) exercises the public `Game` API without
injecting resources, enemy stats, positions, or random outcomes:

- Dagger, seeds 0, 7, and 42: three techniques leave a living sentinel and only 3 Focus. Further techniques
  are blocked, but bumping still strikes without spending Focus or replacing the prepared maneuver.
  Recovery even at home and changing the preparation remain blocked by the nearby sentinel.
- Dagger, seed 42: sneaking again spends Focus and a turn but does not restore surprise against the
  tracking sentinel.
- Staff, seed 42: the fire leaves a 3-Health survivor and 6 Focus. Another discharge is blocked, but
  free staff strikes finish the survivor. Recovery and preparation are still blocked while it is nearby.

The dagger and staff scenarios now guard the free-strike fix. The other characterization tests still
reproduce limitations, not certify an acceptable play loop. They do not establish that retreat is
impossible or diagnose keyboard focus.

Print each scenario's action, state, and new log messages in order:

```sh
mise exec -- cargo test -p blah-core --test combat_scenarios -- --nocapture --test-threads=1
```

Run `mise exec -- just test` and `mise exec -- just lint` for the full checks. Compilation and automated
frontend tests are not a substitute for live GUI validation; no live GUI validation has been performed.

This is evidence for a small interaction kernel. It does not mark all of CP0–CP2 or the later CP7 composition
workspace complete, and it does not prove long-term balance, emergent build breadth, or fun.

# Actions, resources, and recovery

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Core action contract

All gameplay actions use one authoritative resolution path:

1. Query legal targets and preview observable consequences.
2. Select and confirm an action.
3. Revalidate against the current state.
4. Pay costs and commit action time.
5. Resolve the effect and permitted reactions in deterministic order.
6. Update actors, awareness, encounter state, and game time.
7. Award eligible learning from resolved outcomes.
8. Emit structured events for the UI and persistence boundary.

Looking, opening an overlay, and moving an aiming cursor do not advance time. A deliberately attempted
action, such as striking a valid target and missing, does.

Invalid input and an in-world failed attempt must not be conflated. The preview uses only facts the player
is allowed to know; it cannot leak hidden enemies or resistance values.

## Health and Focus

Selected prototype:

- **Health** measures injury.
- **Focus** is a shared budget for special physical exertion and magical expression.
- **Sustained capacity** limits ongoing effects independently of the remaining Focus total.

Basic movement and ordinary attacks remain available at zero Focus. A character with basic magical training
has a modest basic magical action, so a magical identity is not forced to become a swordsman when depleted.
Its baseline action does not grant untrained characters free high-tier magic.

Focus is not a universal conversion rate that makes every build interchangeable. Foundations and technique
requirements still determine what can be attempted.

## Action economy

Keep the existing turn-based model and energy-driven creature actions as the starting point. New techniques
must define:

- When preparation starts.
- When costs are paid.
- When an effect lands.
- Which events can interrupt it.
- Whether recovery or commitment persists.
- How the next creature actions interact with those states.

Do not grant a hidden free turn for changing weapons, breaking a sustain, equipping a relic, or activating a
stance. Specify each operation as either an interface action or a game action.

Prototype preparation changes occur at safe camps. Any later in-combat switching has an explicit cost.

## Stealth

Stealth is an interaction between observer, target, environment, and emitted signals. It is not one global
invisibility boolean.

Minimum observer states:

1. **Unaware:** no relevant current evidence.
2. **Suspicious:** has a signal or last-known location worth investigating.
3. **Alerted:** has sufficient evidence to act against a target.

An observer:

- Receives sight, sound, or supported magical signals according to its senses.
- Records what it actually detected.
- Investigates a location without learning an exact target position for free.
- Loses certainty under defined conditions.
- Can share information only through a supported behavior, not global omniscience.

The UI should show the player's best available read of these states without revealing unseen observers.

Quiet movement, distraction, cover, and quiet casting affect different parts of the process. Armor can
increase detectable movement without making stealth categorically impossible.

## Healing

Healing is an ordinary part of the skill ecosystem:

- Consumables provide reliable emergency options.
- Healing techniques consume Focus and action opportunity.
- Advanced healing can become a specialized identity.
- Protection, positioning, and stealth create opportunities to heal.

Early prototypes avoid permanent injury, multiple wound resources, and mandatory healer investment.

Healing does not award learning based on requested magnitude. It uses actual eligible injury restored.
Self-created, repeated training injuries do not create unlimited evidence.

## Camping and supplies

Selected prototype:

- Safe camping restores Health and Focus.
- It consumes a camping supply and advances game time.
- Nearby unresolved danger can prevent camping, with an explanation.
- Waiting without a camp does not refill both resources indefinitely.
- Home provides a reliable recovery floor.

Only replace the current automatic Health regeneration when the replacement recovery loop is usable.
Removing regeneration before supplies, home recovery, and basic actions exist would create a broken
intermediate build.

Camp safety must not require a tedious clearing radius the player cannot inspect. Start with explicit
shelters and simple visible safety rules.

## Death and retreat

Preserve the existing high-level rule:

- Death returns the character home.
- Skills, committed development, and owned equipment remain.
- Game time advances.
- No practice is awarded for the elapsed interval.

Death is not a world reset. Durable apex actors, treasure claims, discovered arcs, and completed objectives
must survive it. The current clearing of transient nearby monsters cannot become the persistence policy for
named encounters.

Retreat is possible before commitment unless the encounter explicitly and visibly establishes otherwise.
After an encounter is abandoned, apply a defined recovery policy: for the first prototype, combatants
recover after an in-game respite while permanent site changes and claimed rewards remain recorded. A planned
phase transition can use different recovery rules only if they are explicit.

This prevents harmless repeated chip damage from replacing a confrontation without silently restoring
everything as soon as the player steps out of view.

Until ecology catches up over time, death's cost is primarily location and lost journey time. Do not claim
that the current slice already has the eventual world consequences.

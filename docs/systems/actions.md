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

The CP5 first-journey implementation uses a deliberately small recovery loop:

- `P` camps anywhere with no creature in view. It takes one exposed turn and restores 1 Health and 2 Focus;
  it does not consume a camping supply. With two different herbs and room in the two-draught stack, it
  also brews one Focus draught, even if Health and Focus are full.
- `E` gathers from the patch underfoot (three of each kind fit in the pouch). Herbalism can add a second
  herb, keep Skilled picking quiet, and shorten 300-turn regrowth to 150 turns. Reentry and reload do not
  refresh patches.
- `T` drinks a carried draught for up to 8–12 Focus during an exposed turn, including combat, and breaks
  sneaking. Drinking at full Focus or without a draught is refused without spending time or supplies.
- `V` uses one of at most two salves for 10 Health during an exposed turn, including in combat. A fresh
  character starts with two; a cache adds one if there is room.
- `R` rests at home for 2 Health and 3 Focus per turn. Home rest shelters the wanderer from attacks while
  creatures still move and observe, and it refills salves.
- Preparation and relic attunement are free inside home's shelter even with followers nearby. `U`
  exchanges the selected spare cache weapon during a sheltered turn. These actions neither remove
  creatures nor grant practice. Waiting and combat remain exposed at home.
- Waiting without a camp or rest restores neither Health nor Focus; passive regeneration is gone.

The long-term design may add a camping supply. The current camp action instead has a clear safety rule that
can be inspected in the visible neighborhood. The home recovery floor and basic actions remain available
when the character is depleted.

## Death and retreat

Dying is a legitimate way out of a fight. It must never leave you better off than retreating would have.

**What happens when you die:**

- You wake at home after game time advances (1,000 turns in the current slice).
- You wake with **1 Health and no Focus**, and recover by resting at home. Resting restores both
  gradually, over turns, while the world keeps moving. You may leave before you are fully recovered.
- Home is safe to rest in, so waking weak cannot chain into a second death.
- **What you carried stays where you fell.** It is still yours, and is never deleted. Getting it back
  means returning to the place that killed you. Gear stored at home is unaffected.
- Skills and committed development remain. No practice is awarded for the elapsed interval.

The dropped gear is explicit, visible, and recoverable, not a silent loss. Dying again before recovering
it leaves the new gear at the new place; nothing is destroyed. In the first prototype, dropped gear stays
put. Creatures taking it is a later, separate rule.

**What death does not do:**

- Refill Health or Focus. "Die to refill" must not be the cheapest recovery.
- Reset the world. Durable apex actors, treasure claims, discovered arcs, and completed objectives
  survive it. Wounded creatures recover during the time you are away. The current clearing of transient
  nearby monsters cannot become the persistence policy for named encounters.
- Promote the creature that killed you as a reaction to your build. A creature's own history may
  change it, under ordinary [ecology](ecology.md) rules.

Retreat is possible before commitment unless the encounter explicitly and visibly establishes otherwise.
After an encounter is abandoned, apply a defined recovery policy: for the first prototype, combatants
recover after an in-game respite while permanent site changes and claimed rewards remain recorded. A planned
phase transition can use different recovery rules only if they are explicit.

This prevents harmless repeated chip damage from replacing a confrontation without silently restoring
everything as soon as the player steps out of view.

**Implemented so far:** death wakes you at home with 1 Health and no Focus, and **R** rests one turn at
home for 2 Health and 3 Focus. A rest action shelters you from creature attacks even if creatures are
nearby. They still move and observe, and the world still spawns creatures. Rest does not clear or heal
them. A fully recovered character cannot use rest as a protected wait. Ordinary waiting, attacking,
casting, toggling sneak, and moving are exposed, including at home. This gives living retreat and
post-death recovery the same floor without granting a combat shield.

Creatures survive death and fully recover; those within view of home are driven off.
**Not yet:** dropped gear. Owned weapons and carried supplies currently stay with you; death does not refill
supplies. Herb patches regrow during the elapsed 1,000 turns.
See [measured retreat and recovery](retreat-recovery.md) for the cautious-policy comparison and limits.

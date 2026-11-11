# Equipment, treasure, and gathering

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Equipment changes expression

Equipment should matter through physical and magical properties:

- Reach and delivery.
- Protection and exposure.
- Noise and movement.
- What effects can be conducted or sustained.
- Ammunition and recovery behavior.
- Capacity to support a specialized technique.

Numerical upgrades are allowed. They cannot be the whole item system.

[Loadout properties](loadout-properties.md) maps these into concrete properties and the actions that read them.

## Three item families

| Family | Purpose | Example |
| --- | --- | --- |
| Ordinary equipment | Dependable tools with clear trade-offs | A longer spear, quieter armor, throwing implements |
| Specialist equipment | Supports a practiced identity or unusual environment | A close-casting focus, a projectile harness, protective travel gear |
| Relics | Change one bounded rule | Sustain a particular ward hands-free; make a thrown object emit a decoy signal |

An item must state whether it grants an effect, modifies a known technique, or changes a capacity. Do not
let those meanings blur in tooltips or validation.

## Preparation and fitting

The first loadout experiment starts with:

- Four readied special techniques.
- Two relic slots.
- A small set of explicit weapon, protection, and utility equipment positions.
- Basic actions outside the special-technique count.

These are tuning values. The design requirement is understandable opportunity cost, not these exact counts.

Keep two checks separate:

1. **Structural feasibility:** can this character support the technique or effect?
2. **Current fitting:** can this equipment and prepared combination be used together now?

A spare relic slot cannot bypass an incompatible adaptation. Conversely, a learned technique is not deleted
because it is currently unprepared.

## Treasure placement

Treasure belongs to sites and events, not automatically to kills.

A guarded cache may be reached by:

- Winning a confrontation.
- Diverting or separating its guardians.
- Approaching through another traversable route.
- Using a capability that bypasses an obstacle.

The engine validates access and claim rules. A chest must not require its own contents to open the only
route leading to it.

Ordinary creatures generally yield plausible materials or nothing. Do not make every animal drop a sword
because the progression economy needs one.

## Implemented CP5 first-journey items

The first journey starts with one plain item from each weapon family: dagger, spear, club, and staff. The
first cache in a land region is deterministic from the world seed and region identity, is marked on the map,
and is guarded by two or three territorial creatures with a four-tile leash. Cache guards never stand in
the home view, so a cache cannot make home unsafe; a tiny pocket region lying wholly inside the home view
has no room for out-of-sight guards and so has no cache. `G` claims it for one exposed
turn without requiring the guards to die. Claims and guard health/deaths survive leaving the region, death,
and save/load.

Cache variants keep the same learned weapon family and have explicit trade-offs:

| Item | Effect | Cost |
| --- | --- | --- |
| Heavy club | +1 force | 2 percentage points more weapon misses |
| Keen dagger | +2 opening precision | −1 force |
| Long spear | Reach costs 1 less Focus | −1 force |
| Quiet staff | +1 concealment while sneaking | −1 force |

Inventory contains only owned items. The workshop can equip an owned item at a safe point; it cannot equip a
cache reward that has not been claimed. This is a narrow journey slice, not the full equipment, relic, or
gathering system proposed above.

## Reward value

A successful reward can:

- Strengthen a recognizable part of the current identity.
- Introduce a nearby developmental possibility.
- Provide knowledge of a technique not yet attainable.
- Open access to a habitat or site.
- Extend a journey through supplies.
- Create a reason to attempt a more ambitious encounter.

The world does not secretly tailor every reward to the current build. Some treasure will be unsuitable.
Players need useful choices for it, such as storage or a simple conversion mechanism, but a full trading
economy is not required for the first slice.

## Gathering and materials

Gathering provides progression outside compulsory combat:

- Resources have locations, extraction conditions, and a finite current yield.
- Delicate extraction can reward precision and suitable magic.
- Threats and environmental access can make gathering interesting.
- Resource provenance can connect equipment to the living world.

Start with a few materials used for supplies and a small number of equipment improvements. Do not add a
large crafting graph before gathering is an interesting action.

Creature-derived materials should reflect actual traits. Harvesting and scarcity rules should not make
exterminating every population the default strategy for character growth.

## Economy safeguards

- No duplicate reward on retry, reload, death, or late remote completion.
- A stack split, equipment swap, or dropped item cannot duplicate a unique relic.
- A discarded item remains discarded or located in the world according to a recorded rule.
- Depleted resources do not regenerate merely because their cache entry was evicted.
- Resupply cannot require spending the consumable needed to reach all available sources.
- Failure leaves a viable basic recovery path.
- Item descriptions must match their actual catalog effects.

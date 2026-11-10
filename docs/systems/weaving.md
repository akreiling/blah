# Weaving and unforeseen builds

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Weaving disciplines

A weave is stronger because the components interact causally, not because the character receives a bonus
for having two labels on a sheet.

Five useful relationships:

| Relationship | Example | What the integration contributes |
| --- | --- | --- |
| Delivery | Ranged arms delivers an altered venom carrier | Applies an effect in a way the alteration alone cannot reach |
| Enablement | Warding protects a committed surge | Permits an aggressive physical action that would otherwise be interrupted |
| Transformation | Binding turns repeated frost pulses into a maintained field | Creates a persistent spatial problem rather than repeated standalone attacks |
| Concealment | Veiling suppresses the delivery signal of a subtle effect | Changes detection and approach, not only damage |
| Coordination | Acrobatics preserves a compatible ward while changing position | Combines mobility and ongoing protection without pretending either is free |

Prototype weave lifecycle:

1. The player learns useful components.
2. They encounter situations where those components can interact.
3. Resolved activity provides evidence of that relationship.
4. The engine exposes a legal integration from the actual compatible components and relationship.
5. The player practices or commits the integration under its actual costs.
6. Later refinements improve coordination, scale, and control within that structure.

There is no automatic "two tracks at level ten equals a new class" rule. Nor should every combination need
a unique hidden quest or an authored pairing. Generic relationship rules and a small composition grammar
provide lawful surprises without searching over arbitrary programs. A player's unanticipated but legal
combination is a success case, not a reason to reject it.

## Specific laws for the requested combinations

**Poison and stealth**

- A toxin needs a supported susceptibility and a real delivery.
- Its damage or debuff has a defined dose, duration, and stacking rule.
- Quiet delivery does not guarantee that consequences remain undetected.
- A delayed effect does not grant a fresh learning allowance every turn.
- A protected, resistant, or non-susceptible target calls for another plan, not an unannounced immunity
  invented for the player.
- The current prototype is fictional game logic, not a system of real-world toxin recipes.

**Rage and protection**

- Rage is a controlled committed state, not arbitrary loss of input control.
- It can amplify momentum, follow-through, or resistance to interruption.
- A practiced ward can cover part of the exposure created by that commitment.
- The ward still consumes supported capacity and can be broken or exhausted.
- Rage does not categorically prohibit casting; unsupported simultaneous demands do.
- Self-harm, endless harmless hits, and repeated toggles do not generate infinite power or practice.

**Area-of-effect frost**

- The area, duration, obstruction rules, and upkeep are explicit.
- A field and a target's personal cold tolerance remain separate facts.
- Cold resistance can reduce a biological effect without deleting a separately supported terrain obstacle.
- Freezing water or changing footing occurs only if an authored overlay mechanic supports it.
- Wide coverage has concentration and control costs, but an expert's area power must still be substantial.
- The field needs escape, interruption, counterplay, or a stated limitation; it cannot permanently lock every
  target out of taking actions.

## Weave implementation contract

The shared integration system must provide the following for any accepted composition:

- Stable component IDs and a canonical composed identity.
- A generic, authored relationship rule for when the interaction is meaningful.
- Supported inputs and effect outputs.
- Combined costs, preparation, sustain load, and break conditions.
- Practice-credit attribution without double counting.
- A queryable explanation of legality and failure.
- Tests for components alone, together, interrupted, and at their strongest legal refinement.

Do not add a free-form "synergy percentage" field as a substitute for this relationship.

At maturity, a familiar weave can become a single prepared technique rather than requiring the player to
repeat the same long setup sequence for every peon. Its combined costs and dependencies still apply.
Compression should reduce input repetition, not create free actions or erase its constituent commitments.

## Emergence without an unbounded rules engine

There are two kinds of emergence:

1. **Tactical emergence:** independent techniques, gear, creatures, and terrain interact through shared
   states. The player can discover an unexpected sequence before it becomes a practiced single technique.
2. **Developmental emergence:** related practice and structural commitments make a distinctive integration
   increasingly fluent. Where a composition fits the grammar, the player can preserve and refine it.

We author contracts such as:

- A carrier can hold a bounded compatible payload.
- A link can preserve a supported relationship within its range and load.
- An observer reacts to the signals it actually detects.
- A ward protects a defined boundary against specified effects.
- A movement technique changes position while preserving only supported commitments.

We do not author only pairwise exceptions such as "venom shuriken gets a bonus" or "frost plus this one ring
unlocks the field mage." Those can be example compositions, but the underlying rules must also work for
other compatible carriers, effects, links, and equipment.

For instance, a player might combine a recoverable thrown carrier, a decoy signal, and a maintained link to
move an observer's attention across several positions. If every component supports that delivery, the link
has capacity, and the signal behaves lawfully, the result should work without a bespoke "moving lure" recipe.
This example is not itself an exhaustive prescription for Binding or a promise that the first prototype
already supports it.

## Bounded execution, open outcomes

Constrain execution rather than imagination:

- Maximum composition size and depth.
- Explicit input/output compatibility.
- Limited active links, carriers, projectiles, targets, and reactions.
- Deterministic ordering and ownership.
- Declared duration, upkeep, and break conditions.
- Resource and learning conservation.
- No arbitrary recursion, loops, scripts, or external calls.

Those limits still permit combinations no designer has tested individually. Use invariant tests, generated
legal compositions, and adversarial independent playtests instead of pretending to enumerate every possible
build.

Never accept a composition solely because a remote model says it should work. Never reject a composition
solely because no remote model or designer has named it.

## How to treat an unexpected powerful build

First ask which kind of discovery it is:

| Discovery | Response |
| --- | --- |
| Lawful, costly, specialized, and unusually effective | Preserve it and learn from it |
| Strong against one ecological situation | Usually desirable specialization |
| Uses an obscure but coherent rule | Improve explanation and evidence |
| Creates unlimited resources, repeated rewards, or unbounded execution | Fix the broken invariant |
| Dominates nearly every meaningful situation with no real opportunity cost | Examine the general cost or interaction law, not a punitive build-specific exception |

Do not automatically flatten surprising power to preserve the designer's expected damage chart. Endgame
power is supposed to be remarkable. Likewise, do not manufacture adaptive counters against the particular
player who discovered a strong combination.

If a rule changes, version it and handle existing saved compositions explicitly. Do not silently delete a
player's signature technique or reinterpret an old save under incompatible rules.

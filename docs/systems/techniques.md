# Techniques and composition

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## The genetics analogy

The analogy is useful at the level of structure and expression:

| Creature system | Character system |
| --- | --- |
| Inherited potential | Initially open developmental potential |
| Environmental pressure | Repeated meaningful activity and chosen challenges |
| Genome structure | Foundations and committed adaptations |
| Phenotype | Available techniques and how they behave |
| Lineage identity | A character's recognizable way of acting |

Do not literally copy breeding or random mutation into player advancement. The player needs foresight and
agency. Likewise, species evolution and individual learning occur on different timescales and remain
different simulations.

## Technique anatomy

Each technique is a validated composition of authored parts:

| Part | Question | Examples |
| --- | --- | --- |
| Delivery | How does it reach the world? | Self, touch, weapon contact, thrown object, projected effect |
| Effect | What does it do? | Physical harm, force, heat, cold, protection, restoration, distraction |
| Shape | Where does it apply? | One target, a short cone, a line, a bounded area |
| Execution | How is it performed? | Immediate, committed, sustained, reactive |
| Behavior | What changes its operation? | Quiet, returning, chained, concentrated |
| Requirements | What makes it feasible? | Equipment, proficiency, foundations, adaptations |
| Costs | What does using it consume or expose? | Action time, Focus, ammunition, attention, noise, position |

The first implementation should use a small closed set of these parts. It is not a request for arbitrary
new player-authored primitives, a scripting language, or eagerly generating an enormous Cartesian product.
Players can ultimately arrange and develop supported parts into new legal structures. The vocabulary is
bounded; its valid arrangements are not restricted to a designer-written recipe list.

## Composition legality

A technique definition is valid only if:

1. Every referenced part exists in the current rules catalog.
2. Its delivery can carry its effect and shape.
3. Its execution has an explicit timing rule.
4. Its cost and limits are resolved by the core.
5. Its requirements are feasible for at least one intended developmental path.
6. Its information is sufficient for targeting, preview, AI use, and logging.
7. Its modifiers do not produce contradictory behavior.
8. Its strongest legal combination respects the tested effect and resource bounds.

It does **not** need an exact match in a catalog of finished spells or named builds. Catalogs own primitives,
their contracts, and starter examples. A valid composed technique receives a canonical saved identity from
its structure and rule version, even if nobody authored that exact structure before.

For example, "quiet" affects emitted detection signals; it does not erase the light from a visibly blazing
projectile unless the technique separately supports that property.

"Returning" does not imply infinite free projectiles. Recovery, interruption, collision, and ammunition
ownership still need rules.

## How new techniques appear

Use three sources:

- **Practice developments:** a known action develops a compatible variation.
- **Found knowledge:** instruction, an artifact, or a world observation introduces a possible new structure.
- **Integration:** meaningful use of two compatible activities exposes a coordinated form.

These are sources of knowledge, not an exhaustive list of suggestions the engine may offer. Once the
composition interface is delivered, players can also propose bounded structures from components they know.
The core previews feasibility, costs, and requirements, and explains invalid connections. A legal unfamiliar
structure can be saved and practiced without a designer adding its recipe. Proposal alone grants neither
mastery nor a bypass of structural commitments.

A discovery can reveal a technique the character cannot yet support. It becomes an ambition, not an
automatic bypass of foundations.

A found scroll must not let a heavily committed physical specialist instantly operate high-tier magic with
the same fluency as a lifelong channeler. A limited-use item can grant an exceptional effect, but that does
not rewrite the user's structure or award its mastery.

## A concrete starter vocabulary

These are prototype candidates. Build a small selection first; expand only after their interactions work.

| Technique | Standalone job | Natural interactions | Cost or limitation |
| --- | --- | --- | --- |
| Basic strike | Dependable close attack | Openings, commitment, armor | Requires reach and exposes the attacker |
| Throw | Dependable ranged physical attack | Distance, distraction, exposed targets | Ammunition and a clear trajectory |
| Magic missile | Dependable modest magical attack | Targeting, wards, visibility | Requires basic channeling; reveals magical activity |
| Double cut | Two controlled lighter strikes | Unarmored targets, prepared openings | Focus and commitment; armor applies meaningfully |
| Shove | Move or disrupt a nearby target | Terrain, cover, interrupted actions | Mass and bracing can resist it |
| Short burst | Concentrated nearby magical force | Close entry, disruption, escape | Short reach, Focus, conspicuous output |
| Brace ward | Survive a predicted impact | Committed casting, narrow approaches | An action and sustained capacity |
| Magical decoy | Create an investigable signal | Separation, quiet approach, route control | Costs Focus; not universal mind control |
| Quiet casting | Suppress specified casting signals | Concealment, ambush, extraction | Lower speed or output, or higher Focus cost |
| Frost bind | Impede a susceptible target | Distance, retreat, slower committed attacks | Resistance and limited duration |
| Gentle draw | Extract a delicate material intact | Cover, stealth, protection | Time and exposure; not free remote harvesting |
| Mend | Restore recoverable Health | Protection, retreat, recovery planning | Focus, action time, and a defined maximum effect |
| Venom mark | Apply a supported fictional toxin through a valid carrier | Quiet delivery, delayed effects, susceptible physiology | Dose, duration, resistance, and detectable consequences |
| Committed surge | Turn controlled physical exertion into a forceful sequence | Protection, displacement, interruption resistance | Exposure and recovery; not free damage from self-harm |
| Frost field | Control a bounded area with a maintained cold effect | Anchors, route control, selected terrain | Focus, sustain capacity, visible extent, and escape routes |

The grammar should describe these cleanly before it attempts late-game blade storms or terrain-scale magic.

## Development within a technique

A technique can improve along different properties:

- Quieter, but slower.
- Faster, but less concentrated.
- More concentrated, but narrower or more committed.
- Sustained, but occupying attention.
- More selective, but affecting fewer targets.

These developments are connected to the player's practice and adaptations. Do not present every technique
with the same generic upgrade menu.

Major refinements should remain legible in the action preview. A player must know whether their version of
Double Cut is quick, committed, magically extended, or otherwise unusual.

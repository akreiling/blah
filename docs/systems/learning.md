# Practice, learning, and specialization

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Award learning from outcomes

The core emits a resolved activity record after an action or encounter outcome. Learning reads that record;
it does not infer behavior from UI commands, text logs, or wall-clock time.

Relevant facts include:

- The actor and stable activity or encounter identity.
- The technique used and the part of it that actually took effect.
- What obstacle, threat, or resource was involved.
- The actor's state, including active protection and awareness relationships.
- The result: effective damage, successful concealment, useful healing, completed extraction, interruption,
  or another defined outcome.
- Which compatible interaction genuinely occurred.

Do not store every possible fact forever. Each fact needs a rule that consumes it. Aggregate credited
evidence into bounded records and persist enough identity to prevent repeat claims.

## Meaningful practice

Meaningful does not mean "nearly died." It means the action accomplished or attempted a relevant task at a
level that can still teach this character something.

Examples:

| Activity | Qualifying evidence | Non-qualifying loop |
| --- | --- | --- |
| Swordplay | Controlling a real opponent, exploiting a valid opening | Repeated attacks against an inert invulnerable target |
| Protection | Preventing an independently caused threat | Manufacturing harmless impacts solely to trigger credit |
| Stealth | Passing an aware-capable observer or completing a concealed objective | Toggling sneak on an empty tile |
| Healing | Restoring meaningful injury during or after a real situation | Self-injury followed by infinite healing |
| Gathering | Successfully extracting an available resource with relevant difficulty | Repeatedly interacting with a depleted node |
| Ranged control | Landing or redirecting a meaningful projectile action | Firing into empty ground forever |
| Integration | Two abilities contribute to the same actual outcome | Activating a useless ward before every unrelated action |

Failure can provide limited relevant learning when it genuinely tests a skill. It must not become better
than accomplishing the task, nor encourage deliberately sabotaging encounters.

## Learning credit model

Prototype pipeline:

1. Validate that the activity can teach this character at its current stage.
2. Identify the finite learning opportunity associated with the situation.
3. Compute useful contribution from the resolved effect, not requested magnitude.
4. Apply relevance to the practiced foundations and proficiency.
5. Allocate a bounded portion to an actual integration, if one occurred.
6. Record the claim under the durable opportunity identity.
7. Update visible development evidence.

One action may teach several connected things, but it does not multiply the total opportunity simply because
it carries many tags. Teaching ten nominal disciplines with one action must not generate ten times the
advancement.

Repeated attempts in the same situation share a learning allowance. Leaving sight, reloading, changing
weapons, or letting a creature respawn must not mint a new allowance for the same unresolved encounter.

## Baseline competence and advanced learning

Practice has two broad regimes:

- **Basic competence:** ordinary relevant activity can teach it; experimentation should not require rare
  treasure or difficult combat.
- **Deep development:** new situations, substantial execution demands, and related discoveries are needed.
  Solved harmless tasks plateau.

This allows local experimentation while preserving a long arc of adventuring. It also avoids tying all
advancement to kills or arbitrary creature "levels."

## Integration learning

Integration is not awarded simply because two skills were used during the same expedition. The relationship
must matter.

Examples:

- A ward absorbs a blow while a committed close-range cast completes.
- A decoy causes an observer to investigate elsewhere, enabling a concealed crossing.
- A shove creates enough distance for a ranged action that otherwise could not have been completed.
- A movement technique preserves concealment while a quiet spell is prepared.

Start with explicitly authored relationship predicates over resolved effect provenance, such as an actual
carrier delivering a payload or a ward preventing an interruption. These apply to compatible components
generically, not only listed technique pairs. Do not build a universal inference engine that invents causal
relationships from arbitrary action histories.

Each relationship can support several builds and novel compositions. Avoid one secret prerequisite recipe
for every named hybrid. An unforeseen legal interaction must still receive appropriate practice evidence.

## Developmental commitments

Learning evidence makes relevant adaptations available. The player chooses among supported developments,
with structural costs shown before commitment.

The player cannot simply save up a generic currency and buy an unrelated mature identity. Learning a new
direction means doing it, and the character may no longer have the structure to master it deeply.

Progress remains attached to the character, not the currently equipped item. An enchanted weapon can assist
performance but does not become an experience multiplier or reusable training loophole.

## Time and repetition

Three clocks must stay distinct:

| Clock | Used for | Must not do |
| --- | --- | --- |
| Action and game time | Actions, recovery, ecology, death consequences | Grant mastery solely because turns elapsed |
| Meaningful experience | Learning and development | Count identical safe repetition without limits |
| Real-world time | External service budgets and operational scheduling | Train the character, advance the world while the application is closed, or gate ordinary play |

A camping interval and a death time jump can advance ecology but do not award practice.

## Progression pacing

The intended shape is:

- Early useful experimentation.
- A noticeable but explained transition into commitment.
- Many intermediate improvements within the chosen identity.
- A very long path toward extraordinary mastery.

Do not set the final progression curve by guessing a total number of hours. First measure how much relevant
experience normal play produces, including non-combat play, failure, retreat, and difficult exploration.

Later checkpoints must establish a pacing target and test full routes from fresh characters. Accelerated
developer characters can test endgame mechanics but cannot validate the journey to them.

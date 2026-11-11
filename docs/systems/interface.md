# Player information and interface

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Show useful information before numerical completeness

The player needs:

- Where a technique can reach.
- What it will cost.
- Whether it commits or exposes them.
- What visible threats are preparing.
- Whether an observer is investigating or has located them.
- Why a known technique is unavailable.

Raw genome values and full damage formulas are optional advanced information. Basic fairness cannot depend
on unlocking Lore.

## Development view

Show:

- Current foundations and committed adaptations.
- Remaining structural capacity.
- What recent meaningful activities contributed.
- Available breakthroughs and their consequences.
- Related techniques and integrations.
- Which directions are adjacent and which are structurally blocked.

Avoid a wall of independent bars. Group information around the character's emerging identity while keeping
the underlying rules inspectable.

Any identity label is descriptive. The game may describe a pattern as warded swordplay or subtle projection;
it must not silently turn that label into a class restriction.

## Technique view

For each technique:

- Its delivery, effect, shape, and execution.
- Its current refinements.
- Costs and sustained commitments.
- A short list of actual known interactions.
- Foundation and equipment requirements.
- Evidence of progress toward the next available development.

The UI must distinguish "unknown," "known but structurally unsupported," and "supported but not prepared."

## Composition workspace

At a safe preparation point, the player can propose a bounded structure using known components:

1. Select a delivery, effect, and supported modifiers or relationships.
2. Inspect the core's legality, timing, cost, and structural-requirement preview.
3. See specific rejected connections, such as an incompatible carrier or exceeded sustain capacity.
4. Revise or cancel without spending gameplay resources.
5. Save a legal composition under its canonical structure and rules version.
6. Prepare and practice it through the same action and learning rules as authored starter techniques.

Suggested developments can help discovery, but they are not the only allowable recipes. The interface must
admit an unanticipated legal composition and clearly distinguish knowing its structure from mastering its
execution. This workspace is an explicit CP7 deliverable; tactical emergence is tested earlier.

## Creature knowledge

Record observations with species, region, and game time.

Useful knowledge includes:

- Observed temperament.
- Demonstrated movement access.
- Known defenses or tolerances.
- Seen capabilities and warning signs.
- Differences from a named prior observation.

The current nearby comparison should eventually identify its reference region instead of only showing
arrows. Old observations can become outdated after drift; mark them as historical, not silently wrong.

## Encounter information

- Clues are readable world objects or log entries, not only remote prose.
- Telegraphs use consistent visuals and timing.
- Area previews distinguish certain effects from uncertain predictions.
- Players can inspect a clue without advancing several hidden turns.
- Objectives describe observable goals rather than revealing the entire boss script.
- A committed site's details remain consistent across load and revisit.

## Control discipline

Keep keyboard-first play:

- Basic movement and bump combat remain quick.
- Targeted actions have an explicit targeting and confirmation step.
- Canceling targeting does not spend resources.
- Aiming does not reveal hidden actors.
- Technique hotkeys use prepared actions, not a sprawling permanent keymap.
- Peon clearing does not require opening a full ability panel every turn.

## Implemented CP5 journey controls

The first journey exposes its small action set directly in the play view:

- `G` claims the cache underfoot; the map marks an unclaimed cache `C` and a claimed one `c`.
- `P` camps for one exposed turn when no creature is in view, also brewing a draught from two different
  herbs if possible.
- `E` gathers the herb patch underfoot. Ready patches use a bright `*`; regrowing patches use a dim `·`.
  The pouch panel identifies the herb underfoot and whether it is ready.
- `T` drinks a Focus draught, including in combat. `D` remains eastward movement in WASD.
- `V` uses a salve during a normal turn, including combat.
- `R` rests at home. The equipment panel shows the equipped item, owned items, and salve count; the workshop
  lists owned weapon variants and equips one only when preparation is safe.
- The herb pouch shows all three herb counts (capacity three each), the draught count (capacity two), and
  the brewing recipe. Supplies do not automatically refill when camping or resting at home.

The nearby cache panel shows its direction, claim state, living-guard count, and reward before the player
reaches it. Claiming breaks sneaking, so the exposed turn is visible to territorial guards.

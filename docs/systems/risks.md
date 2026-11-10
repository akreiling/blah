# Risks and unresolved tuning

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Risk register

| Risk | Early warning | Response |
| --- | --- | --- |
| Classless becomes universal mastery | Old builds converge despite different histories | Enforce structural limits and conflicting commitments |
| Commitment becomes a hidden trap | Players fear experimenting or discover irreversible costs too late | Improve previews and separate early evidence from major commitment |
| Use-based growth becomes grinding | Contrived repetitive behavior outperforms adventuring | Rework opportunity credit and advanced-learning requirements |
| Hybrids are weak generalists | Pure builds dominate every relevant fixture | Add useful integrations, not indiscriminate stat bonuses |
| Tracks are disguised classes | Each track combination has exactly one prescribed build | Keep delivery, practice, adaptations, and refinement independent |
| Emergence is only a marketing claim | Unlisted pairings fail until someone adds a special recipe | Replace pairing exceptions with shared component contracts |
| Physical growth is shallow | All interesting late choices come from spells | Deepen physical/physical coordination and embodied power |
| Weaving becomes buff upkeep | Every fight starts with the same long activation sequence | Let practiced integrations compress inputs while retaining costs |
| Replayability is only frustration | Players restart to escape mistakes rather than pursue ideas | Improve commitment clarity and early expression of alternative identities |
| One combination dominates | Every identity prepares the same technique/relic chain | Examine its opportunity cost and counterplay in real contexts |
| Ecology remains decorative | Players ignore phenotype information | Connect an existing trait to an actual decision before adding genes |
| Power never feels real | Every foe remains a close numerical match | Remove hidden scaling and preserve ordinary populations |
| Godlike power removes decisions | Late play is one unlimited action | Refine capacity, commitment, scale, and competing objectives |
| Apex fights are arbitrary | Players cannot connect warnings to outcomes | Rebuild telegraphs, prerequisites, and escalation |
| Peons become chores | Lead-up takes many repetitive inputs | Reduce compulsory attrition and improve decisive actions |
| Generated arcs invent mechanics | Prose promises unsupported interactions | Tighten the catalog and validator; reject instead of improvising |
| Generated arcs become repetitive | Different text describes the same sequence | Expand meaningful templates only after identifying the repeated decision pattern |
| Death becomes a progression exploit | Suicide is the best refill or training route | Measure recovery incentives; fix resource design without silently adding punitive loss |
| Drift erases authored facts | A revealed clue or actor no longer matches the site | Separate immutable definition from compatible world/runtime updates |
| Scope stalls the project | Many frameworks exist but no complete journey is playable | Return to the next checkpoint's smallest complete loop |

## Open tuning decisions

These are not invitations to reopen the committed direction:

- Exact foundation categories and how proficiency differs in the UI.
- Number and structural costs of major adaptations.
- Which adaptations are incompatible versus merely expensive to combine.
- Which magical methods and physical practices provide the smallest complete discipline map.
- Timing of the first explicit commitment.
- How many intermediate refinements support the long progression.
- Exact learning-credit and plateau curves.
- Focus capacity, sustain capacity, and recovery values.
- Prepared technique and relic counts.
- Action timing for multi-step casts, reactions, and equipment changes.
- Ammunition recovery and basic magical-action balance.
- Camp supply availability and site safety rules.
- How much advance warning exceptional threats need in different terrain.
- Which late-game powers remain readable in the viewport.
- The supported first terrain-overlay mechanics.
- How much ecological catch-up can be approximated while preserving intended outcomes.
- Which remote model and service limits are appropriate at implementation time.

Resolve a tuning decision at the checkpoint that needs it, record the chosen value and evidence, and keep a
single source of truth in code. Do not spread different versions through the UI, generator prompt, and tests.

## Decisions deliberately deferred

- Exceptional respecialization or bodily transformation.
- Shared worlds for several player characters.
- Account-wide unlocks or inherited treasure.
- Full companion training and the breeding pen.
- Towns, merchants, quests, and a broader economy.
- Deep crafting and extensive resource chains.
- Permanent injury, hunger simulation, and equipment durability.
- Arbitrary player scripting or creation of new primitive mechanics. Structured composition of supported
  components is in scope, not deferred.
- A global ending or ultimate final boss for an endless world.

The current plan supports site-scale apex encounters and long-term power without deciding those systems.

## Scope rule

At each checkpoint, ask:

> Does this work make the next playable experience better, or only make the imagined final game larger?

If the answer is only the latter, defer it.

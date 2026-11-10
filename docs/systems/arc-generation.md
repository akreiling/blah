# OpenRouter as an arc drafter

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Division of authority

| Owner | Authority |
| --- | --- |
| World generation and ecology | What terrain, lineages, resources, and exceptional candidates exist |
| Core rules catalog | What capabilities and interactions can execute |
| Encounter validator | Which placements, sequences, rewards, and budgets are legal |
| OpenRouter drafter | Proposed buildup, clue arrangement, thematic framing, and selection among permitted structures |
| Player actions | How an accepted arc actually unfolds |

The model writes proposals, never executable mechanics. A model's opinion that an encounter is fair or fun
does not replace validation or playtesting.

## Start as a development tool

First, use manually requested drafting batches during development:

1. Prepare a compact fixture containing real game facts and a legal catalog.
2. Request a small number of arc proposals within an explicit budget.
3. Validate them mechanically.
4. Read them for clarity and repetition.
5. Play representative accepted arcs.
6. Improve prompts and the encounter grammar based on evidence.

Only add unattended runtime drafting after the core encounter format and offline fallback work.

This document authorizes no external call or spending. Runtime generation must be explicitly enabled by the
player, with configured limits. Development batches need their own explicit authorization.

## Input snapshot

Supply only the facts needed to draft the site:

- Schema and rules catalog versions.
- Site identity and revision.
- A bounded map or engine-selected candidate placements.
- Local terrain properties and permitted overlays.
- Relevant species and actual candidate capabilities.
- Exceptional resource or apex-candidate facts.
- Budget and reward-catalog constraints.
- Available encounter templates.
- Recent nearby arc summaries for variety.

Do not send:

- API keys or authentication material.
- Files, raw saves, player-authored notes, or personal information.
- Player stats, equipment, prepared techniques, or hidden build preferences.
- Unbounded action histories or the entire world.

The engine chooses a bounded summary. Text from names, prior drafts, or imported content is data, not an
instruction that can override the schema or catalog.

## Output contract

The draft references existing catalog and snapshot IDs. It does not provide arbitrary damage formulas,
scripts, URLs to execute, asset downloads, or new predicates.

Illustrative output shape, with example IDs rather than a committed Rust schema:

```json
{
  "schema_version": 1,
  "rules_version": "prototype-1",
  "site_id": "reservoir-example",
  "snapshot_revision": 3,
  "template_id": "territory-escalation",
  "apex_candidate_id": "candidate-reservoir-guardian",
  "beats": [
    {
      "id": "dry-nests",
      "kind": "clue",
      "placement_id": "placement-west-bank",
      "template_id": "abandoned-nests",
      "requires": [],
      "fact_refs": ["fact-nests-abandoned", "fact-channel-water-drawn"],
      "text": "Empty nests line the bank. Fresh water runs uphill through narrow channels."
    },
    {
      "id": "channel-guard",
      "kind": "encounter",
      "placement_id": "placement-channel-crossing",
      "template_id": "supported-local-guardians",
      "requires": ["dry-nests"],
      "fact_refs": ["fact-local-shell-defense"]
    },
    {
      "id": "reservoir-apex",
      "kind": "apex",
      "placement_id": "placement-reservoir",
      "template_id": "reservoir-release-cycle",
      "requires": ["channel-guard"],
      "fact_refs": ["fact-apex-reservoir-bond"]
    }
  ],
  "opportunities": [
    "opportunity-observe-release",
    "opportunity-cross-upper-bank",
    "opportunity-disrupt-channel"
  ],
  "reward_table_id": "reward-supported-relic-and-supplies"
}
```

Every ID in such a draft must be provided or explicitly allowed by the engine. The model cannot invent an
uphill-water fact and make it true by citing its own invented ID.

`requires` determines an engine-defined beat dependency. It must not mean every physical approach is blocked
until all prior enemies are killed. Spatial access, observation, and objective completion are separate rules.

## Validation stages

**Structural validation**

- [ ] Valid supported schema; bounded text, arrays, and nesting.
- [ ] Unique IDs and valid references.
- [ ] No executable code, external dependencies, or unrecognized capability names.
- [ ] Acyclic and satisfiable dependency graph.
- [ ] Supported template and rules versions.

**World validation**

- [ ] Snapshot revision still matches at acceptance.
- [ ] All placements are real, available, and compatible with the site's terrain.
- [ ] Actors can stand and move where required.
- [ ] Referenced resources and powers have valid provenance.
- [ ] Persistent actors are not duplicated across sites.
- [ ] The arc does not overwrite a revealed site or a player's completed action.

**Mechanical validation**

- [ ] Costs and threat budgets are recomputed from actual mechanics.
- [ ] At least the intended approach families are feasible with fixture capabilities.
- [ ] No required key or access tool is locked behind itself.
- [ ] Required consumables have an available source.
- [ ] Targeting, range, awareness, and terrain rules agree with the core.
- [ ] Summons, reactions, environmental changes, and recovery are bounded.
- [ ] Reward claims and completion transitions have deterministic rules.

**Presentation review**

- [ ] Clues are supported by actual facts.
- [ ] Text does not promise an unimplemented interaction.
- [ ] Warning precedes the intended commitment point.
- [ ] The escalation teaches something relevant.
- [ ] Neighboring arcs are not near-identical.
- [ ] Text fits the interface and tone.

Automated validation establishes legality, not fun. Human review and actual play remain required.

## Optional model review

The existing design uses a separate judging model for names and sprites. The same pattern may help flag
contradictory or repetitive arc prose, but:

- Core validation remains mandatory.
- A judging model cannot approve an illegal mechanic.
- A model score is not calibrated evidence that a boss is enjoyable.
- Review and retry costs count toward the same explicit spending policy.
- Provider model names and capabilities must be checked when implementation begins.

Do not copy pricing, quotas, or API behavior from old design notes without verifying them against current
service documentation.

For development-time system questions, use the separate
[Jev review protocol and question bank](../review/README.md). It permits extensive structured interrogation of
tracks, weaves, identity, pacing, and replayability. That is a different job from reviewing one generated arc.

## Runtime pipeline

Selected future flow:

```text
eligible unrevealed site
  -> bounded factual snapshot
  -> durable budget reservation
  -> queued request
  -> draft received
  -> schema and semantic validation
  -> accepted candidate
  -> reveal-time commitment
  -> immutable arc definition plus mutable runtime state
```

Remote generation should run ahead of possible discovery, within a small speculative budget. It must not
generate the entire unexplored world.

If a player reaches a site before a valid draft is available, the engine commits an offline fallback.
The pending remote result may be discarded or retained only for diagnostics; it cannot later replace the
revealed mechanics.

Names and art may still receive presentation-only improvements under their own rules. Those replacements
cannot alter clue meaning, mechanical identity, or reward promises.

## Failure, retries, and cost

- One accepted arc definition per site revision.
- Bounded request size, response size, attempts, and total service spending.
- An invalid response is rejected with structured reasons.
- No automatic endless "repair until valid" loop.
- Retry only within the reserved budget and a still-eligible site revision.
- Cancellation, timeout, malformed output, refusal, and service outage all leave offline play available.
- Persist request identity and budget state before dispatch.
- Use provider idempotency only if the provider actually supports it.
- If a submission result is ambiguous after a crash, do not assume it was uncharged and submit again.
- Local activation and reward claims are idempotent even when remote delivery is duplicated.

"Exactly once" is a local acceptance and state-transition property. It is not a promise that an arbitrary
external provider offers exactly-once billing or execution.

## Determinism and world history

Two newly generated worlds with the same seed may accept different remote arc drafts. That is expected once
external content becomes a recorded input.

The replay contract is:

> The same rules version, seed, initial state, accepted external-content records, and action history produce
> the same gameplay state.

Request completion timing can determine whether an unrevealed site commits a draft or a fallback. Record that
choice before it affects play. A saved world never redrafts the choice on load.

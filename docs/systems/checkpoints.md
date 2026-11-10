# Implementation checkpoints

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Validation](validation.md)

These checkpoints are delivery gates, not a calendar or a claim that the features exist. All implementation
checklists begin unchecked. Mark work complete only after its code, real entry points, and required playtests
support the claim.

## Dependency order

```text
CP0 -> CP1 -> CP2 -> CP3 -> CP4 -> CP5 -> CP6 -> CP7
                                                |
                                                +-> CP8 -> CP9
                                                |
                                                +-> CP10

CP9 + CP10 -> CP11
```

CP10 can proceed after CP7 using only the ecology and capabilities already implemented. It cannot draft
unimplemented powers just because they appear elsewhere in the design.

## Stage 1: make one encounter interesting

Detailed work and acceptance checklists: [CP0–CP2](checkpoints-combat.md).

| Gate | Purpose | Critical evidence |
| --- | --- | --- |
| CP0 | Freeze a bounded first slice | A small set of distinct disciplines, actions, and test identities |
| CP1 | Shared action and interaction rules | Preview, cost, timing, and resolution agree through the real app |
| CP2 | Tactical synergy | Different approaches work; an unlisted legal composition needs no special recipe |

Do not require all physical and magical disciplines before testing a few useful interactions.

## Stage 2: make a character and a journey matter

Detailed work and acceptance checklists: [CP3–CP7](checkpoints-development.md).

| Gate | Purpose | Critical evidence |
| --- | --- | --- |
| CP3 | Earned, committed identity | Actual practice creates different characters; no grinding or accidental lock-in |
| CP4 | Persistent character and world | Save/load preserves commitments, resources, and learning claims |
| CP5 | A complete worthwhile journey | Preparation, treasure, gathering, healing, and recovery form a usable loop |
| CP6 | Authored buildup and apex | Peons demonstrate power; a coherent apex demands a plan |
| CP7 | Deepening identity and replayability | Requested weaves work, unforeseen builds emerge, and players want another character |

Use developer fixtures to test advanced mechanics, but fresh-character journeys to test advancement.
One cannot stand in for the other.

## Stage 3: make the wider world deepen the game

Detailed work and acceptance checklists: [CP8–CP11](checkpoints-world.md).

| Gate | Purpose | Critical evidence |
| --- | --- | --- |
| CP8 | Life becomes recorded history | Neighbor seeding, catch-up, persistence, and cache behavior agree |
| CP9 | Biological capability diversity | New traits change decisions and have visible, ecological provenance |
| CP10 | Validated remote drafting | Legal, persistent arcs; offline play; bounded spending; no build-tailored counters |
| CP11 | Integrated long-term validation | Distinct powerful identities, compelling apex encounters, replayability, measured costs |

## Evidence record for a completed gate

Keep the record with the implementation change or a linked playtest report:

- Rules and save versions.
- Relevant seed, encounter, and character histories.
- Tests and actual commands run.
- Which checkboxes the evidence supports.
- Which checks failed, were skipped, or remain unmeasured.
- Playtest observations, including counterexamples to the intended experience.
- Worst-case measurements for any applicable resource gate.
- Remaining decisions and the next smallest playable step.

A Jev verdict can motivate a test. It cannot complete a playtest checkbox. A feature running once cannot
complete an unmeasured performance gate.

## Scope discipline

After each gate, ask:

> Does the next change improve the next playable experience, or only expand the imagined final game?

Preserve lawful, surprising player combinations. Fix broken contracts and universal cost-free strategies,
not every result outside the original fixture list.

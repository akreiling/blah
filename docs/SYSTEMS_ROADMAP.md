# Growing Into Power: design index

**Status:** proposed long-term systems and delivery plan. A narrow [composition proof](systems/composition-poc.md)
is now executable; the full progression system is not.
**Baseline:** 2026-11-09, repository revision `0b82077`.

> The world changes what you do. What you do changes who you become.

The character begins with broad potential, grows into a committed identity, and spends the endgame
refining real power. Creatures and exceptional foes have their own paths to power. Ordinary enemies can
become trivial; a major encounter builds toward an apex that demands a plan.

**Replayability:** "I want to try this next."

**Emergence:** players should discover effective builds we never envisioned. We author the building blocks
and their laws, not an exhaustive menu of legal builds.

The detailed material is split into focused documents. Implementation checklists are intentionally unchecked.
Checked items in the decisions document record agreed direction, not completed features.

## Start here

1. [Vision, inspirations, and game loops](systems/vision.md)
2. [Committed decisions and prototype assumptions](systems/decisions.md)
3. [Current implementation baseline](systems/baseline.md)
4. [Implementation checkpoints](systems/checkpoints.md)
5. [Play the composition proof](systems/composition-poc.md)

## Character and skill systems

| Document | Covers |
| --- | --- |
| [Playable composition proof](systems/composition-poc.md) | Dagger/stealth/burst synergy, costly staff fire, controls, and current limits |
| [Visual skill-web example](systems/skill-web-example.md) | A diagram of practice, weaving, lasting choices, and extraordinary refinement |
| [Character development](systems/character.md) | Potential, foundations, irreversible commitments, specialization, godlike refinement |
| [Techniques](systems/techniques.md) | Composable skill structure, legality, acquisition, starter actions |
| [Physical and magical disciplines](systems/disciplines.md) | Distinct tracks with their own practices, constraints, and connections |
| [Weaving and emergence](systems/weaving.md) | Cross-track interactions, player-created compositions, unforeseen builds, lawful limits |
| [Learning](systems/learning.md) | Use-based growth, meaningful evidence, anti-grind rules, integration practice |
| [Build diversity and replayability](systems/builds.md) | Example identities, poison-stealth mage, warded juggernaut, area frost, next-character appeal |
| [Actions and recovery](systems/actions.md) | Turn resolution, Focus, stealth, healing, camping, death |

The example builds are tests of the system, not classes. Physical/physical, magical/magical, and
physical/magical combinations must all have interesting possibilities.

## World, rewards, and encounters

| Document | Covers |
| --- | --- |
| [Equipment and treasure](systems/equipment.md) | Loadouts, relics, gathering, rewards, resource ownership |
| [Loadout properties](systems/loadout-properties.md) | Mass, noise, and material deciding what each loadout can do; backstab, cleave, magic and armor |
| [Ecology and enemy power](systems/ecology.md) | Inherited potential, individual development, persistent life, no player-level scaling |
| [Encounter arcs](systems/encounters.md) | Peon dominance, warning, escalation, exceptional apex foes, persistent site state |
| [OpenRouter arc drafting](systems/arc-generation.md) | Factual snapshots, structured drafts, validation, budgets, offline fallbacks |
| [Worked reservoir encounter](systems/reservoir-example.md) | One concrete arc, apex cycle, different build approaches, acceptance checklist |

## Delivery and verification

| Document | Covers |
| --- | --- |
| [Interface](systems/interface.md) | Development previews, targeting, creature knowledge, readable threats |
| [Architecture and persistence](systems/architecture.md) | Core authority, state ownership, save versions, deterministic replay |
| [Checkpoint overview](systems/checkpoints.md) | Dependencies and links to staged work |
| [CP0–CP2: combat foundation](systems/checkpoints-combat.md) | Bounded first slice, common action rules, tactical synergy |
| [CP3–CP7: development and a complete game loop](systems/checkpoints-development.md) | Earned identities, saves, journeys, apex encounters, power contrast |
| [CP8–CP11: living world and generated arcs](systems/checkpoints-world.md) | Persistent ecology, capability diversity, remote drafting, integrated validation |
| [Validation and playtesting](systems/validation.md) | Correctness matrices, lifecycle cases, emergence tests, real-use measurement |
| [Risks and unresolved tuning](systems/risks.md) | Failure signals, deferred features, decisions that need evidence |
| [Jev review plan](review/README.md) | A structured 120-question bank, review protocol, evidence handling |

## Immediate next work

Implement CP0, CP1, and CP2 first, not the entire design:

- [ ] Select a few useful components and shared interaction rules.
- [ ] Specify action timing, costs, and observer behavior.
- [ ] Build the close-range blaster, shuriken specialist, and battlemage thief as developer fixtures.
- [ ] Test different approaches to the same encounter.
- [ ] Test a legal combination that is not in the authored example list.
- [ ] Prove tactical synergy before implementing a long progression curve.

**Status, revision after `4c67b0b`:** the composition proof, practiced weapon, Focus, and stealth skills, six
practiced-rank moves, and the pacing and paired-branch probes exist. The first CP4 save slice now resumes a
character from SQLite after every turn ([details](systems/architecture.md#implemented-first-slice)).
The first CP3 [commitment slice](systems/character.md#implemented-cp3-commitment-slice) adds skilled-rank
offers, two permanent adaptation slots, explicit confirmation, and version-1 save migration. The
checklists stay unchecked by convention.

The CP5 first-journey slice now includes deterministic guarded caches and ruins, owned weapon variants,
landmarks, relics, exposed camping and salves, and snapshot-v5 migration. Its implementation status and
remaining gate evidence are recorded in [CP5](systems/checkpoints-development.md#cp5-complete-one-worthwhile-journey);
the [whole-journey pass](systems/journey-playtest.md) records public-action routes, tuning, and remaining
limits. CP5 checklists remain unchecked by convention and do not imply human playtesting is complete.

The first CP6 slice adds one authored arc, [Cinder Watch](systems/encounters.md#implemented-cp6-slice-cinder-watch),
with durable sentries, a telegraphed fire cycle, braziers, a stealth objective, retreat, and a restored
spring, saved in snapshot v6.

Then grow those identities through practice and commitment, preserve them in saves, and give them a
worthwhile journey with treasure, recovery, and an authored apex encounter.

The poison-stealth mage, raging warded juggernaut, and area-of-effect frost mage are explicit expansion
targets. They do not require completing every discipline before the first playable test.

## Relationship to the original design

[DESIGN.md](DESIGN.md) remains the overview and reference for terrain, regions, the original playable slice,
and the earlier art pipeline. This document set supersedes its older progression, difficulty, and encounter
assumptions.

The original slice remains accurately documented as independent regional evolution with fixed wanderer
stats, bump combat, no save system at that time, and no implemented remote clients. Proposed features are not claimed
as shipped.

The original roadmap was documentation-only. The separate composition proof now implements a small
interaction kernel; no paid generation or external posting was performed for it.

# Current implementation baseline

> Historical blah baseline, not current Isle behavior. The original window has been removed.
> Shared core systems remain; see [Isle](../ISLE.md) for the current window and rules.

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Code-inspected baseline at `0b82077`; proposed features are listed separately.

This inventory is a code-inspected baseline, not a completion checklist for the future system.

**Later delta:** the [playable composition proof](composition-poc.md) adds a bounded technique grammar,
prototype equipment, Focus, simple observer tracking and sneaking, a keyboard workshop, and costly area
fire. [Weapon practice](learning.md#implemented-prototype-weapon-practice) adds word-only weapon ranks
earned from landed strikes, and [Focus and stealth practice](learning.md#implemented-prototype-focus-and-stealth-practice)
adds ranks for those too. The table below remains the historical `0b82077` baseline. Other progression,
saves, AI naming, and Jev integration are not part of that historical baseline.

**CP5 delta:** deterministic guarded caches, durable one-time claims, owned weapon variants with safe-point
equipment, exposed camping, salves, and snapshot-v3 migrations are now implemented. See
[equipment](equipment.md), [recovery](retreat-recovery.md), and [architecture](architecture.md). This does
not claim CP5's full delivery gate.

| Area | Implemented at the baseline | Not implemented |
| --- | --- | --- |
| World | Seed-based terrain, biomes, regions, region cache | Persistent landmarks and mutable terrain overlays |
| Life | Eight genes, eight clades, region-specific evolved pools, phenotype-derived sprites | Neighbor seeding, ongoing drift, lineage history, individual mastery |
| Character | One wanderer with fixed combat stats and hit points | Foundations, skills, techniques, Focus, inventory, progression |
| Actions | Step, bump attack, wait | Targeted magic, ranged attacks, stealth, gathering, active defense |
| Creatures | Aggressive, territorial, and skittish behavior; energy-based action frequency | Suspicion, investigation, advanced telegraphs, boss phases |
| Information | Nearby species descriptions, recent-region comparisons, combat log | Durable bestiary, inspect targeting, development UI |
| Recovery | One hit point every six turns; death advances time and returns home | Resource-based healing and camp system |
| Encounters | Transient monsters spawned around the wanderer | Durable encounters, treasure, buildup, apex runtime |
| Persistence | No world save implementation | Save/load, migrations, durable learning and reward ledgers |
| Remote services | Design notes only | Remote clients, generation queue, arc planner, budget ledger |

## Existing implementation points

- [`Game`](../../crates/core/src/game.rs) owns stepping, waiting, combat, spawn/despawn, death, comparison history,
  and cached regional life.
- [`Wanderer` and `Monster`](../../crates/core/src/monster.rs) hold combatant state. Wanderer stats are currently
  fixed constants.
- [`Phenotype`](../../crates/core/src/phenotype.rs) derives stats, temperament, sensing distance, and terrain
  access from the genome.
- [`Genome`](../../crates/core/src/genome.rs) currently has eight genes.
- [`RegionLife`](../../crates/core/src/life.rs) currently grows independent pools from `(seed, region)`, with
  24 representative genomes per pool and 32 generations.
- [`Biome`](../../crates/core/src/biome.rs) owns climate, fitness data, terrain properties, and productivity.
- The former `crates/app` owned input, GPUI rendering, and displayed sprite images; it has been removed.
- [`justfile`](../../justfile) retains `test`, `lint`, `map`, and `bestiary`; `isle` launches the current window.

## Baseline evidence

Earlier in this design session, before these documentation changes:

- `mise exec -- just test` passed 107 tests, including the doctest.
- `mise exec -- just lint` passed formatting and Clippy.
- `mise exec -- cargo test -p blah-core --release pinned_to_exact_bits` passed both selected tests.
- The bestiary contact sheet was generated and visually inspected.
- The live GPUI application was not exercised because the desktop driver was unavailable.
- Cargo reported future-compatibility warnings in dependencies `block` and `proc-macro-error2`.

These results are a historical baseline, not evidence that any proposed system works. No performance target
or cross-platform guarantee in this roadmap has yet been validated for the proposed implementation.

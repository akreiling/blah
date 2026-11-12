# Architecture, persistence, and determinism

> Retained blah architecture research. The SQLite window adapter described below was
> removed with the old frontend. Shared core snapshots remain; [Isle](../ISLE.md) uses
> its own deterministic rules and JSON saves.

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Keep the pure core

`blah-core` remains the authority for:

- Action legality and resolution.
- Character development and learning.
- Technique composition and capability evaluation.
- Awareness and creature behavior.
- Encounter validation and runtime transitions.
- Rewards, resource ownership, and terrain overlays.
- Ecology and world history transitions.

It does no filesystem I/O, network I/O, wall-clock reads, or provider work.

The app owns presentation and input. Persistence and generation adapters serialize or submit core-owned
data; they do not independently rederive gameplay rules.

## Evolve modules when their callers exist

Likely responsibilities, not a mandate to create all files immediately:

| Responsibility | Initial integration |
| --- | --- |
| Action query and execution | Extend `Game` through a shared action API |
| Development and practice | Replace fixed wanderer-only assumptions with character state |
| Techniques and effects | A closed catalog and deterministic evaluator in core |
| Awareness | Extend monster state and behavior with observer-specific evidence |
| Items and loadouts | Core-owned inventory and equipment rules |
| Encounters | Durable definitions and runtime state separate from nearby transient spawns |
| Saves | An I/O adapter around versioned core snapshots |
| Drafting | A future `gen` adapter behind fixture-capable interfaces |

Do not introduce a generic ECS, scripting engine, large event bus, or configurable rules platform merely to
anticipate the full document.

## One action vocabulary

The future public interface should express actions such as step, use technique, interact, gather, and camp,
with one query/resolve contract.

Preserve existing callers while migrating:

- `Game::step` delegates to the same rules as the new action API.
- `Game::wait` advances through the same resolution path.
- `Game::can_step` and targeting previews share legality rules rather than duplicating them in GPUI.
- The existing autoplay aid either uses the new action interface or remains explicitly a basic-movement
  diagnostic.

Structured resolution events should carry facts for the view, learning, and tests. Human log text is not
an input to rules.

## State ownership

| State | Authority | Persistence |
| --- | --- | --- |
| Base terrain | Seed plus terrain generator version | Save seed and version, not every tile |
| Terrain changes | World overlay records | Save persistent overlays and transient effect state as needed |
| Regional life after history is enabled | Saved regional populations and catch-up rules | Save populations, lineage links, and simulation time |
| Character development | Character state | Save evidence aggregates, commitments, and learned techniques |
| Learning opportunities | Stable activity/encounter records | Save spent allowances and completion identity |
| Technique definitions | Versioned authored catalog plus validated learned variants | Save stable references and variant parameters |
| Inventory | Character/world item ownership | Save item identity, location, charges, and equipment state |
| Encounter definition | Accepted validated world record | Save definition, versions, and provenance |
| Encounter progress | Runtime state | Save phase, actors, objectives, mechanisms, and reward claims |
| Remote requests | Generation adapter ledger | Save status, reservation, request identity, and bounded diagnostics |
| Render caches | Frontend | Regenerate; never gameplay authority |

Avoid a second copy of a monster's or item's mutable state inside the encounter record. The encounter refers
to stable owned entities; loading creates views of the same state.

## Save format and versioning

Start with one SQLite save per character/world, consistent with the older design's storage choice.

Record:

- Save schema version.
- Terrain generator and simulation rules versions.
- Seed and game time.
- Character identity, development, inventory, Health, and Focus.
- Current position, home, and supported recovery locations.
- Durable site, creature, item, and reward identities.
- Relevant live effect and pending-action state.
- Regional history when implemented.
- Accepted external content and its catalog/schema versions.

Changing a biome table can change terrain, region IDs, and encounter placement even though terrain is not
stored tile-by-tile. Never silently load an old save under incompatible generation rules.

Until a migration exists, reject an unsupported save with a clear explanation and leave the original intact.
An overall package version alone is not a sufficient compatibility contract.

## Implemented first slice

`blah-core` exposes `Game::snapshot` and `Game::from_snapshot`. A `GameSnapshot` carries snapshot, terrain,
and rules versions (snapshot 6, terrain and rules 1), the seed, turn, position, home, visited regions, log,
the wanderer's Health, Focus, practice, permanent adaptations and burst-release history, nearby creature
state, the prepared composition, owned and equipped weapon items, salves, and durable cache claims and guard
state, herb counts, draughts, recent patch-pick turns, landmark-use history, relic ownership, and two
attunement slots, and the Cinder Watch encounter record (definition, phase, sentry identities and cycles,
braziers, claims, spring use, and retreats). Herb sites, cache positions, field-sign destinations, and weapon rewards are
regenerated from the seed and region; region lives, labels, clades,
and sprites are also regenerated. Loading checks versions and value ranges and rejects anything else.

Version-1 through version-5 snapshots migrate to version 6; all of them start with no charted encounter. Migration from v1/v2 preserves existing practice and
commitments, grants the four-item starting kit and two salves, equips the plain item matching the prepared
weapon family, and starts with no cache claims. Version 1 also receives the version-2 adaptation defaults.
Version 3 keeps its inventory, equipment, salves, claims, and guards. Versions 1–3 start with empty herb
pouches and no draughts or picks; v4 keeps its supplies and picks. Versions 1–4 start with no Herbalism
practice, relics, or landmark-use history. Version 5 validates capacity, slot order, exclusions,
adaptation history, owned equipment, cache identity, guard state, supply caps, and unique recent picks at
real herb sites. Version 6 also rejects an encounter whose definition differs from the one re-derived
from the seed and home, or whose sentries, phase, claims, and timestamps disagree with the saved creatures.
Only unregrown picks are authoritative; visible herb sites and cache-trail destinations can be evicted
freely. Loading regenerates field signs without spawning guards or committing new encounters. Commitment
is a no-time action through the same transactional save path; a failed write rolls it back.

The app stores the snapshot as JSON in a single-row SQLite table (`PRAGMA user_version = 1`) and rewrites it
in one transaction after every action, under `synchronous = FULL`. A row revision makes a second running
session's write fail rather than overwrite. If a write fails, the in-memory action is undone and the title
bar shows the error. A turn's snapshot is under 9 KB and the file stays near 20 KB; a test bounds both.

`blah` resumes the newest `character-NNNNNN.sqlite3` in `$XDG_DATA_HOME/blah/saves`,
or `~/.local/share/blah/saves` when that is unset, on every platform. `--new` creates the next numbered file and never
replaces one; `--save PATH` names a file. An explicit seed that differs from the save's is an error.
Developer demos are never saved. Missing, corrupt, and unsupported files are reported and left untouched.
`blah --list` prints each character's number, seed, turn, deaths, and path, opening the files read-only.

An action that leaves the snapshot byte-for-byte unchanged writes nothing. Refused actions still write,
because their explanation is a log line and the log is saved. Rollback keeps the pre-action snapshot rather
than cloning the whole game, and restores from it on the rare failed write.

## Atomicity and replay

An action boundary must not save half a reward claim or half an adaptation commitment.

At minimum:

- Costs, effects, learning credit, and resulting state belong to one coherent transition.
- Reward claims and item grants commit atomically.
- Site definition acceptance and its revision lock commit atomically.
- Load cannot apply a completed action twice.
- Save interruption leaves the previous committed state usable.

Deterministic streams remain purpose-keyed. New random draws for loot, learning variation if any, or
encounters must not shift unrelated movement and combat streams.

Stable iteration order, stable IDs, and supported arithmetic remain part of the core contract. Exact-bit
tests must be reviewed when mechanics intentionally change, not blindly updated to silence failures.

## Working-set limits

An endless world requires bounded active work:

- Load nearby gameplay records and relevant encounter state, not the entire save.
- Keep cache eviction separate from deletion of recorded facts.
- Aggregate practice instead of retaining an unbounded action transcript in memory.
- Keep a bounded queue of prospective remote sites.
- Bound effects, projectiles, summons, and reactions per action.
- Catch up ecology in a cost-bounded way with defined semantics.

Catch-up approximations must be tested for their stated invariants and accuracy on meaningful scenarios.
Do not silently discard elapsed time or important population events merely to meet a latency target.

## Validation commands

Existing commands:

```sh
cd /Users/andy/projects/blah
mise exec -- just test
mise exec -- just lint
mise exec -- cargo test -p blah-core --release pinned_to_exact_bits
mise exec -- just isle --seed 42
```

For visual evolution checks, choose a workspace output path:

```sh
cd /Users/andy/projects/blah
mise exec -- just bestiary /Users/andy/projects/blah/target/bestiary-check.png 42
```

Future test harnesses and benchmark commands must be added when implemented. Do not document imaginary
commands as if they already exist.

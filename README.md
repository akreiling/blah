# Isle

A solo, turn-based island adventure inspired by *Island of Kesmai*, with generated towns,
endless Deeps, procedural terrain, and genome-shaped creatures. Practice earns power.
Explore cairns, abandoned camps, and ruined shrines for one-time caches. Gather biome-specific herbs
for exposed field recovery. The world waits while you type.

## Play

Install [mise](https://mise.jdx.dev/), then run:

```sh
mise install
mise exec -- just isle
```

The GPUI window uses Metal on macOS; building there requires Xcode.
Use `just isle --new` for a fresh character, or if an older save no longer loads.
See [the Isle guide](docs/ISLE.md) for commands, rules, saves, and window tests.

## Develop

- `crates/core` (`blah-core`): shared terrain, genetics, evolution, procedural sprites,
  and the retained systems prototype.
- `crates/isle-core`: deterministic Isle rules and serializable game state.
- `crates/isle`: the GPUI window and local control socket.

```sh
mise exec -- just lint
mise exec -- just test
mise exec -- cargo build -p isle
python3 crates/isle/scripts/smoke.py
```

`just map` and `just bestiary` inspect shared terrain and evolved life.
`just composition-demo`, `just skill-pacing`, and `just journey-playtest` exercise the
retained core prototype, not Isle's current progression rules. `just jev-review` is an
optional, paid design-review tool; it is never part of the game or tests.

The old `blah` window and `just run` have been removed. Its
[design overview](docs/DESIGN.md) and [systems research](docs/SYSTEMS_ROADMAP.md)
remain as historical research for the merge, not a description of the current Isle UI.
